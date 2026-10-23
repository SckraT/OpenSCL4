//! Talks to the library on a background runtime. The window sends commands and is told what the device and the
//! database say back. The device is the source of truth: the window shows its settings as they come back, never what
//! it asked for.

use std::{path::PathBuf, sync::Arc};

use macaddr::MacAddr6;
use openscq30_lib::{
    DeviceModel, OpenSCQ30Session,
    connection::{self, ConnectionDescriptor, SessionFailureReason, SessionStatus},
    device::{self, OpenSCQ30Device, SettingWriteOutcome, WriteOutcome},
    quick_presets::{QuickPresetActivation, QuickPresetsHandler},
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
    /// Read the settings again, for their names and values in another language.
    Reread,
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
    pub last_applied_preset: Option<String>,
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
    #[cfg(target_os = "linux")]
    TrayStatus(crate::linux_tray::TrayStatus),
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
    ConnectionLost {
        mac_address: MacAddr6,
        reason: SessionFailureReason,
    },
    /// The transport dropped but the session is reconnecting; the last snapshot stays on screen.
    Recovering {
        mac_address: MacAddr6,
        reason: SessionFailureReason,
    },
    ConnectFailed {
        mac_address: MacAddr6,
        failure: Failure,
    },
    /// The command did nothing; what the device has is unchanged.
    CommandFailed {
        command: Command,
        failure: Failure,
    },
    /// The write was sent, but the device did not confirm every requested setting.
    SettingsNotApplied {
        command: Command,
        outcomes: Vec<SettingWriteOutcome>,
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
    Recovering(u64, SessionFailureReason),
    ConnectionLost(u64, SessionFailureReason),
    /// Let go of the device and stop, then say so.
    Shutdown(tokio::sync::oneshot::Sender<()>),
}

pub struct Controller {
    messages: mpsc::UnboundedSender<Message>,
    runtime: tokio::runtime::Runtime,
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
        let emit: Arc<dyn Fn(Event) + Send + Sync> = Arc::new(emit);
        #[cfg(target_os = "linux")]
        {
            let emit = emit.clone();
            runtime.spawn(crate::linux_tray::watch(move |status| {
                emit(Event::TrayStatus(status))
            }));
        }
        let actor = Actor {
            presets: session.quick_preset_handler(),
            session,
            config_dir,
            emit,
            messages: messages.clone(),
            generation: 0,
            connecting: None,
            device: None,
            watchers: Vec::new(),
            paired: Vec::new(),
            preset_list: Vec::new(),
            preset_activation: None,
        };
        runtime.spawn(actor.run(inbox));
        Ok(Self { messages, runtime })
    }

    pub fn send(&self, command: Command) {
        if self.messages.send(Message::Command(command)).is_err() {
            tracing::error!("the controller has stopped");
        }
    }

    /// Disconnects and stops, waiting a little at most. Dropping the runtime instead would wait for every blocking
    /// task, and the Windows Bluetooth backend can keep one blocked on a connected device for as long as it stays
    /// connected, so the app would never end.
    pub fn shutdown(self) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        let (done, stopped) = tokio::sync::oneshot::channel();
        if self.messages.send(Message::Shutdown(done)).is_ok() {
            let waited = self.runtime.block_on(async {
                tokio::time::timeout(std::time::Duration::from_secs(1), stopped).await
            });
            if waited.is_err() {
                tracing::warn!("the controller did not stop in time");
            }
        }
        self.runtime
            .shutdown_timeout(deadline.saturating_duration_since(std::time::Instant::now()));
    }
}

struct Actor {
    session: Arc<OpenSCQ30Session>,
    presets: QuickPresetsHandler,
    config_dir: PathBuf,
    emit: Arc<dyn Fn(Event) + Send + Sync>,
    messages: mpsc::UnboundedSender<Message>,
    /// Counts connection attempts, so a late answer for an earlier one is ignored.
    generation: u64,
    connecting: Option<(MacAddr6, JoinHandle<()>)>,
    device: Option<(MacAddr6, Device)>,
    watchers: Vec<JoinHandle<()>>,
    paired: Vec<PairedDevice>,
    preset_list: Vec<QuickPreset>,
    preset_activation: Option<QuickPresetActivation>,
}

