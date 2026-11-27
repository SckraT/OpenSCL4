#![forbid(unsafe_code)]

#[cfg(test)]
#[path = "support/recording.rs"]
mod recording;

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use openscl4_models::{
        AutoPowerOff, AutoPowerOffSetting, EqualizerSetting, ModelDriver, ModelError, SoundMode,
        WEARING_DETECTION_COMMAND,
    };
    use openscl4_session::{
        CommandError, CommandOutcome, CommandResult, ConnectionState, DeviceState, PendingCommand,
        QuickPresetValues, SessionCommand, SessionHandle,
    };
    use openscl4_transport_mock::{VirtualA3030, VirtualA3031};

    use crate::recording::{Recorder, request};

    const IDENTITY_COMMAND: [u8; 2] = [0x01, 0x05];
    const STATE_COMMAND: [u8; 2] = [0x01, 0x01];
    const SOUND_MODE_COMMAND: [u8; 2] = [0x06, 0x81];
    const EQUALIZER_COMMAND: [u8; 2] = [0x02, 0x81];
    const AUTO_POWER_OFF_COMMAND: [u8; 2] = [0x01, 0x86];
    const REFERENCE_CURVE: EqualizerSetting = EqualizerSetting {
        preset_id: 0xfefe,
        volume_adjustments: [-60, 60, 23, 40, 22, 60, -4, 16, 0, 0],
    };

    #[tokio::test(start_paused = true)]
    async fn reads_the_identity_and_state_of_the_vortex() {
        let recorder = Recorder::new();
        let handle = recorder.start(VirtualA3031);
        let state = ready(&handle).await;
        assert_eq!(state.model, ModelDriver::A3031);
        assert_eq!(state.battery.left, Some(100));
        assert_eq!(state.battery.right, Some(80));
        assert_eq!(state.battery.device, None);
        assert_eq!(
            state
                .firmware_left
                .map(|firmware| firmware.to_string())
                .as_deref(),
            Some("01.10"),
            "firmware comes from the separate 01 05 identity response"
        );
        assert_eq!(
            state
                .firmware_right
                .map(|firmware| firmware.to_string())
                .as_deref(),
            Some("01.20")
        );
        assert_eq!(state.sound_mode, Some(SoundMode::Normal));
        assert_eq!(state.tws_connected, Some(true));
        assert_eq!(state.equalizer, Some(REFERENCE_CURVE));
        assert_eq!(
            state.auto_power_off,
            Some(AutoPowerOff {
                enabled: true,
                duration: 1,
            })
        );
        assert_eq!(state.wearing_detection, None);
        handle.close().await.expect("close virtual link");

        assert_eq!(recorder.commands(), vec![IDENTITY_COMMAND, STATE_COMMAND]);
    }

    #[tokio::test(start_paused = true)]
    async fn confirms_sound_mode_writes_and_keeps_the_identity_firmware() {
        let recorder = Recorder::new();
        let handle = recorder.start(VirtualA3031);
        let initial = ready(&handle).await;

        let CommandOutcome::Applied(changed) = wait(
            handle
                .dispatch(SessionCommand::SetAmbientSoundMode(
                    SoundMode::NoiseCanceling,
                ))
                .expect("queue noise canceling"),
        )
        .await
        .expect("confirm noise canceling");
        assert_eq!(changed.sound_mode, Some(SoundMode::NoiseCanceling));
        assert_eq!(changed.firmware_left, initial.firmware_left);
        assert_eq!(changed.firmware_right, initial.firmware_right);
        assert_eq!(changed.equalizer, initial.equalizer);

        let CommandOutcome::Applied(restored) = wait(
            handle
                .dispatch(SessionCommand::SetAmbientSoundMode(SoundMode::Normal))
                .expect("queue normal"),
        )
        .await
        .expect("confirm normal");
        assert_eq!(restored.sound_mode, Some(SoundMode::Normal));
        assert_eq!(
            recorder.count(&request(SOUND_MODE_COMMAND, &[0x00, 0x00, 0x01, 0x00])),
            1
        );
        assert_eq!(
            recorder.count(&request(SOUND_MODE_COMMAND, &[0x02, 0x00, 0x01, 0x00])),
            1
        );
        handle.close().await.expect("close virtual link");
    }

    #[tokio::test(start_paused = true)]
    async fn confirms_auto_power_off_writes_from_the_state_read_back() {
        let recorder = Recorder::new();
        let handle = recorder.start(VirtualA3031);
        ready(&handle).await;

        let CommandOutcome::Applied(disabled) = wait(
            handle
                .dispatch(SessionCommand::SetAutoPowerOff(
                    AutoPowerOffSetting::Disabled,
                ))
                .expect("queue disable"),
        )
        .await
        .expect("confirm disable");
        assert_eq!(
            disabled.auto_power_off,
            Some(AutoPowerOff {
                enabled: false,
                duration: 1,
            }),
            "disabling keeps the previous duration index"
        );

        let CommandOutcome::Applied(enabled) = wait(
            handle
                .dispatch(SessionCommand::SetAutoPowerOff(
                    AutoPowerOffSetting::Enabled(3),
                ))
                .expect("queue enable"),
        )
        .await
        .expect("confirm enable");
        assert_eq!(
            enabled.auto_power_off,
            Some(AutoPowerOff {
                enabled: true,
                duration: 3,
            })
        );
        assert_eq!(
            recorder.bodies(AUTO_POWER_OFF_COMMAND),
            vec![vec![0x00, 0x01], vec![0x01, 0x03]]
        );
        handle.close().await.expect("close virtual link");
    }

    #[tokio::test(start_paused = true)]
    async fn confirms_the_two_channel_curve_from_the_state_read_back() {
        let recorder = Recorder::new();
        let handle = recorder.start(VirtualA3031);
        assert_eq!(ready(&handle).await.equalizer, Some(REFERENCE_CURVE));

        let requested = EqualizerSetting {
            preset_id: 0,
            volume_adjustments: [0; 10],
        };
        handle.set_eq_live(requested).expect("accept live curve");
        let applied = wait_for_equalizer(&handle, requested).await;
        assert_eq!(applied.equalizer, Some(requested));
        assert_eq!(
            recorder.bodies(EQUALIZER_COMMAND),
            vec![vec![
                0x00, 0x00, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78,
                0x78, 0x78, 0x78, 0x78,
            ]],
            "the TWS write applies the same curve to both channels"
        );
        handle.close().await.expect("close virtual link");
    }

    #[tokio::test(start_paused = true)]
    async fn equalizer_is_unavailable_while_tws_is_disconnected() {
        let recorder = Recorder::new();
        let handle = recorder.start(VirtualA3031::with_disconnected_tws());
        let initial = ready(&handle).await;
        assert_eq!(initial.sound_mode, Some(SoundMode::Normal));
        assert_eq!(initial.tws_connected, Some(false));
        assert_eq!(
            initial.equalizer, None,
            "the legacy equalizer setting is unavailable while TWS is disconnected"
        );

        let result = wait(
            handle
                .dispatch(SessionCommand::ApplyQuickPreset(QuickPresetValues {
                    sound_mode: None,
                    ambient_sound_mode: None,
                    equalizer: Some(REFERENCE_CURVE),
                }))
                .expect("queue the equalizer preset"),
        )
        .await;
        assert_eq!(
            result,
            Err(CommandError::Model(ModelError::InvalidEqualizerWrite))
        );
        assert!(recorder.bodies(EQUALIZER_COMMAND).is_empty());
        handle.close().await.expect("close virtual link");
    }

    #[tokio::test(start_paused = true)]
    async fn wearing_detection_is_rejected_before_transmission_on_a3031() {
        let recorder = Recorder::new();
        let handle = recorder.start(VirtualA3031);
        let initial = ready(&handle).await;
        let result = wait(
            handle
                .dispatch(SessionCommand::SetWearingDetection(true))
                .expect("queue the typed rejection"),
        )
        .await;
        assert_eq!(
            result,
            Err(CommandError::Model(
                ModelError::InvalidWearingDetectionWrite
            ))
        );
        assert!(recorder.bodies(WEARING_DETECTION_COMMAND).is_empty());
        assert_eq!(ready(&handle).await, initial);
        handle.close().await.expect("close virtual link");
    }

    #[tokio::test(start_paused = true)]
    async fn a3030_keeps_the_a3031_operations_unsupported() {
        let recorder = Recorder::new();
        let handle = recorder.start(VirtualA3030);
        ready(&handle).await;
        let result = wait(
            handle
                .dispatch(SessionCommand::SetAutoPowerOff(
                    AutoPowerOffSetting::Enabled(0),
                ))
                .expect("queue the typed rejection"),
        )
        .await;
        assert_eq!(
            result,
            Err(CommandError::Model(ModelError::InvalidAutoPowerOffWrite))
        );
        assert!(recorder.bodies(AUTO_POWER_OFF_COMMAND).is_empty());
        handle.close().await.expect("close virtual link");
    }

    async fn ready(handle: &SessionHandle) -> DeviceState {
        let view = tokio::time::timeout(Duration::from_secs(3), handle.confirmed_view())
            .await
            .expect("sync deadline")
            .expect("confirmed state");
        let ConnectionState::Ready(state) = view.connection else {
            panic!("expected a confirmed state");
        };
        state
    }

    async fn wait(pending: PendingCommand) -> CommandResult {
        tokio::time::timeout(Duration::from_secs(10), pending.wait())
            .await
            .expect("command deadline")
    }

    async fn wait_for_equalizer(
        handle: &SessionHandle,
        requested: EqualizerSetting,
    ) -> DeviceState {
        let mut view = handle.subscribe();
        let snapshot = tokio::time::timeout(
            Duration::from_secs(3),
            view.wait_for(
                |view| matches!(view.connection, ConnectionState::Ready(state) if state.equalizer == Some(requested)),
            ),
        )
        .await
        .expect("the confirmed curve deadline")
        .expect("confirmed curve");
        let ConnectionState::Ready(state) = snapshot.connection else {
            unreachable!("wait_for matched a ready state");
        };
        state
    }
}
