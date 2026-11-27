//! Soundcore Sleep A30 (D1301) state, sleep control, timer and alarm codecs.
//!
//! The wire formats come from the legacy driver in
//! `lib/src/devices/soundcore/d1301`; the published references are
//! `core/data/d1301-post-sleep-*.bin` and `core/data/d1301-sleep-reference.json`.

use std::ops::Range;

use crate::{
    BatteryLevels, DeviceState, FirmwareVersion, ModelDriver, ModelError, RedactedSerial,
    battery_percent,
};

/// Body length of every captured D1301 state response.
///
/// The captures carry one trailing byte after the auto-switch flag that the
/// legacy parser ignores; the confirmed layout still requires the measured
/// length.
pub const STATE_LENGTH: usize = 150;

const BATTERY_LEFT: usize = 2;
const BATTERY_RIGHT: usize = 3;
const BATTERY_SCALE: u8 = 10;
const BATTERY_OFFSET: u8 = 1;
const FIRMWARE_LEFT: Range<usize> = 4..9;
const FIRMWARE_RIGHT: Range<usize> = 9..14;
/// Body offsets of the 16-byte serial in the measured state layout.
pub const STATE_SERIAL: Range<usize> = 14..30;
/// Body offset of the listening mode in the measured state layout.
pub const LISTENING_MODE: usize = 65;
/// Body offset of the default listening mode in the measured state layout.
pub const DEFAULT_LISTENING_MODE: usize = 66;
/// Body offset of the noise canceling flag in the measured state layout.
pub const NOISE_CANCELING: usize = 140;
/// Body offset of the post-sleep audio action in the measured state layout.
pub const POST_SLEEP_AUDIO: usize = 144;
/// Body offset of the auto-switch-once-asleep flag in the measured state layout.
pub const AUTO_SWITCH_ONCE_ASLEEP: usize = 148;

/// Reads the auto-stop timer report.
pub const AUTO_STOP_TIMER_COMMAND: [u8; 2] = [0x15, 0x03];
/// Writes the auto-stop timer or the post-sleep action, depending on the body.
pub const SLEEP_CONTROL_COMMAND: [u8; 2] = [0x15, 0x85];
/// Writes the auto-switch-once-asleep flag.
pub const AUTO_SWITCH_ONCE_ASLEEP_COMMAND: [u8; 2] = [0x15, 0x8F];
/// Writes the listening mode.
pub const LISTENING_MODE_COMMAND: [u8; 2] = [0x01, 0xA9];
/// Writes the default listening mode.
pub const DEFAULT_LISTENING_MODE_COMMAND: [u8; 2] = [0x10, 0x95];
/// Writes the noise canceling flag.
pub const NOISE_CANCELING_COMMAND: [u8; 2] = [0x06, 0x87];
/// Reads the alarm list.
pub const ALARMS_COMMAND: [u8; 2] = [0x14, 0x01];
/// Creates or replaces one alarm record.
pub const SET_ALARM_COMMAND: [u8; 2] = [0x14, 0x81];
/// Deletes the alarm with the given id.
pub const DELETE_ALARM_COMMAND: [u8; 2] = [0x14, 0x82];

/// Length of one alarm record on the wire.
pub const ALARM_LENGTH: usize = 8;
/// Number of alarms the legacy driver lets the user create.
pub const MAX_ALARMS: usize = 5;
/// Lowest auto-stop timer duration the legacy setting handler accepts, in minutes.
pub const AUTO_STOP_TIMER_MIN_MINUTES: i16 = 1;
/// Highest auto-stop timer duration the legacy setting handler accepts, in minutes.
pub const AUTO_STOP_TIMER_MAX_MINUTES: i16 = 240;

/// The listening mode of the sleep earbuds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListeningMode {
    /// Audio comes from the paired Bluetooth source.
    Bluetooth,
    /// The earbuds play the audio stored on them.
    Local,
}

impl ListeningMode {
    fn decode(byte: u8) -> Option<Self> {
        match byte {
            0 => Some(Self::Bluetooth),
            1 => Some(Self::Local),
            _ => None,
        }
    }

    /// Encodes the mode accepted by the device.
    pub const fn wire(self) -> u8 {
        match self {
            Self::Bluetooth => 0,
            Self::Local => 1,
        }
    }

