#![forbid(unsafe_code)]

#[cfg(test)]
mod tests {
    use std::process::Command;

    use serde_json::{Value, json};

    #[test]
    fn pilot_reads_the_virtual_p31i_state() {
        assert_eq!(
            pilot("D1202", &[]),
            json!({
                "model": "D1202",
                "firmware": {"left": "01.83", "right": "01.83"},
                "battery": {"leftPercent": 100, "rightPercent": 100, "casePercent": 40},
                "soundMode": "NoiseCanceling",
                "spatialAudio": {"enabled": false, "mode": "Music"},
                "autoPowerOff": {"enabled": true, "durationIndex": 2},
            })
        );
    }

    #[test]
    fn pilot_reads_the_virtual_r60i_nc_state() {
        assert_eq!(
            pilot("D1202C", &[]),
            json!({
                "model": "D1202C",
                "firmware": {"left": "04.89", "right": "04.89"},
                "battery": {"leftPercent": 100, "rightPercent": 100, "casePercent": 100},
                "soundMode": "NoiseCanceling",
                "spatialAudio": {"enabled": false, "mode": "Music"},
                "autoPowerOff": {"enabled": true, "durationIndex": 3},
            })
        );
    }

    #[test]
    fn pilot_cycles_the_p31i_ambient_mode_and_restores_it() {
        let snapshot = pilot("D1202", &["--cycle-mode"]);
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
    fn pilot_cycles_the_r60i_nc_ambient_mode_and_restores_it() {
        let snapshot = pilot("D1202C", &["--cycle-mode"]);
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
