#![forbid(unsafe_code)]

#[cfg(test)]
mod tests {
    use std::process::Command;

    use serde_json::{Value, json};

    #[test]
    fn pilot_reads_the_virtual_a3027_reference_state() {
        assert_eq!(
            pilot("A3027", &[]),
            json!({
                "model": "A3027",
                "firmware": {"device": "01.20"},
                "battery": {"devicePercent": 100},
                "soundMode": "Normal",
                "wearingDetection": true,
            })
        );
    }

    #[test]
    fn pilot_reads_the_virtual_a3030_reference_state() {
        assert_eq!(
            pilot("A3030", &[]),
            json!({
                "model": "A3030",
                "firmware": {"device": "01.20"},
                "battery": {"devicePercent": 100},
                "soundMode": "Normal",
                "wearingDetection": true,
            })
        );
    }

    #[test]
    fn pilot_reads_the_virtual_a3031_identity_and_state() {
        assert_eq!(
            pilot("A3031", &[]),
            json!({
                "model": "A3031",
                "firmware": {"left": "01.10", "right": "01.20"},
                "battery": {"leftPercent": 100, "rightPercent": 80},
                "soundMode": "Normal",
                "twsConnected": true,
            })
        );
    }

    #[test]
    fn pilot_cycles_the_a3027_modes_and_restores_them() {
        let snapshot = pilot("A3027", &["--cycle-mode"]);
        assert_eq!(snapshot["soundMode"], "Normal");
        assert_eq!(
            snapshot["modeCycle"],
            json!({
                "changed": {"result": "Applied", "soundMode": "NoiseCanceling"},
                "restored": {"result": "Applied", "soundMode": "Normal"},
            })
        );
    }

    #[test]
    fn pilot_cycles_the_a3031_modes_and_restores_them() {
        let snapshot = pilot("A3031", &["--cycle-mode"]);
        assert_eq!(snapshot["soundMode"], "Normal");
        assert_eq!(
            snapshot["firmware"],
            json!({"left": "01.10", "right": "01.20"})
        );
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
