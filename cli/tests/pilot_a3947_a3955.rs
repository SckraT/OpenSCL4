#![forbid(unsafe_code)]

#[cfg(test)]
mod tests {
    use std::process::Command;

    use serde_json::{Value, json};

    #[test]
    fn pilot_reads_the_virtual_liberty_state() {
        assert_eq!(
            pilot("A3947", &[]),
            json!({
                "model": "A3947",
                "firmware": {"left": "05.88", "right": "05.88"},
                "battery": {
                    "leftPercent": 40,
                    "rightPercent": 80,
                    "casePercent": 40,
                },
                "soundMode": "NoiseCanceling",
                "gamingMode": false,
            })
        );
    }

    #[test]
    fn pilot_reads_the_virtual_p40i_state() {
        assert_eq!(
            pilot("A3955", &[]),
            json!({
                "model": "A3955",
                "firmware": {"left": "01.61", "right": "01.61"},
                "battery": {
                    "leftPercent": 100,
                    "rightPercent": 100,
                    "casePercent": 60,
                },
                "soundMode": "NoiseCanceling",
            })
        );
    }

    #[test]
    fn pilot_cycles_the_liberty_gaming_mode_and_restores_it() {
        let snapshot = pilot("A3947", &["--cycle-mode"]);
        assert_eq!(snapshot["gamingMode"], false);
        assert_eq!(
            snapshot["modeCycle"],
            json!({
                "changed": {"result": "Applied", "gamingMode": true},
                "restored": {"result": "Applied", "gamingMode": false},
            })
        );
    }

    #[test]
    fn pilot_cycles_the_p40i_sound_mode_and_restores_it() {
        let snapshot = pilot("A3955", &["--cycle-mode"]);
        assert_eq!(snapshot["soundMode"], "NoiseCanceling");
        assert_eq!(
            snapshot["modeCycle"],
            json!({
                "changed": {"result": "Applied", "soundMode": "Normal"},
                "restored": {"result": "Applied", "soundMode": "NoiseCanceling"},
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
            .expect("run the model pilot");

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
