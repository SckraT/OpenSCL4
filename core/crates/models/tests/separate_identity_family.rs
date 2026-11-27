#![forbid(unsafe_code)]

#[cfg(test)]
mod tests {
    use openscl4_models::{
        EQUALIZER_COMMAND, EQUALIZER_WITHOUT_HEAR_ID_COMMAND, EqualizerSetting, ModelDriver,
        ModelError, SoundMode, decode_identity_firmware, decode_ldac_state, registry,
    };

    /// Separate-identity frames from `core/data/README.md`. A3909 keeps the
    /// captured legacy body with its trailing unread byte. The other three
    /// bodies and every identity are derived from the legacy layout because no
    /// physical capture exists; all serial numbers are synthetic.
    const A3909_IDENTITY: &[u8] = include_bytes!("../../../data/a3909-identity.bin");
    const A3909_STATE: &[u8] = include_bytes!("../../../data/a3909-state.bin");
    const A3926_IDENTITY: &[u8] = include_bytes!("../../../data/a3926-identity.bin");
    const A3926_STATE: &[u8] = include_bytes!("../../../data/a3926-state.bin");
    const A3930_IDENTITY: &[u8] = include_bytes!("../../../data/a3930-identity.bin");
    const A3930_STATE: &[u8] = include_bytes!("../../../data/a3930-state.bin");
    const A3930_LDAC: &[u8] = include_bytes!("../../../data/a3930-ldac.bin");
    const A3951_IDENTITY: &[u8] = include_bytes!("../../../data/a3951-identity.bin");
    const A3951_STATE: &[u8] = include_bytes!("../../../data/a3951-state.bin");
    const A3951_LDAC: &[u8] = include_bytes!("../../../data/a3951-ldac.bin");

    const IDENTITY_COMMAND: [u8; 2] = [0x01, 0x05];
    const STATE_COMMAND: [u8; 2] = [0x01, 0x01];
    const LDAC_STATE_COMMAND: [u8; 2] = [0x01, 0x7f];

    /// Requested curve for the derived write checks: preset 0xfefe with the
    /// fixture's first channel.
    const REQUESTED_CURVE: EqualizerSetting = EqualizerSetting {
        preset_id: 0xfefe,
        volume_adjustments: [-60, -30, 0, 30, 60, -10, 10, 20, 0, 0],
    };

    /// The legacy dynamic-range-compression output for `REQUESTED_CURVE` and the
    /// golden curve, in tenths of a decibel.
    const REQUESTED_DRC: [u8; 8] = [0x72, 0x77, 0x78, 0x78, 0x81, 0x72, 0x7a, 0x7a];
    const GOLDEN_DRC: [u8; 8] = [0x74, 0x74, 0x74, 0x77, 0x6b, 0x7c, 0x75, 0x6c];

    /// Golden inputs and body of the legacy `SetEqualizerAndCustomHearId` test.
    /// The state carries the genre as one byte, so the legacy two-byte genre
    /// `fe fe` becomes `fe 00` in a write built from a device state.
    const GOLDEN_VOLUME: [u8; 16] = [
        0x35, 0x10, 0x30, 0x09, 0x4c, 0x45, 0x2c, 0x46, 0x2a, 0x10, 0x6e, 0x19, 0x74, 0x1c, 0x28,
        0x36,
    ];
    const GOLDEN_CUSTOM: [u8; 16] = [
        0x0b, 0x44, 0x2f, 0x76, 0x13, 0x04, 0x02, 0x51, 0x6c, 0x08, 0x54, 0x4f, 0x60, 0x07, 0x0e,
        0x63,
    ];
    const GOLDEN_TIME: [u8; 4] = [0xa0, 0x86, 0x01, 0x00];
    const GOLDEN_CURVE: EqualizerSetting = EqualizerSetting {
        preset_id: 0xfefe,
        volume_adjustments: [-52, -66, -64, -67, -108, -22, -49, -101, 0, 0],
    };
    const GOLDEN_BODY: [u8; 76] = [
        0xfe, 0xfe, 0xfe, 0x00, 0x44, 0x36, 0x38, 0x35, 0x0c, 0x62, 0x47, 0x13, 0x44, 0x36, 0x38,
        0x35, 0x0c, 0x62, 0x47, 0x13, 0x01, 0x02, 0x00, 0x35, 0x10, 0x30, 0x09, 0x4c, 0x45, 0x2c,
        0x46, 0x2a, 0x10, 0x6e, 0x19, 0x74, 0x1c, 0x28, 0x36, 0xa0, 0x86, 0x01, 0x00, 0x01, 0x0b,
        0x44, 0x2f, 0x76, 0x13, 0x04, 0x02, 0x51, 0x6c, 0x08, 0x54, 0x4f, 0x60, 0x07, 0x0e, 0x63,
        0x74, 0x74, 0x74, 0x77, 0x6b, 0x7c, 0x75, 0x6c, 0x74, 0x74, 0x74, 0x77, 0x6b, 0x7c, 0x75,
        0x6c,
    ];

