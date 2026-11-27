#![forbid(unsafe_code)]

#[cfg(test)]
mod tests {
    use openscl4_models::{
        AutoPowerOff, AutoPowerOffSetting, EqualizerSetting, ModelDriver, ModelError, SoundMode,
        registry,
    };

    /// State bodies from `core/data/a3028-a3029-mode-reference.json`. The
    /// reference starts from one legacy A3029-prefixed example and replaces the
    /// full serial at body offsets 44–59 with the synthetic model-specific
    /// value, then recomputes the checksum. Neither body has extra fields.
    const A3028_BODY: &[u8] = &[
        0x05, 0x00, 0xfe, 0xfe, 0x3c, 0xb4, 0x8f, 0xa0, 0x8e, 0xb4, 0x74, 0x88, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x01, 0x00, 0x30, 0x32, 0x2e, 0x33, 0x30, 0x33,
        0x30, 0x32, 0x38, 0x54, 0x45, 0x53, 0x54, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30,
    ];
    const A3029_BODY: &[u8] = &[
        0x05, 0x00, 0xfe, 0xfe, 0x3c, 0xb4, 0x8f, 0xa0, 0x8e, 0xb4, 0x74, 0x88, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x01, 0x00, 0x30, 0x32, 0x2e, 0x33, 0x30, 0x33,
        0x30, 0x32, 0x39, 0x54, 0x45, 0x53, 0x54, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30,
    ];
    /// The documented extra fields: unknown1, touch control, dual connections,
    /// auto power off (enabled, duration index 1) and the two prompt tones.
    const EXTRA_FIELDS: [u8; 7] = [0x00, 0x00, 0x00, 0x01, 0x01, 0x01, 0x01];
    const REFERENCE_CURVE: [i16; 10] = [-60, 60, 23, 40, 22, 60, -4, 16, 0, 0];

    #[test]
    fn identifies_both_public_codes_from_the_measured_state() {
        assert_eq!(A3028_BODY.len(), 60);
        assert_eq!(registry::identify_state(A3028_BODY), Ok(ModelDriver::A3028));
        assert_eq!(registry::identify_state(A3029_BODY), Ok(ModelDriver::A3029));
    }

    #[test]
    fn keeps_the_public_code_in_the_confirmed_state() {
        for (body, model) in [
            (A3028_BODY, ModelDriver::A3028),
            (A3029_BODY, ModelDriver::A3029),
        ] {
            let state = model.decode_confirmed_state(body).expect("confirmed state");
            assert_eq!(state.model, model);
            assert_eq!(state.sound_mode, Some(SoundMode::Normal));
            assert_eq!(state.battery.device, Some(100));
            assert_eq!(
                state.firmware_device.map(|firmware| firmware.to_string()),
                Some("02.30".to_owned())
            );
            assert_eq!(state.equalizer, Some(reference_curve()));
            assert_eq!(state.auto_power_off, None);
        }
    }

    #[test]
    fn rejects_a_state_whose_serial_names_the_other_public_code() {
        assert!(
            ModelDriver::A3028
                .decode_confirmed_state(A3029_BODY)
                .is_none()
        );
        assert!(
            ModelDriver::A3029
                .decode_confirmed_state(A3028_BODY)
                .is_none()
        );
    }

    #[test]
    fn state_only_discovery_rejects_an_unregistered_serial() {
        let mut unknown = A3028_BODY.to_vec();
        unknown[44..60].copy_from_slice(b"9999UNKNOWN00000");
        assert_eq!(
            registry::identify_state(&unknown),
            Err(ModelError::UnknownModel)
        );
    }

    #[test]
    fn admits_only_the_two_documented_state_lengths() {
        for model in [ModelDriver::A3028, ModelDriver::A3029] {
            let body = if model == ModelDriver::A3028 {
                A3028_BODY
            } else {
                A3029_BODY
            };
            for length in [59, 61, 66, 68] {
                let mut truncated = body.to_vec();
                truncated.resize(length, 0);
                assert!(
                    model.decode_confirmed_state(&truncated).is_none(),
                    "{model:?} accepted a {length}-byte body"
                );
            }
        }
        assert_eq!(
            ModelDriver::A3028.decode_state(&[0; 59]),
            Err(ModelError::IncompleteState)
        );
    }

    #[test]
    fn writes_the_reference_sound_mode_bodies() {
        let state = ModelDriver::A3028
            .decode_confirmed_state(A3028_BODY)
            .expect("confirmed state");
        assert_eq!(
            state.sound_modes_write(SoundMode::NoiseCanceling),
            Ok([0x00, 0x00, 0x01, 0x00])
        );
        assert_eq!(
            state.sound_modes_write(SoundMode::Normal),
            Ok([0x02, 0x00, 0x01, 0x00])
        );
        assert_eq!(
            state.ambient_sound_mode_write(SoundMode::NoiseCanceling),
            Err(ModelError::InvalidSoundModeWrite)
        );
    }

    #[test]
    fn writes_only_the_family_eight_band_curve() {
        let state = ModelDriver::A3028
            .decode_confirmed_state(A3028_BODY)
            .expect("confirmed state");
        assert_eq!(
            state
                .model
                .equalizer_write_body(reference_curve(), A3028_BODY),
            Ok(vec![
                0xfe, 0xfe, 0x3c, 0xb4, 0x8f, 0xa0, 0x8e, 0xb4, 0x74, 0x88,
            ])
        );

        let signature = EqualizerSetting {
            preset_id: 0,
            volume_adjustments: [0; 10],
        };
        assert_eq!(
            state
                .model
                .equalizer_write_body(signature, A3028_BODY)
                .expect("eight-band signature write"),
            vec![0x00, 0x00, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78]
        );

        let mut out_of_range = signature;
        out_of_range.volume_adjustments[9] = 1;
        assert_eq!(
            state.model.equalizer_write_body(out_of_range, A3028_BODY),
            Err(ModelError::InvalidEqualizerWrite)
        );
    }

