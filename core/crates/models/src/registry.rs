//! Selection of the measured model drivers supported by the pilot.

use crate::{ModelDriver, ModelError, RedactedSerial, a3028, a3062, a3949};

/// Models implemented by the new session.
pub const DRIVERS: [ModelDriver; 9] = [
    ModelDriver::A3028,
    ModelDriver::A3029,
    ModelDriver::A3062,
    ModelDriver::A3909,
    ModelDriver::A3926,
    ModelDriver::A3930,
    ModelDriver::A3949,
    ModelDriver::A3951,
    ModelDriver::A3954,
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
///
/// The separate-identity family has no serial in its state body, so a state
/// response alone cannot select one of its models. A body that also matches a
/// model with a measured serial is identified from that serial first: the A3949
/// state is 88 bytes, the same length as an A3951 state with its new battery
/// fields. The remaining family bodies are rejected here, and the session keeps
/// the model from the `01 05` identity response.
pub fn identify_state(body: &[u8]) -> Result<ModelDriver, ModelError> {
    if a3028::has_confirmed_layout(body) {
        return match from_serial(&a3028::state_serial(body)?)? {
            model @ (ModelDriver::A3028 | ModelDriver::A3029) => Ok(model),
            _ => Err(ModelError::UnknownModel),
        };
    }
    let a3062 = a3062::state_serial(body).and_then(|serial| from_serial(&serial));
    if a3062 == Ok(ModelDriver::A3062) {
        return Ok(ModelDriver::A3062);
    }
    if a3949::state_serial(body).is_ok_and(|serial| serial.identifies(ModelDriver::A3949)) {
        return Ok(ModelDriver::A3949);
    }
    Err(ModelError::UnknownModel)
}

fn from_serial(serial: &RedactedSerial<'_>) -> Result<ModelDriver, ModelError> {
    DRIVERS
        .into_iter()
        .find(|driver| serial.identifies(*driver))
        .ok_or(ModelError::UnknownModel)
}
