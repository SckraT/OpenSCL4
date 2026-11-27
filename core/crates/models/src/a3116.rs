use std::ops::Range;

use crate::{
    AutoPowerOff, AutoPowerOffSetting, AutoPowerOffWrite, BatteryLevels, DeviceState,
    EqualizerSetting, FirmwareVersion, ModelDriver, ModelError, RedactedSerial, WireRequests,
    battery_percent,
};

/// Body length of the A3116 state response.
///
/// The legacy parser consumes the body completely, so the confirmed layout is
/// exactly this length. The frame carries no trailing checksum.
pub const STATE_LENGTH: usize = 36;

const BATTERY_CHARGING: usize = 0;
const BATTERY_LEVEL: usize = 1;
const VOLUME: usize = 2;
// Body offset 3 is an unknown byte in the legacy parser.
const AUTO_POWER_OFF: usize = 4;
const FIRMWARE: Range<usize> = 5..10;
const SERIAL: Range<usize> = 10..26;

/// Body offset of the volume value in the A3116 state.
pub const A3116_VOLUME_FIELD: usize = VOLUME;
/// Body offset of the auto power off duration byte in the A3116 state.
pub const A3116_AUTO_POWER_OFF_FIELD: usize = AUTO_POWER_OFF;

/// Body offset of the equalizer preset id in the A3116 state.
pub const A3116_EQUALIZER_PRESET: usize = 26;
/// Body offsets of the nine equalizer band gains in the A3116 state.
pub const A3116_EQUALIZER_BANDS: Range<usize> = 27..36;
/// Number of equalizer bands the A3116 curve carries.
pub const EQUALIZER_BAND_COUNT: usize = 9;
/// Highest volume value accepted by the legacy A3116 volume setting.
pub const VOLUME_MAX: u8 = 16;
/// The underlying battery scale, as registered by the legacy driver.
const BATTERY_SCALE: u8 = 5;
/// Wire range of one A3116 equalizer band: integer decibels from -6 to 6.
const EQUALIZER_MIN_DECIBELS: i16 = -6;
const EQUALIZER_MAX_DECIBELS: i16 = 6;
/// Number of entries in the A3116 auto power off table.
pub const AUTO_POWER_OFF_DURATION_COUNT: u8 = 4;

/// The A3116 volume write command.
pub const A3116_VOLUME_COMMAND: [u8; 2] = [0x01, 0x81];
/// The A3116 equalizer preset write command.
pub const A3116_EQUALIZER_PRESET_COMMAND: [u8; 2] = [0x02, 0x81];
/// The A3116 equalizer band write command.
pub const A3116_EQUALIZER_BANDS_COMMAND: [u8; 2] = [0x02, 0x83];
/// The A3116 auto power off write command.
pub const A3116_AUTO_POWER_OFF_COMMAND: [u8; 2] = [0x01, 0x83];
/// The A3116 power off action.
pub const A3116_POWER_OFF_COMMAND: [u8; 2] = [0x01, 0x85];
/// The A3116 voice prompt write command.
pub const A3116_VOICE_PROMPT_COMMAND: [u8; 2] = [0x01, 0x90];
/// The targeted A3116 voice prompt read command.
pub const A3116_VOICE_PROMPT_READ_COMMAND: [u8; 2] = [0x01, 0x10];

/// The first firmware version that exposes the voice prompt flag.
pub const VOICE_PROMPT_FIRMWARE: FirmwareVersion = FirmwareVersion::new(38, 39);

/// Reads the model-specific serial from the measured state layout.
pub fn state_serial(body: &[u8]) -> Result<RedactedSerial<'_>, ModelError> {
    if body.len() < STATE_LENGTH {
        return Err(ModelError::IncompleteState);
    }
    RedactedSerial::parse(&body[SERIAL])
}

/// Whether the state response has the confirmed A3116 layout.
pub fn has_confirmed_layout(body: &[u8]) -> bool {
    body.len() == STATE_LENGTH
}

pub fn decode_state(model: ModelDriver, body: &[u8]) -> Result<DeviceState, ModelError> {
    if !has_confirmed_layout(body) {
        return Err(ModelError::IncompleteState);
    }
    if !state_serial(body)?.identifies(model) {
        return Err(ModelError::UnknownModel);
    }
    Ok(DeviceState {
        model,
        battery: BatteryLevels {
            device: battery_percent(body[BATTERY_LEVEL], BATTERY_SCALE, 0)?,
            left: None,
            right: None,
            case: None,
        },
        // The A3116 reports charging before the level. The legacy parser maps
        // every value other than one to "not charging".
        battery_charging: Some(body[BATTERY_CHARGING] == 1),
        volume: Some(body[VOLUME]),
        sound_mode: None,
        sound_mode_controls: None,
        equalizer: equalizer_setting(body),
        gaming_mode: None,
        auto_power_off: auto_power_off(body),
        voice_prompt: None,
        firmware_device: FirmwareVersion::parse(&body[FIRMWARE])?,
        firmware_left: None,
        firmware_right: None,
    })
}