impl Actor {
    async fn run(mut self, mut inbox: mpsc::UnboundedReceiver<Message>) {
        self.refresh_paired().await;
        while let Some(message) = inbox.recv().await {
            match message {
                Message::Command(command) => self.handle(command).await,
                Message::Shutdown(done) => {
                    if let Some((_, task)) = self.connecting.take() {
                        task.abort();
                    }
                    // the connection closes its socket when dropped, which ends the backend's reading thread
                    drop(self.drop_device());
                    let _ = done.send(());
                    return;
                }
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
                Message::Recovering(generation, reason) if generation == self.generation => {
                    if let Some((mac_address, _)) = self.device.as_ref() {
                        (self.emit)(Event::Recovering {
                            mac_address: *mac_address,
                            reason,
                        });
                    }
                }
                Message::ConnectionLost(generation, reason) if generation == self.generation => {
                    if let Some((mac_address, _)) = self.drop_device() {
                        (self.emit)(Event::ConnectionLost {
                            mac_address,
                            reason,
                        });
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
            Command::Reread => self.emit_snapshot(false),
            Command::SetSettings(values) => {
                let Some(device) = self.current() else { return };
                match device.set_setting_values(values).await {
                    Ok(outcomes) => {
                        // The device is the source of truth for what it applied, even when it applied nothing.
                        self.emit_snapshot(false);
                        let outcomes = not_applied(outcomes);
                        if !outcomes.is_empty() {
                            (self.emit)(Event::SettingsNotApplied {
                                command: command.clone(),
                                outcomes,
                            });
                        }
                    }
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
                let Some((mac_address, _)) = self.device.clone() else {
                    return;
                };
                let Some(activation) = &self.preset_activation else {
                    return;
                };
                match activation.activate(&self.presets, name.clone()).await {
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
        let mut status = device.session_status();
        self.watchers.push(tokio::spawn(async move {
            loop {
                match status.borrow_and_update().clone() {
                    SessionStatus::Recovering { reason, .. } => {
                        if messages
                            .send(Message::Recovering(generation, reason))
                            .is_err()
                        {
                            break;
                        }
                    }
                    SessionStatus::Lost { reason } => {
                        let _ = messages.send(Message::ConnectionLost(generation, reason));
                        break;
                    }
                    SessionStatus::Closed => break,
                    _ => {}
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
        self.preset_activation = Some(QuickPresetActivation::new(device.clone()));
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
        self.preset_activation = None;
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
            last_applied_preset: self
                .preset_activation
                .as_ref()
                .and_then(QuickPresetActivation::last_applied),
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

/// Only the outcomes that must be reported to the user, in request order.
fn not_applied(outcomes: Vec<SettingWriteOutcome>) -> Vec<SettingWriteOutcome> {
    outcomes
        .into_iter()
        .filter(|outcome| !matches!(outcome.outcome, WriteOutcome::Applied { .. }))
        .collect()
}

fn failure(err: &device::Error) -> Failure {
    use connection::Error as Connection;
    match err {
        device::Error::ConnectionError {
            source: Connection::BluetoothAdapterUnavailable { .. },
            ..
        } => Failure::BluetoothOff,
        device::Error::ConnectionError {
            source:
                Connection::DeviceNotFound { .. }
                | Connection::Unpaired { .. }
                | Connection::PermissionDenied { .. }
                | Connection::SecurityFailure { .. },
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        sync::mpsc as sync_channel,
        time::{Duration, Instant},
    };

    #[test]
    fn preset_activation_lives_in_the_connection_snapshot() {
        let dir = tempfile::tempdir().unwrap();
        let (emit, events) = sync_channel::channel();
        let controller = Controller::start(
            dir.path().join("database.sqlite"),
            dir.path().to_path_buf(),
            move |event| {
                let _ = emit.send(event);
            },
        )
        .unwrap();
        let model = DeviceModel::SoundcoreA3954;
        let address = model.demo_mac_address();
        controller.send(Command::Pair(PairedDevice {
            mac_address: address,
            model,
            is_demo: true,
        }));
        controller.send(Command::Connect(address));
        let next_snapshot = |matches: &dyn Fn(&DeviceSnapshot) -> bool| {
            let deadline = Instant::now() + Duration::from_secs(10);
            loop {
                match events
                    .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                    .unwrap()
                {
                    Event::Connected(snapshot) | Event::Changed(snapshot) if matches(&snapshot) => {
                        break snapshot;
                    }
                    Event::CommandFailed { command, failure } => panic!("{command:?}: {failure:?}"),
                    _ => (),
                }
            }
        };
        assert_eq!(next_snapshot(&|_| true).last_applied_preset, None);
        controller.send(Command::SavePreset("First".into()));
        next_snapshot(&|s| !s.presets.is_empty());
        controller.send(Command::TogglePresetField {
            name: "First".into(),
            setting_id: SettingId::WearingDetection,
            enabled: true,
        });
        next_snapshot(&|s| s.presets[0].fields.iter().any(|field| field.is_enabled));
        controller.send(Command::ActivatePreset("First".into()));
        next_snapshot(&|s| s.last_applied_preset.as_deref() == Some("First"));
        controller.send(Command::Disconnect);
        controller.send(Command::Connect(address));
        let reconnected = next_snapshot(&|s| s.last_applied_preset.is_none());
        assert_eq!(reconnected.presets.len(), 1);
        assert_eq!(reconnected.presets[0].name, "First");
        controller.shutdown();
    }

    #[test]
    fn only_outcomes_that_were_not_applied_are_reported() {
        let outcomes = vec![
            SettingWriteOutcome {
                setting_id: SettingId::WearingDetection,
                outcome: WriteOutcome::Applied {
                    observed: Value::Bool(true),
                },
            },
            SettingWriteOutcome {
                setting_id: SettingId::WearingTone,
                outcome: WriteOutcome::Rejected {
                    observed: Some(Value::Bool(false)),
                },
            },
            SettingWriteOutcome {
                setting_id: SettingId::TouchTone,
                outcome: WriteOutcome::TimedOut,
            },
            SettingWriteOutcome {
                setting_id: SettingId::Ldac,
                outcome: WriteOutcome::Disconnected,
            },
        ];

        let reported = not_applied(outcomes);

        assert_eq!(
            vec![
                SettingId::WearingTone,
                SettingId::TouchTone,
                SettingId::Ldac
            ],
            reported
                .into_iter()
                .map(|outcome| outcome.setting_id)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn setting_write_emits_the_confirmed_snapshot() {
        let dir = tempfile::tempdir().unwrap();
        let (emit, events) = sync_channel::channel();
        let controller = Controller::start(
            dir.path().join("database.sqlite"),
            dir.path().to_path_buf(),
            move |event| {
                let _ = emit.send(event);
            },
        )
        .unwrap();
        let model = DeviceModel::SoundcoreA3954;
        let address = model.demo_mac_address();
        controller.send(Command::Pair(PairedDevice {
            mac_address: address,
            model,
            is_demo: true,
        }));
        controller.send(Command::Connect(address));
        let next_snapshot = || {
            let deadline = Instant::now() + Duration::from_secs(10);
            loop {
                match events
                    .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                    .unwrap()
                {
                    Event::Connected(snapshot) | Event::Changed(snapshot) => break snapshot,
                    Event::SettingsNotApplied { outcomes, .. } => {
                        panic!("the demo device did not apply {outcomes:?}")
                    }
                    Event::CommandFailed { command, failure } => panic!("{command:?}: {failure:?}"),
                    _ => (),
                }
            }
        };
        next_snapshot();

        controller.send(Command::SetSettings(vec![(
            SettingId::WearingDetection,
            Value::Bool(true),
        )]));
        let snapshot = next_snapshot();
        assert!(matches!(
            snapshot.settings.get(&SettingId::WearingDetection),
            Some(openscq30_lib::settings::Setting::Toggle { value: true, .. })
        ));
        controller.shutdown();
    }

    #[test]
    fn shutdown_stops_even_when_a_blocking_task_is_stuck() {
        let dir = tempfile::tempdir().unwrap();
        let controller = Controller::start(
            dir.path().join("database.sqlite"),
            dir.path().to_path_buf(),
            |_| {},
        )
        .unwrap();
        let (started, running) = sync_channel::channel();
        let (release, blocked) = sync_channel::channel();
        controller.runtime.spawn_blocking(move || {
            started.send(()).unwrap();
            let _ = blocked.recv_timeout(Duration::from_secs(10));
        });
        running.recv_timeout(Duration::from_secs(5)).unwrap();
        let start = Instant::now();
        controller.shutdown();
        let elapsed = start.elapsed();
        let _ = release.send(());
        assert!(
            elapsed < Duration::from_millis(2500),
            "shutdown took {elapsed:?}"
        );
    }
}
