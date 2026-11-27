#![forbid(unsafe_code)]

#[cfg(test)]
mod tests {
    use std::process::Command;

    use serde_json::{Value, json};

    #[test]
    fn pilot_reads_the_virtual_space_a40_state() {
        assert_eq!(
            pilot("A3936", &[]),
            json!({
                "model": "A3936",
                "firmware": {"left": "04.19", "right": "04.19"},
                "battery": {"leftPercent": 100, "rightPercent": 60, "casePercent": 80},
                "soundMode": "Normal",
                "gamingMode": false,
                "twsStatus": {"connected": true, "host": "Right"},
                "ambientCycle": {"shared": {
                    "noiseCanceling": true,
                    "transparency": true,
                    "normal": true,
                }},
            })
        );
    }

    #[test]
    fn pilot_reads_the_virtual_liberty_4_state() {
        assert_eq!(
            pilot("A3952", &[]),
            json!({
                "model": "A3952",
                "firmware": {"left": "04.96", "right": "04.96"},
                "battery": {"leftPercent": 80, "rightPercent": 80, "casePercent": 80},
                "soundMode": "Normal",
                "twsStatus": {"connected": true, "host": "Right"},
                "ambientCycle": {"shared": {
                    "noiseCanceling": true,
                    "transparency": true,
                    "normal": false,
                }},
            })
        );
    }

    #[test]
    fn pilot_reads_the_virtual_sport_x20_state() {
        assert_eq!(
            pilot("A3968", &[]),
            json!({
                "model": "A3968",
                "firmware": {"left": "01.65", "right": "01.65"},
                "battery": {"leftPercent": 100, "rightPercent": 100, "casePercent": 40},
                "soundMode": "NoiseCanceling",
                "twsStatus": {"connected": true, "host": "Left"},
                "ambientCycle": {"tws": {
                    "enabled": {
                        "noiseCanceling": true,
                        "transparency": true,
                        "normal": true,
                    },
                    "disconnected": {
                        "noiseCanceling": true,
                        "transparency": true,
                        "normal": false,
                    },
                }},
            })
        );
    }

    #[test]
    fn pilot_cycles_the_virtual_space_a40_gaming_mode_and_restores_it() {
        let snapshot = pilot("A3936", &["--cycle-mode"]);
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
    fn pilot_cycles_the_virtual_liberty_4_sound_mode_and_restores_it() {
        let snapshot = pilot("A3952", &["--cycle-mode"]);
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
    fn pilot_cycles_the_virtual_sport_x20_sound_mode_and_restores_it() {
        let snapshot = pilot("A3968", &["--cycle-mode"]);
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
            .expect("run the dual-battery pilot");

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
