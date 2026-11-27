//! Model drivers for the device pilot.

#![forbid(unsafe_code)]

use std::fmt;

mod a3027;
mod a3028;
mod a3031;
mod a3062;
mod a3949;
pub mod registry;

pub use a3027::A3027_WEARING_DETECTION;
pub use a3028::{
    A3028_AUTO_POWER_OFF, A3028_EQUALIZER_BANDS, A3028_EQUALIZER_COMMAND, A3028_EQUALIZER_PRESET,
    A3028_SOUND_MODE_FIELDS, AUTO_POWER_OFF_DURATION_COUNT,
};
pub use a3031::{
    A3031_AUTO_POWER_OFF, A3031_EQUALIZER_CHANNEL_1, A3031_EQUALIZER_CHANNEL_2,
    A3031_EQUALIZER_PRESET, A3031_SOUND_MODE_FIELDS, A3031_TWS_CONNECTED, A3031_TWS_STATUS,
};
pub use a3062::{A3062_EQUALIZER_BANDS, A3062_EQUALIZER_PRESET, A3062_SOUND_MODE_FIELDS};
pub use a3949::A3949_GAMING_MODE;

/// Probe for the firmware versions and the device identifier.
pub const IDENTITY_COMMAND: [u8; 2] = [0x01, 0x05];

/// Read the state of a device using the common state command.
pub const STATE_COMMAND: [u8; 2] = [0x01, 0x01];

/// Write the selected driver's sound-mode controls.
pub const SOUND_MODE_COMMAND: [u8; 2] = [0x06, 0x81];

/// Write the equalizer curve together with the hear-id controls that gate it.
pub const EQUALIZER_COMMAND: [u8; 2] = [0x03, 0x87];

/// Write the A3949 gaming-mode flag.
pub const GAMING_MODE_COMMAND: [u8; 2] = [0x01, 0x87];

/// Write the auto power off value of a model that reports its extra fields.
pub const AUTO_POWER_OFF_COMMAND: [u8; 2] = [0x01, 0x86];

/// Write the wearing-detection toggle of a model whose state reports the flag.
pub const WEARING_DETECTION_COMMAND: [u8; 2] = [0x01, 0x81];

const A3954_STATE_LENGTH: usize = 165;
const A3954_CASE_BATTERY: usize = 37;
const A3954_SOUND_MODE: usize = 125;

