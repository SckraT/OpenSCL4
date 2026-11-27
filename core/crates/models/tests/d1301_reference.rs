#![forbid(unsafe_code)]

#[cfg(test)]
mod tests {
    use openscl4_models::{
        ModelDriver, ModelError, d1301, d1301::Alarm, d1301::AlarmRepeat, d1301::AlarmWakeUpTune,
        d1301::AutoStopTimer, d1301::ListeningMode, d1301::PostSleepAudio, d1301::SleepAudio,
        registry,
    };

    // Captured frames from `core/data/d1301-sleep-reference.json`. Every state
    // fixture masks the measured serial with the synthetic model prefix.
    const POST_SLEEP_KEEP: &[u8] = include_bytes!("../../../data/d1301-post-sleep-keep.bin");
    const POST_SLEEP_PAUSE: &[u8] = include_bytes!("../../../data/d1301-post-sleep-pause.bin");
    const POST_SLEEP_PLAY_LOCAL: &[u8] =
        include_bytes!("../../../data/d1301-post-sleep-play-local.bin");
    const AUTO_STOP_TIMER: &[u8] = include_bytes!("../../../data/d1301-auto-stop-timer.bin");
    const ALARMS_EMPTY: &[u8] = include_bytes!("../../../data/d1301-alarms-empty.bin");
    const ALARMS_FILLED: &[u8] = include_bytes!("../../../data/d1301-alarms-filled.bin");
    const ALARMS_CHANGED: &[u8] = include_bytes!("../../../data/d1301-alarms-changed.bin");

    fn frame_body(fixture: &[u8], command: [u8; 2]) -> &[u8] {
        assert_eq!(fixture[..2], [0x09, 0xFF], "inbound prefix");
        assert_eq!(fixture[5..7], command);
        assert_eq!(
            usize::from(u16::from_le_bytes([fixture[7], fixture[8]])),
            fixture.len(),
            "declared length"
        );
        assert_eq!(
            fixture[fixture.len() - 1],
            fixture[..fixture.len() - 1]
                .iter()
                .fold(0u8, |sum, byte| sum.wrapping_add(*byte)),
            "additive checksum"
        );
        &fixture[9..fixture.len() - 1]
    }

    #[test]
    fn identifies_the_model_from_the_masked_state_serial() {
        let body = frame_body(POST_SLEEP_PAUSE, [0x01, 0x01]);
        assert_eq!(body.len(), d1301::STATE_LENGTH);
        assert_eq!(&body[d1301::STATE_SERIAL], b"1301TEST00000000");
        assert_eq!(
            registry::identify_state(body),
            Ok(ModelDriver::D1301),
            "the state serial selects the driver"
        );
        assert!(registry::DRIVERS.contains(&ModelDriver::D1301));
        assert_eq!(registry::by_code("D1301"), Some(ModelDriver::D1301));
        assert!(
            ModelDriver::D1301.decode_confirmed_state(body).is_some(),
            "the captured layout admits writes"
        );
    }

    #[test]
    fn decodes_the_three_captured_post_sleep_states() {
        for (fixture, expected) in [
            (
                POST_SLEEP_KEEP,
                SleepAudio {
                    enabled: false,
                    action: PostSleepAudio::PlayLocal,
                },
            ),
            (
                POST_SLEEP_PAUSE,
                SleepAudio {
                    enabled: true,
                    action: PostSleepAudio::Pause,
                },
            ),
            (
                POST_SLEEP_PLAY_LOCAL,
                SleepAudio {
                    enabled: true,
                    action: PostSleepAudio::PlayLocal,
                },
            ),
        ] {
            let state = ModelDriver::D1301
                .decode_confirmed_state(frame_body(fixture, [0x01, 0x01]))
                .expect("captured sleep state");
            assert_eq!(state.sleep_audio, Some(expected));
            assert_eq!(state.battery.left, Some(100));
            assert_eq!(state.battery.right, Some(100));
            assert_eq!(
                state.firmware_left.map(|firmware| firmware.to_string()),
                Some("01.91".to_owned())
            );
            assert_eq!(state.listening_mode, Some(ListeningMode::Bluetooth));
            assert_eq!(state.default_listening_mode, Some(ListeningMode::Bluetooth));
            assert_eq!(state.noise_canceling, Some(false));
            assert_eq!(state.sound_mode, None);
        }
    }

