//! Versioned storage for equalizer profiles and quick presets.

#![forbid(unsafe_code)]

use std::{
    collections::HashSet,
    fs::File,
    io::{self, BufReader},
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;
use thiserror::Error;

const SCHEMA_VERSION: u32 = 1;

/// Keys are scoped to a model so devices can use the same profile or preset name.
#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileKey {
    pub model: String,
    pub name: String,
}

/// The named gain values for one model's equalizer.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct EqualizerProfile {
    pub key: ProfileKey,
    pub volume_adjustments: Vec<i16>,
}

/// A saved setting value in the format used by OpenSCL4 1.x presets.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "camelCase")]
pub enum PresetValue {
    Bool(bool),
    U16(u16),
    U16Vec(Vec<u16>),
    OptionalU16(Option<u16>),
    I16Vec(Vec<i16>),
    I32(i32),
    F32(f32),
    String(String),
    StringVec(Vec<String>),
    OptionalString(Option<String>),
    ModifiableSelectCommand(ModifiableSelectCommand),
    MultiSelectWithRemoveCommand(MultiSelectWithRemoveCommand),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "name", rename_all = "camelCase")]
pub enum ModifiableSelectCommand {
    Add(String),
    Remove(String),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "name", rename_all = "camelCase")]
pub enum MultiSelectWithRemoveCommand {
    Remove(String),
}

/// Retains the value and whether it is selected for application.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct QuickPresetField {
    pub setting_id: String,
    pub value: PresetValue,
    pub is_enabled: bool,
}

/// A preset preserves field order, including repeated IDs from older data.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct QuickPreset {
    pub key: ProfileKey,
    pub fields: Vec<QuickPresetField>,
}

/// The profiles and presets that belong to a single committed file.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ProfileSnapshot {
    pub equalizer_profiles: Vec<EqualizerProfile>,
    pub quick_presets: Vec<QuickPreset>,
}

impl ProfileSnapshot {
    /// Looks up an equalizer profile by model and exact name.
    pub fn equalizer_profile(&self, model: &str, name: &str) -> Option<&EqualizerProfile> {
        self.equalizer_profiles
            .iter()
            .find(|profile| profile.key.model == model && profile.key.name == name)
    }

    /// Looks up a quick preset by model and exact name.
    pub fn quick_preset(&self, model: &str, name: &str) -> Option<&QuickPreset> {
        self.quick_presets
            .iter()
            .find(|preset| preset.key.model == model && preset.key.name == name)
    }

    fn validate(&self) -> Result<(), ProfileStoreError> {
        validate_keys(self.equalizer_profiles.iter().map(|profile| &profile.key))?;
        validate_keys(self.quick_presets.iter().map(|preset| &preset.key))?;
        for field in self.quick_presets.iter().flat_map(|preset| &preset.fields) {
            if field.setting_id.is_empty()
                || matches!(field.value, PresetValue::F32(value) if !value.is_finite())
            {
                return Err(ProfileStoreError::InvalidPresetField);
            }
        }
        Ok(())
    }
}

fn validate_keys<'a>(keys: impl Iterator<Item = &'a ProfileKey>) -> Result<(), ProfileStoreError> {
    let mut seen = HashSet::new();
    for key in keys {
        if key.model.is_empty() {
            return Err(ProfileStoreError::MissingModel);
        }
        if !seen.insert(key) {
            return Err(ProfileStoreError::DuplicateKey);
        }
    }
    Ok(())
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredProfiles<T> {
    schema_version: u32,
    snapshot: T,
}

/// Opens and atomically replaces a versioned profile snapshot at a caller-owned path.
pub struct ProfileStore {
    path: PathBuf,
}

impl ProfileStore {
    /// Selects a destination in an existing application configuration directory.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Returns `None` for a missing store and reports unreadable or invalid data as an error.
    pub fn load(&self) -> Result<Option<ProfileSnapshot>, ProfileStoreError> {
        let file = match File::open(&self.path) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        let stored: StoredProfiles<serde_json::Value> =
            serde_json::from_reader(BufReader::new(file))?;
        if stored.schema_version != SCHEMA_VERSION {
            return Err(ProfileStoreError::UnsupportedVersion(stored.schema_version));
        }
        let snapshot: ProfileSnapshot = serde_json::from_value(stored.snapshot)?;
        snapshot.validate()?;
        Ok(Some(snapshot))
    }

    /// Validates and syncs a new snapshot before atomically replacing the previous file.
    pub fn replace(&self, snapshot: &ProfileSnapshot) -> Result<(), ProfileStoreError> {
        snapshot.validate()?;
        let parent = self
            .path
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let mut temporary = NamedTempFile::new_in(parent)?;
        let stored = StoredProfiles {
            schema_version: SCHEMA_VERSION,
            snapshot,
        };
        serde_json::to_writer(temporary.as_file_mut(), &stored)?;
        temporary.as_file().sync_all()?;
        temporary.persist(&self.path).map_err(|error| error.error)?;
        Ok(())
    }
}

/// A profile snapshot could not be read or written.
#[derive(Debug, Error)]
pub enum ProfileStoreError {
    #[error("profile store I/O failed")]
    Io(#[from] io::Error),
    #[error("invalid profile store data")]
    Json(#[from] serde_json::Error),
    #[error("unsupported profile store schema version {0}")]
    UnsupportedVersion(u32),
    #[error("a profile or preset is missing its model")]
    MissingModel,
    #[error("duplicate model and name in profile store")]
    DuplicateKey,
    #[error("invalid quick preset field")]
    InvalidPresetField,
}
