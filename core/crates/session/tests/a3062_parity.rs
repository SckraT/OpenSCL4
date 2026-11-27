//! Exercises the A3062 parity operations of the modular session.
//!
//! Every write is checked against the frame the legacy driver builds and a
//! read-back derived from the measured `core/data/a3062-pilot-state.bin`
//! capture. The peer list replay is the masked `0B 01` capture.

#![forbid(unsafe_code)]

#[cfg(test)]
#[path = "support/capture_replay.rs"]
mod capture_replay;

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;

    use bytes::Bytes;
    use openscl4_models::{
        AmbientSoundModeCycle, AutoPowerOff, AutoPowerOffSetting, ButtonAction, IDENTITY_COMMAND,
        LimitHighVolume, ModelDriver, PEERS_COMMAND, PeerDevice, STATE_COMMAND,
        VolumeLimitRefreshRate,
    };
    use openscl4_protocol::{Direction, encode_frame};
    use openscl4_session::{
        CommandError, CommandOutcome, ConnectionState, DeviceState, SessionCommand, SessionHandle,
        SessionRunner,
    };
    use openscl4_transport::{ConnectTarget, RedactedMac};

    use super::capture_replay::{Exchange, Replay};

    const STATE_FRAME: &[u8] = include_bytes!("../../../data/a3062-pilot-state.bin");
    const PEERS_FRAME: &[u8] = include_bytes!("../../../data/a3062-dual-connections.bin");

    const BUTTON_CONFIGURATION_COMMAND: [u8; 2] = [0x04, 0x81];
    const SOUND_MODE_CYCLE_COMMAND: [u8; 2] = [0x06, 0x82];
    const LIMIT_HIGH_VOLUME_COMMAND: [u8; 2] = [0x20, 0x82];
    const LIMIT_HIGH_VOLUME_REFRESH_RATE_COMMAND: [u8; 2] = [0x20, 0x81];
    const AUTO_POWER_OFF_COMMAND: [u8; 2] = [0x01, 0x86];
    const DUAL_CONNECTIONS_COMMAND: [u8; 2] = [0x0b, 0x84];
    const LDAC_COMMAND: [u8; 2] = [0x01, 0xff];
    const LDAC_STATE_COMMAND: [u8; 2] = [0x01, 0x7f];
    const LOW_BATTERY_PROMPT_COMMAND: [u8; 2] = [0x10, 0x82];
    const DOLBY_AUDIO_COMMAND: [u8; 2] = [0x02, 0x86];
    const SIDE_TONE_COMMAND: [u8; 2] = [0x01, 0x84];
    const VOICE_PROMPT_COMMAND: [u8; 2] = [0x01, 0xae];

    fn request(command: [u8; 2], body: &[u8]) -> Bytes {
        encode_frame(Direction::Outbound, command, body).expect("outbound frame")
    }

    fn reply(command: [u8; 2], body: &[u8]) -> Bytes {
        encode_frame(Direction::Inbound, command, body).expect("inbound frame")
    }

    fn frame_body(fixture: &[u8]) -> Bytes {
        let mut decoder = openscl4_protocol::FrameDecoder::default();
        decoder.push(fixture);
        let frame = decoder
            .next_frame()
            .expect("valid fixture")
            .expect("complete fixture");
        assert_eq!(frame.command, STATE_COMMAND);
        frame.body
    }

    fn start(exchanges: VecDeque<Exchange>) -> SessionHandle {
        SessionRunner::start(
            Replay(exchanges),
            ConnectTarget {
                mac: RedactedMac::from_octets([0; 6]),
            },
        )
    }

    async fn ready(handle: &SessionHandle) -> DeviceState {
        let view = handle
            .confirmed_view()
            .await
            .expect("confirmed A3062 state");
        let ConnectionState::Ready(state) = view.connection else {
            panic!("expected a confirmed A3062 state");
        };
        state
    }

    /// Builds the replay of the measured operations in one session.
    struct ReplayPlan {
        exchanges: VecDeque<Exchange>,
        body: Vec<u8>,
    }

    impl ReplayPlan {
        fn new() -> Self {
            let body = frame_body(STATE_FRAME).to_vec();
            let exchanges = VecDeque::from([
                (request(IDENTITY_COMMAND, &[]), None),
                (
                    request(STATE_COMMAND, &[]),
                    Some(reply(STATE_COMMAND, &body)),
                ),
                (
                    request(PEERS_COMMAND, &[]),
                    Some(Bytes::from_static(PEERS_FRAME)),
                ),
            ]);
            Self { exchanges, body }
        }

        fn state(&self) -> Exchange {
            (
                request(STATE_COMMAND, &[]),
                Some(reply(STATE_COMMAND, &self.body)),
            )
        }

        /// A write the legacy driver sends with an acknowledgement: a fresh
        /// state read, the write, its empty ack and the read-back.
        fn acknowledged(
            &mut self,
            command: [u8; 2],
            wire: &[u8],
            read_back: impl FnOnce(&mut [u8]),
        ) {
            self.exchanges.push_back(self.state());
            self.exchanges
                .push_back((request(command, wire), Some(reply(command, &[]))));
            read_back(&mut self.body);
            self.exchanges.push_back(self.state());
        }

        /// A flag the legacy driver sends without an acknowledgement: a fresh
        /// state read, the write and the read-back of the next state response.
        fn state_read(&mut self, command: [u8; 2], wire: &[u8], read_back: impl FnOnce(&mut [u8])) {
            self.exchanges.push_back(self.state());
            self.exchanges.push_back((request(command, wire), None));
            read_back(&mut self.body);
            self.exchanges.push_back(self.state());
        }

        /// The LDAC write, its dedicated `01 7F` confirmation and the snapshot.
        fn targeted(&mut self, wire: &[u8], read_back: impl FnOnce(&mut [u8])) {
            self.exchanges.push_back(self.state());
            self.exchanges
                .push_back((request(LDAC_COMMAND, wire), None));
            self.exchanges.push_back((
                request(LDAC_STATE_COMMAND, &[]),
                Some(reply(LDAC_STATE_COMMAND, &[1])),
            ));
            read_back(&mut self.body);
            self.exchanges.push_back(self.state());
        }
    }

    #[tokio::test(start_paused = true)]
    async fn reads_the_masked_multipoint_peer_capture() {
        let plan = ReplayPlan::new();
        let handle = start(plan.exchanges);
        let view = handle.confirmed_view().await.expect("confirmed state");
        let ConnectionState::Ready(state) = view.connection else {
            panic!("expected the confirmed A3062 state");
        };
        assert_eq!(state.model, ModelDriver::A3062);
        assert_eq!(
            view.peers.as_deref(),
            Some(
                [
                    PeerDevice {
                        connected: true,
                        mac: [0x02, 0, 0, 0, 0, 0x01],
                        name: "Desktop".to_owned(),
                    },
                    PeerDevice {
                        connected: false,
                        mac: [0x02, 0, 0, 0, 0, 0x02],
                        name: "Phone model".to_owned(),
                    },
                ]
                .as_slice()
            )
        );
        handle.close().await.expect("release the link");
    }

    #[tokio::test(start_paused = true)]
    async fn confirms_every_legacy_a3062_setting_from_capture_derived_read_back() {
        let mut plan = ReplayPlan::new();
        plan.acknowledged(BUTTON_CONFIGURATION_COMMAND, &[0, 0, 7], |body| {
            body[67] = 7;
        });
        plan.acknowledged(SOUND_MODE_CYCLE_COMMAND, &[0x05], |body| body[68] = 0x05);
        plan.acknowledged(LIMIT_HIGH_VOLUME_COMMAND, &[1, 80], |body| {
            body[82..84].copy_from_slice(&[1, 80]);
        });
        plan.acknowledged(LIMIT_HIGH_VOLUME_REFRESH_RATE_COMMAND, &[2], |body| {
            body[84] = 2;
        });
        plan.acknowledged(AUTO_POWER_OFF_COMMAND, &[1, 2], |body| {
            body[80..82].copy_from_slice(&[1, 2]);
        });
        plan.acknowledged(AUTO_POWER_OFF_COMMAND, &[0, 2], |body| {
            body[80..82].copy_from_slice(&[0, 2]);
        });
        plan.acknowledged(DUAL_CONNECTIONS_COMMAND, &[0], |body| body[79] = 0);
        plan.targeted(&[1], |body| body[78] = 1);
        plan.state_read(LOW_BATTERY_PROMPT_COMMAND, &[0], |body| body[76] = 0);
        plan.state_read(DOLBY_AUDIO_COMMAND, &[1], |body| body[77] = 1);
        plan.state_read(SIDE_TONE_COMMAND, &[0], |body| body[85] = 0);
        plan.state_read(VOICE_PROMPT_COMMAND, &[0], |body| body[86] = 0);

        let handle = start(plan.exchanges);
        ready(&handle).await;

        let state = apply(
            &handle,
            SessionCommand::SetButtonDoublePress(Some(ButtonAction::BassUp)),
        )
        .await;
        assert_eq!(
            state.settings.button_double_press,
            Some(ButtonAction::BassUp)
        );

        let state = apply(
            &handle,
            SessionCommand::SetAmbientSoundModeCycle(AmbientSoundModeCycle {
                noise_canceling: true,
                transparency: false,
                normal: true,
            }),
        )
        .await;
        assert_eq!(
            state.settings.ambient_sound_mode_cycle,
            Some(AmbientSoundModeCycle {
                noise_canceling: true,
                transparency: false,
                normal: true,
            })
        );

        let state = apply(
            &handle,
            SessionCommand::SetLimitHighVolume {
                enabled: true,
                db_limit: 80,
            },
        )
        .await;
        assert_eq!(
            state.settings.limit_high_volume,
            Some(LimitHighVolume {
                enabled: true,
                db_limit: 80,
                refresh_rate: VolumeLimitRefreshRate::RealTime,
            })
        );

        let state = apply(
            &handle,
            SessionCommand::SetLimitHighVolumeRefreshRate(VolumeLimitRefreshRate::OneMinute),
        )
        .await;
        assert_eq!(
            state.settings.limit_high_volume,
            Some(LimitHighVolume {
                enabled: true,
                db_limit: 80,
                refresh_rate: VolumeLimitRefreshRate::OneMinute,
            })
        );

        let state = apply(
            &handle,
            SessionCommand::SetAutoPowerOff(AutoPowerOffSetting::Enabled(2)),
        )
        .await;
        assert_eq!(
            state.auto_power_off,
            Some(AutoPowerOff {
                enabled: true,
                duration: 2,
            })
        );

        let state = apply(
            &handle,
            SessionCommand::SetAutoPowerOff(AutoPowerOffSetting::Disabled),
        )
        .await;
        assert_eq!(
            state.auto_power_off,
            Some(AutoPowerOff {
                enabled: false,
                duration: 2,
            })
        );

        let state = apply(&handle, SessionCommand::SetDualConnections(false)).await;
        assert_eq!(state.settings.dual_connections, Some(false));

        let state = apply(&handle, SessionCommand::SetLdac(true)).await;
        assert_eq!(state.settings.ldac, Some(true));

        let state = apply(&handle, SessionCommand::SetLowBatteryPrompt(false)).await;
        assert_eq!(state.settings.low_battery_prompt, Some(false));

        let state = apply(&handle, SessionCommand::SetDolbyAudio(true)).await;
        assert_eq!(state.settings.dolby_audio, Some(true));

        let state = apply(&handle, SessionCommand::SetSideTone(false)).await;
        assert_eq!(state.settings.side_tone, Some(false));

        let state = apply(&handle, SessionCommand::SetVoicePrompt(false)).await;
        assert_eq!(state.settings.voice_prompt, Some(false));

        handle.close().await.expect("release the link");
    }

    #[tokio::test(start_paused = true)]
    async fn rejects_unrepresentable_values_without_a_write() {
        let mut plan = ReplayPlan::new();
        // A rejected value is checked against a fresh state read before the
        // write, so only that read is replayed.
        plan.exchanges.push_back(plan.state());
        let handle = start(plan.exchanges);
        ready(&handle).await;

        let rejected = handle
            .dispatch(SessionCommand::SetLimitHighVolume {
                enabled: true,
                db_limit: 76,
            })
            .expect("queue the limiter write")
            .wait()
            .await;
        assert_eq!(
            rejected,
            Err(CommandError::Model(
                openscl4_models::ModelError::InvalidLimitHighVolumeWrite
            ))
        );
        handle.close().await.expect("release the link");
    }

    async fn apply(handle: &SessionHandle, command: SessionCommand) -> DeviceState {
        let CommandOutcome::Applied(state) = handle
            .dispatch(command)
            .expect("queue the operation")
            .wait()
            .await
            .expect("confirm the operation");
        state
    }
}