    /// Returns the stable CLI identifier.
    pub const fn code(self) -> &'static str {
        match self {
            Self::Bluetooth => "Bluetooth",
            Self::Local => "Local",
        }
    }
}

/// What the earbuds do with audio once they detect that the wearer is asleep.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PostSleepAudio {
    /// Pause playback.
    Pause,
    /// Play the audio stored on the earbuds.
    PlayLocal,
}

impl PostSleepAudio {
    fn decode(byte: u8) -> Option<Self> {
        match byte {
            0 => Some(Self::Pause),
            1 => Some(Self::PlayLocal),
            _ => None,
        }
    }

    /// Encodes the action accepted by the device.
    pub const fn wire(self) -> u8 {
        match self {
            Self::Pause => 0,
            Self::PlayLocal => 1,
        }
    }

    /// Returns the stable CLI identifier.
    pub const fn code(self) -> &'static str {
        match self {
            Self::Pause => "Pause",
            Self::PlayLocal => "PlayLocal",
        }
    }
}

/// The reported post-sleep behavior. The action is retained while disabled.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SleepAudio {
    /// Whether audio switches when the device detects sleep.
    pub enabled: bool,
    /// The action that is applied when it does.
    pub action: PostSleepAudio,
}

/// The auto-stop timer state reported by the `15 03` reply.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AutoStopTimer {
    /// Whether the timer is armed.
    pub enabled: bool,
    /// Configured duration in minutes.
    pub duration_minutes: i16,
    /// Unidentified byte the legacy driver preserves when writing the timer.
    pub unknown: u8,
    /// Seconds left before the device stops playback.
    pub time_left_seconds: i32,
}

impl AutoStopTimer {
    /// Parses the eight-byte `15 03` reply body.
    pub fn decode(body: &[u8]) -> Result<Self, ModelError> {
        let [
            enabled,
            duration_lo,
            duration_hi,
            unknown,
            left_0,
            left_1,
            left_2,
            left_3,
        ] = body
        else {
            return Err(ModelError::InvalidAutoStopTimer);
        };
        Ok(Self {
            enabled: *enabled == 1,
            duration_minutes: i16::from_le_bytes([*duration_lo, *duration_hi]),
            unknown: *unknown,
            time_left_seconds: i32::from_le_bytes([*left_0, *left_1, *left_2, *left_3]),
        })
    }

    /// Encodes the `15 85` timer body.
    ///
    /// The write repeats the saved unidentified byte and leaves the remaining
    /// seconds to the device. The duration range is the legacy setting range.
    pub fn write_body(self, enabled: bool, duration_minutes: i16) -> Result<[u8; 4], ModelError> {
        if !(AUTO_STOP_TIMER_MIN_MINUTES..=AUTO_STOP_TIMER_MAX_MINUTES).contains(&duration_minutes)
        {
            return Err(ModelError::InvalidAutoStopTimer);
        }
        let [duration_lo, duration_hi] = duration_minutes.to_le_bytes();
        Ok([u8::from(enabled), duration_lo, duration_hi, self.unknown])
    }
}

/// Days of the week an alarm repeats on, matching the legacy bit flags.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AlarmRepeat(u8);

impl AlarmRepeat {
    /// Sunday, the highest bit.
    pub const SUNDAY: Self = Self(1 << 7);
    /// Monday.
    pub const MONDAY: Self = Self(1 << 6);
    /// Tuesday.
    pub const TUESDAY: Self = Self(1 << 5);
    /// Wednesday.
    pub const WEDNESDAY: Self = Self(1 << 4);
    /// Thursday.
    pub const THURSDAY: Self = Self(1 << 3);
    /// Friday.
    pub const FRIDAY: Self = Self(1 << 2);
    /// Saturday.
    pub const SATURDAY: Self = Self(1 << 1);

    /// Drops bits the legacy driver does not use.
    pub const fn from_bits_truncate(bits: u8) -> Self {
        Self(bits & 0b1111_1110)
    }

    /// The raw repeat byte.
    pub const fn bits(self) -> u8 {
        self.0
    }

    /// Whether every day in `other` is set in this value.
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}

impl std::ops::BitOr for AlarmRepeat {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

/// The wake-up tune an alarm plays.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlarmWakeUpTune {
    /// Nature.
    Nature,
    /// Glow.
    Glow,
    /// Rhythm.
    Rhythm,
    /// Bloom.
    Bloom,
}

