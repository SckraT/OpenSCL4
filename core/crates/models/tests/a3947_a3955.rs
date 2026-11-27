//! Measured layout and write formats of the Liberty 4 NC and P40i.

#![forbid(unsafe_code)]

#[cfg(test)]
mod tests {
    use openscl4_models::{
        A3947_EQUALIZER_BANDS, A3947_EQUALIZER_PRESET, A3947_GAMING_MODE,
        A3947_GAMING_MODE_COMMAND, A3947_SOUND_MODE_FIELDS, A3955_EQUALIZER_BANDS,
        A3955_EQUALIZER_PRESET, A3955_SOUND_MODE_FIELDS, AUTO_POWER_OFF_DURATION_COUNT,
        AutoPowerOff, EqualizerSetting, GAMING_MODE_COMMAND, IMMERSIVE_EXPERIENCE_COMMAND,
        ImmersiveExperience, ModelDriver, ModelError, SoundMode, registry,
    };

    /// The masked `01 01` state responses from `core/data`. Each fixture is a
    /// complete frame: a nine-byte header, the body and a checksum.
    const A3947_STATE: &[u8] = include_bytes!("../../../data/a3947-pilot-state.bin");
    const A3955_STATE: &[u8] = include_bytes!("../../../data/a3955-pilot-state.bin");
    /// The measured write bodies from the legacy equalizer golden tests.
    const A3947_EQ_WRITE: &[u8] = include_bytes!("../../../data/a3947-eq-write.bin");
    const A3955_EQ_WRITE: &[u8] = include_bytes!("../../../data/a3955-eq-write.bin");

    const A3947_CURVE: EqualizerSetting = EqualizerSetting {
        preset_id: 0xfefe,
        volume_adjustments: [0, 1, 2, 3, 4, 5, 6, 7, 0, 0],
    };
    const A3955_CURVE: EqualizerSetting = EqualizerSetting {
        preset_id: 0xfefe,
        volume_adjustments: [60, -60, 60, -60, 60, -60, 60, -60, 0, 0],
    };

    fn body(frame: &[u8]) -> &[u8] {
        assert_eq!(frame[5..7], [0x01, 0x01]);
        &frame[9..frame.len() - 1]
    }

    #[test]
    fn identifies_both_models_from_the_measured_state() {
        assert_eq!(body(A3947_STATE).len(), 158);
        assert_eq!(body(A3955_STATE).len(), 143);
        assert_eq!(
            registry::identify_state(body(A3947_STATE)),
            Ok(ModelDriver::A3947)
        );
        assert_eq!(
            registry::identify_state(body(A3955_STATE)),
            Ok(ModelDriver::A3955)
        );
        assert_eq!(registry::by_code("A3947"), Some(ModelDriver::A3947));
        assert_eq!(registry::by_code("A3955"), Some(ModelDriver::A3955));
        assert_eq!(registry::DRIVERS.len(), 7);
    }

    #[test]
    fn reads_the_measured_liberty_state() {
        let model = ModelDriver::A3947;
        let state = model
            .decode_confirmed_state(body(A3947_STATE))
            .expect("measured Liberty 4 NC state");
        assert_eq!(state.model, model);
        assert_eq!(state.battery.left, Some(40));
        assert_eq!(state.battery.right, Some(80));
        assert_eq!(state.battery.case, Some(40));
        assert_eq!(
            state.firmware_left.map(|version| version.to_string()),
            Some("05.88".to_owned())
        );
        assert_eq!(
            state.firmware_right.map(|version| version.to_string()),
            Some("05.88".to_owned())
        );
        assert_eq!(state.sound_mode, Some(SoundMode::NoiseCanceling));
        assert_eq!(state.gaming_mode, Some(false));
        assert_eq!(
            state.equalizer,
            Some(EqualizerSetting {
                preset_id: 0xfefe,
                volume_adjustments: [0; 10],
            })
        );
        assert_eq!(state.tws_connected, Some(true));
        assert!(state.equalizer_write_available());
        assert_eq!(
            state.auto_power_off,
            Some(AutoPowerOff {
                enabled: true,
                duration: 1,
            })
        );
    }

    #[test]
    fn reads_the_measured_p40i_state() {
        let model = ModelDriver::A3955;
        let state = model
            .decode_confirmed_state(body(A3955_STATE))
            .expect("measured P40i state");
        assert_eq!(state.model, model);
        assert_eq!(state.battery.left, Some(100));
        assert_eq!(state.battery.right, Some(100));
        assert_eq!(state.battery.case, Some(60));
        assert_eq!(
            state.firmware_left.map(|version| version.to_string()),
            Some("01.61".to_owned())
        );
        assert_eq!(
            state.firmware_right.map(|version| version.to_string()),
            Some("01.61".to_owned())
        );
        assert_eq!(state.sound_mode, Some(SoundMode::NoiseCanceling));
        assert_eq!(state.gaming_mode, None);
        assert_eq!(
            state.equalizer,
            Some(EqualizerSetting {
                preset_id: 8,
                volume_adjustments: [30, 20, -20, 20, 10, 20, 30, 30, 0, -120],
            })
        );
        assert_eq!(state.tws_connected, Some(true));
        assert_eq!(
            state.auto_power_off,
            Some(AutoPowerOff {
                enabled: true,
                duration: 2,
            })
        );
    }

