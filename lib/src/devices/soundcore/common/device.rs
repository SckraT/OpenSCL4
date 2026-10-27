use std::{marker::PhantomData, pin::Pin, sync::Arc};

use async_trait::async_trait;
use indexmap::IndexMap;
use openscq30_i18n::Translate;
use openscq30_lib_has::{Has, MaybeHas};
use paste::paste;
use tokio::{
    select,
    sync::{Semaphore, mpsc, watch},
};

use crate::{
    api::{
        connection::{
            ConnectionDescriptor, ConnectionSession, ConnectionSessionConfig, ConnectionStatus,
            ReconnectHook, RfcommBackend, SessionStatus,
        },
        device::{self, OpenSCQ30Device, OpenSCQ30DeviceRegistry},
        settings::{CategoryId, Setting, SettingId, Value},
    },
    connection::RfcommServiceSelectionStrategy,
    devices::{
        DeviceModel,
        soundcore::{
            self,
            common::{
                modules::{
                    self, button_configuration::ButtonConfigurationSettings,
                    reset_button_configuration::ResetButtonConfigurationPending,
                },
                packet::{self, PacketIOController, outbound::ToPacket},
                state::Update,
                structures::{
                    AmbientSoundModeCycleTws, AutoPlayPause, AutoPowerOff, BatteryLevel,
                    CaseBatteryLevel, DisableAllButtons, DualBatteryLevel, DualConnections,
                    EqualizerConfiguration, GamingMode, Ldac, LimitHighVolume, LowBatteryPrompt,
                    SoundLeakCompensation, SurroundSound, TouchLock, TouchTone, VoicePrompt,
                    WearingDetection, WearingTone, button_configuration::ButtonStatusCollection,
                },
            },
        },
    },
    storage::OpenSCQ30Database,
};

use super::{
    modules::{
        ModuleCollection, ModuleCollectionSpawnPacketHandlerExt, sound_modes::AvailableSoundModes,
    },
    packet::{
        inbound::{FromPacketBody, TryToPacket},
        outbound::RequestState,
    },
    structures::{
        AgeRange, AmbientSoundModeCycle, BasicHearId, CustomHearId, DualBattery,
        DualFirmwareVersion, FirmwareVersion, Gender, SerialNumber, SingleBattery, SoundModes,
        TwsStatus,
    },
};

type FetchStateFn<StateType> = Arc<
    dyn Fn(
            Arc<PacketIOController>,
        ) -> Pin<Box<dyn Future<Output = device::Result<StateType>> + Send>>
        + Send
        + Sync,
>;

pub async fn fetch_state_from_state_update_packet<State, StateUpdate>(
    packet_io: Arc<PacketIOController>,
) -> device::Result<State>
where
    StateUpdate: FromPacketBody + Default + Into<State>,
{
    let state_update_packet: StateUpdate = packet_io
        .send_with_response(&RequestState.to_packet())
        .await?
        .try_to_packet()
        .map_err(|err| device::Error::other(err))?;
    Ok(state_update_packet.into())
}

#[derive(Copy, Clone)]
pub struct SoundcoreDeviceConfig {
    /// Most devices have a checksum at the end of their packets, but for the ones that don't,
    /// this can be set to ChecksumKind::None to disable checksums.
    pub checksum_kind: packet::ChecksumKind,
    pub rfcomm_service_selection_strategy: RfcommServiceSelectionStrategy,
}

impl Default for SoundcoreDeviceConfig {
    fn default() -> Self {
        Self {
            checksum_kind: Default::default(),
            rfcomm_service_selection_strategy: RfcommServiceSelectionStrategy::Dynamic(
                |service_uuids| {
                    service_uuids
                        .into_iter()
                        .find(soundcore::is_soundcore_vendor_rfcomm_uuid)
                        .unwrap_or(soundcore::RFCOMM_UUID)
                },
            ),
        }
    }
}

pub struct SoundcoreDeviceRegistry<StateType> {
    backend: Arc<dyn RfcommBackend + Send + Sync>,
    database: Arc<OpenSCQ30Database>,
    device_model: DeviceModel,
    fetch_state: FetchStateFn<StateType>,
    config: SoundcoreDeviceConfig,
    is_demo: bool,
    _state: PhantomData<StateType>,
}

impl<StateType> SoundcoreDeviceRegistry<StateType> {
    pub fn new(
        backend: Arc<dyn RfcommBackend + Send + Sync>,
        database: Arc<OpenSCQ30Database>,
        device_model: DeviceModel,
        fetch_state: FetchStateFn<StateType>,
        config: SoundcoreDeviceConfig,
    ) -> Self {
        Self::new_with_mode(backend, database, device_model, fetch_state, config, false)
    }

    pub fn new_demo(
        backend: Arc<dyn RfcommBackend + Send + Sync>,
        database: Arc<OpenSCQ30Database>,
        device_model: DeviceModel,
        fetch_state: FetchStateFn<StateType>,
        config: SoundcoreDeviceConfig,
    ) -> Self {
        Self::new_with_mode(backend, database, device_model, fetch_state, config, true)
    }

