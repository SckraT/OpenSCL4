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

    /// The state the virtual Liberty 4 Pro serves from
    /// `core/data/a3954-pilot-state.bin`.
    fn snapshot_state() -> Value {
        json!({
            "model": "A3954",
            "firmware": {"left": "04.29", "right": "04.29", "case": "01.58"},
            "battery": {"leftPercent": 96, "rightPercent": 98, "casePercent": 100},
            "soundMode": "NoiseCanceling",
            "twsConnected": true,
            "equalizer": {
                "presetId": 65278,
                "volumeAdjustments": [18, 8, 0, 0, 0, 0, 0, 0, 0, 0],
            },
            "autoPowerOff": {"enabled": true, "duration": 2},
            "caseFeatures": {
                "atmospheric": false,
                "remoteCamera": false,
                "findDevice": true,
                "caseSpatialAudio": false,
            },
            "spatialAudio": {"enabled": false, "mode": "Music", "musicMode": "Fixed"},
            "soundLeakCompensation": false,
            "caseLanguage": "English",
        })
    }

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
        assert_eq!(snapshot, snapshot_state());
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

    #[test]
    fn pilot_applies_and_restores_the_parity_settings() {
        let output = Command::new(env!("CARGO_BIN_EXE_openscq30"))
            .args(["pilot", "--cycle-settings"])
            .output()
            .expect("run the CLI settings cycle");

        assert!(
            output.status.success(),
            "pilot settings cycle failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let snapshot: Value = serde_json::from_slice(&output.stdout).expect("pilot snapshot JSON");
        assert_eq!(
            snapshot["settingsCycle"],
            json!([
                {"setting": "caseFeatures", "changed": "Applied", "restored": "Applied"},
                {"setting": "spatialAudio", "changed": "Applied", "restored": "Applied"},
                {"setting": "soundLeakCompensation", "changed": "Applied", "restored": "Applied"},
                {"setting": "autoPowerOff", "changed": "Applied", "restored": "Applied"},
                {"setting": "caseLanguage", "changed": "Applied", "restored": "Applied"},
            ])
        );
        // The restored snapshot keeps every measured value.
        for (key, value) in snapshot_state().as_object().expect("object").iter() {
            assert_eq!(&snapshot[key], value, "restored {key}");
        }
    }

    #[test]
    fn pilot_reports_an_unknown_identity_as_read_only_without_payloads() {
        let output = run(&["pilot", "--mock-scenario", "unknown-identity"]);
        assert!(
            output.status.success(),
            "read-only pilot failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let snapshot: Value = serde_json::from_slice(&output.stdout).expect("pilot snapshot JSON");
        assert_eq!(
            snapshot,
            json!({"model": null, "readOnly": true, "reason": "unknownIdentity"})
        );
        let stdout = String::from_utf8_lossy(&output.stdout).to_ascii_lowercase();
        assert!(!stdout.contains("unknown00000"));
        assert!(!stdout.contains("9999"));
    }

    #[test]
    fn pilot_reports_incompatible_firmware_as_read_only() {
        let output = run(&["pilot", "--mock-scenario", "incompatible-state"]);
        assert!(
            output.status.success(),
            "read-only pilot failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let snapshot: Value = serde_json::from_slice(&output.stdout).expect("pilot snapshot JSON");
        assert_eq!(
            snapshot,
            json!({
                "model": "A3954",
                "readOnly": true,
                "reason": "incompatibleState",
            })
        );
    }

    #[test]
    fn pilot_cycle_mode_is_rejected_before_writing_for_a_read_only_device() {
        let output = run(&[
            "pilot",
            "--mock-scenario",
            "unknown-identity",
            "--cycle-mode",
        ]);
        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("read-only"),
            "expected a read-only rejection, got: {stderr}"
        );
        assert!(output.stdout.is_empty(), "a rejected cycle printed output");
    }

    #[test]
    fn pilot_settings_cycle_is_rejected_for_a_model_without_the_settings() {
        let output = run(&["pilot", "--mock-model", "A3949", "--cycle-settings"]);
        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("A3954"),
            "expected an A3954-only rejection, got: {stderr}"
        );
        assert!(output.stdout.is_empty(), "a rejected cycle printed output");
    }

    fn run(args: &[&str]) -> std::process::Output {
        let directory = tempfile::tempdir().expect("isolated configuration");
        let database = directory.path().join("pilot.sqlite");
        Command::new(env!("CARGO_BIN_EXE_openscq30"))
            .args(args)
            .env("OPENSCQ30_DATABASE_PATH", &database)
            .output()
            .expect("run the CLI pilot")
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
