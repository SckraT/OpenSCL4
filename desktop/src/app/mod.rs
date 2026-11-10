//! The window's state and what happens when the user or the device does something. Runs on the UI thread; the
//! controller's events are brought here with `slint::invoke_from_event_loop`.

use std::{
    cell::RefCell,
    path::Path,
    str::FromStr,
    time::{Duration, Instant},
};

use i18n_embed::{DesktopLanguageRequester, unic_langid::LanguageIdentifier};
use macaddr::MacAddr6;
use openscq30_i18n::Translate;
use openscq30_lib::{
    DeviceModel,
    client::{
        availability::{SettingAvailability, setting_availability},
        controls::ControlSide,
    },
    connection::{ConnectionDescriptor, RECOVERY_WINDOW, SessionFailureReason},
    device::{SettingWriteOutcome, WriteOutcome},
    settings::SettingId,
    storage::PairedDevice,
};
use slint::{ComponentHandle, ModelRc, VecModel};

mod equalizer;
mod presets;
mod settings;
mod tray;

pub use tray::show_from_elsewhere;

use crate::{
    Actions, AppState, AppWindow, Connection as ConnectionView, FailureKind, Page, PairedOption,
    PaletteChoice, Theme, ToastKind,
    config::{Accent, AppTheme, Config, SoundModeView},
    controller::{Command, Controller, DeviceSnapshot, Event, Failure},
    model::overview::{OverviewLayout, OverviewPreset},
    model::{
        battery::is_only_one_earbud_connected,
        device_art::{EarbudsColor, art_color},
        devices::featured_device,
        languages,
        legacy::LegacyEqualizerProfile,
        sound_mode::sound_mode_scale_settings,
    },
    overview,
    rows::{self, Edit},
    system_settings, view, window_chrome,
};

enum Connection {
    Off,
    Connecting,
    Connected(Box<DeviceSnapshot>),
    Recovering(Box<DeviceSnapshot>, SessionFailureReason),
    Lost(SessionFailureReason),
    Failed(Failure),
}

/// How long a write cut off by a reconnect waits before its error is shown.
///
/// This is the shared session's recovery budget (`openscq30_lib::connection::RECOVERY_WINDOW`), which
/// is also the window Android waits out before it reports the device lost: 30 s. An LDAC switch alone
/// takes about 10 s, so reporting the write sooner would call a change failed while the session is
/// still reconnecting.
const RECONNECT_WAIT: Duration = RECOVERY_WINDOW;

/// A write the device cut off by reconnecting. It is not final: the session retries for
/// [`RECONNECT_WAIT`], and the refreshed snapshot shows what the device actually has.
#[derive(Debug)]
struct PendingWrite {
    command: Command,
    deadline: Instant,
}

impl PendingWrite {
    fn is_due(&self, now: Instant) -> bool {
        now >= self.deadline
    }
}

/// What a write the device did not apply means for the error banner.
#[derive(Debug)]
enum WriteFailure {
    /// The device answered, so the error is final.
    Now(Command),
    /// The link dropped mid-write; the error waits for the reconnect window.
    AfterReconnect(PendingWrite),
    /// Nothing to report.
    Nothing,
}

fn write_failure(command: Command, outcomes: &[SettingWriteOutcome], now: Instant) -> WriteFailure {
    let mut disconnected = false;
    let mut final_failure = false;
    for outcome in outcomes {
        match outcome.outcome {
            WriteOutcome::Applied { .. } => {}
            WriteOutcome::Disconnected => disconnected = true,
            _ => final_failure = true,
        }
    }
    if final_failure {
        WriteFailure::Now(command)
    } else if disconnected {
        WriteFailure::AfterReconnect(PendingWrite {
            command,
            deadline: now + RECONNECT_WAIT,
        })
    } else {
        WriteFailure::Nothing
    }
}

#[derive(Default)]
struct AddFlow {
    model: Option<DeviceModel>,
    demo: bool,
    loading: bool,
    found: Vec<ConnectionDescriptor>,
    bluetooth_off: bool,
    error: Option<Failure>,
    /// Pair the first device found, for OPENSCQ30_DEMO.
    auto_pair: bool,
}

pub struct App {
    window: slint::Weak<AppWindow>,
    controller: Controller,
    config: Config,
    paired: Vec<PairedDevice>,
    /// The device the window is about: connected, connecting, or the one to connect.
    current: Option<MacAddr6>,
    connection: Connection,
    /// Open the device's overview once it connects.
    open_on_connect: bool,
    failed_command: Option<Command>,
    /// A write cut off by a reconnect; its error is shown only once its window has passed.
    pending_write: Option<PendingWrite>,
    add: AddFlow,
    dark: bool,
    /// Whether Linux prefers dark, once the desktop portal has said; Windows is asked each time.
    system_dark: Option<bool>,
    title_bar: window_chrome::TitleBar,
    legacy: Vec<(String, LegacyEqualizerProfile)>,
    started: bool,
    /// "ru" or "en", for numbers the window shows.
    language: &'static str,
    equalizer: equalizer::EqualizerState,
    /// The preset whose settings are being chosen.
    editing_preset: Option<String>,
    /// The icon in the notification area, while the settings have it.
    tray: Option<crate::Tray>,
    tray_host: bool,
    tray_registered: bool,
}

thread_local! {
    static APP: RefCell<Option<App>> = const { RefCell::new(None) };
}

/// Runs `f` with the app, unless the app is already busy (a callback while it updates the window) or gone.
fn with_app(f: impl FnOnce(&mut App)) {
    APP.with(|app| match app.try_borrow_mut() {
        Ok(mut app) => {
            if let Some(app) = app.as_mut() {
                f(app);
            }
        }
        Err(_) => tracing::warn!("ignored a nested call into the app"),
    });
}

pub fn start(window: &AppWindow, config_dir: &Path) -> anyhow::Result<()> {
    let config = Config::load(config_dir.join("openscl4-config.toml"));
    let weak = window.as_weak();
    let controller = Controller::start(
        config_dir.join("database.sqlite"),
        config_dir.to_path_buf(),
        move |event| {
            if slint::invoke_from_event_loop(move || with_app(|app| app.on_event(event))).is_err() {
                tracing::debug!("the window is gone");
            }
        },
    )?;
    let app = App {
        window: weak,
        controller,
        config,
        paired: Vec::new(),
        current: None,
        connection: Connection::Off,
        open_on_connect: false,
        failed_command: None,
        pending_write: None,
        add: AddFlow::default(),
        dark: false,
        system_dark: None,
        title_bar: window_chrome::TitleBar::default(),
        legacy: Vec::new(),
        started: false,
        language: "en",
        equalizer: equalizer::EqualizerState::default(),
        editing_preset: None,
        tray: None,
        tray_host: !cfg!(target_os = "linux"),
        tray_registered: false,
    };
    APP.with(|cell| *cell.borrow_mut() = Some(app));
    install_actions(window);
    with_app(|app| {
        app.apply_language();
        app.apply_theme();
        app.sync_tray();
        app.render();
    });
    // closing the window leaves the app in the notification area, unless the settings say to quit
    window.window().on_close_requested(|| {
        with_app(|app| {
            if !app.closes_to_tray() {
                app.quit();
            }
        });
        slint::CloseRequestResponse::HideWindow
    });
    Ok(())
}

