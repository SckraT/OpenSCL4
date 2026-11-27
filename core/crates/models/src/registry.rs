//! Selection of the measured model drivers supported by the pilot.

use crate::{ModelDriver, ModelError, RedactedSerial, a3028, a3062, a3947, a3949, a3955};

/// Models implemented by the new session.
pub const DRIVERS: [ModelDriver; 7] = [
    ModelDriver::A3028,
    ModelDriver::A3029,
    ModelDriver::A3062,
    ModelDriver::A3947,
    ModelDriver::A3949,
    ModelDriver::A3954,
    ModelDriver::A3955,
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
    // The Liberty 4 NC and P40i do not have a recorded identity response, so
    // their measured state length and serial select the driver.
    if body.len() == a3947::STATE_LENGTH {
        return match a3947::state_serial(body).and_then(|serial| from_serial(&serial)) {
            Ok(ModelDriver::A3947) => Ok(ModelDriver::A3947),
            Ok(_) | Err(_) => Err(ModelError::UnknownModel),
        };
    }
    if body.len() == a3955::STATE_LENGTH {
        return match a3955::state_serial(body).and_then(|serial| from_serial(&serial)) {
            Ok(ModelDriver::A3955) => Ok(ModelDriver::A3955),
            Ok(_) | Err(_) => Err(ModelError::UnknownModel),
        };
    }
    match a3062::state_serial(body).and_then(|serial| from_serial(&serial)) {
        Ok(ModelDriver::A3062) => Ok(ModelDriver::A3062),
        _ if a3949::state_serial(body)
            .is_ok_and(|serial| serial.identifies(ModelDriver::A3949)) =>
        {
            Ok(ModelDriver::A3949)
        }
        Ok(_) => Err(ModelError::UnknownModel),
        Err(error) => Err(error),
    }
}

fn from_serial(serial: &RedactedSerial<'_>) -> Result<ModelDriver, ModelError> {
    DRIVERS
        .into_iter()
        .find(|driver| serial.identifies(*driver))
        .ok_or(ModelError::UnknownModel)
}
