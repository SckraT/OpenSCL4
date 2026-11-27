//! Model drivers for the device pilot.

#![forbid(unsafe_code)]

use std::fmt;

mod a3028;
mod a3062;
mod a3949;
pub mod registry;

pub use a3028::{
    A3028_AUTO_POWER_OFF, A3028_EQUALIZER_BANDS, A3028_EQUALIZER_COMMAND, A3028_EQUALIZER_PRESET,
    A3028_SOUND_MODE_FIELDS, AUTO_POWER_OFF_DURATION_COUNT,
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

/// Write the A3954 case toggles.
pub const CASE_FEATURES_COMMAND: [u8; 2] = [0x07, 0x87];

/// Write the A3954 case language.
pub const CASE_LANGUAGE_COMMAND: [u8; 2] = [0x07, 0x8A];

/// Write the A3954 spatial-audio controls.
pub const SPATIAL_AUDIO_COMMAND: [u8; 2] = [0x10, 0x81];

/// Write the A3954-specific sound leak compensation flag.
pub const SOUND_LEAK_COMPENSATION_COMMAND: [u8; 2] = [0x10, 0x9A];

/// Write the A3949 gaming-mode flag.
pub const GAMING_MODE_COMMAND: [u8; 2] = [0x01, 0x87];

/// Write the auto power off value of a model that reports its extra fields.
pub const AUTO_POWER_OFF_COMMAND: [u8; 2] = [0x01, 0x86];

const A3954_STATE_LENGTH: usize = 165;
const A3954_CASE_FIRMWARE: std::ops::Range<usize> = 32..37;
const A3954_CASE_BATTERY: usize = 37;
/// Body offset of the TWS-connected flag; the preceding byte is the host device.
const A3954_TWS_CONNECTED: usize = 1;
/// Body offsets of the equalizer preset id and the first measured channel.
pub const A3954_EQUALIZER_PRESET: std::ops::Range<usize> = 44..46;
/// Body offsets of the first equalizer channel in the measured state layout.
pub const A3954_EQUALIZER_BANDS: std::ops::Range<usize> = 46..56;
/// Body offsets of the second channel, used when the first is absent.
pub const A3954_EQUALIZER_BANDS_SECOND: std::ops::Range<usize> = 56..66;
/// Body offsets of the custom hear-id block echoed by an equalizer write.
const A3954_HEAR_ID_VOLUME: std::ops::Range<usize> = 68..88;
const A3954_HEAR_ID_TIME: std::ops::Range<usize> = 88..92;
const A3954_HEAR_ID_TYPE: usize = 92;
const A3954_HEAR_ID_CUSTOM: std::ops::Range<usize> = 93..113;
const A3954_HEAR_ID_GENRE: std::ops::Range<usize> = 113..115;
const A3954_SOUND_MODE: usize = 125;
/// Body offsets of the case-feature toggles.
pub const A3954_CASE_FEATURES: std::ops::Range<usize> = 132..136;
/// Body offsets of the auto power off value.
pub const A3954_AUTO_POWER_OFF: std::ops::Range<usize> = 143..145;
/// Body offsets of the earbud spatial-audio controls.
pub const A3954_SPATIAL_AUDIO: std::ops::Range<usize> = 148..151;
/// Body offset of the sound leak compensation flag.
pub const A3954_SOUND_LEAK_COMPENSATION: usize = 153;
/// Body offset of the case language.
pub const A3954_CASE_LANGUAGE: usize = 157;
/// Length of the measured A3954 equalizer write body.
const A3954_EQUALIZER_WRITE_LENGTH: usize = 114;
/// Offsets of the preset and the two channel curves inside the equalizer write.
pub const A3954_EQUALIZER_WRITE_PRESET: std::ops::Range<usize> = 0..2;
/// Offsets of the two channel curves inside the equalizer write.
pub const A3954_EQUALIZER_WRITE_BANDS: std::ops::Range<usize> = 4..24;
/// Offset of the disabled hear-id flag inside the equalizer write.
pub const A3954_EQUALIZER_WRITE_HEAR_ID_ENABLED: usize = 26;

/// A driver selected from a checked identity response.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelDriver {
    /// Soundcore Q30 / Life Q30 (A3028).
    A3028,
    /// Soundcore Life Tune (A3029).
    A3029,
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
            ambient_sound_mode_write: matches!(self, Self::A3028 | Self::A3029 | Self::A3062),
            equalizer_write: matches!(self, Self::A3028 | Self::A3029 | Self::A3062 | Self::A3954),
            gaming_mode_write: matches!(self, Self::A3949),
            auto_power_off_write: matches!(self, Self::A3028 | Self::A3029 | Self::A3954),
            case_features_write: matches!(self, Self::A3954),
            spatial_audio_write: matches!(self, Self::A3954),
            sound_leak_compensation_write: matches!(self, Self::A3954),
            case_language_write: matches!(self, Self::A3954),
        }
    }

    /// Returns the stable model code.
    pub const fn code(self) -> &'static str {
        match self {
            Self::A3028 => "A3028",
            Self::A3029 => "A3029",
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
            Self::A3028 | Self::A3029 => a3028::EQUALIZER_BAND_COUNT,
            Self::A3062 | Self::A3949 | Self::A3954 => EQUALIZER_BANDS,
        }
    }

    /// The wire command for the model's equalizer write.
    pub const fn equalizer_command(self) -> [u8; 2] {
        match self {
            Self::A3028 | Self::A3029 => a3028::A3028_EQUALIZER_COMMAND,
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
            Self::A3028 | Self::A3029 => a3028::decode_state(self, body),
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
    /// and the A3028 family needs the serial that selects its public code.
    pub fn decode_confirmed_state(self, body: &[u8]) -> Option<DeviceState> {
        let layout_matches = match self {
            Self::A3028 | Self::A3029 => a3028::has_confirmed_layout(body),
            Self::A3062 => body.len() == a3062::STATE_LENGTH,
            Self::A3949 => body.len() == a3949::STATE_LENGTH,
            Self::A3954 => body.len() == A3954_STATE_LENGTH,
        };
        let serial_matches = match self {
            Self::A3028 | Self::A3029 => {
                a3028::state_serial(body).is_ok_and(|serial| serial.identifies(self))
            }
            Self::A3954 => a3954_state_serial_is_recognized(body),
            Self::A3062 | Self::A3949 => true,
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
        let tws_connected = a3954_bool(body[A3954_TWS_CONNECTED]);
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
            // The legacy driver marks the curve unavailable until both earbuds
            // are connected; a write then must be rejected before transmission.
            equalizer: if tws_connected == Some(true) {
                a3954_equalizer_setting(body)
            } else {
                None
            },
            gaming_mode: None,
            auto_power_off: a3954_auto_power_off(body),
            tws_connected,
            case_features: CaseFeatures::decode(&body[A3954_CASE_FEATURES]),
            spatial_audio: SpatialAudio::decode(&body[A3954_SPATIAL_AUDIO]),
            sound_leak_compensation: a3954_bool(body[A3954_SOUND_LEAK_COMPENSATION]),
            case_language: CaseLanguage::decode(body[A3954_CASE_LANGUAGE]),
            firmware_device: None,
            firmware_left: FirmwareVersion::parse(&body[6..11])?,
            firmware_right: FirmwareVersion::parse(&body[11..16])?,
            firmware_case: FirmwareVersion::parse(&body[A3954_CASE_FIRMWARE])?,
        })
    }

    /// Builds the driver's measured equalizer write body from a fresh state response.
    ///
    /// The write echoes the state's hear-id fields with hear-id disabled, so the
    /// requested curve is the one in effect. The A3028 family writes only its
    /// eight bands and has no hear-id block.
    pub fn equalizer_write_body(
        self,
        setting: EqualizerSetting,
        state: &[u8],
    ) -> Result<Vec<u8>, ModelError> {
        match self {
            Self::A3028 | Self::A3029 => a3028::equalizer_write_body(setting, state),
            Self::A3062 => a3062::equalizer_write_body(setting, state),
            Self::A3949 => Err(ModelError::InvalidEqualizerWrite),
            Self::A3954 => a3954_equalizer_write_body(setting, state),
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
    /// The session can confirm writes of the case-feature toggles.
    pub case_features_write: bool,
    /// The session can confirm writes of the earbud spatial-audio controls.
    pub spatial_audio_write: bool,
    /// The session can confirm writes of the sound leak compensation flag.
    pub sound_leak_compensation_write: bool,
    /// The session can confirm writes of the case language.
    pub case_language_write: bool,
}

/// Case toggles carried by one A3954 case-feature write.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CaseFeatures {
    /// Atmospheric pressure detection during descent.
    pub atmospheric: bool,
    /// Remote camera shutter from the case.
    pub remote_camera: bool,
    /// Find-my-device ringing from the case.
    pub find_device: bool,
    /// Spatial audio controlled from the case.
    pub case_spatial_audio: bool,
}

impl CaseFeatures {
    /// Encodes the four toggles in the measured wire order.
    pub const fn bytes(self) -> [u8; 4] {
        [
            self.atmospheric as u8,
            self.remote_camera as u8,
            self.find_device as u8,
            self.case_spatial_audio as u8,
        ]
    }

    /// Reads the four toggles; a non-boolean byte makes the field unknown.
    fn decode(body: &[u8]) -> Option<Self> {
        let [
            atmospheric @ 0..=1,
            remote_camera @ 0..=1,
            find_device @ 0..=1,
            case_spatial_audio @ 0..=1,
        ] = body
        else {
            return None;
        };
        Some(Self {
            atmospheric: *atmospheric == 1,
            remote_camera: *remote_camera == 1,
            find_device: *find_device == 1,
            case_spatial_audio: *case_spatial_audio == 1,
        })
    }
}

/// Spatial-audio playback mode reported by an A3954.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SpatialAudioMode {
    /// Music.
    #[default]
    Music,
    /// Podcast.
    Podcast,
    /// Movie.
    Movie,
    /// Gaming.
    Gaming,
}

