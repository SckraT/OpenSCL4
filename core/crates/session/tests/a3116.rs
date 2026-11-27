#![forbid(unsafe_code)]

#[cfg(test)]
mod tests {
    use std::{
        sync::{Arc, Mutex},
        time::Duration,
    };

    use async_trait::async_trait;
    use bytes::Bytes;
    use openscl4_models::{
        A3116_EQUALIZER_BANDS_COMMAND, A3116_EQUALIZER_PRESET_COMMAND, A3116_VOICE_PROMPT_COMMAND,
        A3116_VOICE_PROMPT_READ_COMMAND, A3116_VOLUME_COMMAND, AutoPowerOff, AutoPowerOffSetting,
        ModelError,
    };
    use openscl4_session::{
        CommandError, CommandOutcome, CommandResult, ConnectionState, DeviceState,
        EqualizerSetting, IoStage, ModelDriver, PendingCommand, SessionCommand, SessionError,
        SessionHandle, SessionRunner,
    };
    use openscl4_transport::{
        ConnectTarget, LinkRx, LinkTx, RedactedMac, TransportError, TransportLink,
        TransportProvider,
    };
    use openscl4_transport_mock::{MockDisconnect, VirtualA3116};

    // Golden outbound frames from the legacy A3116 builders. The speaker's
    // frames carry no checksum: the length field counts the nine header bytes
    // and the body only.
    const CHECKSUMMED_IDENTITY_PROBE: &[u8] = b"\x08\xee\0\0\0\x01\x05\x0a\0\x06";
    const CHECKSUMMED_STATE_READ: &[u8] = b"\x08\xee\0\0\0\x01\x01\x0a\0\x02";
    const STATE_READ: &[u8] = b"\x08\xee\0\0\0\x01\x01\x09\0";
    const VOICE_PROMPT_READ: &[u8] = b"\x08\xee\0\0\0\x01\x10\x09\0";
    const VOLUME_WRITE: &[u8] = b"\x08\xee\0\0\0\x01\x81\x0a\0\x0c";
    const EQ_PRESET_WRITE: &[u8] = b"\x08\xee\0\0\0\x02\x81\x0a\0\x0f";
    const EQ_BANDS_WRITE: &[u8] = b"\x08\xee\0\0\0\x02\x83\x12\0\0\x01\x02\x03\x04\x05\x06\x07\x08";
    const VOICE_PROMPT_WRITE: &[u8] = b"\x08\xee\0\0\0\x01\x90\x0a\0\0";
    const AUTO_POWER_OFF_WRITE: &[u8] = b"\x08\xee\0\0\0\x01\x83\x0a\0\x04";
    const POWER_OFF: &[u8] = b"\x08\xee\0\0\0\x01\x85\x09\0";

    #[tokio::test(start_paused = true)]
    async fn reads_the_virtual_a3116_state_from_the_checksum_less_framing() {
        let frames = Arc::new(Mutex::new(Vec::new()));
        let handle = start(VirtualA3116, Arc::clone(&frames));
        let state = ready(&handle).await;
        assert_eq!(state.model, ModelDriver::A3116);
        assert_eq!(state.battery.device, Some(100));
        assert_eq!(state.battery_charging, Some(false));
        assert_eq!(state.volume, Some(7));
        assert_eq!(
            state.firmware_device.map(|firmware| firmware.to_string()),
            Some("38.39".to_owned())
        );
        assert_eq!(
            state.equalizer,
            Some(EqualizerSetting {
                preset_id: 255,
                volume_adjustments: [0, 10, 20, 30, 40, 0, -10, -20, -30, 0],
            })
        );
        assert_eq!(
            state.auto_power_off,
            Some(AutoPowerOff {
                enabled: true,
                duration: 1,
            })
        );
        assert_eq!(state.voice_prompt, Some(true));
        handle.close().await.expect("close the virtual link");

        assert_eq!(
            *frames.lock().expect("request trace"),
            vec![
                CHECKSUMMED_IDENTITY_PROBE.to_vec(),
                CHECKSUMMED_STATE_READ.to_vec(),
                STATE_READ.to_vec(),
                VOICE_PROMPT_READ.to_vec(),
            ]
        );
    }

