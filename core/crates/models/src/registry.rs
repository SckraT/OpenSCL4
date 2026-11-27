//! Selection of the measured model drivers supported by the pilot.

use crate::{
    ModelDriver, ModelError, RedactedSerial, a3028, a3062, a3944, a3945, a3948, a3949, a3959,
};

/// Models implemented by the new session.
pub const DRIVERS: [ModelDriver; 9] = [
    ModelDriver::A3028,
    ModelDriver::A3029,
    ModelDriver::A3062,
    ModelDriver::A3944,
    ModelDriver::A3945,
    ModelDriver::A3948,
    ModelDriver::A3949,
    ModelDriver::A3954,
    ModelDriver::A3959,
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
    if a3062::state_serial(body).and_then(|serial| from_serial(&serial)) == Ok(ModelDriver::A3062) {
        return Ok(ModelDriver::A3062);
    }
    // The dual-state models carry the serial and the model prefix at the same
    // offsets, so the prefix selects the driver and the layout has to match.
    if let Some(serial) = body.get(16..32)
        && let Ok(serial) = RedactedSerial::parse(serial)
        && let Ok(model) = from_serial(&serial)
        && has_confirmed_layout(model, body)
    {
        return Ok(model);
    }
    Err(ModelError::UnknownModel)
}

/// Whether the body carries the layout documented for a dual-state model.
fn has_confirmed_layout(model: ModelDriver, body: &[u8]) -> bool {
    match model {
        ModelDriver::A3944 => a3944::has_confirmed_layout(body),
        ModelDriver::A3945 => a3945::has_confirmed_layout(body),
        ModelDriver::A3948 => a3948::has_confirmed_layout(body),
        ModelDriver::A3949 => body.len() >= a3949::STATE_LENGTH,
        ModelDriver::A3959 => a3959::has_confirmed_layout(body),
        ModelDriver::A3028 | ModelDriver::A3029 | ModelDriver::A3062 | ModelDriver::A3954 => false,
    }
}

fn from_serial(serial: &RedactedSerial<'_>) -> Result<ModelDriver, ModelError> {
    DRIVERS
        .into_iter()
        .find(|driver| serial.identifies(*driver))
        .ok_or(ModelError::UnknownModel)
}
