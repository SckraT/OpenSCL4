#![forbid(unsafe_code)]

#[cfg(test)]
mod tests {
    use std::{fs, io, path::Path, sync::Barrier, thread};

    use openscl4_profiles::{PresetValue, ProfileSnapshot, ProfileStore, ProfileStoreError};
    use openscq30_lib::storage::{
        ProfileMigrationError, ProfileMigrationOutcome, migrate_legacy_profiles,
    };
    use rusqlite::{Connection, OpenFlags};

    const MODEL: &str = "SoundcoreA3954";
    const PROFILE: &str = "В дороге";
    const PRESET_FIELDS: &str = r#"[
    {"settingId":"ambientSoundMode","value":{"type":"string","value":"normal"},"isEnabled":true},
    {"settingId":"ambientSoundMode","value":{"type":"string","value":"transparency"},"isEnabled":false},
    {"settingId":"volumeAdjustments","value":{"type":"i16Vec","value":[-120,0,135]},"isEnabled":true}
]"#;

    #[test]
    fn imports_desktop_and_android_profiles_without_replacing_the_source() {
        for filename in ["database.sqlite", "openscq30_lib.sqlite"] {
            let directory = tempfile::tempdir().unwrap();
            let source = directory.path().join(filename);
            let backup = directory.path().join("legacy-profiles.sqlite");
            let destination = directory.path().join("profiles.json");
            drop(fixture(&source));
            let original = fs::read(&source).unwrap();

            assert_eq!(
                migrate_legacy_profiles(&source, &backup, &destination).unwrap(),
                ProfileMigrationOutcome::Imported
            );

            let snapshot = ProfileStore::new(&destination).load().unwrap().unwrap();
            assert_eq!(
                snapshot
                    .equalizer_profile(MODEL, PROFILE)
                    .unwrap()
                    .volume_adjustments,
                [-120, 0, 135]
            );
            assert_eq!(
                snapshot
                    .equalizer_profile("SoundcoreA3062", PROFILE)
                    .unwrap()
                    .volume_adjustments,
                [0, 25]
            );
            let fields = &snapshot.quick_preset(MODEL, PROFILE).unwrap().fields;
            assert_eq!(fields[0].setting_id, fields[1].setting_id);
            assert_eq!(fields[0].value, PresetValue::String("normal".to_owned()));
            assert_eq!(
                fields[1].value,
                PresetValue::String("transparency".to_owned())
            );
            assert!(fields[0].is_enabled);
            assert!(!fields[1].is_enabled);
            assert_eq!(fields[2].value, PresetValue::I16Vec(vec![-120, 0, 135]));
            assert_eq!(fs::read(&source).unwrap(), original);

            let copy =
                Connection::open_with_flags(&backup, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
            let count: i64 = copy
                .query_row("SELECT count(*) FROM quick_preset", [], |row| row.get(0))
                .unwrap();
            assert_eq!(count, 1);
        }
    }

    #[test]
    fn retries_from_the_retained_backup_and_keeps_later_edits() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("database.sqlite");
        let backup = directory.path().join("legacy-profiles.sqlite");
        let target_directory = directory.path().join("destination");
        let destination = target_directory.join("profiles.json");
        let connection = fixture(&source);

        assert!(matches!(
            migrate_legacy_profiles(&source, &backup, &destination),
            Err(ProfileMigrationError::Store(ProfileStoreError::Io(error)))
                if error.kind() == io::ErrorKind::NotFound
        ));
        assert!(backup.exists());
        assert!(!destination.exists());
        let retained = fs::read(&backup).unwrap();

        connection
            .execute("DELETE FROM equalizer_profile", [])
            .unwrap();
        drop(connection);
        fs::create_dir(target_directory).unwrap();
        assert_eq!(
            migrate_legacy_profiles(&source, &backup, &destination).unwrap(),
            ProfileMigrationOutcome::Imported
        );
        let store = ProfileStore::new(&destination);
        let original = store.load().unwrap().unwrap();
        assert_eq!(original.equalizer_profiles.len(), 2);

        let edited = ProfileSnapshot {
            equalizer_profiles: Vec::new(),
            ..original
        };
        store.replace(&edited).unwrap();
        let unavailable_source = directory.path().join("unavailable.sqlite");
        assert_eq!(
            migrate_legacy_profiles(&unavailable_source, &backup, &destination).unwrap(),
            ProfileMigrationOutcome::AlreadyActive
        );
        assert_eq!(store.load().unwrap(), Some(edited));
        assert_eq!(fs::read(&backup).unwrap(), retained);
    }

    #[test]
    fn invalid_legacy_data_does_not_publish_a_backup_or_partial_store() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("openscq30_lib.sqlite");
        let backup = directory.path().join("legacy-profiles.sqlite");
        let destination = directory.path().join("profiles.json");
        let connection = fixture(&source);
        connection
            .execute("UPDATE quick_preset SET fields = jsonb('{}')", [])
            .unwrap();
        let original = fs::read(&source).unwrap();

        assert!(matches!(
            migrate_legacy_profiles(&source, &backup, &destination),
            Err(ProfileMigrationError::InvalidRecord(_))
        ));
        assert!(!backup.exists());
        assert!(!destination.exists());
        assert_eq!(fs::read(&source).unwrap(), original);

        connection
            .execute(
                "UPDATE quick_preset SET fields = jsonb(?1)",
                [PRESET_FIELDS],
            )
            .unwrap();
        assert_eq!(
            migrate_legacy_profiles(&source, &backup, &destination).unwrap(),
            ProfileMigrationOutcome::Imported
        );
    }

    #[test]
    fn backup_includes_committed_updates_from_the_source_wal() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("database.sqlite");
        let backup = directory.path().join("legacy-profiles.sqlite");
        let destination = directory.path().join("profiles.json");
        let connection = fixture(&source);
        connection
            .execute_batch("PRAGMA journal_mode = WAL; PRAGMA wal_autocheckpoint = 0;")
            .unwrap();
        connection
            .execute(
                "UPDATE equalizer_profile SET volume_adjustments = '[22]' WHERE device_model = ?1",
                [MODEL],
            )
            .unwrap();
        assert!(directory.path().join("database.sqlite-wal").exists());

        assert_eq!(
            migrate_legacy_profiles(&source, &backup, &destination).unwrap(),
            ProfileMigrationOutcome::Imported
        );
        let snapshot = ProfileStore::new(destination).load().unwrap().unwrap();
        assert_eq!(
            snapshot
                .equalizer_profile(MODEL, PROFILE)
                .unwrap()
                .volume_adjustments,
            [22]
        );
        let retained =
            Connection::open_with_flags(backup, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
        let gains: String = retained
            .query_row(
                "SELECT volume_adjustments FROM equalizer_profile WHERE device_model = ?1",
                [MODEL],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(gains, "[22]");
    }

    #[test]
    fn concurrent_imports_publish_one_complete_snapshot() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("database.sqlite");
        let backup = directory.path().join("legacy-profiles.sqlite");
        let destination = directory.path().join("profiles.json");
        drop(fixture(&source));
        let barrier = Barrier::new(2);

        let migrate = || {
            barrier.wait();
            migrate_legacy_profiles(&source, &backup, &destination).unwrap()
        };
        let (first, second) = thread::scope(|scope| {
            let first = scope.spawn(migrate);
            let second = migrate();
            (first.join().unwrap(), second)
        });
        assert!(matches!(
            (first, second),
            (
                ProfileMigrationOutcome::Imported,
                ProfileMigrationOutcome::AlreadyActive
            ) | (
                ProfileMigrationOutcome::AlreadyActive,
                ProfileMigrationOutcome::Imported
            )
        ));
        let snapshot = ProfileStore::new(destination).load().unwrap().unwrap();
        assert_eq!(snapshot.equalizer_profiles.len(), 2);
        assert_eq!(snapshot.quick_presets.len(), 1);
    }

    fn fixture(path: &Path) -> Connection {
        let connection = Connection::open(path).unwrap();
        connection
            .execute_batch(include_str!("../src/storage/migrations/0.sql"))
            .unwrap();
        connection.execute(
        "INSERT INTO equalizer_profile (device_model, name, volume_adjustments) VALUES (?1, ?2, ?3)",
        (MODEL, PROFILE, "[-120,0,135]"),
    ).unwrap();
        connection.execute(
        "INSERT INTO equalizer_profile (device_model, name, volume_adjustments) VALUES (?1, ?2, ?3)",
        ("SoundcoreA3062", PROFILE, "[0,25]"),
    ).unwrap();
        connection
            .execute(
                "INSERT INTO quick_preset (device_model, name, fields) VALUES (?1, ?2, jsonb(?3))",
                (MODEL, PROFILE, PRESET_FIELDS),
            )
            .unwrap();
        connection
    }
}