    #[test]
    fn rejects_a_state_whose_serial_names_another_registered_model() {
        let mut contradictory = body(A3947_STATE).to_vec();
        contradictory[16..32].copy_from_slice(b"3955TEST00000000");
        assert!(
            ModelDriver::A3947
                .decode_confirmed_state(&contradictory)
                .is_none()
        );
        assert_eq!(
            registry::identify_state(&contradictory),
            Err(ModelError::UnknownModel)
        );

        let mut unknown = body(A3955_STATE).to_vec();
        unknown[16..32].copy_from_slice(b"9999UNKNOWN00000");
        assert_eq!(
            registry::identify_state(&unknown),
            Err(ModelError::UnknownModel)
        );
    }

    #[test]
    fn admits_only_the_measured_state_lengths() {
        for (model, frame) in [
            (ModelDriver::A3947, A3947_STATE),
            (ModelDriver::A3955, A3955_STATE),
        ] {
            let state = body(frame);
            assert!(model.decode_confirmed_state(state).is_some());
            for length in [state.len() - 1, state.len() + 1] {
                let mut resized = state.to_vec();
                resized.resize(length, 0);
                assert!(
                    model.decode_confirmed_state(&resized).is_none(),
                    "{model:?} accepted a {length}-byte body"
                );
            }
        }
        assert_eq!(
            ModelDriver::A3955.decode_state(&[0; 142]),
            Err(ModelError::IncompleteState)
        );
    }

    #[test]
    fn writes_the_measured_extended_sound_mode_bodies() {
        let liberty = ModelDriver::A3947
            .decode_confirmed_state(body(A3947_STATE))
            .expect("measured Liberty 4 NC state");
        // The legacy `set_sound_mode_normal` golden body changes only the mode byte.
        assert_eq!(
            liberty.extended_sound_mode_write(SoundMode::Normal),
            Ok([0x02, 0x50, 0x01, 0x00, 0x01, 0x01, 0x03])
        );
        assert_eq!(
            liberty.extended_sound_mode_write(SoundMode::NoiseCanceling),
            Ok([0x00, 0x50, 0x01, 0x00, 0x01, 0x01, 0x03])
        );
        assert_eq!(
            liberty.extended_sound_mode_write(SoundMode::AirplaneMode),
            Err(ModelError::InvalidSoundModeWrite)
        );
        assert_eq!(
            liberty.ambient_sound_mode_write(SoundMode::Normal),
            Err(ModelError::InvalidSoundModeWrite)
        );

        let p40i = ModelDriver::A3955
            .decode_confirmed_state(body(A3955_STATE))
            .expect("measured P40i state");
        assert_eq!(
            p40i.extended_sound_mode_write(SoundMode::Transparency),
            Ok([0x01, 0x51, 0x01, 0x02, 0x01, 0xff, 0x01])
        );
        assert_eq!(
            p40i.extended_sound_mode_write(SoundMode::Normal),
            Ok([0x02, 0x51, 0x01, 0x02, 0x01, 0xff, 0x01])
        );
        assert_eq!(
            p40i.extended_sound_mode_write(SoundMode::NoiseCanceling),
            Ok([0x00, 0x51, 0x01, 0x02, 0x01, 0xff, 0x01])
        );
    }

    #[test]
    fn rejects_a_state_with_an_unrecognized_extended_mode() {
        for (frame, fields) in [
            (A3947_STATE, A3947_SOUND_MODE_FIELDS),
            (A3955_STATE, A3955_SOUND_MODE_FIELDS),
        ] {
            let mut malformed = body(frame).to_vec();
            malformed[fields.start] = 0x03;
            assert_eq!(
                ModelDriver::decode_state(
                    registry::identify_state(&malformed).expect("model by length"),
                    &malformed
                ),
                Err(ModelError::UnknownSoundMode)
            );

            let mut malformed = body(frame).to_vec();
            // The transparency byte only accepts the documented values.
            malformed[fields.start + 2] = 0x02;
            assert_eq!(
                ModelDriver::decode_state(
                    registry::identify_state(&malformed).expect("model by length"),
                    &malformed
                ),
                Err(ModelError::UnknownSoundMode)
            );
        }
    }

