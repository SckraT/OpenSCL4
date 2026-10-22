use std::{collections::HashSet, sync::Arc};

use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, watch};

use crate::{
    connection::ConnectionStatus,
    storage::{self, OpenSCQ30Database, QuickPreset, QuickPresetField},
};

use super::{
    device::{self, OpenSCQ30Device, SettingWriteOutcome, WriteOutcome},
    settings::{self, SettingId},
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

pub struct QuickPresetActivation {
    device: Arc<dyn OpenSCQ30Device + Send + Sync>,
    connection_status: watch::Receiver<ConnectionStatus>,
    last_applied: watch::Sender<Option<String>>,
    activation: Mutex<()>,
}

impl QuickPresetActivation {
    pub fn new(device: Arc<dyn OpenSCQ30Device + Send + Sync>) -> Self {
        Self {
            connection_status: device.connection_status(),
            device,
            last_applied: watch::channel(None).0,
            activation: Mutex::new(()),
        }
    }

    pub fn last_applied(&self) -> Option<String> {
        if *self.connection_status.borrow() == ConnectionStatus::Connected {
            self.last_applied.borrow().clone()
        } else {
            None
        }
    }

    pub fn watch_for_changes(&self) -> watch::Receiver<Option<String>> {
        self.last_applied.subscribe()
    }

    pub async fn activate(
        &self,
        handler: &QuickPresetsHandler,
        name: String,
    ) -> device::Result<()> {
        let _activation = self.activation.lock().await;
        if *self.connection_status.borrow() != ConnectionStatus::Connected {
            return Err(device::Error::Disconnected);
        }
        let outcomes = handler.activate(self.device.as_ref(), name.clone()).await?;
        if *self.connection_status.borrow() != ConnectionStatus::Connected {
            return Err(device::Error::Disconnected);
        }
        if let Some(outcome) = outcomes
            .iter()
            .find(|outcome| !matches!(outcome.outcome, WriteOutcome::Applied { .. }))
        {
            return Err(device::Error::SettingWriteNotApplied {
                setting_id: outcome.setting_id,
            });
        }
        self.last_applied.send_replace(Some(name));
        Ok(())
    }
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
    /// field is toggled on or not will remain unchanged. Fields for settings that are currently unavailable on the
    /// device are kept as they were.
    pub async fn save(
        &self,
        device: &(dyn OpenSCQ30Device + Send + Sync),
        name: String,
    ) -> storage::Result<()> {
        let mut captured_setting_ids = HashSet::new();
        let mut fields = device
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
                    // Actions are commands rather than state, so there is no value to save and restore.
                    .filter(|setting| setting.mode().is_writable() && !setting.is_action())
                    .map(|setting| QuickPresetField {
                        setting_id,
                        value: setting.into(),
                        is_enabled: false,
                    })
            })
            .collect::<Vec<_>>();

        // Some settings are only available in certain states, such as the noise canceling strength while in
        // transparency mode. Don't discard the saved value of a field just because it can't be read right now.
        let unavailable_fields = match self
            .database
            .fetch_quick_preset(device.model(), name.clone())
            .await
        {
            Ok(existing) => existing
                .fields
                .into_iter()
                .filter(|field| device.setting(&field.setting_id).is_none())
                .filter(|field| fields.iter().all(|f| f.setting_id != field.setting_id))
                .collect::<Vec<_>>(),
            Err(storage::Error::NotFound { .. }) => Vec::new(),
            Err(err) => return Err(err),
        };
        fields.extend(unavailable_fields);

        self.database
            .upsert_quick_preset(device.model(), QuickPreset { name, fields })
            .await
    }

    /// Toggles on/off a field for the specified quick preset. When a quick preset is activated, only the enabled
    /// fields will be set. The equalizer can only be restored as either a factory profile or a custom curve, so
    /// enabling one of the fields that describe it disables the others.
    pub async fn toggle_field(
        &self,
        device: &(dyn OpenSCQ30Device + Send + Sync),
        name: String,
        setting_id: SettingId,
        is_enabled: bool,
    ) -> storage::Result<()> {
        if is_enabled && EQUALIZER_SETTING_IDS.contains(&setting_id) {
            for conflicting in EQUALIZER_SETTING_IDS
                .into_iter()
                .filter(|other| *other != setting_id)
            {
                match self
                    .database
                    .toggle_quick_preset_field(device.model(), name.clone(), conflicting, false)
                    .await
                {
                    // The preset doesn't have to contain every equalizer field.
                    Ok(()) | Err(storage::Error::NotFound { .. }) => (),
                    Err(err) => return Err(err),
                }
            }
        }
        self.database
            .toggle_quick_preset_field(device.model(), name, setting_id, is_enabled)
            .await
    }

    /// Activates a quick preset, setting all enabled fields' settings.
    pub async fn activate(
        &self,
        device: &(dyn OpenSCQ30Device + Send + Sync),
        name: impl Into<String>,
    ) -> device::Result<Vec<SettingWriteOutcome>> {
        let quick_preset = self
            .database
            .fetch_quick_preset(device.model(), name.into())
            .await?;
        let settings = settings_to_apply(device, quick_preset.fields);
        let mut outcomes = device.set_setting_values(settings.clone()).await?;

        let retryable = outcomes
            .iter()
            .filter(|outcome| {
                matches!(
                    outcome.outcome,
                    WriteOutcome::Applied { .. } | WriteOutcome::Rejected { .. }
                )
            })
            .map(|outcome| outcome.setting_id)
            .collect::<HashSet<_>>();
        let retry = settings
            .iter()
            .filter(|(setting_id, value)| {
                retryable.contains(setting_id)
                    && device
                        .setting(setting_id)
                        .is_some_and(|setting| settings::Value::from(setting) != *value)
            })
            .cloned()
            .collect::<Vec<_>>();
        if !retry.is_empty() {
            for retry_outcome in device.set_setting_values(retry).await? {
                match outcomes
                    .iter_mut()
                    .find(|outcome| outcome.setting_id == retry_outcome.setting_id)
                {
                    Some(outcome) => *outcome = retry_outcome,
                    None => outcomes.push(retry_outcome),
                }
            }
        }

        Ok(outcomes)
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

/// The settings that all describe the same thing, the equalizer's curve, and so can't be restored independently.
const EQUALIZER_SETTING_IDS: [SettingId; 3] = [
    SettingId::PresetEqualizerProfile,
    SettingId::CustomEqualizerProfile,
    SettingId::VolumeAdjustments,
];

/// Picks the enabled fields that should be set when activating a quick preset, in the order they should be set.
fn settings_to_apply(
    device: &(dyn OpenSCQ30Device + Send + Sync),
    fields: Vec<QuickPresetField>,
) -> Vec<(SettingId, settings::Value)> {
    let mut settings = fields
        .into_iter()
        .filter(|field| field.is_enabled)
        // Presets saved by older versions may contain actions, which would run every time the preset is activated.
        .filter(|field| {
            !device
                .setting(&field.setting_id)
                .is_some_and(|setting| setting.is_action())
        })
        .map(|field| (field.setting_id, field.value))
        .collect::<Vec<_>>();

    // Setting the volume adjustments always switches to the custom profile, so if a factory profile and a curve are
    // both enabled, applying both would undo the factory profile. Presets saved by older versions may do this.
    let has_factory_profile = settings.iter().any(|(setting_id, value)| {
        *setting_id == SettingId::PresetEqualizerProfile
            && value.try_as_optional_str().is_ok_and(|name| name.is_some())
    });
    if has_factory_profile {
        settings.retain(|(setting_id, _)| {
            !matches!(
                setting_id,
                SettingId::CustomEqualizerProfile | SettingId::VolumeAdjustments
            )
        });
    }
    settings
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex as StdMutex;

    use async_trait::async_trait;
    use tokio::sync::{mpsc, oneshot};

    use crate::DeviceModel;

    use super::*;

    #[derive(Debug)]
    enum WriteBehavior {
        Apply,
        Ignore,
        Fail,
    }

    struct ControlledDevice {
        inner: Arc<dyn OpenSCQ30Device + Send + Sync>,
        status: watch::Sender<ConnectionStatus>,
        writes: mpsc::UnboundedSender<oneshot::Sender<WriteBehavior>>,
        write_count: StdMutex<usize>,
    }

    #[async_trait]
    impl OpenSCQ30Device for ControlledDevice {
        fn connection_status(&self) -> watch::Receiver<ConnectionStatus> {
            self.status.subscribe()
        }

        fn session_status(&self) -> watch::Receiver<crate::api::connection::SessionStatus> {
            self.inner.session_status()
        }

        fn model(&self) -> DeviceModel {
            self.inner.model()
        }

        fn categories(&self) -> Vec<settings::CategoryId> {
            self.inner.categories()
        }

        fn settings_in_category(&self, id: &settings::CategoryId) -> Vec<SettingId> {
            self.inner.settings_in_category(id)
        }

        fn setting(&self, id: &SettingId) -> Option<settings::Setting> {
            self.inner.setting(id)
        }

        fn watch_for_changes(&self) -> watch::Receiver<()> {
            self.inner.watch_for_changes()
        }

        async fn set_setting_values(
            &self,
            values: Vec<(SettingId, settings::Value)>,
        ) -> device::Result<Vec<SettingWriteOutcome>> {
            *self.write_count.lock().unwrap() += 1;
            let (reply, outcome) = oneshot::channel();
            self.writes.send(reply).unwrap();
            match outcome.await.unwrap() {
                WriteBehavior::Apply => self.inner.set_setting_values(values).await,
                WriteBehavior::Ignore => Ok(values
                    .into_iter()
                    .map(|(setting_id, _)| SettingWriteOutcome {
                        setting_id,
                        outcome: WriteOutcome::Rejected {
                            observed: self.inner.setting(&setting_id).map(settings::Value::from),
                        },
                    })
                    .collect()),
                WriteBehavior::Fail => Err(device::Error::ActionTimedOut {
                    action: "test write",
                }),
            }
        }
    }

    async fn activation_setup() -> (
        Arc<ControlledDevice>,
        mpsc::UnboundedReceiver<oneshot::Sender<WriteBehavior>>,
        QuickPresetsHandler,
    ) {
        let (inner, database, handler) = connect_demo_a3954().await;
        let settings::Setting::Toggle { value } =
            inner.setting(&SettingId::WearingDetection).unwrap()
        else {
            panic!("demo supports wearing detection");
        };
        for name in ["First", "Second"] {
            database
                .upsert_quick_preset(
                    inner.model(),
                    QuickPreset {
                        name: name.into(),
                        fields: vec![field(SettingId::WearingDetection, (!value).into(), true)],
                    },
                )
                .await
                .unwrap();
        }
        let (writes, requests) = mpsc::unbounded_channel();
        (
            Arc::new(ControlledDevice {
                inner,
                status: watch::channel(ConnectionStatus::Connected).0,
                writes,
                write_count: StdMutex::new(0),
            }),
            requests,
            handler,
        )
    }

    #[tokio::test(start_paused = true)]
    async fn activation_records_only_success_and_shares_it_across_handlers() {
        let (device, mut writes, handler) = activation_setup().await;
        let activation = QuickPresetActivation::new(device);
        let mut changes = activation.watch_for_changes();
        let activate = activation.activate(&handler, "First".into());
        let finish = async {
            let reply = writes.recv().await.unwrap();
            assert_eq!(activation.last_applied(), None);
            assert!(!changes.has_changed().unwrap());
            reply.send(WriteBehavior::Apply).unwrap();
        };
        let (result, ()) = tokio::join!(activate, finish);
        result.unwrap();
        changes.changed().await.unwrap();
        assert_eq!(activation.last_applied().as_deref(), Some("First"));

        let another_handler = QuickPresetsHandler::new(handler.database.clone());
        assert!(
            activation
                .activate(&another_handler, "Missing".into())
                .await
                .is_err()
        );
        assert_eq!(activation.last_applied().as_deref(), Some("First"));
        assert!(!changes.has_changed().unwrap());

        let (result, ()) = tokio::join!(
            activation.activate(&another_handler, "Second".into()),
            async {
                writes
                    .recv()
                    .await
                    .unwrap()
                    .send(WriteBehavior::Apply)
                    .unwrap();
            }
        );
        result.unwrap();
        assert_eq!(activation.last_applied().as_deref(), Some("Second"));
    }

    #[tokio::test(start_paused = true)]
    async fn activation_waits_for_retry_and_retains_previous_success_on_write_failure() {
        let (device, mut writes, handler) = activation_setup().await;
        let activation = QuickPresetActivation::new(device.clone());
        let (result, ()) = tokio::join!(activation.activate(&handler, "First".into()), async {
            writes
                .recv()
                .await
                .unwrap()
                .send(WriteBehavior::Apply)
                .unwrap();
        });
        result.unwrap();
        let settings::Setting::Toggle { value } =
            device.setting(&SettingId::WearingDetection).unwrap()
        else {
            unreachable!()
        };
        device
            .inner
            .set_setting_values(vec![(SettingId::WearingDetection, (!value).into())])
            .await
            .unwrap();
        let (result, ()) = tokio::join!(activation.activate(&handler, "Second".into()), async {
            writes
                .recv()
                .await
                .unwrap()
                .send(WriteBehavior::Ignore)
                .unwrap();
            let retry = writes.recv().await.unwrap();
            assert_eq!(activation.last_applied().as_deref(), Some("First"));
            retry.send(WriteBehavior::Fail).unwrap();
        });
        assert!(result.is_err());
        assert_eq!(activation.last_applied().as_deref(), Some("First"));
    }

    #[tokio::test(start_paused = true)]
    async fn disconnected_activation_cannot_mark_a_new_connection() {
        let (device, mut writes, handler) = activation_setup().await;
        let old = QuickPresetActivation::new(device.clone());
        let new = QuickPresetActivation::new(device.inner.clone());
        let (result, ()) = tokio::join!(old.activate(&handler, "First".into()), async {
            let reply = writes.recv().await.unwrap();
            device.status.send_replace(ConnectionStatus::Disconnected);
            reply.send(WriteBehavior::Apply).unwrap();
        });
        assert!(matches!(result, Err(device::Error::Disconnected)));
        assert_eq!(old.last_applied(), None);
        assert_eq!(new.last_applied(), None);
        let writes_before = *device.write_count.lock().unwrap();
        assert!(old.activate(&handler, "Second".into()).await.is_err());
        assert_eq!(*device.write_count.lock().unwrap(), writes_before);
    }

    #[tokio::test(start_paused = true)]
    async fn disconnect_and_new_connection_clear_a_successful_activation() {
        let (device, mut writes, handler) = activation_setup().await;
        let old = QuickPresetActivation::new(device.clone());
        let (result, ()) = tokio::join!(old.activate(&handler, "First".into()), async {
            writes
                .recv()
                .await
                .unwrap()
                .send(WriteBehavior::Apply)
                .unwrap();
        });
        result.unwrap();
        assert_eq!(old.last_applied().as_deref(), Some("First"));
        device.status.send_replace(ConnectionStatus::Disconnected);
        assert_eq!(old.last_applied(), None);
        let new = QuickPresetActivation::new(device.inner.clone());
        assert_eq!(new.last_applied(), None);
        assert_eq!(
            handler
                .quick_presets(device.inner.as_ref())
                .await
                .unwrap()
                .len(),
            2
        );
    }

    #[tokio::test(start_paused = true)]
    async fn simultaneous_activations_finish_in_order_without_losing_previous_success() {
        let (device, mut writes, handler) = activation_setup().await;
        let activation = QuickPresetActivation::new(device);
        let (first, second, ()) = tokio::join!(
            activation.activate(&handler, "First".into()),
            activation.activate(&handler, "Second".into()),
            async {
                let first = writes.recv().await.unwrap();
                tokio::task::yield_now().await;
                assert!(matches!(
                    writes.try_recv(),
                    Err(mpsc::error::TryRecvError::Empty)
                ));
                first.send(WriteBehavior::Apply).unwrap();
                let second = writes.recv().await.unwrap();
                assert_eq!(activation.last_applied().as_deref(), Some("First"));
                second.send(WriteBehavior::Fail).unwrap();
            },
        );
        first.unwrap();
        assert!(second.is_err());
        assert_eq!(activation.last_applied().as_deref(), Some("First"));
    }

    async fn connect_demo_a3954() -> (
        Arc<dyn OpenSCQ30Device + Send + Sync>,
        Arc<OpenSCQ30Database>,
        QuickPresetsHandler,
    ) {
        let database = Arc::new(OpenSCQ30Database::new_in_memory().await.unwrap());
        let model = DeviceModel::SoundcoreA3954;
        let registry = model.demo_device_registry(database.clone()).await.unwrap();
        let device = registry.connect(model.demo_mac_address()).await.unwrap();
        let handler = QuickPresetsHandler::new(database.clone());
        (device, database, handler)
    }

    fn current_value(device: &dyn OpenSCQ30Device, setting_id: SettingId) -> settings::Value {
        device.setting(&setting_id).unwrap().into()
    }

    fn field(setting_id: SettingId, value: settings::Value, is_enabled: bool) -> QuickPresetField {
        QuickPresetField {
            setting_id,
            value,
            is_enabled,
        }
    }

    async fn fetch_field(
        database: &OpenSCQ30Database,
        name: &str,
        setting_id: SettingId,
    ) -> Option<QuickPresetField> {
        database
            .fetch_quick_preset(DeviceModel::SoundcoreA3954, name.into())
            .await
            .unwrap()
            .fields
            .into_iter()
            .find(|field| field.setting_id == setting_id)
    }

    #[tokio::test(start_paused = true)]
    async fn save_does_not_capture_action_settings() {
        let (device, database, handler) = connect_demo_a3954().await;

        handler
            .save(device.as_ref(), "Preset".into())
            .await
            .unwrap();

        assert_eq!(
            None,
            fetch_field(&database, "Preset", SettingId::ResetButtonsToDefault).await,
            "actions have no state, so they shouldn't be saved",
        );
        assert!(
            fetch_field(&database, "Preset", SettingId::LeftSinglePress)
                .await
                .is_some(),
            "regular settings should still be saved",
        );
    }

    #[tokio::test(start_paused = true)]
    async fn activate_does_not_run_action_settings() {
        let (device, database, handler) = connect_demo_a3954().await;
        let original = current_value(device.as_ref(), SettingId::LeftSinglePress);
        device
            .set_setting_values(vec![(SettingId::LeftSinglePress, Some("NextSong").into())])
            .await
            .unwrap();
        assert_ne!(
            original,
            current_value(device.as_ref(), SettingId::LeftSinglePress),
            "the button should have been changed for the test to mean anything",
        );
        // Older versions saved actions as if they were toggles, so presets like this can already exist.
        database
            .upsert_quick_preset(
                DeviceModel::SoundcoreA3954,
                QuickPreset {
                    name: "Preset".into(),
                    fields: vec![field(SettingId::ResetButtonsToDefault, false.into(), true)],
                },
            )
            .await
            .unwrap();

        handler.activate(device.as_ref(), "Preset").await.unwrap();

        assert_eq!(
            settings::Value::from(Some("NextSong")),
            current_value(device.as_ref(), SettingId::LeftSinglePress),
            "activating a quick preset shouldn't reset the buttons",
        );
    }

    #[tokio::test(start_paused = true)]
    async fn save_keeps_enabled_field_for_unavailable_setting() {
        let (device, database, handler) = connect_demo_a3954().await;
        device
            .set_setting_values(vec![
                (SettingId::AmbientSoundMode, "NoiseCanceling".into()),
                (SettingId::ManualNoiseCanceling, 5.into()),
            ])
            .await
            .unwrap();
        handler.save(device.as_ref(), "ANC".into()).await.unwrap();
        handler
            .toggle_field(
                device.as_ref(),
                "ANC".into(),
                SettingId::ManualNoiseCanceling,
                true,
            )
            .await
            .unwrap();

        // The noise canceling strength can't be read in transparency mode
        device
            .set_setting_values(vec![(SettingId::AmbientSoundMode, "Transparency".into())])
            .await
            .unwrap();
        assert!(device.setting(&SettingId::ManualNoiseCanceling).is_none());
        handler.save(device.as_ref(), "ANC".into()).await.unwrap();

        assert_eq!(
            Some(field(SettingId::ManualNoiseCanceling, 5.into(), true)),
            fetch_field(&database, "ANC", SettingId::ManualNoiseCanceling).await,
            "the field should be unchanged since there was no new value to save",
        );
        assert_eq!(
            Some(settings::Value::from("Transparency")),
            fetch_field(&database, "ANC", SettingId::AmbientSoundMode)
                .await
                .map(|field| field.value),
            "available fields should still be updated",
        );
    }

    #[tokio::test(start_paused = true)]
    async fn save_new_preset_does_not_invent_unavailable_fields() {
        let (device, database, handler) = connect_demo_a3954().await;
        device
            .set_setting_values(vec![(SettingId::AmbientSoundMode, "Transparency".into())])
            .await
            .unwrap();

        handler.save(device.as_ref(), "New".into()).await.unwrap();

        assert_eq!(
            None,
            fetch_field(&database, "New", SettingId::ManualNoiseCanceling).await,
        );
    }

    #[tokio::test(start_paused = true)]
    async fn eq_and_spatial_audio_are_applied_in_one_activation() {
        activate_eq_and_spatial_audio_preset(false).await;
    }

    #[tokio::test(start_paused = true)]
    async fn eq_and_spatial_audio_are_applied_in_one_activation_when_spatial_audio_is_on() {
        activate_eq_and_spatial_audio_preset(true).await;
    }

    /// Changing the equalizer turns off spatial audio on the A3954, which must not stop a quick preset from turning
    /// it on.
    async fn activate_eq_and_spatial_audio_preset(is_spatial_audio_on_initially: bool) {
        let (device, database, handler) = connect_demo_a3954().await;
        // SpatialAudio is registered by two modules on this device, so saving produces two fields with that setting
        // id and toggling either fails. Create the quick preset directly instead.
        database
            .upsert_quick_preset(
                DeviceModel::SoundcoreA3954,
                QuickPreset {
                    name: "Spatial".into(),
                    fields: vec![
                        field(SettingId::VolumeAdjustments, vec![10i16; 8].into(), true),
                        field(SettingId::SpatialAudio, true.into(), true),
                    ],
                },
            )
            .await
            .unwrap();
        device
            .set_setting_values(vec![(SettingId::VolumeAdjustments, vec![20i16; 8].into())])
            .await
            .unwrap();
        device
            .set_setting_values(vec![(
                SettingId::SpatialAudio,
                is_spatial_audio_on_initially.into(),
            )])
            .await
            .unwrap();
        assert_eq!(
            settings::Value::from(is_spatial_audio_on_initially),
            current_value(device.as_ref(), SettingId::SpatialAudio),
        );

        handler.activate(device.as_ref(), "Spatial").await.unwrap();

        assert_eq!(
            settings::Value::from(vec![10i16; 8]),
            current_value(device.as_ref(), SettingId::VolumeAdjustments),
        );
        assert_eq!(
            settings::Value::from(true),
            current_value(device.as_ref(), SettingId::SpatialAudio),
            "spatial audio should be on after a single activation",
        );
    }

    #[tokio::test(start_paused = true)]
    async fn factory_profile_wins_over_volume_adjustments() {
        let (device, database, handler) = connect_demo_a3954().await;
        device
            .set_setting_values(vec![(SettingId::VolumeAdjustments, vec![5i16; 8].into())])
            .await
            .unwrap();
        // Older versions allowed both to be enabled at the same time, even though setting the volume adjustments
        // always replaces the factory profile with a custom one.
        database
            .upsert_quick_preset(
                DeviceModel::SoundcoreA3954,
                QuickPreset {
                    name: "Acoustic".into(),
                    fields: vec![
                        field(
                            SettingId::PresetEqualizerProfile,
                            Some("Acoustic").into(),
                            true,
                        ),
                        field(SettingId::VolumeAdjustments, vec![1i16; 8].into(), true),
                    ],
                },
            )
            .await
            .unwrap();

        handler.activate(device.as_ref(), "Acoustic").await.unwrap();

        assert_eq!(
            settings::Value::from(Some("Acoustic")),
            current_value(device.as_ref(), SettingId::PresetEqualizerProfile),
        );
    }

    #[tokio::test(start_paused = true)]
    async fn volume_adjustments_win_when_there_is_no_factory_profile() {
        let (device, database, handler) = connect_demo_a3954().await;
        device
            .set_setting_values(vec![(
                SettingId::PresetEqualizerProfile,
                Some("Acoustic").into(),
            )])
            .await
            .unwrap();
        database
            .upsert_quick_preset(
                DeviceModel::SoundcoreA3954,
                QuickPreset {
                    name: "Custom".into(),
                    fields: vec![
                        field(
                            SettingId::PresetEqualizerProfile,
                            settings::Value::OptionalString(None),
                            true,
                        ),
                        field(SettingId::VolumeAdjustments, vec![1i16; 8].into(), true),
                    ],
                },
            )
            .await
            .unwrap();

        handler.activate(device.as_ref(), "Custom").await.unwrap();

        assert_eq!(
            settings::Value::OptionalString(None),
            current_value(device.as_ref(), SettingId::PresetEqualizerProfile),
        );
        assert_eq!(
            settings::Value::from(vec![1i16; 8]),
            current_value(device.as_ref(), SettingId::VolumeAdjustments),
        );
    }

    #[tokio::test(start_paused = true)]
    async fn toggling_on_an_equalizer_field_toggles_off_the_others() {
        let (device, database, handler) = connect_demo_a3954().await;
        device
            .set_setting_values(vec![(
                SettingId::PresetEqualizerProfile,
                Some("Acoustic").into(),
            )])
            .await
            .unwrap();
        handler
            .save(device.as_ref(), "Preset".into())
            .await
            .unwrap();
        handler
            .toggle_field(
                device.as_ref(),
                "Preset".into(),
                SettingId::PresetEqualizerProfile,
                true,
            )
            .await
            .unwrap();
        handler
            .toggle_field(
                device.as_ref(),
                "Preset".into(),
                SettingId::AmbientSoundMode,
                true,
            )
            .await
            .unwrap();

        handler
            .toggle_field(
                device.as_ref(),
                "Preset".into(),
                SettingId::VolumeAdjustments,
                true,
            )
            .await
            .unwrap();

        for (setting_id, expected) in [
            (SettingId::PresetEqualizerProfile, false),
            (SettingId::CustomEqualizerProfile, false),
            (SettingId::VolumeAdjustments, true),
            (SettingId::AmbientSoundMode, true),
        ] {
            assert_eq!(
                Some(expected),
                fetch_field(&database, "Preset", setting_id)
                    .await
                    .map(|field| field.is_enabled),
                "{setting_id:?}",
            );
        }
    }

    #[tokio::test(start_paused = true)]
    async fn toggling_off_an_equalizer_field_leaves_the_others_alone() {
        let (device, database, handler) = connect_demo_a3954().await;
        handler
            .save(device.as_ref(), "Preset".into())
            .await
            .unwrap();
        handler
            .toggle_field(
                device.as_ref(),
                "Preset".into(),
                SettingId::VolumeAdjustments,
                true,
            )
            .await
            .unwrap();

        handler
            .toggle_field(
                device.as_ref(),
                "Preset".into(),
                SettingId::PresetEqualizerProfile,
                false,
            )
            .await
            .unwrap();

        assert_eq!(
            Some(true),
            fetch_field(&database, "Preset", SettingId::VolumeAdjustments)
                .await
                .map(|field| field.is_enabled),
        );
    }

    #[tokio::test(start_paused = true)]
    async fn saves_each_setting_once_in_category_order_and_retains_enabled_fields() {
        use crate::api::settings::Setting;

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
        // The case's spatial audio flag used to share this id, which made a preset capture it twice.
        assert_eq!(
            category_setting_ids
                .iter()
                .filter(|&&setting_id| setting_id == SettingId::SpatialAudio)
                .count(),
            1,
            "only the earbuds' spatial audio uses this id"
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
                    .is_some_and(|setting| setting.mode().is_writable() && !setting.is_action())
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