/// Keeps the app's theme on the title bar; on Linux `window_chrome` calls it after each round of events. Skipped
/// quietly while the app is busy.
pub fn keep_title_bar() {
    APP.with(|cell| {
        if let Ok(mut slot) = cell.try_borrow_mut()
            && let Some(app) = slot.as_mut()
            && let Some(window) = app.window()
        {
            app.title_bar.keep(window.window());
        }
    });
}

/// Lets go of the device and stops the controller, once the event loop has ended.
pub fn stop() {
    let app = APP.with(|cell| cell.borrow_mut().take());
    if let Some(app) = app {
        app.controller.shutdown();
    }
}

/// Called once the window is shown, when Windows can be told about the title bar.
pub fn shown() {
    // winit makes its window once the event loop runs; only then can it be told the title bar's theme and be
    // listened to
    let _ = slint::invoke_from_event_loop(|| {
        with_app(|app| {
            app.apply_theme();
            // "As in Windows" follows Windows when it changes; winit doesn't report Linux changes, the portal does
            if let Some(window) = app.window() {
                use slint::winit_030::{
                    EventResult, WinitWindowAccessor, winit::event::WindowEvent,
                };
                window.window().on_winit_window_event(|_, event| {
                    if matches!(event, WindowEvent::ThemeChanged(_)) {
                        with_app(|app| {
                            if app.config.settings.theme == AppTheme::System {
                                app.apply_theme();
                                app.render();
                            }
                        });
                    }
                    EventResult::Propagate
                });
            }
        });
    });
}

fn install_actions(window: &AppWindow) {
    equalizer::install_actions(window);
    presets::install_actions(window);
    settings::install_actions(window);
    let actions = window.global::<Actions>();
    actions.on_navigate(|page| with_app(|app| app.navigate(page)));
    actions.on_connect(|mac| {
        with_app(|app| {
            if let Ok(mac) = MacAddr6::from_str(&mac) {
                app.connect(mac, true);
            }
        });
    });
    actions.on_cancel_connect(|| with_app(|app| app.controller.send(Command::CancelConnect)));
    actions.on_disconnect(|| with_app(|app| app.controller.send(Command::Disconnect)));
    actions.on_forget(|mac| {
        with_app(|app| {
            if let Ok(mac) = MacAddr6::from_str(&mac) {
                app.controller.send(Command::Unpair(mac));
            }
        });
    });
    actions.on_open_add_device(|| with_app(App::open_add_device));
    actions.on_filter_models(|query, kind| with_app(|app| app.filter_models(&query, kind)));
    actions.on_choose_model(|code| with_app(|app| app.choose_model(&code)));
    actions.on_set_demo(|demo| with_app(|app| app.set_demo(demo)));
    actions.on_refresh_paired(|| with_app(App::list_devices));
    actions.on_pair(|mac, _name| with_app(|app| app.pair(&mac)));
    actions.on_retry(|| with_app(App::retry));
    actions.on_dismiss_error(|| {
        with_app(|app| {
            app.failed_command = None;
            app.pending_write = None;
            app.render();
        });
    });
    actions.on_set_toggle(|id, on| with_app(|app| app.edit(&id, Edit::Toggle(on))));
    actions.on_set_option(|id, index| with_app(|app| app.edit(&id, Edit::Option(index))));
    actions.on_set_number(|id, value| {
        with_app(|app| {
            let edit = if id == "hue" {
                Edit::Hue(value)
            } else {
                Edit::Number(value)
            };
            app.edit(&id, edit);
        });
    });
    actions.on_press(|id| with_app(|app| app.edit(&id, Edit::Press)));
    actions.on_remove_item(|id, index| with_app(|app| app.edit(&id, Edit::Remove(index))));
    actions.on_set_scale(|position| {
        with_app(|app| app.send(Command::SetSettings(sound_mode_scale_settings(position))));
    });
    actions.on_set_side(|side| {
        with_app(|app| {
            if let Some(window) = app.window() {
                window.global::<AppState>().set_controls_side(side);
            }
            app.render();
        });
    });
    actions.on_copy(|text| with_app(|app| app.copy(&text)));
    actions.on_choose_toggle(|id, on| with_app(|app| app.choose_toggle(&id, on)));
    actions.on_layout_visible(|index, visible| {
        with_app(|app| {
            app.change_layout(|layout, blocks| {
                let block = blocks.get(usize::try_from(index).ok()?)?;
                Some(layout.with_visibility(*block, visible))
            });
        });
    });
    actions.on_layout_move(|index, delta| {
        with_app(|app| {
            app.change_layout(|layout, blocks| {
                // the dialog lists only the blocks the device has; move past the hidden ones too
                let from = blocks.get(usize::try_from(index).ok()?)?;
                let to = blocks.get(usize::try_from(index + delta).ok()?)?;
                let position = |block| layout.blocks.iter().position(|(b, _)| b == block);
                Some(layout.moved(position(from)?, position(to)?))
            });
        });
    });
    actions.on_layout_use(|preset| {
        with_app(|app| {
            app.change_layout(|_, _| {
                Some(OverviewLayout::of(
                    *OverviewPreset::ALL.get(usize::try_from(preset).ok()?)?,
                ))
            });
        });
    });
    actions.on_apply_preset(|name| {
        with_app(|app| app.send(Command::ActivatePreset(name.to_string())));
    });
    actions.on_open_uri(|uri| {
        if let Err(err) = open::that_detached(uri.as_str()) {
            tracing::warn!("could not open {uri}: {err}");
        }
    });
    actions.on_open_system_settings(|page| {
        if let Err(err) = system_settings::open(page) {
            tracing::warn!("could not open the system's {page:?} settings: {err}");
        }
    });
}

impl App {
    fn window(&self) -> Option<AppWindow> {
        self.window.upgrade()
    }

    fn art_color(&self) -> EarbudsColor {
        art_color(self.dark, self.config.settings.earbuds_color)
    }

