//! Talks to the library on a background runtime. The window sends commands and is told what the device and the
//! database say back. The device is the source of truth: the window shows its settings as they come back, never what
//! it asked for.

use std::{path::PathBuf, sync::Arc};

use macaddr::MacAddr6;
use openscq30_lib::{
    DeviceModel, OpenSCQ30Session,
    connection::{self, ConnectionDescriptor, ConnectionStatus},
    device::{self, OpenSCQ30Device},
    quick_presets::QuickPresetsHandler,
    settings::{CategoryId, SettingId, Value},
    storage::{PairedDevice, QuickPreset},
};
use tokio::{sync::mpsc, task::JoinHandle};

use crate::model::{
    SettingMap,
    legacy::{self, LegacyEqualizerProfile},
};

type Device = Arc<dyn OpenSCQ30Device + Send + Sync>;

#[derive(Clone, Debug)]
pub enum Command {
    /// What can be paired as the model: Windows' paired devices, or a demo device.
    ListDevices {
        model: DeviceModel,
        demo: bool,
    },
    Pair(PairedDevice),
    Unpair(MacAddr6),
    Connect(MacAddr6),
    CancelConnect,
    Disconnect,
    SetSettings(Vec<(SettingId, Value)>),
    SavePreset(String),
    TogglePresetField {
        name: String,
        setting_id: SettingId,
        enabled: bool,
    },
    ActivatePreset(String),
    DeletePreset(String),
    MigrateLegacyProfiles(Vec<(String, Vec<i16>)>),
}

/// What the device has, as read after connecting and after every change.
#[derive(Clone, Debug)]
#[expect(
    dead_code,
    reason = "the status bar is to say when the data is a demo device's"
)]
pub struct DeviceSnapshot {
    pub mac_address: MacAddr6,
    pub model: DeviceModel,
    pub is_demo: bool,
    /// In the device's order; a setting can be in several categories.
    pub categories: Vec<(CategoryId, Vec<SettingId>)>,
    pub settings: SettingMap,
    pub presets: Vec<QuickPreset>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Failure {
    BluetoothOff,
    NotFound,
    TimedOut,
    Other(String),
}

#[derive(Debug)]
pub enum Event {
    PairedDevices(Vec<PairedDevice>),
    Devices {
        model: DeviceModel,
        demo: bool,
        result: Result<Vec<ConnectionDescriptor>, Failure>,
    },
    Connecting(MacAddr6),
    Connected(Box<DeviceSnapshot>),
    Changed(Box<DeviceSnapshot>),
    /// Disconnected as asked.
    Disconnected(MacAddr6),
    /// The device went away by itself.
    ConnectionLost(MacAddr6),
    ConnectFailed {
        mac_address: MacAddr6,
        failure: Failure,
    },
    /// The command did nothing; what the device has is unchanged.
    CommandFailed {
        command: Command,
        failure: Failure,
    },
    PresetApplied {
        mac_address: MacAddr6,
        name: String,
    },
    LegacyProfiles(Vec<(String, LegacyEqualizerProfile)>),
}

enum Message {
    Command(Command),
    Connected {
        generation: u64,
        device: Device,
    },
    ConnectFailed {
        generation: u64,
        mac_address: MacAddr6,
        failure: Failure,
    },
    DeviceChanged(u64),
    ConnectionLost(u64),
}

pub struct Controller {
    messages: mpsc::UnboundedSender<Message>,
    _runtime: tokio::runtime::Runtime,
}

impl Controller {
    /// Opens the database and starts listening for commands. `emit` is called on a background thread.
    pub fn start(
        database: PathBuf,
        config_dir: PathBuf,
        emit: impl Fn(Event) + Send + Sync + 'static,
    ) -> anyhow::Result<Self> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()?;
        let session = Arc::new(runtime.block_on(OpenSCQ30Session::new(database))?);
        let (messages, inbox) = mpsc::unbounded_channel();
        let actor = Actor {
            presets: session.quick_preset_handler(),
            session,
            config_dir,
            emit: Box::new(emit),
            messages: messages.clone(),
            generation: 0,
            connecting: None,
            device: None,
            watchers: Vec::new(),
            paired: Vec::new(),
            preset_list: Vec::new(),
        };
        runtime.spawn(actor.run(inbox));
        Ok(Self {
            messages,
            _runtime: runtime,
        })
    }

    pub fn send(&self, command: Command) {
        if self.messages.send(Message::Command(command)).is_err() {
            tracing::error!("the controller has stopped");
        }
    }
}

struct Actor {
    session: Arc<OpenSCQ30Session>,
    presets: QuickPresetsHandler,
    config_dir: PathBuf,
    emit: Box<dyn Fn(Event) + Send + Sync>,
    messages: mpsc::UnboundedSender<Message>,
    /// Counts connection attempts, so a late answer for an earlier one is ignored.
    generation: u64,
    connecting: Option<(MacAddr6, JoinHandle<()>)>,
    device: Option<(MacAddr6, Device)>,
    watchers: Vec<JoinHandle<()>>,
    paired: Vec<PairedDevice>,
    preset_list: Vec<QuickPreset>,
}

