use crate::{
    devices::soundcore::{
        common::{
            modules::{
                ModuleCollection,
                flag::{FlagConfiguration, FlagVerification},
            },
            packet,
        },
        d1301::{self, state::D1301State},
    },
    settings::{CategoryId, SettingId},
};

impl ModuleCollection<D1301State> {
    pub fn add_d1301_auto_power_off_prompt(&mut self) {
        self.add_flag::<d1301::structures::AutoPowerOffPrompt>(FlagConfiguration {
            category_id: CategoryId::Miscellaneous,
            setting_id: SettingId::AutoPowerOffPrompt,
            set_command: packet::Command([16, 158]),
            update_command: None,
            is_inverted: false,
            verification: FlagVerification::Bulk,
        });
    }

    pub fn add_d1301_listening_mode_prompt(&mut self) {
        self.add_flag::<d1301::structures::ListeningModePrompt>(FlagConfiguration {
            category_id: CategoryId::Miscellaneous,
            setting_id: SettingId::ListeningModePrompt,
            set_command: packet::Command([16, 159]),
            update_command: None,
            is_inverted: false,
            verification: FlagVerification::Bulk,
        });
    }

    pub fn add_d1301_noise_canceling(&mut self) {
        self.add_flag::<d1301::structures::NoiseCanceling>(FlagConfiguration {
            category_id: CategoryId::Miscellaneous,
            setting_id: SettingId::NoiseCanceling,
            set_command: packet::Command([6, 135]),
            update_command: None,
            is_inverted: false,
            verification: FlagVerification::Bulk,
        });
    }

    pub fn add_d1301_incoming_calls_during_bluetooth_mode(&mut self) {
        self.add_flag::<d1301::structures::IncomingCallsDuringBluetoothMode>(FlagConfiguration {
            category_id: CategoryId::Miscellaneous,
            setting_id: SettingId::IncomingCallsDuringBluetoothMode,
            set_command: packet::Command([16, 117]),
            update_command: None,
            is_inverted: false,
            verification: FlagVerification::Bulk,
        });
    }

    pub fn add_d1301_tap_controls_disabled(&mut self) {
        self.add_flag::<d1301::structures::TapControlsDisabled>(FlagConfiguration {
            category_id: CategoryId::ButtonConfiguration,
            setting_id: SettingId::ButtonsEnabled,
            set_command: packet::Command([16, 148]),
            update_command: None,
            is_inverted: true,
            verification: FlagVerification::Bulk,
        });
    }

    pub fn add_d1301_noise_canceling_prompt(&mut self) {
        self.add_flag::<d1301::structures::NoiseCancelingPrompt>(FlagConfiguration {
            category_id: CategoryId::Miscellaneous,
            setting_id: SettingId::NoiseCancelingPrompt,
            set_command: packet::Command([16, 119]),
            update_command: None,
            is_inverted: false,
            verification: FlagVerification::Bulk,
        });
    }
}
