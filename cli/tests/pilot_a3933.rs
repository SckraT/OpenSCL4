#![forbid(unsafe_code)]

#[cfg(test)]
mod tests {
    use std::process::Command;

    use serde_json::{Value, json};

    #[test]
    fn pilot_reads_the_virtual_a3933_reference_state() {
        assert_eq!(
            pilot("A3933", &[]),
            json!({
                "model": "A3933",
                "firmware": {"left": "00.00", "right": "00.00"},
                "battery": {"leftPercent": 0, "rightPercent": 0, "casePercent": 0},
                "soundMode": "NoiseCanceling",
                "gamingMode": false,
            })
        );
    }

    #[test]
    fn pilot_reads_the_virtual_a3939_reference_state() {
        assert_eq!(
            pilot("A3939", &[]),
            json!({
                "model": "A3939",
                "firmware": {"left": "00.00", "right": "00.00"},
                "battery": {"leftPercent": 0, "rightPercent": 0, "casePercent": 0},
                "soundMode": "NoiseCanceling",
                "gamingMode": false,
            })
        );
    }

    #[test]
    fn pilot_cycles_the_a3933_gaming_mode_and_restores_it() {
        let snapshot = pilot("A3933", &["--cycle-mode"]);
        assert_eq!(snapshot["gamingMode"], false);
        assert_eq!(
            snapshot["modeCycle"],
            json!({
                "changed": {"result": "Applied", "gamingMode": true},
                "restored": {"result": "Applied", "gamingMode": false},
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
            .expect("run the family pilot");

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