    fn on_event(&mut self, event: Event) {
        match event {
            #[cfg(target_os = "linux")]
            Event::TrayStatus(status) => {
                let registered = status.registered && self.tray.is_some();
                if self.tray_host == status.host && self.tray_registered == registered {
                    return;
                }
                if !registered && self.tray_registered {
                    self.open_window();
                }
                self.tray_host = status.host;
                self.tray_registered = registered;
                self.sync_tray();
            }
            #[cfg(target_os = "linux")]
            Event::SystemDark(dark) => {
                self.system_dark = Some(dark);
                // Slint gives the title bar the desktop's theme on every report, the same one too, sooner or later than
                // this. Wayland gets the app's back from keep_title_bar; X11 can't say the window's theme, so it is set
                // now and again once Slint is done.
                self.apply_theme();
                slint::Timer::single_shot(Duration::from_millis(500), || {
                    with_app(Self::apply_theme);
                });
            }
            Event::PairedDevices(paired) => {
                self.paired = paired;
                if !self
                    .current
                    .is_some_and(|mac| self.paired.iter().any(|p| p.mac_address == mac))
                {
                    let last = self
                        .config
                        .settings
                        .last_device
                        .as_deref()
                        .and_then(|mac| MacAddr6::from_str(mac).ok());
                    self.current = featured_device(&self.paired, None, last).map(|p| p.mac_address);
                    self.connection = Connection::Off;
                    self.pending_write = None;
                }
                // open the device used last when the app starts
                if !self.started {
                    self.started = true;
                    if let Some(model) = dev::demo_model() {
                        match self.paired.iter().find(|p| p.model == model && p.is_demo) {
                            Some(paired) => self.connect(paired.mac_address, true),
                            None => {
                                self.add = AddFlow {
                                    model: Some(model),
                                    demo: true,
                                    auto_pair: true,
                                    ..AddFlow::default()
                                };
                                self.controller
                                    .send(Command::ListDevices { model, demo: true });
                            }
                        }
                    } else if let Some(mac) = self.current {
                        self.connect(mac, true);
                    }
                }
            }
            Event::Devices {
                model,
                demo,
                result,
            } => {
                if self.add.model == Some(model) && self.add.demo == demo {
                    self.add.loading = false;
                    self.add.bluetooth_off = false;
                    match result {
                        Ok(found) if self.add.auto_pair && !found.is_empty() => {
                            self.add.auto_pair = false;
                            let mac = found[0].mac_address.to_string();
                            self.add.found = found;
                            self.pair(&mac);
                        }
                        Ok(found) => self.add.found = found,
                        Err(Failure::BluetoothOff) => {
                            self.add.found.clear();
                            self.add.bluetooth_off = true;
                        }
                        Err(failure) => {
                            self.add.found.clear();
                            self.add.error = Some(failure);
                        }
                    }
                }
            }
            Event::Connecting(mac) => {
                self.current = Some(mac);
                self.connection = Connection::Connecting;
                self.pending_write = None;
            }
            Event::Connected(snapshot) | Event::Changed(snapshot) => {
                if self.current == Some(snapshot.mac_address) {
                    self.equalizer_changed(&snapshot);
                    self.connection = Connection::Connected(snapshot);
                    // the snapshot is newer than any write cut off by the reconnect it followed
                    self.pending_write = None;
                    if std::mem::take(&mut self.open_on_connect) {
                        self.set_page(dev::start_page().unwrap_or(Page::Overview));
                    }
                }
            }
            Event::Disconnected(mac) => {
                if self.current == Some(mac) {
                    self.equalizer_disconnected();
                    self.editing_preset = None;
                    self.connection = Connection::Off;
                    self.pending_write = None;
                }
            }
            Event::Recovering {
                mac_address,
                reason,
            } => {
                if self.current == Some(mac_address) {
                    let snapshot = match std::mem::replace(&mut self.connection, Connection::Off) {
                        Connection::Connected(snapshot) | Connection::Recovering(snapshot, _) => {
                            snapshot
                        }
                        other => {
                            self.connection = other;
                            return;
                        }
                    };
                    self.connection = Connection::Recovering(snapshot, reason);
                }
            }
            Event::ConnectionLost {
                mac_address,
                reason,
            } => {
                if self.current == Some(mac_address) {
                    self.equalizer_disconnected();
                    self.editing_preset = None;
                    self.connection = Connection::Lost(reason);
                    // the session has used its whole recovery budget, so the cut-off write is final
                    if let Some(pending) = self.pending_write.take() {
                        self.failed_command = Some(pending.command);
                    }
                }
            }
            Event::ConnectFailed {
                mac_address,
                failure,
            } => {
                if self.current == Some(mac_address) {
                    self.open_on_connect = false;
                    self.connection = Connection::Failed(failure);
                    self.pending_write = None;
                }
            }
            Event::CommandFailed { command, failure } => match command {
                Command::Pair(_) => self.add.error = Some(failure),
                command => {
                    tracing::warn!("{command:?} failed: {failure:?}");
                    self.equalizer_failed();
                    self.failed_command = Some(command);
                }
            },
            Event::SettingsNotApplied { command, outcomes } => {
                tracing::warn!("{command:?} was not confirmed by the device: {outcomes:?}");
                match write_failure(command, &outcomes, Instant::now()) {
                    WriteFailure::Now(command) => {
                        self.equalizer_failed();
                        self.failed_command = Some(command);
                    }
                    WriteFailure::AfterReconnect(pending) => {
                        // The device dropped the link while applying the change (dual connections and
                        // LDAC both reset it). The session reconnects on its own, so the error waits
                        // instead of appearing while the user still sees a working connection.
                        self.equalizer_failed();
                        if matches!(self.connection, Connection::Lost(_)) {
                            // the session used its budget before the write answered
                            self.failed_command = Some(pending.command);
                        } else {
                            self.pending_write = Some(pending);
                            slint::Timer::single_shot(RECONNECT_WAIT, || {
                                with_app(Self::show_pending_write_error);
                            });
                        }
                    }
                    WriteFailure::Nothing => {}
                }
            }
            Event::PresetApplied { mac_address, name } => {
                if self.current == Some(mac_address) {
                    self.preset_applied(&name);
                }
            }
            Event::LegacyProfiles(profiles) => self.legacy = profiles,
        }
        self.render();
    }