/// Reads the nine-band curve. A band block that starts with the absent marker
/// reports no curve; a band outside the wire range is not a confirmed value.
fn equalizer_setting(body: &[u8]) -> Option<EqualizerSetting> {
    let preset_id = body.get(A3116_EQUALIZER_PRESET).copied()?;
    let bands: [u8; EQUALIZER_BAND_COUNT] = body.get(A3116_EQUALIZER_BANDS)?.try_into().ok()?;
    if bands[0] == u8::MAX {
        return None;
    }
    // The shared curve is ten bands; the last one is not part of this model.
    let mut volume_adjustments = [0; crate::EQUALIZER_BANDS];
    for (gain, byte) in volume_adjustments.iter_mut().zip(bands) {
        *gain = band_gain(byte)?;
    }
    Some(EqualizerSetting {
        preset_id: u16::from(preset_id),
        volume_adjustments,
    })
}

/// Decodes one band byte into the shared tenths-of-a-decibel scale.
fn band_gain(byte: u8) -> Option<i16> {
    let decibels = i16::from(byte) + EQUALIZER_MIN_DECIBELS;
    (EQUALIZER_MIN_DECIBELS..=EQUALIZER_MAX_DECIBELS)
        .contains(&decibels)
        .then_some(decibels * 10)
}

/// Reads the auto power off duration index. The legacy enum starts at five
/// minutes, so wire value one is index zero. An unrecognized value is not
/// reported as a confirmed duration.
fn auto_power_off(body: &[u8]) -> Option<AutoPowerOff> {
    let wire = *body.get(AUTO_POWER_OFF)?;
    let duration = wire.checked_sub(1)?;
    (duration < AUTO_POWER_OFF_DURATION_COUNT).then_some(AutoPowerOff {
        enabled: true,
        duration,
    })
}

/// Whether the firmware exposes the targeted voice prompt read.
pub fn has_voice_prompt(firmware: Option<FirmwareVersion>) -> bool {
    firmware.is_some_and(|firmware| firmware >= VOICE_PROMPT_FIRMWARE)
}

/// Extra state reads the A3116 needs after its bulk state response.
pub fn post_state_reads(state: &DeviceState) -> Vec<[u8; 2]> {
    if has_voice_prompt(state.firmware_device) {
        vec![A3116_VOICE_PROMPT_READ_COMMAND]
    } else {
        Vec::new()
    }
}

/// Applies a targeted voice prompt response to the state.
pub fn apply_state_read(
    state: &mut DeviceState,
    command: [u8; 2],
    body: &[u8],
) -> Result<(), ModelError> {
    if command != A3116_VOICE_PROMPT_READ_COMMAND || body.len() != 1 {
        return Err(ModelError::IncompleteState);
    }
    // The legacy flag parser treats every value other than one as disabled.
    state.voice_prompt = Some(body[0] == 1);
    Ok(())
}

/// Whether the shared curve can be written as the A3116 nine-band, whole-dB
/// curve with its one-byte preset id.
pub fn equalizer_is_writable(setting: EqualizerSetting) -> bool {
    setting.preset_id <= u16::from(u8::MAX)
        && setting.volume_adjustments[EQUALIZER_BAND_COUNT..]
            .iter()
            .all(|gain| *gain == 0)
        && setting.volume_adjustments[..EQUALIZER_BAND_COUNT]
            .iter()
            .all(|gain| band_byte(*gain).is_some())
}

/// Encodes one whole-dB band gain as the wire byte.
fn band_byte(gain: i16) -> Option<u8> {
    if gain % 10 != 0 {
        return None;
    }
    let decibels = gain / 10;
    if !(EQUALIZER_MIN_DECIBELS..=EQUALIZER_MAX_DECIBELS).contains(&decibels) {
        return None;
    }
    u8::try_from(decibels - EQUALIZER_MIN_DECIBELS).ok()
}

