#![forbid(unsafe_code)]

#[cfg(test)]
mod tests {
    use openscl4_models::{
        AutoPowerOff, AutoPowerOffSetting, ConnectionSettings, D1202_EQUALIZER_COMMAND,
        D1202_SERVICE_ID, D1202C_SERVICE_ID, DecibelReadingRefreshRate, EqualizerSetting,
        HostDevice, LimitHighVolume, ModelDriver, ModelError, SpatialAudio, SpatialAudioMode,
        registry,
    };

    /// The measured P31i state from `core/data/d1202-pilot-state.bin`.
    const D1202_STATE: &[u8] = include_bytes!("../../../data/d1202-pilot-state.bin");
    /// The measured R60i NC state from `core/data/d1202c-pilot-state.bin`.
    const D1202C_STATE: &[u8] = include_bytes!("../../../data/d1202c-pilot-state.bin");
    /// The legacy builder example curve of the `set_preset_eq` test.
    const LEGACY_CURVE: [i16; 10] = [40, 10, 20, 20, 40, 40, 40, 20, 0, -120];
    const LEGACY_BANDS: [u8; 10] = [160, 130, 140, 140, 160, 160, 160, 140, 120, 0];

    fn body(fixture: &[u8]) -> &[u8] {
        &fixture[9..fixture.len() - 1]
    }

    #[test]
    fn identifies_each_public_code_from_the_recorded_connection_metadata() {
        for (state, model) in [
            (D1202_STATE, ModelDriver::D1202),
            (D1202C_STATE, ModelDriver::D1202C),
        ] {
            let state = body(state);
            assert_eq!(
                registry::identify_state(state),
                Ok(ModelDriver::D1202),
                "the shared serial alone names the canonical code"
            );
            assert_eq!(
                registry::identify_state_hinted(state, Some(model)),
                Ok(model)
            );
            assert_eq!(
                registry::identify_state_hinted(state, Some(ModelDriver::A3949)),
                Ok(ModelDriver::D1202),
                "an unrelated hint cannot move the family"
            );
        }
    }

    #[test]
    fn recorded_services_select_the_public_codes() {
        assert_eq!(
            registry::identify_service(&D1202_SERVICE_ID),
            Some(ModelDriver::D1202)
        );
        assert_eq!(
            registry::identify_service(&D1202C_SERVICE_ID),
            Some(ModelDriver::D1202C)
        );
        assert_eq!(
            ModelDriver::service_id(ModelDriver::D1202),
            Some(D1202_SERVICE_ID)
        );
        assert_eq!(
            ModelDriver::service_id(ModelDriver::D1202C),
            Some(D1202C_SERVICE_ID)
        );
        assert_eq!(ModelDriver::service_id(ModelDriver::A3028), None);
    }

    #[test]
    fn overlapping_public_codes_are_registered_and_resolved_separately() {
        assert_eq!(registry::by_code("D1202"), Some(ModelDriver::D1202));
        assert_eq!(registry::by_code("D1202C"), Some(ModelDriver::D1202C));
        assert_ne!(
            registry::by_code("D1202").expect("registered D1202").code(),
            registry::by_code("D1202C")
                .expect("registered D1202C")
                .code()
        );
        assert_eq!(registry::by_code("D120"), None);
    }

    #[test]
    fn decodes_the_measured_p31i_state() {
        let model = ModelDriver::D1202;
        let state = model
            .decode_confirmed_state(body(D1202_STATE))
            .expect("measured P31i state");
        assert_eq!(state.model, ModelDriver::D1202);
        assert_eq!(state.battery.left, Some(100));
        assert_eq!(state.battery.right, Some(100));
        assert_eq!(state.battery.case, Some(40));
        assert_eq!(
            state.firmware_left.map(|firmware| firmware.to_string()),
            Some("01.83".to_owned())
        );
        assert_eq!(
            state.firmware_right.map(|firmware| firmware.to_string()),
            Some("01.83".to_owned())
        );
        assert_eq!(
            state.sound_mode,
            Some(openscl4_models::SoundMode::NoiseCanceling)
        );
        assert_eq!(
            state.equalizer,
            Some(EqualizerSetting {
                preset_id: 0,
                volume_adjustments: [0; 10],
            })
        );
        assert_eq!(
            state.spatial_audio,
            Some(SpatialAudio {
                enabled: false,
                mode: SpatialAudioMode::Music,
            })
        );
        assert_eq!(
            state.auto_power_off,
            Some(AutoPowerOff {
                enabled: true,
                duration: 2,
            })
        );
        assert_eq!(
            state.connection,
            Some(ConnectionSettings {
                tws_connected: true,
                host_device: HostDevice::Right,
                dual_connections_enabled: true,
                ldac: false,
                touch_tone: true,
                low_battery_prompt: true,
                limit_high_volume: LimitHighVolume {
                    enabled: false,
                    db_limit: 90,
                    refresh_rate: DecibelReadingRefreshRate::RealTime,
                },
            })
        );
    }

