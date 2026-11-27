#![forbid(unsafe_code)]

#[cfg(test)]
mod tests {
    use openscl4_models::{
        AmbientSoundModeCycle, AutoPowerOff, AutoPowerOffSetting, ButtonAction, EqualizerSetting,
        LimitHighVolume, ModelDriver, ModelError, SoundMode, VolumeLimitRefreshRate, registry,
    };

    const A3035_STATE: &[u8] = include_bytes!("../../../data/a3035-state-reference.bin");
    const A3040_STATE: &[u8] = include_bytes!("../../../data/a3040-state-reference.bin");

    /// The measured curve in the A3035 reference: SoundcoreSignature preset 0.
    const A3035_REFERENCE_CURVE: EqualizerSetting = EqualizerSetting {
        preset_id: 0,
        volume_adjustments: [0, 0, 0, 0, 0, 0, 0, 0, 0, -120],
    };
    /// The measured curve in the A3040 reference: preset 17 (Rock).
    const A3040_REFERENCE_CURVE: EqualizerSetting = EqualizerSetting {
        preset_id: 17,
        volume_adjustments: [30, 20, -10, -10, 10, 30, 40, 50, 0, -120],
    };

    #[test]
    fn identifies_both_models_from_their_measured_state() {
        let a3035 = state_body(A3035_STATE, ModelDriver::A3035);
        let a3040 = state_body(A3040_STATE, ModelDriver::A3040);
        assert_eq!(registry::identify_state(&a3035), Ok(ModelDriver::A3035));
        assert_eq!(registry::identify_state(&a3040), Ok(ModelDriver::A3040));

        let mut unknown = a3035.clone();
        unknown[7..23].copy_from_slice(b"9999UNKNOWN00000");
        assert_eq!(
            registry::identify_state(&unknown),
            Err(ModelError::UnknownModel)
        );
    }

    #[test]
    fn rejects_a_state_whose_serial_names_the_other_model() {
        let a3035 = state_body(A3035_STATE, ModelDriver::A3035);
        assert!(ModelDriver::A3040.decode_confirmed_state(&a3035).is_none());
        let a3040 = state_body(A3040_STATE, ModelDriver::A3040);
        assert!(ModelDriver::A3035.decode_confirmed_state(&a3040).is_none());
    }

    #[test]
    fn admits_only_the_measured_state_lengths() {
        for (model, fixture, length) in [
            (ModelDriver::A3035, A3035_STATE, 95),
            (ModelDriver::A3040, A3040_STATE, 142),
        ] {
            let body = state_body(fixture, model);
            assert_eq!(body.len(), length);
            assert!(model.decode_confirmed_state(&body).is_some());
            for other in [length - 1, length + 1] {
                let mut resized = body.clone();
                resized.resize(other, 0);
                assert!(
                    model.decode_confirmed_state(&resized).is_none(),
                    "{model:?} accepted {other} bytes"
                );
            }
        }
    }

    #[test]
    fn reads_the_a3035_reference_state() {
        let state = confirmed(ModelDriver::A3035, A3035_STATE);
        assert_eq!(state.sound_mode, Some(SoundMode::NoiseCanceling));
        assert_eq!(state.battery.device, Some(100));
        assert_eq!(
            state.firmware_device.map(|version| version.to_string()),
            Some("06.87".to_owned())
        );
        assert_eq!(state.equalizer, Some(A3035_REFERENCE_CURVE));
        assert_eq!(
            state.auto_power_off,
            Some(AutoPowerOff {
                enabled: true,
                duration: 2,
            })
        );
        assert_eq!(
            state.settings.button_double_press,
            Some(ButtonAction::BassUp)
        );
        assert_eq!(
            state.settings.ambient_sound_mode_cycle,
            Some(AmbientSoundModeCycle {
                noise_canceling: true,
                transparency: true,
                normal: false,
            })
        );
        assert_eq!(
            state.settings.limit_high_volume,
            Some(LimitHighVolume {
                enabled: false,
                db_limit: 90,
                refresh_rate: VolumeLimitRefreshRate::RealTime,
            })
        );
        assert_eq!(state.settings.dual_connections, Some(false));
        assert_eq!(state.settings.ldac, Some(true));
        assert_eq!(state.settings.auto_play_pause, Some(true));
        assert_eq!(state.settings.battery_alert, Some(true));
        assert_eq!(state.settings.voice_prompt, Some(true));
        assert_eq!(state.settings.low_battery_prompt, None);
    }

