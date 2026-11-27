#![forbid(unsafe_code)]

#[cfg(test)]
mod tests {
    use openscl4_models::{
        A3031_AUTO_POWER_OFF, A3031_EQUALIZER_CHANNEL_1, A3031_EQUALIZER_CHANNEL_2,
        A3031_EQUALIZER_PRESET, A3031_SOUND_MODE_FIELDS, A3031_TWS_CONNECTED, AutoPowerOff,
        AutoPowerOffSetting, EqualizerSetting, ModelDriver, ModelError, SoundMode,
        identity_firmware, registry,
    };

    /// Inbound frames built from the legacy A3031 layouts. The state carries no
    /// serial by design; the separate `01 05` identity frame carries the
    /// synthetic `A3031TEST0000000` serial and the dual firmware versions.
    const STATE_FRAME: &[u8] = include_bytes!("../../../data/a3031-state.bin");
    const IDENTITY_FRAME: &[u8] = include_bytes!("../../../data/a3031-identity.bin");
    const REFERENCE_CURVE: EqualizerSetting = EqualizerSetting {
        preset_id: 0xfefe,
        volume_adjustments: [-60, 60, 23, 40, 22, 60, -4, 16, 0, 0],
    };

    #[test]
    fn identifies_the_model_from_the_separate_identity_response() {
        let identity = identity_body(IDENTITY_FRAME);
        assert_eq!(identity.len(), 26);
        assert_eq!(ModelDriver::identify(identity), Ok(ModelDriver::A3031));
        assert_eq!(registry::by_code("A3031"), Some(ModelDriver::A3031));

        let firmware = identity_firmware(identity).expect("dual identity firmware");
        assert_eq!(
            firmware.left.map(|version| version.to_string()).as_deref(),
            Some("01.10")
        );
        assert_eq!(
            firmware.right.map(|version| version.to_string()).as_deref(),
            Some("01.20")
        );
    }

    #[test]
    fn state_only_discovery_cannot_select_the_model() {
        // The state has no serial: only the separate identity response can
        // select A3031.
        assert_eq!(
            registry::identify_state(body(STATE_FRAME)),
            Err(ModelError::IncompleteState)
        );
    }

    #[test]
    fn keeps_the_public_code_in_the_confirmed_state() {
        let state = ModelDriver::A3031
            .decode_confirmed_state(body(STATE_FRAME))
            .expect("confirmed state");
        assert_eq!(state.model, ModelDriver::A3031);
        assert_eq!(state.battery.left, Some(100));
        assert_eq!(state.battery.right, Some(80));
        assert_eq!(state.battery.device, None);
        assert_eq!(state.battery.case, None);
        assert_eq!(state.tws_connected, Some(true));
        assert_eq!(state.sound_mode, Some(SoundMode::Normal));
        assert_eq!(state.equalizer, Some(REFERENCE_CURVE));
        assert_eq!(
            state.auto_power_off,
            Some(AutoPowerOff {
                enabled: true,
                duration: 1,
            })
        );
        assert_eq!(state.wearing_detection, None);
        assert_eq!(state.firmware_device, None);
        assert_eq!(state.firmware_left, None);
        assert_eq!(state.firmware_right, None);
    }

    #[test]
    fn admits_only_the_measured_state_length() {
        let original = body(STATE_FRAME).to_vec();
        for length in [43, 45, 61] {
            let mut body = original.clone();
            body.resize(length, 0);
            assert!(
                ModelDriver::A3031.decode_confirmed_state(&body).is_none(),
                "{length}-byte body was admitted"
            );
        }
        assert_eq!(
            ModelDriver::A3031.decode_state(&[0; 43]),
            Err(ModelError::IncompleteState)
        );
    }

    #[test]
    fn rejects_an_unrecognized_tws_status() {
        let mut body = body(STATE_FRAME).to_vec();
        body[0] = 2;
        assert_eq!(
            ModelDriver::A3031.decode_state(&body),
            Err(ModelError::InvalidTwsStatus)
        );
        assert!(ModelDriver::A3031.decode_confirmed_state(&body).is_none());
    }

    #[test]
    fn equalizer_is_available_only_while_tws_is_connected() {
        let mut disconnected = body(STATE_FRAME).to_vec();
        disconnected[A3031_TWS_CONNECTED] = 0;
        let state = ModelDriver::A3031
            .decode_confirmed_state(&disconnected)
            .expect("the rest of the state stays readable");
        assert_eq!(state.tws_connected, Some(false));
        assert_eq!(state.equalizer, None);
        assert_eq!(
            ModelDriver::A3031.equalizer_write_body(REFERENCE_CURVE, &disconnected),
            Err(ModelError::InvalidEqualizerWrite),
            "the legacy setting is unavailable while TWS is disconnected"
        );
        assert!(ModelDriver::A3031.capabilities().equalizer_write);
    }