    #[test]
    fn decodes_the_measured_r60i_nc_state() {
        let model = ModelDriver::D1202C;
        let state = model
            .decode_confirmed_state(body(D1202C_STATE))
            .expect("measured R60i NC state");
        assert_eq!(state.model, ModelDriver::D1202C);
        assert_eq!(state.battery.left, Some(100));
        assert_eq!(state.battery.right, Some(100));
        assert_eq!(state.battery.case, Some(100));
        assert_eq!(
            state.firmware_left.map(|firmware| firmware.to_string()),
            Some("04.89".to_owned())
        );
        assert_eq!(
            state.equalizer,
            Some(EqualizerSetting {
                preset_id: 0xfefe,
                volume_adjustments: [11, 20, 16, 8, 1, 3, 16, 31, 0, 0],
            })
        );
        assert_eq!(
            state.auto_power_off,
            Some(AutoPowerOff {
                enabled: true,
                duration: 3,
            })
        );
        assert_eq!(
            state.connection.map(|settings| settings.limit_high_volume),
            Some(LimitHighVolume {
                enabled: false,
                db_limit: 95,
                refresh_rate: DecibelReadingRefreshRate::RealTime,
            })
        );
    }

    #[test]
    fn admits_only_the_measured_layout_and_the_family_serial() {
        for model in [ModelDriver::D1202, ModelDriver::D1202C] {
            for length in [139, 151, 153] {
                let mut truncated = body(D1202_STATE).to_vec();
                truncated.resize(length, 0);
                assert!(
                    model.decode_confirmed_state(&truncated).is_none(),
                    "{model:?} accepted a {length}-byte body"
                );
            }
            let mut foreign = body(D1202_STATE).to_vec();
            foreign[16..32].copy_from_slice(b"9999TEST00000000");
            assert!(
                registry::identify_state(&foreign).is_err(),
                "a non-family serial must not select a registered model"
            );
            assert!(model.decode_confirmed_state(&foreign).is_none());
        }
    }

    #[test]
    fn rejects_an_unrecognized_ambient_mode() {
        let mut state = body(D1202_STATE).to_vec();
        state[119] = 3;
        assert_eq!(
            ModelDriver::D1202.decode_state(&state),
            Err(ModelError::UnknownSoundMode)
        );
        assert!(ModelDriver::D1202.decode_confirmed_state(&state).is_none());
    }

    #[test]
    fn capabilities_follow_the_confirmed_operations() {
        for model in [ModelDriver::D1202, ModelDriver::D1202C] {
            let capabilities = model.capabilities();
            assert!(capabilities.ambient_sound_mode_write);
            assert!(capabilities.equalizer_write);
            assert!(capabilities.auto_power_off_write);
            assert!(capabilities.spatial_audio_write);
            assert!(!capabilities.positioned_sound_mode_write);
            assert!(!capabilities.gaming_mode_write);
            assert_eq!(model.equalizer_bands(), 10);
            assert_eq!(model.equalizer_command(), D1202_EQUALIZER_COMMAND);
        }
        assert!(!ModelDriver::A3028.capabilities().spatial_audio_write);
        assert!(!ModelDriver::A3062.capabilities().spatial_audio_write);
    }