/// A driver selected from a checked identity response.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelDriver {
    /// Soundcore Life Q35 (A3027).
    A3027,
    /// Soundcore Life Tune Pro (A3030).
    A3030,
    /// Soundcore Q30 / Life Q30 (A3028).
    A3028,
    /// Soundcore Life Tune (A3029).
    A3029,
    /// Soundcore Vortex (A3031).
    A3031,
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
            ambient_sound_mode_write: matches!(
                self,
                Self::A3027 | Self::A3030 | Self::A3028 | Self::A3029 | Self::A3031 | Self::A3062
            ),
            equalizer_write: matches!(
                self,
                Self::A3027 | Self::A3030 | Self::A3028 | Self::A3029 | Self::A3031 | Self::A3062
            ),
            gaming_mode_write: matches!(self, Self::A3949),
            auto_power_off_write: matches!(self, Self::A3028 | Self::A3029 | Self::A3031),
            wearing_detection_write: matches!(self, Self::A3027 | Self::A3030),
        }
    }

    /// Returns the stable model code.
    pub const fn code(self) -> &'static str {
        match self {
            Self::A3027 => "A3027",
            Self::A3030 => "A3030",
            Self::A3028 => "A3028",
            Self::A3029 => "A3029",
            Self::A3031 => "A3031",
            Self::A3062 => "A3062",
            Self::A3949 => "A3949",
            Self::A3954 => "A3954",
        }
    }

    /// Returns the request and reply command for the initial state.
    pub const fn state_command(self) -> [u8; 2] {
        STATE_COMMAND
    }

    /// Number of equalizer bands carried in the model's curve.
    pub const fn equalizer_bands(self) -> usize {
        match self {
            Self::A3027 | Self::A3030 | Self::A3028 | Self::A3029 | Self::A3031 => {
                a3028::EQUALIZER_BAND_COUNT
            }
            Self::A3062 | Self::A3949 | Self::A3954 => EQUALIZER_BANDS,
        }
    }

    /// The wire command for the model's equalizer write.
    pub const fn equalizer_command(self) -> [u8; 2] {
        match self {
            Self::A3027 | Self::A3030 | Self::A3028 | Self::A3029 | Self::A3031 => {
                a3028::A3028_EQUALIZER_COMMAND
            }
            Self::A3062 | Self::A3949 | Self::A3954 => EQUALIZER_COMMAND,
        }
    }

    /// Whether the shared curve fits this model's wire and write format.
    pub fn equalizer_is_writable(self, setting: EqualizerSetting) -> bool {
        self.capabilities().equalizer_write
            && setting.is_writable()
            && setting.volume_adjustments[self.equalizer_bands()..]
                .iter()
                .all(|gain| *gain == 0)
    }

    /// Decodes the state response for the selected model.
    pub fn decode_state(self, body: &[u8]) -> Result<DeviceState, ModelError> {
        match self {
            Self::A3027 | Self::A3030 => a3027::decode_state(self, body),
            Self::A3028 | Self::A3029 => a3028::decode_state(self, body),
            Self::A3031 => a3031::decode_state(body),
            Self::A3062 => a3062::decode_state(body),
            Self::A3949 => a3949::decode_state(body),
            Self::A3954 => self.decode_a3954_state(body),
        }
    }

    /// Decodes a state response only when it has this pilot's measured layout.
    ///
    /// The general decoder accepts trailing data for diagnostics and future
    /// layouts; a write is admitted only from the confirmed state formats. An
    /// A3954 state also needs the measured serial evidence used for redaction,
    /// the A3027/A3030 pair needs the serial that selects its public code, and
    /// an A3028/A3029 state needs its own. The A3031 state carries no serial:
    /// its public code comes from the separate `01 05` identity response, so a
    /// confirmed A3031 state is one whose measured length matches after that
    /// identity selected the driver.
    pub fn decode_confirmed_state(self, body: &[u8]) -> Option<DeviceState> {
        let layout_matches = match self {
            Self::A3027 | Self::A3030 => a3027::has_confirmed_layout(body),
            Self::A3028 | Self::A3029 => a3028::has_confirmed_layout(body),
            Self::A3031 => a3031::has_confirmed_layout(body),
            Self::A3062 => body.len() == a3062::STATE_LENGTH,
            Self::A3949 => body.len() == a3949::STATE_LENGTH,
            Self::A3954 => body.len() == A3954_STATE_LENGTH,
        };
        let serial_matches = match self {
            Self::A3027 | Self::A3030 => {
                a3027::state_serial(body).is_ok_and(|serial| serial.identifies(self))
            }
            Self::A3028 | Self::A3029 => {
                a3028::state_serial(body).is_ok_and(|serial| serial.identifies(self))
            }
            Self::A3954 => a3954_state_serial_is_recognized(body),
            Self::A3031 | Self::A3062 | Self::A3949 => true,
        };
        (layout_matches && serial_matches)
            .then(|| self.decode_state(body).ok())
            .flatten()
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
            equalizer: None,
            gaming_mode: None,
            auto_power_off: None,
            wearing_detection: None,
            tws_connected: None,
            firmware_left: FirmwareVersion::parse(&body[6..11])?,
            firmware_right: FirmwareVersion::parse(&body[11..16])?,
        })
    }

    /// Builds the driver's measured equalizer write body from a fresh state response.
    ///
    /// The write echoes the state's hear-id fields with hear-id disabled, so the
    /// requested curve is the one in effect. The A3027/A3030 pair and the
    /// A3028/A3029 pair write only their eight bands and have no hear-id block;
    /// the A3031 write applies the same curve to both TWS channels.
    pub fn equalizer_write_body(
        self,
        setting: EqualizerSetting,
        state: &[u8],
    ) -> Result<Vec<u8>, ModelError> {
        match self {
            Self::A3027 | Self::A3030 => a3027::equalizer_write_body(setting, state),
            Self::A3028 | Self::A3029 => a3028::equalizer_write_body(setting, state),
            Self::A3031 => a3031::equalizer_write_body(setting, state),
            Self::A3062 => a3062::equalizer_write_body(setting, state),
            Self::A3949 | Self::A3954 => Err(ModelError::InvalidEqualizerWrite),
        }
    }
}

