//! Model drivers for the device pilot.

#![forbid(unsafe_code)]

use std::fmt;

mod a3062;
mod a3949;
pub mod registry;

/// Probe for the firmware versions and the device identifier.
pub const IDENTITY_COMMAND: [u8; 2] = [0x01, 0x05];

/// Read the state of a device using the common state command.
pub const STATE_COMMAND: [u8; 2] = [0x01, 0x01];

/// Write the selected driver's sound-mode controls.
pub const SOUND_MODE_COMMAND: [u8; 2] = [0x06, 0x81];

const A3954_STATE_LENGTH: usize = 165;
const A3954_CASE_BATTERY: usize = 37;
const A3954_SOUND_MODE: usize = 125;

/// A driver selected from a checked identity response.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelDriver {
    /// Soundcore Space One Pro.
    A3062,
    /// Soundcore P20i, P25i and R50i.
    A3949,
    /// Soundcore Liberty 4 Pro.
    A3954,
}

impl ModelDriver {
    /// Selects a driver from a validated identity response.
    pub fn identify(body: &[u8]) -> Result<Self, ModelError> {
        registry::identify_identity(body)
    }

    /// Operations implemented in the new session for this model.
    pub const fn capabilities(self) -> Capabilities {
        Capabilities {
            positioned_sound_mode_write: matches!(self, Self::A3954),
            ambient_sound_mode_write: matches!(self, Self::A3062),
        }
    }

    /// Returns the stable model code.
    pub const fn code(self) -> &'static str {
        match self {
            Self::A3062 => "A3062",
            Self::A3949 => "A3949",
            Self::A3954 => "A3954",
        }
    }

    /// Returns the request and reply command for the initial state.
    pub const fn state_command(self) -> [u8; 2] {
        STATE_COMMAND
    }

    /// Decodes the state response for the selected model.
    pub fn decode_state(self, body: &[u8]) -> Result<DeviceState, ModelError> {
        match self {
            Self::A3062 => a3062::decode_state(body),
            Self::A3949 => a3949::decode_state(body),
            Self::A3954 => self.decode_a3954_state(body),
        }
    }

    fn decode_a3954_state(self, body: &[u8]) -> Result<DeviceState, ModelError> {
        if body.len() < A3954_STATE_LENGTH {
            return Err(ModelError::IncompleteState);
        }
        let battery = BatteryLevels {
            device: None,
            left: battery_percent(body[2], 100, 0)?,
            right: battery_percent(body[3], 100, 0)?,
            case: battery_percent(body[A3954_CASE_BATTERY], 10, 1)?,
        };
        let sound_mode = SoundMode::decode(body[A3954_SOUND_MODE])?;
        Ok(DeviceState {
            model: self,
            battery,
            sound_mode: Some(sound_mode),
            sound_mode_controls: Some(SoundModeControls::A3954(A3954SoundModeControls {
                position: body[A3954_SOUND_MODE + 1],
                airplane_mode: body[A3954_SOUND_MODE + 2],
                wind_noise_suppression: body[A3954_SOUND_MODE + 3] & 1 != 0,
            })),
            firmware_device: None,
            firmware_left: FirmwareVersion::parse(&body[6..11])?,
            firmware_right: FirmwareVersion::parse(&body[11..16])?,
        })
    }
}

/// Implemented operations exposed by a model driver.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Capabilities {
    /// The session can confirm writes of the sound mode and its scale position.
    pub positioned_sound_mode_write: bool,
    /// The session can confirm writes of the ambient mode without a scale position.
    pub ambient_sound_mode_write: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SoundModeControls {
    A3062([u8; 5]),
    A3954(A3954SoundModeControls),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct A3954SoundModeControls {
    position: u8,
    airplane_mode: u8,
    wind_noise_suppression: bool,
}

/// State reported by the device through its state response.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeviceState {
    /// Driver that decoded the response.
    pub model: ModelDriver,
    /// Reported battery percentages.
    pub battery: BatteryLevels,
    /// Current sound mode, if the device reports one for the pilot.
    pub sound_mode: Option<SoundMode>,
    sound_mode_controls: Option<SoundModeControls>,
    /// The firmware of a device with a single battery, if reported.
    pub firmware_device: Option<FirmwareVersion>,
    /// Left earbud firmware, if reported.
    pub firmware_left: Option<FirmwareVersion>,
    /// Right earbud firmware, if reported.
    pub firmware_right: Option<FirmwareVersion>,
}

