#![forbid(unsafe_code)]

#[cfg(test)]
mod tests {
    use openscl4_models::{
        A3944_EQUALIZER_BANDS, A3944_EQUALIZER_PRESET, A3945_EQUALIZER_LEFT_BANDS,
        A3945_EQUALIZER_PRESET, A3945_EQUALIZER_RIGHT_BANDS, A3945_GAMING_MODE,
        A3948_EQUALIZER_BANDS, A3948_EQUALIZER_PRESET, A3949_GAMING_MODE, A3959_AUTO_POWER_OFF,
        A3959_EQUALIZER_BANDS, A3959_EQUALIZER_PRESET, A3959_GAMING_MODE, A3959_SOUND_MODE_FIELDS,
        AutoPowerOff, AutoPowerOffSetting, EqualizerSetting, ModelDriver, ModelError, SoundMode,
        registry,
    };

    const A3944_FRAME: &[u8] = include_bytes!("../../../data/a3944-pilot-state.bin");
    const A3945_FRAME: &[u8] = include_bytes!("../../../data/a3945-pilot-state.bin");
    const A3948_FRAME: &[u8] = include_bytes!("../../../data/a3948-pilot-state.bin");
    const A3959_FRAME: &[u8] = include_bytes!("../../../data/a3959-pilot-state.bin");
    const A3959_OLD_FIRMWARE_FRAME: &[u8] =
        include_bytes!("../../../data/a3959-gaming-unavailable-state.bin");

    /// The reference curve requested for the A3959 legacy equalizer test.
    const A3959_REQUESTED_CURVE: EqualizerSetting = EqualizerSetting {
        preset_id: 0xfefe,
        volume_adjustments: [-19, 0, 41, 51, 51, 32, 13, -35, 0, 0],
    };

    #[test]
    fn identifies_each_public_code_from_its_state_response() {
        for (frame, model) in [
            (A3944_FRAME, ModelDriver::A3944),
            (A3945_FRAME, ModelDriver::A3945),
            (A3948_FRAME, ModelDriver::A3948),
            (A3959_FRAME, ModelDriver::A3959),
            (A3959_OLD_FIRMWARE_FRAME, ModelDriver::A3959),
        ] {
            let body = state_body(frame);
            assert_eq!(registry::identify_state(body), Ok(model));
            assert_eq!(
                model.decode_confirmed_state(body).map(|state| state.model),
                Some(model)
            );
        }
        for (code, model) in [
            ("A3944", ModelDriver::A3944),
            ("A3945", ModelDriver::A3945),
            ("A3948", ModelDriver::A3948),
            ("A3959", ModelDriver::A3959),
        ] {
            assert_eq!(registry::by_code(code), Some(model));
        }
    }

    #[test]
    fn rejects_a_state_whose_serial_names_another_code() {
        let mut body = state_body(A3944_FRAME).to_vec();
        body[16..32].copy_from_slice(b"3945TEST00000000");
        assert_eq!(
            ModelDriver::A3944.decode_state(&body),
            Err(ModelError::UnknownModel)
        );
        assert!(ModelDriver::A3944.decode_confirmed_state(&body).is_none());
    }

    #[test]
    fn rejects_a_state_with_an_unregistered_serial() {
        let mut body = state_body(A3948_FRAME).to_vec();
        body[16..32].copy_from_slice(b"9999UNKNOWN00000");
        assert_eq!(
            registry::identify_state(&body),
            Err(ModelError::UnknownModel)
        );
    }

    #[test]
    fn admits_only_the_documented_body_prefixes() {
        for (model, frame, documented) in [
            (ModelDriver::A3944, A3944_FRAME, 73),
            (ModelDriver::A3945, A3945_FRAME, 72),
            (ModelDriver::A3948, A3948_FRAME, 88),
            (ModelDriver::A3959, A3959_FRAME, 91),
        ] {
            let body = state_body(frame);
            let mut truncated = body[..documented - 1].to_vec();
            assert!(
                model.decode_confirmed_state(&truncated).is_none(),
                "{} accepted a shortened body",
                model.code()
            );
            assert_eq!(
                model.decode_state(&truncated),
                Err(ModelError::IncompleteState)
            );
            // The A3944 and A3945 parsers stop before the example's trailing
            // bytes, so a body of at least the documented length is accepted.
            if model != ModelDriver::A3948 && model != ModelDriver::A3959 {
                truncated.push(0);
                assert!(
                    model.decode_confirmed_state(&truncated).is_some(),
                    "{} rejected the documented prefix",
                    model.code()
                );
            }
        }
        for (model, frame) in [
            (ModelDriver::A3948, A3948_FRAME),
            (ModelDriver::A3959, A3959_FRAME),
        ] {
            let mut extended = state_body(frame).to_vec();
            extended.push(0);
            assert!(
                model.decode_confirmed_state(&extended).is_none(),
                "{} accepted a trailing byte",
                model.code()
            );
            assert!(
                model.decode_state(&extended).is_ok(),
                "{} tolerant decoder rejected a trailing byte",
                model.code()
            );
        }
    }