/// Implemented operations exposed by a model driver.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Capabilities {
    /// The session can confirm writes of the sound mode and its scale position.
    pub positioned_sound_mode_write: bool,
    /// The session can confirm writes of the ambient mode without a scale position.
    pub ambient_sound_mode_write: bool,
    /// The session can confirm writes of the equalizer curve.
    pub equalizer_write: bool,
    /// The session can confirm writes of the gaming-mode flag.
    pub gaming_mode_write: bool,
    /// The session can confirm writes of the auto power off value.
    pub auto_power_off_write: bool,
    /// The session can confirm writes of the wearing-detection toggle.
    pub wearing_detection_write: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SoundModeControls {
    /// Ambient mode controls with the ANC mode, transparency and custom level.
    Ambient([u8; 3]),
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
    /// The reported equalizer curve, if the model's state response carries one.
    pub equalizer: Option<EqualizerSetting>,
    /// Whether gaming mode is on, when the device reports a recognized flag.
    pub gaming_mode: Option<bool>,
    /// The reported auto power off value, when the state carries extra fields.
    pub auto_power_off: Option<AutoPowerOff>,
    /// Whether wearing detection is enabled, when the model reports the flag.
    pub wearing_detection: Option<bool>,
    /// Whether the two earbuds are connected, when the model reports TWS status.
    pub tws_connected: Option<bool>,
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

    /// Builds an ambient-mode write from the latest controls of the shared
    /// four-byte sound-mode family (A3027, A3030, A3028, A3029 and A3031).
    pub fn sound_modes_write(self, mode: SoundMode) -> Result<[u8; 4], ModelError> {
        let Some(SoundModeControls::Ambient(controls)) = self.sound_mode_controls else {
            return Err(ModelError::InvalidSoundModeWrite);
        };
        a3027::sound_mode_write(mode, controls)
    }

    /// Builds a wearing-detection write for a device that reports the flag.
    pub fn wearing_detection_write(self, enabled: bool) -> Result<[u8; 1], ModelError> {
        if !self.model.capabilities().wearing_detection_write || self.wearing_detection.is_none() {
            return Err(ModelError::InvalidWearingDetectionWrite);
        }
        Ok([u8::from(enabled)])
    }

    /// Builds a gaming-mode write for a device that reports the measured flag.
    pub fn gaming_mode_write(self, enabled: bool) -> Result<[u8; 1], ModelError> {
        if !self.model.capabilities().gaming_mode_write || self.gaming_mode.is_none() {
            return Err(ModelError::InvalidGamingModeWrite);
        }
        Ok([u8::from(enabled)])
    }

    /// Builds an auto power off write, available only when the state carries
    /// the documented extra fields of a supporting model.
    ///
    /// Disabling keeps the device's previous duration index, matching the
    /// legacy setting handler. The returned expected value is what a fresh
    /// read-back must report.
    pub fn auto_power_off_write(
        self,
        setting: AutoPowerOffSetting,
    ) -> Result<AutoPowerOffWrite, ModelError> {
        if !self.model.capabilities().auto_power_off_write {
            return Err(ModelError::InvalidAutoPowerOffWrite);
        }
        let current = self
            .auto_power_off
            .ok_or(ModelError::InvalidAutoPowerOffWrite)?;
        let (body, expected) = match setting {
            AutoPowerOffSetting::Enabled(index) if index < a3028::AUTO_POWER_OFF_DURATION_COUNT => {
                (
                    [1, index],
                    AutoPowerOff {
                        enabled: true,
                        duration: index,
                    },
                )
            }
            AutoPowerOffSetting::Enabled(_) => return Err(ModelError::InvalidAutoPowerOffWrite),
            AutoPowerOffSetting::Disabled => (
                [0, current.duration],
                AutoPowerOff {
                    enabled: false,
                    duration: current.duration,
                },
            ),
        };
        Ok(AutoPowerOffWrite { body, expected })
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

/// Number of equalizer band gains in the measured models' curves.
pub const EQUALIZER_BANDS: usize = 10;

/// Lowest measured band gain, in tenths of a decibel.
pub const EQUALIZER_MIN_GAIN: i16 = -120;

/// Highest measured band gain, in tenths of a decibel.
pub const EQUALIZER_MAX_GAIN: i16 = 134;

/// An equalizer curve: the preset it belongs to and the gain of every band.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EqualizerSetting {
    /// The preset the curve belongs to, such as the custom profile marker.
    pub preset_id: u16,
    /// Band gains from the lowest band, in tenths of a decibel.
    pub volume_adjustments: [i16; EQUALIZER_BANDS],
}

impl EqualizerSetting {
    /// Whether every band gain can be sent without changing its value.
    pub fn is_writable(self) -> bool {
        self.volume_adjustments
            .iter()
            .all(|gain| (EQUALIZER_MIN_GAIN..=EQUALIZER_MAX_GAIN).contains(gain))
    }

    /// Encodes one band gain in the measured wire scale.
    pub(crate) fn band_byte(gain: i16) -> u8 {
        (gain - EQUALIZER_MIN_GAIN) as u8
    }

    /// Decodes one band gain from the measured wire scale.
    pub(crate) fn band_gain(byte: u8) -> i16 {
        i16::from(byte) + EQUALIZER_MIN_GAIN
    }
}

/// Auto power off value reported by the optional extra fields of the A3028 family.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AutoPowerOff {
    /// Whether the device powers off after the selected idle duration.
    pub enabled: bool,
    /// Index into the family's half-hour duration table.
    pub duration: u8,
}

/// A requested auto power off change.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AutoPowerOffSetting {
    /// Enable auto power off with an index into the half-hour duration table.
    Enabled(u8),
    /// Disable auto power off and keep the device's previous duration index.
    Disabled,
}

