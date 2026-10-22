//! The window's state and what happens when the user or the device does something. Runs on the UI thread; the
//! controller's events are brought here with `slint::invoke_from_event_loop`.

use std::{cell::RefCell, path::Path, str::FromStr, time::Duration};

use i18n_embed::{DesktopLanguageRequester, unic_langid::LanguageIdentifier};
use macaddr::MacAddr6;
use openscq30_i18n::Translate;
use openscq30_lib::{
    DeviceModel,
    client::availability::{SettingAvailability, setting_availability},
    connection::ConnectionDescriptor,
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
    view, window_chrome,
};

enum Connection {
    Off,
    Connecting,
    Connected(Box<DeviceSnapshot>),
    Lost,
    Failed(Failure),
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
    add: AddFlow,
    dark: bool,
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
        add: AddFlow::default(),
        dark: false,
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
            // "As in Windows" follows Windows when it changes
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
            }
            Event::Connected(snapshot) | Event::Changed(snapshot) => {
                if self.current == Some(snapshot.mac_address) {
                    self.equalizer_changed(&snapshot);
                    self.connection = Connection::Connected(snapshot);
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
                }
            }
            Event::ConnectionLost(mac) => {
                if self.current == Some(mac) {
                    self.equalizer_disconnected();
                    self.editing_preset = None;
                    self.connection = Connection::Lost;
                }
            }
            Event::ConnectFailed {
                mac_address,
                failure,
            } => {
                if self.current == Some(mac_address) {
                    self.open_on_connect = false;
                    self.connection = Connection::Failed(failure);
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
                self.equalizer_failed();
                self.failed_command = Some(command);
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
        self.failed_command = None;
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
                .window()
                .is_some_and(|window| window_chrome::system_prefers_dark(window.window())),
            AppTheme::Light => false,
            AppTheme::Dark => true,
        };
        if let Some(window) = self.window() {
            let theme = window.global::<Theme>();
            theme.set_dark(self.dark);
            theme.set_palette(match self.config.settings.accent {
                Accent::Purple => PaletteChoice::Purple,
                Accent::Sage => PaletteChoice::Sage,
                Accent::Rose => PaletteChoice::Rose,
            });
            window_chrome::set_title_bar(
                window.window(),
                match self.config.settings.theme {
                    AppTheme::System => None,
                    AppTheme::Light => Some(false),
                    AppTheme::Dark => Some(true),
                },
            );
        }
    }

    fn snapshot(&self) -> Option<&DeviceSnapshot> {
        match &self.connection {
            Connection::Connected(snapshot) => Some(snapshot),
            _ => None,
        }
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

        state.set_devices(ModelRc::new(VecModel::from(view::device_cards(
            &self.paired,
            self.current,
            color,
        ))));
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
        state.set_connection(match (&self.connection, current) {
            (_, None) if self.paired.is_empty() => ConnectionView::NoDevice,
            (Connection::Connected(_), _) => ConnectionView::Connected,
            (Connection::Connecting, _) => ConnectionView::Connecting,
            (Connection::Lost, _) => ConnectionView::Lost,
            (Connection::Failed(Failure::BluetoothOff), _) => ConnectionView::BluetoothOff,
            (Connection::Failed(_), _) => ConnectionView::Failed,
            (Connection::Off, _) => ConnectionView::Off,
        });
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
        state.set_sound_pages(ModelRc::new(VecModel::from(sound)));
        state.set_device_pages(ModelRc::new(VecModel::from(device)));
        if let Some(snapshot) = self.snapshot() {
            self.render_section(&state, snapshot);
        }
        if self.snapshot().is_none() {
            state.set_editing_preset("".into());
        }

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
        state.set_has_sides(snapshot.settings.contains_key(&SettingId::LeftSinglePress));
        let side = state.get_controls_side().to_string();
        let panels = rows::panels(page, snapshot, &side);
        if let Some(panels) = crate::models::updated_panels(&state.get_panels(), panels) {
            state.set_panels(panels);
        }
        state.set_has_ldac(snapshot.settings.contains_key(&SettingId::Ldac));
        state.set_has_spatial(snapshot.settings.contains_key(&SettingId::SpatialAudio));
        let information = |id: SettingId| match snapshot.settings.get(&id) {
            Some(openscq30_lib::settings::Setting::Information {
                translated_value, ..
            }) => translated_value.clone(),
            _ => String::new(),
        };
        state.set_case_battery(information(SettingId::CaseBatteryLevel).into());

        // the overview
        let one_earbud = is_only_one_earbud_connected(&snapshot.settings);
        let (sound, device) = view::pages(snapshot, one_earbud);
        let layout = self.layout();
        state.set_batteries(ModelRc::new(VecModel::from(overview::batteries(
            snapshot, one_earbud,
        ))));
        let model: &'static str = snapshot.model.into();
        let (shown, all) = overview::quick_toggles(
            snapshot,
            self.config
                .settings
                .quick_toggles
                .get(model)
                .map(Vec::as_slice),
        );
        state.set_quick_toggles(ModelRc::new(VecModel::from(shown)));
        state.set_all_toggles(ModelRc::new(VecModel::from(all)));
        state.set_overview_rows(ModelRc::new(VecModel::from(overview::rows(
            &layout, snapshot, &sound, &device,
        ))));
        state.set_layout_blocks(ModelRc::new(VecModel::from(overview::layout_blocks(
            &layout, snapshot, &sound, &device,
        ))));
        state.set_layout_preset(
            layout
                .preset()
                .and_then(|p| OverviewPreset::ALL.iter().position(|q| *q == p))
                .map_or(-1, |i| i as i32),
        );
        state.set_show_one_earbud_badge(self.config.settings.one_earbud_badge);
        let last_preset = snapshot.last_applied_preset.as_deref();
        let (on, off) = self.on_off();
        state.set_presets(ModelRc::new(VecModel::from(overview::presets(
            snapshot,
            last_preset,
            crate::presets::OnOff { on: &on, off: &off },
        ))));
        self.render_equalizer_summary(state, snapshot);
        self.render_equalizer(state, snapshot);
        self.render_presets(state, snapshot);

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
            state.set_modes(ModelRc::new(VecModel::from(mode.options)));
        } else {
            state.set_modes(ModelRc::default());
        }
    }

    /// The equalizer's current profile and curve, for the overview.
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
            state.set_eq_curve(
                overview::curve(value, setting.min, setting.max, 600.0, 100.0).into(),
            );
            state.set_eq_band_count(value.len() as i32);
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