/// Builds the equalizer write requests for a fresh state response.
///
/// The legacy state modifier sends the preset only when it changes and the
/// curve only when it changes, so an unchanged field sends no frame.
pub fn equalizer_write_requests(
    setting: EqualizerSetting,
    state: &[u8],
) -> Result<WireRequests, ModelError> {
    if !equalizer_is_writable(setting) {
        return Err(ModelError::InvalidEqualizerWrite);
    }
    let current = equalizer_setting(state);
    let mut requests = Vec::new();
    let preset_changed = current
        .as_ref()
        .is_none_or(|current| current.preset_id != setting.preset_id);
    if preset_changed {
        requests.push((
            A3116_EQUALIZER_PRESET_COMMAND,
            vec![setting.preset_id as u8],
        ));
    }
    let curve_changed = current
        .as_ref()
        .is_none_or(|current| current.volume_adjustments != setting.volume_adjustments);
    if curve_changed {
        let body = setting.volume_adjustments[..EQUALIZER_BAND_COUNT]
            .iter()
            .map(|gain| band_byte(*gain).expect("gains were validated"))
            .collect();
        requests.push((A3116_EQUALIZER_BANDS_COMMAND, body));
    }
    Ok(requests)
}

/// Builds the volume write body. The legacy setting clamps to the registered
/// maximum; the session validates the requested value instead of clamping.
pub fn volume_write_body(volume: u8) -> Result<Vec<u8>, ModelError> {
    if volume > VOLUME_MAX {
        return Err(ModelError::InvalidVolumeWrite);
    }
    Ok(vec![volume])
}

/// Builds the voice prompt write body.
pub fn voice_prompt_write_body(enabled: bool) -> Vec<u8> {
    vec![u8::from(enabled)]
}