    /// A change to a row of a section page.
    fn edit(&mut self, id: &str, edit: Edit) {
        if !matches!(self.connection, Connection::Connected(_)) {
            tracing::debug!("ignored a change to {id} while the device is not connected");
            return;
        }
        let Some(snapshot) = self.snapshot() else {
            return;
        };
        if !matches!(
            setting_availability(&snapshot.settings, id),
            SettingAvailability::Available
        ) {
            tracing::debug!("ignored a change to {id}, which cannot be changed in this state");
            return;
        }
        // hue and time rows take numbers too; tell them apart by the setting
        let edit = match (
            edit,
            snapshot
                .settings
                .get(&SettingId::from_str(id).unwrap_or(SettingId::Volume)),
        ) {
            (
                Edit::Number(value),
                Some(openscq30_lib::settings::Setting::HueColorPicker { .. }),
            ) => Edit::Hue(value),
            (Edit::Number(value), Some(openscq30_lib::settings::Setting::TimeOfDay { .. })) => {
                Edit::Time(value)
            }
            (edit, _) => edit,
        };
        match rows::values(snapshot, id, edit) {
            Some(values) => self.send(Command::SetSettings(values)),
            None => tracing::warn!("no change for {id}"),
        }
    }

    fn send(&mut self, command: Command) {
        if !matches!(self.connection, Connection::Connected(_))
            && matches!(
                command,
                Command::SetSettings(_)
                    | Command::ActivatePreset(_)
                    | Command::SavePreset(_)
                    | Command::TogglePresetField { .. }
                    | Command::MigrateLegacyProfiles(_)
            )
        {
            tracing::debug!("ignored {command:?} while the device is not connected");
            return;
        }
        self.failed_command = None;
        self.pending_write = None;
        self.controller.send(command);
        self.render();
    }

    fn layout(&self) -> OverviewLayout {
        OverviewLayout::parse(self.config.settings.overview_layout.as_deref())
    }

    /// Changes the overview's blocks; `change` gets the blocks in the order the dialog lists them.
    fn change_layout(
        &mut self,
        change: impl FnOnce(
            &OverviewLayout,
            &[crate::model::overview::OverviewBlock],
        ) -> Option<OverviewLayout>,
    ) {
        let Some(snapshot) = self.snapshot() else {
            return;
        };
        let one_earbud = is_only_one_earbud_connected(&snapshot.settings);
        let (sound, device) = view::pages(snapshot, one_earbud);
        let layout = self.layout();
        let listed: Vec<_> = layout
            .blocks
            .iter()
            .map(|(block, _)| *block)
            .filter(|block| overview::supported(*block, snapshot, &sound, &device))
            .collect();
        if let Some(changed) = change(&layout, &listed) {
            self.config.settings.overview_layout = Some(changed.serialize());
            self.config.save();
        }
        self.render();
    }

    fn choose_toggle(&mut self, id: &str, on: bool) {
        let Some(snapshot) = self.snapshot() else {
            return;
        };
        let model: &'static str = snapshot.model.into();
        let chosen = self.config.settings.quick_toggles.get(model).cloned();
        let available = overview::toggle_ids(snapshot);
        let mut ids = crate::model::quick_toggles::quick_toggle_ids(chosen.as_deref(), &available);
        ids.retain(|other| other != id);
        if on {
            ids.push(id.to_string());
        }
        self.config
            .settings
            .quick_toggles
            .insert(model.to_string(), ids);
        self.config.save();
        self.render();
    }

    fn copy(&self, text: &str) {
        match arboard::Clipboard::new()
            .and_then(|mut clipboard| clipboard.set_text(text.to_string()))
        {
            Ok(()) => self.toast(ToastKind::Copied),
            Err(err) => tracing::warn!("could not copy: {err}"),
        }
    }

    fn connect(&mut self, mac: MacAddr6, open: bool) {
        self.current = Some(mac);
        self.open_on_connect = open;
        self.failed_command = None;
        self.pending_write = None;
        self.config.settings.last_device = Some(mac.to_string());
        self.config.save();
        self.controller.send(Command::Connect(mac));
        self.render();
    }

    fn navigate(&mut self, page: Page) {
        self.set_page(page);
        self.render();
    }

    fn set_page(&self, page: Page) {
        if let Some(window) = self.window() {
            window.global::<AppState>().set_page(page);
        }
    }

    fn retry(&mut self) {
        if let Some(command) = self.failed_command.take() {
            self.controller.send(command);
        } else if let Some(mac) = self.current {
            self.connect(mac, false);
        }
        self.render();
    }

    /// Shows the error of a write that a reconnect cut off, once its window has passed. A snapshot
    /// that arrived first means the device is back and the write is no longer in doubt.
    fn show_pending_write_error(&mut self) {
        let Some(pending) = self.pending_write.take() else {
            return;
        };
        if !pending.is_due(Instant::now()) {
            // a newer write replaced this one and its own window is still running
            self.pending_write = Some(pending);
            return;
        }
        if !matches!(self.connection, Connection::Recovering(_, _)) {
            // the device is back, or the session has already said it was lost
            return;
        }
        self.equalizer_failed();
        self.failed_command = Some(pending.command);
        self.render();
    }

    fn open_add_device(&mut self) {
        self.add = AddFlow::default();
        if let Some(window) = self.window() {
            let state = window.global::<AppState>();
            state.set_add_step(0);
            state.set_model_query("".into());
            state.set_model_kind(0);
            state.set_add_demo(false);
            state.set_adding(true);
        }
        self.filter_models("", 0);
        self.render();
    }

    fn filter_models(&self, query: &str, kind: i32) {
        if let Some(window) = self.window() {
            let models = view::model_options(query, kind, self.art_color());
            window
                .global::<AppState>()
                .set_models(ModelRc::new(VecModel::from(models)));
        }
    }

    fn choose_model(&mut self, code: &str) {
        let Some(model) = view::find_model(code) else {
            return;
        };
        self.add.model = Some(model);
        self.add.error = None;
        if let Some(window) = self.window() {
            window.global::<AppState>().set_add_step(1);
        }
        self.list_devices();
    }

    fn set_demo(&mut self, demo: bool) {
        self.add.demo = demo;
        self.list_devices();
    }

    fn list_devices(&mut self) {
        let Some(model) = self.add.model else { return };
        self.add.loading = true;
        self.add.found.clear();
        self.add.error = None;
        self.controller.send(Command::ListDevices {
            model,
            demo: self.add.demo,
        });
        self.render();
    }

    fn pair(&mut self, mac: &str) {
        let (Some(model), Ok(mac)) = (self.add.model, MacAddr6::from_str(mac)) else {
            return;
        };
        self.controller.send(Command::Pair(PairedDevice {
            mac_address: mac,
            model,
            is_demo: self.add.demo,
        }));
        if let Some(window) = self.window() {
            window.global::<AppState>().set_adding(false);
        }
        self.connect(mac, true);
    }

