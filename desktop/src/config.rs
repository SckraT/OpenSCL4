//! The app's own settings, kept in openscl4-config.toml next to its database.

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::model::device_art::EarbudsColor;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AppTheme {
    /// As in the system
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Accent {
    #[default]
    Purple,
    Sage,
    Rose,
}

/// How sound modes are shown on devices that have the 1 to 11 scale.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SoundModeView {
    Tiles,
    #[default]
    Scale,
}

/// What the tray icon's menu shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct TrayItems {
    pub battery: bool,
    pub modes: bool,
    pub presets: bool,
    pub ldac: bool,
}

impl Default for TrayItems {
    fn default() -> Self {
        Self {
            battery: true,
            modes: true,
            presets: true,
            ldac: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub theme: AppTheme,
    pub accent: Accent,
    /// None follows the theme.
    pub earbuds_color: Option<EarbudsColor>,
    pub sound_mode_view: SoundModeView,
    pub one_earbud_badge: bool,
    /// A language such as "ru", or None for the system's.
    pub language: Option<String>,
    /// As OverviewLayout::serialize writes it.
    pub overview_layout: Option<String>,
    /// The quick toggles chosen for each device model.
    pub quick_toggles: HashMap<String, Vec<String>>,
    /// The device opened last, by its address.
    pub last_device: Option<String>,
    /// The icon in the notification area.
    pub tray: bool,
    pub tray_items: TrayItems,
    /// Closing the window leaves the app in the notification area, while the icon is there.
    pub close_to_tray: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: AppTheme::default(),
            accent: Accent::default(),
            earbuds_color: None,
            sound_mode_view: SoundModeView::default(),
            one_earbud_badge: true,
            language: None,
            overview_layout: None,
            quick_toggles: HashMap::new(),
            last_device: None,
            tray: true,
            tray_items: TrayItems::default(),
            close_to_tray: true,
        }
    }
}

pub struct Config {
    path: PathBuf,
    pub settings: Settings,
}

impl Config {
    /// The saved settings, or the defaults when there are none or they can not be read.
    pub fn load(path: PathBuf) -> Self {
        let settings = match fs::read_to_string(&path) {
            Ok(text) => toml::from_str(&text).unwrap_or_else(|err| {
                tracing::warn!("ignoring unreadable settings in {}: {err}", path.display());
                Settings::default()
            }),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Settings::default(),
            Err(err) => {
                tracing::warn!("could not read {}: {err}", path.display());
                Settings::default()
            }
        };
        Self { path, settings }
    }

    pub fn save(&self) {
        let result = toml::to_string_pretty(&self.settings)
            .map_err(anyhow::Error::from)
            .and_then(|text| write(&self.path, &text));
        if let Err(err) = result {
            tracing::error!(
                "could not save settings to {}: {err:?}",
                self.path.display()
            );
        }
    }
}

fn write(path: &Path, text: &str) -> anyhow::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(path, text)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_survive_saving_and_missing_ones_get_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let mut config = Config::load(path.clone());
        assert_eq!(Settings::default(), config.settings);
        config.settings.theme = AppTheme::Dark;
        config.settings.tray = false;
        config.settings.close_to_tray = false;
        config.settings.tray_items.modes = false;
        config
            .settings
            .quick_toggles
            .insert("SoundcoreA3954".to_string(), vec!["ldac".to_string()]);
        config.save();
        assert_eq!(config.settings, Config::load(path.clone()).settings);

        fs::write(&path, "theme = \"light\"\n").unwrap();
        let loaded = Config::load(path);
        assert_eq!(AppTheme::Light, loaded.settings.theme);
        assert!(loaded.settings.one_earbud_badge);
        assert!(loaded.settings.tray && loaded.settings.close_to_tray);
        assert_eq!(TrayItems::default(), loaded.settings.tray_items);
        assert!(loaded.settings.tray_items.ldac);
        let partial: Settings = toml::from_str("[trayItems]\nldac = false\n").unwrap();
        assert!(!partial.tray_items.ldac);
        assert!(
            partial.tray_items.battery && partial.tray_items.modes && partial.tray_items.presets
        );
    }
}
