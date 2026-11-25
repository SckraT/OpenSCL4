#![forbid(unsafe_code)]

#[cfg(test)]
mod tests {
    use std::process::Command;

    use serde_json::{Value, json};

    #[test]
    fn pilot_reads_virtual_a3062_from_the_single_battery_state() {
        let directory = tempfile::tempdir().expect("isolated configuration");
        let database = directory.path().join("pilot.sqlite");
        let output = Command::new(env!("CARGO_BIN_EXE_openscq30"))
            .args(["pilot", "--mock-model", "A3062"])
            .env("OPENSCQ30_DATABASE_PATH", &database)
            .output()
            .expect("run the A3062 pilot");

        assert!(
            output.status.success(),
            "pilot failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let snapshot: Value = serde_json::from_slice(&output.stdout).expect("pilot JSON");
        assert_eq!(
            snapshot,
            json!({
                "model": "A3062",
                "firmware": {"device": "04.39"},
                "battery": {"devicePercent": 30},
                "soundMode": "Normal",
            })
        );
        assert!(!database.exists());
    }

    #[test]
    fn pilot_confirms_the_a3062_mode_change_and_restores_the_original() {
        let output = Command::new(env!("CARGO_BIN_EXE_openscq30"))
            .args(["pilot", "--mock-model", "A3062", "--cycle-mode"])
            .output()
            .expect("run the A3062 mode cycle");

        assert!(
            output.status.success(),
            "pilot cycle failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let snapshot: Value = serde_json::from_slice(&output.stdout).expect("pilot JSON");
        assert_eq!(snapshot["soundMode"], "Normal");
        assert_eq!(
            snapshot["modeCycle"],
            json!({
                "changed": {"result": "Applied", "soundMode": "NoiseCanceling"},
                "restored": {"result": "Applied", "soundMode": "Normal"},
            })
        );
    }
}
