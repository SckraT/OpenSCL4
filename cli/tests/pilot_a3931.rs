#![forbid(unsafe_code)]

#[cfg(test)]
mod tests {
    use std::process::Command;

    use serde_json::{Value, json};

    #[test]
    fn pilot_reads_the_virtual_a3931_reference_state() {
        assert_eq!(
            pilot("A3931", &[]),
            json!({
                "model": "A3931",
                "firmware": {"left": null, "right": null},
                "battery": {"leftPercent": 0, "rightPercent": 0},
                "soundMode": "NoiseCanceling",
            })
        );
    }

    #[test]
    fn pilot_reads_the_virtual_a3935_reference_state() {
        assert_eq!(
            pilot("A3935", &[]),
            json!({
                "model": "A3935",
                "firmware": {"left": null, "right": null},
                "battery": {"leftPercent": 0, "rightPercent": 0},
                "soundMode": "NoiseCanceling",
            })
        );
    }

    #[test]
    fn pilot_cycles_the_a3931_ambient_mode_and_restores_it() {
        let snapshot = pilot("A3931", &["--cycle-mode"]);
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