    #[test]
    fn identifies_each_family_model_from_the_separate_identity() {
        for (identity, model) in [
            (A3909_IDENTITY, ModelDriver::A3909),
            (A3926_IDENTITY, ModelDriver::A3926),
            (A3930_IDENTITY, ModelDriver::A3930),
            (A3951_IDENTITY, ModelDriver::A3951),
        ] {
            let body = inbound_body(identity, IDENTITY_COMMAND);
            assert_eq!(ModelDriver::identify(body), Ok(model), "{model:?}");
            assert_eq!(registry::by_code(model.code()), Some(model));
            assert!(model.has_separate_identity(), "{model:?}");
        }
        assert!(!ModelDriver::A3909.reads_ldac_state());
        assert!(!ModelDriver::A3926.reads_ldac_state());
        assert!(ModelDriver::A3930.reads_ldac_state());
        assert!(ModelDriver::A3951.reads_ldac_state());
    }

    #[test]
    fn state_bodies_do_not_select_a_family_model() {
        for state in [A3909_STATE, A3926_STATE, A3930_STATE, A3951_STATE] {
            assert_eq!(
                registry::identify_state(inbound_body(state, STATE_COMMAND)),
                Err(ModelError::UnknownModel)
            );
        }
    }

    #[test]
    fn keeps_the_capability_differences_of_the_family() {
        for (model, equalizer, ambient) in [
            (ModelDriver::A3909, false, false),
            (ModelDriver::A3926, true, false),
            (ModelDriver::A3930, true, true),
            (ModelDriver::A3951, true, true),
        ] {
            let capabilities = model.capabilities();
            assert_eq!(capabilities.equalizer_write, equalizer, "{model:?}");
            assert_eq!(capabilities.ambient_sound_mode_write, ambient, "{model:?}");
            assert!(!capabilities.positioned_sound_mode_write, "{model:?}");
            assert!(!capabilities.gaming_mode_write, "{model:?}");
            assert!(!capabilities.auto_power_off_write, "{model:?}");
        }
        assert!(ModelDriver::A3930.supports_ambient_mode(SoundMode::Normal));
        assert!(ModelDriver::A3930.supports_ambient_mode(SoundMode::Transparency));
        assert!(!ModelDriver::A3930.supports_ambient_mode(SoundMode::NoiseCanceling));
        assert!(ModelDriver::A3951.supports_ambient_mode(SoundMode::NoiseCanceling));
        assert!(!ModelDriver::A3951.supports_ambient_mode(SoundMode::AirplaneMode));
        assert!(!ModelDriver::A3926.supports_ambient_mode(SoundMode::Normal));
    }

    #[test]
    fn reads_the_captured_a3909_state() {
        let body = inbound_body(A3909_STATE, STATE_COMMAND);
        assert_eq!(body.len(), 36);
        let state = ModelDriver::A3909
            .decode_confirmed_state(body)
            .expect("captured A3909 state");
        assert_eq!(state.model, ModelDriver::A3909);
        assert_eq!(state.battery.left, None);
        assert_eq!(state.battery.right, Some(100));
        assert_eq!(state.equalizer, None);
        assert_eq!(state.sound_mode, None);
        assert_eq!(state.ldac, None);

        let parsed_without_trailing_byte = &body[..35];
        assert!(
            ModelDriver::A3909
                .decode_confirmed_state(parsed_without_trailing_byte)
                .is_some()
        );
        assert!(
            ModelDriver::A3909
                .decode_confirmed_state(&body[..34])
                .is_none()
        );
        assert_eq!(
            ModelDriver::A3909.decode_state(&body[..34]),
            Err(ModelError::IncompleteState)
        );
        // The captured state does not expose an equalizer curve.
        assert_eq!(
            ModelDriver::A3909.equalizer_write(REQUESTED_CURVE, body),
            Err(ModelError::InvalidEqualizerWrite)
        );
    }

