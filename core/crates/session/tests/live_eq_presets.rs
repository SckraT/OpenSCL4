//! Live equalizer values and quick presets across the session boundary.

#![forbid(unsafe_code)]

#[cfg(test)]
mod tests {
    use std::{sync::Arc, time::Duration};

    use openscl4_session::{
        CommandError, CommandOutcome, ConnectionState, EqualizerSetting, QuickPresetValues,
        SessionCommand, SessionHandle, SessionRunner, SessionView, SoundMode,
    };
    use openscl4_transport::{ConnectTarget, RedactedMac, TransportProvider};
    use openscl4_transport_mock::VirtualA3062;
    use tokio::time::Instant;

    #[tokio::test(start_paused = true)]
    async fn live_values_coalesce_into_one_confirmed_curve() {
        let handle = start(VirtualA3062);
        handle.confirmed_view().await.expect("initial A3062 state");

        // Three rapid positions merge; the write starts only after the interval.
        handle
            .set_eq_live(curve(0xfefe, 60))
            .expect("queue first position");
        tokio::time::sleep(Duration::from_millis(20)).await;
        handle
            .set_eq_live(curve(0xfefe, 30))
            .expect("queue second position");
        tokio::time::sleep(Duration::from_millis(20)).await;
        handle
            .set_eq_live(curve(0xfefe, -60))
            .expect("queue third position");
        let written_at = Instant::now();

        let final_curve = curve(0xfefe, -60);
        wait_for_curve(&handle, final_curve).await;
        // The interval is anchored at the first value: 40 ms remain after the last.
        assert!(
            written_at.elapsed() >= Duration::from_millis(40),
            "the write waits out the interval after the last value"
        );
        let ConnectionState::Ready(state) = handle.subscribe().borrow().connection else {
            panic!("expected a confirmed state");
        };
        assert_eq!(state.equalizer, Some(final_curve));
    }

    #[tokio::test(start_paused = true)]
    async fn a_preset_with_equalizer_supersedes_the_pending_live_curve() {
        let handle = start(VirtualA3062);
        handle.confirmed_view().await.expect("initial A3062 state");
        handle
            .set_eq_live(curve(0xfefe, 60))
            .expect("queue a pending position");
        // Let the session accept the value before dispatching the preset.
        tokio::time::sleep(Duration::from_millis(10)).await;

        let preset_curve = curve(5, -30);
        let preset = handle
            .dispatch(SessionCommand::ApplyQuickPreset(QuickPresetValues {
                sound_mode: None,
                ambient_sound_mode: Some(SoundMode::Transparency),
                equalizer: Some(preset_curve),
            }))
            .expect("queue preset");
        let CommandOutcome::Applied(applied) = preset.wait().await.expect("preset confirmed");
        assert_eq!(applied.sound_mode, Some(SoundMode::Transparency));
        assert_eq!(applied.equalizer, Some(preset_curve));

        // The superseded live position never writes.
        tokio::time::sleep(Duration::from_millis(300)).await;
        let ConnectionState::Ready(state) = handle.subscribe().borrow().connection else {
            panic!("expected a confirmed state");
        };
        assert_eq!(state.equalizer, Some(preset_curve));
    }

    #[tokio::test(start_paused = true)]
    async fn a_preset_without_equalizer_keeps_the_pending_live_curve() {
        let handle = start(VirtualA3062);
        handle.confirmed_view().await.expect("initial A3062 state");
        let live = curve(0xfefe, -60);
        handle.set_eq_live(live).expect("queue a pending position");
        tokio::time::sleep(Duration::from_millis(10)).await;

        let preset = handle
            .dispatch(SessionCommand::ApplyQuickPreset(QuickPresetValues {
                sound_mode: None,
                ambient_sound_mode: Some(SoundMode::Transparency),
                equalizer: None,
            }))
            .expect("queue preset");
        let CommandOutcome::Applied(applied) = preset.wait().await.expect("preset confirmed");
        assert_eq!(applied.sound_mode, Some(SoundMode::Transparency));

        // The deferred position still applies after the preset completes.
        let view = wait_for_curve(&handle, live).await;
        assert!(
            matches!(
                view.connection,
                ConnectionState::Ready(state) if state.sound_mode == Some(SoundMode::Transparency)
            ),
            "the preset's confirmed sound mode stays applied"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_failed_preset_field_keeps_the_confirmed_fields() {
        let handle = start(VirtualA3062::without_equalizer_write());
        handle.confirmed_view().await.expect("initial A3062 state");
        let preset = handle
            .dispatch(SessionCommand::ApplyQuickPreset(QuickPresetValues {
                sound_mode: None,
                ambient_sound_mode: Some(SoundMode::Transparency),
                equalizer: Some(curve(0xfefe, -60)),
            }))
            .expect("queue preset");

        let result = preset.wait().await;
        let Err(CommandError::PresetEqualizerMismatch { confirmed, .. }) = result else {
            panic!("expected the preset to stop at its equalizer field");
        };
        // The sound-mode field confirmed first and stays applied.
        assert_eq!(confirmed.sound_mode, Some(SoundMode::Transparency));
        let ConnectionState::Ready(state) = handle.subscribe().borrow().connection else {
            panic!("expected a confirmed state");
        };
        assert_eq!(state.sound_mode, Some(SoundMode::Transparency));
        // The equalizer keeps the device's reported curve.
        assert_eq!(
            state.equalizer,
            Some(EqualizerSetting {
                preset_id: 0,
                volume_adjustments: [0, 0, 0, 0, 0, 0, 0, 0, 0, -120],
            })
        );
    }

    fn start(provider: impl TransportProvider) -> SessionHandle {
        SessionRunner::start(
            provider,
            ConnectTarget {
                mac: RedactedMac::from_octets([0; 6]),
            },
        )
    }

    fn curve(preset_id: u16, first_band: i16) -> EqualizerSetting {
        EqualizerSetting {
            preset_id,
            volume_adjustments: [first_band, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        }
    }

    async fn wait_for_curve(handle: &SessionHandle, setting: EqualizerSetting) -> Arc<SessionView> {
        let mut view = handle.subscribe();
        let confirmed = view
            .wait_for(|view| {
                matches!(
                    view.connection,
                    ConnectionState::Ready(state) if state.equalizer == Some(setting)
                )
            })
            .await
            .expect("confirmed curve");
        Arc::clone(&confirmed)
    }
}
