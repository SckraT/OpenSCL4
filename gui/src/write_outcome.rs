use openscq30_lib::device::{self, SettingWriteOutcome, WriteOutcome};

/// The legacy client only has one error path, so any setting the device did not confirm is an error.
pub fn ensure_applied(outcomes: Vec<SettingWriteOutcome>) -> Result<(), device::Error> {
    outcomes
        .into_iter()
        .find(|outcome| !matches!(outcome.outcome, WriteOutcome::Applied { .. }))
        .map_or(Ok(()), |outcome| {
            Err(device::Error::SettingWriteNotApplied {
                setting_id: outcome.setting_id,
            })
        })
}

#[cfg(test)]
mod tests {
    use openscq30_lib::settings::{SettingId, Value};

    use super::*;

    #[test]
    fn applied_outcomes_are_a_success() {
        let outcomes = vec![SettingWriteOutcome {
            setting_id: SettingId::WearingDetection,
            outcome: WriteOutcome::Applied {
                observed: Value::Bool(true),
            },
        }];

        assert!(ensure_applied(outcomes).is_ok());
    }

    #[test]
    fn a_rejected_outcome_is_an_error() {
        let outcomes = vec![SettingWriteOutcome {
            setting_id: SettingId::WearingDetection,
            outcome: WriteOutcome::Rejected { observed: None },
        }];

        assert!(matches!(
            ensure_applied(outcomes),
            Err(device::Error::SettingWriteNotApplied {
                setting_id: SettingId::WearingDetection
            })
        ));
    }
}