impl AlarmWakeUpTune {
    /// The tune the legacy parser falls back to for an unknown byte.
    const DEFAULT: Self = Self::Nature;

    const fn decode(byte: u8) -> Self {
        match byte {
            0 => Self::Nature,
            1 => Self::Glow,
            2 => Self::Rhythm,
            3 => Self::Bloom,
            _ => Self::DEFAULT,
        }
    }

    /// Encodes the tune accepted by the device.
    pub const fn wire(self) -> u8 {
        match self {
            Self::Nature => 0,
            Self::Glow => 1,
            Self::Rhythm => 2,
            Self::Bloom => 3,
        }
    }
}

/// One alarm record from the `14 01` alarm list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Alarm {
    /// Slot id; the legacy driver expects the list to be sorted by it.
    pub id: u8,
    /// Whether the alarm rings.
    pub enabled: bool,
    /// Wake-up time in minutes after midnight.
    pub time: i16,
    /// Days the alarm repeats on.
    pub repeat: AlarmRepeat,
    /// Tune the alarm plays.
    pub wake_up_tune: AlarmWakeUpTune,
    /// Volume percentage, clamped to the legacy range on decode.
    pub volume: u8,
    /// Snooze duration in minutes.
    pub snooze_duration_in_minutes: u8,
}

impl Alarm {
    /// The record the legacy driver creates for a new alarm.
    pub const fn default_with_id(id: u8) -> Self {
        Self {
            id,
            enabled: false,
            time: 0,
            repeat: AlarmRepeat::from_bits_truncate(0),
            wake_up_tune: AlarmWakeUpTune::Nature,
            volume: 50,
            snooze_duration_in_minutes: 10,
        }
    }

    /// Parses one eight-byte record.
    pub fn decode(bytes: &[u8]) -> Result<Self, ModelError> {
        let [id, enabled, time_lo, time_hi, repeat, tune, volume, snooze] = bytes else {
            return Err(ModelError::InvalidAlarmList);
        };
        Ok(Self {
            id: *id,
            enabled: *enabled == 1,
            time: i16::from_le_bytes([*time_lo, *time_hi]),
            repeat: AlarmRepeat::from_bits_truncate(*repeat),
            wake_up_tune: AlarmWakeUpTune::decode(*tune),
            // The legacy structure clamps the volume to its percentage range.
            volume: (*volume).min(100),
            snooze_duration_in_minutes: *snooze,
        })
    }

    /// Encodes the record carried by a `14 81` write.
    pub fn bytes(self) -> [u8; ALARM_LENGTH] {
        let [time_lo, time_hi] = self.time.to_le_bytes();
        [
            self.id,
            u8::from(self.enabled),
            time_lo,
            time_hi,
            self.repeat.bits(),
            self.wake_up_tune.wire(),
            self.volume,
            self.snooze_duration_in_minutes,
        ]
    }
}

/// Parses the `14 01` alarm list.
///
/// Every record is eight bytes, so any other body length is rejected rather
/// than silently truncated.
pub fn decode_alarms(body: &[u8]) -> Result<Vec<Alarm>, ModelError> {
    let (records, remainder) = body.as_chunks::<ALARM_LENGTH>();
    if !remainder.is_empty() {
        return Err(ModelError::InvalidAlarmList);
    }
    records.iter().map(|record| Alarm::decode(record)).collect()
}

/// Encodes the body that creates or replaces the given alarm.
pub fn set_alarm_body(alarm: &Alarm) -> [u8; ALARM_LENGTH] {
    alarm.bytes()
}

/// Encodes the `14 82` body that deletes the alarm with this id.
pub fn delete_alarm_body(id: u8) -> [u8; 1] {
    [id]
}

/// Encodes the `15 85` body that sets the post-sleep action.
///
/// The timer fields are the three `FF` sentinels the official app sends so the
/// device leaves the timer alone.
pub fn post_sleep_audio_body(action: PostSleepAudio) -> [u8; 4] {
    [0xFF, 0xFF, 0xFF, action.wire()]
}

