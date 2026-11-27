#![forbid(unsafe_code)]

#[cfg(test)]
mod tests {
    use std::process::Command;

    use serde_json::{Value, json};

    #[test]
    fn pilot_reads_the_virtual_a3028_reference_state() {
        assert_eq!(
            pilot("A3028", &[]),
            json!({
                "model": "A3028",
                "firmware": {"device": "02.30"},
                "battery": {"devicePercent": 100},
                "soundMode": "Normal",
            })
        );
    }

    #[test]
    fn pilot_reads_the_virtual_a3029_reference_state() {
        assert_eq!(
            pilot("A3029", &[]),
            json!({
                "model": "A3029",
                "firmware": {"device": "02.30"},
                "battery": {"devicePercent": 100},
                "soundMode": "Normal",
            })
        );
    }

    #[test]
    fn pilot_cycles_the_a3028_reference_modes_and_restores_them() {
        let snapshot = pilot("A3028", &["--cycle-mode"]);
        assert_eq!(snapshot["soundMode"], "Normal");
        assert_eq!(
            snapshot["modeCycle"],
            json!({
                "changed": {"result": "Applied", "soundMode": "NoiseCanceling"},
                "restored": {"result": "Applied", "soundMode": "Normal"},
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
