#![forbid(unsafe_code)]

#[cfg(test)]
mod tests {
    use openscl4_models::{
        A3004_EQUALIZER_BANDS, A3004_EQUALIZER_PRESET, A3005_AUTO_POWER_OFF,
        A3005_DUAL_CONNECTIONS, A3005_EQUALIZER_BANDS, A3005_EQUALIZER_PRESET,
        A3033_EQUALIZER_BANDS, A3033_EQUALIZER_PRESET, A3033_WEARING_DETECTION, AutoPowerOff,
        DUAL_CONNECTIONS_COMMAND, EQUALIZER_WITH_DRC_COMMAND, EqualizerSetting, ModelDriver,
        ModelError, SoundMode, WEARING_DETECTION_COMMAND, registry,
    };

    /// Complete inbound `01 01` frames from `core/data`.
    const A3004_STATE: &[u8] = include_bytes!("../../../data/a3004-pilot-state.bin");
    const A3005_STATE: &[u8] = include_bytes!("../../../data/a3005-pilot-state.bin");
    const A3033_STATE: &[u8] = include_bytes!("../../../data/a3033-pilot-state.bin");

    /// The legacy `set_equalizer` example curve and its eight-band write body.
    const LEGACY_EIGHT_BAND_CURVE: [i16; 10] = [-60, 60, 23, 40, 22, 60, -4, 16, 0, 0];
    const LEGACY_EIGHT_BAND_BODY: [u8; 10] =
        [0xfe, 0xfe, 0x3c, 0xb4, 0x8f, 0xa0, 0x8e, 0xb4, 0x74, 0x88];

    /// The legacy `set_equalizer_with_drc` example curve (a ten-band curve with
    /// the family defaults in the last two bands) and its DRC block.
    const LEGACY_DRC_CURVE: [i16; 10] = [-60, 60, 23, 120, 22, -120, -4, 16, 0, -120];
    const LEGACY_DRC_BODY: [u8; 22] = [
        0xfe, 0xfe, 0x3c, 0xb4, 0x8f, 0xf0, 0x8e, 0x00, 0x74, 0x88, 0x78, 0x00, 0x6d, 0x86, 0x70,
        0x88, 0x7b, 0x66, 0x7e, 0x79, 0x78, 0x00,
    ];

    /// One complete inbound frame is nine header bytes, the body, and a checksum.
    fn state_body(fixture: &[u8]) -> &[u8] {
        assert_eq!(fixture[5..7], [0x01, 0x01]);
        &fixture[9..fixture.len() - 1]
    }

    #[test]
    fn identifies_all_three_codes_from_a_serial_identity() {
        for (code, model) in [
            ("3004TEST00000000", ModelDriver::A3004),
            ("3005TEST00000000", ModelDriver::A3005),
            ("3033TEST00000000", ModelDriver::A3033),
        ] {
            let mut body = b"00.2600.26".to_vec();
            body.extend_from_slice(code.as_bytes());
            assert_eq!(body.len(), 26);
            assert_eq!(ModelDriver::identify(&body), Ok(model));
        }
    }

    #[test]
    fn identifies_all_three_codes_from_their_measured_state() {
        assert_eq!(
            registry::identify_state(state_body(A3004_STATE)),
            Ok(ModelDriver::A3004)
        );
        assert_eq!(
            registry::identify_state(state_body(A3005_STATE)),
            Ok(ModelDriver::A3005)
        );
        assert_eq!(
            registry::identify_state(state_body(A3033_STATE)),
            Ok(ModelDriver::A3033)
        );
        for code in ["A3004", "A3005", "A3033"] {
            assert!(registry::by_code(code).is_some(), "{code} is registered");
        }
        assert_eq!(registry::DRIVERS.len(), 8);
    }