    #[test]
    fn reads_the_a3040_reference_state() {
        let state = confirmed(ModelDriver::A3040, A3040_STATE);
        assert_eq!(state.sound_mode, Some(SoundMode::NoiseCanceling));
        assert_eq!(state.battery.device, Some(60));
        assert_eq!(
            state.firmware_device.map(|version| version.to_string()),
            Some("02.34".to_owned())
        );
        assert_eq!(state.equalizer, Some(A3040_REFERENCE_CURVE));
        assert_eq!(
            state.auto_power_off,
            Some(AutoPowerOff {
                enabled: true,
                duration: 0,
            })
        );
        assert_eq!(
            state.settings.button_double_press,
            Some(ButtonAction::BassUp)
        );
        assert_eq!(
            state.settings.ambient_sound_mode_cycle,
            Some(AmbientSoundModeCycle {
                noise_canceling: true,
                transparency: true,
                normal: true,
            })
        );
        assert_eq!(
            state.settings.limit_high_volume,
            Some(LimitHighVolume {
                enabled: false,
                db_limit: 90,
                refresh_rate: VolumeLimitRefreshRate::RealTime,
            })
        );
        assert_eq!(state.settings.dual_connections, Some(true));
        assert_eq!(state.settings.ldac, Some(false));
        assert_eq!(state.settings.auto_play_pause, None);
        assert_eq!(state.settings.battery_alert, None);
        assert_eq!(state.settings.voice_prompt, Some(true));
        assert_eq!(state.settings.low_battery_prompt, Some(true));
    }

    #[test]
    fn writes_the_a3035_reference_sound_modes() {
        let body = state_body(A3035_STATE, ModelDriver::A3035);
        let state = ModelDriver::A3035
            .decode_confirmed_state(&body)
            .expect("confirmed A3035 state");
        assert_eq!(
            state.ambient_sound_mode_write(SoundMode::Normal),
            Ok([0x02, 0x51, 0x02, 0x00, 0x01, 0x05])
        );
        assert_eq!(
            state.ambient_sound_mode_write(SoundMode::NoiseCanceling),
            Ok([0x00, 0x51, 0x00, 0x00, 0x01, 0x05])
        );
        assert_eq!(
            state.ambient_sound_mode_write(SoundMode::AirplaneMode),
            Err(ModelError::InvalidSoundModeWrite)
        );
    }

    #[test]
    fn writes_the_a3040_reference_sound_modes() {
        let body = state_body(A3040_STATE, ModelDriver::A3040);
        let state = ModelDriver::A3040
            .decode_confirmed_state(&body)
            .expect("confirmed A3040 state");
        assert_eq!(
            state.ambient_sound_mode_write(SoundMode::Normal),
            Ok([0x02, 0x50, 0x00, 0x01, 0x00, 0x05])
        );
        assert_eq!(
            state.ambient_sound_mode_write(SoundMode::Transparency),
            Ok([0x01, 0x50, 0x00, 0x01, 0x00, 0x05])
        );
    }

    #[test]
    fn keeps_the_legacy_clamps_of_the_packed_controls() {
        let mut body = state_body(A3035_STATE, ModelDriver::A3035);
        body[70] = 0x00;
        let state = ModelDriver::A3035
            .decode_confirmed_state(&body)
            .expect("confirmed state with packed controls");
        // Both A3035 values clamp to 1, so the unused byte stays the ambient mode.
        assert_eq!(
            state.ambient_sound_mode_write(SoundMode::Normal),
            Ok([0x02, 0x11, 0x02, 0x00, 0x01, 0x05])
        );

        let mut body = state_body(A3040_STATE, ModelDriver::A3040);
        body[54] = 3;
        let state = ModelDriver::A3040
            .decode_confirmed_state(&body)
            .expect("confirmed state with a transparency level");
        // The A3040 parser keeps the raw custom transparency byte.
        assert_eq!(
            state.ambient_sound_mode_write(SoundMode::Normal),
            Ok([0x02, 0x50, 0x00, 0x01, 0x00, 0x03])
        );
    }

    #[test]
    fn rejects_unrecognized_and_airplane_sound_modes() {
        for value in [3, 4, u8::MAX] {
            let mut body = state_body(A3035_STATE, ModelDriver::A3035);
            body[69] = value;
            assert_eq!(
                ModelDriver::A3035.decode_state(&body),
                Err(ModelError::UnknownSoundMode),
                "sound mode byte {value}"
            );
        }
    }