impl Actor {
    async fn run(mut self, mut inbox: mpsc::UnboundedReceiver<Message>) {
        self.refresh_paired().await;
        while let Some(message) = inbox.recv().await {
            match message {
                Message::Command(command) => self.handle(command).await,
                Message::Connected { generation, device } if generation == self.generation => {
                    self.connected(device).await;
                }
                Message::ConnectFailed {
                    generation,
                    mac_address,
                    failure,
                } if generation == self.generation => {
                    self.connecting = None;
                    (self.emit)(Event::ConnectFailed {
                        mac_address,
                        failure,
                    });
                }
                Message::DeviceChanged(generation) if generation == self.generation => {
                    self.emit_snapshot(false);
                }
                Message::ConnectionLost(generation) if generation == self.generation => {
                    if let Some((mac_address, _)) = self.drop_device() {
                        (self.emit)(Event::ConnectionLost(mac_address));
                    }
                }
                // about an earlier connection
                _ => (),
            }
        }
    }

    async fn handle(&mut self, command: Command) {
        match command.clone() {
            Command::ListDevices { model, demo } => {
                let result = if demo {
                    self.session.list_demo_devices(model).await
                } else {
                    self.session.list_devices(model).await
                };
                (self.emit)(Event::Devices {
                    model,
                    demo,
                    result: result.map_err(|err| failure(&err)),
                });
            }
            Command::Pair(paired_device) => {
                if let Err(err) = self.session.pair(paired_device).await {
                    self.failed(command, &err);
                }
                self.refresh_paired().await;
            }
            Command::Unpair(mac_address) => {
                if self
                    .device
                    .as_ref()
                    .is_some_and(|(mac, _)| *mac == mac_address)
                {
                    self.drop_device();
                    (self.emit)(Event::Disconnected(mac_address));
                }
                if let Err(err) = self.session.unpair(mac_address).await {
                    self.failed(command, &err);
                }
                self.refresh_paired().await;
            }
            Command::Connect(mac_address) => self.connect(mac_address),
            Command::CancelConnect => {
                if let Some((mac_address, task)) = self.connecting.take() {
                    task.abort();
                    self.generation += 1;
                    (self.emit)(Event::Disconnected(mac_address));
                }
            }
            Command::Disconnect => {
                if let Some((mac_address, _)) = self.drop_device() {
                    (self.emit)(Event::Disconnected(mac_address));
                }
            }
            Command::SetSettings(values) => {
                let Some(device) = self.current() else { return };
                match device.set_setting_values(values).await {
                    Ok(()) => self.emit_snapshot(false),
                    Err(err) => self.failed(command, &err),
                }
            }
            Command::SavePreset(name) => {
                let Some(device) = self.current() else { return };
                if let Err(err) = self.presets.save(device.as_ref(), name).await {
                    self.failed(command, &err);
                }
                self.refresh_presets(&device).await;
            }
            Command::TogglePresetField {
                name,
                setting_id,
                enabled,
            } => {
                let Some(device) = self.current() else { return };
                if let Err(err) = self
                    .presets
                    .toggle_field(device.as_ref(), name, setting_id, enabled)
                    .await
                {
                    self.failed(command, &err);
                }
                self.refresh_presets(&device).await;
            }
            Command::ActivatePreset(name) => {
                let Some((mac_address, device)) = self.device.clone() else {
                    return;
                };
                match self.presets.activate(device.as_ref(), name.clone()).await {
                    Ok(()) => {
                        (self.emit)(Event::PresetApplied { mac_address, name });
                        self.emit_snapshot(false);
                    }
                    Err(err) => self.failed(command, &err),
                }
            }
            Command::DeletePreset(name) => {
                let Some(device) = self.current() else { return };
                if let Err(err) = self.presets.delete(device.as_ref(), name).await {
                    self.failed(command, &err);
                }
                self.refresh_presets(&device).await;
            }
            Command::MigrateLegacyProfiles(profiles) => {
                let Some(device) = self.current() else { return };
                for (name, values) in profiles {
                    if let Err(err) = legacy::migrate_profile(device.as_ref(), name, values).await {
                        (self.emit)(Event::CommandFailed {
                            command: command.clone(),
                            failure: Failure::Other(format!("{err:#}")),
                        });
                        break;
                    }
                }
                self.emit_snapshot(false);
            }
        }
    }