    #[test]
    fn reads_the_captured_a3004_state() {
        let model = ModelDriver::A3004;
        let body = state_body(A3004_STATE);
        assert_eq!(body.len(), 61);
        let state = model.decode_confirmed_state(body).expect("captured state");
        assert_eq!(state.model, model);
        assert_eq!(state.battery.device, Some(80));
        assert_eq!(
            state.firmware_device.map(|firmware| firmware.to_string()),
            Some("00.26".to_owned())
        );
        assert_eq!(state.sound_mode, Some(SoundMode::NoiseCanceling));
        assert_eq!(
            state.equalizer,
            Some(EqualizerSetting {
                preset_id: 0xfefe,
                volume_adjustments: [0; 10],
            })
        );
        assert_eq!(state.auto_power_off, None);
        assert_eq!(state.wearing_detection, None);
        assert_eq!(state.dual_connections, None);
    }

    #[test]
    fn reads_the_captured_a3005_state() {
        let model = ModelDriver::A3005;
        let body = state_body(A3005_STATE);
        assert_eq!(body.len(), 55);
        let state = model.decode_confirmed_state(body).expect("captured state");
        assert_eq!(state.model, model);
        assert_eq!(state.battery.device, Some(80));
        assert_eq!(
            state.firmware_device.map(|firmware| firmware.to_string()),
            Some("01.21".to_owned())
        );
        assert_eq!(state.sound_mode, None);
        assert_eq!(
            state.equalizer,
            Some(EqualizerSetting {
                preset_id: 0,
                volume_adjustments: [0, 0, 0, 0, 0, 0, 0, 0, 0, -120],
            })
        );
        assert_eq!(state.dual_connections, Some(true));
        assert_eq!(
            state.auto_power_off,
            Some(AutoPowerOff {
                enabled: true,
                duration: 1,
            })
        );
        assert_eq!(state.wearing_detection, None);
    }

    #[test]
    fn reads_the_derived_a3033_state() {
        let model = ModelDriver::A3033;
        let body = state_body(A3033_STATE);
        assert_eq!(body.len(), 34);
        let state = model.decode_confirmed_state(body).expect("reference state");
        assert_eq!(state.model, model);
        assert_eq!(state.battery.device, Some(80));
        assert_eq!(
            state.firmware_device.map(|firmware| firmware.to_string()),
            Some("01.21".to_owned())
        );
        assert_eq!(state.sound_mode, None);
        assert_eq!(
            state.equalizer,
            Some(EqualizerSetting {
                preset_id: 0,
                volume_adjustments: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            })
        );
        assert_eq!(state.wearing_detection, Some(true));
        assert_eq!(state.dual_connections, None);
    }

    #[test]
    fn confirmed_layouts_require_the_measured_body_lengths() {
        for (model, fixture, expected) in [
            (ModelDriver::A3004, A3004_STATE, 61),
            (ModelDriver::A3005, A3005_STATE, 55),
            (ModelDriver::A3033, A3033_STATE, 34),
        ] {
            let body = state_body(fixture);
            assert_eq!(body.len(), expected);
            assert!(model.decode_confirmed_state(body).is_some());
            let mut extra = body.to_vec();
            extra.push(0);
            assert!(
                model.decode_confirmed_state(&extra).is_none(),
                "{} accepted a trailing byte",
                model.code()
            );
        }
    }

    #[test]
    fn rejects_a_state_whose_serial_names_a_different_code() {
        let a3004 = ModelDriver::A3004.decode_confirmed_state(state_body(A3004_STATE));
        assert!(a3004.is_some());
        let other_serial = {
            let mut body = state_body(A3004_STATE).to_vec();
            body[7..23].copy_from_slice(b"3005TEST00000000");
            body
        };
        assert!(
            ModelDriver::A3004
                .decode_confirmed_state(&other_serial)
                .is_none()
        );
        let a3033 = {
            let mut body = state_body(A3033_STATE).to_vec();
            body[17..33].copy_from_slice(b"9999UNKNOWN00000");
            body
        };
        assert_eq!(
            registry::identify_state(&a3033),
            Err(ModelError::UnknownModel)
        );
    }

