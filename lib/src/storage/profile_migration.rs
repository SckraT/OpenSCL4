use std::{fs, io, path::Path};

use openscl4_profiles::{
    EqualizerProfile, ProfileKey, ProfileSnapshot, ProfileStore, ProfileStoreError, QuickPreset,
};
use rusqlite::{Connection, MAIN_DB, OpenFlags};
use tempfile::NamedTempFile;
use thiserror::Error;

/// Which snapshot is active after migration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfileMigrationOutcome {
    Imported,
    AlreadyActive,
}

#[derive(Debug, Error)]
pub enum ProfileMigrationError {
    #[error("legacy database, retained backup, and destination must be distinct files")]
    OverlappingPaths,
    #[error("legacy profile migration I/O failed")]
    Io(#[from] io::Error),
    #[error("legacy profile database could not be read or backed up")]
    Sqlite(#[from] rusqlite::Error),
    #[error("a legacy profile or preset contains invalid data")]
    InvalidRecord(#[from] serde_json::Error),
    #[error(transparent)]
    Store(#[from] ProfileStoreError),
}

/// Imports 1.x profiles from a caller-selected SQLite database into a versioned store.
///
/// The backup path is owned by this migration. A retained backup from an interrupted attempt
/// supplies the exact snapshot on retry. Publishing the complete destination file marks the
/// migration complete. This function performs blocking filesystem and database operations.
pub fn migrate_legacy_profiles(
    source: &Path,
    backup: &Path,
    destination: &Path,
) -> Result<ProfileMigrationOutcome, ProfileMigrationError> {
    if source == backup || source == destination || backup == destination {
        return Err(ProfileMigrationError::OverlappingPaths);
    }

    let store = ProfileStore::new(destination);
    if store.load()?.is_some() {
        return Ok(ProfileMigrationOutcome::AlreadyActive);
    }

    let snapshot = get_or_create_backup(source, backup)?;
    if store.create_if_absent(&snapshot)? {
        Ok(ProfileMigrationOutcome::Imported)
    } else {
        // Another process completed the migration while this one was preparing the snapshot.
        store
            .load()?
            .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))?;
        Ok(ProfileMigrationOutcome::AlreadyActive)
    }
}

fn get_or_create_backup(
    source: &Path,
    backup: &Path,
) -> Result<ProfileSnapshot, ProfileMigrationError> {
    if backup.try_exists()? {
        let retained = fs::canonicalize(backup)?;
        if fs::canonicalize(source).is_ok_and(|source| source == retained) {
            return Err(ProfileMigrationError::OverlappingPaths);
        }
        return read_snapshot(backup);
    }

    let directory = backup
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let staged = NamedTempFile::new_in(directory)?;
    let source = Connection::open_with_flags(source, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    source.backup(MAIN_DB, staged.path(), None)?;

    let snapshot = read_snapshot(staged.path())?;
    snapshot.validate()?;
    staged.as_file().sync_all()?;
    match staged.persist_noclobber(backup) {
        Ok(_) => Ok(snapshot),
        Err(error) if error.error.kind() == io::ErrorKind::AlreadyExists => read_snapshot(backup),
        Err(error) => Err(error.error.into()),
    }
}

fn read_snapshot(path: &Path) -> Result<ProfileSnapshot, ProfileMigrationError> {
    let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let mut snapshot = ProfileSnapshot::default();
    let mut profiles = connection.prepare(
        "SELECT device_model, name, volume_adjustments FROM equalizer_profile ORDER BY device_model, name",
    )?;
    let mut profile_rows = profiles.query([])?;
    while let Some(row) = profile_rows.next()? {
        snapshot.equalizer_profiles.push(EqualizerProfile {
            key: ProfileKey {
                model: row.get(0)?,
                name: row.get(1)?,
            },
            volume_adjustments: serde_json::from_str(&row.get::<_, String>(2)?)?,
        });
    }

    let mut presets = connection.prepare(
        "SELECT device_model, name, json(fields) FROM quick_preset ORDER BY device_model, name",
    )?;
    let mut preset_rows = presets.query([])?;
    while let Some(row) = preset_rows.next()? {
        snapshot.quick_presets.push(QuickPreset {
            key: ProfileKey {
                model: row.get(0)?,
                name: row.get(1)?,
            },
            fields: serde_json::from_str(&row.get::<_, String>(2)?)?,
        });
    }
    Ok(snapshot)
}