    #[test]
    fn reads_the_reference_state_of_each_model() {
        let a3944 = ModelDriver::A3944
            .decode_confirmed_state(state_body(A3944_FRAME))
            .expect("A3944 confirmed state");
        assert_eq!(a3944.battery.left, Some(100));
        assert_eq!(a3944.battery.right, Some(100));
        assert_eq!(a3944.battery.case, None);
        assert_eq!(firmware(&a3944), versions("02.27", "02.27"));
        assert_eq!(
            a3944.equalizer,
            Some(EqualizerSetting {
                preset_id: 0,
                volume_adjustments: [0, 0, 0, 0, 0, 0, 0, 0, 0, -120],
            })
        );
        assert_eq!(a3944.sound_mode, None);
        assert_eq!(a3944.gaming_mode, None);
        assert_eq!(a3944.auto_power_off, None);

        let a3945 = ModelDriver::A3945
            .decode_confirmed_state(state_body(A3945_FRAME))
            .expect("A3945 confirmed state");
        assert_eq!(a3945.battery.left, Some(0));
        assert_eq!(a3945.battery.right, Some(0));
        assert_eq!(a3945.battery.case, Some(0));
        assert_eq!(firmware(&a3945), versions("00.00", "00.00"));
        // The two equalizer channels carry different curves, which the new
        // state cannot represent as one curve.
        assert_eq!(a3945.equalizer, None);
        assert_eq!(a3945.sound_mode, None);
        assert_eq!(a3945.gaming_mode, Some(false));

        let a3948 = ModelDriver::A3948
            .decode_confirmed_state(state_body(A3948_FRAME))
            .expect("A3948 confirmed state");
        assert_eq!(a3948.battery.left, Some(100));
        assert_eq!(a3948.battery.right, Some(100));
        assert_eq!(a3948.battery.case, None);
        assert_eq!(firmware(&a3948), versions("24.56", "24.56"));
        assert_eq!(
            a3948.equalizer,
            Some(EqualizerSetting {
                preset_id: 2,
                volume_adjustments: [40, 30, 10, 0, 0, 0, 0, 0, 0, -120],
            })
        );

        let a3959 = ModelDriver::A3959
            .decode_confirmed_state(state_body(A3959_FRAME))
            .expect("A3959 confirmed state");
        assert_eq!(a3959.battery.left, Some(50));
        assert_eq!(a3959.battery.right, Some(60));
        assert_eq!(a3959.battery.case, None);
        assert_eq!(firmware(&a3959), versions("01.64", "01.64"));
        assert_eq!(
            a3959.equalizer,
            Some(EqualizerSetting {
                preset_id: 0xfefe,
                volume_adjustments: [-19, 0, 41, 51, 51, 32, 24, 59, 0, 0],
            })
        );
        assert_eq!(a3959.sound_mode, Some(SoundMode::NoiseCanceling));
        assert_eq!(a3959.gaming_mode, Some(false));
        assert_eq!(
            a3959.auto_power_off,
            Some(AutoPowerOff {
                enabled: true,
                duration: 0,
            })
        );
    }

    #[test]
    fn reads_each_public_code_capability_set() {
        for (model, ambient, equalizer, gaming, auto_power_off) in [
            (ModelDriver::A3944, false, true, false, false),
            (ModelDriver::A3945, false, true, true, false),
            (ModelDriver::A3948, false, true, false, false),
            (ModelDriver::A3959, true, true, true, true),
        ] {
            let capabilities = model.capabilities();
            assert!(!capabilities.positioned_sound_mode_write);
            assert_eq!(capabilities.ambient_sound_mode_write, ambient);
            assert_eq!(capabilities.equalizer_write, equalizer);
            assert_eq!(capabilities.gaming_mode_write, gaming);
            assert_eq!(capabilities.auto_power_off_write, auto_power_off);
            assert_eq!(model.equalizer_bands(), 10);
        }
        assert_eq!(ModelDriver::A3028.auto_power_off_duration_count(), 4);
        assert_eq!(ModelDriver::A3959.auto_power_off_duration_count(), 4);
        for model in [ModelDriver::A3944, ModelDriver::A3945, ModelDriver::A3948] {
            assert_eq!(model.auto_power_off_duration_count(), 0);
        }
    }

