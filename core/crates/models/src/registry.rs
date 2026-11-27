//! Selection of the measured model drivers supported by the pilot.

use crate::{ModelDriver, ModelError, RedactedSerial, a3028, a3062, a3949, d1202};

/// Models implemented by the new session.
pub const DRIVERS: [ModelDriver; 7] = [
    ModelDriver::A3028,
    ModelDriver::A3029,
    ModelDriver::A3062,
    ModelDriver::A3949,
    ModelDriver::A3954,
    ModelDriver::D1202,
    ModelDriver::D1202C,
];

/// Finds a registered driver by its stable model code.
///
/// The two public codes of the D1202 family are compared as whole strings, so
/// `D1202` never resolves to `D1202C` or the reverse.
pub fn by_code(code: &str) -> Option<ModelDriver> {
    DRIVERS.into_iter().find(|driver| driver.code() == code)
}

/// Returns the recorded connection metadata that selects a public code.
pub fn service_id(model: ModelDriver) -> Option<[u8; 16]> {
    model.service_id()
}

/// Selects a public code from recorded connection metadata.
pub fn identify_service(service: &[u8; 16]) -> Option<ModelDriver> {
    d1202::identify_service(service)
}

pub(crate) fn identify_identity(body: &[u8]) -> Result<ModelDriver, ModelError> {
    identify_identity_hinted(body, None)
}

/// Selects a driver from a validated identity response and connection metadata.
pub fn identify_identity_hinted(
    body: &[u8],
    hint: Option<ModelDriver>,
) -> Result<ModelDriver, ModelError> {
    if !matches!(body.len(), 26 | 31) {
        return Err(ModelError::InvalidIdentity);
    }
    let model = from_serial(&RedactedSerial::parse(&body[10..26])?, hint)?;
    if body.len() == 31 && model != ModelDriver::A3954 {
        return Err(ModelError::InvalidIdentity);
    }
    Ok(model)
}

/// Identifies a model from the serial at the offset in its measured state layout.
pub fn identify_state(body: &[u8]) -> Result<ModelDriver, ModelError> {
    identify_state_hinted(body, None)
}

/// Identifies a model from a measured state body and connection metadata.
///
/// The D1202 family reports one state serial for both public codes, so the
/// recorded connection metadata selects the code; without it the family
/// resolves to its canonical `D1202` code.
pub fn identify_state_hinted(
    body: &[u8],
    hint: Option<ModelDriver>,
) -> Result<ModelDriver, ModelError> {
    if a3028::has_confirmed_layout(body) {
        return match from_serial(&a3028::state_serial(body)?, hint)? {
            model @ (ModelDriver::A3028 | ModelDriver::A3029) => Ok(model),
            _ => Err(ModelError::UnknownModel),
        };
    }
    if d1202::has_confirmed_layout(body)
        && d1202::state_serial(body).is_ok_and(|serial| d1202::serial_is_family(&serial))
    {
        return Ok(family_code(hint));
    }
    match a3062::state_serial(body).and_then(|serial| from_serial(&serial, hint)) {
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

fn from_serial(
    serial: &RedactedSerial<'_>,
    hint: Option<ModelDriver>,
) -> Result<ModelDriver, ModelError> {
    if d1202::serial_is_family(serial) {
        return Ok(family_code(hint));
    }
    DRIVERS
        .into_iter()
        .find(|driver| serial.identifies(*driver))
        .ok_or(ModelError::UnknownModel)
}

/// The public code of the D1202 family selected by the connection metadata.
fn family_code(hint: Option<ModelDriver>) -> ModelDriver {
    match hint {
        Some(ModelDriver::D1202C) => ModelDriver::D1202C,
        _ => ModelDriver::D1202,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlapping_public_codes_resolve_whole_strings() {
        assert_eq!(by_code("D1202"), Some(ModelDriver::D1202));
        assert_eq!(by_code("D1202C"), Some(ModelDriver::D1202C));
        assert_eq!(by_code("D120"), None);
    }

    #[test]
    fn overlapping_serials_select_the_family_without_metadata() {
        let serial = RedactedSerial::parse(b"1202TEST00000000").expect("family serial");
        assert_eq!(from_serial(&serial, None), Ok(ModelDriver::D1202));
        assert_eq!(
            from_serial(&serial, Some(ModelDriver::D1202C)),
            Ok(ModelDriver::D1202C)
        );
        // A hint for another model cannot move a family serial out of the family.
        assert_eq!(
            from_serial(&serial, Some(ModelDriver::A3949)),
            Ok(ModelDriver::D1202)
        );
    }

    #[test]
    fn recorded_services_select_the_public_codes() {
        assert_eq!(
            identify_service(&d1202::D1202_SERVICE_ID),
            Some(ModelDriver::D1202)
        );
        assert_eq!(
            identify_service(&d1202::D1202C_SERVICE_ID),
            Some(ModelDriver::D1202C)
        );
        assert_eq!(identify_service(&[0; 16]), None);
        assert_eq!(
            service_id(ModelDriver::D1202C),
            Some(d1202::D1202C_SERVICE_ID)
        );
        assert_eq!(service_id(ModelDriver::A3954), None);
    }
}
