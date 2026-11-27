//! Selection of the measured model drivers supported by the pilot.

use crate::{ModelDriver, ModelError, RedactedSerial, a3004, a3005, a3028, a3033, a3062, a3949};

/// Models implemented by the new session.
pub const DRIVERS: [ModelDriver; 8] = [
    ModelDriver::A3004,
    ModelDriver::A3005,
    ModelDriver::A3028,
    ModelDriver::A3029,
    ModelDriver::A3033,
    ModelDriver::A3062,
    ModelDriver::A3949,
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
pub fn identify_state(body: &[u8]) -> Result<ModelDriver, ModelError> {
    if a3028::has_confirmed_layout(body) {
        return match from_serial(&a3028::state_serial(body)?)? {
            model @ (ModelDriver::A3028 | ModelDriver::A3029) => Ok(model),
            _ => Err(ModelError::UnknownModel),
        };
    }
    match body.len() {
        a3004::STATE_LENGTH => {
            return match from_serial(&a3004::state_serial(body)?)? {
                ModelDriver::A3004 => Ok(ModelDriver::A3004),
                _ => Err(ModelError::UnknownModel),
            };
        }
        a3005::STATE_LENGTH => {
            return match from_serial(&a3005::state_serial(body)?)? {
                ModelDriver::A3005 => Ok(ModelDriver::A3005),
                _ => Err(ModelError::UnknownModel),
            };
        }
        a3033::STATE_LENGTH => {
            return match from_serial(&a3033::state_serial(body)?)? {
                ModelDriver::A3033 => Ok(ModelDriver::A3033),
                _ => Err(ModelError::UnknownModel),
            };
        }
        _ => {}
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
