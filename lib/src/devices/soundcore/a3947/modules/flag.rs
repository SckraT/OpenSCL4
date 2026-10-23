use openscq30_lib_has::Has;

use crate::{
    devices::soundcore::common::{
        modules::{
            ModuleCollection,
            flag::{FlagConfiguration, FlagVerification},
        },
        packet,
        structures::GamingMode,
    },
    settings::{CategoryId, SettingId},
};

impl<T> ModuleCollection<T>
where
    T: Has<GamingMode> + Clone + Send + Sync,
{
    pub fn add_a3947_gaming_mode(&mut self) {
        self.add_flag(FlagConfiguration {
            category_id: CategoryId::Miscellaneous,
            setting_id: SettingId::GamingMode,
            set_command: packet::Command([0x10, 0x85]),
            update_command: None,
            is_inverted: false,
            verification: FlagVerification::Bulk,
        });
    }
}
