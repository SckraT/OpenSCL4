use std::{collections::HashSet, sync::Arc};

use serde::{Deserialize, Serialize};

use crate::storage::{self, OpenSCQ30Database, QuickPreset, QuickPresetField};

use super::{
    device::{self, OpenSCQ30Device},
    settings::SettingId,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum QuickPresetAction {
    Create(String, HashSet<SettingId>),
    Activate(String),
    Delete(String),
}

#[derive(Debug, Clone)]
pub struct QuickPresetsHandler {
    database: Arc<OpenSCQ30Database>,
}

impl QuickPresetsHandler {
    pub fn new(database: Arc<OpenSCQ30Database>) -> Self {
        Self { database }
    }

    /// Returns all saved quick presets for the device.
    pub async fn quick_presets(
        &self,
        device: &(dyn OpenSCQ30Device + Send + Sync),
    ) -> storage::Result<Vec<QuickPreset>> {
        self.database.fetch_all_quick_presets(device.model()).await
    }

    /// Saves the device's current settings to a quick preset. By default, all fields will be toggled off. If a quick
    /// preset with the specified name already exists, the setting values will be updated, but whether or not each
    /// field is toggled on or not will remain unchanged.
    pub async fn save(
        &self,
        device: &(dyn OpenSCQ30Device + Send + Sync),
        name: String,
    ) -> storage::Result<()> {
        let mut captured_setting_ids = HashSet::new();
        let fields = device
            .categories()
            .iter()
            .flat_map(|category_id| device.settings_in_category(category_id))
            // TODO make the SettingId blacklist a part of OpenSCQ30Device?
            .filter(|setting_id| {
                !matches!(
                    setting_id,
                    SettingId::ImportCustomEqualizerProfiles
                        | SettingId::ExportCustomEqualizerProfiles,
                )
            })
            // A setting can appear in multiple categories, but a preset should capture it only once.
            .filter(|setting_id| captured_setting_ids.insert(*setting_id))
            .filter_map(|setting_id| {
                device
                    .setting(&setting_id)
                    .filter(|setting| setting.mode().is_writable())
                    .map(|setting| QuickPresetField {
                        setting_id,
                        value: setting.into(),
                        is_enabled: false,
                    })
            })
            .collect::<Vec<_>>();

        self.database
            .upsert_quick_preset(device.model(), QuickPreset { name, fields })
            .await
    }

    /// Toggles on/off a field for the specified quick preset. When a quick preset is activated, only the enabled
    /// fields will be set.
    pub async fn toggle_field(
        &self,
        device: &(dyn OpenSCQ30Device + Send + Sync),
        name: String,
        setting_id: SettingId,
        is_enabled: bool,
    ) -> storage::Result<()> {
        self.database
            .toggle_quick_preset_field(device.model(), name, setting_id, is_enabled)
            .await
    }

    /// Activates a quick preset, setting all enabled fields' settings.
    pub async fn activate(
        &self,
        device: &(dyn OpenSCQ30Device + Send + Sync),
        name: impl Into<String>,
    ) -> device::Result<()> {
        let quick_preset = self
            .database
            .fetch_quick_preset(device.model(), name.into())
            .await?;
        let settings = quick_preset
            .fields
            .into_iter()
            .filter_map(|field| field.is_enabled.then_some((field.setting_id, field.value)))
            .collect::<Vec<_>>();
        device.set_setting_values(settings).await
    }

    /// Deletes a quick preset.
    pub async fn delete(
        &self,
        device: &(dyn OpenSCQ30Device + Send + Sync),
        name: String,
    ) -> storage::Result<()> {
        self.database
            .delete_quick_preset(device.model(), name)
            .await
    }
}

#[cfg(test)]
mod tests {
    use crate::{api::settings::Setting, devices::DeviceModel};

    use super::*;

    #[tokio::test(start_paused = true)]
    async fn saves_shared_settings_once_in_category_order_and_retains_enabled_fields() {
        let database = Arc::new(OpenSCQ30Database::new_in_memory().await.unwrap());
        let model = DeviceModel::SoundcoreA3954;
        let registry = model.demo_device_registry(database.clone()).await.unwrap();
        let device = registry.connect(model.demo_mac_address()).await.unwrap();
        let handler = QuickPresetsHandler::new(database.clone());
        let name = "Liberty 4 Pro".to_owned();
        let category_setting_ids = device
            .categories()
            .iter()
            .flat_map(|category_id| device.settings_in_category(category_id))
            .collect::<Vec<_>>();
        assert!(
            category_setting_ids
                .iter()
                .filter(|&&setting_id| setting_id == SettingId::SpatialAudio)
                .count()
                > 1,
            "the real device exposes spatial audio in more than one category"
        );

        handler.save(device.as_ref(), name.clone()).await.unwrap();
        let saved = database
            .fetch_quick_preset(model, name.clone())
            .await
            .unwrap();
        let expected_setting_ids = category_setting_ids
            .iter()
            .copied()
            .filter(|setting_id| {
                !matches!(
                    setting_id,
                    SettingId::ImportCustomEqualizerProfiles
                        | SettingId::ExportCustomEqualizerProfiles
                ) && device
                    .setting(setting_id)
                    .is_some_and(|setting| setting.mode().is_writable())
            })
            .collect::<HashSet<_>>();
        let captured_setting_ids = saved
            .fields
            .iter()
            .map(|field| field.setting_id)
            .collect::<HashSet<_>>();
        assert_eq!(captured_setting_ids, expected_setting_ids);
        assert_eq!(saved.fields.len(), expected_setting_ids.len());
        assert!(saved.fields.iter().all(|field| !field.is_enabled));

        let first_appearance_indices = saved
            .fields
            .iter()
            .map(|field| {
                category_setting_ids
                    .iter()
                    .position(|&setting_id| setting_id == field.setting_id)
                    .unwrap()
            })
            .collect::<Vec<_>>();
        assert!(
            first_appearance_indices
                .windows(2)
                .all(|pair| pair[0] < pair[1])
        );

        handler
            .toggle_field(device.as_ref(), name.clone(), SettingId::SpatialAudio, true)
            .await
            .unwrap();
        let Some(Setting::Toggle { value }) = device.setting(&SettingId::WearingDetection) else {
            panic!("Liberty 4 Pro supports wearing detection");
        };
        device
            .set_setting_values(vec![(SettingId::WearingDetection, (!value).into())])
            .await
            .unwrap();
        handler.save(device.as_ref(), name.clone()).await.unwrap();
        let refreshed = database.fetch_quick_preset(model, name).await.unwrap();
        assert_eq!(
            refreshed
                .fields
                .iter()
                .map(|field| field.setting_id)
                .collect::<Vec<_>>(),
            saved
                .fields
                .iter()
                .map(|field| field.setting_id)
                .collect::<Vec<_>>()
        );
        for field in refreshed.fields {
            assert_eq!(
                field.is_enabled,
                field.setting_id == SettingId::SpatialAudio
            );
            if field.setting_id == SettingId::WearingDetection {
                assert_eq!(field.value, (!value).into());
            }
        }
    }
}
