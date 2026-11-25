//! Read-only model drivers for the pilot.

#![forbid(unsafe_code)]

use std::fmt;

/// Read the two earbud firmware versions and the device identifier.
pub const IDENTITY_COMMAND: [u8; 2] = [0x01, 0x05];

/// Write the A3954 sound mode and its position on the mode scale.
pub const SOUND_MODE_COMMAND: [u8; 2] = [0x06, 0x81];

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
        let sound_mode = SoundMode::decode(body[A3954_SOUND_MODE])?;
        Ok(DeviceState {
            model: self,
            battery,
            sound_mode,
            sound_mode_position: body[A3954_SOUND_MODE + 1],
            airplane_mode: body[A3954_SOUND_MODE + 2],
            wind_noise_suppression: body[A3954_SOUND_MODE + 3] & 1 != 0,
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
    sound_mode_position: u8,
    airplane_mode: u8,
    wind_noise_suppression: bool,
    /// Left earbud firmware, if reported.
    pub firmware_left: Option<FirmwareVersion>,
    /// Right earbud firmware, if reported.
    pub firmware_right: Option<FirmwareVersion>,
}

impl DeviceState {
    /// Returns the reported sound mode and its position.
    pub const fn sound_mode_setting(self) -> SoundModeSetting {
        SoundModeSetting {
            mode: self.sound_mode,
            position: self.sound_mode_position,
        }
    }

    /// Builds a measured write while retaining airplane and wind controls.
    pub fn sound_mode_write(self, setting: SoundModeSetting) -> Result<SoundModeWrite, ModelError> {
        SoundModeWrite::new(setting, self.airplane_mode, self.wind_noise_suppression)
    }
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
    fn decode(byte: u8) -> Result<Self, ModelError> {
        match byte {
            0 => Ok(Self::NoiseCanceling),
            1 => Ok(Self::Transparency),
            2 => Ok(Self::Normal),
            3 => Ok(Self::AirplaneMode),
            _ => Err(ModelError::UnknownSoundMode),
        }
    }

    const fn wire(self) -> u8 {
        match self {
            Self::NoiseCanceling => 0,
            Self::Transparency => 1,
            Self::Normal => 2,
            Self::AirplaneMode => 3,
        }
    }

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

/// Sound mode and position of an A3954 snapshot or write.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SoundModeSetting {
    /// Ambient sound mode.
    pub mode: SoundMode,
    /// Position on the device's sound-mode scale.
    pub position: u8,
}

impl SoundModeSetting {
    /// The measured normal-mode position.
    pub const NORMAL: Self = Self {
        mode: SoundMode::Normal,
        position: 6,
    };
    /// Noise canceling at the highest measured position.
    pub const NOISE_CANCELING: Self = Self {
        mode: SoundMode::NoiseCanceling,
        position: 11,
    };

    fn is_writable(self) -> bool {
        matches!(
            (self.mode, self.position),
            (SoundMode::Normal, 6)
                | (SoundMode::NoiseCanceling, 7..=11)
                | (SoundMode::Transparency, 1..=5)
        )
    }
}

/// Measured five-byte A3954 sound-mode write.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SoundModeWrite {
    setting: SoundModeSetting,
    airplane_mode: u8,
    wind_noise_suppression: bool,
}

impl SoundModeWrite {
    fn new(
        setting: SoundModeSetting,
        airplane_mode: u8,
        wind_noise_suppression: bool,
    ) -> Result<Self, ModelError> {
        if airplane_mode > 1 || !setting.is_writable() {
            return Err(ModelError::InvalidSoundModeWrite);
        }
        Ok(Self {
            setting,
            airplane_mode,
            wind_noise_suppression,
        })
    }

    /// Parses the command body as received by the virtual A3954.
    pub fn decode(body: &[u8]) -> Result<Self, ModelError> {
        let [mode, position, airplane_mode, wind @ 0..=1, 0xff] = body else {
            return Err(ModelError::InvalidSoundModeWrite);
        };
        Self::new(
            SoundModeSetting {
                mode: SoundMode::decode(*mode)?,
                position: *position,
            },
            *airplane_mode,
            *wind != 0,
        )
    }

    /// Encodes the body format measured on the device.
    pub const fn bytes(self) -> [u8; 5] {
        [
            self.setting.mode.wire(),
            self.setting.position,
            self.airplane_mode,
            self.wind_noise_suppression as u8,
            0xff,
        ]
    }

    /// Applies the command to a virtual A3954's state payload.
    pub fn apply_to_state(self, body: &mut [u8]) -> Result<(), ModelError> {
        if body.len() < A3954_STATE_LENGTH {
            return Err(ModelError::IncompleteState);
        }
        body[A3954_SOUND_MODE] = self.setting.mode.wire();
        body[A3954_SOUND_MODE + 1] = self.setting.position;
        body[A3954_SOUND_MODE + 2] = self.airplane_mode;
        let wind = &mut body[A3954_SOUND_MODE + 3];
        *wind = (*wind & !1) | u8::from(self.wind_noise_suppression);
        Ok(())
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
    /// The selected mode or its controls do not match the measured write format.
    #[error("unsupported device sound mode write")]
    InvalidSoundModeWrite,
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
