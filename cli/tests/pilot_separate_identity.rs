#![forbid(unsafe_code)]

#[cfg(test)]
mod tests {
    use std::process::Command;

    use serde_json::{Value, json};

    #[test]
    fn pilot_reads_the_captured_a3909_state() {
        assert_eq!(
            pilot("A3909", &[]),
            json!({
                "model": "A3909",
                "firmware": {"left": "01.28", "right": "01.28"},
                "battery": {"leftPercent": null, "rightPercent": 100},
            })
        );
    }

    #[test]
    fn pilot_reads_the_derived_a3926_state() {
        assert_eq!(
            pilot("A3926", &[]),
            json!({
                "model": "A3926",
                "firmware": {"left": "01.60", "right": "01.60"},
                "battery": {"leftPercent": 60, "rightPercent": 80},
            })
        );
    }

    #[test]
    fn pilot_reads_the_derived_a3930_state_and_ambient_mode() {
        assert_eq!(
            pilot("A3930", &[]),
            json!({
                "model": "A3930",
                "firmware": {"left": "01.70", "right": "01.70"},
                "battery": {"leftPercent": 80, "rightPercent": 100},
                "soundMode": "Normal",
                "ldac": false,
            })
        );
    }

    #[test]
    fn pilot_reads_the_derived_a3951_state_and_ldac_flag() {
        assert_eq!(
            pilot("A3951", &[]),
            json!({
                "model": "A3951",
                "firmware": {"left": "01.80", "right": "01.80"},
                "battery": {"leftPercent": 100, "rightPercent": 40},
                "soundMode": "Normal",
                "ldac": true,
            })
        );
    }

    #[test]
    fn pilot_cycles_the_a3930_normal_and_transparency_modes() {
        let snapshot = pilot("A3930", &["--cycle-mode"]);
        assert_eq!(snapshot["soundMode"], "Normal");
        assert_eq!(
            snapshot["modeCycle"],
            json!({
                "changed": {"result": "Applied", "soundMode": "Transparency"},
                "restored": {"result": "Applied", "soundMode": "Normal"},
            })
        );
    }

    #[test]
    fn pilot_cycles_the_a3951_normal_and_noise_canceling_modes() {
        let snapshot = pilot("A3951", &["--cycle-mode"]);
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
    fn pilot_rejects_a_mode_cycle_without_a_sound_mode() {
        let output = run_pilot("A3909", &["--cycle-mode"]);
        assert!(!output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("read-only"),
            "stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn pilot(model: &str, args: &[&str]) -> Value {
        let output = run_pilot(model, args);
        assert!(
            output.status.success(),
            "pilot failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).expect("pilot JSON")
    }

    fn run_pilot(model: &str, args: &[&str]) -> std::process::Output {
        let directory = tempfile::tempdir().expect("isolated configuration");
        let database = directory.path().join("pilot.sqlite");
        let output = Command::new(env!("CARGO_BIN_EXE_openscq30"))
            .args(["pilot", "--mock-model", model])
            .args(args)
            .env("OPENSCQ30_DATABASE_PATH", &database)
            .output()
            .expect("run the family pilot");
        assert!(!database.exists(), "the pilot must not create a database");
        output
    }
}