    #[test]
    fn writes_the_measured_equalizer_bodies() {
        let liberty = ModelDriver::A3947
            .decode_confirmed_state(body(A3947_STATE))
            .expect("measured Liberty 4 NC state");
        assert_eq!(
            liberty
                .model
                .equalizer_write_body(A3947_CURVE, body(A3947_STATE))
                .expect("measured Liberty 4 NC equalizer write"),
            A3947_EQ_WRITE
        );

        let p40i = ModelDriver::A3955
            .decode_confirmed_state(body(A3955_STATE))
            .expect("measured P40i state");
        assert_eq!(
            p40i.model
                .equalizer_write_body(A3955_CURVE, body(A3955_STATE))
                .expect("measured P40i equalizer write"),
            A3955_EQ_WRITE
        );
        assert_eq!(A3947_EQ_WRITE.len(), 93);
        assert_eq!(A3955_EQ_WRITE.len(), 93);
    }

    #[test]
    fn equalizer_write_requires_both_earbuds_connected() {
        for (frame, curve) in [(A3947_STATE, A3947_CURVE), (A3955_STATE, A3955_CURVE)] {
            let mut disconnected = body(frame).to_vec();
            // The measured TWS status is the second state byte.
            disconnected[1] = 0;
            let model = registry::identify_state(&disconnected).expect("model by length");
            let state = model
                .decode_confirmed_state(&disconnected)
                .expect("the layout is still confirmed");
            assert_eq!(state.tws_connected, Some(false));
            assert!(!state.equalizer_write_available());
            assert_eq!(
                model.equalizer_write_body(curve, &disconnected),
                Err(ModelError::InvalidEqualizerWrite)
            );
        }
    }

    #[test]
    fn gaming_mode_uses_the_liberty_write_command() {
        assert_eq!(A3947_GAMING_MODE_COMMAND, [0x10, 0x85]);
        assert_eq!(
            ModelDriver::A3947.gaming_mode_command(),
            A3947_GAMING_MODE_COMMAND
        );
        assert_eq!(
            ModelDriver::A3949.gaming_mode_command(),
            GAMING_MODE_COMMAND
        );
        assert_eq!(
            ModelDriver::A3955.gaming_mode_command(),
            GAMING_MODE_COMMAND
        );
        assert_eq!(A3955_SOUND_MODE_FIELDS.len(), 7);
        assert_eq!(A3947_SOUND_MODE_FIELDS.len(), 7);
        assert_eq!(A3947_GAMING_MODE, 143);
        assert_eq!(A3947_EQUALIZER_PRESET, 37..39);
        assert_eq!(A3947_EQUALIZER_BANDS, 39..59);
        assert_eq!(A3955_EQUALIZER_PRESET, 38..40);
        assert_eq!(A3955_EQUALIZER_BANDS, 40..60);
        assert_eq!(AUTO_POWER_OFF_DURATION_COUNT, 4);

        let state = ModelDriver::A3947
            .decode_confirmed_state(body(A3947_STATE))
            .expect("measured Liberty 4 NC state");
        assert_eq!(state.gaming_mode_write(true), Ok([1]));
        assert_eq!(state.gaming_mode_write(false), Ok([0]));
        assert_eq!(
            ModelDriver::A3955
                .decode_confirmed_state(body(A3955_STATE))
                .expect("measured P40i state")
                .gaming_mode_write(true),
            Err(ModelError::InvalidGamingModeWrite)
        );
    }

    #[test]
    fn immersive_experience_is_write_only_on_the_p40i() {
        assert_eq!(IMMERSIVE_EXPERIENCE_COMMAND, [0x12, 0x81]);
        assert_eq!(ImmersiveExperience::Disabled.wire(), 0);
        assert_eq!(ImmersiveExperience::GamingMode.wire(), 1);
        assert_eq!(ImmersiveExperience::MovieMode.wire(), 2);
        let p40i = ModelDriver::A3955
            .decode_confirmed_state(body(A3955_STATE))
            .expect("measured P40i state");
        assert_eq!(
            p40i.immersive_experience_write(ImmersiveExperience::MovieMode),
            Ok([2])
        );
        let liberty = ModelDriver::A3947
            .decode_confirmed_state(body(A3947_STATE))
            .expect("measured Liberty 4 NC state");
        assert_eq!(
            liberty.immersive_experience_write(ImmersiveExperience::MovieMode),
            Err(ModelError::InvalidImmersiveExperienceWrite)
        );
    }

    #[test]
    fn equalizer_capability_is_limited_to_the_measured_models() {
        let curve = EqualizerSetting {
            preset_id: 0,
            volume_adjustments: [0; 10],
        };
        for model in [
            ModelDriver::A3028,
            ModelDriver::A3029,
            ModelDriver::A3062,
            ModelDriver::A3947,
            ModelDriver::A3955,
        ] {
            assert!(model.equalizer_is_writable(curve), "{model:?}");
        }
        for model in [ModelDriver::A3949, ModelDriver::A3954] {
            assert!(!model.equalizer_is_writable(curve), "{model:?}");
            assert_eq!(
                model.equalizer_write_body(curve, &[0; 158]),
                Err(ModelError::InvalidEqualizerWrite)
            );
        }
        let out_of_range = EqualizerSetting {
            preset_id: 0,
            volume_adjustments: [135, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        };
        assert!(!ModelDriver::A3947.equalizer_is_writable(out_of_range));
    }
}