    #[test]
    fn a3004_writes_the_ambient_mode_and_keeps_the_captured_controls() {
        let state = ModelDriver::A3004
            .decode_confirmed_state(state_body(A3004_STATE))
            .expect("captured state");
        assert_eq!(
            state.a3004_sound_mode_write(SoundMode::Normal),
            Ok([0x02, 0x3f, 0x02, 0xff])
        );
        assert_eq!(
            state.a3004_sound_mode_write(SoundMode::NoiseCanceling),
            Ok([0x00, 0x3f, 0x02, 0xff])
        );
        assert_eq!(
            state.a3004_sound_mode_write(SoundMode::AirplaneMode),
            Err(ModelError::InvalidSoundModeWrite)
        );
        assert!(state.model.capabilities().ambient_sound_mode_write);
        assert!(!state.model.capabilities().auto_power_off_write);
    }

    #[test]
    fn a3004_writes_the_ten_band_curve_with_its_drc_block() {
        let state = ModelDriver::A3004
            .decode_confirmed_state(state_body(A3004_STATE))
            .expect("captured state");
        assert_eq!(
            state
                .model
                .equalizer_write_body(drc_curve(), state_body(A3004_STATE)),
            Ok(LEGACY_DRC_BODY.to_vec())
        );
        assert_eq!(state.model.equalizer_command(), EQUALIZER_WITH_DRC_COMMAND);
        assert_eq!(state.model.equalizer_bands(), 10);
        assert_eq!(
            state
                .model
                .equalizer_write_body(signature_curve(), state_body(A3004_STATE)),
            Ok(signature_body().to_vec())
        );
    }

    #[test]
    fn a3005_writes_the_curve_the_dual_connections_flag_and_auto_power_off() {
        let state = ModelDriver::A3005
            .decode_confirmed_state(state_body(A3005_STATE))
            .expect("captured state");
        assert_eq!(state.model.equalizer_command(), EQUALIZER_WITH_DRC_COMMAND);
        assert_eq!(
            state
                .model
                .equalizer_write_body(drc_curve(), state_body(A3005_STATE)),
            Ok(LEGACY_DRC_BODY.to_vec())
        );
        assert_eq!(state.dual_connections_write(false), Ok([0x00]));
        assert_eq!(state.dual_connections_write(true), Ok([0x01]));
        assert_eq!(DUAL_CONNECTIONS_COMMAND, [0x0b, 0x84]);
        let disabled = state
            .auto_power_off_write(openscl4_models::AutoPowerOffSetting::Disabled)
            .expect("supported disable");
        assert_eq!(disabled.body(), [0x00, 0x01]);
        assert_eq!(
            disabled.expected(),
            AutoPowerOff {
                enabled: false,
                duration: 1,
            }
        );
        let enabled = state
            .auto_power_off_write(openscl4_models::AutoPowerOffSetting::Enabled(3))
            .expect("last supported duration");
        assert_eq!(enabled.body(), [0x01, 0x03]);
        assert_eq!(
            enabled.expected(),
            AutoPowerOff {
                enabled: true,
                duration: 3,
            }
        );
        for index in [4, 5, u8::MAX] {
            assert_eq!(
                state.auto_power_off_write(openscl4_models::AutoPowerOffSetting::Enabled(index)),
                Err(ModelError::InvalidAutoPowerOffWrite)
            );
        }
        assert!(state.model.capabilities().dual_connections_write);
        assert!(state.model.capabilities().auto_power_off_write);
        assert!(!state.model.capabilities().ambient_sound_mode_write);
    }