    #[test]
    fn decodes_the_captured_auto_stop_timer_report() {
        let body = frame_body(AUTO_STOP_TIMER, [0x15, 0x03]);
        assert_eq!(
            AutoStopTimer::decode(body),
            Ok(AutoStopTimer {
                enabled: false,
                duration_minutes: 30,
                unknown: 1,
                time_left_seconds: 60,
            })
        );
    }

    #[test]
    fn decodes_the_empty_filled_and_changed_alarm_lists() {
        assert!(frame_body(ALARMS_EMPTY, [0x14, 0x01]).is_empty());

        let filled = d1301::decode_alarms(frame_body(ALARMS_FILLED, [0x14, 0x01]))
            .expect("captured alarm list");
        assert_eq!(filled.len(), 2);
        assert_eq!(
            filled[0],
            Alarm {
                id: 0,
                enabled: false,
                time: 405,
                repeat: AlarmRepeat::from_bits_truncate(0),
                wake_up_tune: AlarmWakeUpTune::Bloom,
                volume: 50,
                snooze_duration_in_minutes: 10,
            }
        );
        assert_eq!(filled[1].id, 1);
        assert!(filled[1].enabled);
        assert_eq!(filled[1].time, 661);
        assert!(filled[1].repeat.contains(AlarmRepeat::SUNDAY));
        assert_eq!(filled[1].volume, 75);
        assert_eq!(filled[1].snooze_duration_in_minutes, 5);

        let changed = d1301::decode_alarms(frame_body(ALARMS_CHANGED, [0x14, 0x01]))
            .expect("changed alarm list");
        assert_eq!(changed.len(), 2);
        assert_eq!(changed[0].id, 0);
        assert!(changed[0].enabled);
        assert_eq!(changed[0].time, 465);
        assert_eq!(changed[0].wake_up_tune, AlarmWakeUpTune::Glow);
        assert_eq!(changed[0].snooze_duration_in_minutes, 5);
        assert_eq!(changed[1], filled[1], "the other slot is preserved");

        // Re-encoding either list reproduces its captured body.
        let encode = |alarms: &[Alarm]| -> Vec<u8> {
            alarms.iter().flat_map(|alarm| alarm.bytes()).collect()
        };
        assert_eq!(encode(&filled), frame_body(ALARMS_FILLED, [0x14, 0x01]));
        assert_eq!(encode(&changed), frame_body(ALARMS_CHANGED, [0x14, 0x01]));
    }

    #[test]
    fn alarm_writes_match_the_captured_records() {
        let filled = d1301::decode_alarms(frame_body(ALARMS_FILLED, [0x14, 0x01]))
            .expect("captured alarm list");
        // The captured `set_alarm` update of slot 0 from the legacy builder tests.
        let updated = Alarm {
            id: 0,
            enabled: true,
            time: 465,
            repeat: AlarmRepeat::from_bits_truncate(0),
            wake_up_tune: AlarmWakeUpTune::Glow,
            volume: 50,
            snooze_duration_in_minutes: 5,
        };
        assert_eq!(
            d1301::set_alarm_body(&updated),
            [0x00, 0x01, 0xD1, 0x01, 0x00, 0x01, 0x32, 0x05]
        );
        // The captured `set_alarm` creation of slot 1 from the same tests.
        let created = Alarm {
            id: 1,
            enabled: true,
            time: 560,
            repeat: AlarmRepeat::SATURDAY | AlarmRepeat::SUNDAY,
            wake_up_tune: AlarmWakeUpTune::Nature,
            volume: 50,
            snooze_duration_in_minutes: 10,
        };
        assert_eq!(
            d1301::set_alarm_body(&created),
            [0x01, 0x01, 0x30, 0x02, 0x82, 0x00, 0x32, 0x0A]
        );
        assert_eq!(d1301::delete_alarm_body(1), [0x01]);
        assert_eq!(filled.len(), 2);
    }