    fn apply_language(&mut self) {
        let language = self
            .config
            .settings
            .language
            .as_deref()
            .and_then(languages::find)
            .unwrap_or_else(|| {
                let requested: Vec<String> = DesktopLanguageRequester::requested_languages()
                    .iter()
                    .map(ToString::to_string)
                    .collect();
                languages::choose(&requested)
            })
            .code;
        self.language = language;
        if let Err(err) = slint::select_bundled_translation(language) {
            tracing::warn!("could not select the {language} translation: {err:?}");
        }
        if let Ok(identifier) = LanguageIdentifier::from_str(language) {
            openscq30_lib::i18n::init(&[identifier]);
        }
    }

    fn apply_theme(&mut self) {
        self.dark = match self.config.settings.theme {
            AppTheme::System => self
                .system_dark
                .unwrap_or_else(window_chrome::system_prefers_dark),
            AppTheme::Light => false,
            AppTheme::Dark => true,
        };
        if let Some(window) = self.window() {
            let theme = window.global::<Theme>();
            theme.set_dark(self.dark);
            theme.set_content_scale(self.config.settings.content_scale.factor());
            theme.set_palette(match self.config.settings.accent {
                Accent::Purple => PaletteChoice::Purple,
                Accent::Sage => PaletteChoice::Sage,
                Accent::Rose => PaletteChoice::Rose,
            });
            self.title_bar
                .set(window.window(), self.config.settings.theme, self.dark);
        }
    }

    fn snapshot(&self) -> Option<&DeviceSnapshot> {
        snapshot_of(&self.connection)
    }

    fn render(&self) {
        let Some(window) = self.window() else { return };
        let state = window.global::<AppState>();
        self.render_settings(&state);
        let color = self.art_color();
        let current = self
            .current
            .and_then(|mac| self.paired.iter().find(|p| p.mac_address == mac));
        let model = self
            .snapshot()
            .map(|s| s.model)
            .or(current.map(|p| p.model));

        state.set_device_name(model.map(view::model_name).unwrap_or_default().into());
        state.set_device_code(model.map(view::model_code).unwrap_or_default().into());
        state.set_device_art(model.map_or_else(
            || crate::art::image(crate::model::device_art::DeviceArt::LibertyStem, color),
            |m| view::art_image(m, color),
        ));
        state.set_device_is_speaker(model.is_some_and(view::is_speaker));

        let one_earbud = self
            .snapshot()
            .is_some_and(|s| is_only_one_earbud_connected(&s.settings));
        state.set_one_earbud(one_earbud);
        state.set_connection(connection_view(&self.connection, self.paired.is_empty()));
        state.set_connection_reason(
            match &self.connection {
                Connection::Recovering(_, reason) | Connection::Lost(reason) => reason.translate(),
                _ => String::new(),
            }
            .into(),
        );
        if let Connection::Failed(failure) = &self.connection {
            state.set_failure_kind(failure_kind(failure));
            state.set_failure_text(failure_details(failure).into());
        }

        let (sound, device) = self
            .snapshot()
            .map(|s| view::pages(s, one_earbud))
            .unwrap_or_default();
        // a page the device does not have opens its overview instead
        let page = state.get_page();
        let device_page = !matches!(page, Page::Devices | Page::Settings);
        if self.snapshot().is_some()
            && device_page
            && page != Page::Overview
            && !sound.contains(&page)
            && !device.contains(&page)
        {
            state.set_page(Page::Overview);
        }
        if self.paired.is_empty() && device_page {
            state.set_page(Page::Devices);
        }
        let page = state.get_page();
        if page == Page::Devices {
            state.set_devices(ModelRc::new(VecModel::from(view::device_cards(
                &self.paired,
                self.current,
                color,
            ))));
        }
        if let Some(snapshot) = self.snapshot() {
            state.set_has_spatial(snapshot.settings.contains_key(&SettingId::SpatialAudio));
            if !matches!(
                page,
                Page::Devices | Page::Settings | Page::Overview | Page::Equalizer | Page::Presets
            ) {
                self.render_section(&state, snapshot);
            }
            if page == Page::Overview || state.get_editing_layout() || state.get_choosing_toggles()
            {
                self.render_overview(&state, snapshot, one_earbud, &sound, &device);
            }
            if matches!(page, Page::Overview | Page::SoundModes) {
                self.render_sound_modes(&state, snapshot);
            }
            if matches!(page, Page::Overview | Page::Equalizer) {
                self.render_equalizer_summary(&state, snapshot);
            }
            // The import dialog also reads the band's count from the detailed equalizer model.
            if page == Page::Equalizer || state.get_importing() {
                self.render_equalizer(&state, snapshot);
            }
            if page == Page::Presets {
                self.render_presets(&state, snapshot);
            }
            if self.editing_preset.is_some() || !state.get_editing_preset().is_empty() {
                self.render_preset_editor(&state, snapshot);
            }
        } else {
            state.set_editing_preset("".into());
        }
        if let Some(pages) = crate::models::updated(&state.get_sound_pages(), sound) {
            state.set_sound_pages(pages);
        }
        if let Some(pages) = crate::models::updated(&state.get_device_pages(), device) {
            state.set_device_pages(pages);
        }
        state.set_show_one_earbud_badge(self.config.settings.one_earbud_badge);

        state.set_error_label(
            self.failed_command
                .as_ref()
                .map(command_label)
                .unwrap_or_default()
                .into(),
        );

        // adding a device
        if let Some(model) = self.add.model {
            state.set_add_model_name(view::model_name(model).into());
            state.set_add_model_code(view::model_code(model).into());
            state.set_add_model_art(view::art_image(model, color));
            state.set_add_model_is_speaker(view::is_speaker(model));
        }
        state.set_add_demo(self.add.demo);
        state.set_add_loading(self.add.loading);
        state.set_add_bluetooth_off(self.add.bluetooth_off);
        state.set_add_failure_kind(
            self.add
                .error
                .as_ref()
                .map_or(FailureKind::None, failure_kind),
        );
        state.set_add_error(
            self.add
                .error
                .as_ref()
                .map(failure_details)
                .unwrap_or_default()
                .into(),
        );
        let model_name = self
            .add
            .model
            .map(view::model_name)
            .unwrap_or_default()
            .to_lowercase();
        let mut options: Vec<PairedOption> = self
            .add
            .found
            .iter()
            .map(|d| PairedOption {
                mac: d.mac_address.to_string().into(),
                name: d.name.clone().into(),
                suggested: !model_name.is_empty() && d.name.to_lowercase().contains(&model_name),
            })
            .collect();
        options.sort_by_key(|o| !o.suggested);
        state.set_paired_options(ModelRc::new(VecModel::from(options)));
        self.render_tray();
    }