    #[test]
    fn a3005_reports_unrecognized_flags_as_unconfirmed() {
        for (offset, value) in [(A3005_DUAL_CONNECTIONS, 2), (A3005_AUTO_POWER_OFF.start, 2)] {
            let mut body = state_body(A3005_STATE).to_vec();
            body[offset] = value;
            let state = ModelDriver::A3005
                .decode_confirmed_state(&body)
                .expect("the rest of the state is still readable");
            if offset == A3005_DUAL_CONNECTIONS {
                assert_eq!(state.dual_connections, None);
                assert_eq!(
                    state.dual_connections_write(false),
                    Err(ModelError::InvalidDualConnectionsWrite)
                );
            } else {
                assert_eq!(state.auto_power_off, None);
                assert_eq!(
                    state.auto_power_off_write(openscl4_models::AutoPowerOffSetting::Disabled),
                    Err(ModelError::InvalidAutoPowerOffWrite)
                );
            }
        }
        let mut body = state_body(A3005_STATE).to_vec();
        body[A3005_AUTO_POWER_OFF.start + 1] = 4;
        let state = ModelDriver::A3005
            .decode_confirmed_state(&body)
            .expect("duration byte is outside the table only");
        assert_eq!(state.auto_power_off, None);
    }

    #[test]
    fn a3033_writes_the_eight_band_curve_and_the_wearing_detection_flag() {
        let state = ModelDriver::A3033
            .decode_confirmed_state(state_body(A3033_STATE))
            .expect("reference state");
        assert_eq!(state.model.equalizer_command(), [0x02, 0x81]);
        assert_eq!(state.model.equalizer_bands(), 8);
        assert_eq!(
            state.model.equalizer_write_body(
                EqualizerSetting {
                    preset_id: 0xfefe,
                    volume_adjustments: LEGACY_EIGHT_BAND_CURVE,
                },
                state_body(A3033_STATE),
            ),
            Ok(LEGACY_EIGHT_BAND_BODY.to_vec())
        );
        assert_eq!(state.wearing_detection_write(false), Ok([0x00]));
        assert_eq!(state.wearing_detection_write(true), Ok([0x01]));
        assert_eq!(WEARING_DETECTION_COMMAND, [0x01, 0x81]);
        assert!(state.model.capabilities().wearing_detection_write);
        assert!(!state.model.capabilities().dual_connections_write);
        assert!(!state.model.capabilities().ambient_sound_mode_write);

        let mut body = state_body(A3033_STATE).to_vec();
        body[A3033_WEARING_DETECTION] = 2;
        let state = ModelDriver::A3033
            .decode_confirmed_state(&body)
            .expect("the rest of the state is still readable");
        assert_eq!(state.wearing_detection, None);
        assert_eq!(
            state.wearing_detection_write(false),
            Err(ModelError::InvalidWearingDetectionWrite)
        );
    }

    #[test]
    fn equalizer_band_offsets_match_the_documented_layouts() {
        assert_eq!(A3004_EQUALIZER_PRESET, 23..25);
        assert_eq!(A3004_EQUALIZER_BANDS, 25..35);
        assert_eq!(A3005_EQUALIZER_PRESET, 23..25);
        assert_eq!(A3005_EQUALIZER_BANDS, 25..35);
        assert_eq!(A3033_EQUALIZER_PRESET, 2..4);
        assert_eq!(A3033_EQUALIZER_BANDS, 4..12);
    }

    #[test]
    fn keeps_the_ten_band_path_for_the_previous_models() {
        assert_eq!(ModelDriver::A3028.equalizer_bands(), 8);
        assert_eq!(ModelDriver::A3062.equalizer_bands(), 10);
        assert_eq!(ModelDriver::A3062.equalizer_command(), [0x03, 0x87]);
        assert_eq!(ModelDriver::A3028.equalizer_command(), [0x02, 0x81]);
    }

    fn drc_curve() -> EqualizerSetting {
        EqualizerSetting {
            preset_id: 0xfefe,
            volume_adjustments: LEGACY_DRC_CURVE,
        }
    }

    fn signature_curve() -> EqualizerSetting {
        EqualizerSetting {
            preset_id: 0,
            volume_adjustments: [0, 0, 0, 0, 0, 0, 0, 0, 0, -120],
        }
    }

    fn signature_body() -> [u8; 22] {
        [
            0x00, 0x00, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x00, 0x78, 0x78,
            0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x00,
        ]
    }
}
