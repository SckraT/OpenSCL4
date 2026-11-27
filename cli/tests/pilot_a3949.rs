#![forbid(unsafe_code)]

#[cfg(test)]
mod tests {
    use std::process::Command;

    use serde_json::{Value, json};

    #[test]
    fn pilot_reads_virtual_a3949_from_its_state_frame() {
        assert_eq!(
            pilot(&[]),
            json!({
                "model": "A3949",
                "firmware": {"left": "14.43", "right": "14.43"},
                "battery": {"leftPercent": 100, "rightPercent": 80},
                "gamingMode": false,
            })
        );
    }

    #[test]
    fn pilot_confirms_the_a3949_gaming_mode_change_and_restores_it() {
        let snapshot = pilot(&["--cycle-mode"]);
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
    fn pilot_confirms_and_restores_the_a3949_settings_cycle() {
        let snapshot = pilot(&["--cycle-settings"]);
        assert_eq!(snapshot["gamingMode"], false);
        let cycle = &snapshot["settingsCycle"];
        assert_eq!(cycle["equalizer"]["changed"]["result"], "Applied");
        assert_eq!(cycle["equalizer"]["changed"]["presetId"], 14);
        assert_eq!(cycle["equalizer"]["restored"]["result"], "Applied");
        assert_eq!(cycle["equalizer"]["restored"]["presetId"], 0);
        assert_eq!(cycle["touchTone"]["changed"]["touchTone"], true);
        assert_eq!(cycle["touchTone"]["restored"]["touchTone"], false);
        assert_eq!(cycle["buttonAction"]["changed"]["action"], 2);
        assert_eq!(cycle["buttonAction"]["restored"]["action"], Value::Null);
        assert_eq!(cycle["resetButtons"]["result"], "Accepted");
    }

    fn pilot(args: &[&str]) -> Value {
        let directory = tempfile::tempdir().expect("isolated configuration");
        let database = directory.path().join("pilot.sqlite");
        let output = Command::new(env!("CARGO_BIN_EXE_openscq30"))
            .args(["pilot", "--mock-model", "A3949"])
            .args(args)
            .env("OPENSCQ30_DATABASE_PATH", &database)
            .output()
            .expect("run the A3949 pilot");

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