    #[test]
    fn writes_the_a3035_reference_custom_equalizer_body() {
        let body = state_body(A3035_STATE, ModelDriver::A3035);
        let setting = EqualizerSetting {
            preset_id: 0xfefe,
            volume_adjustments: [-60, 0, 0, 0, 0, 0, 0, 60, 0, 0],
        };
        assert_eq!(
            ModelDriver::A3035.equalizer_write_body(setting, &body),
            Ok(vec![
                254, 254, // preset
                0, 0, // favorite music genre
                60, 120, 120, 120, 120, 120, 120, 180, 120, 120, // requested curve
                255, 255, // unknown hear-id block
                0,   // hear id disabled
                255, 255, 255, 255, 255, 255, 255, 255, 255, 0, // hear-id volume
                0, 0, 0, 0, // hear-id time
                0, // hear-id type
                255, 255, 255, 255, 255, 255, 255, 255, 255, 0, // custom hear-id curve
                112, 124, 119, 121, 119, 121, 116, 129, 120, 0, // dynamic range compression
                0, // unknown
            ])
        );
    }

    #[test]
    fn writes_the_a3035_reference_preset_equalizer_body() {
        let body = state_body(A3035_STATE, ModelDriver::A3035);
        let acoustic = EqualizerSetting {
            preset_id: 1,
            volume_adjustments: [40, 10, 20, 20, 40, 40, 40, 20, 0, -120],
        };
        assert_eq!(
            ModelDriver::A3035.equalizer_write_body(acoustic, &body),
            Ok(vec![
                1, 0, // Acoustic preset
                0, 0, // favorite music genre
                160, 130, 140, 140, 160, 160, 160, 140, 120, 0, // requested curve
                255, 255, 0, // disabled hear-id block
                255, 255, 255, 255, 255, 255, 255, 255, 255, 0, // hear-id volume
                0, 0, 0, 0, // hear-id time
                0, // hear-id type
                255, 255, 255, 255, 255, 255, 255, 255, 255, 0, // custom hear-id curve
                125, 118, 123, 120, 124, 122, 124, 121, 120, 0, // dynamic range compression
                0, // unknown
            ])
        );
    }

    #[test]
    fn writes_the_a3040_reference_signature_equalizer_body() {
        let body = state_body(A3040_STATE, ModelDriver::A3040);
        let signature = EqualizerSetting {
            preset_id: 0,
            volume_adjustments: [0, 0, 0, 0, 0, 0, 0, 0, 0, -120],
        };
        let write = ModelDriver::A3040
            .equalizer_write_body(signature, &body)
            .expect("measured signature write");
        assert_eq!(write.len(), 93);
        assert_eq!(&write[0..4], &[0, 0, 17, 0]);
        assert_eq!(
            &write[4..24],
            &[
                120, 120, 120, 120, 120, 120, 120, 120, 120, 0, 120, 120, 120, 120, 120, 120, 120,
                120, 120, 0
            ]
        );
        assert_eq!(&write[24..27], &[0, 1, 0]);
        assert_eq!(
            &write[27..47],
            &[
                143, 143, 128, 138, 150, 150, 143, 143, 60, 60, 143, 143, 128, 138, 150, 150, 143,
                143, 60, 60
            ]
        );
        assert_eq!(&write[47..52], &[100, 152, 140, 205, 0]);
        assert_eq!(
            &write[52..72],
            &[
                143, 143, 128, 138, 150, 150, 143, 143, 60, 60, 143, 143, 128, 138, 150, 150, 143,
                143, 60, 60
            ]
        );
        assert_eq!(
            &write[72..92],
            &[
                120, 120, 120, 120, 120, 120, 120, 120, 120, 0, 120, 120, 120, 120, 120, 120, 120,
                120, 120, 0
            ]
        );
        assert_eq!(write[92], 0);
    }

    #[test]
    fn writes_the_a3040_reference_custom_equalizer_body() {
        let body = state_body(A3040_STATE, ModelDriver::A3040);
        let setting = EqualizerSetting {
            preset_id: 0xfefe,
            volume_adjustments: [60, 0, 0, 0, 0, 0, 0, -60, 0, 0],
        };
        let write = ModelDriver::A3040
            .equalizer_write_body(setting, &body)
            .expect("measured custom write");
        assert_eq!(&write[0..4], &[254, 254, 17, 0]);
        assert_eq!(
            &write[4..24],
            &[
                180, 120, 120, 120, 120, 120, 120, 60, 120, 120, 180, 120, 120, 120, 120, 120, 120,
                60, 120, 120
            ]
        );
        assert_eq!(&write[24..27], &[0, 1, 0]);
        assert_eq!(
            &write[72..92],
            &[
                128, 116, 121, 119, 121, 119, 124, 111, 120, 0, 128, 116, 121, 119, 121, 119, 124,
                111, 120, 0
            ]
        );
        assert_eq!(write[92], 0);
    }