    /// The settings of the page that is open, and what its side column needs.
    fn render_section(&self, state: &AppState<'_>, snapshot: &DeviceSnapshot) {
        let page = state.get_page();
        let mut side = ControlSide::Left;
        if page == Page::Controls {
            let has_controls = |side: ControlSide| {
                side.settings()
                    .any(|id| snapshot.settings.contains_key(&id))
            };
            let left = has_controls(ControlSide::Left);
            let right = has_controls(ControlSide::Right);
            state.set_has_sides(left && right);
            let show_right = right && (!left || state.get_controls_side() == "right");
            if show_right {
                side = ControlSide::Right;
            }
            state.set_controls_side(if show_right { "right" } else { "left" }.into());
        }
        let panels = rows::panels(page, snapshot, side);
        let panels = if matches!(self.connection, Connection::Recovering(_, _)) {
            rows::read_only(panels)
        } else {
            panels
        };
        if let Some(panels) = crate::models::updated_panels(&state.get_panels(), panels) {
            state.set_panels(panels);
        }
        state.set_has_ldac(snapshot.settings.contains_key(&SettingId::Ldac));
        let information = |id: SettingId| match snapshot.settings.get(&id) {
            Some(openscq30_lib::settings::Setting::Information {
                translated_value, ..
            }) => translated_value.clone(),
            _ => String::new(),
        };
        state.set_case_battery(information(SettingId::CaseBatteryLevel).into());
    }

    fn render_overview(
        &self,
        state: &AppState<'_>,
        snapshot: &DeviceSnapshot,
        one_earbud: bool,
        sound: &[Page],
        device: &[Page],
    ) {
        let layout = self.layout();
        state.set_batteries(ModelRc::new(VecModel::from(overview::batteries(
            snapshot, one_earbud,
        ))));
        let model: &'static str = snapshot.model.into();
        let (mut shown, mut all) = overview::quick_toggles(
            snapshot,
            self.config
                .settings
                .quick_toggles
                .get(model)
                .map(Vec::as_slice),
        );
        if matches!(self.connection, Connection::Recovering(_, _)) {
            for toggle in shown.iter_mut().chain(all.iter_mut()) {
                toggle.enabled = false;
            }
        }
        state.set_quick_toggles(ModelRc::new(VecModel::from(shown)));
        state.set_all_toggles(ModelRc::new(VecModel::from(all)));
        state.set_overview_rows(ModelRc::new(VecModel::from(overview::rows(
            &layout, snapshot, sound, device,
        ))));
        state.set_layout_blocks(ModelRc::new(VecModel::from(overview::layout_blocks(
            &layout, snapshot, sound, device,
        ))));
        state.set_layout_preset(
            layout
                .preset()
                .and_then(|p| OverviewPreset::ALL.iter().position(|q| *q == p))
                .map_or(-1, |i| i as i32),
        );
        let last_preset = snapshot.last_applied_preset.as_deref();
        let (on, off) = self.on_off();
        state.set_presets(ModelRc::new(VecModel::from(overview::presets(
            snapshot,
            last_preset,
            crate::presets::OnOff { on: &on, off: &off },
        ))));
    }

    fn render_sound_modes(&self, state: &AppState<'_>, snapshot: &DeviceSnapshot) {
        let scale_preferred = self.config.settings.sound_mode_view == SoundModeView::Scale;
        if let Some(mode) = rows::mode_view(snapshot, scale_preferred) {
            let label = |value: &str| {
                mode.options
                    .iter()
                    .find(|o| o.value == value)
                    .map(|o| o.label.clone())
                    .unwrap_or_default()
            };
            state.set_normal_label(label("Normal"));
            state.set_transparency_label(label("Transparency"));
            state.set_noise_canceling_label(label("NoiseCanceling"));
            state.set_airplane_label(label("AirplaneMode"));
            state.set_airplane_index(
                mode.options
                    .iter()
                    .position(|o| o.value == "AirplaneMode")
                    .map_or(-1, |i| i as i32),
            );
            state.set_mode_index(mode.index);
            state.set_mode_scale(mode.scale);
            state.set_scale_position(mode.position);
            if let Some(rows) = crate::models::updated(&state.get_mode_rows(), mode.rows) {
                state.set_mode_rows(rows);
            }
            if let Some(modes) = crate::models::updated(&state.get_modes(), mode.options) {
                state.set_modes(modes);
            }
        } else {
            state.set_modes(ModelRc::default());
        }
    }

    /// The profile and limits used by both the overview and the equalizer page.
    fn render_equalizer_summary(&self, state: &AppState<'_>, snapshot: &DeviceSnapshot) {
        use openscq30_lib::settings::Setting;
        let name = match (
            snapshot.settings.get(&SettingId::PresetEqualizerProfile),
            snapshot.settings.get(&SettingId::CustomEqualizerProfile),
        ) {
            (
                Some(Setting::PresetEqualizerProfileSelect {
                    select,
                    value: Some(value),
                    ..
                }),
                _,
            ) => select
                .options
                .iter()
                .position(|o| o == value)
                .and_then(|i| select.localized_options.get(i).cloned())
                .unwrap_or_default(),
            (
                _,
                Some(Setting::ModifiableSelect {
                    value: Some(value), ..
                }),
            ) => value.to_string(),
            _ => String::new(),
        };
        state.set_eq_name(name.into());
        if let Some(Setting::Equalizer { setting, value, .. }) =
            snapshot.settings.get(&SettingId::VolumeAdjustments)
        {
            if state.get_page() == Page::Overview {
                state.set_eq_curve(
                    overview::curve(value, setting.min, setting.max, 600.0, 100.0).into(),
                );
                state.set_eq_band_count(value.len() as i32);
            }
            let numbers = self.numbers();
            state.set_eq_min_text(numbers.signed(setting.min, setting.fraction_digits).into());
            state.set_eq_max_text(numbers.signed(setting.max, setting.fraction_digits).into());
            state.set_eq_min(i32::from(setting.min));
            state.set_eq_max(i32::from(setting.max));
        }
    }

    fn toast_count(&self, kind: ToastKind, count: i32) {
        if let Some(window) = self.window() {
            window.global::<AppState>().set_toast_count(count);
        }
        self.toast(kind);
    }

    /// Shows a short message at the bottom of the window for a few seconds.
    fn toast(&self, kind: ToastKind) {
        let Some(window) = self.window() else { return };
        window.global::<AppState>().set_toast(kind);
        let weak = self.window.clone();
        slint::Timer::single_shot(Duration::from_secs(3), move || {
            if let Some(window) = weak.upgrade() {
                let state = window.global::<AppState>();
                if state.get_toast() == kind {
                    state.set_toast(ToastKind::None);
                }
            }
        });
    }
}

