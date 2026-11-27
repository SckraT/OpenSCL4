#![forbid(unsafe_code)]

#[cfg(test)]
mod tests {
    use std::process::Command;

    use serde_json::{Value, json};

    #[test]
    fn pilot_reads_the_virtual_a3035_reference_state() {
        assert_eq!(
            pilot("A3035", &[]),
            json!({
                "model": "A3035",
                "firmware": {"device": "06.87"},
                "battery": {"devicePercent": 100},
                "soundMode": "NoiseCanceling",
            })
        );
    }

    #[test]
    fn pilot_reads_the_virtual_a3040_reference_state() {
        assert_eq!(
            pilot("A3040", &[]),
            json!({
                "model": "A3040",
                "firmware": {"device": "02.34"},
                "battery": {"devicePercent": 60},
                "soundMode": "NoiseCanceling",
            })
        );
    }

    #[test]
    fn pilot_cycles_the_a3035_reference_modes_and_restores_them() {
        let snapshot = pilot("A3035", &["--cycle-mode"]);
        assert_eq!(snapshot["soundMode"], "NoiseCanceling");
        assert_eq!(
            snapshot["modeCycle"],
            json!({
                "changed": {"result": "Applied", "soundMode": "Normal"},
                "restored": {"result": "Applied", "soundMode": "NoiseCanceling"},
            })
        );
    }

    #[test]
    fn pilot_cycles_the_a3040_reference_modes_and_restores_them() {
        let snapshot = pilot("A3040", &["--cycle-mode"]);
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
            .expect("run the single-peer pilot");

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