    #[test]
    fn reads_the_derived_a3926_state() {
        let body = inbound_body(A3926_STATE, STATE_COMMAND);
        assert_eq!(body.len(), 59);
        let state = ModelDriver::A3926
            .decode_confirmed_state(body)
            .expect("derived A3926 state");
        assert_eq!(state.battery.left, Some(60));
        assert_eq!(state.battery.right, Some(80));
        assert_eq!(state.sound_mode, None);
        assert_eq!(
            state.equalizer,
            Some(EqualizerSetting {
                preset_id: 0xfefe,
                volume_adjustments: [-60, -30, 0, 30, 60, -10, 10, 20, 0, 0],
            })
        );
        for length in [58, 60] {
            let mut other = body.to_vec();
            other.resize(length, 0);
            assert!(
                ModelDriver::A3926.decode_confirmed_state(&other).is_none(),
                "accepted a {length}-byte body"
            );
        }
    }

    #[test]
    fn reads_the_derived_a3930_state_and_its_sound_mode_controls() {
        let body = inbound_body(A3930_STATE, STATE_COMMAND);
        assert_eq!(body.len(), 82);
        let state = ModelDriver::A3930
            .decode_confirmed_state(body)
            .expect("derived A3930 state");
        assert_eq!(state.battery.left, Some(80));
        assert_eq!(state.battery.right, Some(100));
        assert_eq!(state.sound_mode, Some(SoundMode::Normal));
        assert_eq!(
            state.equalizer,
            Some(EqualizerSetting {
                preset_id: 0xfefe,
                volume_adjustments: [-120, -90, -60, -30, 0, 30, 60, 90, 0, 0],
            })
        );
        assert_eq!(
            state.ambient_sound_mode_write(SoundMode::Transparency),
            Ok(vec![1, 0, 0, 0])
        );
        assert_eq!(
            state.ambient_sound_mode_write(SoundMode::Normal),
            Ok(vec![2, 0, 0, 0])
        );
        assert_eq!(
            state.ambient_sound_mode_write(SoundMode::NoiseCanceling),
            Err(ModelError::InvalidSoundModeWrite)
        );
        assert_eq!(
            state.ambient_sound_mode_write(SoundMode::AirplaneMode),
            Err(ModelError::InvalidSoundModeWrite)
        );

        let mut with_optional_field = body.to_vec();
        with_optional_field.extend_from_slice(&[0x01, 0x02]);
        assert_eq!(with_optional_field.len(), 84);
        assert!(
            ModelDriver::A3930
                .decode_confirmed_state(&with_optional_field)
                .is_some()
        );
        for length in [81, 83, 85] {
            let mut other = body.to_vec();
            other.resize(length, 0);
            assert!(
                ModelDriver::A3930.decode_confirmed_state(&other).is_none(),
                "accepted a {length}-byte body"
            );
        }
    }

    #[test]
    fn reads_the_derived_a3951_state_and_its_sound_mode_controls() {
        let body = inbound_body(A3951_STATE, STATE_COMMAND);
        assert_eq!(body.len(), 84);
        let state = ModelDriver::A3951
            .decode_confirmed_state(body)
            .expect("derived A3951 state");
        assert_eq!(state.battery.left, Some(100));
        assert_eq!(state.battery.right, Some(40));
        assert_eq!(state.sound_mode, Some(SoundMode::Normal));
        assert_eq!(
            state.equalizer,
            Some(EqualizerSetting {
                preset_id: 0xfefe,
                volume_adjustments: [-100, -50, 0, 50, 100, -20, 20, 40, 0, 0],
            })
        );
        assert_eq!(
            state.ambient_sound_mode_write(SoundMode::NoiseCanceling),
            Ok(vec![0, 0, 1, 0])
        );
        assert_eq!(
            state.ambient_sound_mode_write(SoundMode::Transparency),
            Ok(vec![1, 0, 1, 0])
        );
        assert_eq!(
            state.ambient_sound_mode_write(SoundMode::Normal),
            Ok(vec![2, 0, 1, 0])
        );

        for length in [86, 88] {
            let mut with_optional_fields = body.to_vec();
            with_optional_fields.resize(length, 0);
            assert!(
                ModelDriver::A3951
                    .decode_confirmed_state(&with_optional_fields)
                    .is_some(),
                "the optional fields are tolerated"
            );
        }
        for length in [83, 85, 87, 89] {
            let mut other = body.to_vec();
            other.resize(length, 0);
            assert!(
                ModelDriver::A3951.decode_confirmed_state(&other).is_none(),
                "accepted a {length}-byte body"
            );
        }
    }