impl DeviceState {
    /// Returns the reported setting for a model with a measured position scale.
    pub fn sound_mode_setting(self) -> Option<SoundModeSetting> {
        let Some(SoundModeControls::A3954(controls)) = self.sound_mode_controls else {
            return None;
        };
        Some(SoundModeSetting {
            mode: self.sound_mode?,
            position: controls.position,
        })
    }

    /// Builds a measured write while retaining airplane and wind controls.
    pub fn sound_mode_write(self, setting: SoundModeSetting) -> Result<SoundModeWrite, ModelError> {
        let Some(SoundModeControls::A3954(controls)) = self.sound_mode_controls else {
            return Err(ModelError::InvalidSoundModeWrite);
        };
        SoundModeWrite::new(
            setting,
            controls.airplane_mode,
            controls.wind_noise_suppression,
        )
    }

    /// Builds an ambient-mode write from the latest measured A3062 controls.
    pub fn ambient_sound_mode_write(self, mode: SoundMode) -> Result<[u8; 6], ModelError> {
        let Some(SoundModeControls::A3062(controls)) = self.sound_mode_controls else {
            return Err(ModelError::InvalidSoundModeWrite);
        };
        a3062::ambient_sound_mode_write(mode, controls)
    }
}

/// Battery percentages, with absent components represented by `None`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BatteryLevels {
    /// Battery percentage for a device with a single battery.
    pub device: Option<u8>,
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

/// Four-byte A3954 sound-mode write.
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
        let [mode, position, airplane_mode, wind @ 0..=1] = body else {
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

    /// Encodes the four controls accepted by the device.
    pub const fn bytes(self) -> [u8; 4] {
        [
            self.setting.mode.wire(),
            self.setting.position,
            self.airplane_mode,
            self.wind_noise_suppression as u8,
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

impl<'a> RedactedSerial<'a> {
    fn parse(bytes: &'a [u8]) -> Result<Self, ModelError> {
        let bytes: &[u8; 16] = bytes.try_into().map_err(|_| ModelError::InvalidIdentity)?;
        if !bytes.iter().all(u8::is_ascii_alphanumeric) {
            return Err(ModelError::InvalidIdentity);
        }
        Ok(Self(bytes))
    }

    fn identifies(&self, model: ModelDriver) -> bool {
        let code = model.code().as_bytes();
        self.0.starts_with(code) || self.0.starts_with(&code[1..])
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifies_a3954_with_either_known_identity_layout() {
        for body in [
            b"04.2904.29A3954DEMO0000000".as_slice(),
            b"04.3004.303954TEST0000000002.58".as_slice(),
        ] {
            assert_eq!(ModelDriver::identify(body), Ok(ModelDriver::A3954));
        }
    }

    #[test]
    fn rejects_unknown_models_and_partial_identities() {
        assert_eq!(
            ModelDriver::identify(b"04.3004.303988TEST0000000002.58"),
            Err(ModelError::UnknownModel),
        );
        assert_eq!(
            ModelDriver::identify(b"04.3004.303954TEST0000000002.5"),
            Err(ModelError::InvalidIdentity),
        );
    }

    #[test]
    fn registry_selects_the_a3062_driver_from_a_numeric_prefix() {
        let driver =
            ModelDriver::identify(b"04.3904.393062TEST00000000").expect("registered A3062 prefix");
        assert_eq!(driver, ModelDriver::A3062);
        assert!(!driver.capabilities().positioned_sound_mode_write);
        assert!(driver.capabilities().ambient_sound_mode_write);
        assert_eq!(registry::by_code("A3062"), Some(driver));
    }
}
