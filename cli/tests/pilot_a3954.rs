#![forbid(unsafe_code)]

#[cfg(test)]
#[path = "pilot_a3954/disconnect.rs"]
mod disconnect;

#[cfg(test)]
mod tests {
    use std::{process::Command, time::Duration};

    use openscl4_session::{ConnectionState, SessionEvent, SessionRunner};
    use openscl4_transport::{ConnectTarget, RedactedMac};
    use openscl4_transport_mock::VirtualSoundcoreDevice;
    use tokio::time::timeout;

    use serde_json::{Value, json};

    #[test]
    fn pilot_reads_virtual_a3954() {
        let directory = tempfile::tempdir().expect("create isolated configuration directory");
        let database = directory.path().join("pilot.sqlite");
        let output = Command::new(env!("CARGO_BIN_EXE_openscq30"))
            .arg("pilot")
            .env("OPENSCQ30_DATABASE_PATH", &database)
            .output()
            .expect("run the CLI pilot");

        assert!(
            output.status.success(),
            "pilot failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let snapshot: Value = serde_json::from_slice(&output.stdout).expect("pilot snapshot JSON");
        assert_eq!(
            snapshot,
            json!({
                "model": "A3954",
                "firmware": {"left": "04.29", "right": "04.29"},
                "battery": {"leftPercent": 96, "rightPercent": 98, "casePercent": 100},
                "soundMode": "NoiseCanceling",
            })
        );
        assert!(
            !database.exists(),
            "pilot should leave the legacy database untouched"
        );
    }

    #[test]
    fn pilot_applies_and_restores_the_sound_mode() {
        let output = Command::new(env!("CARGO_BIN_EXE_openscq30"))
            .args(["pilot", "--cycle-mode"])
            .output()
            .expect("run the CLI mode cycle");

        assert!(
            output.status.success(),
            "pilot mode cycle failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let snapshot: Value = serde_json::from_slice(&output.stdout).expect("pilot snapshot JSON");
        assert_eq!(snapshot["soundMode"], "NoiseCanceling");
        assert_eq!(
            snapshot["modeCycle"],
            json!({
                "changed": {"result": "Applied", "soundMode": "Normal", "position": 6},
                "restored": {"result": "Applied", "soundMode": "NoiseCanceling", "position": 11},
            })
        );
    }

    #[tokio::test]
    async fn pilot_closes_the_session_and_notifies_other_handles() {
        let target = ConnectTarget {
            mac: RedactedMac::from_octets([0; 6]),
        };
        let handle = SessionRunner::start(VirtualSoundcoreDevice, target);
        let observer = handle.clone();
        let mut events = observer.events();
        let confirmed = observer
            .confirmed_view()
            .await
            .expect("read the confirmed state");

        handle.close().await.expect("release the virtual link");

        let terminal = observer.subscribe().borrow().clone();
        assert_eq!(terminal.connection, ConnectionState::Closed);
        assert!(terminal.revision > confirmed.revision);
        let closed_event = timeout(Duration::from_secs(5), async {
            loop {
                match events.recv().await.expect("receive a session event") {
                    SessionEvent::ViewChanged(view)
                        if view.connection == ConnectionState::Closed =>
                    {
                        break view;
                    }
                    _ => {}
                }
            }
        })
        .await
        .expect("receive the close notification");
        assert_eq!(closed_event, terminal);
    }
}