impl SpatialAudioMode {
    /// The measured value for a valid wire byte.
    const fn decode(byte: u8) -> Option<Self> {
        match byte {
            0 => Some(Self::Music),
            1 => Some(Self::Podcast),
            2 => Some(Self::Movie),
            3 => Some(Self::Gaming),
            _ => None,
        }
    }

    const fn wire(self) -> u8 {
        match self {
            Self::Music => 0,
            Self::Podcast => 1,
            Self::Movie => 2,
            Self::Gaming => 3,
        }
    }

    /// Returns the stable identifier.
    pub const fn code(self) -> &'static str {
        match self {
            Self::Music => "Music",
            Self::Podcast => "Podcast",
            Self::Movie => "Movie",
            Self::Gaming => "Gaming",
        }
    }
}

/// Head-tracking variant of the A3954 spatial-audio music mode.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SpatialAudioMusicMode {
    /// Fixed.
    #[default]
    Fixed,
    /// Head tracking.
    HeadTracking,
}

impl SpatialAudioMusicMode {
    /// The measured value for a valid wire byte; the legacy parser falls back
    /// to its default for an unrecognized byte.
    const fn decode(byte: u8) -> Self {
        match byte {
            2 => Self::HeadTracking,
            _ => Self::Fixed,
        }
    }

