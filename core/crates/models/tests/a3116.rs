#![forbid(unsafe_code)]

#[cfg(test)]
mod tests {
    use openscl4_models::{
        A3116_AUTO_POWER_OFF_COMMAND, A3116_EQUALIZER_BANDS_COMMAND, A3116_EQUALIZER_PRESET,
        A3116_EQUALIZER_PRESET_COMMAND, A3116_POWER_OFF_COMMAND, A3116_VOICE_PROMPT_COMMAND,
        A3116_VOICE_PROMPT_READ_COMMAND, A3116_VOLUME_COMMAND, AutoPowerOff, AutoPowerOffSetting,
        EqualizerSetting, FirmwareVersion, ModelDriver, ModelError, STATE_COMMAND, registry,
    };
    use openscl4_protocol::{ChecksumKind, Direction, FrameDecoder};

    /// A3116 `01 01` state frames derived from the legacy driver fixture in
    /// `lib/src/devices/soundcore/a3116.rs`. The frames carry no checksum; the
    /// serial at body offsets 10–25 is replaced with the synthetic
    /// `3116TEST00000000` value. The 38.39 firmware exposes the voice prompt.
    const PILOT_FRAME: &[u8] = include_bytes!("../../../data/a3116-pilot-state.bin");
    const OLD_FIRMWARE_FRAME: &[u8] = include_bytes!("../../../data/a3116-old-firmware-state.bin");
    const VOICE_PROMPT_ON_FRAME: &[u8] = include_bytes!("../../../data/a3116-voice-prompt-on.bin");
    const VOICE_PROMPT_OFF_FRAME: &[u8] =
        include_bytes!("../../../data/a3116-voice-prompt-off.bin");

    #[test]
    fn identifies_the_a3116_from_its_state_serial() {
        let body = state_body(PILOT_FRAME);
        assert_eq!(body.len(), 36);
        assert_eq!(registry::identify_state(&body), Ok(ModelDriver::A3116));
        assert_eq!(registry::by_code("A3116"), Some(ModelDriver::A3116));
    }

    #[test]
    fn rejects_unregistered_serial_and_unmeasured_lengths() {
        let mut unknown = state_body(PILOT_FRAME);
        unknown[10..26].copy_from_slice(b"9999UNKNOWN00000");
        assert_eq!(
            registry::identify_state(&unknown),
            Err(ModelError::UnknownModel)
        );

        for length in [35, 37] {
            let mut body = state_body(PILOT_FRAME);
            body.resize(length, 0);
            assert_eq!(
                registry::identify_state(&body),
                Err(ModelError::IncompleteState)
            );
        }
    }

