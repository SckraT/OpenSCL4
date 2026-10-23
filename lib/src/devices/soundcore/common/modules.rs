use std::sync::Arc;

use tokio::{
    select,
    sync::{Semaphore, mpsc, watch},
    task::JoinHandle,
};
use tracing::{Instrument, debug_span, trace};

use crate::{
    api::{
        device,
        settings::{SettingId, Value},
    },
    devices::soundcore::common::packet,
};

use super::{
    packet_manager::PacketManager, settings_manager::SettingsManager,
    state_modifier::StateModifier, write, write::FlagAccess,
};

pub mod ambient_sound_mode_cycle;
mod ambient_sound_mode_cycle_tws;
pub mod auto_power_off;
pub mod button_configuration;
pub mod case_battery_level;
pub mod dual_battery;
pub mod dual_battery_level;
pub mod dual_connections;
pub mod equalizer;
pub mod flag;
pub mod limit_high_volume;
pub mod reset_button_configuration;
pub mod serial_number_and_dual_firmware_version;
pub mod serial_number_and_firmware_version;
pub mod single_battery;
pub mod single_battery_level;
pub mod sound_modes;
pub mod sound_modes_v2;
pub mod tws_status;

pub struct ModuleCollection<StateType> {
    pub setting_manager: SettingsManager<StateType>,
    pub packet_handlers: PacketManager<StateType>,
    pub state_modifiers: Vec<Box<dyn StateModifier<StateType> + Send + Sync>>,
    pub flags: std::collections::HashMap<SettingId, Box<FlagAccess<StateType>>>,
    pub write_lock: tokio::sync::Mutex<write::WriteThrottle>,
    /// The command of the latest packet that changed the device state. Write confirmation uses it
    /// to distinguish a bulk state report from unrelated updates such as the battery level.
    pub last_state_update: watch::Sender<Option<packet::Command>>,
}

impl<T> Default for ModuleCollection<T> {
    fn default() -> Self {
        Self {
            setting_manager: Default::default(),
            packet_handlers: Default::default(),
            state_modifiers: Default::default(),
            flags: Default::default(),
            write_lock: Default::default(),
            last_state_update: watch::channel(None).0,
        }
    }
}

impl<StateType> ModuleCollection<StateType>
where
    StateType: Clone,
{
    pub async fn set_setting_values(
        &self,
        state_sender: &watch::Sender<StateType>,
        packet_io: &packet::PacketIOController,
        setting_values: impl IntoIterator<Item = (SettingId, Value)>,
    ) -> device::Result<Vec<device::SettingWriteOutcome>> {
        write::set_setting_values(self, state_sender, packet_io, setting_values, false).await
    }

    pub async fn set_setting_values_demo(
        &self,
        state_sender: &watch::Sender<StateType>,
        packet_io: &packet::PacketIOController,
        setting_values: impl IntoIterator<Item = (SettingId, Value)>,
    ) -> device::Result<Vec<device::SettingWriteOutcome>> {
        write::set_setting_values(self, state_sender, packet_io, setting_values, true).await
    }
}

pub trait ModuleCollectionSpawnPacketHandlerExt<T> {
    async fn spawn_packet_handler(
        &self,
        state_sender: watch::Sender<T>,
        packet_receiver: mpsc::Receiver<packet::Inbound>,
        exit_signal: Arc<Semaphore>,
    ) -> JoinHandle<()>
    where
        T: 'static + Send + Sync;
}

impl<T> ModuleCollectionSpawnPacketHandlerExt<T> for Arc<ModuleCollection<T>> {
    async fn spawn_packet_handler(
        &self,
        state_sender: watch::Sender<T>,
        mut packet_receiver: mpsc::Receiver<packet::Inbound>,
        exit_signal: Arc<Semaphore>,
    ) -> JoinHandle<()>
    where
        T: 'static + Send + Sync,
    {
        let module_collection = self.clone();
        let state_sender = state_sender.clone();
        while let Ok(packet) = packet_receiver.try_recv() {
            match module_collection
                .packet_handlers
                .handle(&state_sender, &packet)
                .await
            {
                Ok(()) => {
                    module_collection
                        .last_state_update
                        .send_replace(Some(packet.command));
                }
                Err(err) => {
                    tracing::warn!("error handling packet: {packet:?}, error: {err:?}");
                }
            }
        }
        tokio::spawn(
            async move {
                trace!("started receiving");
                loop {
                    select! {
                        maybe_packet = packet_receiver.recv() => {
                            if let Some(packet) = maybe_packet {
                                match module_collection
                                    .packet_handlers
                                    .handle(&state_sender, &packet)
                                    .await
                                {
                                    Ok(()) => {
                                        module_collection
                                            .last_state_update
                                            .send_replace(Some(packet.command));
                                    }
                                    Err(err) => {
                                        tracing::warn!("error handling packet: {packet:?}, error: {err:?}");
                                    }
                                }
                            }
                        }
                        _ = exit_signal.acquire() => break,
                    }
                }
                trace!("done receiving");
            }
            .instrument(debug_span!(
                "ModuleCollectionSpawnPacketHandlerExt::spawn_packet_handler"
            )),
        )
    }
}
