#![forbid(unsafe_code)]

#[cfg(test)]
mod tests {
    use openscl4_models::{
        AutoPowerOff, AutoPowerOffSetting, ColorfulLights, ColorfulLightsMode, EqualizerSetting,
        HostDevice, ModelDriver, ModelError, RgbColor, SoundMode, registry,
    };

    const A3330_STATE: &[u8] = include_bytes!("../../../data/a3330-paired-state.bin");
    const A3388_STATE: &[u8] = include_bytes!("../../../data/a3388-paired-state.bin");
    const A3876_STATE: &[u8] = include_bytes!("../../../data/a3876-paired-state.bin");
    const D1101_STATE: &[u8] = include_bytes!("../../../data/d1101-paired-state.bin");

    /// The reference curve used by the legacy DRC test, extended with the two
    /// invisible bands' defaults.
    const REFERENCE_CURVE: EqualizerSetting = EqualizerSetting {
        preset_id: 0xfefe,
        volume_adjustments: [-60, 60, 23, 120, 22, -120, -4, 16, 0, -120],
    };

    fn body(fixture: &[u8]) -> &[u8] {
        assert_eq!(fixture[5..7], [0x01, 0x01]);
        &fixture[9..fixture.len() - 1]
    }

    #[test]
    fn identifies_each_paired_model_from_its_measured_state() {
        for (fixture, model) in [
            (A3330_STATE, ModelDriver::A3330),
            (A3388_STATE, ModelDriver::A3388),
            (A3876_STATE, ModelDriver::A3876),
            (D1101_STATE, ModelDriver::D1101),
        ] {
            let body = body(fixture);
            assert_eq!(registry::identify_state(body), Ok(model), "{model:?}");
        }
    }

    #[test]
    fn identifies_each_paired_model_from_its_identity_layout() {
        for (fixture, model) in [
            (A3330_STATE, ModelDriver::A3330),
            (A3388_STATE, ModelDriver::A3388),
            (A3876_STATE, ModelDriver::A3876),
            (D1101_STATE, ModelDriver::D1101),
        ] {
            let body = body(fixture);
            let mut identity = Vec::with_capacity(26);
            identity.extend_from_slice(&body[4..14]);
            identity.extend_from_slice(&body[14..30]);
            assert_eq!(identity.len(), 26);
            assert_eq!(ModelDriver::identify(&identity), Ok(model), "{model:?}");
        }
    }

    #[test]
    fn confirmed_layout_requires_the_measured_capture_length() {
        for (fixture, model) in [
            (A3330_STATE, ModelDriver::A3330),
            (A3388_STATE, ModelDriver::A3388),
            (A3876_STATE, ModelDriver::A3876),
            (D1101_STATE, ModelDriver::D1101),
        ] {
            let body = body(fixture);
            assert!(
                model.decode_confirmed_state(body).is_some(),
                "measured {} state",
                model.code()
            );
            let mut short = body.to_vec();
            short.pop();
            assert!(
                model.decode_confirmed_state(&short).is_none(),
                "one byte short for {}",
                model.code()
            );
            let mut extra = body.to_vec();
            extra.push(0);
            assert!(
                model.decode_confirmed_state(&extra).is_none(),
                "trailing byte for {}",
                model.code()
            );
        }
    }

    #[test]
    fn rejects_a_state_whose_serial_names_another_model() {
        for (fixture, model) in [
            (A3330_STATE, ModelDriver::A3330),
            (A3388_STATE, ModelDriver::A3388),
            (A3876_STATE, ModelDriver::A3876),
            (D1101_STATE, ModelDriver::D1101),
        ] {
            let mut body = body(fixture).to_vec();
            body[14..30].copy_from_slice(b"9999TEST00000000");
            assert!(model.decode_confirmed_state(&body).is_none());
            assert_eq!(
                registry::identify_state(&body),
                Err(ModelError::UnknownModel)
            );
        }
    }

    #[test]
    fn reads_the_a3330_state() {
        let state = ModelDriver::A3330
            .decode_confirmed_state(body(A3330_STATE))
            .expect("confirmed A3330 state");
        assert_eq!(state.model, ModelDriver::A3330);
        assert_eq!(state.battery.left, Some(100));
        assert_eq!(state.battery.right, Some(100));
        assert_eq!(state.battery.case, Some(40));
        assert_eq!(
            state.firmware_left.map(|version| version.to_string()),
            Some("04.27".to_owned())
        );
        assert_eq!(
            state.firmware_right.map(|version| version.to_string()),
            Some("04.27".to_owned())
        );
        let paired = state.paired.expect("paired readings");
        assert!(paired.tws.connected);
        assert_eq!(paired.tws.host, HostDevice::Right);
        assert_eq!(paired.buttons_enabled, Some(true));
        assert_eq!(paired.touch_tone, Some(true));
        assert_eq!(paired.low_battery_prompt, Some(true));
        assert_eq!(paired.dual_connections_enabled, Some(false));
        assert_eq!(paired.ldac, None);
        assert_eq!(paired.colorful_lights, None);
        assert_eq!(
            state.equalizer,
            Some(EqualizerSetting {
                preset_id: 0xfefe,
                volume_adjustments: [11, 15, 16, 11, 4, 0, 0, 0, 0, 0],
            })
        );
        assert_eq!(state.sound_mode, None);
    }