/// A D1301 setting the session can write and confirm from a fresh state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum D1301Setting {
    /// The current listening mode.
    ListeningMode(ListeningMode),
    /// The listening mode the device selects when it powers on.
    DefaultListeningMode(ListeningMode),
    /// The noise canceling flag.
    NoiseCanceling(bool),
    /// The action applied after the device detects sleep.
    PostSleepAudio(PostSleepAudio),
    /// Whether audio switches once the device detects sleep.
    AutoSwitchOnceAsleep(bool),
}

impl D1301Setting {
    /// The wire command that carries this write.
    pub const fn command(self) -> [u8; 2] {
        match self {
            Self::ListeningMode(_) => LISTENING_MODE_COMMAND,
            Self::DefaultListeningMode(_) => DEFAULT_LISTENING_MODE_COMMAND,
            Self::NoiseCanceling(_) => NOISE_CANCELING_COMMAND,
            Self::PostSleepAudio(_) => SLEEP_CONTROL_COMMAND,
            Self::AutoSwitchOnceAsleep(_) => AUTO_SWITCH_ONCE_ASLEEP_COMMAND,
        }
    }

    /// The command body, taken from the legacy builders.
    pub fn write_body(self) -> Vec<u8> {
        match self {
            Self::ListeningMode(mode) => vec![mode.wire()],
            Self::DefaultListeningMode(mode) => vec![mode.wire()],
            Self::NoiseCanceling(enabled) => vec![u8::from(enabled)],
            Self::PostSleepAudio(action) => post_sleep_audio_body(action).to_vec(),
            Self::AutoSwitchOnceAsleep(enabled) => vec![u8::from(enabled)],
        }
    }

    /// Whether the confirmed state carries this setting.
    pub fn is_reported(self, state: DeviceState) -> bool {
        self.observed(state).is_some()
    }

    /// The same setting as reported by a confirmed state.
    pub fn observed(self, state: DeviceState) -> Option<Self> {
        match self {
            Self::ListeningMode(_) => state.listening_mode.map(Self::ListeningMode),
            Self::DefaultListeningMode(_) => {
                state.default_listening_mode.map(Self::DefaultListeningMode)
            }
            Self::NoiseCanceling(_) => state.noise_canceling.map(Self::NoiseCanceling),
            Self::PostSleepAudio(_) => state
                .sleep_audio
                .map(|sleep_audio| Self::PostSleepAudio(sleep_audio.action)),
            Self::AutoSwitchOnceAsleep(_) => state
                .sleep_audio
                .map(|sleep_audio| Self::AutoSwitchOnceAsleep(sleep_audio.enabled)),
        }
    }

    /// Whether a confirmed state already reports the requested value.
    pub fn matches(self, state: DeviceState) -> bool {
        self.observed(state) == Some(self)
    }
}

/// Reads the model serial from the measured state layout.
pub(crate) fn state_serial(body: &[u8]) -> Result<RedactedSerial<'_>, ModelError> {
    if body.len() < STATE_LENGTH {
        return Err(ModelError::IncompleteState);
    }
    RedactedSerial::parse(&body[STATE_SERIAL])
}

/// Whether the body has the measured 150-byte D1301 layout.
pub fn has_confirmed_layout(body: &[u8]) -> bool {
    body.len() == STATE_LENGTH
}

