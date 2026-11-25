//! Read-only model drivers for the pilot.

#![forbid(unsafe_code)]

use std::fmt;

/// Read the two earbud firmware versions and the device identifier.
pub const IDENTITY_COMMAND: [u8; 2] = [0x01, 0x05];

const A3954_STATE_LENGTH: usize = 165;
const A3954_CASE_BATTERY: usize = 37;
const A3954_SOUND_MODE: usize = 125;

/// A driver selected from a checked identity response.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelDriver {
    /// Soundcore Liberty 4 Pro.
    A3954,
}

impl ModelDriver {
    /// Selects a driver from a dual-firmware identity response.
    pub fn identify(body: &[u8]) -> Result<Self, ModelError> {
        let [_, _, _, _, _, _, _, _, _, _, serial @ ..] = body else {
            return Err(ModelError::InvalidIdentity);
        };
        let serial = RedactedSerial(serial.try_into().map_err(|_| ModelError::InvalidIdentity)?);
        if serial.0.starts_with(b"A3954") {
            Ok(Self::A3954)
        } else {
            Err(ModelError::UnknownModel)
        }
    }

    /// Returns the stable model code.
    pub const fn code(self) -> &'static str {
        match self {
            Self::A3954 => "A3954",
        }
    }

    /// Returns the request and reply command for the initial state.
    pub const fn state_command(self) -> [u8; 2] {
        match self {
            Self::A3954 => [0x01, 0x01],
        }
    }

    /// Decodes the state response for the selected model.
    pub fn decode_state(self, body: &[u8]) -> Result<DeviceState, ModelError> {
        match self {
            Self::A3954 => self.decode_a3954_state(body),
        }
    }

    fn decode_a3954_state(self, body: &[u8]) -> Result<DeviceState, ModelError> {
        if body.len() < A3954_STATE_LENGTH {
            return Err(ModelError::IncompleteState);
        }
        let battery = BatteryLevels {
            left: battery_percent(body[2], 100, 0)?,
            right: battery_percent(body[3], 100, 0)?,
            case: battery_percent(body[A3954_CASE_BATTERY], 10, 1)?,
        };
        let sound_mode = match body[A3954_SOUND_MODE] {
            0 => SoundMode::NoiseCanceling,
            1 => SoundMode::Transparency,
            2 => SoundMode::Normal,
            3 => SoundMode::AirplaneMode,
            _ => return Err(ModelError::UnknownSoundMode),
        };
        Ok(DeviceState {
            model: self,
            battery,
            sound_mode,
            firmware_left: FirmwareVersion::parse(&body[6..11])?,
            firmware_right: FirmwareVersion::parse(&body[11..16])?,
        })
    }
}

/// State reported by the device through its state response.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeviceState {
    /// Driver that decoded the response.
    pub model: ModelDriver,
    /// Reported battery percentages.
    pub battery: BatteryLevels,
    /// Current sound mode.
    pub sound_mode: SoundMode,
    /// Left earbud firmware, if reported.
    pub firmware_left: Option<FirmwareVersion>,
    /// Right earbud firmware, if reported.
    pub firmware_right: Option<FirmwareVersion>,
}

/// Battery percentages, with absent components represented by `None`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BatteryLevels {
    /// Left earbud percentage.
    pub left: Option<u8>,
    /// Right earbud percentage.
    pub right: Option<u8>,
    /// Charging case percentage.
    pub case: Option<u8>,
}

/// Sound mode values reported by the device.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SoundMode {
    /// Active noise canceling.
    NoiseCanceling,
    /// Transparency.
    Transparency,
    /// Normal sound.
    Normal,
    /// Airplane mode.
    AirplaneMode,
}

impl SoundMode {
    /// Returns the stable CLI identifier.
    pub const fn code(self) -> &'static str {
        match self {
            Self::NoiseCanceling => "NoiseCanceling",
            Self::Transparency => "Transparency",
            Self::Normal => "Normal",
            Self::AirplaneMode => "AirplaneMode",
        }
    }
}

/// A validated firmware version in the device's two-part decimal format.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FirmwareVersion {
    major: u8,
    minor: u8,
}

impl FirmwareVersion {
    fn parse(bytes: &[u8]) -> Result<Option<Self>, ModelError> {
        if matches!(bytes, [0, 0, 0, 0, 0] | [255, 255, 255, 255, 255]) {
            return Ok(None);
        }
        match bytes {
            [
                a @ b'0'..=b'9',
                b @ b'0'..=b'9',
                b'.',
                c @ b'0'..=b'9',
                d @ b'0'..=b'9',
            ] => Ok(Some(Self {
                major: (a - b'0') * 10 + (b - b'0'),
                minor: (c - b'0') * 10 + (d - b'0'),
            })),
            _ => Err(ModelError::InvalidFirmware),
        }
    }
}

impl fmt::Display for FirmwareVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:02}.{:02}", self.major, self.minor)
    }
}

/// A model probe or state response that cannot be used by the pilot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ModelError {
    /// The identity payload does not have the supported wire layout.
    #[error("invalid device identity response")]
    InvalidIdentity,
    /// The serial prefix is outside the supported registry.
    #[error("unknown device model")]
    UnknownModel,
    /// The state payload does not contain the model's measured fields.
    #[error("incomplete device state response")]
    IncompleteState,
    /// A battery level is outside its model scale.
    #[error("invalid device battery level")]
    InvalidBattery,
    /// The device returned an unrecognized mode.
    #[error("unknown device sound mode")]
    UnknownSoundMode,
    /// A firmware string does not follow the reported version format.
    #[error("invalid device firmware version")]
    InvalidFirmware,
}

struct RedactedSerial<'a>(&'a [u8; 16]);

impl fmt::Display for RedactedSerial<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<redacted-serial>")
    }
}

impl fmt::Debug for RedactedSerial<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

fn battery_percent(raw: u8, scale: u8, offset: u8) -> Result<Option<u8>, ModelError> {
    if raw == u8::MAX {
        return Ok(None);
    }
    let step = raw.checked_add(offset).ok_or(ModelError::InvalidBattery)?;
    if step > scale {
        return Err(ModelError::InvalidBattery);
    }
    Ok(Some((u16::from(step) * 100 / u16::from(scale)) as u8))
}
