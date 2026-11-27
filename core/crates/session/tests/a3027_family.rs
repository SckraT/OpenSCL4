#![forbid(unsafe_code)]

#[cfg(test)]
#[path = "support/recording.rs"]
mod recording;

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use openscl4_models::{
        AutoPowerOffSetting, EqualizerSetting, ModelDriver, ModelError, SoundMode,
        WEARING_DETECTION_COMMAND,
    };
    use openscl4_session::{
        CommandError, CommandOutcome, CommandResult, ConnectionState, DeviceState, PendingCommand,
        SessionCommand, SessionHandle,
    };
    use openscl4_transport::TransportProvider;
    use openscl4_transport_mock::{VirtualA3027, VirtualA3028, VirtualA3030};

    use crate::recording::{Recorder, request};

    const IDENTITY_COMMAND: [u8; 2] = [0x01, 0x05];
    const STATE_COMMAND: [u8; 2] = [0x01, 0x01];
    const SOUND_MODE_COMMAND: [u8; 2] = [0x06, 0x81];
    const EQUALIZER_COMMAND: [u8; 2] = [0x02, 0x81];
    const REFERENCE_CURVE: EqualizerSetting = EqualizerSetting {
        preset_id: 0xfefe,
        volume_adjustments: [-60, 60, 23, 40, 22, 60, -4, 16, 0, 0],
    };

    #[tokio::test(start_paused = true)]
    async fn reads_the_reference_state_for_each_public_code() {
        reference_state(VirtualA3027, ModelDriver::A3027).await;
        reference_state(VirtualA3030, ModelDriver::A3030).await;
    }

    #[tokio::test(start_paused = true)]
    async fn confirms_the_reference_mode_cycle_from_fresh_state_reads() {
        mode_cycle(VirtualA3027, ModelDriver::A3027).await;
        mode_cycle(VirtualA3030, ModelDriver::A3030).await;
    }

    #[tokio::test(start_paused = true)]
    async fn confirms_the_eight_band_curve_from_the_state_read_back() {
        let recorder = Recorder::new();
        let handle = recorder.start(VirtualA3027);
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
                0x00, 0x00, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78,
            ]],
            "the legacy eight-band signature body"
        );
        handle.close().await.expect("close virtual link");
    }

    #[tokio::test(start_paused = true)]
    async fn confirms_wearing_detection_from_the_state_read_back() {
        wearing_detection(VirtualA3027, ModelDriver::A3027).await;
        wearing_detection(VirtualA3030, ModelDriver::A3030).await;
    }

    #[tokio::test(start_paused = true)]
    async fn wearing_detection_is_rejected_before_transmission_when_unsupported() {
        let recorder = Recorder::new();
        let handle = recorder.start(VirtualA3028);
        let initial = ready(&handle).await;

        let result = wait(
            handle
                .dispatch(SessionCommand::SetWearingDetection(false))
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
    async fn a3028_keeps_auto_power_off_unsupported() {
        let recorder = Recorder::new();
        let handle = recorder.start(VirtualA3028);
        ready(&handle).await;
        let result = wait(
            handle
                .dispatch(SessionCommand::SetAutoPowerOff(
                    AutoPowerOffSetting::Disabled,
                ))
                .expect("queue the typed rejection"),
        )
        .await;
        assert_eq!(
            result,
            Err(CommandError::Model(ModelError::InvalidAutoPowerOffWrite))
        );
        assert!(recorder.bodies([0x01, 0x86]).is_empty());
        handle.close().await.expect("close virtual link");
    }

    async fn reference_state<P: TransportProvider>(provider: P, model: ModelDriver) {
        let recorder = Recorder::new();
        let handle = recorder.start(provider);
        let state = ready(&handle).await;
        assert_eq!(state.model, model);
        assert_eq!(state.battery.device, Some(100));
        assert_eq!(state.sound_mode, Some(SoundMode::Normal));
        assert_eq!(state.equalizer, Some(REFERENCE_CURVE));
        assert_eq!(state.wearing_detection, Some(true));
        assert_eq!(
            state
                .firmware_device
                .map(|firmware| firmware.to_string())
                .as_deref(),
            Some("01.20")
        );
        handle.close().await.expect("close virtual link");

        // No identity response is recorded for the pair, so the session probes
        // the state once and reads it without additional writes.
        assert_eq!(recorder.commands(), vec![IDENTITY_COMMAND, STATE_COMMAND]);
    }

    async fn mode_cycle<P: TransportProvider>(provider: P, model: ModelDriver) {
        let recorder = Recorder::new();
        let handle = recorder.start(provider);
        let initial = ready(&handle).await;
        assert_eq!(initial.model, model);
        assert_eq!(initial.sound_mode, Some(SoundMode::Normal));

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
        assert_eq!(changed.equalizer, initial.equalizer);

        let CommandOutcome::Applied(restored) = wait(
            handle
                .dispatch(SessionCommand::SetAmbientSoundMode(SoundMode::Normal))
                .expect("queue normal"),
        )
        .await
        .expect("confirm normal");
        assert_eq!(restored.sound_mode, Some(SoundMode::Normal));
        assert_eq!(restored.equalizer, initial.equalizer);
        assert_eq!(ready(&handle).await, restored);

        assert_eq!(
            recorder.count(&request(SOUND_MODE_COMMAND, &[0x00, 0x00, 0x01, 0x00])),
            1,
            "the measured noise-canceling body"
        );
        assert_eq!(
            recorder.count(&request(SOUND_MODE_COMMAND, &[0x02, 0x00, 0x01, 0x00])),
            1,
            "the measured normal body"
        );
        // Identity probe, state sync, then each write is preceded by a fresh
        // state read and confirmed by a read-back.
        assert_eq!(
            recorder.commands(),
            vec![
                IDENTITY_COMMAND,
                STATE_COMMAND,
                STATE_COMMAND,
                SOUND_MODE_COMMAND,
                STATE_COMMAND,
                STATE_COMMAND,
                SOUND_MODE_COMMAND,
                STATE_COMMAND,
            ]
        );
        handle.close().await.expect("close virtual link");
    }

    async fn wearing_detection<P: TransportProvider>(provider: P, model: ModelDriver) {
        let recorder = Recorder::new();
        let handle = recorder.start(provider);
        let initial = ready(&handle).await;
        assert_eq!(initial.model, model);
        assert_eq!(initial.wearing_detection, Some(true));

        let CommandOutcome::Applied(disabled) = wait(
            handle
                .dispatch(SessionCommand::SetWearingDetection(false))
                .expect("queue wearing detection off"),
        )
        .await
        .expect("confirm wearing detection off");
        assert_eq!(disabled.wearing_detection, Some(false));

        let CommandOutcome::Applied(enabled) = wait(
            handle
                .dispatch(SessionCommand::SetWearingDetection(true))
                .expect("queue wearing detection on"),
        )
        .await
        .expect("confirm wearing detection on");
        assert_eq!(enabled.wearing_detection, Some(true));

        // The legacy flag write is sent without waiting for a response; the
        // following state read confirms it.
        assert_eq!(
            recorder.bodies(WEARING_DETECTION_COMMAND),
            vec![vec![0x00], vec![0x01]]
        );
        assert_eq!(ready(&handle).await, enabled);
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