    #[test]
    fn writes_the_two_channel_curve() {
        let state = body(STATE_FRAME);
        assert_eq!(
            ModelDriver::A3031.equalizer_write_body(REFERENCE_CURVE, state),
            Ok(vec![
                0xfe, 0xfe, 0x3c, 0xb4, 0x8f, 0xa0, 0x8e, 0xb4, 0x74, 0x88, 0x3c, 0xb4, 0x8f, 0xa0,
                0x8e, 0xb4, 0x74, 0x88,
            ])
        );
        let signature = EqualizerSetting {
            preset_id: 0,
            volume_adjustments: [0; 10],
        };
        assert_eq!(
            ModelDriver::A3031.equalizer_write_body(signature, state),
            Ok(vec![
                0x00, 0x00, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78,
                0x78, 0x78, 0x78, 0x78,
            ])
        );
        let mut out_of_range = signature;
        out_of_range.volume_adjustments[8] = 1;
        assert_eq!(
            ModelDriver::A3031.equalizer_write_body(out_of_range, state),
            Err(ModelError::InvalidEqualizerWrite)
        );
        assert_eq!(ModelDriver::A3031.equalizer_bands(), 8);
        assert_eq!(ModelDriver::A3031.equalizer_command(), [0x02, 0x81]);
    }

    #[test]
    fn writes_the_shared_four_byte_sound_modes() {
        let state = ModelDriver::A3031
            .decode_confirmed_state(body(STATE_FRAME))
            .expect("confirmed state");
        assert_eq!(
            state.sound_modes_write(SoundMode::NoiseCanceling),
            Ok([0x00, 0x00, 0x01, 0x00])
        );
        assert_eq!(
            state.sound_modes_write(SoundMode::Transparency),
            Ok([0x01, 0x00, 0x01, 0x00])
        );
        assert_eq!(
            state.sound_modes_write(SoundMode::Normal),
            Ok([0x02, 0x00, 0x01, 0x00])
        );
        assert_eq!(
            state.sound_modes_write(SoundMode::AirplaneMode),
            Err(ModelError::InvalidSoundModeWrite)
        );
    }

    #[test]
    fn writes_auto_power_off_from_the_measured_state_fields() {
        let state = ModelDriver::A3031
            .decode_confirmed_state(body(STATE_FRAME))
            .expect("confirmed state");
        let write = state
            .auto_power_off_write(AutoPowerOffSetting::Disabled)
            .expect("supported disable");
        assert_eq!(write.body(), [0, 1]);
        assert_eq!(
            write.expected(),
            AutoPowerOff {
                enabled: false,
                duration: 1,
            }
        );

        let write = state
            .auto_power_off_write(AutoPowerOffSetting::Enabled(3))
            .expect("last supported duration");
        assert_eq!(write.body(), [1, 3]);

        for index in [4, u8::MAX] {
            assert_eq!(
                state.auto_power_off_write(AutoPowerOffSetting::Enabled(index)),
                Err(ModelError::InvalidAutoPowerOffWrite)
            );
        }
        assert!(!ModelDriver::A3031.capabilities().wearing_detection_write);
        assert!(!ModelDriver::A3031.capabilities().gaming_mode_write);
        assert!(
            !ModelDriver::A3031
                .capabilities()
                .positioned_sound_mode_write
        );
    }

    #[test]
    fn layout_offsets_match_the_legacy_state_packet() {
        assert_eq!(A3031_TWS_CONNECTED, 1);
        assert_eq!(A3031_SOUND_MODE_FIELDS, 36..40);
        assert_eq!(A3031_EQUALIZER_PRESET, 6..8);
        assert_eq!(A3031_EQUALIZER_CHANNEL_1, 8..16);
        assert_eq!(A3031_EQUALIZER_CHANNEL_2, 16..24);
        assert_eq!(A3031_AUTO_POWER_OFF, 42..44);
        let body = body(STATE_FRAME);
        assert_eq!(body[36], 0x02);
        assert_eq!(body[42..44], [0x01, 0x01]);
    }

    fn body(frame: &[u8]) -> &[u8] {
        assert_eq!(frame[5..7], [0x01, 0x01]);
        &frame[9..frame.len() - 1]
    }

    fn identity_body(frame: &[u8]) -> &[u8] {
        assert_eq!(frame[5..7], [0x01, 0x05]);
        &frame[9..frame.len() - 1]
    }
}
