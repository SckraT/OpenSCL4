#![forbid(unsafe_code)]

#[cfg(test)]
mod tests {
    use openscl4_models::{
        AutoPowerOff, AutoPowerOffSetting, EqualizerSetting, ModelDriver, ModelError, SoundMode,
        registry,
    };

    /// The A3931 faker state body from `core/data/a3931-a3935-state-reference.json`.
    /// The inherited device-faker config carries a synthetic MAC and no serial in
    /// its `01 01` state, so the published frame only recomputes the checksum.
    const A3931_BODY: &[u8] = &[
        0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78,
        0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    /// The faker `01 05` identity body with the synthetic A3931 serial.
    const A3931_IDENTITY: &[u8] = &[
        0x30, 0x30, 0x2e, 0x30, 0x30, 0x30, 0x30, 0x2e, 0x30, 0x30, 0x33, 0x39, 0x33, 0x31, 0x54,
        0x45, 0x53, 0x54, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30,
    ];
    /// The same identity body with the synthetic A3935 serial.
    const A3935_IDENTITY: &[u8] = &[
        0x30, 0x30, 0x2e, 0x30, 0x30, 0x30, 0x30, 0x2e, 0x30, 0x30, 0x33, 0x39, 0x33, 0x35, 0x54,
        0x45, 0x53, 0x54, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30,
    ];
    const REFERENCE_CURVE: [i16; 10] = [-60, 60, 23, 40, 22, 60, -4, 16, 0, 0];

    #[test]
    fn identifies_both_public_codes_from_the_separate_identity_response() {
        assert_eq!(A3931_IDENTITY.len(), 26);
        assert_eq!(
            ModelDriver::identify(A3931_IDENTITY),
            Ok(ModelDriver::A3931)
        );
        assert_eq!(
            ModelDriver::identify(A3935_IDENTITY),
            Ok(ModelDriver::A3935)
        );
        assert_eq!(registry::by_code("A3931"), Some(ModelDriver::A3931));
        assert_eq!(registry::by_code("A3935"), Some(ModelDriver::A3935));
    }

    #[test]
    fn rejects_an_identity_naming_no_registered_driver() {
        let mut unknown = A3931_IDENTITY.to_vec();
        unknown[10..26].copy_from_slice(b"9999UNKNOWN00000");
        assert_eq!(
            ModelDriver::identify(&unknown),
            Err(ModelError::UnknownModel)
        );
        assert_eq!(
            ModelDriver::identify(&A3931_IDENTITY[..25]),
            Err(ModelError::InvalidIdentity)
        );
    }

    #[test]
    fn identifies_the_pair_only_from_the_identity_response() {
        // The state layout carries no serial, so state-only discovery must not
        // select a driver for the pair.
        assert_eq!(
            registry::identify_state(A3931_BODY),
            Err(ModelError::IncompleteState)
        );
        for model in [ModelDriver::A3931, ModelDriver::A3935] {
            let state = model
                .decode_confirmed_state(A3931_BODY)
                .expect("confirmed state");
            assert_eq!(state.model, model);
        }
    }

    #[test]
    fn keeps_the_pair_capabilities_together() {
        let expected = ModelDriver::A3931.capabilities();
        assert_eq!(ModelDriver::A3935.capabilities(), expected);
        assert!(expected.ambient_sound_mode_write);
        assert!(expected.equalizer_write);
        assert!(expected.auto_power_off_write);
        assert!(!expected.gaming_mode_write);
        assert!(!expected.positioned_sound_mode_write);
        assert_eq!(
            ModelDriver::A3931.auto_power_off_duration_count(),
            ModelDriver::A3935.auto_power_off_duration_count()
        );
        assert_eq!(ModelDriver::A3931.auto_power_off_duration_count(), 4);
    }

    #[test]
    fn keeps_the_reference_state_for_both_codes() {
        for model in [ModelDriver::A3931, ModelDriver::A3935] {
            let state = model
                .decode_confirmed_state(A3931_BODY)
                .expect("confirmed state");
            assert_eq!(state.sound_mode, Some(SoundMode::NoiseCanceling));
            assert_eq!(state.battery.left, Some(0));
            assert_eq!(state.battery.right, Some(0));
            assert_eq!(state.battery.device, None);
            assert_eq!(state.battery.case, None);
            assert_eq!(state.firmware_device, None);
            assert_eq!(state.firmware_left, None);
            assert_eq!(state.gaming_mode, None);
            assert_eq!(
                state.equalizer,
                Some(EqualizerSetting {
                    preset_id: 0,
                    volume_adjustments: [0; 10],
                })
            );
            assert_eq!(
                state.auto_power_off,
                Some(AutoPowerOff {
                    enabled: false,
                    duration: 0,
                })
            );
        }
    }

    #[test]
    fn admits_only_the_measured_state_length() {
        for length in [43, 45, 60] {
            let mut body = A3931_BODY.to_vec();
            body.resize(length, 0);
            assert!(
                ModelDriver::A3931.decode_confirmed_state(&body).is_none(),
                "accepted a {length}-byte body"
            );
        }
        assert_eq!(
            ModelDriver::A3931.decode_state(&[0; 43]),
            Err(ModelError::IncompleteState)
        );
    }

    #[test]
    fn writes_the_reference_sound_mode_bodies() {
        let state = ModelDriver::A3931
            .decode_confirmed_state(A3931_BODY)
            .expect("confirmed state");
        assert_eq!(
            state.ambient_sound_mode_write(SoundMode::NoiseCanceling),
            Ok([0x00, 0x00, 0x01, 0x00])
        );
        assert_eq!(
            state.ambient_sound_mode_write(SoundMode::Normal),
            Ok([0x02, 0x00, 0x01, 0x00])
        );
        assert_eq!(
            state.ambient_sound_mode_write(SoundMode::AirplaneMode),
            Err(ModelError::InvalidSoundModeWrite)
        );
    }

    #[test]
    fn writes_the_family_curve_with_both_channels_and_the_drc_echo() {
        let state = ModelDriver::A3931
            .decode_confirmed_state(A3931_BODY)
            .expect("confirmed state");
        assert_eq!(
            state
                .model
                .equalizer_write_body(reference_curve(), A3931_BODY),
            Ok(reference_curve_body())
        );
    }

    #[test]
    fn matches_the_legacy_drc_bytes_for_the_legacy_test_curve() {
        // The legacy `set_equalizer_with_drc` test curve and its DRC bytes from
        // `CommonVolumeAdjustments::apply_drc`.
        let curve = EqualizerSetting {
            preset_id: 0xfefe,
            volume_adjustments: [-60, 60, 23, 120, 22, -120, -4, 16, 0, 0],
        };
        let mut expected = vec![0xfe, 0xfe];
        expected.extend_from_slice(&[
            0x3c, 0xb4, 0x8f, 0xf0, 0x8e, 0x00, 0x74, 0x88, //
            0x3c, 0xb4, 0x8f, 0xf0, 0x8e, 0x00, 0x74, 0x88, //
            0x6d, 0x86, 0x70, 0x88, 0x7b, 0x66, 0x7e, 0x79, //
            0x6d, 0x86, 0x70, 0x88, 0x7b, 0x66, 0x7e, 0x79,
        ]);
        assert_eq!(
            ModelDriver::A3931.equalizer_write_body(curve, A3931_BODY),
            Ok(expected)
        );
    }

    #[test]
    fn writes_the_signature_curve_only_with_eight_bands() {
        let signature = EqualizerSetting {
            preset_id: 0,
            volume_adjustments: [0; 10],
        };
        let mut expected = vec![0x00, 0x00];
        expected.extend(std::iter::repeat_n(0x78, 32));
        assert_eq!(
            ModelDriver::A3931.equalizer_write_body(signature, A3931_BODY),
            Ok(expected)
        );

        let mut out_of_range = signature;
        out_of_range.volume_adjustments[9] = 1;
        assert_eq!(
            ModelDriver::A3931.equalizer_write_body(out_of_range, A3931_BODY),
            Err(ModelError::InvalidEqualizerWrite)
        );
        let mut out_of_range = signature;
        out_of_range.volume_adjustments[0] = 135;
        assert_eq!(
            ModelDriver::A3931.equalizer_write_body(out_of_range, A3931_BODY),
            Err(ModelError::InvalidEqualizerWrite)
        );
        assert_eq!(ModelDriver::A3931.equalizer_bands(), 8);
        assert_eq!(ModelDriver::A3935.equalizer_bands(), 8);
        assert_eq!(ModelDriver::A3931.equalizer_command(), [0x02, 0x83]);
    }

    #[test]
    fn disabling_auto_power_off_keeps_the_previous_duration_index() {
        let body = body_with_auto_power_off(true, 1);
        let state = ModelDriver::A3931
            .decode_confirmed_state(&body)
            .expect("confirmed state");
        assert_eq!(
            state.auto_power_off,
            Some(AutoPowerOff {
                enabled: true,
                duration: 1,
            })
        );

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
        assert_eq!(
            write.expected(),
            AutoPowerOff {
                enabled: true,
                duration: 3,
            }
        );
        assert_eq!(
            state.auto_power_off_write(AutoPowerOffSetting::Enabled(4)),
            Err(ModelError::InvalidAutoPowerOffWrite)
        );
    }

    #[test]
    fn unrecognized_auto_power_off_fields_are_not_confirmed_values() {
        for body in [body_with_auto_power_off(true, 4), {
            let mut body = body_with_auto_power_off(false, 0);
            body[42] = 2;
            body
        }] {
            let state = ModelDriver::A3931
                .decode_confirmed_state(&body)
                .expect("the rest of the state is still readable");
            assert_eq!(state.auto_power_off, None);
            assert_eq!(
                state.auto_power_off_write(AutoPowerOffSetting::Disabled),
                Err(ModelError::InvalidAutoPowerOffWrite)
            );
        }
    }

    #[test]
    fn rejects_airplane_mode_and_unrecognized_control_values() {
        let mut body = A3931_BODY.to_vec();
        body[36] = 3;
        assert_eq!(
            ModelDriver::A3931.decode_state(&body),
            Err(ModelError::UnknownSoundMode)
        );
        for (offset, value) in [(37, 4), (37, u8::MAX), (38, 2), (38, u8::MAX)] {
            let mut body = A3931_BODY.to_vec();
            body[offset] = value;
            assert_eq!(
                ModelDriver::A3931.decode_state(&body),
                Err(ModelError::UnknownSoundMode),
                "byte {value} at {offset}"
            );
            assert!(ModelDriver::A3931.decode_confirmed_state(&body).is_none());
        }
    }

    #[test]
    fn keeps_the_legacy_clamp_for_the_custom_noise_canceling_field() {
        for (value, expected) in [(20, 10), (u8::MAX, u8::MAX)] {
            let mut body = A3931_BODY.to_vec();
            body[39] = value;
            let state = ModelDriver::A3931
                .decode_confirmed_state(&body)
                .expect("custom value is accepted and clamped");
            assert_eq!(
                state.ambient_sound_mode_write(SoundMode::NoiseCanceling),
                Ok([0x00, 0x00, 0x01, expected])
            );
        }
    }

    fn reference_curve() -> EqualizerSetting {
        EqualizerSetting {
            preset_id: 0xfefe,
            volume_adjustments: REFERENCE_CURVE,
        }
    }

    fn reference_curve_body() -> Vec<u8> {
        vec![
            0xfe, 0xfe, // preset
            0x3c, 0xb4, 0x8f, 0xa0, 0x8e, 0xb4, 0x74, 0x88, // left channel
            0x3c, 0xb4, 0x8f, 0xa0, 0x8e, 0xb4, 0x74, 0x88, // right channel
            0x6d, 0x85, 0x74, 0x7e, 0x74, 0x82, 0x72, 0x7c, // left DRC echo
            0x6d, 0x85, 0x74, 0x7e, 0x74, 0x82, 0x72, 0x7c, // right DRC echo
        ]
    }

    fn body_with_auto_power_off(enabled: bool, duration: u8) -> Vec<u8> {
        let mut body = A3931_BODY.to_vec();
        body[42..44].copy_from_slice(&[u8::from(enabled), duration]);
        body
    }
}