    fn new_with_mode(
        backend: Arc<dyn RfcommBackend + Send + Sync>,
        database: Arc<OpenSCQ30Database>,
        device_model: DeviceModel,
        fetch_state: FetchStateFn<StateType>,
        config: SoundcoreDeviceConfig,
        is_demo: bool,
    ) -> Self {
        Self {
            backend,
            device_model,
            database,
            fetch_state,
            config,
            is_demo,
            _state: PhantomData,
        }
    }
}

#[async_trait]
impl<StateType> OpenSCQ30DeviceRegistry for SoundcoreDeviceRegistry<StateType>
where
    StateType: Clone + Send + Sync + 'static,
    Self: BuildDevice<StateType>,
{
    async fn devices(&self) -> device::Result<Vec<ConnectionDescriptor>> {
        self.backend
            .devices()
            .await
            .map(|descriptors| {
                descriptors
                    .into_iter()
                    .map(|d| ConnectionDescriptor {
                        name: d.name,
                        mac_address: d.mac_address,
                    })
                    .collect()
            })
            .map_err(Into::into)
    }

    async fn connect(
        &self,
        mac_address: macaddr::MacAddr6,
    ) -> device::Result<Arc<dyn OpenSCQ30Device + Send + Sync>> {
        let session = ConnectionSession::connect(
            self.backend.clone(),
            mac_address,
            self.config.rfcomm_service_selection_strategy,
            ConnectionSessionConfig::default(),
        )
        .await?;
        let mut builder = SoundcoreDeviceBuilder::new(
            self.database.clone(),
            Arc::new(session),
            self.device_model,
            self.fetch_state.clone(),
            self.config,
            self.is_demo,
        )
        .await?;
        Self::build_device(&mut builder).await;
        Ok(Arc::new(builder.build().await))
    }
}

pub trait BuildDevice<StateType>
where
    StateType: Clone + Send + Sync,
{
    fn build_device(
        builder: &mut SoundcoreDeviceBuilder<StateType>,
    ) -> impl Future<Output = ()> + Send;
}

pub struct SoundcoreDeviceBuilder<StateType> {
    device_model: DeviceModel,
    state_sender: watch::Sender<StateType>,
    module_collection: ModuleCollection<StateType>,
    packet_io_controller: Arc<PacketIOController>,
    database: Arc<OpenSCQ30Database>,
    packet_receiver: mpsc::Receiver<packet::Inbound>,
    change_notify: watch::Sender<()>,
    is_demo: bool,
}

macro_rules! flag {
    ($struct_name:ident) => {
        paste! {
            pub fn [< $struct_name:snake >](&mut self)
            where
                StateType: MaybeHas<$struct_name>,
            {
                self.module_collection.[< add_ $struct_name:snake >]();
            }
        }
    };
}

