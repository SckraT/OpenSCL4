//! Selection of the measured model drivers supported by the pilot.

use crate::{ModelDriver, ModelError, RedactedSerial, a3028, a3062, dual};

/// Models implemented by the new session.
pub const DRIVERS: [ModelDriver; 8] = [
    ModelDriver::A3028,
    ModelDriver::A3029,
    ModelDriver::A3062,
    ModelDriver::A3936,
    ModelDriver::A3949,
    ModelDriver::A3952,
    ModelDriver::A3954,
    ModelDriver::A3968,
];

/// Models whose state carries the serial at body offset 16, in registry order.
const DUAL_BATTERY_DRIVERS: [ModelDriver; 4] = [
    ModelDriver::A3936,
    ModelDriver::A3952,
    ModelDriver::A3949,
    ModelDriver::A3968,
];

/// Finds a registered driver by its stable model code.
pub fn by_code(code: &str) -> Option<ModelDriver> {
    DRIVERS.into_iter().find(|driver| driver.code() == code)
}

pub(crate) fn identify_identity(body: &[u8]) -> Result<ModelDriver, ModelError> {
    if !matches!(body.len(), 26 | 31) {
        return Err(ModelError::InvalidIdentity);
    }
    let model = from_serial(&RedactedSerial::parse(&body[10..26])?)?;
    if body.len() == 31 && model != ModelDriver::A3954 {
        return Err(ModelError::InvalidIdentity);
    }
    Ok(model)
}

/// Identifies a model from the serial at the offset in its measured state layout.
pub fn identify_state(body: &[u8]) -> Result<ModelDriver, ModelError> {
    if a3028::has_confirmed_layout(body) {
        return match from_serial(&a3028::state_serial(body)?)? {
            model @ (ModelDriver::A3028 | ModelDriver::A3029) => Ok(model),
            _ => Err(ModelError::UnknownModel),
        };
    }
    // The A3062 state keeps its serial at offset 7; the dual-battery models keep
    // it at offset 16 with their own model prefix.
    if a3062::state_serial(body).and_then(|serial| from_serial(&serial)) == Ok(ModelDriver::A3062) {
        return Ok(ModelDriver::A3062);
    }
    let serial = dual::state_serial(body)?;
    DUAL_BATTERY_DRIVERS
        .into_iter()
        .find(|driver| serial.identifies(*driver))
        .ok_or(ModelError::UnknownModel)
}

fn from_serial(serial: &RedactedSerial<'_>) -> Result<ModelDriver, ModelError> {
    DRIVERS
        .into_iter()
        .find(|driver| serial.identifies(*driver))
        .ok_or(ModelError::UnknownModel)
}
