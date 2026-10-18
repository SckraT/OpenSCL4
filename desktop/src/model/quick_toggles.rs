//! The on/off settings shown as chips on the device card, as quickToggleIds in
//! android/app/src/main/java/com/oppzippy/openscq30/ui/sound/DeviceHeroCard.kt.

/// The ids of the quick toggles to show: the user's choice, or LDAC when the device has it and nothing was chosen.
pub fn quick_toggle_ids(stored: Option<&[String]>, available: &[&str]) -> Vec<String> {
    let chosen: Vec<&str> = match stored {
        None => vec!["ldac"],
        Some(ids) => ids.iter().map(|id| id.trim()).collect(),
    };
    let mut ids: Vec<String> = Vec::new();
    for id in chosen {
        if !id.is_empty() && available.contains(&id) && !ids.iter().any(|other| other == id) {
            ids.push(id.to_string());
        }
    }
    ids
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stored(ids: &[&str]) -> Vec<String> {
        ids.iter().map(|id| (*id).to_string()).collect()
    }

    #[test]
    fn ldac_is_the_default_quick_toggle() {
        assert_eq!(
            vec!["ldac"],
            quick_toggle_ids(None, &["wearingDetection", "ldac"])
        );
        assert!(quick_toggle_ids(None, &["wearingDetection"]).is_empty());
    }

    #[test]
    fn chosen_quick_toggles_keep_their_order_and_drop_unavailable_ones() {
        let chosen = stored(&["wearingDetection", "removed", "ldac", "ldac"]);
        assert_eq!(
            vec!["wearingDetection", "ldac"],
            quick_toggle_ids(Some(&chosen), &["ldac", "wearingDetection"])
        );
        assert!(quick_toggle_ids(Some(&[]), &["ldac"]).is_empty());
    }
}
