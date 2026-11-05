pub mod demo;
pub mod device;
pub mod macros;
pub mod modules;
// The calculation is finished and tested, but nothing calls it yet: the device packet that carries its result is
// not implemented, and no model is known to accept it. Wiring it is a separate task.
#[allow(dead_code)]
pub mod nl2;
pub mod packet;
pub mod packet_manager;
pub mod settings_manager;
pub mod state;
pub mod state_modifier;
pub mod structures;
pub mod write;