/// Decodes the state response of a device identified as D1301.
pub fn decode_state(body: &[u8]) -> Result<DeviceState, ModelError> {
    if body.len() < STATE_LENGTH {
        return Err(ModelError::IncompleteState);
    }
    if !state_serial(body)?.identifies(ModelDriver::D1301) {
        return Err(ModelError::UnknownModel);
    }
    Ok(DeviceState {
        model: ModelDriver::D1301,
        battery: BatteryLevels {
            device: None,
            left: battery_percent(body[BATTERY_LEFT], BATTERY_SCALE, BATTERY_OFFSET)?,
            right: battery_percent(body[BATTERY_RIGHT], BATTERY_SCALE, BATTERY_OFFSET)?,
            case: None,
        },
        sound_mode: None,
        sound_mode_controls: None,
        equalizer: None,
        gaming_mode: None,
        auto_power_off: None,
        listening_mode: ListeningMode::decode(body[LISTENING_MODE]),
        default_listening_mode: ListeningMode::decode(body[DEFAULT_LISTENING_MODE]),
        // The legacy flag parser treats any value other than one as false.
        noise_canceling: Some(body[NOISE_CANCELING] == 1),
        sleep_audio: PostSleepAudio::decode(body[POST_SLEEP_AUDIO]).map(|action| SleepAudio {
            enabled: body[AUTO_SWITCH_ONCE_ASLEEP] == 1,
            action,
        }),
        firmware_device: None,
        firmware_left: FirmwareVersion::parse(&body[FIRMWARE_LEFT])?,
        firmware_right: FirmwareVersion::parse(&body[FIRMWARE_RIGHT])?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAUSE_STATE: &[u8] = include_bytes!("../../../data/d1301-post-sleep-pause.bin");

    fn state_body(fixture: &[u8]) -> &[u8] {
        assert_eq!(fixture[5..7], [0x01, 0x01]);
        &fixture[9..fixture.len() - 1]
    }

    #[test]
    fn decodes_the_captured_pause_state() {
        let body = state_body(PAUSE_STATE);
        assert_eq!(body.len(), STATE_LENGTH);
        let state = decode_state(body).expect("captured state");
        assert_eq!(state.model, ModelDriver::D1301);
        assert_eq!(state.battery.left, Some(100));
        assert_eq!(state.battery.right, Some(100));
        assert_eq!(
            state.firmware_left.map(|firmware| firmware.to_string()),
            Some("01.91".to_owned())
        );
        assert_eq!(
            state.firmware_right.map(|firmware| firmware.to_string()),
            Some("01.91".to_owned())
        );
        assert_eq!(state.listening_mode, Some(ListeningMode::Bluetooth));
        assert_eq!(state.default_listening_mode, Some(ListeningMode::Bluetooth));
        assert_eq!(state.noise_canceling, Some(false));
        assert_eq!(
            state.sleep_audio,
            Some(SleepAudio {
                enabled: true,
                action: PostSleepAudio::Pause,
            })
        );
    }

    #[test]
    fn confirmed_layout_requires_the_captured_body_length() {
        let body = state_body(PAUSE_STATE);
        assert!(ModelDriver::D1301.decode_confirmed_state(body).is_some());
        assert!(
            ModelDriver::D1301
                .decode_confirmed_state(&body[..STATE_LENGTH - 1])
                .is_none()
        );
        let mut longer = body.to_vec();
        longer.push(0);
        assert!(ModelDriver::D1301.decode_confirmed_state(&longer).is_none());
    }

    #[test]
    fn rejects_a_state_whose_serial_is_not_the_model() {
        let mut body = state_body(PAUSE_STATE).to_vec();
        body[STATE_SERIAL].copy_from_slice(b"3949TEST00000000");
        assert_eq!(decode_state(&body), Err(ModelError::UnknownModel));
    }

    #[test]
    fn decodes_and_encodes_the_auto_stop_timer_reference() {
        let body = [0, 30, 0, 1, 60, 0, 0, 0];
        let timer = AutoStopTimer::decode(&body).expect("captured timer report");
        assert_eq!(
            timer,
            AutoStopTimer {
                enabled: false,
                duration_minutes: 30,
                unknown: 1,
                time_left_seconds: 60,
            }
        );
        // The write preserves the captured unidentified byte and the duration.
        assert_eq!(
            timer.write_body(true, 45),
            Ok([1, 45, 0, 1]),
            "the legacy builder repeats the saved final byte"
        );
        assert_eq!(
            timer.write_body(false, 30),
            Ok([0, 30, 0, 1]),
            "disabling keeps the configured duration"
        );
        assert_eq!(
            timer.write_body(true, 0),
            Err(ModelError::InvalidAutoStopTimer)
        );
        assert_eq!(
            timer.write_body(true, 241),
            Err(ModelError::InvalidAutoStopTimer)
        );
    }

    #[test]
    fn alarm_records_round_trip_the_captured_list() {
        let filled: &[u8] = &[0, 0, 149, 1, 0, 3, 50, 10, 1, 1, 149, 2, 128, 0, 75, 5];
        let alarms = decode_alarms(filled).expect("captured alarm list");
        assert_eq!(alarms.len(), 2);
        assert_eq!(
            alarms[0],
            Alarm {
                id: 0,
                enabled: false,
                time: 405,
                repeat: AlarmRepeat::from_bits_truncate(0),
                wake_up_tune: AlarmWakeUpTune::Bloom,
                volume: 50,
                snooze_duration_in_minutes: 10,
            }
        );
        assert_eq!(
            alarms[1],
            Alarm {
                id: 1,
                enabled: true,
                time: 661,
                repeat: AlarmRepeat::SUNDAY,
                wake_up_tune: AlarmWakeUpTune::Nature,
                volume: 75,
                snooze_duration_in_minutes: 5,
            }
        );
        let encoded: Vec<u8> = alarms.iter().flat_map(|alarm| alarm.bytes()).collect();
        assert_eq!(encoded, filled);
        assert_eq!(decode_alarms(&[]), Ok(Vec::new()));
    }

    #[test]
    fn captured_alarm_writes_match_the_legacy_bodies() {
        // `set_alarm_matches_a_captured_packet` from the legacy builder tests.
        let created = Alarm {
            id: 1,
            enabled: true,
            time: 560,
            repeat: AlarmRepeat::SATURDAY | AlarmRepeat::SUNDAY,
            wake_up_tune: AlarmWakeUpTune::Nature,
            volume: 50,
            snooze_duration_in_minutes: 10,
        };
        assert_eq!(
            set_alarm_body(&created),
            [0x01, 0x01, 0x30, 0x02, 0x82, 0x00, 0x32, 0x0A]
        );
        // `set_alarm_updates_an_existing_alarm` from the same tests.
        let updated = Alarm {
            id: 0,
            enabled: true,
            time: 465,
            repeat: AlarmRepeat::from_bits_truncate(0),
            wake_up_tune: AlarmWakeUpTune::Glow,
            volume: 50,
            snooze_duration_in_minutes: 5,
        };
        assert_eq!(
            set_alarm_body(&updated),
            [0x00, 0x01, 0xD1, 0x01, 0x00, 0x01, 0x32, 0x05]
        );
        assert_eq!(delete_alarm_body(1), [0x01]);
    }

    #[test]
    fn rejects_an_alarm_list_that_is_not_whole_records() {
        assert_eq!(decode_alarms(&[0; 7]), Err(ModelError::InvalidAlarmList));
        assert_eq!(decode_alarms(&[0; 9]), Err(ModelError::InvalidAlarmList));
    }

    #[test]
    fn post_sleep_audio_write_keeps_the_timer_sentinels() {
        assert_eq!(
            post_sleep_audio_body(PostSleepAudio::Pause),
            [0xFF, 0xFF, 0xFF, 0x00]
        );
        assert_eq!(
            post_sleep_audio_body(PostSleepAudio::PlayLocal),
            [0xFF, 0xFF, 0xFF, 0x01]
        );
    }

    #[test]
    fn settings_map_to_the_legacy_commands_and_bodies() {
        assert_eq!(
            D1301Setting::ListeningMode(ListeningMode::Local)
                .command()
                .to_vec(),
            [0x01, 0xA9]
        );
        assert_eq!(
            D1301Setting::DefaultListeningMode(ListeningMode::Local).write_body(),
            vec![1]
        );
        assert_eq!(D1301Setting::NoiseCanceling(true).write_body(), vec![1]);
        assert_eq!(
            D1301Setting::PostSleepAudio(PostSleepAudio::PlayLocal).write_body(),
            vec![0xFF, 0xFF, 0xFF, 1]
        );
        assert_eq!(
            D1301Setting::AutoSwitchOnceAsleep(false).write_body(),
            vec![0]
        );
    }

    #[test]
    fn setting_matches_only_the_reported_value() {
        let body = state_body(PAUSE_STATE);
        let state = decode_state(body).expect("captured state");
        assert!(D1301Setting::PostSleepAudio(PostSleepAudio::Pause).matches(state));
        assert!(!D1301Setting::PostSleepAudio(PostSleepAudio::PlayLocal).matches(state));
        assert!(D1301Setting::AutoSwitchOnceAsleep(true).matches(state));
        assert!(!D1301Setting::AutoSwitchOnceAsleep(false).matches(state));
        assert!(D1301Setting::ListeningMode(ListeningMode::Bluetooth).matches(state));
        assert!(!D1301Setting::ListeningMode(ListeningMode::Local).matches(state));
        assert!(D1301Setting::NoiseCanceling(false).matches(state));
        assert_eq!(
            D1301Setting::NoiseCanceling(true).observed(state),
            Some(D1301Setting::NoiseCanceling(false))
        );
    }
}