    const fn wire(self) -> u8 {
        match self {
            Self::Fixed => 1,
            Self::HeadTracking => 2,
        }
    }

    /// Returns the stable identifier.
    pub const fn code(self) -> &'static str {
        match self {
            Self::Fixed => "Fixed",
            Self::HeadTracking => "HeadTracking",
        }
    }
}

/// Earbud spatial-audio controls carried by one A3954 write.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SpatialAudio {
    /// Whether spatial audio is on.
    pub enabled: bool,
    /// Playback mode.
    pub mode: SpatialAudioMode,
    /// Music mode.
    pub music_mode: SpatialAudioMusicMode,
}

impl SpatialAudio {
    /// Encodes the three controls in the measured wire order.
    pub const fn bytes(self) -> [u8; 3] {
        [self.enabled as u8, self.music_mode.wire(), self.mode.wire()]
    }

    /// Reads the three controls; a non-boolean enable flag makes the field unknown.
    fn decode(body: &[u8]) -> Option<Self> {
        let [enabled @ 0..=1, music_mode, mode] = body else {
            return None;
        };
        Some(Self {
            enabled: *enabled == 1,
            mode: SpatialAudioMode::decode(*mode)?,
            music_mode: SpatialAudioMusicMode::decode(*music_mode),
        })
    }
}

