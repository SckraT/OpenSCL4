#![forbid(unsafe_code)]

#[cfg(test)]
mod tests {
    use openscl4_models::{EqualizerSetting, ModelDriver, ModelError, SoundMode, registry};

    /// Inbound `01 01` frames built from the legacy A3027 state layout in
    /// `lib/src/devices/soundcore/a3027/packets/state_update_packet.rs`. The
    /// serial at body offsets 44–59 is synthetic: `3027TEST00000000` and
    /// `3030TEST00000000`. No physical capture exists for the pair.
    const A3027_FRAME: &[u8] = include_bytes!("../../../data/a3027-state.bin");
    const A3030_FRAME: &[u8] = include_bytes!("../../../data/a3030-state.bin");
    const REFERENCE_CURVE: EqualizerSetting = EqualizerSetting {
        preset_id: 0xfefe,
        volume_adjustments: [-60, 60, 23, 40, 22, 60, -4, 16, 0, 0],
    };
    const SIGNATURE_CURVE: EqualizerSetting = EqualizerSetting {
        preset_id: 0,
        volume_adjustments: [0; 10],
    };

    #[test]
    fn identifies_both_public_codes_from_the_measured_state() {
        let a3027 = body(A3027_FRAME);
        let a3030 = body(A3030_FRAME);
        assert_eq!(a3027.len(), 61);
        assert_eq!(registry::identify_state(a3027), Ok(ModelDriver::A3027));
        assert_eq!(registry::identify_state(a3030), Ok(ModelDriver::A3030));
        assert_eq!(registry::by_code("A3027"), Some(ModelDriver::A3027));
        assert_eq!(registry::by_code("A3030"), Some(ModelDriver::A3030));
    }

    #[test]
    fn keeps_the_public_code_in_the_confirmed_state() {
        for (frame, model, serial) in [
            (A3027_FRAME, ModelDriver::A3027, "3027TEST00000000"),
            (A3030_FRAME, ModelDriver::A3030, "3030TEST00000000"),
        ] {
            let state = model
                .decode_confirmed_state(body(frame))
                .expect("confirmed state");
            assert_eq!(state.model, model);
            assert_eq!(state.battery.device, Some(100));
            assert_eq!(state.sound_mode, Some(SoundMode::Normal));
            assert_eq!(state.equalizer, Some(REFERENCE_CURVE));
            assert_eq!(state.wearing_detection, Some(true));
            assert_eq!(state.auto_power_off, None);
            assert_eq!(state.tws_connected, None);
            assert_eq!(
                state.firmware_device.map(|firmware| firmware.to_string()),
                Some("01.20".to_owned())
            );
            assert_eq!(&body(frame)[44..60], serial.as_bytes());
        }
    }

    #[test]
    fn rejects_a_state_whose_serial_names_the_other_public_code() {
        assert!(
            ModelDriver::A3027
                .decode_confirmed_state(body(A3030_FRAME))
                .is_none()
        );
        assert!(
            ModelDriver::A3030
                .decode_confirmed_state(body(A3027_FRAME))
                .is_none()
        );
    }

    #[test]
    fn state_only_discovery_rejects_an_unregistered_serial() {
        let mut unknown = body(A3027_FRAME).to_vec();
        unknown[44..60].copy_from_slice(b"9999UNKNOWN00000");
        assert_eq!(
            registry::identify_state(&unknown),
            Err(ModelError::UnknownModel)
        );
    }

    #[test]
    fn admits_only_the_two_documented_state_lengths() {
        let original = body(A3027_FRAME).to_vec();
        assert_eq!(original.len(), 61);
        for length in [60, 63, 67] {
            let mut body = original.clone();
            body.resize(length, 0);
            assert!(
                ModelDriver::A3027.decode_confirmed_state(&body).is_none(),
                "{length}-byte body was admitted"
            );
        }
        assert_eq!(
            ModelDriver::A3027.decode_state(&[0; 60]),
            Err(ModelError::IncompleteState)
        );

        // The documented optional touch flag follows the wearing-detection byte.
        let mut with_touch = original;
        with_touch.push(1);
        let state = ModelDriver::A3027
            .decode_confirmed_state(&with_touch)
            .expect("the optional touch flag extends the confirmed layout");
        assert_eq!(state.wearing_detection, Some(true));
    }

    #[test]
    fn writes_the_shared_four_byte_sound_modes() {
        let state = ModelDriver::A3027
            .decode_confirmed_state(body(A3027_FRAME))
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
        assert_eq!(
            state.ambient_sound_mode_write(SoundMode::Normal),
            Err(ModelError::InvalidSoundModeWrite),
            "the pair has no positioned or A3062 control layout"
        );
    }

