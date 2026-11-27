#![forbid(unsafe_code)]

#[cfg(test)]
mod tests {
    use std::process::Command;

    use serde_json::{Value, json};

    #[test]
    fn pilot_reads_the_virtual_a3953_state() {
        assert_eq!(
            pilot("A3953", &[]),
            json!({
                "model": "A3953",
                "firmware": {"left": "03.23", "right": "03.23"},
                "battery": {"leftPercent": 100, "rightPercent": 100, "casePercent": 100},
                "soundMode": "Normal",
                "spatialAudio": {"enabled": false, "mode": "Fixed", "contentMode": "Music"},
                "pressSensitivity": 0,
            })
        );
    }

    #[test]
    fn pilot_cycles_the_a3953_ambient_mode_and_restores_it() {
        let snapshot = pilot("A3953", &["--cycle-mode"]);
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
            .expect("run the Liberty 4 pilot");

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