/// What a failed command was about, for the error banner.
fn command_label(command: &Command) -> String {
    match command {
        Command::SetSettings(values) => values
            .first()
            .map(|(id, _)| id.translate())
            .unwrap_or_default(),
        Command::ActivatePreset(name) | Command::SavePreset(name) | Command::DeletePreset(name) => {
            name.clone()
        }
        Command::TogglePresetField { setting_id, .. } => setting_id.translate(),
        Command::MigrateLegacyProfiles(_) => SettingId::CustomEqualizerProfile.translate(),
        _ => String::new(),
    }
}

fn snapshot_of(connection: &Connection) -> Option<&DeviceSnapshot> {
    match connection {
        Connection::Connected(snapshot) | Connection::Recovering(snapshot, _) => Some(snapshot),
        _ => None,
    }
}

fn connection_view(connection: &Connection, no_paired_devices: bool) -> ConnectionView {
    match connection {
        _ if no_paired_devices => ConnectionView::NoDevice,
        Connection::Connected(_) => ConnectionView::Connected,
        Connection::Recovering(_, _) => ConnectionView::Recovering,
        Connection::Connecting => ConnectionView::Connecting,
        Connection::Lost(_) => ConnectionView::Lost,
        Connection::Failed(Failure::BluetoothOff) => ConnectionView::BluetoothOff,
        Connection::Failed(_) => ConnectionView::Failed,
        Connection::Off => ConnectionView::Off,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::SettingMap;
    use openscq30_lib::{
        settings::{CategoryId, Equalizer, Setting, Value},
        storage::{QuickPreset, QuickPresetField},
    };
    use slint::{
        Model,
        platform::{
            Platform, PlatformError, WindowAdapter,
            software_renderer::{MinimalSoftwareWindow, RepaintBufferType},
        },
    };

    fn snapshot() -> Box<DeviceSnapshot> {
        Box::new(DeviceSnapshot {
            mac_address: MacAddr6::nil(),
            model: DeviceModel::SoundcoreA3954,
            is_demo: true,
            categories: Vec::new(),
            settings: SettingMap::new(),
            presets: Vec::new(),
            last_applied_preset: None,
        })
    }

    fn with_headless_window(test: impl FnOnce(&AppWindow)) {
        struct Headless;
        impl Platform for Headless {
            fn create_window_adapter(
                &self,
            ) -> Result<std::rc::Rc<dyn WindowAdapter>, PlatformError> {
                Ok(MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer))
            }
        }
        slint::platform::set_platform(Box::new(Headless)).unwrap();
        let window = AppWindow::new().unwrap();
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("openscl4-config.toml"),
            "tray = false\nlanguage = \"en\"",
        )
        .unwrap();
        start(&window, dir.path()).unwrap();
        test(&window);
        stop();
    }

    #[test]
    fn c30i_exposes_controls_for_both_earbuds() {
        use openscq30_lib::OpenSCQ30Session;

        let model = DeviceModel::SoundcoreA3330;
        let mac_address = model.demo_mac_address();
        let paired = PairedDevice {
            mac_address,
            model,
            is_demo: true,
        };
        let dir = tempfile::tempdir().unwrap();
        let by_category = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let session = OpenSCQ30Session::new(dir.path().join("demo.sqlite"))
                    .await
                    .unwrap();
                session.pair(paired).await.unwrap();
                session
                    .connect(mac_address)
                    .await
                    .unwrap()
                    .settings_by_category()
            });
        let mut snapshot = snapshot();
        snapshot.mac_address = mac_address;
        snapshot.model = model;
        snapshot.categories = by_category
            .iter()
            .map(|(category, settings)| (*category, settings.keys().copied().collect()))
            .collect();
        snapshot.settings = by_category.into_values().flatten().collect();
        assert!(!snapshot.settings.contains_key(&SettingId::LeftSinglePress));
        assert!(!snapshot.settings.contains_key(&SettingId::RightSinglePress));

        with_headless_window(|window| {
            with_app(|app| {
                app.paired.push(paired);
                app.current = Some(mac_address);
                app.on_event(Event::Connected(snapshot));
                app.navigate(Page::Controls);
            });
            let state = window.global::<AppState>();
            assert!(state.get_has_sides(), "the earbud selector must be visible");
            let shows = |id: &str| {
                state
                    .get_panels()
                    .iter()
                    .any(|panel| panel.rows.iter().any(|row| row.id.as_str() == id))
            };
            assert!(shows("leftDoublePress"));
            assert!(shows("leftDoublePressDuringCall"));
            assert!(!shows("rightDoublePress"));
            window.global::<Actions>().invoke_set_side("right".into());
            for id in [
                "rightDoublePress",
                "rightTriplePress",
                "rightLongPress",
                "rightDoublePressDuringCall",
                "rightLongPressDuringCall",
            ] {
                assert!(shows(id), "{id} is missing");
            }
            assert!(!shows("leftDoublePress"));
            assert!(!shows("leftDoublePressDuringCall"));
        });
    }

    #[test]
    fn renders_the_current_page_and_open_dialogs_from_the_latest_snapshot() {
        with_headless_window(|window| {
            let state = window.global::<AppState>();

            with_app(|app| {
                let mut snapshot = snapshot();
                let eq_id = SettingId::VolumeAdjustments;
                snapshot.categories = vec![(CategoryId::Equalizer, vec![eq_id])];
                let eq = |value| Setting::Equalizer {
                    setting: Equalizer {
                        band_hz: vec![100].into(),
                        fraction_digits: 1,
                        min: -120,
                        max: 134,
                    },
                    value: vec![value],
                    read_only: false,
                };
                snapshot.settings.insert(eq_id, eq(0));
                snapshot.presets.push(QuickPreset {
                    name: "Test".into(),
                    fields: vec![QuickPresetField {
                        setting_id: eq_id,
                        value: vec![0i16].into(),
                        is_enabled: true,
                    }],
                });
                app.current = Some(snapshot.mac_address);
                app.paired.push(PairedDevice {
                    mac_address: snapshot.mac_address,
                    model: snapshot.model,
                    is_demo: true,
                });
                app.on_event(Event::Connected(snapshot));
                app.navigate(Page::Overview);
                assert_eq!(state.get_eq_band_count(), 1);
                assert_eq!(state.get_eq_bands().row_count(), 0);
                assert_eq!(state.get_preset_cards().row_count(), 0);

                app.navigate(Page::Equalizer);
                let bands = state.get_eq_bands();
                assert_eq!(bands.row_data(0).unwrap().value, 0);
                app.navigate(Page::Overview);
                let Connection::Connected(snapshot) = &mut app.connection else {
                    panic!("expected a connected demo device");
                };
                snapshot.settings.insert(eq_id, eq(15));
                app.render();
                assert_eq!(bands.row_data(0).unwrap().value, 0);

                state.set_importing(true);
                app.editing_preset = Some("Test".into());
                app.render();
                assert_eq!(bands.row_data(0).unwrap().value, 15);
                assert_eq!(state.get_preset_groups().row_count(), 1);
                app.navigate(Page::Presets);
                assert_eq!(state.get_preset_cards().row_count(), 1);
            });
        });
    }

    #[test]
    fn a_preset_card_keeps_its_fields_model_while_the_fields_do_not_change() {
        with_headless_window(|window| {
            let state = window.global::<AppState>();
            with_app(|app| {
                let mut snapshot = snapshot();
                let id = SettingId::SpatialAudio;
                snapshot
                    .settings
                    .insert(id, Setting::Toggle { value: true });
                snapshot.presets.push(QuickPreset {
                    name: "Commute".into(),
                    fields: vec![QuickPresetField {
                        setting_id: id,
                        value: true.into(),
                        is_enabled: true,
                    }],
                });
                app.current = Some(snapshot.mac_address);
                app.paired.push(PairedDevice {
                    mac_address: snapshot.mac_address,
                    model: snapshot.model,
                    is_demo: true,
                });
                app.on_event(Event::Connected(snapshot));
                app.navigate(Page::Presets);
            });

            let shown = state.get_preset_cards().row_data(0).unwrap().fields;
            assert_eq!(1, shown.row_count());

            with_app(|app| app.render());
            let kept = state.get_preset_cards().row_data(0).unwrap().fields;
            assert_eq!(
                shown, kept,
                "the card's field list was rebuilt even though it says the same"
            );

            with_app(|app| {
                let Connection::Connected(snapshot) = &mut app.connection else {
                    panic!("expected a connected demo device");
                };
                snapshot.presets[0].fields[0].value = false.into();
                app.render();
            });
            let changed = state.get_preset_cards().row_data(0).unwrap().fields;
            assert_ne!(
                changed, shown,
                "the card still shows the fields it had before"
            );
        });
    }

    #[test]
    fn recovering_keeps_the_snapshot_and_shows_a_banner() {
        let connection = Connection::Recovering(snapshot(), SessionFailureReason::TransportLost);

        assert!(snapshot_of(&connection).is_some());
        assert_eq!(
            ConnectionView::Recovering,
            connection_view(&connection, false)
        );
    }

    #[test]
    fn lost_drops_the_snapshot_and_uses_the_lost_screen() {
        let connection = Connection::Lost(SessionFailureReason::ConnectTimedOut);

        assert!(snapshot_of(&connection).is_none());
        assert_eq!(ConnectionView::Lost, connection_view(&connection, false));
    }

    #[test]
    fn a_write_cut_off_by_the_reconnect_waits_before_it_shows_an_error() {
        let now = Instant::now();
        let command = Command::SetSettings(vec![(SettingId::DualConnections, Value::Bool(true))]);
        let outcomes = [SettingWriteOutcome {
            setting_id: SettingId::DualConnections,
            outcome: WriteOutcome::Disconnected,
        }];

        let WriteFailure::AfterReconnect(pending) = write_failure(command, &outcomes, now) else {
            panic!("a write cut off by the reconnect must wait");
        };
        assert!(
            !pending.is_due(now),
            "the error showed right after the command"
        );
        assert!(
            !pending.is_due(now + RECONNECT_WAIT - Duration::from_millis(1)),
            "the error showed before the reconnect window ended"
        );
        assert!(
            pending.is_due(now + RECONNECT_WAIT),
            "the error never showed after the reconnect window"
        );
    }

    #[test]
    fn ldac_and_dual_connections_wait_the_same_way() {
        for setting_id in [SettingId::DualConnections, SettingId::Ldac] {
            let outcomes = [SettingWriteOutcome {
                setting_id,
                outcome: WriteOutcome::Disconnected,
            }];

            assert!(
                matches!(
                    write_failure(
                        Command::SetSettings(vec![(setting_id, Value::Bool(true))]),
                        &outcomes,
                        Instant::now()
                    ),
                    WriteFailure::AfterReconnect(_)
                ),
                "{setting_id:?} reported its error at once"
            );
        }
    }

    #[test]
    fn a_write_the_device_rejected_is_reported_at_once() {
        let outcomes = [SettingWriteOutcome {
            setting_id: SettingId::Ldac,
            outcome: WriteOutcome::Rejected { observed: None },
        }];

        assert!(matches!(
            write_failure(
                Command::SetSettings(vec![(SettingId::Ldac, Value::Bool(true))]),
                &outcomes,
                Instant::now()
            ),
            WriteFailure::Now(_)
        ));
    }
}