    #[test]
    fn rejects_unrecognized_sound_mode_bytes_in_a_confirmed_state() {
        for (offset, value) in [(77, 3), (77, u8::MAX), (78, 4), (79, 2)] {
            let mut body = inbound_body(A3930_STATE, STATE_COMMAND).to_vec();
            body[offset] = value;
            assert_eq!(
                ModelDriver::A3930.decode_state(&body),
                Err(ModelError::UnknownSoundMode),
                "byte {value} at {offset}"
            );
            assert!(ModelDriver::A3930.decode_confirmed_state(&body).is_none());
        }
    }

    #[test]
    fn reads_the_identity_firmware_versions() {
        for (identity, left, right, model) in [
            (A3909_IDENTITY, "01.28", "01.28", ModelDriver::A3909),
            (A3926_IDENTITY, "01.60", "01.60", ModelDriver::A3926),
            (A3930_IDENTITY, "01.70", "01.70", ModelDriver::A3930),
            (A3951_IDENTITY, "01.80", "01.80", ModelDriver::A3951),
        ] {
            let firmware = decode_identity_firmware(inbound_body(identity, IDENTITY_COMMAND))
                .expect("identity firmware");
            assert_eq!(
                firmware.left.map(|version| version.to_string()),
                Some(left.to_owned()),
                "{model:?}"
            );
            assert_eq!(
                firmware.right.map(|version| version.to_string()),
                Some(right.to_owned()),
                "{model:?}"
            );

            let state = model
                .decode_confirmed_state(match model {
                    ModelDriver::A3909 => inbound_body(A3909_STATE, STATE_COMMAND),
                    ModelDriver::A3926 => inbound_body(A3926_STATE, STATE_COMMAND),
                    ModelDriver::A3930 => inbound_body(A3930_STATE, STATE_COMMAND),
                    ModelDriver::A3951 => inbound_body(A3951_STATE, STATE_COMMAND),
                    _ => unreachable!("the family is exhaustive here"),
                })
                .expect("family state");
            let state = state.with_identity_firmware(firmware);
            assert_eq!(
                state.firmware_left.map(|version| version.to_string()),
                Some(left.to_owned())
            );
            assert_eq!(
                state.firmware_right.map(|version| version.to_string()),
                Some(right.to_owned())
            );
        }
    }

    #[test]
    fn rejects_an_identity_without_firmware_or_with_a_wrong_length() {
        let mut absent = [0u8; 26];
        absent[10..26].copy_from_slice(b"3909TEST00000000");
        assert_eq!(
            decode_identity_firmware(&absent),
            Err(ModelError::InvalidIdentity)
        );
        let mut invalid = [0u8; 26];
        invalid[0..5].copy_from_slice(b"01.28");
        invalid[5..10].copy_from_slice(&[0xff, 0xff, 0xff, 0xff, 0xfe]);
        invalid[10..26].copy_from_slice(b"3909TEST00000000");
        assert_eq!(
            decode_identity_firmware(&invalid),
            Err(ModelError::InvalidFirmware)
        );
        invalid[0..5].copy_from_slice(&[0; 5]);
        invalid[5..10].copy_from_slice(&[0xff; 5]);
        assert_eq!(
            decode_identity_firmware(&invalid),
            Err(ModelError::InvalidIdentity)
        );
        assert_eq!(
            decode_identity_firmware(&b"01.28"[..5]),
            Err(ModelError::InvalidIdentity)
        );
    }