    #[test]
    fn rejects_equalizer_writes_outside_the_shared_curve_range() {
        let body = state_body(A3035_STATE, ModelDriver::A3035);
        let mut setting = A3035_REFERENCE_CURVE;
        setting.volume_adjustments[0] = 135;
        assert_eq!(
            ModelDriver::A3035.equalizer_write_body(setting, &body),
            Err(ModelError::InvalidEqualizerWrite)
        );
        let short = &body[..94];
        assert_eq!(
            ModelDriver::A3035.equalizer_write_body(A3035_REFERENCE_CURVE, short),
            Err(ModelError::InvalidEqualizerWrite)
        );
    }

    #[test]
    fn writes_the_reference_auto_power_off_values() {
        let a3035 = ModelDriver::A3035
            .decode_confirmed_state(&state_body(A3035_STATE, ModelDriver::A3035))
            .expect("confirmed A3035 state");
        let disabled = a3035
            .auto_power_off_write(AutoPowerOffSetting::Disabled)
            .expect("supported disable");
        assert_eq!(disabled.body(), [0, 2]);
        assert_eq!(
            disabled.expected(),
            AutoPowerOff {
                enabled: false,
                duration: 2,
            }
        );
        let enabled = a3035
            .auto_power_off_write(AutoPowerOffSetting::Enabled(3))
            .expect("supported enable");
        assert_eq!(enabled.body(), [1, 3]);
        assert_eq!(
            enabled.expected(),
            AutoPowerOff {
                enabled: true,
                duration: 3,
            }
        );
        assert_eq!(
            a3035.auto_power_off_write(AutoPowerOffSetting::Enabled(4)),
            Err(ModelError::InvalidAutoPowerOffWrite)
        );

        let a3040 = ModelDriver::A3040
            .decode_confirmed_state(&state_body(A3040_STATE, ModelDriver::A3040))
            .expect("confirmed A3040 state");
        let disabled = a3040
            .auto_power_off_write(AutoPowerOffSetting::Disabled)
            .expect("supported disable");
        assert_eq!(disabled.body(), [0, 0]);
        let enabled = a3040
            .auto_power_off_write(AutoPowerOffSetting::Enabled(2))
            .expect("supported enable");
        assert_eq!(enabled.body(), [1, 2]);
    }

    #[test]
    fn model_capabilities_cover_the_transferred_settings() {
        let a3035 = ModelDriver::A3035.capabilities();
        assert!(a3035.ambient_sound_mode_write);
        assert!(a3035.equalizer_write);
        assert!(a3035.auto_power_off_write);
        assert!(a3035.button_configuration_write);
        assert!(a3035.ambient_sound_mode_cycle_write);
        assert!(a3035.limit_high_volume_write);
        assert!(a3035.dual_connections_write);
        assert!(a3035.ldac_write);
        assert!(a3035.auto_play_pause_write);
        assert!(a3035.battery_alert_write);
        assert!(a3035.voice_prompt_write);
        assert!(!a3035.low_battery_prompt_write);
        assert!(!a3035.positioned_sound_mode_write);
        assert!(!a3035.gaming_mode_write);

        let a3040 = ModelDriver::A3040.capabilities();
        assert!(a3040.low_battery_prompt_write);
        assert!(!a3040.auto_play_pause_write);
        assert!(!a3040.battery_alert_write);
        assert_eq!(ModelDriver::A3035.equalizer_bands(), 10);
        assert_eq!(ModelDriver::A3040.equalizer_bands(), 10);
        assert_eq!(
            registry::by_code("A3035").map(ModelDriver::code),
            Some("A3035")
        );
        assert_eq!(
            registry::by_code("A3040").map(ModelDriver::code),
            Some("A3040")
        );
        assert_eq!(registry::DRIVERS.len(), 7);
    }

    fn confirmed(model: ModelDriver, fixture: &[u8]) -> openscl4_models::DeviceState {
        model
            .decode_confirmed_state(&state_body(fixture, model))
            .expect("confirmed reference state")
    }

    /// One complete inbound frame is nine header bytes, the body, and a checksum.
    fn state_body(fixture: &[u8], model: ModelDriver) -> Vec<u8> {
        assert_eq!(fixture[5..7], model.state_command());
        fixture[9..fixture.len() - 1].to_vec()
    }
}