/// Builds the auto power off write. The A3116 has no "disabled" value: the
/// legacy setting only selects one of the four durations.
pub fn auto_power_off_write(setting: AutoPowerOffSetting) -> Result<AutoPowerOffWrite, ModelError> {
    match setting {
        AutoPowerOffSetting::Enabled(index) if index < AUTO_POWER_OFF_DURATION_COUNT => {
            Ok(AutoPowerOffWrite::new(
                A3116_AUTO_POWER_OFF_COMMAND,
                vec![index + 1],
                AutoPowerOff {
                    enabled: true,
                    duration: index,
                },
            ))
        }
        AutoPowerOffSetting::Enabled(_) | AutoPowerOffSetting::Disabled => {
            Err(ModelError::InvalidAutoPowerOffWrite)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The legacy `manually_crafted_packet_matches_soundcore_app` body with its
    /// synthetic serial replaced by a model-prefixed value.
    const STATE: &[u8] = &[
        0, 5, 7, 0, 2, b'9', b'9', b'.', b'9', b'9', b'3', b'1', b'1', b'6', b'T', b'E', b'S',
        b'T', b'0', b'0', b'0', b'0', b'0', b'0', b'0', b'0', 255, 6, 7, 8, 9, 10, 6, 5, 4, 3,
    ];

    #[test]
    fn decodes_the_legacy_state_body() {
        let state = decode_state(ModelDriver::A3116, STATE).expect("legacy state");
        assert_eq!(state.model, ModelDriver::A3116);
        assert_eq!(state.battery.device, Some(100));
        assert_eq!(state.battery_charging, Some(false));
        assert_eq!(state.volume, Some(7));
        assert_eq!(
            state.firmware_device.map(|firmware| firmware.to_string()),
            Some("99.99".to_owned())
        );
        assert_eq!(
            state.equalizer,
            Some(EqualizerSetting {
                preset_id: 255,
                volume_adjustments: [0, 10, 20, 30, 40, 0, -10, -20, -30, 0],
            })
        );
        assert_eq!(
            state.auto_power_off,
            Some(AutoPowerOff {
                enabled: true,
                duration: 1,
            })
        );
        assert_eq!(state.voice_prompt, None);
    }

    #[test]
    fn reads_the_voice_prompt_only_from_the_targeted_read() {
        let mut state = decode_state(ModelDriver::A3116, STATE).expect("legacy state");
        assert_eq!(state.voice_prompt, None);
        apply_state_read(&mut state, A3116_VOICE_PROMPT_READ_COMMAND, &[1])
            .expect("voice prompt on");
        assert_eq!(state.voice_prompt, Some(true));
        apply_state_read(&mut state, A3116_VOICE_PROMPT_READ_COMMAND, &[0])
            .expect("voice prompt off");
        assert_eq!(state.voice_prompt, Some(false));
        assert_eq!(
            apply_state_read(&mut state, A3116_VOICE_PROMPT_READ_COMMAND, &[]),
            Err(ModelError::IncompleteState)
        );
    }

    #[test]
    fn voice_prompt_is_conditional_on_firmware_38_39() {
        assert!(!has_voice_prompt(None));
        assert!(!has_voice_prompt(Some(FirmwareVersion::new(38, 38))));
        assert!(has_voice_prompt(Some(FirmwareVersion::new(38, 39))));
        assert!(has_voice_prompt(Some(FirmwareVersion::new(39, 0))));
    }

    #[test]
    fn writes_the_legacy_volume_and_power_off_bodies() {
        assert_eq!(volume_write_body(0), Ok(vec![0]));
        assert_eq!(volume_write_body(16), Ok(vec![16]));
        assert_eq!(volume_write_body(17), Err(ModelError::InvalidVolumeWrite));
        assert_eq!(voice_prompt_write_body(true), vec![1]);
        assert_eq!(voice_prompt_write_body(false), vec![0]);
    }

    #[test]
    fn writes_auto_power_off_as_one_based_duration() {
        let write = auto_power_off_write(AutoPowerOffSetting::Enabled(0)).expect("five minutes");
        assert_eq!(write.command(), A3116_AUTO_POWER_OFF_COMMAND);
        assert_eq!(write.body(), [1]);
        assert_eq!(
            write.expected(),
            AutoPowerOff {
                enabled: true,
                duration: 0,
            }
        );
        let write = auto_power_off_write(AutoPowerOffSetting::Enabled(3)).expect("sixty minutes");
        assert_eq!(write.body(), [4]);
        for rejected in [
            AutoPowerOffSetting::Enabled(4),
            AutoPowerOffSetting::Disabled,
        ] {
            assert_eq!(
                auto_power_off_write(rejected),
                Err(ModelError::InvalidAutoPowerOffWrite)
            );
        }
    }

    #[test]
    fn equalizer_write_sends_only_the_changed_frames() {
        let state = STATE;
        let current = equalizer_setting(state).expect("legacy curve");
        let curve_only = EqualizerSetting {
            volume_adjustments: [-60, -50, -40, -30, -20, -10, 0, 10, 20, 0],
            ..current
        };
        assert_eq!(
            equalizer_write_requests(curve_only, state).expect("curve write"),
            vec![(
                A3116_EQUALIZER_BANDS_COMMAND,
                vec![0, 1, 2, 3, 4, 5, 6, 7, 8]
            )]
        );

        let preset_only = EqualizerSetting {
            preset_id: 15,
            ..current
        };
        assert_eq!(
            equalizer_write_requests(preset_only, state).expect("preset write"),
            vec![(A3116_EQUALIZER_PRESET_COMMAND, vec![15])]
        );

        assert_eq!(
            equalizer_write_requests(current, state).expect("unchanged curve"),
            Vec::new()
        );
    }

    #[test]
    fn equalizer_write_rejects_gains_the_model_cannot_carry() {
        let current = equalizer_setting(STATE).expect("legacy curve");
        for gain in [-70, -61, 1, 61, 70, i16::MIN, i16::MAX] {
            let mut setting = current;
            setting.volume_adjustments[0] = gain;
            assert_eq!(
                equalizer_write_requests(setting, STATE),
                Err(ModelError::InvalidEqualizerWrite),
                "gain {gain}"
            );
        }
        let mut tenth_band = current;
        tenth_band.volume_adjustments[9] = 10;
        assert_eq!(
            equalizer_write_requests(tenth_band, STATE),
            Err(ModelError::InvalidEqualizerWrite)
        );
        let mut two_byte_preset = current;
        two_byte_preset.preset_id = 0x0100;
        assert_eq!(
            equalizer_write_requests(two_byte_preset, STATE),
            Err(ModelError::InvalidEqualizerWrite)
        );
    }

    #[test]
    fn absent_curve_or_out_of_range_band_is_not_confirmed() {
        let mut absent = STATE.to_vec();
        absent[A3116_EQUALIZER_BANDS.start] = u8::MAX;
        assert_eq!(
            decode_state(ModelDriver::A3116, &absent).unwrap().equalizer,
            None
        );

        let mut out_of_range = STATE.to_vec();
        out_of_range[A3116_EQUALIZER_BANDS.start] = 13;
        assert_eq!(
            decode_state(ModelDriver::A3116, &out_of_range)
                .unwrap()
                .equalizer,
            None
        );
    }

    #[test]
    fn rejects_a_state_whose_serial_names_another_model() {
        let mut body = STATE.to_vec();
        body[SERIAL].copy_from_slice(b"3028TEST00000000");
        assert_eq!(
            decode_state(ModelDriver::A3116, &body),
            Err(ModelError::UnknownModel)
        );
    }

    #[test]
    fn rejects_a_body_with_the_wrong_length() {
        for length in [STATE.len() - 1, STATE.len() + 1] {
            let mut body = STATE.to_vec();
            body.resize(length, 0);
            assert_eq!(
                decode_state(ModelDriver::A3116, &body),
                Err(ModelError::IncompleteState)
            );
        }
    }
}
