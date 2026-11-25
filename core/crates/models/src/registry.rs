//! Selection of the measured model drivers supported by the pilot.

use crate::{ModelDriver, ModelError, RedactedSerial, a3062};

/// Models implemented by the new session.
pub const DRIVERS: [ModelDriver; 2] = [ModelDriver::A3062, ModelDriver::A3954];

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

/// Identifies the measured single-battery state layout for a read-only probe.
pub fn identify_state(body: &[u8]) -> Result<ModelDriver, ModelError> {
    let model = from_serial(&a3062::state_serial(body)?)?;
    if model == ModelDriver::A3062 {
        Ok(model)
    } else {
        Err(ModelError::UnknownModel)
    }
}

fn from_serial(serial: &RedactedSerial<'_>) -> Result<ModelDriver, ModelError> {
    DRIVERS
        .into_iter()
        .find(|driver| serial.identifies(*driver))
        .ok_or(ModelError::UnknownModel)
}