    #[test]
    fn reads_the_a3388_state() {
        let state = ModelDriver::A3388
            .decode_confirmed_state(body(A3388_STATE))
            .expect("confirmed A3388 state");
        assert_eq!(state.battery.left, Some(90));
        assert_eq!(state.battery.right, Some(90));
        assert_eq!(state.battery.case, Some(80));
        let paired = state.paired.expect("paired readings");
        assert_eq!(paired.tws.host, HostDevice::Left);
        assert_eq!(paired.touch_tone, Some(true));
        assert_eq!(paired.low_battery_prompt, Some(true));
        assert_eq!(paired.dual_connections_enabled, Some(true));
        assert_eq!(paired.buttons_enabled, Some(true));
        assert_eq!(
            state.equalizer,
            Some(EqualizerSetting {
                preset_id: 5,
                volume_adjustments: [-30, 20, 40, 40, 30, 20, 0, -20, -120, -120],
            })
        );
    }

    #[test]
    fn reads_the_a3876_state_and_its_light_settings() {
        let state = ModelDriver::A3876
            .decode_confirmed_state(body(A3876_STATE))
            .expect("confirmed A3876 state");
        assert_eq!(state.battery.left, Some(60));
        assert_eq!(state.battery.right, Some(100));
        assert_eq!(state.battery.case, None);
        assert_eq!(
            state.firmware_left.map(|version| version.to_string()),
            Some("03.15".to_owned())
        );
        assert_eq!(state.gaming_mode, Some(false));
        assert_eq!(
            state.auto_power_off,
            Some(AutoPowerOff {
                enabled: true,
                duration: 2,
            })
        );
        let paired = state.paired.expect("paired readings");
        assert_eq!(
            paired.colorful_lights,
            Some(ColorfulLights {
                enabled: true,
                brightness: 4,
                auto_lights_off_minutes: 45,
                mode: ColorfulLightsMode::Breathing,
                color: RgbColor {
                    red: 23,
                    green: 187,
                    blue: 239,
                },
            })
        );
        assert_eq!(paired.volume_balance, Some(0));
        assert_eq!(paired.voice_prompt, Some(true));
        assert_eq!(paired.dual_connections_enabled, Some(true));
        assert_eq!(paired.touch_tone, None);
        assert_eq!(state.equalizer.map(|eq| eq.preset_id), Some(0xfefe));
    }