    #[tokio::test(start_paused = true)]
    async fn writes_volume_and_confirms_it_from_a_fresh_state() {
        let frames = Arc::new(Mutex::new(Vec::new()));
        let handle = start(VirtualA3116, Arc::clone(&frames));
        ready(&handle).await;

        let outcome = wait(
            handle
                .dispatch(SessionCommand::SetVolume(12))
                .expect("queue the volume write"),
        )
        .await
        .expect("confirm the volume write");
        let CommandOutcome::Applied(state) = outcome else {
            panic!("the volume write was not applied");
        };
        assert_eq!(state.volume, Some(12));
        handle.close().await.expect("close the virtual link");

        assert_eq!(
            count(&frames, VOLUME_WRITE),
            1,
            "the write body is the legacy volume byte"
        );
        assert_eq!(
            count(&frames, STATE_READ),
            3,
            "initial read, write read-current and read-back"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn writes_the_two_frame_equalizer_and_confirms_the_curve() {
        let frames = Arc::new(Mutex::new(Vec::new()));
        let handle = start(VirtualA3116, Arc::clone(&frames));
        ready(&handle).await;

        let requested = EqualizerSetting {
            preset_id: 15,
            volume_adjustments: [-60, -50, -40, -30, -20, -10, 0, 10, 20, 0],
        };
        let outcome = wait(
            handle
                .dispatch(SessionCommand::ApplyQuickPreset(
                    openscl4_session::QuickPresetValues {
                        equalizer: Some(requested),
                        ..openscl4_session::QuickPresetValues::default()
                    },
                ))
                .expect("queue the equalizer write"),
        )
        .await
        .expect("confirm the equalizer write");
        let CommandOutcome::Applied(state) = outcome else {
            panic!("the equalizer write was not applied");
        };
        assert_eq!(state.equalizer, Some(requested));
        handle.close().await.expect("close the virtual link");

        // The legacy state modifier sends the preset before the band block.
        let trace = frames.lock().expect("request trace").clone();
        let equalizer_frames: Vec<&[u8]> = trace
            .iter()
            .map(Vec::as_slice)
            .filter(|frame| {
                frame.get(5..7) == Some(A3116_EQUALIZER_PRESET_COMMAND.as_slice())
                    || frame.get(5..7) == Some(A3116_EQUALIZER_BANDS_COMMAND.as_slice())
            })
            .collect();
        assert_eq!(
            equalizer_frames,
            vec![EQ_PRESET_WRITE, EQ_BANDS_WRITE],
            "preset and band frames in the legacy order"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn writes_the_voice_prompt_and_confirms_it_with_the_targeted_read() {
        let frames = Arc::new(Mutex::new(Vec::new()));
        let handle = start(VirtualA3116, Arc::clone(&frames));
        ready(&handle).await;

        let outcome = wait(
            handle
                .dispatch(SessionCommand::SetVoicePrompt(false))
                .expect("queue the voice prompt write"),
        )
        .await
        .expect("confirm the voice prompt write");
        let CommandOutcome::Applied(state) = outcome else {
            panic!("the voice prompt write was not applied");
        };
        assert_eq!(state.voice_prompt, Some(false));
        handle.close().await.expect("close the virtual link");

        assert_eq!(count(&frames, VOICE_PROMPT_WRITE), 1);
        // The initial sync read and the write read-back use the targeted read.
        let trace = frames.lock().expect("request trace").clone();
        let targeted_reads = trace
            .iter()
            .filter(|frame| frame.as_slice() == VOICE_PROMPT_READ)
            .count();
        assert_eq!(targeted_reads, 2, "the targeted flag is read back once");
    }

    #[tokio::test(start_paused = true)]
    async fn writes_the_auto_power_off_duration_and_confirms_it() {
        let frames = Arc::new(Mutex::new(Vec::new()));
        let handle = start(VirtualA3116, Arc::clone(&frames));
        ready(&handle).await;

        let outcome = wait(
            handle
                .dispatch(SessionCommand::SetAutoPowerOff(
                    AutoPowerOffSetting::Enabled(3),
                ))
                .expect("queue the auto power off write"),
        )
        .await
        .expect("confirm the auto power off write");
        let CommandOutcome::Applied(state) = outcome else {
            panic!("the auto power off write was not applied");
        };
        assert_eq!(
            state.auto_power_off,
            Some(AutoPowerOff {
                enabled: true,
                duration: 3,
            })
        );
        handle.close().await.expect("close the virtual link");
        assert_eq!(count(&frames, AUTO_POWER_OFF_WRITE), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn power_off_is_accepted_on_the_acknowledgement_and_the_link_ends() {
        let frames = Arc::new(Mutex::new(Vec::new()));
        let disconnect = Arc::new(Mutex::new(None));
        let handle = SessionRunner::start(
            DisconnectingA3116 {
                frames: Arc::clone(&frames),
                disconnect: Arc::clone(&disconnect),
            },
            target(),
        );
        let confirmed = ready(&handle).await;

        let outcome = wait(
            handle
                .dispatch(SessionCommand::PowerOff)
                .expect("queue the power off action"),
        )
        .await
        .expect("accept the power off action");
        assert_eq!(outcome, CommandOutcome::Accepted(confirmed));
        assert_eq!(count(&frames, POWER_OFF), 1);

        // The speaker loses its link after powering off; the session reports
        // the expected termination instead of a confirmation of a setting.
        disconnect
            .lock()
            .expect("disconnect handle")
            .as_ref()
            .expect("the virtual link exists")
            .disconnect();
        let mut view = handle.subscribe();
        let snapshot = tokio::time::timeout(
            Duration::from_secs(3),
            view.wait_for(|view| view.connection.is_terminal()),
        )
        .await
        .expect("session termination deadline")
        .expect("session termination");
        assert_eq!(
            snapshot.connection,
            ConnectionState::Failed(SessionError::Transport {
                stage: IoStage::Receive,
                error: TransportError::Disconnected,
            })
        );
    }

    #[tokio::test(start_paused = true)]
    async fn firmware_before_38_39_has_no_voice_prompt_and_no_targeted_read() {
        let frames = Arc::new(Mutex::new(Vec::new()));
        let handle = start(VirtualA3116::with_old_firmware(), Arc::clone(&frames));
        let state = ready(&handle).await;
        assert_eq!(
            state.firmware_device.map(|firmware| firmware.to_string()),
            Some("00.99".to_owned())
        );
        assert_eq!(state.voice_prompt, None);

        let result = wait(
            handle
                .dispatch(SessionCommand::SetVoicePrompt(true))
                .expect("queue the rejected voice prompt write"),
        )
        .await;
        assert_eq!(
            result,
            Err(CommandError::Model(ModelError::InvalidVoicePromptWrite))
        );
        handle.close().await.expect("close the virtual link");

        assert_eq!(
            frames
                .lock()
                .expect("request trace")
                .iter()
                .filter(|frame| frame.get(5..7) == Some(A3116_VOICE_PROMPT_READ_COMMAND.as_slice()))
                .count(),
            0,
            "older firmware is not asked for the targeted flag"
        );
        assert_eq!(
            frames
                .lock()
                .expect("request trace")
                .iter()
                .filter(|frame| frame.get(5..7) == Some(A3116_VOICE_PROMPT_COMMAND.as_slice()))
                .count(),
            0,
            "an unavailable flag is rejected before transmission"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn rejects_out_of_range_values_without_writing_them() {
        let frames = Arc::new(Mutex::new(Vec::new()));
        let handle = start(VirtualA3116, Arc::clone(&frames));
        ready(&handle).await;

        let result = wait(
            handle
                .dispatch(SessionCommand::SetVolume(openscl4_models::VOLUME_MAX + 1))
                .expect("queue the rejected volume"),
        )
        .await;
        assert_eq!(
            result,
            Err(CommandError::Model(ModelError::InvalidVolumeWrite))
        );
        let result = wait(
            handle
                .dispatch(SessionCommand::ApplyQuickPreset(
                    openscl4_session::QuickPresetValues {
                        equalizer: Some(EqualizerSetting {
                            preset_id: 0,
                            volume_adjustments: [5, 0, 0, 0, 0, 0, 0, 0, 0, 0],
                        }),
                        ..openscl4_session::QuickPresetValues::default()
                    },
                ))
                .expect("queue the rejected curve"),
        )
        .await;
        assert_eq!(
            result,
            Err(CommandError::Model(ModelError::InvalidEqualizerWrite))
        );
        handle.close().await.expect("close the virtual link");

        let trace = frames.lock().expect("request trace").clone();
        assert!(
            trace
                .iter()
                .all(|frame| frame.get(5..7) != Some(A3116_VOLUME_COMMAND.as_slice())),
            "the rejected volume was never written"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn the_write_read_back_starts_from_fresh_state() {
        // Any write first reads the current state, then transmits the command,
        // then reads the state again. The trace shows that order for a volume
        // write: read, `01 81`, read.
        let frames = Arc::new(Mutex::new(Vec::new()));
        let handle = start(VirtualA3116, Arc::clone(&frames));
        ready(&handle).await;
        wait(
            handle
                .dispatch(SessionCommand::SetVolume(3))
                .expect("queue the volume write"),
        )
        .await
        .expect("confirm the volume write");
        handle.close().await.expect("close the virtual link");

        let trace = frames.lock().expect("request trace").clone();
        let commands: Vec<[u8; 2]> = trace
            .iter()
            .filter_map(|frame| frame.get(5..7)?.try_into().ok())
            .collect();
        let volume_index = commands
            .iter()
            .position(|command| *command == A3116_VOLUME_COMMAND)
            .expect("the volume write");
        assert_eq!(commands[volume_index - 1], [0x01, 0x01]);
        assert_eq!(commands[volume_index + 1], [0x01, 0x01]);
    }

    async fn ready(handle: &SessionHandle) -> DeviceState {
        // The unavailable identity probe and the checksummed state probe each
        // take the reply deadline before the checksum-less framing is tried.
        let view = tokio::time::timeout(Duration::from_secs(10), handle.confirmed_view())
            .await
            .expect("sync deadline")
            .expect("confirmed state");
        let ConnectionState::Ready(state) = view.connection else {
            panic!("expected a confirmed A3116 state");
        };
        state
    }

    async fn wait(pending: PendingCommand) -> CommandResult {
        tokio::time::timeout(Duration::from_secs(10), pending.wait())
            .await
            .expect("command deadline")
    }

    fn target() -> ConnectTarget {
        ConnectTarget {
            mac: RedactedMac::from_octets([0; 6]),
        }
    }

    fn count(frames: &Arc<Mutex<Vec<Vec<u8>>>>, expected: &[u8]) -> usize {
        frames
            .lock()
            .expect("request trace")
            .iter()
            .filter(|frame| frame.as_slice() == expected)
            .count()
    }

    fn start<P: TransportProvider>(provider: P, frames: Arc<Mutex<Vec<Vec<u8>>>>) -> SessionHandle {
        SessionRunner::start(
            Recording {
                inner: provider,
                frames,
            },
            target(),
        )
    }

    struct Recording<P> {
        inner: P,
        frames: Arc<Mutex<Vec<Vec<u8>>>>,
    }

    #[async_trait]
    impl<P: TransportProvider> TransportProvider for Recording<P> {
        async fn connect(
            &self,
            target: &ConnectTarget,
        ) -> Result<Box<dyn TransportLink>, TransportError> {
            let (tx, rx) = self.inner.connect(target).await?.split();
            Ok(Box::new(RecordingLink {
                tx: RecordingTx {
                    inner: tx,
                    frames: Arc::clone(&self.frames),
                },
                rx,
            }))
        }
    }

    struct RecordingLink {
        tx: RecordingTx,
        rx: Box<dyn LinkRx>,
    }

    impl TransportLink for RecordingLink {
        fn split(self: Box<Self>) -> (Box<dyn LinkTx>, Box<dyn LinkRx>) {
            (Box::new(self.tx), self.rx)
        }
    }

    struct RecordingTx {
        inner: Box<dyn LinkTx>,
        frames: Arc<Mutex<Vec<Vec<u8>>>>,
    }

    #[async_trait]
    impl LinkTx for RecordingTx {
        async fn send(&mut self, request: Bytes) -> Result<(), TransportError> {
            self.frames
                .lock()
                .expect("request trace")
                .push(request.to_vec());
            self.inner.send(request).await
        }

        async fn close(&mut self) {
            self.inner.close().await;
        }
    }

    struct DisconnectingA3116 {
        frames: Arc<Mutex<Vec<Vec<u8>>>>,
        disconnect: Arc<Mutex<Option<MockDisconnect>>>,
    }

    #[async_trait]
    impl TransportProvider for DisconnectingA3116 {
        async fn connect(
            &self,
            _target: &ConnectTarget,
        ) -> Result<Box<dyn TransportLink>, TransportError> {
            let (link, disconnect) = VirtualA3116.connect_with_disconnect();
            *self.disconnect.lock().expect("disconnect handle") = Some(disconnect);
            let (tx, rx) = link.split();
            Ok(Box::new(RecordingLink {
                tx: RecordingTx {
                    inner: tx,
                    frames: Arc::clone(&self.frames),
                },
                rx,
            }))
        }
    }
}
