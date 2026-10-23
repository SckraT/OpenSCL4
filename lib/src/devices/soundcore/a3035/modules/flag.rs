use openscq30_lib_has::Has;

use crate::{
    devices::soundcore::{
        a3035,
        common::{
            modules::{
                ModuleCollection,
                flag::{FlagConfiguration, FlagVerification},
            },
            packet,
        },
    },
    settings::{CategoryId, SettingId},
};

impl<T> ModuleCollection<T>
where
    T: Has<a3035::structures::BatteryAlert> + Clone + Send + Sync,
{
    pub fn add_a3035_battery_alert(&mut self) {
        self.add_flag(FlagConfiguration {
            category_id: CategoryId::Miscellaneous,
            setting_id: SettingId::LowBatteryPrompt,
            set_command: packet::Command([1, 175]),
            update_command: None,
            is_inverted: false,
            verification: FlagVerification::Bulk,
        });
    }
}

impl<T> ModuleCollection<T>
where
    T: Has<a3035::structures::AmbientSoundModeVoicePrompt> + Clone + Send + Sync,
{
    pub fn add_a3035_ambient_sound_mode_voice_prompt(&mut self) {
        self.add_flag(FlagConfiguration {
            category_id: CategoryId::Miscellaneous,
            setting_id: SettingId::VoicePrompt,
            set_command: packet::Command([1, 174]),
            update_command: None,
            is_inverted: false,
            verification: FlagVerification::Bulk,
        });
    }
}
