//! Live equalizer values and quick presets across the session boundary.

#![forbid(unsafe_code)]

#[cfg(test)]
mod tests {
    use std::{sync::Arc, time::Duration};

    use async_trait::async_trait;
    use bytes::Bytes;
    use openscl4_models::{EQUALIZER_COMMAND, ModelError};
    use openscl4_session::{
        CommandError, CommandOutcome, ConnectionState, DispatchError, EqualizerSetting, IoStage,
        PresetFieldMismatch, QuickPresetValues, ReconnectPolicy, SessionCommand, SessionError,
        SessionHandle, SessionRunner, SessionView, SoundMode,
    };
    use openscl4_transport::{
        ConnectTarget, LinkRx, LinkTx, RedactedMac, TransportError, TransportLink,
        TransportProvider,
    };
    use openscl4_transport_mock::{VirtualA3062, VirtualA3949, VirtualSoundcoreDevice};
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
        let Err(CommandError::PresetMismatch {
            field: PresetFieldMismatch::Equalizer { .. },
            confirmed,
        }) = result
        else {
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

    #[tokio::test(start_paused = true)]
    async fn a_slider_value_submitted_after_a_preset_applies_last() {
        let handle = start(VirtualA3062);
        handle.confirmed_view().await.expect("initial A3062 state");
        let preset_curve = curve(5, -30);
        let live_curve = curve(0xfefe, 60);
        let preset = handle
            .dispatch(SessionCommand::ApplyQuickPreset(QuickPresetValues {
                ambient_sound_mode: Some(SoundMode::Transparency),
                equalizer: Some(preset_curve),
                ..QuickPresetValues::default()
            }))
            .expect("queue preset");
        handle
            .clone()
            .set_eq_live(live_curve)
            .expect("publish the next live position");

        let CommandOutcome::Applied(applied) = preset.wait().await.expect("preset confirmed");
        assert_eq!(applied.sound_mode, Some(SoundMode::Transparency));
        assert_eq!(applied.equalizer, Some(preset_curve));
        tokio::time::sleep(Duration::from_millis(300)).await;
        let ConnectionState::Ready(state) = handle.subscribe().borrow().connection else {
            panic!("expected a confirmed state");
        };
        assert_eq!(state.sound_mode, Some(SoundMode::Transparency));
        assert_eq!(state.equalizer, Some(live_curve));
    }

    #[tokio::test(start_paused = true)]
    async fn an_unread_slider_value_submitted_before_a_preset_is_superseded() {
        let handle = start(VirtualA3062);
        handle.confirmed_view().await.expect("initial A3062 state");
        let preset_curve = curve(5, -30);
        handle
            .set_eq_live(curve(0xfefe, 60))
            .expect("publish the earlier live position");
        let preset = handle
            .clone()
            .dispatch(SessionCommand::ApplyQuickPreset(QuickPresetValues {
                equalizer: Some(preset_curve),
                ..QuickPresetValues::default()
            }))
            .expect("queue preset");

        preset.wait().await.expect("preset confirmed");
        tokio::time::sleep(Duration::from_millis(300)).await;
        let ConnectionState::Ready(state) = handle.subscribe().borrow().connection else {
            panic!("expected a confirmed state");
        };
        assert_eq!(state.equalizer, Some(preset_curve));
    }

    #[tokio::test(start_paused = true)]
    async fn an_ambient_only_preset_preserves_the_already_confirmed_curve() {
        let handle = start(VirtualA3062);
        handle.confirmed_view().await.expect("initial A3062 state");
        let live_curve = curve(0xfefe, 60);
        handle.set_eq_live(live_curve).expect("publish an EQ curve");
        wait_for_curve(&handle, live_curve).await;

        let preset = handle
            .dispatch(SessionCommand::ApplyQuickPreset(QuickPresetValues {
                ambient_sound_mode: Some(SoundMode::Transparency),
                ..QuickPresetValues::default()
            }))
            .expect("queue ambient preset");
        let CommandOutcome::Applied(applied) = preset.wait().await.expect("preset confirmed");
        assert_eq!(applied.sound_mode, Some(SoundMode::Transparency));
        assert_eq!(applied.equalizer, Some(live_curve));
    }

    #[tokio::test(start_paused = true)]
    async fn an_invalid_eq_curve_does_not_change_any_preset_field() {
        let handle = start(VirtualA3062);
        let initial = handle.confirmed_view().await.expect("initial A3062 state");

        for gain in [-121, 135] {
            let invalid = curve(0xfefe, gain);
            let preset = handle
                .dispatch(SessionCommand::ApplyQuickPreset(QuickPresetValues {
                    ambient_sound_mode: Some(SoundMode::Transparency),
                    equalizer: Some(invalid),
                    ..QuickPresetValues::default()
                }))
                .expect("queue preset");
            assert_eq!(
                preset.wait().await,
                Err(CommandError::Model(ModelError::InvalidEqualizerWrite))
            );
            assert_eq!(handle.subscribe().borrow().connection, initial.connection);
            assert_eq!(
                handle.set_eq_live(invalid),
                Err(DispatchError::InvalidEqualizer)
            );
        }

        let pending = curve(0xfefe, 60);
        handle
            .set_eq_live(pending)
            .expect("publish a valid pending curve");
        let invalid = handle
            .dispatch(SessionCommand::ApplyQuickPreset(QuickPresetValues {
                equalizer: Some(curve(0xfefe, 135)),
                ..QuickPresetValues::default()
            }))
            .expect("queue preset");
        assert_eq!(
            invalid.wait().await,
            Err(CommandError::Model(ModelError::InvalidEqualizerWrite))
        );
        wait_for_curve(&handle, pending).await;
    }

    #[tokio::test(start_paused = true)]
    async fn live_eq_reports_unsupported_models_on_submission() {
        for handle in [start(VirtualA3949), start(VirtualSoundcoreDevice)] {
            handle.confirmed_view().await.expect("initial state");
            assert_eq!(
                handle.set_eq_live(curve(0xfefe, 60)),
                Err(DispatchError::UnsupportedEqualizer)
            );
            handle.close().await.expect("release the link");
        }
    }

    #[tokio::test(start_paused = true)]
    async fn a_disconnect_preserves_a_confirmed_preset_field() {
        for policy in [ReconnectPolicy::External, ReconnectPolicy::Internal] {
            let handle = SessionRunner::start_with_policy(DisconnectOnEq, target(), policy);
            handle.confirmed_view().await.expect("initial A3062 state");
            let preset = handle
                .dispatch(SessionCommand::ApplyQuickPreset(QuickPresetValues {
                    ambient_sound_mode: Some(SoundMode::Transparency),
                    equalizer: Some(curve(0xfefe, 60)),
                    ..QuickPresetValues::default()
                }))
                .expect("queue preset");

            let queued = handle
                .dispatch(SessionCommand::SetAmbientSoundMode(SoundMode::Normal))
                .expect("queue another command");
            let result = preset.wait().await;
            let Err(CommandError::PresetInterrupted {
                error,
                confirmed,
                may_have_written,
            }) = result
            else {
                panic!("expected the partial preset outcome under {policy:?}, got {result:?}");
            };
            assert_eq!(
                error,
                SessionError::Transport {
                    stage: IoStage::Send,
                    error: TransportError::Disconnected,
                }
            );
            assert!(may_have_written);
            assert_eq!(confirmed.sound_mode, Some(SoundMode::Transparency));
            assert_eq!(
                confirmed.equalizer,
                Some(EqualizerSetting {
                    preset_id: 0,
                    volume_adjustments: [0, 0, 0, 0, 0, 0, 0, 0, 0, -120],
                })
            );
            assert_eq!(queued.wait().await, Err(CommandError::Disconnected));
            let view = handle.subscribe();
            let view = view.borrow();
            assert_eq!(view.last_confirmed, Some(confirmed));
            assert_eq!(
                view.connection,
                match policy {
                    ReconnectPolicy::External => ConnectionState::Failed(error),
                    ReconnectPolicy::Internal => ConnectionState::Reconnecting {
                        last_confirmed: confirmed,
                        reason: error,
                    },
                }
            );
        }
    }

    struct DisconnectOnEq;
    struct DisconnectingLink(Box<dyn TransportLink>);
    struct DisconnectingTx(Box<dyn LinkTx>);

    #[async_trait]
    impl TransportProvider for DisconnectOnEq {
        async fn connect(
            &self,
            target: &ConnectTarget,
        ) -> Result<Box<dyn TransportLink>, TransportError> {
            Ok(Box::new(DisconnectingLink(
                VirtualA3062.connect(target).await?,
            )))
        }
    }

    impl TransportLink for DisconnectingLink {
        fn split(self: Box<Self>) -> (Box<dyn LinkTx>, Box<dyn LinkRx>) {
            let (tx, rx) = self.0.split();
            (Box::new(DisconnectingTx(tx)), rx)
        }
    }

    #[async_trait]
    impl LinkTx for DisconnectingTx {
        async fn send(&mut self, request: Bytes) -> Result<(), TransportError> {
            if request.get(5..7) == Some(&EQUALIZER_COMMAND) {
                return Err(TransportError::Disconnected);
            }
            self.0.send(request).await
        }

        async fn close(&mut self) {
            self.0.close().await;
        }
    }

    fn start(provider: impl TransportProvider) -> SessionHandle {
        SessionRunner::start(provider, target())
    }

    fn target() -> ConnectTarget {
        ConnectTarget {
            mac: RedactedMac::from_octets([0; 6]),
            service: None,
        }
    }

    fn curve(preset_id: u16, first_band: i16) -> EqualizerSetting {
        EqualizerSetting {
            preset_id,
            volume_adjustments: [first_band, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        }
    }

    async fn wait_for_curve(handle: &SessionHandle, setting: EqualizerSetting) -> Arc<SessionView> {
        let mut view = handle.subscribe();
        let confirmed = tokio::time::timeout(
            Duration::from_secs(5),
            view.wait_for(|view| {
                matches!(
                    view.connection,
                    ConnectionState::Ready(state) if state.equalizer == Some(setting)
                )
            }),
        )
        .await
        .expect("read-back deadline")
        .expect("confirmed curve");
        Arc::clone(&confirmed)
    }
}
