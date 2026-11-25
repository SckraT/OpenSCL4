#![forbid(unsafe_code)]

#[cfg(test)]
mod tests {
    use std::process::Command;

    use serde_json::{Value, json};

    #[test]
    fn pilot_reads_virtual_a3949_from_its_state_frame() {
        let directory = tempfile::tempdir().expect("isolated configuration");
        let database = directory.path().join("pilot.sqlite");
        let output = Command::new(env!("CARGO_BIN_EXE_openscq30"))
            .args(["pilot", "--mock-model", "A3949"])
            .env("OPENSCQ30_DATABASE_PATH", &database)
            .output()
            .expect("run the A3949 pilot");

        assert!(
            output.status.success(),
            "pilot failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let snapshot: Value = serde_json::from_slice(&output.stdout).expect("pilot JSON");
        assert_eq!(
            snapshot,
            json!({
                "model": "A3949",
                "firmware": {"left": "14.43", "right": "14.43"},
                "battery": {"leftPercent": 100, "rightPercent": 80},
            })
        );
        assert!(!database.exists());
    }
}