    #[test]
    fn reads_the_targeted_ldac_state() {
        assert_eq!(
            decode_ldac_state(inbound_body(A3930_LDAC, LDAC_STATE_COMMAND)),
            Ok(false)
        );
        assert_eq!(
            decode_ldac_state(inbound_body(A3951_LDAC, LDAC_STATE_COMMAND)),
            Ok(true)
        );
        for invalid in [&[][..], &[2][..], &[1, 0][..]] {
            assert_eq!(
                decode_ldac_state(invalid),
                Err(ModelError::InvalidLdacState)
            );
        }
    }

    #[test]
    fn writes_the_basic_hear_id_equalizer_body() {
        let state = inbound_body(A3926_STATE, STATE_COMMAND);
        let write = ModelDriver::A3926
            .equalizer_write(REQUESTED_CURVE, state)
            .expect("basic hear-id write");
        assert_eq!(write.command(), EQUALIZER_COMMAND);
        assert_eq!(
            write.body(),
            &[
                0xfe, 0xfe, // preset
                0x00, 0x00, // default favorite music genre
                0x3c, 0x5a, 0x78, 0x96, 0xb4, 0x6e, 0x82, 0x8c, // first channel
                0x3c, 0x5a, 0x78, 0x96, 0xb4, 0x6e, 0x82, 0x8c, // second channel
                0x01, 0x02, 0x00, // gender, age range, unknown
                0x00, 0x3c, 0x78, 0xb4, 0xf0, 0xfe, 0x6c, 0x84, // hear-id volume channel 1
                0x79, 0x7a, 0x7b, 0x7c, 0x7d, 0x7e, 0x7f, 0x80, // hear-id volume channel 2
                0x00, 0x00, 0x00, 0x00, // measurement time
                0x00, // hear-id type
                0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, // custom channel 1
                0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, // custom channel 2
                0x72, 0x77, 0x78, 0x78, 0x81, 0x72, 0x7a, 0x7a, // active curve
                0x72, 0x77, 0x78, 0x78, 0x81, 0x72, 0x7a, 0x7a,
            ]
        );
        assert_eq!(write.body().len(), 76);
        assert_eq!(&write.body()[60..68], &REQUESTED_DRC);
    }

    #[test]
    fn writes_the_custom_hear_id_equalizer_body_from_the_derived_states() {
        for (
            model,
            state,
            expected_gender,
            expected_age,
            expected_genre,
            expected_type,
            volume_first_byte,
        ) in [
            (
                ModelDriver::A3930,
                inbound_body(A3930_STATE, STATE_COMMAND),
                1u8,
                2u8,
                3u8,
                2u8,
                0x46u8,
            ),
            (
                ModelDriver::A3951,
                inbound_body(A3951_STATE, STATE_COMMAND),
                2,
                3,
                2,
                1,
                0x3c,
            ),
        ] {
            let write = model
                .equalizer_write(REQUESTED_CURVE, state)
                .expect("custom hear-id write");
            assert_eq!(write.command(), EQUALIZER_COMMAND);
            assert_eq!(write.body().len(), 76);
            assert_eq!(&write.body()[0..2], &[0xfe, 0xfe]);
            assert_eq!(&write.body()[2..4], &[expected_genre, 0x00]);
            assert_eq!(
                &write.body()[4..20],
                &[
                    0x3c, 0x5a, 0x78, 0x96, 0xb4, 0x6e, 0x82, 0x8c, 0x3c, 0x5a, 0x78, 0x96, 0xb4,
                    0x6e, 0x82, 0x8c,
                ][..]
            );
            assert_eq!(
                &write.body()[20..23],
                &[expected_gender, expected_age, 0x00]
            );
            assert_eq!(write.body()[23], volume_first_byte);
            assert_eq!(write.body()[39], 0x01, "the measurement time starts here");
            assert_eq!(&write.body()[39..43], &[0x01, 0x02, 0x03, 0x04]);
            assert_eq!(&write.body()[43], &expected_type);
            assert_eq!(&write.body()[60..], &REQUESTED_DRC[..].repeat(2)[..]);
        }
    }

