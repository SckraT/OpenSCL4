#![forbid(unsafe_code)]

#[cfg(test)]
mod tests {
    use std::process::Command;

    use serde_json::{Value, json};

    #[test]
    fn pilot_reads_the_virtual_a3116_state() {
        let snapshot = pilot(&[]);
        assert_eq!(
            snapshot,
            json!({
                "model": "A3116",
                "firmware": {"device": "38.39"},
                "battery": {"devicePercent": 100, "charging": false},
                "volume": 7,
                "equalizer": {
                    "presetId": 255,
                    "volumeAdjustments": [0, 10, 20, 30, 40, 0, -10, -20, -30, 0],
                },
                "autoPowerOff": {"enabled": true, "duration": 1},
                "voicePrompt": true,
            })
        );
    }

    fn pilot(args: &[&str]) -> Value {
        let directory = tempfile::tempdir().expect("isolated configuration");
        let database = directory.path().join("pilot.sqlite");
        let output = Command::new(env!("CARGO_BIN_EXE_openscq30"))
            .args(["pilot", "--mock-model", "A3116"])
            .args(args)
            .env("OPENSCQ30_DATABASE_PATH", &database)
            .output()
            .expect("run the A3116 pilot");

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