impl<StateType> SoundcoreDeviceBuilder<StateType>
where
    StateType: Send + Sync + Clone + 'static,
{
    pub async fn new(
        database: Arc<OpenSCQ30Database>,
        session: Arc<ConnectionSession>,
        device_model: DeviceModel,
        fetch_state: FetchStateFn<StateType>,
        config: SoundcoreDeviceConfig,
        is_demo: bool,
    ) -> device::Result<Self> {
        let (packet_io_controller, packet_receiver) =
            PacketIOController::new_with_session(session.clone(), config.checksum_kind).await?;
        let packet_io_controller = Arc::new(packet_io_controller);
        let state = fetch_state(packet_io_controller.clone()).await?;
        let (state_sender, _) = watch::channel::<StateType>(state);

        let reconnect_fetch_state = fetch_state.clone();
        let reconnect_packet_io = Arc::downgrade(&packet_io_controller);
        let reconnect_state_sender = state_sender.clone();
        let hook: ReconnectHook = Arc::new(move || {
            let fetch_state = reconnect_fetch_state.clone();
            let packet_io = reconnect_packet_io.clone();
            let state_sender = reconnect_state_sender.clone();
            Box::pin(async move {
                let packet_io = packet_io
                    .upgrade()
                    .ok_or_else(|| "device was dropped".to_owned())?;
                let state = fetch_state(packet_io)
                    .await
                    .map_err(|err| format!("state refresh failed: {err}"))?;
                state_sender.send_replace(state);
                Ok(())
            })
        });
        session.set_reconnect_hook(hook);
        // The watchdog is covered directly by PacketIOController tests. It is not started in
        // crate unit tests because paused-time harnesses jump straight to its timer.
        #[cfg(not(test))]
        if !is_demo {
            packet_io_controller.start_watchdog();
        }

        let module_collection = ModuleCollection::<StateType>::default();

        Ok(Self {
            device_model,
            state_sender,
            packet_io_controller,
            module_collection,
            database,
            packet_receiver,
            change_notify: watch::channel(()).0,
            is_demo,
        })
    }

    pub async fn build(self) -> SoundcoreDeviceTemplate<StateType> {
        SoundcoreDeviceTemplate::new(
            self.packet_io_controller,
            self.state_sender,
            self.module_collection,
            self.packet_receiver,
            self.device_model,
            self.change_notify.subscribe(),
            self.is_demo,
        )
        .await
    }

    pub fn module_collection(&mut self) -> &mut ModuleCollection<StateType> {
        &mut self.module_collection
    }

    pub fn packet_io_controller(&self) -> &Arc<PacketIOController> {
        &self.packet_io_controller
    }

    pub fn database(&self) -> Arc<OpenSCQ30Database> {
        self.database.clone()
    }

    pub fn device_model(&self) -> DeviceModel {
        self.device_model
    }

    pub fn change_notify(&self) -> watch::Sender<()> {
        self.change_notify.clone()
    }

    pub fn sound_modes(&mut self, available_sound_modes: AvailableSoundModes)
    where
        StateType: Has<SoundModes>,
    {
        self.module_collection
            .add_sound_modes(self.packet_io_controller.clone(), available_sound_modes);
    }

    pub async fn equalizer<
        const CHANNELS: usize,
        const BANDS: usize,
        const VISIBLE_BANDS: usize,
        const PRESET_BANDS: usize,
        const MIN_VOLUME: i16,
        const MAX_VOLUME: i16,
        const FRACTION_DIGITS: u8,
    >(
        &mut self,
        settings: modules::equalizer::EqualizerModuleSettings<
            VISIBLE_BANDS,
            PRESET_BANDS,
            MIN_VOLUME,
            MAX_VOLUME,
            FRACTION_DIGITS,
        >,
    ) where
        StateType:
            Has<EqualizerConfiguration<CHANNELS, BANDS, MIN_VOLUME, MAX_VOLUME, FRACTION_DIGITS>>,
    {
        self.module_collection
            .add_equalizer(
                self.packet_io_controller.clone(),
                self.database.clone(),
                self.device_model,
                self.change_notify.clone(),
                settings,
            )
            .await;
    }

    pub async fn equalizer_tws<
        const CHANNELS: usize,
        const BANDS: usize,
        const VISIBLE_BANDS: usize,
        const PRESET_BANDS: usize,
        const MIN_VOLUME: i16,
        const MAX_VOLUME: i16,
        const FRACTION_DIGITS: u8,
    >(
        &mut self,
        settings: modules::equalizer::EqualizerModuleSettings<
            VISIBLE_BANDS,
            PRESET_BANDS,
            MIN_VOLUME,
            MAX_VOLUME,
            FRACTION_DIGITS,
        >,
    ) where
        StateType: Has<EqualizerConfiguration<CHANNELS, BANDS, MIN_VOLUME, MAX_VOLUME, FRACTION_DIGITS>>
            + Has<TwsStatus>,
    {
        self.module_collection
            .add_equalizer_tws(
                self.packet_io_controller.clone(),
                self.database.clone(),
                self.device_model,
                self.change_notify.clone(),
                settings,
            )
            .await;
    }

    pub async fn equalizer_with_drc<
        const CHANNELS: usize,
        const BANDS: usize,
        const VISIBLE_BANDS: usize,
        const PRESET_BANDS: usize,
        const MIN_VOLUME: i16,
        const MAX_VOLUME: i16,
        const FRACTION_DIGITS: u8,
    >(
        &mut self,
        settings: modules::equalizer::EqualizerModuleSettings<
            VISIBLE_BANDS,
            PRESET_BANDS,
            MIN_VOLUME,
            MAX_VOLUME,
            FRACTION_DIGITS,
        >,
    ) where
        StateType:
            Has<EqualizerConfiguration<CHANNELS, BANDS, MIN_VOLUME, MAX_VOLUME, FRACTION_DIGITS>>,
    {
        self.module_collection
            .add_equalizer_with_drc(
                self.packet_io_controller.clone(),
                self.database.clone(),
                self.device_model,
                self.change_notify.clone(),
                settings,
            )
            .await;
    }

    pub async fn equalizer_with_drc_tws<
        const CHANNELS: usize,
        const BANDS: usize,
        const VISIBLE_BANDS: usize,
        const PRESET_BANDS: usize,
        const MIN_VOLUME: i16,
        const MAX_VOLUME: i16,
        const FRACTION_DIGITS: u8,
    >(
        &mut self,
        settings: modules::equalizer::EqualizerModuleSettings<
            VISIBLE_BANDS,
            PRESET_BANDS,
            MIN_VOLUME,
            MAX_VOLUME,
            FRACTION_DIGITS,
        >,
    ) where
        StateType: Has<EqualizerConfiguration<CHANNELS, BANDS, MIN_VOLUME, MAX_VOLUME, FRACTION_DIGITS>>
            + Has<TwsStatus>,
    {
        self.module_collection
            .add_equalizer_with_drc_tws(
                self.packet_io_controller.clone(),
                self.database.clone(),
                self.device_model,
                self.change_notify.clone(),
                settings,
            )
            .await;
    }

    pub async fn equalizer_with_basic_hear_id_tws<
        const CHANNELS: usize,
        const BANDS: usize,
        const VISIBLE_BANDS: usize,
        const PRESET_BANDS: usize,
        const MIN_VOLUME: i16,
        const MAX_VOLUME: i16,
        const FRACTION_DIGITS: u8,
    >(
        &mut self,
        settings: modules::equalizer::EqualizerModuleSettings<
            VISIBLE_BANDS,
            PRESET_BANDS,
            MIN_VOLUME,
            MAX_VOLUME,
            FRACTION_DIGITS,
        >,
    ) where
        StateType: Has<EqualizerConfiguration<CHANNELS, BANDS, MIN_VOLUME, MAX_VOLUME, FRACTION_DIGITS>>
            + Has<TwsStatus>
            + Has<BasicHearId<CHANNELS, BANDS>>
            + Has<Gender>
            + Has<AgeRange>,
    {
        self.module_collection
            .add_equalizer_with_basic_hear_id_tws(
                self.packet_io_controller.clone(),
                self.database.clone(),
                self.device_model,
                self.change_notify.clone(),
                settings,
            )
            .await;
    }

    pub async fn equalizer_with_custom_hear_id_tws<
        const CHANNELS: usize,
        const BANDS: usize,
        const VISIBLE_BANDS: usize,
        const PRESET_BANDS: usize,
        const MIN_VOLUME: i16,
        const MAX_VOLUME: i16,
        const FRACTION_DIGITS: u8,
    >(
        &mut self,
        settings: modules::equalizer::EqualizerModuleSettings<
            VISIBLE_BANDS,
            PRESET_BANDS,
            MIN_VOLUME,
            MAX_VOLUME,
            FRACTION_DIGITS,
        >,
    ) where
        StateType: Has<EqualizerConfiguration<CHANNELS, BANDS, MIN_VOLUME, MAX_VOLUME, FRACTION_DIGITS>>
            + Has<TwsStatus>
            + Has<CustomHearId<CHANNELS, BANDS>>
            + Has<Gender>
            + Has<AgeRange>,
    {
        self.module_collection
            .add_equalizer_with_custom_hear_id_tws(
                self.packet_io_controller.clone(),
                self.database.clone(),
                self.device_model,
                self.change_notify.clone(),
                settings,
            )
            .await;
    }

    pub async fn equalizer_with_custom_hear_id_tws_force_supports_hear_id<
        const CHANNELS: usize,
        const BANDS: usize,
        const VISIBLE_BANDS: usize,
        const PRESET_BANDS: usize,
        const MIN_VOLUME: i16,
        const MAX_VOLUME: i16,
        const FRACTION_DIGITS: u8,
    >(
        &mut self,
        settings: modules::equalizer::EqualizerModuleSettings<
            VISIBLE_BANDS,
            PRESET_BANDS,
            MIN_VOLUME,
            MAX_VOLUME,
            FRACTION_DIGITS,
        >,
    ) where
        StateType: Has<EqualizerConfiguration<CHANNELS, BANDS, MIN_VOLUME, MAX_VOLUME, FRACTION_DIGITS>>
            + Has<TwsStatus>
            + Has<CustomHearId<CHANNELS, BANDS>>
            + Has<Gender>
            + Has<AgeRange>,
    {
        self.module_collection
            .add_equalizer_with_custom_hear_id_tws_force_supports_hear_id(
                self.packet_io_controller.clone(),
                self.database.clone(),
                self.device_model,
                self.change_notify.clone(),
                settings,
            )
            .await;
    }

    pub fn button_configuration<const NUM_BUTTONS: usize, const NUM_PRESS_KINDS: usize>(
        &mut self,
        settings: &'static ButtonConfigurationSettings<NUM_BUTTONS, NUM_PRESS_KINDS>,
    ) where
        StateType: Has<ButtonStatusCollection<NUM_BUTTONS>>
            + Has<TwsStatus>
            + Has<ResetButtonConfigurationPending>,
    {
        self.module_collection
            .add_button_configuration(self.packet_io_controller.clone(), settings);
    }

    pub fn ambient_sound_mode_cycle(&mut self)
    where
        StateType: Has<AmbientSoundModeCycle> + Has<ResetButtonConfigurationPending>,
    {
        self.module_collection
            .add_ambient_sound_mode_cycle(self.packet_io_controller.clone());
    }

    pub fn ambient_sound_mode_cycle_tws(&mut self)
    where
        StateType:
            Has<TwsStatus> + Has<AmbientSoundModeCycleTws> + Has<ResetButtonConfigurationPending>,
    {
        self.module_collection
            .add_ambient_sound_mode_cycle_tws(self.packet_io_controller.clone());
    }

    pub fn reset_button_configuration<ButtonConfigurationPacketType>(
        &mut self,
        request_button_configuration_packet: packet::Outbound,
    ) where
        StateType: Has<ResetButtonConfigurationPending> + Update<ButtonConfigurationPacketType>,
        ButtonConfigurationPacketType: FromPacketBody,
    {
        self.module_collection
            .add_reset_button_configuration::<ButtonConfigurationPacketType>(
                self.packet_io_controller.clone(),
                request_button_configuration_packet,
            );
    }

    pub fn single_battery(&mut self, max_level: u8)
    where
        StateType: Has<SingleBattery>,
    {
        self.module_collection.add_single_battery(
            modules::single_battery::SingleBatteryConfiguration {
                max_level,
                ..Default::default()
            },
        );
    }

    pub fn single_battery_custom(
        &mut self,
        configuration: modules::single_battery::SingleBatteryConfiguration,
    ) where
        StateType: Has<SingleBattery>,
    {
        self.module_collection.add_single_battery(configuration);
    }

    pub fn single_battery_level(&mut self, max_level: u8)
    where
        StateType: Has<BatteryLevel>,
    {
        self.module_collection.add_single_battery_level(max_level);
    }

    pub fn dual_battery(&mut self, max_level: u8)
    where
        StateType: Has<DualBattery>,
    {
        self.module_collection
            .add_dual_battery(modules::dual_battery::DualBatteryConfiguration {
                max_level,
                ..Default::default()
            });
    }

    pub fn dual_battery_custom(
        &mut self,
        configuration: modules::dual_battery::DualBatteryConfiguration,
    ) where
        StateType: Has<DualBattery>,
    {
        self.module_collection.add_dual_battery(configuration);
    }

    pub fn dual_battery_level_custom(
        &mut self,
        configuration: modules::dual_battery_level::DualBatteryLevelConfiguration,
    ) where
        StateType: Has<DualBatteryLevel>,
    {
        self.module_collection.add_dual_battery_level(configuration);
    }

    pub fn case_battery_level(&mut self, max_level: u8)
    where
        StateType: Has<CaseBatteryLevel>,
    {
        self.module_collection.add_case_battery_level(
            modules::case_battery_level::CaseBatteryLevelConfiguration {
                max_level,
                ..Default::default()
            },
        );
    }

    pub fn case_battery_level_custom(
        &mut self,
        configuration: modules::case_battery_level::CaseBatteryLevelConfiguration,
    ) where
        StateType: Has<CaseBatteryLevel>,
    {
        self.module_collection.add_case_battery_level(configuration);
    }

    pub fn serial_number_and_firmware_version(&mut self)
    where
        StateType: Has<SerialNumber> + Has<FirmwareVersion>,
    {
        self.module_collection
            .add_serial_number_and_firmware_version();
    }

    pub fn serial_number_and_dual_firmware_version(&mut self)
    where
        StateType: Has<SerialNumber> + Has<DualFirmwareVersion>,
    {
        self.module_collection
            .add_serial_number_and_dual_firmware_version();
    }

    pub fn tws_status(&mut self)
    where
        StateType: Has<TwsStatus>,
    {
        self.module_collection.add_tws_status();
    }

    pub fn auto_power_off<Duration>(&mut self, durations: &'static [Duration])
    where
        StateType: MaybeHas<AutoPowerOff>,
        Duration: Translate + Send + Sync + 'static,
        &'static str: for<'a> From<&'a Duration>,
    {
        self.module_collection
            .add_auto_power_off(self.packet_io_controller.clone(), durations);
    }

    pub fn limit_high_volume(&mut self)
    where
        StateType: Has<LimitHighVolume>,
    {
        self.module_collection
            .add_limit_high_volume(self.packet_io_controller.clone());
    }

    pub fn dual_connections(&mut self)
    where
        StateType: Has<DualConnections>,
    {
        self.module_collection
            .add_dual_connections(self.packet_io_controller.clone());
    }

    flag!(TouchTone);
    flag!(GamingMode);
    flag!(SoundLeakCompensation);
    flag!(SurroundSound);
    flag!(AutoPlayPause);
    flag!(WearingTone);
    flag!(TouchLock);
    flag!(LowBatteryPrompt);
    flag!(WearingDetection);
    flag!(VoicePrompt);
    flag!(Ldac);
    flag!(DisableAllButtons);

    pub fn voice_prompt_targeted(&mut self)
    where
        StateType: MaybeHas<VoicePrompt> + Send + Sync,
    {
        self.module_collection.add_voice_prompt_targeted();
    }
}

pub struct SoundcoreDeviceTemplate<StateType>
where
    StateType: Clone + Send + Sync,
{
    device_model: DeviceModel,
    state_sender: watch::Sender<StateType>,
    module_collection: Arc<ModuleCollection<StateType>>,
    packet_io_controller: Arc<PacketIOController>,
    // TODO exit signal is necessary due to the PacketIOController Arc spaghetti.
    exit_signal: Arc<Semaphore>,
    change_notify: watch::Receiver<()>,
    is_demo: bool,
}

impl<StateType> SoundcoreDeviceTemplate<StateType>
where
    StateType: Clone + Send + Sync + 'static,
{
    async fn new(
        packet_io_controller: Arc<PacketIOController>,
        state_sender: watch::Sender<StateType>,
        module_collection: ModuleCollection<StateType>,
        packet_receiver: mpsc::Receiver<packet::Inbound>,
        device_model: DeviceModel,
        change_notify: watch::Receiver<()>,
        is_demo: bool,
    ) -> Self {
        let exit_signal = Arc::new(Semaphore::new(0));
        let module_collection = Arc::new(module_collection);
        module_collection
            .spawn_packet_handler(state_sender.clone(), packet_receiver, exit_signal.clone())
            .await;

        Self {
            device_model,
            state_sender,
            packet_io_controller,
            module_collection,
            exit_signal,
            change_notify,
            is_demo,
        }
    }
}

impl<StateType> Drop for SoundcoreDeviceTemplate<StateType>
where
    StateType: Clone + Send + Sync,
{
    fn drop(&mut self) {
        self.exit_signal.close();
    }
}

#[async_trait]
impl<StateType> OpenSCQ30Device for SoundcoreDeviceTemplate<StateType>
where
    StateType: Clone + Send + Sync + 'static,
{
    fn connection_status(&self) -> watch::Receiver<ConnectionStatus> {
        self.packet_io_controller.connection_status()
    }

    fn session_status(&self) -> watch::Receiver<SessionStatus> {
        self.packet_io_controller.session_status()
    }

    fn model(&self) -> DeviceModel {
        self.device_model
    }

    fn categories(&self) -> Vec<CategoryId> {
        self.module_collection.setting_manager.categories().to_vec()
    }

    fn settings_in_category(&self, category_id: &CategoryId) -> Vec<SettingId> {
        self.module_collection.setting_manager.category(category_id)
    }

    fn setting(&self, setting_id: &SettingId) -> Option<Setting> {
        let state = self.state_sender.borrow().to_owned();
        self.module_collection
            .setting_manager
            .get(&state, setting_id)
    }

    fn settings_by_category(&self) -> IndexMap<CategoryId, IndexMap<SettingId, Setting>> {
        let state = self.state_sender.borrow().to_owned();
        let manager = &self.module_collection.setting_manager;
        manager
            .categories()
            .iter()
            .map(|&category| {
                let settings = manager
                    .category(&category)
                    .into_iter()
                    .filter_map(|id| manager.get(&state, &id).map(|setting| (id, setting)))
                    .collect();
                (category, settings)
            })
            .collect()
    }

    fn watch_for_changes(&self) -> watch::Receiver<()> {
        let mut receiver = self.state_sender.subscribe();
        let (change_sender, change_receiver) = watch::channel(());
        let mut change_notify = self.change_notify.clone();
        // receiver will close when self is dropped, so this will clean itself up
        tokio::spawn(async move {
            loop {
                select! {
                    result = receiver.changed() => if result.is_err() { return },
                    result = change_notify.changed() => if result.is_err() { return },
                }
                if change_sender.send(()).is_err() {
                    return;
                }
            }
        });
        change_receiver
    }

    async fn set_setting_values(
        &self,
        setting_values: Vec<(SettingId, Value)>,
    ) -> device::Result<Vec<device::SettingWriteOutcome>> {
        tracing::debug!("set values: {setting_values:?}");
        if self.is_demo {
            self.module_collection
                .set_setting_values_demo(
                    &self.state_sender,
                    &self.packet_io_controller,
                    setting_values,
                )
                .await
        } else {
            self.module_collection
                .set_setting_values(
                    &self.state_sender,
                    &self.packet_io_controller,
                    setting_values,
                )
                .await
        }
    }
}

#[cfg(test)]
pub mod test_utils {
    use std::{collections::HashMap, time::Duration};

    use macaddr::MacAddr6;
    use nom_language::error::VerboseError;

    use crate::{
        devices::soundcore::common::packet::{self, Command},
        mock::rfcomm::MockRfcommBackend,
    };

    use super::*;

    pub struct TestSoundcoreDevice {
        device: Arc<dyn OpenSCQ30Device + Send + Sync>,
        inbound_sender: mpsc::Sender<Vec<u8>>,
        outbound_receiver: mpsc::Receiver<Vec<u8>>,
        config: SoundcoreDeviceConfig,
    }

    impl TestSoundcoreDevice {
        // HACK: we should instead have packet_responses's values be async functions that have
        // PacketIOController as a parameter
        pub async fn new_with_delayed_responses<StateType>(
            constructor: fn(
                Arc<dyn RfcommBackend + Send + Sync>,
                Arc<OpenSCQ30Database>,
                DeviceModel,
            ) -> SoundcoreDeviceRegistry<StateType>,
            device_model: DeviceModel,
            packet_responses: HashMap<Command, packet::Inbound>,
            config: SoundcoreDeviceConfig,
            delays: HashMap<packet::Command, Duration>,
        ) -> Self
        where
            StateType: Clone + Send + Sync + 'static,
            SoundcoreDeviceRegistry<StateType>: BuildDevice<StateType>,
        {
            let (inbound_sender, inbound_receiver) = mpsc::channel(100);
            let (outbound_sender, mut outbound_receiver) = mpsc::channel(100);
            let database = Arc::new(OpenSCQ30Database::new_in_memory().await.unwrap());

            let mut registry = constructor(
                Arc::new(MockRfcommBackend::new(inbound_receiver, outbound_sender)),
                database,
                device_model,
            );
            registry.is_demo = true;

            // spawn a future to connect to the device rather than to handle packets so that we don't have to move
            // outbound_receiver into the future and back out when it's done
            let (device_sender, mut device_receiver) = mpsc::channel(1);
            tokio::spawn(async move {
                device_sender
                    .try_send(registry.connect(MacAddr6::nil()).await.unwrap())
                    .unwrap();
            });

            // respond to packets until the device is ready
            let device = loop {
                select! {
                    maybe_packet = outbound_receiver.recv() => {
                        if let Some(packet) = maybe_packet {
                            let command = Command(packet[5..7].try_into().unwrap());
                            match packet_responses.get(&command) {
                                Some(response) => {
                                    if let Some(delay) = delays.get(&command) {
                                        tokio::time::sleep(*delay).await;
                                    }
                                    inbound_sender.send(response.bytes(config.checksum_kind)).await.unwrap();
                                },
                                // Dual connections answer with zero or more packets. An empty
                                // list is one packet with no devices, not silence.
                                None if command == Command([0x0b, 0x01]) => {
                                    inbound_sender
                                        .send(
                                            packet::Inbound::new(command, vec![1, 1])
                                                .bytes(config.checksum_kind),
                                        )
                                        .await
                                        .unwrap();
                                }
                                None => panic!("missing response for {command:?}")
                            }
                        }
                    }
                    device = device_receiver.recv() => {
                        break device.unwrap();
                    }
                }
            };

            Self {
                device,
                inbound_sender,
                outbound_receiver,
                config,
            }
        }

        pub async fn new<StateType>(
            constructor: fn(
                Arc<dyn RfcommBackend + Send + Sync>,
                Arc<OpenSCQ30Database>,
                DeviceModel,
            ) -> SoundcoreDeviceRegistry<StateType>,
            device_model: DeviceModel,
            packet_responses: HashMap<Command, packet::Inbound>,
            config: SoundcoreDeviceConfig,
        ) -> Self
        where
            StateType: Clone + Send + Sync + 'static,
            SoundcoreDeviceRegistry<StateType>: BuildDevice<StateType>,
        {
            Self::new_with_delayed_responses(
                constructor,
                device_model,
                packet_responses,
                config,
                HashMap::new(),
            )
            .await
        }

        pub fn inner(&self) -> &Arc<dyn OpenSCQ30Device + Send + Sync> {
            &self.device
        }

        /// Sends a packet to the device as if the earbuds sent it, and waits for it to be handled.
        pub async fn receive(&self, packet: packet::Inbound) {
            self.inbound_sender
                .send(packet.bytes(self.config.checksum_kind))
                .await
                .unwrap();
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        #[track_caller]
        pub fn assert_setting_values(
            &self,
            expected_values: impl IntoIterator<Item = (SettingId, Value)>,
        ) {
            for (setting_id, expected) in expected_values {
                let setting = self
                    .device
                    .setting(&setting_id)
                    .unwrap_or_else(|| panic!("{setting_id} returned None"));
                assert_eq!(
                    Value::from(setting),
                    expected,
                    "{setting_id} should be {expected:?}"
                );
            }
        }

        pub async fn set_settings(&mut self, settings: Vec<(SettingId, Value)>) {
            _ = self.set_settings_and_gather_sent_packets(settings).await;
        }

        pub async fn assert_set_settings_response_unordered(
            &mut self,
            settings: Vec<(SettingId, Value)>,
            expected_packets: Vec<packet::Outbound>,
        ) {
            let mut expected_packets_bytes = expected_packets
                .iter()
                .map(|expected| expected.bytes(self.config.checksum_kind))
                .collect::<Vec<_>>();
            let mut sent_packets_bytes = self.set_settings_and_gather_sent_packets(settings).await;
            expected_packets_bytes.sort();
            sent_packets_bytes.sort();

            assert_eq!(sent_packets_bytes, expected_packets_bytes);
        }

        pub async fn assert_set_settings_response(
            &mut self,
            settings: Vec<(SettingId, Value)>,
            expected_packets: Vec<packet::Outbound>,
        ) {
            let expected_packets_bytes = expected_packets
                .iter()
                .map(|expected| expected.bytes(self.config.checksum_kind))
                .collect::<Vec<_>>();
            let sent_packets_bytes = self.set_settings_and_gather_sent_packets(settings).await;
            assert_eq!(sent_packets_bytes, expected_packets_bytes);
        }

        #[must_use]
        async fn set_settings_and_gather_sent_packets(
            &mut self,
            settings: Vec<(SettingId, Value)>,
        ) -> Vec<Vec<u8>> {
            self.clear_sent_packets().await;
            let device = self.device.clone();
            let join_handle = tokio::spawn(async move {
                device.set_setting_values(settings).await.unwrap();
            });

            let mut sent_packets = Vec::new();
            select! {
                 () = self.gather_sent_packets(&mut sent_packets) => (),
                 _ = join_handle => (),
            };
            while let Ok(bytes) = self.outbound_receiver.try_recv() {
                let packet =
                    packet::Outbound::take::<VerboseError<_>>(self.config.checksum_kind)(&bytes)
                        .unwrap()
                        .1;
                self.ack(&packet).await;
                if !matches!(
                    packet.command,
                    packet::Command([0x01, 0x01])
                        | packet::Command([0x01, 0x7F])
                        | packet::Command([0x01, 0x10])
                ) {
                    sent_packets.push(bytes);
                }
            }
            sent_packets
        }

        async fn clear_sent_packets(&mut self) {
            // gather packets and discard return value
            // start_paused should be set to true to prevent this from taking forever
            _ = tokio::time::timeout(
                Duration::from_secs(30),
                self.gather_sent_packets(&mut Vec::new()),
            )
            .await;
        }

        async fn gather_sent_packets(&mut self, sent_packets: &mut Vec<Vec<u8>>) {
            while let Some(bytes) = self.outbound_receiver.recv().await {
                let packet =
                    packet::Outbound::take::<VerboseError<_>>(self.config.checksum_kind)(&bytes)
                        .unwrap()
                        .1;
                self.ack(&packet).await;
                if !matches!(
                    packet.command,
                    packet::Command([0x01, 0x01])
                        | packet::Command([0x01, 0x7F])
                        | packet::Command([0x01, 0x10])
                ) {
                    sent_packets.push(bytes);
                }
            }
        }

        async fn ack(&self, packet: &packet::Outbound) {
            // for cancel safety
            let permit = self.inbound_sender.reserve().await.unwrap();
            permit.send(packet.ack().bytes(self.config.checksum_kind));
        }
    }
}

#[cfg(test)]
mod snapshot_tests {
    use super::*;
    use crate::{
        devices::soundcore::common::settings_manager::{SettingHandler, SettingHandlerResult},
        mock::rfcomm::MockRfcommConnection,
    };
    use std::sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
        mpsc as sync_channel,
    };
    use std::time::Duration;

    struct State {
        value: bool,
        clones: Arc<AtomicUsize>,
    }

    impl Clone for State {
        fn clone(&self) -> Self {
            self.clones.fetch_add(1, Ordering::SeqCst);
            Self {
                value: self.value,
                clones: self.clones.clone(),
            }
        }
    }

    type Gate = Arc<Mutex<Option<(sync_channel::Sender<()>, sync_channel::Receiver<()>)>>>;

    struct Handler {
        ids: Vec<SettingId>,
        gate: Gate,
    }

    #[async_trait]
    impl SettingHandler<State> for Handler {
        fn settings(&self) -> Vec<SettingId> {
            self.ids.clone()
        }
        fn get(&self, state: &State, id: &SettingId) -> Option<Setting> {
            let gate = self.gate.lock().unwrap().take();
            if let Some((start, updated)) = gate {
                start.send(()).unwrap();
                updated.recv_timeout(Duration::from_secs(3)).unwrap();
            }
            (*id != SettingId::VoicePrompt).then_some(Setting::Toggle { value: state.value })
        }
        async fn set(
            &self,
            _state: &mut State,
            _id: &SettingId,
            _value: Value,
        ) -> SettingHandlerResult<()> {
            unreachable!()
        }
    }

    #[tokio::test]
    async fn bulk_read_captures_state_once_and_preserves_category_order() {
        let clones = Arc::new(AtomicUsize::new(0));
        let state_sender = watch::channel(State {
            value: false,
            clones: clones.clone(),
        })
        .0;
        let (start, ready) = sync_channel::channel();
        let (finished, updated) = sync_channel::channel();
        let gate = Arc::new(Mutex::new(Some((start, updated))));
        let mut modules = ModuleCollection::default();
        for (category, ids) in [
            (
                CategoryId::General,
                vec![
                    SettingId::WearingDetection,
                    SettingId::WearingDetection,
                    SettingId::LimitHighVolume,
                    SettingId::VoicePrompt,
                ],
            ),
            (CategoryId::Equalizer, vec![]),
            (
                CategoryId::DeviceInformation,
                vec![SettingId::WearingDetection],
            ),
        ] {
            modules.setting_manager.add_handler(
                category,
                Handler {
                    ids,
                    gate: gate.clone(),
                },
            );
        }
        let (_inbound, receiver) = mpsc::channel(1);
        let (outbound, _receiver) = mpsc::channel(1);
        let (packet_io, _) = PacketIOController::new(
            Arc::new(MockRfcommConnection::new(receiver, outbound)),
            packet::ChecksumKind::Suffix,
        )
        .await
        .unwrap();
        let device = SoundcoreDeviceTemplate {
            device_model: DeviceModel::SoundcoreA3954,
            state_sender: state_sender.clone(),
            module_collection: Arc::new(modules),
            packet_io_controller: Arc::new(packet_io),
            exit_signal: Arc::new(Semaphore::new(0)),
            change_notify: watch::channel(()).1,
            is_demo: false,
        };
        let writer_clones = clones.clone();
        let writer = std::thread::spawn(move || {
            ready.recv_timeout(Duration::from_secs(3)).unwrap();
            state_sender.send_replace(State {
                value: true,
                clones: writer_clones,
            });
            finished.send(()).unwrap();
        });
        let settings = device.settings_by_category();
        writer.join().unwrap();
        assert_eq!(clones.load(Ordering::SeqCst), 1);
        assert!(device.state_sender.borrow().value);
        assert_eq!(
            settings.keys().copied().collect::<Vec<_>>(),
            vec![
                CategoryId::General,
                CategoryId::Equalizer,
                CategoryId::DeviceInformation
            ]
        );
        assert_eq!(
            settings[&CategoryId::General]
                .keys()
                .copied()
                .collect::<Vec<_>>(),
            vec![SettingId::WearingDetection, SettingId::LimitHighVolume]
        );
        assert!(settings[&CategoryId::Equalizer].is_empty());
        assert_eq!(settings[&CategoryId::DeviceInformation].len(), 1);
        assert!(
            settings
                .values()
                .flat_map(|category| category.values())
                .all(|setting| *setting == Setting::Toggle { value: false })
        );
    }
}
