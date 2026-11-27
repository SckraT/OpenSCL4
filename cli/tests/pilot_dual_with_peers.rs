#![forbid(unsafe_code)]

#[cfg(test)]
mod tests {
    use std::process::Command;

    use serde_json::{Value, json};

    #[test]
    fn pilot_reads_the_virtual_c30i_reference_state() {
        assert_eq!(
            pilot("A3330", &[]),
            json!({
                "model": "A3330",
                "firmware": {"left": "04.27", "right": "04.27"},
                "battery": {"leftPercent": 100, "rightPercent": 100, "casePercent": 40},
                "tws": {"connected": true, "hostDevice": "Right"},
                "buttonsEnabled": true,
                "touchTone": true,
                "lowBatteryPrompt": true,
                "dualConnections": false,
            })
        );
    }

    #[test]
    fn pilot_reads_the_virtual_aeroclip_reference_state() {
        assert_eq!(
            pilot("A3388", &[]),
            json!({
                "model": "A3388",
                "firmware": {"left": "01.49", "right": "01.49"},
                "battery": {"leftPercent": 90, "rightPercent": 90, "casePercent": 80},
                "tws": {"connected": true, "hostDevice": "Left"},
                "buttonsEnabled": true,
                "touchTone": true,
                "lowBatteryPrompt": true,
                "dualConnections": true,
            })
        );
    }

    #[test]
    fn pilot_reads_the_virtual_v20i_reference_state() {
        assert_eq!(
            pilot("A3876", &[]),
            json!({
                "model": "A3876",
                "firmware": {"left": "03.15", "right": "03.15"},
                "battery": {"leftPercent": 60, "rightPercent": 100},
                "tws": {"connected": true, "hostDevice": "Right"},
                "gamingMode": false,
                "autoPowerOff": {"enabled": true, "durationIndex": 2},
                "colorfulLights": {
                    "enabled": true,
                    "brightness": 4,
                    "autoLightsOffMinutes": 45,
                    "mode": "Breathing",
                    "color": {"red": 23, "green": 187, "blue": 239},
                },
                "volumeBalance": 0,
                "voicePrompt": true,
                "dualConnections": true,
            })
        );
    }

    #[test]
    fn pilot_reads_the_virtual_c50i_reference_state() {
        assert_eq!(
            pilot("D1101", &[]),
            json!({
                "model": "D1101",
                "firmware": {"left": "01.28", "right": "01.28"},
                "battery": {"leftPercent": 100, "rightPercent": 10},
                "tws": {"connected": true, "hostDevice": "Left"},
                "buttonsEnabled": true,
                "ldac": false,
                "lowBatteryPrompt": true,
                "dualConnections": true,
                "autoPowerOff": {"enabled": true, "durationIndex": 0},
            })
        );
    }

    #[test]
    fn pilot_cycles_the_v20i_gaming_mode_and_restores_it() {
        let snapshot = pilot("A3876", &["--cycle-mode"]);
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
            .expect("run the paired pilot");

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