/// Language of the A3954 charging case prompts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CaseLanguage {
    /// Japanese.
    #[default]
    Japanese,
    /// English.
    English,
    /// Chinese.
    Chinese,
}

impl CaseLanguage {
    /// The measured value for a valid wire byte.
    const fn decode(byte: u8) -> Option<Self> {
        match byte {
            0 => Some(Self::Japanese),
            1 => Some(Self::English),
            2 => Some(Self::Chinese),
            _ => None,
        }
    }

    const fn wire(self) -> u8 {
        match self {
            Self::Japanese => 0,
            Self::English => 1,
            Self::Chinese => 2,
        }
    }

    /// Returns the stable identifier.
    pub const fn code(self) -> &'static str {
        match self {
            Self::Japanese => "Japanese",
            Self::English => "English",
            Self::Chinese => "Chinese",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SoundModeControls {
    A3028([u8; 3]),
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
    /// Whether both earbuds are connected, when the state reports TWS status.
    pub tws_connected: Option<bool>,
    /// The reported case-feature toggles, if the state carries them.
    pub case_features: Option<CaseFeatures>,
    /// The reported earbud spatial-audio controls, if the state carries them.
    pub spatial_audio: Option<SpatialAudio>,
    /// Whether sound leak compensation is on, if the state reports the flag.
    pub sound_leak_compensation: Option<bool>,
    /// The reported case language, if the state carries it.
    pub case_language: Option<CaseLanguage>,
    /// The firmware of a device with a single battery, if reported.
    pub firmware_device: Option<FirmwareVersion>,
    /// Left earbud firmware, if reported.
    pub firmware_left: Option<FirmwareVersion>,
    /// Right earbud firmware, if reported.
    pub firmware_right: Option<FirmwareVersion>,
    /// Charging-case firmware, if reported.
    pub firmware_case: Option<FirmwareVersion>,
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

    /// Builds an ambient-mode write from the latest A3028/A3029 controls.
    pub fn family_sound_mode_write(self, mode: SoundMode) -> Result<[u8; 4], ModelError> {
        let Some(SoundModeControls::A3028(controls)) = self.sound_mode_controls else {
            return Err(ModelError::InvalidSoundModeWrite);
        };
        a3028::sound_mode_write(mode, controls)
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

    /// Builds a case-feature write for a device that reports the toggles.
    pub fn case_features_write(self, setting: CaseFeatures) -> Result<[u8; 4], ModelError> {
        if !self.model.capabilities().case_features_write || self.case_features.is_none() {
            return Err(ModelError::InvalidCaseFeaturesWrite);
        }
        Ok(setting.bytes())
    }

    /// Builds a spatial-audio write for a device that reports the controls.
    pub fn spatial_audio_write(self, setting: SpatialAudio) -> Result<[u8; 3], ModelError> {
        if !self.model.capabilities().spatial_audio_write || self.spatial_audio.is_none() {
            return Err(ModelError::InvalidSpatialAudioWrite);
        }
        Ok(setting.bytes())
    }

    /// Builds a sound leak compensation write for a device that reports the flag.
    pub fn sound_leak_compensation_write(self, enabled: bool) -> Result<[u8; 1], ModelError> {
        if !self.model.capabilities().sound_leak_compensation_write
            || self.sound_leak_compensation.is_none()
        {
            return Err(ModelError::InvalidSoundLeakCompensationWrite);
        }
        Ok([u8::from(enabled)])
    }

    /// Builds a case language write for a device that reports the language.
    pub fn case_language_write(self, language: CaseLanguage) -> Result<[u8; 1], ModelError> {
        if !self.model.capabilities().case_language_write || self.case_language.is_none() {
            return Err(ModelError::InvalidCaseLanguageWrite);
        }
        Ok([language.wire()])
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
    /// The device does not report writable case-feature toggles.
    #[error("unsupported device case feature write")]
    InvalidCaseFeaturesWrite,
    /// The device does not report writable spatial-audio controls.
    #[error("unsupported device spatial audio write")]
    InvalidSpatialAudioWrite,
    /// The device does not report a writable sound leak compensation flag.
    #[error("unsupported device sound leak compensation write")]
    InvalidSoundLeakCompensationWrite,
    /// The device does not report a writable case language.
    #[error("unsupported device case language write")]
    InvalidCaseLanguageWrite,
    /// A firmware string does not follow the reported version format.
    #[error("invalid device firmware version")]
    InvalidFirmware,
}

/// Reads the A3954 curve from the first measured channel. A channel that
/// starts with the absent marker reports no curve.
fn a3954_equalizer_setting(body: &[u8]) -> Option<EqualizerSetting> {
    let preset_id: [u8; 2] = body.get(A3954_EQUALIZER_PRESET)?.try_into().ok()?;
    for range in [A3954_EQUALIZER_BANDS, A3954_EQUALIZER_BANDS_SECOND] {
        let bands: [u8; 10] = body.get(range.clone())?.try_into().ok()?;
        if bands[0] == u8::MAX {
            continue;
        }
        return Some(EqualizerSetting {
            preset_id: u16::from_le_bytes(preset_id),
            volume_adjustments: bands.map(EqualizerSetting::band_gain),
        });
    }
    None
}

/// Reads the A3954 auto power off value. An unrecognized flag or duration index
/// is not reported as a confirmed value.
fn a3954_auto_power_off(body: &[u8]) -> Option<AutoPowerOff> {
    let [enabled, duration] = body.get(A3954_AUTO_POWER_OFF)?.try_into().ok()?;
    let enabled = a3954_bool(enabled)?;
    (duration < a3028::AUTO_POWER_OFF_DURATION_COUNT).then_some(AutoPowerOff { enabled, duration })
}

/// Reads a byte the legacy parser treats as `true` only at one.
fn a3954_bool(byte: u8) -> Option<bool> {
    match byte {
        0 => Some(false),
        1 => Some(true),
        _ => None,
    }
}

/// Builds the measured A3954 equalizer write from the current state.
///
/// The write repeats the requested curve on both channels, carries the state's
/// hear-id fields with hear-id disabled so the curve takes effect, and ends with
/// the active curve interleaved with zeroes. Both earbuds must be connected for
/// the legacy driver to expose the curve, so a state that reports only one
/// earbud rejects the write before transmission.
fn a3954_equalizer_write_body(
    setting: EqualizerSetting,
    state: &[u8],
) -> Result<Vec<u8>, ModelError> {
    if !setting.is_writable() {
        return Err(ModelError::InvalidEqualizerWrite);
    }
    if state.len() < A3954_STATE_LENGTH {
        return Err(ModelError::IncompleteState);
    }
    if state[A3954_TWS_CONNECTED] != 1 {
        return Err(ModelError::InvalidEqualizerWrite);
    }
    let bands = setting.volume_adjustments.map(EqualizerSetting::band_byte);
    let mut body = Vec::with_capacity(A3954_EQUALIZER_WRITE_LENGTH);
    body.extend_from_slice(&setting.preset_id.to_le_bytes());
    // The favorite music genre precedes the curve on the write.
    body.extend_from_slice(&state[A3954_HEAR_ID_GENRE]);
    body.extend_from_slice(&bands);
    body.extend_from_slice(&bands);
    body.extend_from_slice(&[0, 0]);
    // The measured write disables hear id so the requested curve is in effect.
    body.push(0);
    body.extend_from_slice(&state[A3954_HEAR_ID_VOLUME]);
    body.extend_from_slice(&state[A3954_HEAR_ID_TIME]);
    body.push(state[A3954_HEAR_ID_TYPE]);
    body.extend_from_slice(&state[A3954_HEAR_ID_CUSTOM]);
    // The active curve repeats as interleaved gain/zero pairs after the hear-id fields.
    for _ in 0..2 {
        for band in bands {
            body.push(band);
            body.push(0);
        }
    }
    body.extend_from_slice(&[0, 0]);
    debug_assert_eq!(body.len(), A3954_EQUALIZER_WRITE_LENGTH);
    Ok(body)
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

    /// The state body of a complete inbound fixture frame.
    fn fixture_body(fixture: &[u8]) -> &[u8] {
        &fixture[9..fixture.len() - 1]
    }

    #[test]
    fn reads_the_measured_a3954_capture() {
        let state = ModelDriver::A3954
            .decode_state(fixture_body(include_bytes!(
                "../../../data/a3954-pilot-state.bin"
            )))
            .expect("captured A3954 state");
        assert_eq!(state.tws_connected, Some(true));
        assert_eq!(state.battery.left, Some(96));
        assert_eq!(state.battery.right, Some(98));
        assert_eq!(state.battery.case, Some(100));
        assert_eq!(
            state.firmware_left.map(|version| version.to_string()),
            Some("04.29".to_owned())
        );
        assert_eq!(
            state.firmware_right.map(|version| version.to_string()),
            Some("04.29".to_owned())
        );
        assert_eq!(
            state.firmware_case.map(|version| version.to_string()),
            Some("01.58".to_owned())
        );
        assert_eq!(state.sound_mode, Some(SoundMode::NoiseCanceling));
        assert_eq!(
            state.equalizer,
            Some(EqualizerSetting {
                preset_id: 0xfefe,
                volume_adjustments: [18, 8, 0, 0, 0, 0, 0, 0, 0, 0],
            })
        );
        assert_eq!(
            state.auto_power_off,
            Some(AutoPowerOff {
                enabled: true,
                duration: 2,
            })
        );
        assert_eq!(
            state.case_features,
            Some(CaseFeatures {
                atmospheric: false,
                remote_camera: false,
                find_device: true,
                case_spatial_audio: false,
            })
        );
        assert_eq!(
            state.spatial_audio,
            Some(SpatialAudio {
                enabled: false,
                mode: SpatialAudioMode::Music,
                music_mode: SpatialAudioMusicMode::Fixed,
            })
        );
        assert_eq!(state.sound_leak_compensation, Some(false));
        assert_eq!(state.case_language, Some(CaseLanguage::English));
    }

    #[test]
    fn reads_the_legacy_a3954_reference_state() {
        let state = ModelDriver::A3954
            .decode_state(fixture_body(include_bytes!(
                "../../../data/a3954-legacy-state.bin"
            )))
            .expect("legacy reference state");
        assert_eq!(state.battery.left, Some(99));
        assert_eq!(state.battery.right, Some(100));
        assert_eq!(state.battery.case, Some(100));
        assert_eq!(state.sound_mode, Some(SoundMode::Normal));
        assert_eq!(
            state.equalizer,
            Some(EqualizerSetting {
                preset_id: 0xfefe,
                volume_adjustments: [0; 10],
            })
        );
        assert_eq!(
            state.auto_power_off,
            Some(AutoPowerOff {
                enabled: true,
                duration: 0,
            })
        );
        assert_eq!(state.case_features, Some(CaseFeatures::default()));
        assert_eq!(state.sound_leak_compensation, Some(false));
        assert_eq!(state.case_language, Some(CaseLanguage::English));
    }

    #[test]
    fn a3954_equalizer_write_matches_the_legacy_reference_frame() {
        let body = fixture_body(include_bytes!("../../../data/a3954-legacy-state.bin"));
        let curve = EqualizerSetting {
            preset_id: 0xfefe,
            volume_adjustments: [60, 0, 0, 0, 0, 0, 0, -60, 0, 0],
        };
        let write = ModelDriver::A3954
            .equalizer_write_body(curve, body)
            .expect("legacy equalizer write");
        let expected: [u8; 114] = [
            0xfe, 0xfe, 0x00, 0x00, 0xb4, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x3c, 0x78, 0x78,
            0xb4, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x3c, 0x78, 0x78, 0x00, 0x00, 0x00, 0x91,
            0x93, 0x8b, 0x8d, 0x7a, 0x6f, 0x69, 0x64, 0x3c, 0x3c, 0x91, 0x93, 0x8b, 0x8d, 0x7a,
            0x6f, 0x69, 0x64, 0x3c, 0x3c, 0x00, 0x00, 0x00, 0x00, 0x01, 0x91, 0x93, 0x8b, 0x82,
            0x7a, 0x85, 0x85, 0x72, 0x3c, 0x3c, 0x91, 0x93, 0x8b, 0x82, 0x7a, 0x85, 0x85, 0x72,
            0x3c, 0x3c, 0xb4, 0x00, 0x78, 0x00, 0x78, 0x00, 0x78, 0x00, 0x78, 0x00, 0x78, 0x00,
            0x78, 0x00, 0x3c, 0x00, 0x78, 0x00, 0x78, 0x00, 0xb4, 0x00, 0x78, 0x00, 0x78, 0x00,
            0x78, 0x00, 0x78, 0x00, 0x78, 0x00, 0x78, 0x00, 0x3c, 0x00, 0x78, 0x00, 0x78, 0x00,
            0x00, 0x00,
        ];
        assert_eq!(write, expected);
    }

    #[test]
    fn a3954_equalizer_requires_both_earbuds() {
        let mut body = fixture_body(include_bytes!("../../../data/a3954-pilot-state.bin")).to_vec();
        body[1] = 0;
        let state = ModelDriver::A3954
            .decode_state(&body)
            .expect("one-earbud state");
        assert_eq!(state.tws_connected, Some(false));
        assert_eq!(state.equalizer, None);
        assert_eq!(
            ModelDriver::A3954.equalizer_write_body(
                EqualizerSetting {
                    preset_id: 0xfefe,
                    volume_adjustments: [0; 10],
                },
                &body,
            ),
            Err(ModelError::InvalidEqualizerWrite)
        );
    }

    #[test]
    fn a3954_writes_use_the_legacy_wire_formats() {
        let body = fixture_body(include_bytes!("../../../data/a3954-legacy-state.bin"));
        let state = ModelDriver::A3954
            .decode_state(body)
            .expect("legacy reference state");
        assert_eq!(
            state
                .case_features_write(CaseFeatures {
                    case_spatial_audio: true,
                    ..CaseFeatures::default()
                })
                .expect("case feature write"),
            [0x00, 0x00, 0x00, 0x01]
        );
        assert_eq!(
            state
                .spatial_audio_write(SpatialAudio {
                    enabled: true,
                    mode: SpatialAudioMode::Music,
                    music_mode: SpatialAudioMusicMode::Fixed,
                })
                .expect("spatial audio write"),
            [0x01, 0x01, 0x00]
        );
        assert_eq!(
            state
                .sound_leak_compensation_write(true)
                .expect("sound leak compensation write"),
            [0x01]
        );
        assert_eq!(
            state
                .case_language_write(CaseLanguage::Chinese)
                .expect("case language write"),
            [0x02]
        );
        let enabled = state
            .auto_power_off_write(AutoPowerOffSetting::Enabled(3))
            .expect("auto power off write");
        assert_eq!(enabled.body(), [0x01, 0x03]);
        assert_eq!(
            enabled.expected(),
            AutoPowerOff {
                enabled: true,
                duration: 3,
            }
        );
        let disabled = state
            .auto_power_off_write(AutoPowerOffSetting::Disabled)
            .expect("auto power off disable");
        assert_eq!(disabled.body(), [0x00, 0x00]);
        assert_eq!(
            disabled.expected(),
            AutoPowerOff {
                enabled: false,
                duration: 0,
            }
        );
    }

    #[test]
    fn a3954_unknown_fields_are_not_reported_as_confirmed() {
        let captured = fixture_body(include_bytes!("../../../data/a3954-pilot-state.bin"));
        let decode = |offset: usize, value: u8| {
            let mut body = captured.to_vec();
            body[offset] = value;
            ModelDriver::A3954
                .decode_state(&body)
                .expect("measured layout with one changed field")
        };

        assert_eq!(decode(A3954_CASE_FEATURES.start, 2).case_features, None);
        assert_eq!(decode(A3954_SPATIAL_AUDIO.start, 2).spatial_audio, None);
        assert_eq!(decode(A3954_SPATIAL_AUDIO.start + 2, 4).spatial_audio, None);
        assert_eq!(
            decode(A3954_SOUND_LEAK_COMPENSATION, 2).sound_leak_compensation,
            None
        );
        assert_eq!(decode(A3954_CASE_LANGUAGE, 3).case_language, None);
        assert_eq!(
            decode(A3954_AUTO_POWER_OFF.start + 1, 4).auto_power_off,
            None
        );
    }
}
