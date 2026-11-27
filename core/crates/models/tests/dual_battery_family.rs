#![forbid(unsafe_code)]

#[cfg(test)]
mod tests {
    use openscl4_models::{
        AmbientSoundModeCycle, AutoPowerOff, AutoPowerOffSetting, EqualizerSetting, ModelDriver,
        ModelError, SoundMode, SoundModeCycle, TwsStatus, registry,
    };

    fn body(fixture: &[u8]) -> &[u8] {
        assert_eq!(fixture[5..7], [0x01, 0x01]);
        &fixture[9..fixture.len() - 1]
    }

    fn state(model: ModelDriver) -> openscl4_models::DeviceState {
        model
            .decode_confirmed_state(body(fixture(model)))
            .expect("confirmed state")
    }

    fn fixture(model: ModelDriver) -> &'static [u8] {
        match model {
            ModelDriver::A3936 => include_bytes!("../../../data/a3936-pilot-state.bin"),
            ModelDriver::A3952 => include_bytes!("../../../data/a3952-pilot-state.bin"),
            ModelDriver::A3968 => include_bytes!("../../../data/a3968-pilot-state.bin"),
            _ => unreachable!("dual-battery model"),
        }
    }

    fn signature_curve() -> EqualizerSetting {
        EqualizerSetting {
            preset_id: 0,
            volume_adjustments: [0, 0, 0, 0, 0, 0, 0, 0, 0, -120],
        }
    }

    const ALL_MODES: SoundModeCycle = SoundModeCycle {
        noise_canceling: true,
        transparency: true,
        normal: true,
    };

    #[test]
    fn identifies_every_registered_dual_battery_code_from_its_state() {
        for model in [ModelDriver::A3936, ModelDriver::A3952, ModelDriver::A3968] {
            assert_eq!(registry::identify_state(body(fixture(model))), Ok(model));
            assert_eq!(registry::by_code(model.code()), Some(model));
        }
    }

    #[test]
    fn state_only_discovery_rejects_an_unregistered_serial() {
        for model in [ModelDriver::A3936, ModelDriver::A3952, ModelDriver::A3968] {
            let mut unknown = body(fixture(model)).to_vec();
            unknown[16..32].copy_from_slice(b"9999UNKNOWN00000");
            assert_eq!(
                registry::identify_state(&unknown),
                Err(ModelError::UnknownModel)
            );
        }
    }

    #[test]
    fn reads_the_a3936_space_a40_state() {
        let state = state(ModelDriver::A3936);
        assert_eq!(state.model, ModelDriver::A3936);
        assert_eq!(state.sound_mode, Some(SoundMode::Normal));
        assert_eq!(state.battery.left, Some(100));
        assert_eq!(state.battery.right, Some(60));
        assert_eq!(state.battery.case, Some(80));
        assert_eq!(
            state.firmware_left.map(|version| version.to_string()),
            Some("04.19".to_owned())
        );
        assert_eq!(
            state.firmware_right.map(|version| version.to_string()),
            Some("04.19".to_owned())
        );
        assert_eq!(state.equalizer, Some(signature_curve()));
        assert_eq!(state.gaming_mode, Some(false));
        assert_eq!(
            state.auto_power_off,
            Some(AutoPowerOff {
                enabled: true,
                duration: 0,
            })
        );
        assert_eq!(
            state.tws_status,
            Some(TwsStatus {
                host_is_left: false,
                connected: true,
            })
        );
        assert_eq!(
            state.ambient_sound_mode_cycle,
            Some(AmbientSoundModeCycle::Shared(ALL_MODES))
        );
        assert_eq!(state.firmware_device, None);
    }

    #[test]
    fn reads_the_a3952_liberty_4_state() {
        let state = state(ModelDriver::A3952);
        assert_eq!(state.model, ModelDriver::A3952);
        assert_eq!(state.sound_mode, Some(SoundMode::Normal));
        assert_eq!(state.battery.left, Some(80));
        assert_eq!(state.battery.right, Some(80));
        assert_eq!(state.battery.case, Some(80));
        assert_eq!(
            state.firmware_left.map(|version| version.to_string()),
            Some("04.96".to_owned())
        );
        assert_eq!(
            state.equalizer,
            Some(EqualizerSetting {
                preset_id: 0,
                volume_adjustments: [-60; 10],
            })
        );
        assert_eq!(state.gaming_mode, None);
        assert_eq!(
            state.auto_power_off,
            Some(AutoPowerOff {
                enabled: true,
                duration: 0,
            })
        );
        assert_eq!(
            state.ambient_sound_mode_cycle,
            Some(AmbientSoundModeCycle::Shared(SoundModeCycle {
                noise_canceling: true,
                transparency: true,
                normal: false,
            }))
        );
    }

    #[test]
    fn reads_the_a3968_sport_x20_state() {
        let state = state(ModelDriver::A3968);
        assert_eq!(state.model, ModelDriver::A3968);
        assert_eq!(state.sound_mode, Some(SoundMode::NoiseCanceling));
        assert_eq!(state.battery.left, Some(100));
        assert_eq!(state.battery.right, Some(100));
        assert_eq!(state.battery.case, Some(40));
        assert_eq!(
            state.firmware_left.map(|version| version.to_string()),
            Some("01.65".to_owned())
        );
        assert_eq!(
            state.equalizer,
            Some(EqualizerSetting {
                preset_id: 0xfefe,
                volume_adjustments: [0; 10],
            })
        );
        assert_eq!(state.gaming_mode, None);
        assert_eq!(
            state.auto_power_off,
            Some(AutoPowerOff {
                enabled: true,
                duration: 2,
            })
        );
        assert_eq!(
            state.tws_status,
            Some(TwsStatus {
                host_is_left: true,
                connected: true,
            })
        );
        assert_eq!(
            state.ambient_sound_mode_cycle,
            Some(AmbientSoundModeCycle::Tws {
                enabled: ALL_MODES,
                disconnected: SoundModeCycle {
                    noise_canceling: true,
                    transparency: true,
                    normal: false,
                },
            })
        );
    }

    #[test]
    fn rejects_a_state_whose_serial_names_another_model() {
        assert!(
            ModelDriver::A3952
                .decode_confirmed_state(body(fixture(ModelDriver::A3936)))
                .is_none()
        );
        assert!(
            ModelDriver::A3936
                .decode_confirmed_state(body(fixture(ModelDriver::A3968)))
                .is_none()
        );
    }

    #[test]
    fn admits_only_the_recorded_state_lengths() {
        for (model, accepted, rejected) in [
            (ModelDriver::A3936, vec![143], vec![142, 144]),
            (ModelDriver::A3952, vec![140, 153], vec![139, 141, 154]),
            (ModelDriver::A3968, vec![130, 143], vec![129, 131, 144]),
        ] {
            let full = body(fixture(model));
            for length in accepted {
                let mut candidate = full.to_vec();
                candidate.resize(length, 0);
                assert!(
                    model.decode_confirmed_state(&candidate).is_some(),
                    "{model:?} rejected its {length}-byte layout"
                );
            }
            for length in rejected {
                let mut candidate = full.to_vec();
                candidate.resize(length, 0);
                assert!(
                    model.decode_confirmed_state(&candidate).is_none(),
                    "{model:?} accepted a {length}-byte body"
                );
            }
        }
        assert_eq!(
            ModelDriver::A3936.decode_state(&[0; 142]),
            Err(ModelError::IncompleteState)
        );
    }

    #[test]
    fn the_family_capabilities_follow_the_writes_the_session_confirms() {
        for model in [ModelDriver::A3936, ModelDriver::A3952, ModelDriver::A3968] {
            let capabilities = model.capabilities();
            assert!(capabilities.ambient_sound_mode_write, "{model:?}");
            assert!(capabilities.auto_power_off_write, "{model:?}");
            assert!(!capabilities.equalizer_write, "{model:?}");
            assert!(!capabilities.positioned_sound_mode_write, "{model:?}");
        }
        assert!(ModelDriver::A3936.capabilities().gaming_mode_write);
        assert!(!ModelDriver::A3952.capabilities().gaming_mode_write);
        assert!(!ModelDriver::A3968.capabilities().gaming_mode_write);
    }

    #[test]
    fn writes_the_space_a40_sound_mode_block() {
        let state = state(ModelDriver::A3936);
        assert_eq!(
            state.dual_sound_mode_write(SoundMode::NoiseCanceling),
            Ok(vec![0x00, 0x30, 0x00, 0x01, 0x00, 0x00])
        );
        assert_eq!(
            state.dual_sound_mode_write(SoundMode::Transparency),
            Ok(vec![0x01, 0x30, 0x00, 0x01, 0x00, 0x00])
        );
        assert_eq!(
            state.dual_sound_mode_write(SoundMode::AirplaneMode),
            Err(ModelError::InvalidSoundModeWrite)
        );
    }

    #[test]
    fn writes_the_liberty_4_sound_mode_block() {
        let state = state(ModelDriver::A3952);
        assert_eq!(
            state.dual_sound_mode_write(SoundMode::NoiseCanceling),
            Ok(vec![0x00, 0x30, 0x00, 0x00, 0x01, 0x01])
        );
    }

    #[test]
    fn writes_the_sport_x20_sound_mode_block_with_its_fixed_tail() {
        let state = state(ModelDriver::A3968);
        assert_eq!(
            state.dual_sound_mode_write(SoundMode::Transparency),
            Ok(vec![0x01, 0x32, 0x01, 0x00, 0x00, 0xff])
        );
    }

    #[test]
    fn the_duplicate_state_keeps_the_previous_mode_controls() {
        let state = state(ModelDriver::A3952);
        let write = state
            .dual_sound_mode_write(SoundMode::Transparency)
            .expect("valid write");
        assert_eq!(write[1..], [0x30, 0x00, 0x00, 0x01, 0x01]);
    }

    #[test]
    fn writes_auto_power_off_with_the_reported_duration_index() {
        let state = state(ModelDriver::A3936);
        let write = state
            .auto_power_off_write(AutoPowerOffSetting::Enabled(3))
            .expect("valid write");
        assert_eq!(write.body(), [1, 3]);
        assert_eq!(
            write.expected(),
            AutoPowerOff {
                enabled: true,
                duration: 3,
            }
        );
        let disable = state
            .auto_power_off_write(AutoPowerOffSetting::Disabled)
            .expect("valid write");
        assert_eq!(disable.body(), [0, 0]);
        assert_eq!(
            disable.expected(),
            AutoPowerOff {
                enabled: false,
                duration: 0,
            }
        );
        assert_eq!(
            state.auto_power_off_write(AutoPowerOffSetting::Enabled(4)),
            Err(ModelError::InvalidAutoPowerOffWrite)
        );
    }

    #[test]
    fn writes_the_space_a40_gaming_mode_flag() {
        let a3936 = state(ModelDriver::A3936);
        assert_eq!(a3936.gaming_mode_write(true), Ok([1]));
        assert_eq!(a3936.gaming_mode_write(false), Ok([0]));
        for model in [ModelDriver::A3952, ModelDriver::A3968] {
            assert_eq!(
                state(model).gaming_mode_write(true),
                Err(ModelError::InvalidGamingModeWrite)
            );
        }
    }

    #[test]
    fn the_unexposed_second_equalizer_channel_is_not_written() {
        let state = state(ModelDriver::A3936);
        assert_eq!(
            state
                .model
                .equalizer_write_body(signature_curve(), body(fixture(ModelDriver::A3936))),
            Err(ModelError::InvalidEqualizerWrite)
        );
    }

    #[test]
    fn an_absent_equalizer_block_reports_no_curve() {
        let mut candidate = body(fixture(ModelDriver::A3936)).to_vec();
        candidate[34] = u8::MAX;
        let state = ModelDriver::A3936
            .decode_state(&candidate)
            .expect("state with an absent curve");
        assert_eq!(state.equalizer, None);
        // The same absent marker is not a confirmed curve for the other layouts.
        let mut candidate = body(fixture(ModelDriver::A3968)).to_vec();
        candidate[40] = u8::MAX;
        assert_eq!(
            ModelDriver::A3968
                .decode_state(&candidate)
                .expect("state with an absent curve")
                .equalizer,
            None
        );
    }

    #[test]
    fn an_unrecognized_extra_field_is_not_a_confirmed_value() {
        let mut candidate = body(fixture(ModelDriver::A3952)).to_vec();
        candidate[139] = 4;
        let state = ModelDriver::A3952
            .decode_state(&candidate)
            .expect("state with an unrecognized duration");
        assert_eq!(state.auto_power_off, None);

        let mut candidate = body(fixture(ModelDriver::A3936)).to_vec();
        candidate[128] = 2;
        let state = ModelDriver::A3936
            .decode_state(&candidate)
            .expect("state with an unrecognized flag");
        assert_eq!(state.auto_power_off, None);
    }
}