/// The wire body and the value that confirms an auto power off write.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AutoPowerOffWrite {
    body: [u8; 2],
    expected: AutoPowerOff,
}

impl AutoPowerOffWrite {
    /// The `01 86` body.
    pub const fn body(self) -> [u8; 2] {
        self.body
    }

    /// The value a fresh state response must report for this write.
    pub const fn expected(self) -> AutoPowerOff {
        self.expected
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
    /// The selected curve does not match the measured equalizer write format.
    #[error("unsupported device equalizer write")]
    InvalidEqualizerWrite,
    /// The device does not report a writable gaming-mode flag.
    #[error("unsupported device gaming mode write")]
    InvalidGamingModeWrite,
    /// The device does not report a writable auto power off value.
    #[error("unsupported device auto power off write")]
    InvalidAutoPowerOffWrite,
    /// The device does not report a writable wearing-detection flag.
    #[error("unsupported device wearing detection write")]
    InvalidWearingDetectionWrite,
    /// A firmware string does not follow the reported version format.
    #[error("invalid device firmware version")]
    InvalidFirmware,
    /// The TWS status fields do not match the supported wire values.
    #[error("invalid device TWS status")]
    InvalidTwsStatus,
}

/// Firmware versions carried by the separate `01 05` identity response.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IdentityFirmware {
    /// Left earbud firmware, if the response carries a dual version.
    pub left: Option<FirmwareVersion>,
    /// Right earbud firmware, if the response carries a dual version.
    pub right: Option<FirmwareVersion>,
}

/// Reads the two firmware versions from a confirmed `01 05` identity body.
///
/// The separate identity response carries two five-byte versions followed by
/// the model serial, so it can supply the versions of a model whose state does
/// not report them. An all-zero or all-`FF` block means the version is absent.
pub fn identity_firmware(body: &[u8]) -> Result<IdentityFirmware, ModelError> {
    if !matches!(body.len(), 26 | 31) {
        return Err(ModelError::InvalidIdentity);
    }
    Ok(IdentityFirmware {
        left: FirmwareVersion::parse(&body[0..5])?,
        right: FirmwareVersion::parse(&body[5..10])?,
    })
}

fn a3954_state_serial_is_recognized(body: &[u8]) -> bool {
    let Some(serial) = body.get(16..32) else {
        return false;
    };
    // The published capture keeps the serial as ASCII zeroes.
    serial.iter().all(|byte| *byte == b'0')
        || RedactedSerial::parse(serial).is_ok_and(|serial| serial.identifies(ModelDriver::A3954))
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
        assert!(driver.capabilities().equalizer_write);
        assert_eq!(registry::by_code("A3062"), Some(driver));
    }

    #[test]
    fn confirmed_layout_requires_the_measured_body_length() {
        for (driver, fixture) in [
            (
                ModelDriver::A3062,
                include_bytes!("../../../data/a3062-pilot-state.bin").as_slice(),
            ),
            (
                ModelDriver::A3949,
                include_bytes!("../../../data/a3949-pilot-state.bin").as_slice(),
            ),
            (
                ModelDriver::A3954,
                include_bytes!("../../../data/a3954-pilot-state.bin").as_slice(),
            ),
        ] {
            let body = &fixture[9..fixture.len() - 1];
            assert!(
                driver.decode_confirmed_state(body).is_some(),
                "measured {} state",
                driver.code()
            );
            let mut extra = body.to_vec();
            extra.push(0);
            assert!(
                driver.decode_confirmed_state(&extra).is_none(),
                "trailing byte for {}",
                driver.code()
            );
        }
    }

    #[test]
    fn confirmed_a3954_state_requires_recognized_serial_evidence() {
        let fixture = include_bytes!("../../../data/a3954-pilot-state.bin");
        let body = &fixture[9..fixture.len() - 1];
        assert!(ModelDriver::A3954.decode_confirmed_state(body).is_some());

        let mut contradictory = body.to_vec();
        contradictory[16..32].copy_from_slice(b"3949TEST00000000");
        assert!(
            ModelDriver::A3954
                .decode_confirmed_state(&contradictory)
                .is_none(),
            "a contradictory serial must not admit writes"
        );
        assert!(
            ModelDriver::A3954.decode_state(&contradictory).is_ok(),
            "the tolerant decoder still reads the layout"
        );

        contradictory[16..32].copy_from_slice(b"3954TEST00000000");
        assert!(
            ModelDriver::A3954
                .decode_confirmed_state(&contradictory)
                .is_some()
        );

        contradictory[16..32].copy_from_slice(&[b'0'; 16]);
        assert!(
            ModelDriver::A3954
                .decode_confirmed_state(&contradictory)
                .is_some()
        );
    }
}