    fn connect(&mut self, mac_address: MacAddr6) {
        self.drop_device();
        if let Some((_, task)) = self.connecting.take() {
            task.abort();
        }
        self.generation += 1;
        let generation = self.generation;
        (self.emit)(Event::Connecting(mac_address));
        let session = self.session.clone();
        let messages = self.messages.clone();
        let task = tokio::spawn(async move {
            let message = match session.connect(mac_address).await {
                Ok(device) => Message::Connected { generation, device },
                Err(err) => Message::ConnectFailed {
                    generation,
                    mac_address,
                    failure: failure(&err),
                },
            };
            let _ = messages.send(message);
        });
        self.connecting = Some((mac_address, task));
    }

    async fn connected(&mut self, device: Device) {
        let Some((mac_address, _)) = self.connecting.take() else {
            return;
        };
        let generation = self.generation;
        let messages = self.messages.clone();
        let mut changes = device.watch_for_changes();
        self.watchers.push(tokio::spawn(async move {
            while changes.changed().await.is_ok() {
                if messages.send(Message::DeviceChanged(generation)).is_err() {
                    break;
                }
            }
        }));
        let messages = self.messages.clone();
        let mut status = device.connection_status();
        self.watchers.push(tokio::spawn(async move {
            loop {
                if *status.borrow_and_update() == ConnectionStatus::Disconnected {
                    let _ = messages.send(Message::ConnectionLost(generation));
                    break;
                }
                if status.changed().await.is_err() {
                    break;
                }
            }
        }));
        self.preset_list = self
            .presets
            .quick_presets(device.as_ref())
            .await
            .unwrap_or_else(|err| {
                tracing::error!("could not read the quick presets: {err:?}");
                Vec::new()
            });
        self.device = Some((mac_address, device));
        self.emit_snapshot(true);
        match legacy::equalizer_profiles(&self.config_dir).await {
            Ok(profiles) => (self.emit)(Event::LegacyProfiles(profiles)),
            Err(err) => tracing::warn!("could not read the v1 equalizer profiles: {err:#}"),
        }
    }

    fn current(&self) -> Option<Device> {
        self.device.as_ref().map(|(_, device)| device.clone())
    }

    fn drop_device(&mut self) -> Option<(MacAddr6, Device)> {
        for watcher in self.watchers.drain(..) {
            watcher.abort();
        }
        self.preset_list.clear();
        self.device.take()
    }

    fn emit_snapshot(&self, first: bool) {
        let Some((mac_address, device)) = &self.device else {
            return;
        };
        let by_category = device.settings_by_category();
        let snapshot = Box::new(DeviceSnapshot {
            mac_address: *mac_address,
            model: device.model(),
            is_demo: self
                .paired
                .iter()
                .any(|p| p.mac_address == *mac_address && p.is_demo),
            categories: by_category
                .iter()
                .map(|(category, settings)| (*category, settings.keys().copied().collect()))
                .collect(),
            settings: by_category.into_values().flatten().collect(),
            presets: self.preset_list.clone(),
        });
        (self.emit)(if first {
            Event::Connected(snapshot)
        } else {
            Event::Changed(snapshot)
        });
    }

    async fn refresh_presets(&mut self, device: &Device) {
        match self.presets.quick_presets(device.as_ref()).await {
            Ok(presets) => self.preset_list = presets,
            Err(err) => tracing::error!("could not read the quick presets: {err:?}"),
        }
        self.emit_snapshot(false);
    }

    async fn refresh_paired(&mut self) {
        match self.session.paired_devices().await {
            Ok(paired) => {
                self.paired = paired.clone();
                (self.emit)(Event::PairedDevices(paired));
            }
            Err(err) => tracing::error!("could not read the paired devices: {err:?}"),
        }
    }

    fn failed(&self, command: Command, err: &(dyn std::error::Error + 'static)) {
        tracing::warn!("{command:?} failed: {}", describe(err));
        let failure = err
            .downcast_ref::<device::Error>()
            .map_or_else(|| Failure::Other(describe(err)), failure);
        (self.emit)(Event::CommandFailed { command, failure });
    }
}

fn failure(err: &device::Error) -> Failure {
    use connection::Error as Connection;
    match err {
        device::Error::ConnectionError {
            source: Connection::BluetoothAdapterUnavailable { .. },
            ..
        } => Failure::BluetoothOff,
        device::Error::ConnectionError {
            source: Connection::DeviceNotFound { .. },
            ..
        }
        | device::Error::DeviceNotFound { .. } => Failure::NotFound,
        device::Error::ConnectionError {
            source: Connection::TimedOut { .. },
            ..
        }
        | device::Error::ActionTimedOut { .. } => Failure::TimedOut,
        other => Failure::Other(describe(other)),
    }
}

/// The error and everything it was caused by, for the log and the error banner.
fn describe(err: &(dyn std::error::Error + 'static)) -> String {
    let mut text = err.to_string();
    let mut source = err.source();
    while let Some(cause) = source {
        text.push_str(": ");
        text.push_str(&cause.to_string());
        source = cause.source();
    }
    text
}
