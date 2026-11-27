#![forbid(unsafe_code)]

#[cfg(test)]
mod tests {
    use std::process::Command;

    use serde_json::{Value, json};

    #[test]
    fn pilot_reads_the_virtual_a3004_reference_state() {
        assert_eq!(
            pilot("A3004", &[]),
            json!({
                "model": "A3004",
                "firmware": {"device": "00.26"},
                "battery": {"devicePercent": 80},
                "soundMode": "NoiseCanceling",
            })
        );
    }

    #[test]
    fn pilot_reads_the_virtual_a3005_reference_state() {
        assert_eq!(
            pilot("A3005", &[]),
            json!({
                "model": "A3005",
                "firmware": {"device": "01.21"},
                "battery": {"devicePercent": 80},
                "dualConnections": true,
            })
        );
    }

    #[test]
    fn pilot_reads_the_virtual_a3033_reference_state() {
        assert_eq!(
            pilot("A3033", &[]),
            json!({
                "model": "A3033",
                "firmware": {"device": "01.21"},
                "battery": {"devicePercent": 80},
                "wearingDetection": true,
            })
        );
    }

    #[test]
    fn pilot_cycles_the_a3004_reference_mode_and_restores_it() {
        let snapshot = pilot("A3004", &["--cycle-mode"]);
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