/// How the window tells failures apart; the words for them are in the .slint files.
fn failure_kind(failure: &Failure) -> FailureKind {
    match failure {
        Failure::BluetoothOff | Failure::Other(_) => FailureKind::Other,
        Failure::NotFound => FailureKind::NotFound,
        Failure::TimedOut => FailureKind::TimedOut,
    }
}

/// The library's own words, for failures the window has no name for.
fn failure_details(failure: &Failure) -> String {
    match failure {
        Failure::Other(details) => details.clone(),
        _ => String::new(),
    }
}

/// Ways to start the app in a given state, for development and screenshots; debug builds only.
mod dev {
    use openscq30_lib::DeviceModel;

    use crate::{Page, view};

    /// OPENSCQ30_DEMO=A3954 adds a demo device of the model, unless there is one, and connects to it.
    pub fn demo_model() -> Option<DeviceModel> {
        if !cfg!(debug_assertions) {
            return None;
        }
        view::find_model(&std::env::var("OPENSCQ30_DEMO").ok()?)
    }

    /// OPENSCQ30_PAGE=equalizer opens that page once the device connects.
    pub fn start_page() -> Option<Page> {
        if !cfg!(debug_assertions) {
            return None;
        }
        Some(match std::env::var("OPENSCQ30_PAGE").ok()?.as_str() {
            "modes" => Page::SoundModes,
            "equalizer" => Page::Equalizer,
            "effects" => Page::Effects,
            "presets" => Page::Presets,
            "controls" => Page::Controls,
            "comfort" => Page::Comfort,
            "power" => Page::Power,
            "connections" => Page::Connections,
            "case" => Page::Case,
            "about" => Page::About,
            "devices" => Page::Devices,
            "settings" => Page::Settings,
            _ => Page::Overview,
        })
    }
}
