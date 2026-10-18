//! Equalizer profiles saved by OpenSCQ30 v1, which kept them in config.toml. Copied from
//! gui/src/openscq30_v1_migration.rs so that gui/ stays as it is; both read the same file.

use std::{collections::HashMap, path::Path};

use anyhow::{anyhow, bail};
use openscq30_lib::{
    device::OpenSCQ30Device,
    settings::{self, Setting, SettingId, Value},
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
struct LegacyConfig {
    equalizer_custom_profiles: HashMap<String, LegacyEqualizerProfile>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LegacyEqualizerProfile {
    // not renamed to volume_adjustments to keep backwards compatibility with old settings files
    pub volume_offsets: Vec<i16>,
}

/// The v1 profiles by name, sorted, or none when there is no v1 config file.
pub async fn equalizer_profiles(
    config_dir: &Path,
) -> anyhow::Result<Vec<(String, LegacyEqualizerProfile)>> {
    let text = match tokio::fs::read_to_string(config_dir.join("config.toml")).await {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => return Err(err.into()),
    };
    let config: LegacyConfig = toml::from_str(&text)?;
    let mut profiles: Vec<_> = config.equalizer_custom_profiles.into_iter().collect();
    profiles.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(profiles)
}

/// Saves a v1 profile as a custom profile of the device without changing what the device plays.
pub async fn migrate_profile(
    device: &(dyn OpenSCQ30Device + Send + Sync),
    name: String,
    volume_adjustments: Vec<i16>,
) -> anyhow::Result<()> {
    let Setting::Equalizer {
        value: old_volume_adjustments,
        ..
    } = device
        .setting(&SettingId::VolumeAdjustments)
        .ok_or_else(|| anyhow!("device does not have VolumeAdjustments setting"))?
    else {
        bail!("VolumeAdjustments is not an equalizer");
    };
    // By setting the volume adjustments, saving the profile, and reverting all in one go, no packets are sent, since
    // the end result is equivalent to what we started with, and that is checked before performing any actions.
    device
        .set_setting_values(vec![
            (SettingId::VolumeAdjustments, volume_adjustments.into()),
            (
                SettingId::CustomEqualizerProfile,
                Value::ModifiableSelectCommand(settings::ModifiableSelectCommand::Add(name.into())),
            ),
            (SettingId::VolumeAdjustments, old_volume_adjustments.into()),
        ])
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use macaddr::MacAddr6;
    use openscq30_lib::{DeviceModel, OpenSCQ30Session, storage::PairedDevice};

    use super::*;

    #[tokio::test]
    async fn reads_v1_profiles_and_nothing_without_the_file() {
        let dir = tempfile::tempdir().unwrap();
        assert!(equalizer_profiles(dir.path()).await.unwrap().is_empty());
        std::fs::write(
            dir.path().join("config.toml"),
            "[equalizer_custom_profiles.b]\nvolume_offsets = [1, 2]\n[equalizer_custom_profiles.a]\nvolume_offsets = [3]\n",
        )
        .unwrap();
        let profiles = equalizer_profiles(dir.path()).await.unwrap();
        assert_eq!(
            vec!["a", "b"],
            profiles
                .iter()
                .map(|(name, _)| name.as_str())
                .collect::<Vec<_>>()
        );
        assert_eq!(vec![1, 2], profiles[1].1.volume_offsets);
    }

    #[tokio::test]
    async fn a_migrated_profile_is_saved_but_not_applied() {
        let session = OpenSCQ30Session::new_with_in_memory_db().await.unwrap();
        session
            .pair(PairedDevice {
                mac_address: MacAddr6::nil(),
                model: DeviceModel::SoundcoreA3027,
                is_demo: true,
            })
            .await
            .unwrap();
        let device = session.connect(MacAddr6::nil()).await.unwrap();

        migrate_profile(device.as_ref(), "Test Profile".to_string(), vec![1; 8])
            .await
            .unwrap();

        let Setting::ModifiableSelect { setting, .. } =
            device.setting(&SettingId::CustomEqualizerProfile).unwrap()
        else {
            panic!("CustomEqualizerProfile is not a ModifiableSelect");
        };
        assert_eq!(vec![Cow::from("Test Profile")], setting.options);
        let Setting::Equalizer { value, .. } =
            device.setting(&SettingId::VolumeAdjustments).unwrap()
        else {
            panic!("VolumeAdjustments is not an Equalizer");
        };
        assert_eq!(vec![0; 8], value);
    }
}