    #[test]
    fn keeps_the_ten_band_path_for_the_other_models() {
        assert_eq!(ModelDriver::A3028.equalizer_bands(), 8);
        assert_eq!(ModelDriver::A3029.equalizer_bands(), 8);
        assert_eq!(ModelDriver::A3062.equalizer_bands(), 10);

        let ten_band = EqualizerSetting {
            preset_id: 0xfefe,
            volume_adjustments: [1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
        };
        assert!(ModelDriver::A3062.equalizer_is_writable(ten_band));
        assert!(!ModelDriver::A3028.equalizer_is_writable(ten_band));

        let eight_band = EqualizerSetting {
            preset_id: 0xfefe,
            volume_adjustments: [1, 2, 3, 4, 5, 6, 7, 8, 0, 0],
        };
        assert!(ModelDriver::A3028.equalizer_is_writable(eight_band));
        assert!(ModelDriver::A3029.equalizer_is_writable(eight_band));
    }

    #[test]
    fn auto_power_off_is_available_only_with_the_extra_fields() {
        let with_extras = state_with_extras();
        let state = ModelDriver::A3028
            .decode_confirmed_state(&with_extras)
            .expect("state with extra fields");
        assert_eq!(
            state.auto_power_off,
            Some(AutoPowerOff {
                enabled: true,
                duration: 1,
            })
        );
    }

    #[test]
    fn disabling_auto_power_off_keeps_the_previous_duration_index() {
        let state = ModelDriver::A3028
            .decode_confirmed_state(&state_with_extras())
            .expect("state with extra fields");

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
    }

    #[test]
    fn rejects_auto_power_off_durations_outside_the_family_table() {
        let state = ModelDriver::A3028
            .decode_confirmed_state(&state_with_extras())
            .expect("state with extra fields");
        for index in [4, 5, u8::MAX] {
            assert_eq!(
                state.auto_power_off_write(AutoPowerOffSetting::Enabled(index)),
                Err(ModelError::InvalidAutoPowerOffWrite)
            );
        }
    }

    #[test]
    fn unrecognized_auto_power_off_fields_are_not_confirmed_values() {
        for (offset, value) in [(63, 2), (63, u8::MAX), (64, 4), (64, u8::MAX)] {
            let mut body = state_with_extras();
            body[offset] = value;
            let state = ModelDriver::A3028
                .decode_confirmed_state(&body)
                .expect("the rest of the state is still readable");
            assert_eq!(
                state.auto_power_off, None,
                "unrecognized byte {value} at {offset}"
            );
            assert_eq!(state.sound_mode, Some(SoundMode::Normal));
            assert_eq!(
                state.auto_power_off_write(AutoPowerOffSetting::Disabled),
                Err(ModelError::InvalidAutoPowerOffWrite)
            );
        }
    }

    #[test]
    fn rejects_auto_power_off_without_the_extra_fields() {
        let state = ModelDriver::A3028
            .decode_confirmed_state(A3028_BODY)
            .expect("confirmed state without extras");
        assert_eq!(state.auto_power_off, None);
        for setting in [
            AutoPowerOffSetting::Enabled(0),
            AutoPowerOffSetting::Disabled,
        ] {
            assert_eq!(
                state.auto_power_off_write(setting),
                Err(ModelError::InvalidAutoPowerOffWrite)
            );
        }
    }

    #[test]
    fn rejects_airplane_mode_in_the_family_state() {
        let mut body = A3028_BODY.to_vec();
        body[35] = 3;
        assert_eq!(
            ModelDriver::A3028.decode_state(&body),
            Err(ModelError::UnknownSoundMode)
        );
    }

    #[test]
    fn rejects_unrecognized_anc_and_transparency_values_in_the_state() {
        for (offset, value) in [(36, 4), (36, u8::MAX), (37, 2), (37, u8::MAX)] {
            let mut body = A3028_BODY.to_vec();
            body[offset] = value;
            assert_eq!(
                ModelDriver::A3028.decode_state(&body),
                Err(ModelError::UnknownSoundMode),
                "byte {value} at {offset}"
            );
            assert!(ModelDriver::A3028.decode_confirmed_state(&body).is_none());
        }
    }

    #[test]
    fn keeps_the_legacy_clamp_for_the_custom_noise_canceling_field() {
        for (value, expected) in [(20, 10), (u8::MAX, u8::MAX)] {
            let mut body = A3028_BODY.to_vec();
            body[38] = value;
            let state = ModelDriver::A3028
                .decode_confirmed_state(&body)
                .expect("custom value is accepted and clamped");
            assert_eq!(
                state.sound_modes_write(SoundMode::Normal),
                Ok([0x02, 0x00, 0x01, expected])
            );
        }
    }

    fn reference_curve() -> EqualizerSetting {
        EqualizerSetting {
            preset_id: 0xfefe,
            volume_adjustments: REFERENCE_CURVE,
        }
    }

    /// The 60-byte reference body plus the documented extra fields block.
    fn state_with_extras() -> Vec<u8> {
        let mut body = A3028_BODY.to_vec();
        body.extend_from_slice(&EXTRA_FIELDS);
        assert_eq!(body.len(), 67);
        body
    }
}