    #[test]
    fn decodes_the_legacy_state_and_its_checksum_less_framing() {
        assert_eq!(A3116_EQUALIZER_PRESET, 26);
        let body = state_body(PILOT_FRAME);
        let state = ModelDriver::A3116
            .decode_confirmed_state(&body)
            .expect("confirmed state");
        assert_eq!(state.model, ModelDriver::A3116);
        assert_eq!(state.battery.device, Some(100));
        assert_eq!(state.battery_charging, Some(false));
        assert_eq!(state.volume, Some(7));
        assert_eq!(
            state.firmware_device.map(|firmware| firmware.to_string()),
            Some("38.39".to_owned())
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
        assert_eq!(ModelDriver::A3116.framing(), ChecksumKind::None);
    }

    #[test]
    fn rejects_a_trailing_byte_and_a_contradictory_serial() {
        let mut trailing = state_body(PILOT_FRAME);
        trailing.push(0);
        assert!(
            ModelDriver::A3116
                .decode_confirmed_state(&trailing)
                .is_none()
        );
        assert_eq!(
            ModelDriver::A3116.decode_state(&trailing),
            Err(ModelError::IncompleteState)
        );

        let mut other_serial = state_body(PILOT_FRAME);
        other_serial[10..26].copy_from_slice(b"3028TEST00000000");
        assert!(
            ModelDriver::A3116
                .decode_confirmed_state(&other_serial)
                .is_none()
        );
        assert_eq!(
            ModelDriver::A3116.decode_state(&other_serial),
            Err(ModelError::UnknownModel)
        );
    }

    #[test]
    fn voice_prompt_reads_are_conditional_on_firmware_38_39() {
        let new = ModelDriver::A3116
            .decode_confirmed_state(&state_body(PILOT_FRAME))
            .expect("38.39 state");
        assert_eq!(
            ModelDriver::A3116.post_state_reads(&new),
            vec![A3116_VOICE_PROMPT_READ_COMMAND]
        );

        let old = ModelDriver::A3116
            .decode_confirmed_state(&state_body(OLD_FIRMWARE_FRAME))
            .expect("00.99 state");
        assert_eq!(
            old.firmware_device.map(|firmware| firmware.to_string()),
            Some("00.99".to_owned())
        );
        assert!(ModelDriver::A3116.post_state_reads(&old).is_empty());
        assert_eq!(old.voice_prompt, None);
    }

    #[test]
    fn the_targeted_read_merges_the_voice_prompt_flag() {
        for (frame, expected) in [
            (VOICE_PROMPT_ON_FRAME, true),
            (VOICE_PROMPT_OFF_FRAME, false),
        ] {
            let mut state = ModelDriver::A3116
                .decode_confirmed_state(&state_body(PILOT_FRAME))
                .expect("confirmed state");
            let body = inbound_body(frame);
            ModelDriver::A3116
                .apply_state_read(&mut state, A3116_VOICE_PROMPT_READ_COMMAND, &body)
                .expect("voice prompt read");
            assert_eq!(state.voice_prompt, Some(expected));
        }
        let mut state = ModelDriver::A3116
            .decode_confirmed_state(&state_body(PILOT_FRAME))
            .expect("confirmed state");
        assert_eq!(
            ModelDriver::A3116.apply_state_read(&mut state, A3116_VOICE_PROMPT_READ_COMMAND, &[]),
            Err(ModelError::IncompleteState)
        );
        // Firmware below 38.39 never reports a readable flag, so writes are
        // rejected before the model can encode one.
        let old = ModelDriver::A3116
            .decode_confirmed_state(&state_body(OLD_FIRMWARE_FRAME))
            .expect("old state");
        assert_eq!(
            old.voice_prompt_write(true),
            Err(ModelError::InvalidVoicePromptWrite)
        );
    }

    #[test]
    fn writes_the_legacy_volume_and_rejects_out_of_range_values() {
        let state = ModelDriver::A3116
            .decode_confirmed_state(&state_body(PILOT_FRAME))
            .expect("confirmed state");
        assert_eq!(
            state.volume_write(0).expect("minimum volume"),
            (A3116_VOLUME_COMMAND, vec![0])
        );
        assert_eq!(
            state.volume_write(16).expect("maximum volume"),
            (A3116_VOLUME_COMMAND, vec![16])
        );
        for volume in [17, u8::MAX] {
            assert_eq!(
                state.volume_write(volume),
                Err(ModelError::InvalidVolumeWrite),
                "volume {volume}"
            );
        }
    }

    #[test]
    fn writes_auto_power_off_as_a_one_based_duration_without_a_disabled_value() {
        let state = ModelDriver::A3116
            .decode_confirmed_state(&state_body(PILOT_FRAME))
            .expect("confirmed state");
        let write = state
            .auto_power_off_write(AutoPowerOffSetting::Enabled(0))
            .expect("five minutes");
        assert_eq!(write.command(), A3116_AUTO_POWER_OFF_COMMAND);
        assert_eq!(write.body(), [1]);
        assert_eq!(
            write.expected(),
            AutoPowerOff {
                enabled: true,
                duration: 0,
            }
        );
        let write = state
            .auto_power_off_write(AutoPowerOffSetting::Enabled(3))
            .expect("sixty minutes");
        assert_eq!(write.body(), [4]);
        for rejected in [
            AutoPowerOffSetting::Enabled(4),
            AutoPowerOffSetting::Disabled,
        ] {
            assert_eq!(
                state.auto_power_off_write(rejected),
                Err(ModelError::InvalidAutoPowerOffWrite)
            );
        }
    }

    #[test]
    fn writes_voice_prompt_and_power_off_only_when_available() {
        let mut state = ModelDriver::A3116
            .decode_confirmed_state(&state_body(PILOT_FRAME))
            .expect("confirmed state");
        assert_eq!(
            state.voice_prompt_write(true),
            Err(ModelError::InvalidVoicePromptWrite)
        );
        state.voice_prompt = Some(true);
        assert_eq!(
            state.voice_prompt_write(false).expect("voice prompt off"),
            (A3116_VOICE_PROMPT_COMMAND, vec![0])
        );
        assert_eq!(
            state.power_off_request().expect("power off action"),
            (A3116_POWER_OFF_COMMAND, Vec::new())
        );

        let other = ModelDriver::A3062
            .decode_confirmed_state(&include_bytes!("../../../data/a3062-pilot-state.bin")[9..104])
            .expect("A3062 state");
        assert_eq!(other.volume_write(5), Err(ModelError::InvalidVolumeWrite));
        assert_eq!(
            other.power_off_request(),
            Err(ModelError::InvalidPowerOffAction)
        );
    }

    #[test]
    fn writes_the_two_frame_equalizer_in_the_legacy_order() {
        let body = state_body(PILOT_FRAME);
        let state = ModelDriver::A3116
            .decode_confirmed_state(&body)
            .expect("confirmed state");
        let requested = EqualizerSetting {
            preset_id: 15,
            volume_adjustments: [-60, -50, -40, -30, -20, -10, 0, 10, 20, 0],
        };
        assert_eq!(
            state
                .model
                .equalizer_write_requests(requested, &body)
                .expect("two-frame write"),
            vec![
                (A3116_EQUALIZER_PRESET_COMMAND, vec![15]),
                (
                    A3116_EQUALIZER_BANDS_COMMAND,
                    vec![0, 1, 2, 3, 4, 5, 6, 7, 8]
                ),
            ]
        );

        // The preset already matches the state, so only the curve is sent.
        let current = state.equalizer.expect("legacy curve");
        let curve_only = EqualizerSetting {
            volume_adjustments: [-60, -50, -40, -30, -20, -10, 0, 10, 20, 0],
            ..current
        };
        assert_eq!(
            state
                .model
                .equalizer_write_requests(curve_only, &body)
                .expect("curve write"),
            vec![(
                A3116_EQUALIZER_BANDS_COMMAND,
                vec![0, 1, 2, 3, 4, 5, 6, 7, 8]
            )]
        );

        // An unchanged curve and preset send no frame; the session confirms it
        // against the fresh state read instead.
        assert_eq!(
            state
                .model
                .equalizer_write_requests(current, &body)
                .expect("unchanged curve"),
            Vec::new()
        );
    }

    #[test]
    fn rejects_curves_the_nine_band_model_cannot_carry() {
        let body = state_body(PILOT_FRAME);
        let state = ModelDriver::A3116
            .decode_confirmed_state(&body)
            .expect("confirmed state");
        let current = state.equalizer.expect("legacy curve");
        assert_eq!(state.model.equalizer_bands(), 9);
        assert!(state.model.equalizer_is_writable(current));

        for gain in [-70, -61, 1, 61, 70, i16::MIN, i16::MAX] {
            let mut setting = current;
            setting.volume_adjustments[0] = gain;
            assert_eq!(
                state.model.equalizer_write_requests(setting, &body),
                Err(ModelError::InvalidEqualizerWrite),
                "gain {gain}"
            );
        }

        let mut tenth_band = current;
        tenth_band.volume_adjustments[9] = 0;
        assert!(state.model.equalizer_is_writable(tenth_band));
        tenth_band.volume_adjustments[9] = 10;
        assert!(!state.model.equalizer_is_writable(tenth_band));

        let mut two_byte_preset = current;
        two_byte_preset.preset_id = 0x0100;
        assert!(!state.model.equalizer_is_writable(two_byte_preset));
    }

    #[test]
    fn the_other_measured_models_keep_their_framing_and_operations() {
        assert_eq!(ModelDriver::A3116.framing(), ChecksumKind::None);
        for model in [
            ModelDriver::A3028,
            ModelDriver::A3029,
            ModelDriver::A3062,
            ModelDriver::A3949,
            ModelDriver::A3954,
        ] {
            assert_eq!(model.framing(), ChecksumKind::Suffix);
            assert!(!model.capabilities().volume_write);
            assert!(!model.capabilities().power_off);
            assert!(!model.capabilities().voice_prompt_write);
        }
        assert!(ModelDriver::A3028.capabilities().auto_power_off_write);
        assert!(!ModelDriver::A3062.capabilities().auto_power_off_write);
    }

    #[test]
    fn the_fixture_frames_are_well_formed() {
        for frame in [
            PILOT_FRAME,
            OLD_FIRMWARE_FRAME,
            VOICE_PROMPT_ON_FRAME,
            VOICE_PROMPT_OFF_FRAME,
        ] {
            let mut decoder = FrameDecoder::new(ChecksumKind::None);
            decoder.push(frame);
            let parsed = decoder
                .next_frame()
                .expect("checksum-less frame")
                .expect("complete frame");
            assert_eq!(parsed.direction, Direction::Inbound);
            assert!(!decoder.has_pending_bytes());
        }
        let state = state_body(PILOT_FRAME);
        assert_eq!(state.len(), 36);
        assert_eq!(
            FirmwareVersion::new(38, 39),
            ModelDriver::A3116
                .decode_confirmed_state(&state)
                .expect("confirmed state")
                .firmware_device
                .expect("reported firmware")
        );
    }

    fn state_body(frame: &[u8]) -> Vec<u8> {
        let mut decoder = FrameDecoder::new(ChecksumKind::None);
        decoder.push(frame);
        let parsed = decoder
            .next_frame()
            .expect("checksum-less state frame")
            .expect("complete state frame");
        assert_eq!(parsed.command, STATE_COMMAND);
        parsed.body.to_vec()
    }

    fn inbound_body(frame: &[u8]) -> Vec<u8> {
        let mut decoder = FrameDecoder::new(ChecksumKind::None);
        decoder.push(frame);
        let parsed = decoder
            .next_frame()
            .expect("checksum-less frame")
            .expect("complete frame");
        assert_eq!(parsed.command, A3116_VOICE_PROMPT_READ_COMMAND);
        parsed.body.to_vec()
    }
}
