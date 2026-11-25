#![forbid(unsafe_code)]

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use openscl4_session::{
        CommandOutcome, ConnectionState, SessionCommand, SessionHandle, SessionRunner,
        SoundModeSetting,
    };
    use openscl4_transport::{ConnectTarget, RedactedMac, TransportProvider};
    use openscl4_transport_mock::VirtualSoundcoreDevice;
    use tokio::time::Instant;

    #[tokio::test(start_paused = true)]
    async fn completes_queued_writes_in_order_with_separate_confirmations() {
        let handle = start(VirtualSoundcoreDevice);
        let original = mode(&handle).await;
        let change = handle
            .dispatch(SessionCommand::SetSoundMode(SoundModeSetting::NORMAL))
            .expect("queue change");
        let restore = handle
            .dispatch(SessionCommand::SetSoundMode(original))
            .expect("queue restore");
        assert_ne!(change.id(), restore.id());

        let CommandOutcome::Applied(changed) = change.wait().await.expect("confirm change");
        assert_eq!(changed.sound_mode_setting(), SoundModeSetting::NORMAL);
        let CommandOutcome::Applied(restored) = restore.wait().await.expect("confirm restore");
        assert_eq!(restored.sound_mode_setting(), original);
        assert_eq!(mode(&handle).await, original);
        handle.close().await.expect("close virtual link");
    }

    #[tokio::test(start_paused = true)]
    async fn confirms_applied_mode_by_read_back_when_ack_is_missing() {
        let handle = start(VirtualSoundcoreDevice::without_sound_mode_ack());
        let original = mode(&handle).await;
        assert_ne!(original, SoundModeSetting::NORMAL);
        let dispatched_at = Instant::now();
        let change = handle
            .dispatch(SessionCommand::SetSoundMode(SoundModeSetting::NORMAL))
            .expect("queue change");

        let CommandOutcome::Applied(changed) = change.wait().await.expect("confirm without ACK");
        assert!(dispatched_at.elapsed() >= Duration::from_millis(1500));
        assert_eq!(changed.sound_mode_setting(), SoundModeSetting::NORMAL);
        assert_eq!(mode(&handle).await, SoundModeSetting::NORMAL);
        handle.close().await.expect("close virtual link");
    }

    #[tokio::test(start_paused = true)]
    async fn an_accepted_write_completes_without_a_state_subscriber() {
        let handle = start(VirtualSoundcoreDevice::without_sound_mode_ack());
        mode(&handle).await;
        let change = handle
            .dispatch(SessionCommand::SetSoundMode(SoundModeSetting::NORMAL))
            .expect("queue change");
        drop(handle);

        let CommandOutcome::Applied(changed) =
            change.wait().await.expect("confirm without subscriber");
        assert_eq!(changed.sound_mode_setting(), SoundModeSetting::NORMAL);
    }

    fn start(provider: impl TransportProvider) -> SessionHandle {
        SessionRunner::start(
            provider,
            ConnectTarget {
                mac: RedactedMac::from_octets([0; 6]),
            },
        )
    }

    async fn mode(handle: &SessionHandle) -> SoundModeSetting {
        let view = handle.confirmed_view().await.expect("state snapshot");
        let ConnectionState::Ready(state) = view.connection else {
            panic!("expected a confirmed snapshot");
        };
        state.sound_mode_setting()
    }
}