    #[test]
    fn writes_the_family_eight_band_curve() {
        let state = ModelDriver::A3027
            .decode_confirmed_state(body(A3027_FRAME))
            .expect("confirmed state");
        assert_eq!(
            state
                .model
                .equalizer_write_body(SIGNATURE_CURVE, body(A3027_FRAME)),
            Ok(vec![
                0x00, 0x00, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78,
            ])
        );
        assert_eq!(
            state
                .model
                .equalizer_write_body(REFERENCE_CURVE, body(A3027_FRAME)),
            Ok(vec![
                0xfe, 0xfe, 0x3c, 0xb4, 0x8f, 0xa0, 0x8e, 0xb4, 0x74, 0x88,
            ])
        );
        let mut out_of_range = SIGNATURE_CURVE;
        out_of_range.volume_adjustments[9] = 1;
        assert_eq!(
            state
                .model
                .equalizer_write_body(out_of_range, body(A3027_FRAME)),
            Err(ModelError::InvalidEqualizerWrite)
        );
        assert_eq!(state.model.equalizer_bands(), 8);
        assert_eq!(state.model.equalizer_command(), [0x02, 0x81]);
    }

    #[test]
    fn writes_the_wearing_detection_flag() {
        let state = ModelDriver::A3027
            .decode_confirmed_state(body(A3027_FRAME))
            .expect("confirmed state");
        assert_eq!(state.wearing_detection_write(true), Ok([1]));
        assert_eq!(state.wearing_detection_write(false), Ok([0]));

        let disabled = ModelDriver::A3030
            .decode_confirmed_state(&with_wearing_detection(false))
            .expect("confirmed state");
        assert_eq!(disabled.wearing_detection, Some(false));
        assert_eq!(disabled.wearing_detection_write(true), Ok([1]));
    }

    #[test]
    fn wearing_detection_is_unsupported_for_models_without_the_flag() {
        let a3028 = ModelDriver::A3028
            .decode_confirmed_state(&a3028_reference_body())
            .expect("A3028 reference state");
        assert_eq!(
            a3028.wearing_detection_write(true),
            Err(ModelError::InvalidWearingDetectionWrite)
        );
        assert!(!ModelDriver::A3028.capabilities().wearing_detection_write);
        assert!(ModelDriver::A3027.capabilities().wearing_detection_write);
        assert!(ModelDriver::A3030.capabilities().wearing_detection_write);
        assert_eq!(openscl4_models::WEARING_DETECTION_COMMAND, [0x01, 0x81]);
    }

    #[test]
    fn clamps_or_rejects_the_unexposed_sound_mode_fields() {
        for (value, expected) in [(20, 10), (u8::MAX, u8::MAX)] {
            let mut body = body(A3027_FRAME).to_vec();
            body[38] = value;
            let state = ModelDriver::A3027
                .decode_confirmed_state(&body)
                .expect("custom value is accepted and clamped");
            assert_eq!(
                state.sound_modes_write(SoundMode::Normal),
                Ok([0x02, 0x00, 0x01, expected])
            );
        }
        for (offset, value) in [(36, 4), (36, u8::MAX), (37, 2), (37, u8::MAX)] {
            let mut body = body(A3027_FRAME).to_vec();
            body[offset] = value;
            assert_eq!(
                ModelDriver::A3027.decode_state(&body),
                Err(ModelError::UnknownSoundMode),
                "byte {value} at {offset}"
            );
            assert!(ModelDriver::A3027.decode_confirmed_state(&body).is_none());
        }
    }

    #[test]
    fn rejects_airplane_and_unknown_ambient_modes() {
        for (offset, value) in [(35, 3), (35, u8::MAX)] {
            let mut body = body(A3027_FRAME).to_vec();
            body[offset] = value;
            assert_eq!(
                ModelDriver::A3027.decode_state(&body),
                Err(ModelError::UnknownSoundMode),
                "byte {value} at {offset}"
            );
        }
    }

    fn body(frame: &[u8]) -> &[u8] {
        assert_eq!(frame[5..7], [0x01, 0x01]);
        &frame[9..frame.len() - 1]
    }

    fn with_wearing_detection(enabled: bool) -> Vec<u8> {
        let mut body = body(A3030_FRAME).to_vec();
        body[60] = u8::from(enabled);
        body
    }

    /// The published A3028 body from `core/data/a3028-a3029-mode-reference.json`.
    fn a3028_reference_body() -> Vec<u8> {
        vec![
            0x05, 0x00, 0xfe, 0xfe, 0x3c, 0xb4, 0x8f, 0xa0, 0x8e, 0xb4, 0x74, 0x88, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x01, 0x00, 0x30, 0x32, 0x2e,
            0x33, 0x30, 0x33, 0x30, 0x32, 0x38, 0x54, 0x45, 0x53, 0x54, 0x30, 0x30, 0x30, 0x30,
            0x30, 0x30, 0x30, 0x30,
        ]
    }
}
