#![forbid(unsafe_code)]

#[cfg(test)]
mod tests {
    use std::process::Command;

    use serde_json::{Value, json};

    #[test]
    fn pilot_reads_the_virtual_a3944_reference_state() {
        assert_eq!(
            pilot("A3944", &[]),
            json!({
                "model": "A3944",
                "firmware": {"left": "02.27", "right": "02.27"},
                "battery": {"leftPercent": 100, "rightPercent": 100},
            })
        );
    }

    #[test]
    fn pilot_reads_the_virtual_a3945_reference_state() {
        assert_eq!(
            pilot("A3945", &[]),
            json!({
                "model": "A3945",
                "firmware": {"left": "00.00", "right": "00.00"},
                "battery": {"leftPercent": 0, "rightPercent": 0, "casePercent": 0},
                "gamingMode": false,
            })
        );
    }

    #[test]
    fn pilot_reads_the_virtual_a3948_reference_state() {
        assert_eq!(
            pilot("A3948", &[]),
            json!({
                "model": "A3948",
                "firmware": {"left": "24.56", "right": "24.56"},
                "battery": {"leftPercent": 100, "rightPercent": 100},
            })
        );
    }

    #[test]
    fn pilot_reads_the_virtual_a3959_reference_state() {
        assert_eq!(
            pilot("A3959", &[]),
            json!({
                "model": "A3959",
                "firmware": {"left": "01.64", "right": "01.64"},
                "battery": {"leftPercent": 50, "rightPercent": 60},
                "soundMode": "NoiseCanceling",
                "gamingMode": false,
            })
        );
    }

    #[test]
    fn pilot_cycles_the_a3945_gaming_mode_and_restores_it() {
        let snapshot = pilot("A3945", &["--cycle-mode"]);
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
    fn pilot_cycles_the_a3959_gaming_mode_and_restores_it() {
        let snapshot = pilot("A3959", &["--cycle-mode"]);
        assert_eq!(snapshot["soundMode"], "NoiseCanceling");
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