    #[test]
    fn writes_the_ambient_mode_with_the_measured_controls() {
        let state = ModelDriver::D1202
            .decode_confirmed_state(body(D1202_STATE))
            .expect("measured P31i state");
        assert_eq!(
            state.d1202_sound_mode_write(openscl4_models::SoundMode::Normal),
            Ok([0x02, 0x50, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00])
        );
        assert_eq!(
            state.d1202_sound_mode_write(openscl4_models::SoundMode::NoiseCanceling),
            Ok([0x00, 0x50, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00])
        );
        assert_eq!(
            state.d1202_sound_mode_write(openscl4_models::SoundMode::AirplaneMode),
            Err(ModelError::InvalidSoundModeWrite)
        );
    }

    #[test]
    fn writes_the_measured_equalizer_body_and_reads_it_back() {
        let raw = body(D1202_STATE);
        let setting = EqualizerSetting {
            preset_id: 1,
            volume_adjustments: LEGACY_CURVE,
        };
        let write = ModelDriver::D1202
            .equalizer_write_body(setting, raw)
            .expect("measured write");
        assert_eq!(write.len(), 114);
        assert_eq!(&write[0..2], &1_u16.to_le_bytes());
        assert_eq!(&write[2..4], &[0, 0]);
        assert_eq!(&write[4..24], &[LEGACY_BANDS, LEGACY_BANDS].concat()[..]);
        assert_eq!(&write[24..27], &[0, 0, 0]);
        assert_eq!(&write[27..47], &raw[62..82]);
        assert_eq!(&write[47..51], &raw[82..86]);
        assert_eq!(write[51], raw[86]);
        assert_eq!(&write[52..72], &raw[87..107]);
        assert_eq!(&write[72..92], &interleaved(&LEGACY_BANDS)[..]);
        assert_eq!(&write[92..112], &interleaved(&LEGACY_BANDS)[..]);
        assert_eq!(&write[112..114], &[0, 0]);

        // A reply carrying the requested preset and first channel confirms it.
        let mut applied = raw.to_vec();
        applied[38..40].copy_from_slice(&1_u16.to_le_bytes());
        applied[40..50].copy_from_slice(&LEGACY_BANDS);
        assert_eq!(
            ModelDriver::D1202
                .decode_confirmed_state(&applied)
                .expect("applied state")
                .equalizer,
            Some(setting)
        );
    }

    #[test]
    fn writes_auto_power_off_from_the_measured_state() {
        let state = ModelDriver::D1202
            .decode_confirmed_state(body(D1202_STATE))
            .expect("measured P31i state");
        let disabled = state
            .auto_power_off_write(AutoPowerOffSetting::Disabled)
            .expect("disable");
        assert_eq!(disabled.body(), [0, 2]);
        assert_eq!(
            disabled.expected(),
            AutoPowerOff {
                enabled: false,
                duration: 2,
            }
        );
        let enabled = state
            .auto_power_off_write(AutoPowerOffSetting::Enabled(3))
            .expect("last table entry");
        assert_eq!(enabled.body(), [1, 3]);
        for index in [4, u8::MAX] {
            assert_eq!(
                state.auto_power_off_write(AutoPowerOffSetting::Enabled(index)),
                Err(ModelError::InvalidAutoPowerOffWrite)
            );
        }
    }

    #[test]
    fn writes_spatial_audio_through_the_legacy_commands() {
        let raw = body(D1202_STATE);
        let state = ModelDriver::D1202
            .decode_confirmed_state(raw)
            .expect("measured P31i state");
        let enable = state
            .spatial_audio_write(true, raw)
            .expect("enable spatial audio");
        assert_eq!(enable.command(), [0x10, 0x81]);
        assert_eq!(enable.body(), [1, 0, 0]);
        assert_eq!(
            enable.expected(),
            SpatialAudio {
                enabled: true,
                mode: SpatialAudioMode::Music,
            }
        );

        // Disabling follows the legacy state modifier's equalizer write.
        let disable = state
            .spatial_audio_write(false, raw)
            .expect("disable spatial audio");
        assert_eq!(disable.command(), D1202_EQUALIZER_COMMAND);
        assert_eq!(disable.body().len(), 114);
        assert_eq!(
            disable.expected(),
            SpatialAudio {
                enabled: false,
                mode: SpatialAudioMode::Music,
            }
        );
    }

    fn interleaved(bands: &[u8; 10]) -> [u8; 20] {
        let mut out = [0; 20];
        for (index, band) in bands.iter().enumerate() {
            out[index * 2] = *band;
        }
        out
    }
}