    #[test]
    fn reads_the_d1101_state_ldac_and_flags() {
        let state = ModelDriver::D1101
            .decode_confirmed_state(body(D1101_STATE))
            .expect("confirmed D1101 state");
        assert_eq!(state.battery.left, Some(100));
        assert_eq!(state.battery.right, Some(10));
        assert_eq!(state.battery.case, None);
        let paired = state.paired.expect("paired readings");
        assert_eq!(paired.ldac, Some(false));
        assert_eq!(paired.buttons_enabled, Some(true));
        assert_eq!(paired.low_battery_prompt, Some(true));
        assert_eq!(paired.dual_connections_enabled, Some(true));
        assert_eq!(
            state.auto_power_off,
            Some(AutoPowerOff {
                enabled: true,
                duration: 0,
            })
        );
        assert_eq!(
            state.equalizer,
            Some(EqualizerSetting {
                preset_id: 0xfefe,
                volume_adjustments: [40, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            })
        );
    }

    #[test]
    fn an_absent_earbud_or_case_is_not_a_confirmed_level() {
        // A3388 captures with one earbud out of the case report the case byte
        // as 0xFF; the legacy handler reports no level for it.
        let mut body = body(A3388_STATE).to_vec();
        body[35] = u8::MAX;
        body[2] = u8::MAX;
        let state = ModelDriver::A3388
            .decode_confirmed_state(&body)
            .expect("the rest of the state is readable");
        assert_eq!(state.battery.case, None);
        assert_eq!(state.battery.left, None);
        assert_eq!(state.battery.right, Some(90));
    }

    #[test]
    fn writes_the_a3330_drc_curve() {
        let state = ModelDriver::A3330
            .decode_confirmed_state(body(A3330_STATE))
            .expect("confirmed A3330 state");
        assert_eq!(ModelDriver::A3330.equalizer_command(), [0x02, 0x83]);
        assert_eq!(
            state
                .model
                .equalizer_write_body(REFERENCE_CURVE, body(A3330_STATE)),
            Ok(vec![
                0xfe, 0xfe, // preset
                0x3c, 0xb4, 0x8f, 0xf0, 0x8e, 0x00, 0x74, 0x88, 0x78, 0x00, // curve
                0x6d, 0x86, 0x70, 0x88, 0x7b, 0x66, 0x7e, 0x79, 0x78, 0x00, // DRC
            ])
        );
    }

    #[test]
    fn writes_the_a3388_two_channel_curve() {
        let state = ModelDriver::A3388
            .decode_confirmed_state(body(A3388_STATE))
            .expect("confirmed A3388 state");
        assert_eq!(ModelDriver::A3388.equalizer_command(), [0x02, 0x83]);
        assert_eq!(
            state
                .model
                .equalizer_write_body(REFERENCE_CURVE, body(A3388_STATE)),
            Ok(vec![
                0xfe, 0xfe, // preset
                0x3c, 0xb4, 0x8f, 0xf0, 0x8e, 0x00, 0x74, 0x88, 0x78, 0x00, // left
                0x3c, 0xb4, 0x8f, 0xf0, 0x8e, 0x00, 0x74, 0x88, 0x78, 0x00, // right
            ])
        );
    }

    #[test]
    fn writes_the_a3876_first_channel_with_drc() {
        let state = ModelDriver::A3876
            .decode_confirmed_state(body(A3876_STATE))
            .expect("confirmed A3876 state");
        assert_eq!(ModelDriver::A3876.equalizer_command(), [0x02, 0x83]);
        assert_eq!(
            state
                .model
                .equalizer_write_body(REFERENCE_CURVE, body(A3876_STATE)),
            Ok(vec![
                0xfe, 0xfe, 0x3c, 0xb4, 0x8f, 0xf0, 0x8e, 0x00, 0x74, 0x88, 0x78, 0x00, 0x6d, 0x86,
                0x70, 0x88, 0x7b, 0x66, 0x7e, 0x79, 0x78, 0x00,
            ])
        );
    }

    #[test]
    fn writes_the_d1101_two_channel_curve_on_the_plain_equalizer_command() {
        let state = ModelDriver::D1101
            .decode_confirmed_state(body(D1101_STATE))
            .expect("confirmed D1101 state");
        assert_eq!(ModelDriver::D1101.equalizer_command(), [0x02, 0x81]);
        assert_eq!(
            state
                .model
                .equalizer_write_body(REFERENCE_CURVE, body(D1101_STATE)),
            Ok(vec![
                0xfe, 0xfe, 0x3c, 0xb4, 0x8f, 0xf0, 0x8e, 0x00, 0x74, 0x88, 0x78, 0x00, 0x3c, 0xb4,
                0x8f, 0xf0, 0x8e, 0x00, 0x74, 0x88, 0x78, 0x00,
            ])
        );
    }

    #[test]
    fn gaming_mode_is_writable_only_on_the_a3876() {
        let a3876 = ModelDriver::A3876
            .decode_confirmed_state(body(A3876_STATE))
            .expect("confirmed A3876 state");
        assert_eq!(a3876.gaming_mode_write(true), Ok([1]));
        assert_eq!(a3876.gaming_mode_write(false), Ok([0]));

        let a3330 = ModelDriver::A3330
            .decode_confirmed_state(body(A3330_STATE))
            .expect("confirmed A3330 state");
        assert_eq!(
            a3330.gaming_mode_write(true),
            Err(ModelError::InvalidGamingModeWrite)
        );
    }

    #[test]
    fn auto_power_off_follows_each_model_duration_table() {
        for (fixture, model) in [
            (A3876_STATE, ModelDriver::A3876),
            (D1101_STATE, ModelDriver::D1101),
        ] {
            let state = model
                .decode_confirmed_state(body(fixture))
                .expect("confirmed state");
            let duration = state.auto_power_off.expect("reported value").duration;
            let write = state
                .auto_power_off_write(AutoPowerOffSetting::Disabled)
                .expect("supported disable");
            assert_eq!(write.body(), [0, duration]);
            assert_eq!(
                write.expected(),
                AutoPowerOff {
                    enabled: false,
                    duration,
                }
            );
            let write = state
                .auto_power_off_write(AutoPowerOffSetting::Enabled(3))
                .expect("last supported duration");
            assert_eq!(write.body(), [1, 3]);
            assert_eq!(
                state.auto_power_off_write(AutoPowerOffSetting::Enabled(4)),
                Err(ModelError::InvalidAutoPowerOffWrite)
            );
        }

        let a3330 = ModelDriver::A3330
            .decode_confirmed_state(body(A3330_STATE))
            .expect("confirmed A3330 state");
        assert_eq!(
            a3330.auto_power_off_write(AutoPowerOffSetting::Disabled),
            Err(ModelError::InvalidAutoPowerOffWrite)
        );
    }

    #[test]
    fn buttons_enabled_and_ldac_are_model_specific_writes() {
        let d1101 = ModelDriver::D1101
            .decode_confirmed_state(body(D1101_STATE))
            .expect("confirmed D1101 state");
        // The wire flag is inverted: enabling the buttons writes zero.
        assert_eq!(d1101.buttons_enabled_write(true), Ok([0]));
        assert_eq!(d1101.buttons_enabled_write(false), Ok([1]));
        assert_eq!(d1101.ldac_write(true), Ok([1]));
        assert_eq!(d1101.ldac_write(false), Ok([0]));

        let a3330 = ModelDriver::A3330
            .decode_confirmed_state(body(A3330_STATE))
            .expect("confirmed A3330 state");
        assert_eq!(a3330.buttons_enabled_write(false), Ok([1]));
        assert_eq!(a3330.ldac_write(true), Err(ModelError::InvalidLdacWrite));

        let a3876 = ModelDriver::A3876
            .decode_confirmed_state(body(A3876_STATE))
            .expect("confirmed A3876 state");
        assert_eq!(
            a3876.buttons_enabled_write(true),
            Err(ModelError::InvalidButtonsEnabledWrite)
        );
        assert_eq!(a3876.ldac_write(true), Err(ModelError::InvalidLdacWrite));
    }

    #[test]
    fn capabilities_separate_the_paired_models() {
        let a3330 = ModelDriver::A3330.capabilities();
        assert!(a3330.equalizer_write);
        assert!(a3330.buttons_enabled_write);
        assert!(a3330.case_battery);
        assert!(!a3330.ldac_write);
        assert!(!a3330.colorful_lights);
        assert!(!a3330.gaming_mode_write);

        let a3388 = ModelDriver::A3388.capabilities();
        assert!(a3388.case_battery);
        assert!(!a3388.ldac_write);

        let a3876 = ModelDriver::A3876.capabilities();
        assert!(a3876.equalizer_write);
        assert!(a3876.gaming_mode_write);
        assert!(a3876.auto_power_off_write);
        assert!(a3876.colorful_lights);
        assert!(a3876.volume_balance);
        assert!(!a3876.case_battery);
        assert!(!a3876.buttons_enabled_write);

        let d1101 = ModelDriver::D1101.capabilities();
        assert!(d1101.equalizer_write);
        assert!(d1101.auto_power_off_write);
        assert!(d1101.ldac_write);
        assert!(d1101.buttons_enabled_write);
        assert!(!d1101.case_battery);
        assert!(!d1101.gaming_mode_write);
    }

    #[test]
    fn the_paired_models_keep_the_ten_band_path_and_no_sound_mode() {
        let state = ModelDriver::A3330
            .decode_confirmed_state(body(A3330_STATE))
            .expect("confirmed A3330 state");
        assert_eq!(state.sound_mode, None);
        assert_eq!(ModelDriver::A3330.equalizer_bands(), 10);
        assert!(ModelDriver::A3330.equalizer_is_writable(REFERENCE_CURVE));
        assert_eq!(
            ModelDriver::A3330
                .equalizer_write_body(
                    EqualizerSetting {
                        preset_id: 0,
                        volume_adjustments: [0; 10],
                    },
                    body(A3330_STATE),
                )
                .map(|body| body.len()),
            Ok(22)
        );
    }

    #[test]
    fn rejects_unrecognized_tws_host_values() {
        let mut body = body(A3330_STATE).to_vec();
        body[0] = 2;
        assert_eq!(
            ModelDriver::A3330.decode_state(&body),
            Err(ModelError::InvalidStateValue)
        );
        assert!(ModelDriver::A3330.decode_confirmed_state(&body).is_none());
    }

    #[test]
    fn absent_equalizer_block_reports_no_curve() {
        let mut body = body(A3330_STATE).to_vec();
        body[52] = u8::MAX;
        let state = ModelDriver::A3330
            .decode_confirmed_state(&body)
            .expect("the rest of the state is readable");
        assert_eq!(state.equalizer, None);
    }

    #[test]
    fn paired_models_have_no_sound_mode_command() {
        assert_eq!(
            ModelDriver::A3330.decode_state(&[0; 10]),
            Err(ModelError::IncompleteState)
        );
        assert_eq!(SoundMode::Normal.code(), "Normal");
        assert!(!ModelDriver::A3876.capabilities().ambient_sound_mode_write);
    }
}