    #[test]
    fn auto_power_off_writes_keep_the_disabled_duration_index() {
        let state = ModelDriver::A3959
            .decode_confirmed_state(state_body(A3959_FRAME))
            .expect("A3959 confirmed state");
        let write = state
            .auto_power_off_write(AutoPowerOffSetting::Disabled)
            .expect("supported disable");
        assert_eq!(write.body(), [0, 0]);
        assert_eq!(
            write.expected(),
            AutoPowerOff {
                enabled: false,
                duration: 0,
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
        for index in [4, u8::MAX] {
            assert_eq!(
                state.auto_power_off_write(AutoPowerOffSetting::Enabled(index)),
                Err(ModelError::InvalidAutoPowerOffWrite)
            );
        }
        for model in [ModelDriver::A3944, ModelDriver::A3945, ModelDriver::A3948] {
            assert_eq!(
                state_with_model(model).auto_power_off_write(AutoPowerOffSetting::Disabled),
                Err(ModelError::InvalidAutoPowerOffWrite)
            );
        }
    }

    #[test]
    fn rejects_airplane_and_unrecognized_sound_modes_in_the_a3959_state() {
        for (offset, value) in [(64, 3), (64, u8::MAX), (67, 3), (67, u8::MAX), (70, 4)] {
            let mut body = state_body(A3959_FRAME).to_vec();
            body[offset] = value;
            assert_eq!(
                ModelDriver::A3959.decode_state(&body),
                Err(ModelError::UnknownSoundMode),
                "byte {value} at {offset}"
            );
            assert!(ModelDriver::A3959.decode_confirmed_state(&body).is_none());
        }
    }

    #[test]
    fn rejects_a3959_auto_power_off_bytes_it_cannot_confirm() {
        for (offset, value) in [(75, 2), (75, u8::MAX), (76, 4), (76, u8::MAX)] {
            let mut body = state_body(A3959_FRAME).to_vec();
            body[offset] = value;
            let state = ModelDriver::A3959
                .decode_confirmed_state(&body)
                .expect("the rest of the state is still readable");
            assert_eq!(
                state.auto_power_off, None,
                "unrecognized byte {value} at {offset}"
            );
            assert_eq!(
                state.auto_power_off_write(AutoPowerOffSetting::Disabled),
                Err(ModelError::InvalidAutoPowerOffWrite)
            );
        }
    }

    #[test]
    fn gaming_mode_needs_both_a3959_firmware_versions_at_least_01_60() {
        // The captured state reports 01.64 on both sides.
        assert_eq!(
            ModelDriver::A3959
                .decode_confirmed_state(state_body(A3959_FRAME))
                .expect("01.64 state")
                .gaming_mode,
            Some(false)
        );
        // The captured 01.59 state has no gaming-mode flag.
        assert_eq!(
            ModelDriver::A3959
                .decode_confirmed_state(state_body(A3959_OLD_FIRMWARE_FRAME))
                .expect("01.59 state")
                .gaming_mode,
            None
        );
        // Both sides of the boundary: 01.59 hides the flag, 01.60 exposes it.
        for (version, expected) in [(b"01.59", None), (b"01.60", Some(false))] {
            let mut body = state_body(A3959_FRAME).to_vec();
            body[6..11].copy_from_slice(version);
            body[11..16].copy_from_slice(version);
            assert_eq!(
                ModelDriver::A3959
                    .decode_confirmed_state(&body)
                    .expect("boundary state")
                    .gaming_mode,
                expected,
                "{version:?}"
            );
        }
        // A single older earbud is enough to hide the flag.
        let mut mixed = state_body(A3959_FRAME).to_vec();
        mixed[11..16].copy_from_slice(b"01.59");
        assert_eq!(
            ModelDriver::A3959
                .decode_confirmed_state(&mixed)
                .expect("mixed state")
                .gaming_mode,
            None
        );
        // An unrecognized flag byte is not a confirmed value.
        let mut unrecognized = state_body(A3959_FRAME).to_vec();
        unrecognized[A3959_GAMING_MODE] = 2;
        assert_eq!(
            ModelDriver::A3959
                .decode_confirmed_state(&unrecognized)
                .expect("state with a strange flag")
                .gaming_mode,
            None
        );
    }

    #[test]
    fn a3945_reads_the_gaming_mode_flag_and_reports_unknown_bytes_as_unavailable() {
        let body = state_body(A3945_FRAME);
        assert_eq!(
            ModelDriver::A3945
                .decode_confirmed_state(body)
                .expect("A3945 confirmed state")
                .gaming_mode,
            Some(false)
        );
        for value in [1, 2, u8::MAX] {
            let mut body = body.to_vec();
            body[A3945_GAMING_MODE] = value;
            let state = ModelDriver::A3945
                .decode_confirmed_state(&body)
                .expect("the rest of the state is still readable");
            assert_eq!(
                state.gaming_mode,
                if value == 1 { Some(true) } else { None }
            );
        }
    }

    #[test]
    fn a3945_reports_the_case_battery_and_absent_markers() {
        let mut body = state_body(A3945_FRAME).to_vec();
        body[69] = 5;
        assert_eq!(
            ModelDriver::A3945
                .decode_confirmed_state(&body)
                .expect("case battery")
                .battery
                .case,
            Some(100)
        );
        body[69] = u8::MAX;
        assert_eq!(
            ModelDriver::A3945
                .decode_confirmed_state(&body)
                .expect("absent case battery")
                .battery
                .case,
            None
        );
        // One earbud missing is reported as an absent percentage.
        let mut one_earbud = state_body(A3948_FRAME).to_vec();
        one_earbud[3] = u8::MAX;
        one_earbud[1] = 0;
        let state = ModelDriver::A3948
            .decode_confirmed_state(&one_earbud)
            .expect("one-earbud state");
        assert_eq!(state.battery.left, Some(100));
        assert_eq!(state.battery.right, None);
    }

    #[test]
    fn a3945_reports_the_shared_curve_only_when_both_channels_match() {
        let mut body = state_body(A3945_FRAME).to_vec();
        let left: [u8; 10] = body[A3945_EQUALIZER_LEFT_BANDS]
            .try_into()
            .expect("left band block");
        body[A3945_EQUALIZER_RIGHT_BANDS].copy_from_slice(&left);
        body[A3945_EQUALIZER_PRESET].copy_from_slice(&0x0015_u16.to_le_bytes());
        let state = ModelDriver::A3945
            .decode_confirmed_state(&body)
            .expect("matching channels");
        assert_eq!(
            state.equalizer,
            Some(EqualizerSetting {
                preset_id: 0x0015,
                volume_adjustments: [0, 0, 0, 0, 0, 0, 0, 0, 1, 2],
            })
        );
        body[A3945_EQUALIZER_LEFT_BANDS.start] = u8::MAX;
        assert_eq!(
            ModelDriver::A3945
                .decode_confirmed_state(&body)
                .expect("absent channel")
                .equalizer,
            None
        );
    }

    #[test]
    fn family_equalizer_writes_match_the_legacy_bodies() {
        // The A3944 legacy test writes the Acoustic preset and expects this body.
        let state = ModelDriver::A3944
            .decode_confirmed_state(state_body(A3944_FRAME))
            .expect("A3944 confirmed state");
        let acoustic = EqualizerSetting {
            preset_id: 1,
            volume_adjustments: [40, 10, 20, 20, 40, 40, 40, 20, 0, -120],
        };
        assert_eq!(
            state
                .model
                .equalizer_write_body(acoustic, state_body(A3944_FRAME)),
            Ok(vec![
                1, 0, 160, 130, 140, 140, 160, 160, 160, 140, 120, 0, 125, 118, 123, 120, 124, 122,
                124, 121, 120, 0,
            ])
        );

        // The A3945 legacy test writes TrebleReducer to both channels.
        let treble_reducer = EqualizerSetting {
            preset_id: 0x0015,
            volume_adjustments: [0, 0, 0, -20, -30, -40, -40, -60, 0, -120],
        };
        let a3945_state = ModelDriver::A3945
            .decode_confirmed_state(state_body(A3945_FRAME))
            .expect("A3945 confirmed state");
        assert_eq!(
            a3945_state
                .model
                .equalizer_write_body(treble_reducer, state_body(A3945_FRAME)),
            Ok(vec![
                0x15, 0, 0x78, 0x78, 0x78, 0x64, 0x5a, 0x50, 0x50, 0x3c, 0x78, 0, 0x78, 0x78, 0x78,
                0x64, 0x5a, 0x50, 0x50, 0x3c, 0x78, 0,
            ])
        );

        // The A3948 write is derived from the legacy DRC builder; the same
        // transform reproduces the two captured legacy examples below.
        let a3948_curve = EqualizerSetting {
            preset_id: 2,
            volume_adjustments: [40, 30, 10, 0, 0, 0, 0, 0, 0, -120],
        };
        assert_eq!(
            ModelDriver::A3948
                .decode_confirmed_state(state_body(A3948_FRAME))
                .expect("A3948 confirmed state")
                .model
                .equalizer_write_body(a3948_curve, state_body(A3948_FRAME)),
            Ok(vec![
                2, 0, 160, 150, 130, 120, 120, 120, 120, 120, 120, 0, 123, 122, 120, 120, 120, 120,
                120, 120, 120, 0,
            ])
        );

        // The A3959 legacy test expects this body for the requested curve.
        let a3959_state = ModelDriver::A3959
            .decode_confirmed_state(state_body(A3959_FRAME))
            .expect("A3959 confirmed state");
        assert_eq!(
            a3959_state
                .model
                .equalizer_write_body(A3959_REQUESTED_CURVE, state_body(A3959_FRAME)),
            Ok(vec![
                254, 254, 101, 120, 161, 171, 171, 152, 133, 85, 120, 120, 118, 119, 124, 123, 124,
                121, 123, 114, 120, 0,
            ])
        );
    }

    #[test]
    fn equalizer_writes_reject_the_tws_disconnected_state() {
        for (model, frame) in [
            (ModelDriver::A3945, A3945_FRAME),
            (ModelDriver::A3948, A3948_FRAME),
            (ModelDriver::A3959, A3959_FRAME),
        ] {
            let mut body = state_body(frame).to_vec();
            body[1] = 0;
            let setting = EqualizerSetting {
                preset_id: 0,
                volume_adjustments: [0; 10],
            };
            assert_eq!(
                model.equalizer_write_body(setting, &body),
                Err(ModelError::InvalidEqualizerWrite),
                "{}",
                model.code()
            );
        }
        // The A3944 module has no TWS guard, matching the legacy driver.
        let mut body = state_body(A3944_FRAME).to_vec();
        body[1] = 0;
        ModelDriver::A3944
            .equalizer_write_body(
                EqualizerSetting {
                    preset_id: 0,
                    volume_adjustments: [0; 10],
                },
                &body,
            )
            .expect("the A3944 module writes while its earbuds are disconnected");
    }

    #[test]
    fn equalizer_writes_reject_unrepresentable_gains() {
        let state = state_body(A3944_FRAME);
        for invalid in [i16::MIN, -121, 135, i16::MAX] {
            let mut setting = EqualizerSetting {
                preset_id: 0,
                volume_adjustments: [0; 10],
            };
            setting.volume_adjustments[0] = invalid;
            assert_eq!(
                ModelDriver::A3944.equalizer_write_body(setting, state),
                Err(ModelError::InvalidEqualizerWrite)
            );
        }
    }

    #[test]
    fn a3959_ambient_mode_writes_preserve_the_five_controls() {
        let state = ModelDriver::A3959
            .decode_confirmed_state(state_body(A3959_FRAME))
            .expect("A3959 confirmed state");
        assert_eq!(
            state.a3959_sound_mode_write(SoundMode::Normal),
            Ok([0x02, 0x55, 0x02, 0x00, 0x01, 0xff, 0x01])
        );
        assert_eq!(
            state.a3959_sound_mode_write(SoundMode::NoiseCanceling),
            Ok([0x00, 0x55, 0x00, 0x00, 0x01, 0xff, 0x01])
        );
        assert_eq!(
            state.a3959_sound_mode_write(SoundMode::AirplaneMode),
            Err(ModelError::InvalidSoundModeWrite)
        );
        // The A3944, A3945 and A3948 layouts carry no sound modes.
        for model in [ModelDriver::A3944, ModelDriver::A3945, ModelDriver::A3948] {
            assert_eq!(
                state_with_model(model).ambient_sound_mode_write(SoundMode::Normal),
                Err(ModelError::InvalidSoundModeWrite)
            );
        }
        // The packed manual and adaptive strengths are clamped like the legacy
        // structures, so the write always carries an accepted value.
        let mut body = state_body(A3959_FRAME).to_vec();
        body[65] = 0x00;
        let state = ModelDriver::A3959
            .decode_confirmed_state(&body)
            .expect("state with unset strengths");
        assert_eq!(
            state.a3959_sound_mode_write(SoundMode::Normal),
            Ok([0x02, 0x11, 0x02, 0x00, 0x01, 0xff, 0x01])
        );
        // The wind-noise byte keeps only the suppression and detection bits.
        let mut body = state_body(A3959_FRAME).to_vec();
        body[68] = 0x07;
        let state = ModelDriver::A3959
            .decode_confirmed_state(&body)
            .expect("state with extra wind-noise bits");
        assert_eq!(
            state.a3959_sound_mode_write(SoundMode::Normal),
            Ok([0x02, 0x55, 0x02, 0x00, 0x03, 0xff, 0x01])
        );
    }

    #[test]
    fn gaming_mode_writes_are_limited_to_models_that_report_a_flag() {
        let a3945 = state_with_model(ModelDriver::A3945);
        assert_eq!(a3945.gaming_mode_write(true), Ok([1]));
        assert_eq!(a3945.gaming_mode_write(false), Ok([0]));
        let old_firmware = ModelDriver::A3959
            .decode_confirmed_state(state_body(A3959_OLD_FIRMWARE_FRAME))
            .expect("01.59 state");
        assert_eq!(old_firmware.gaming_mode, None);
        assert_eq!(
            old_firmware.gaming_mode_write(true),
            Err(ModelError::InvalidGamingModeWrite)
        );
        let a3944 = state_with_model(ModelDriver::A3944);
        assert_eq!(
            a3944.gaming_mode_write(true),
            Err(ModelError::InvalidGamingModeWrite)
        );
    }

    #[test]
    fn the_family_reports_its_measured_offsets() {
        assert_eq!(A3944_EQUALIZER_PRESET, 32..34);
        assert_eq!(A3944_EQUALIZER_BANDS, 34..44);
        assert_eq!(A3945_EQUALIZER_PRESET, 32..34);
        assert_eq!(A3945_EQUALIZER_LEFT_BANDS, 34..44);
        assert_eq!(A3945_EQUALIZER_RIGHT_BANDS, 44..54);
        assert_eq!(A3945_GAMING_MODE, 68);
        assert_eq!(A3948_EQUALIZER_PRESET, 32..34);
        assert_eq!(A3948_EQUALIZER_BANDS, 34..44);
        assert_eq!(A3959_EQUALIZER_PRESET, 32..34);
        assert_eq!(A3959_EQUALIZER_BANDS, 34..44);
        assert_eq!(A3959_SOUND_MODE_FIELDS, 64..71);
        assert_eq!(A3959_AUTO_POWER_OFF, 75..77);
        assert_eq!(A3959_GAMING_MODE, 78);
        assert_eq!(A3949_GAMING_MODE, 71);
    }

    fn state_body(frame: &[u8]) -> &[u8] {
        assert_eq!(frame[5..7], [0x01, 0x01]);
        assert_eq!(
            usize::from(u16::from_le_bytes([frame[7], frame[8]])),
            frame.len()
        );
        &frame[9..frame.len() - 1]
    }

    fn firmware(state: &openscl4_models::DeviceState) -> (Option<String>, Option<String>) {
        (
            state.firmware_left.map(|version| version.to_string()),
            state.firmware_right.map(|version| version.to_string()),
        )
    }

    fn versions(left: &str, right: &str) -> (Option<String>, Option<String>) {
        (Some(left.to_owned()), Some(right.to_owned()))
    }

    fn state_with_model(model: ModelDriver) -> openscl4_models::DeviceState {
        model
            .decode_confirmed_state(state_body(match model {
                ModelDriver::A3944 => A3944_FRAME,
                ModelDriver::A3945 => A3945_FRAME,
                ModelDriver::A3948 => A3948_FRAME,
                _ => A3959_FRAME,
            }))
            .expect("confirmed family state")
    }
}