    #[test]
    fn reproduces_the_legacy_golden_equalizer_body() {
        // The legacy golden frame with the genre byte the state can carry.
        for model in [ModelDriver::A3930, ModelDriver::A3951] {
            let fixture = match model {
                ModelDriver::A3930 => inbound_body(A3930_STATE, STATE_COMMAND),
                ModelDriver::A3951 => inbound_body(A3951_STATE, STATE_COMMAND),
                _ => unreachable!(),
            };
            let mut state = fixture.to_vec();
            state[24] = 1; // gender
            state[25] = 2; // age range, so the hear-id block is written
            state[26] = 0; // hear-id disabled in the state
            state[27..43].copy_from_slice(&GOLDEN_VOLUME);
            state[43..47].copy_from_slice(&GOLDEN_TIME);
            state[47] = 1; // hear-id type custom
            state[48] = 0xfe; // favorite music genre, one state byte
            state[49..65].copy_from_slice(&GOLDEN_CUSTOM);

            let write = model
                .equalizer_write(GOLDEN_CURVE, &state)
                .expect("golden hear-id write");
            assert_eq!(write.command(), EQUALIZER_COMMAND);
            assert_eq!(
                write.body(),
                &GOLDEN_BODY,
                "the legacy golden frame for {model:?}"
            );
            assert_eq!(&write.body()[60..68], &GOLDEN_DRC);
        }
    }

    #[test]
    fn writes_the_no_hear_id_equalizer_body_from_an_absent_age_range() {
        let fixture = inbound_body(A3926_STATE, STATE_COMMAND).to_vec();
        let mut state = fixture;
        state[25] = u8::MAX;
        let requested = EqualizerSetting {
            preset_id: 0,
            volume_adjustments: [0; 10],
        };
        let write = ModelDriver::A3926
            .equalizer_write(requested, &state)
            .expect("no hear-id write");
        assert_eq!(write.command(), EQUALIZER_WITHOUT_HEAR_ID_COMMAND);
        assert_eq!(write.body(), &GOLDEN_NO_HEAR_ID_BODY);
    }

    /// The legacy `SetEqualizerAndCustomHearId` output for a zero curve and an
    /// absent hear-id block, which the shared builder emits as `03 86`.
    const GOLDEN_NO_HEAR_ID_BODY: [u8; 74] = [
        0x00, 0x00, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78,
        0x78, 0x78, 0x78, 0xff, 0xff, 0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x00, 0x00, 0x00, 0x00, 0x00, 0xff, 0xff, 0xff,
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x78, 0x78,
        0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78,
    ];

    #[test]
    fn rejects_curves_that_do_not_fit_the_family_write() {
        let state = inbound_body(A3930_STATE, STATE_COMMAND);
        let mut beyond_the_range = REQUESTED_CURVE;
        beyond_the_range.volume_adjustments[9] = 1;
        for model in [ModelDriver::A3926, ModelDriver::A3930, ModelDriver::A3951] {
            let fixture = match model {
                ModelDriver::A3926 => inbound_body(A3926_STATE, STATE_COMMAND),
                ModelDriver::A3930 => state,
                ModelDriver::A3951 => inbound_body(A3951_STATE, STATE_COMMAND),
                _ => unreachable!(),
            };
            assert_eq!(
                model.equalizer_write(beyond_the_range, fixture),
                Err(ModelError::InvalidEqualizerWrite),
                "{model:?}"
            );
            let mut invalid_gain = REQUESTED_CURVE;
            invalid_gain.volume_adjustments[0] = 135;
            assert_eq!(
                model.equalizer_write(invalid_gain, fixture),
                Err(ModelError::InvalidEqualizerWrite),
                "{model:?}"
            );
            assert_eq!(
                model.equalizer_write(REQUESTED_CURVE, &fixture[..10]),
                Err(ModelError::IncompleteState),
                "{model:?}"
            );
        }
        assert_eq!(
            ModelDriver::A3949.equalizer_write(REQUESTED_CURVE, state),
            Err(ModelError::InvalidEqualizerWrite)
        );
    }

    fn inbound_body(fixture: &[u8], command: [u8; 2]) -> &[u8] {
        assert_eq!(fixture[0..2], [0x09, 0xff], "inbound prefix");
        assert_eq!(fixture[5..7], command, "command");
        assert_eq!(
            usize::from(u16::from_le_bytes([fixture[7], fixture[8]])),
            fixture.len(),
            "declared length"
        );
        assert_eq!(
            *fixture.last().expect("checksum"),
            fixture[..fixture.len() - 1]
                .iter()
                .fold(0u8, |sum, byte| sum.wrapping_add(*byte)),
            "additive checksum"
        );
        &fixture[9..fixture.len() - 1]
    }
}
