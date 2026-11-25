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
    fn pilot_reports_a3062_as_read_only_for_the_mode_cycle() {
        let output = Command::new(env!("CARGO_BIN_EXE_openscq30"))
            .args(["pilot", "--mock-model", "A3062", "--cycle-mode"])
            .output()
            .expect("run the A3062 pilot");

        assert!(!output.status.success());
        assert_eq!(
            String::from_utf8_lossy(&output.stderr).trim(),
            "Error: device model is read-only in the pilot"
        );
        assert!(output.stdout.is_empty());
    }
}
