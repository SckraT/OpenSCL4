#![forbid(unsafe_code)]

#[cfg(test)]
mod tests {
    use std::process::Command;

    use serde_json::{Value, json};

    #[test]
    fn pilot_reads_the_virtual_sleep_a30_state() {
        assert_eq!(
            pilot("D1301", &[]),
            json!({
                "model": "D1301",
                "firmware": {"left": "01.91", "right": "01.91"},
                "battery": {"leftPercent": 100, "rightPercent": 100},
                "listeningMode": "Bluetooth",
                "defaultListeningMode": "Bluetooth",
                "noiseCanceling": false,
                "sleepAudio": {"enabled": true, "action": "Pause"},
            })
        );
    }

    #[test]
    fn pilot_cycles_the_sleep_audio_action_and_restores_it() {
        let snapshot = pilot("D1301", &["--cycle-mode"]);
        assert_eq!(
            snapshot["sleepAudio"],
            json!({"enabled": true, "action": "Pause"})
        );
        assert_eq!(
            snapshot["modeCycle"],
            json!({
                "changed": {
                    "result": "Applied",
                    "sleepAudio": {"enabled": true, "action": "PlayLocal"},
                },
                "restored": {
                    "result": "Applied",
                    "sleepAudio": {"enabled": true, "action": "Pause"},
                },
            })
        );
    }

    fn pilot(model: &str, args: &[&str]) -> Value {
        let directory = tempfile::tempdir().expect("isolated configuration");
        let database = directory.path().join("pilot.sqlite");
        let output = Command::new(env!("CARGO_BIN_EXE_openscq30"))
            .args(["pilot", "--mock-model", model])
            .args(args)
            .env("OPENSCQ30_DATABASE_PATH", &database)
            .output()
            .expect("run the sleep pilot");

        assert!(
            output.status.success(),
            "pilot failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let snapshot: Value = serde_json::from_slice(&output.stdout).expect("pilot JSON");
        assert!(!database.exists());
        snapshot
    }
}