    #[test]
    fn rejects_alarm_bodies_that_are_not_whole_records() {
        assert_eq!(
            d1301::decode_alarms(&[0; 7]),
            Err(ModelError::InvalidAlarmList)
        );
        assert_eq!(
            d1301::decode_alarms(&[0; 9]),
            Err(ModelError::InvalidAlarmList)
        );
    }

    #[test]
    fn confirmed_layout_requires_the_measured_state_length() {
        let body = frame_body(POST_SLEEP_PAUSE, [0x01, 0x01]);
        assert!(
            ModelDriver::D1301
                .decode_confirmed_state(&body[..d1301::STATE_LENGTH - 1])
                .is_none()
        );
        let mut longer = body.to_vec();
        longer.push(0);
        assert!(ModelDriver::D1301.decode_confirmed_state(&longer).is_none());
    }

    #[test]
    fn capabilities_cover_only_the_confirmed_sleep_settings() {
        let capabilities = ModelDriver::D1301.capabilities();
        assert!(capabilities.listening_mode_write);
        assert!(capabilities.default_listening_mode_write);
        assert!(capabilities.noise_canceling_write);
        assert!(capabilities.post_sleep_audio_write);
        assert!(capabilities.auto_switch_once_asleep_write);
        // The sleep earbuds do not report the shared mode, equalizer, gaming or
        // auto power off surfaces, so the session must not claim them.
        assert!(!capabilities.positioned_sound_mode_write);
        assert!(!capabilities.ambient_sound_mode_write);
        assert!(!capabilities.equalizer_write);
        assert!(!capabilities.gaming_mode_write);
        assert!(!capabilities.auto_power_off_write);
        assert!(
            !ModelDriver::D1301.equalizer_is_writable(openscl4_models::EqualizerSetting {
                preset_id: 0,
                volume_adjustments: [0; openscl4_models::EQUALIZER_BANDS],
            })
        );
        assert_eq!(
            ModelDriver::D1301.decode_state(&[0; 10]),
            Err(ModelError::IncompleteState)
        );
        assert_eq!(
            ModelDriver::D1301
                .decode_state(&[0; d1301::STATE_LENGTH])
                .err(),
            Some(ModelError::InvalidIdentity),
            "an all-zero state has no readable model serial"
        );
    }

    #[test]
    fn state_reports_no_sound_mode_for_the_sleep_earbuds() {
        let state = ModelDriver::D1301
            .decode_confirmed_state(frame_body(POST_SLEEP_PAUSE, [0x01, 0x01]))
            .expect("captured sleep state");
        assert_eq!(state.sound_mode, None, "the model has no ambient mode");
        assert_eq!(state.sound_mode_setting(), None);
        assert_eq!(state.equalizer, None);
        assert_eq!(state.gaming_mode, None);
        assert_eq!(state.auto_power_off, None);
        assert_eq!(
            state.sleep_audio.map(|sleep| sleep.action),
            Some(PostSleepAudio::Pause)
        );
        assert_eq!(
            d1301::D1301Setting::PostSleepAudio(PostSleepAudio::Pause).observed(state),
            Some(d1301::D1301Setting::PostSleepAudio(PostSleepAudio::Pause))
        );
        assert_eq!(
            d1301::D1301Setting::AutoSwitchOnceAsleep(false).observed(state),
            Some(d1301::D1301Setting::AutoSwitchOnceAsleep(true))
        );
    }
}
