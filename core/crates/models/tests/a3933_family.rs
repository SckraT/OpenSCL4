#![forbid(unsafe_code)]

#[cfg(test)]
mod tests {
    use openscl4_models::{
        A3933_STATE_LENGTH, A3933_STATE_LENGTH_WITHOUT_EXTRA_FIELDS, EqualizerSetting, ModelDriver,
        ModelError, SoundMode, registry,
    };

    /// The A3933 faker state body from `core/data/a3933-a3939-state-reference.json`.
    /// The inherited device-faker config carries a synthetic MAC and an all-zero
    /// serial; the published variant replaces the serial at body offsets 16–31
    /// with the synthetic model prefix and recomputes the checksum.
    const A3933_BODY: &[u8] = &[
        0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x30, 0x30, 0x2e, 0x30, 0x30, 0x30, 0x30, 0x2e, 0x30,
        0x30, 0x33, 0x39, 0x33, 0x33, 0x54, 0x45, 0x53, 0x54, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30,
        0x30, 0x30, 0xfe, 0xfe, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78,
        0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x00, 0x00, 0x78, 0x78, 0x78, 0x78,
        0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78,
        0x78, 0x00, 0x00, 0x00, 0x00, 0x00, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78,
        0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x00, 0x00, 0x01, 0x00,
        0x01, 0x00, 0x01, 0x00, 0x01, 0x00, 0x01, 0x00, 0x01, 0x00, 0x07, 0x00, 0x00, 0x01, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    /// The same faker body with the synthetic A3939 serial at body offsets 16–31.
    const A3939_BODY: &[u8] = &[
        0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x30, 0x30, 0x2e, 0x30, 0x30, 0x30, 0x30, 0x2e, 0x30,
        0x30, 0x33, 0x39, 0x33, 0x39, 0x54, 0x45, 0x53, 0x54, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30,
        0x30, 0x30, 0xfe, 0xfe, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78,
        0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x00, 0x00, 0x78, 0x78, 0x78, 0x78,
        0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78,
        0x78, 0x00, 0x00, 0x00, 0x00, 0x00, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78,
        0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x00, 0x00, 0x01, 0x00,
        0x01, 0x00, 0x01, 0x00, 0x01, 0x00, 0x01, 0x00, 0x01, 0x00, 0x07, 0x00, 0x00, 0x01, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    const REFERENCE_CURVE: [i16; 10] = [-60, 60, 23, 40, 22, 60, -4, 16, 0, -120];

    #[test]
    fn identifies_both_public_codes_from_the_state_serial() {
        assert_eq!(A3933_BODY.len(), A3933_STATE_LENGTH);
        assert_eq!(A3939_BODY.len(), A3933_STATE_LENGTH);
        assert_eq!(registry::identify_state(A3933_BODY), Ok(ModelDriver::A3933));
        assert_eq!(registry::identify_state(A3939_BODY), Ok(ModelDriver::A3939));
        assert_eq!(registry::by_code("A3933"), Some(ModelDriver::A3933));
        assert_eq!(registry::by_code("A3939"), Some(ModelDriver::A3939));
    }

    #[test]
    fn rejects_a_state_whose_serial_names_the_other_public_code() {
        assert!(
            ModelDriver::A3933
                .decode_confirmed_state(A3939_BODY)
                .is_none()
        );
        assert!(
            ModelDriver::A3939
                .decode_confirmed_state(A3933_BODY)
                .is_none()
        );
    }

    #[test]
    fn state_discovery_rejects_an_unregistered_serial() {
        let mut unknown = A3933_BODY.to_vec();
        unknown[16..32].copy_from_slice(b"9999UNKNOWN00000");
        assert_eq!(
            registry::identify_state(&unknown),
            Err(ModelError::UnknownModel)
        );
    }

    #[test]
    fn keeps_the_reference_state_for_both_codes() {
        for (body, model) in [
            (A3933_BODY, ModelDriver::A3933),
            (A3939_BODY, ModelDriver::A3939),
        ] {
            let state = model.decode_confirmed_state(body).expect("confirmed state");
            assert_eq!(state.model, model);
            assert_eq!(state.sound_mode, Some(SoundMode::NoiseCanceling));
            assert_eq!(state.battery.left, Some(0));
            assert_eq!(state.battery.right, Some(0));
            assert_eq!(state.battery.case, Some(0));
            assert_eq!(state.gaming_mode, Some(false));
            assert_eq!(state.auto_power_off, None);
            assert_eq!(
                state.firmware_left.map(|version| version.to_string()),
                Some("00.00".to_owned())
            );
            assert_eq!(
                state.firmware_right.map(|version| version.to_string()),
                Some("00.00".to_owned())
            );
            assert_eq!(
                state.equalizer,
                Some(EqualizerSetting {
                    preset_id: 0xfefe,
                    volume_adjustments: [0; 10],
                })
            );
        }
    }

    #[test]
    fn reads_the_state_battery_levels_on_the_five_step_scale() {
        let mut body = A3933_BODY.to_vec();
        body[2] = 3;
        body[3] = 4;
        body[125] = 5;
        let state = ModelDriver::A3933
            .decode_confirmed_state(&body)
            .expect("confirmed state");
        assert_eq!(state.battery.left, Some(60));
        assert_eq!(state.battery.right, Some(80));
        assert_eq!(state.battery.case, Some(100));
    }

    #[test]
    fn admits_the_documented_lengths_without_the_extra_fields() {
        let mut without_extra = A3933_BODY.to_vec();
        without_extra.truncate(A3933_STATE_LENGTH_WITHOUT_EXTRA_FIELDS);
        let state = ModelDriver::A3933
            .decode_confirmed_state(&without_extra)
            .expect("confirmed state without extra fields");
        assert_eq!(state.gaming_mode, None);
        assert_eq!(state.battery.case, None);
        assert_eq!(state.sound_mode, Some(SoundMode::NoiseCanceling));
        assert_eq!(
            state.gaming_mode_write(true),
            Err(ModelError::InvalidGamingModeWrite)
        );

        for length in [121, 123, 131, 133] {
            let mut body = A3933_BODY.to_vec();
            body.resize(length, 0);
            assert!(
                ModelDriver::A3933.decode_confirmed_state(&body).is_none(),
                "accepted a {length}-byte body"
            );
        }
        assert_eq!(
            ModelDriver::A3933.decode_state(&[0; 121]),
            Err(ModelError::IncompleteState)
        );
    }

    #[test]
    fn writes_the_reference_sound_mode_bodies() {
        let state = ModelDriver::A3933
            .decode_confirmed_state(A3933_BODY)
            .expect("confirmed state");
        assert_eq!(
            state.ambient_sound_mode_write(SoundMode::NoiseCanceling),
            Ok([0x00, 0x00, 0x01, 0x00])
        );
        assert_eq!(
            state.ambient_sound_mode_write(SoundMode::Normal),
            Ok([0x02, 0x00, 0x01, 0x00])
        );
    }

    #[test]
    fn writes_both_ten_band_channels_of_the_family_curve() {
        let curve = EqualizerSetting {
            preset_id: 0xfefe,
            volume_adjustments: REFERENCE_CURVE,
        };
        assert_eq!(
            ModelDriver::A3933.equalizer_write_body(curve, A3933_BODY),
            Ok(vec![
                0xfe, 0xfe, // preset
                0x3c, 0xb4, 0x8f, 0xa0, 0x8e, 0xb4, 0x74, 0x88, 0x78, 0x00, // left channel
                0x3c, 0xb4, 0x8f, 0xa0, 0x8e, 0xb4, 0x74, 0x88, 0x78, 0x00, // right channel
            ])
        );
        assert_eq!(ModelDriver::A3933.equalizer_bands(), 10);
        assert_eq!(ModelDriver::A3933.equalizer_command(), [0x02, 0x81]);
    }

    #[test]
    fn writes_the_ten_band_preset_curve() {
        let treble_reducer = EqualizerSetting {
            preset_id: 0x0015,
            volume_adjustments: [0, 0, 0, -20, -30, -40, -40, -60, 0, -120],
        };
        assert_eq!(
            ModelDriver::A3933.equalizer_write_body(treble_reducer, A3933_BODY),
            Ok(vec![
                0x15, 0x00, // preset
                0x78, 0x78, 0x78, 0x64, 0x5a, 0x50, 0x50, 0x3c, 0x78, 0x00, // left channel
                0x78, 0x78, 0x78, 0x64, 0x5a, 0x50, 0x50, 0x3c, 0x78, 0x00, // right channel
            ])
        );
    }

    #[test]
    fn rejects_equalizer_bands_outside_the_wire_range() {
        let mut curve = EqualizerSetting {
            preset_id: 0xfefe,
            volume_adjustments: REFERENCE_CURVE,
        };
        curve.volume_adjustments[0] = 135;
        assert_eq!(
            ModelDriver::A3933.equalizer_write_body(curve, A3933_BODY),
            Err(ModelError::InvalidEqualizerWrite)
        );
        assert!(!ModelDriver::A3933.equalizer_is_writable(curve));
    }

    #[test]
    fn writes_the_measured_gaming_mode_flag() {
        let state = ModelDriver::A3933
            .decode_confirmed_state(A3933_BODY)
            .expect("confirmed state");
        assert_eq!(state.gaming_mode_write(true), Ok([1]));
        assert_eq!(state.gaming_mode_write(false), Ok([0]));

        for value in [2, u8::MAX] {
            let mut body = A3933_BODY.to_vec();
            body[124] = value;
            let state = ModelDriver::A3933
                .decode_confirmed_state(&body)
                .expect("the rest of the state is still readable");
            assert_eq!(state.gaming_mode, None, "unrecognized byte {value}");
            assert_eq!(
                state.gaming_mode_write(true),
                Err(ModelError::InvalidGamingModeWrite)
            );
        }
    }

    #[test]
    fn keeps_auto_power_off_and_game_mode_out_of_the_other_family() {
        assert!(!ModelDriver::A3931.capabilities().gaming_mode_write);
        assert!(ModelDriver::A3933.capabilities().gaming_mode_write);
        assert_eq!(
            ModelDriver::A3933.capabilities(),
            ModelDriver::A3939.capabilities()
        );
        assert!(!ModelDriver::A3933.capabilities().auto_power_off_write);
        assert!(ModelDriver::A3931.capabilities().auto_power_off_write);
        assert!(!ModelDriver::A3939.capabilities().auto_power_off_write);
        for model in [ModelDriver::A3933, ModelDriver::A3939] {
            assert!(model.capabilities().ambient_sound_mode_write);
            assert!(model.capabilities().equalizer_write);
            assert!(!model.capabilities().positioned_sound_mode_write);
            assert_eq!(model.auto_power_off_duration_count(), 0);
        }
    }

    #[test]
    fn rejects_airplane_mode_and_unrecognized_control_values() {
        let mut body = A3933_BODY.to_vec();
        body[116] = 3;
        assert_eq!(
            ModelDriver::A3933.decode_state(&body),
            Err(ModelError::UnknownSoundMode)
        );
        for (offset, value) in [(117, 4), (117, u8::MAX), (118, 2), (118, u8::MAX)] {
            let mut body = A3933_BODY.to_vec();
            body[offset] = value;
            assert_eq!(
                ModelDriver::A3933.decode_state(&body),
                Err(ModelError::UnknownSoundMode),
                "byte {value} at {offset}"
            );
            assert!(ModelDriver::A3933.decode_confirmed_state(&body).is_none());
        }
    }

    #[test]
    fn keeps_the_legacy_clamp_for_the_custom_noise_canceling_field() {
        for (value, expected) in [(20, 10), (u8::MAX, u8::MAX)] {
            let mut body = A3933_BODY.to_vec();
            body[119] = value;
            let state = ModelDriver::A3933
                .decode_confirmed_state(&body)
                .expect("custom value is accepted and clamped");
            assert_eq!(
                state.ambient_sound_mode_write(SoundMode::NoiseCanceling),
                Ok([0x00, 0x00, 0x01, expected])
            );
        }
    }
}
