#![forbid(unsafe_code)]

#[cfg(test)]
mod tests {
    use std::fs;

    use openscl4_profiles::{
        EqualizerProfile, PresetValue, ProfileKey, ProfileSnapshot, ProfileStore,
        ProfileStoreError, QuickPreset, QuickPresetField,
    };

    #[test]
    fn reopens_profiles_and_presets_by_model_and_name() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("profiles.json");
        let store = ProfileStore::new(&path);
        assert_eq!(store.load().unwrap(), None);

        let snapshot = example_snapshot();
        store.replace(&snapshot).unwrap();

        let reopened = ProfileStore::new(path).load().unwrap().unwrap();
        assert_eq!(reopened, snapshot);
        assert_eq!(
            reopened
                .equalizer_profile("A3954", "В дороге")
                .unwrap()
                .volume_adjustments,
            [-120, 0, 135]
        );
        assert_eq!(
            reopened
                .equalizer_profile("A3062", "В дороге")
                .unwrap()
                .volume_adjustments,
            [0, 25]
        );
        let preset = reopened.quick_preset("A3954", "В дороге").unwrap();
        assert_eq!(preset.fields[0].value, PresetValue::String("normal".into()));
        assert!(!preset.fields[1].is_enabled);
    }

    #[test]
    fn validates_before_replacing_a_committed_snapshot() {
        let directory = tempfile::tempdir().unwrap();
        let store = ProfileStore::new(directory.path().join("profiles.json"));
        let original = example_snapshot();
        store.replace(&original).unwrap();

        let mut invalid = original.clone();
        invalid
            .quick_presets
            .push(original.quick_presets[0].clone());
        assert!(matches!(
            store.replace(&invalid),
            Err(ProfileStoreError::DuplicateKey)
        ));
        invalid.quick_presets.pop();
        invalid.quick_presets[0].fields[0].value = PresetValue::F32(f32::NAN);
        assert!(matches!(
            store.replace(&invalid),
            Err(ProfileStoreError::InvalidPresetField)
        ));
        assert_eq!(store.load().unwrap(), Some(original));

        store.replace(&ProfileSnapshot::default()).unwrap();
        assert_eq!(store.load().unwrap(), Some(ProfileSnapshot::default()));
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[test]
    fn failed_installation_keeps_existing_data_and_removes_the_temporary_file() {
        let directory = tempfile::tempdir().unwrap();
        let occupied = directory.path().join("profiles.json");
        fs::create_dir(&occupied).unwrap();
        fs::write(occupied.join("original"), b"existing data").unwrap();

        let store = ProfileStore::new(&occupied);
        assert!(matches!(
            store.replace(&example_snapshot()),
            Err(ProfileStoreError::Io(_))
        ));
        assert_eq!(
            fs::read(occupied.join("original")).unwrap(),
            b"existing data"
        );
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[test]
    fn distinguishes_invalid_data_from_an_unsupported_schema_version() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("profiles.json");
        let store = ProfileStore::new(&path);

        fs::write(&path, b"{").unwrap();
        assert!(matches!(store.load(), Err(ProfileStoreError::Json(_))));
        fs::write(
            &path,
            br#"{"schema_version":7,"snapshot":{"nextFormat":true}}"#,
        )
        .unwrap();
        assert!(matches!(
            store.load(),
            Err(ProfileStoreError::UnsupportedVersion(7))
        ));
    }

    fn example_snapshot() -> ProfileSnapshot {
        ProfileSnapshot {
            equalizer_profiles: vec![
                EqualizerProfile {
                    key: ProfileKey {
                        model: "A3954".into(),
                        name: "В дороге".into(),
                    },
                    volume_adjustments: vec![-120, 0, 135],
                },
                EqualizerProfile {
                    key: ProfileKey {
                        model: "A3062".into(),
                        name: "В дороге".into(),
                    },
                    volume_adjustments: vec![0, 25],
                },
            ],
            quick_presets: vec![QuickPreset {
                key: ProfileKey {
                    model: "A3954".into(),
                    name: "В дороге".into(),
                },
                fields: vec![
                    QuickPresetField {
                        setting_id: "ambientSoundMode".into(),
                        value: PresetValue::String("normal".into()),
                        is_enabled: true,
                    },
                    QuickPresetField {
                        setting_id: "ambientSoundMode".into(),
                        value: PresetValue::String("transparency".into()),
                        is_enabled: false,
                    },
                ],
            }],
        }
    }
}
