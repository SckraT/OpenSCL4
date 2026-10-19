//! Equalizer values and the profile file format, as android/app/src/main/java/com/oppzippy/openscq30/ui/sound/
//! EqualizerProfileTransfer.kt and SoundEqualizerPage.kt handle them.

use crate::equalizer_profiles::{self, ProfileConstraints, ProfileValidationError};
use crate::settings::Equalizer;

pub use crate::equalizer_profiles::{TransferProfile, decibels, device_values, profiles_json};

pub fn parse_profiles(text: &str) -> Option<Vec<TransferProfile>> {
    equalizer_profiles::parse_profiles(text.trim()).ok()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImportStatus {
    New,
    Replaces,
    WrongBandCount,
    OutOfRange,
}

impl ImportStatus {
    pub fn is_valid(self) -> bool {
        matches!(self, Self::New | Self::Replaces)
    }
}

/// What importing would do with each profile, checked the way the library checks it: the device's number of bands,
/// and each value within its range once scaled to the device's precision. Without the device's equalizer (while only
/// one earbud is connected) only the names are compared, and the library checks the rest.
pub fn preview_import(
    profiles: &[TransferProfile],
    existing_names: &[&str],
    equalizer: Option<&Equalizer>,
) -> Vec<ImportStatus> {
    let constraints = equalizer.map(ProfileConstraints::from);
    profiles
        .iter()
        .map(|profile| {
            match constraints
                .as_ref()
                .map(|limits| equalizer_profiles::validate_profile(profile, limits))
            {
                Some(Err(ProfileValidationError::WrongBandCount { .. })) => {
                    ImportStatus::WrongBandCount
                }
                Some(Err(ProfileValidationError::OutOfRange { .. })) => ImportStatus::OutOfRange,
                _ if existing_names.contains(&profile.name.as_str()) => ImportStatus::Replaces,
                _ => ImportStatus::New,
            }
        })
        .collect()
}

/// A value the user typed, in decibels with a point or a comma, in device units. Rejects overflow, values out of the
/// device's range and precision the device can not represent.
pub fn parse_equalizer_value(text: &str, equalizer: &Equalizer) -> Option<i16> {
    let text = text.trim().replace(',', ".");
    let (negative, digits) = match text
        .strip_prefix('-')
        .or_else(|| text.strip_prefix('\u{2212}'))
    {
        Some(rest) => (true, rest.to_string()),
        None => (false, text.strip_prefix('+').unwrap_or(&text).to_string()),
    };
    let (whole, fraction) = digits.split_once('.').unwrap_or((&digits, ""));
    if whole.is_empty() && fraction.is_empty()
        || !whole
            .chars()
            .chain(fraction.chars())
            .all(|c| c.is_ascii_digit())
    {
        return None;
    }
    let places = usize::try_from(equalizer.fraction_digits).ok()?;
    // digits past the device's precision may only be zeros
    let (kept, dropped) = fraction.split_at(fraction.len().min(places));
    if dropped.chars().any(|c| c != '0') {
        return None;
    }
    let mut units: i64 = 0;
    for c in whole
        .chars()
        .chain(kept.chars())
        .chain(std::iter::repeat_n('0', places - kept.len()))
    {
        units = units
            .checked_mul(10)?
            .checked_add(i64::from(c.to_digit(10)?))?;
    }
    let units = if negative { -units } else { units };
    (i64::from(equalizer.min)..=i64::from(equalizer.max))
        .contains(&units)
        .then_some(units as i16)
}

/// A device value in decibels with a point and no trailing zeros: 134 with one fraction digit is "13.4".
pub fn format_equalizer_value(value: i16, fraction_digits: i16) -> String {
    let places = usize::try_from(fraction_digits).unwrap_or(0);
    let magnitude = i32::from(value).unsigned_abs();
    let text = format!("{magnitude:0width$}", width = places + 1);
    let (whole, fraction) = text.split_at(text.len() - places);
    let fraction = fraction.trim_end_matches('0');
    let sign = if value < 0 { "-" } else { "" };
    if fraction.is_empty() {
        format!("{sign}{whole}")
    } else {
        format!("{sign}{whole}.{fraction}")
    }
}

/// A band's frequency without its unit, and whether it is in kilohertz: 1600 Hz is ("1.6", true).
pub fn frequency(hz: u16) -> (String, bool) {
    if hz < 1000 {
        (hz.to_string(), false)
    } else {
        let khz = format!("{}.{:03}", hz / 1000, hz % 1000);
        (
            khz.trim_end_matches('0').trim_end_matches('.').to_string(),
            true,
        )
    }
}

/// Old profiles still to move over: one that has a custom profile of the same name was moved already.
pub fn unmigrated<'a, T>(legacy: &'a [(String, T)], custom_names: &[&str]) -> Vec<&'a (String, T)> {
    legacy
        .iter()
        .filter(|(name, _)| !custom_names.contains(&name.as_str()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn equalizer() -> Equalizer {
        Equalizer {
            band_hz: vec![100, 200, 400, 800, 1600, 3200, 6400, 12800].into(),
            fraction_digits: 1,
            min: -120,
            max: 134,
        }
    }

    fn profile(name: &str, values: &[f64]) -> TransferProfile {
        TransferProfile {
            name: name.to_string(),
            volume_adjustments: values.to_vec(),
        }
    }

    const FLAT: [f64; 8] = [0.0; 8];

    #[test]
    fn reads_the_library_export_format() {
        let json = r#"[{"name": "test profile", "volumeAdjustments": [0.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7]}]"#;
        assert_eq!(
            Some(vec![profile(
                "test profile",
                &[0.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7]
            )]),
            parse_profiles(json)
        );
    }

    #[test]
    fn written_profiles_read_back_the_same() {
        let profiles = vec![
            profile("a", &FLAT),
            profile("b", &[-12.0, 13.4, 0.5, 0.0, 0.0, 0.0, 0.0, 0.0]),
        ];
        assert_eq!(
            Some(profiles.clone()),
            parse_profiles(&profiles_json(&profiles))
        );
    }

    #[test]
    fn text_that_is_not_a_profiles_file_is_rejected() {
        assert_eq!(None, parse_profiles("hello"));
        assert_eq!(None, parse_profiles(r#"{"name": "not a list"}"#));
        assert_eq!(None, parse_profiles(r#"[{"name": "no values"}]"#));
    }

    #[test]
    fn preview_says_what_importing_would_do() {
        let preview = preview_import(
            &[
                profile("new", &FLAT),
                profile("saved", &FLAT),
                profile("short", &[1.0, 2.0]),
                profile("loud", &[20.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]),
            ],
            &["saved"],
            Some(&equalizer()),
        );
        assert_eq!(
            vec![
                ImportStatus::New,
                ImportStatus::Replaces,
                ImportStatus::WrongBandCount,
                ImportStatus::OutOfRange
            ],
            preview
        );
    }

    #[test]
    fn the_range_is_checked_at_the_device_precision_like_the_library() {
        let status = |value| {
            preview_import(
                &[profile("p", &[value, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0])],
                &[],
                Some(&equalizer()),
            )[0]
        };
        assert_eq!(ImportStatus::New, status(13.4));
        assert_eq!(ImportStatus::New, status(-12.04));
        // -12.05 rounds away from zero to -121, past the minimum of -120
        assert_eq!(ImportStatus::OutOfRange, status(-12.05));
        assert_eq!(ImportStatus::OutOfRange, status(f64::NAN));
    }

    #[test]
    fn without_the_equalizer_only_names_are_compared() {
        let preview = preview_import(
            &[profile("short", &[1.0]), profile("saved", &[1.0])],
            &["saved"],
            None,
        );
        assert_eq!(vec![ImportStatus::New, ImportStatus::Replaces], preview);
    }

    #[test]
    fn values_are_scaled_to_the_device() {
        assert_eq!(
            vec![-120, 134, 5, 0],
            device_values(&profile("p", &[-12.0, 13.4, 0.5, 0.0]), 1)
        );
        assert_eq!(vec![-12.0, 13.4, 0.5], decibels(&[-120, 134, 5], 1));
    }

    #[test]
    fn old_profiles_with_a_saved_profile_of_the_same_name_were_moved_already() {
        let legacy = vec![("Old".to_string(), 1), ("Moved".to_string(), 2)];
        assert_eq!(vec![&legacy[0]], unmigrated(&legacy, &["Moved"]));
    }

    #[test]
    fn accepts_device_limits_and_a_decimal_comma() {
        let eq = Equalizer {
            band_hz: vec![100].into(),
            fraction_digits: 1,
            min: -120,
            max: 134,
        };
        assert_eq!(Some(-120), parse_equalizer_value("-12", &eq));
        assert_eq!(Some(134), parse_equalizer_value("13.4", &eq));
        assert_eq!(Some(25), parse_equalizer_value(" 2,5 ", &eq));
        assert_eq!(Some(0), parse_equalizer_value("0.0", &eq));
        assert_eq!(Some(-15), parse_equalizer_value("\u{2212}1,5", &eq));
    }

    #[test]
    fn rejects_values_that_would_overflow_or_require_rounding() {
        let eq = Equalizer {
            band_hz: vec![100].into(),
            fraction_digits: 1,
            min: -120,
            max: 134,
        };
        for text in [
            "13.5",
            "-12.1",
            "6553.6",
            "1.01",
            "",
            "-",
            "NaN",
            "1.2.3",
            "99999999999999999999",
        ] {
            assert_eq!(None, parse_equalizer_value(text, &eq), "{text}");
        }
    }

    #[test]
    fn formats_stored_values_without_changing_device_precision() {
        assert_eq!("13.4", format_equalizer_value(134, 1));
        assert_eq!("-12", format_equalizer_value(-120, 1));
        assert_eq!("0.25", format_equalizer_value(25, 2));
        assert_eq!("0", format_equalizer_value(0, 1));
        assert_eq!("-0.5", format_equalizer_value(-5, 1));
        assert_eq!("6", format_equalizer_value(6, 0));
    }

    #[test]
    fn frequencies_switch_to_kilohertz_at_one_thousand() {
        assert_eq!(("800".to_string(), false), frequency(800));
        assert_eq!(("1.6".to_string(), true), frequency(1600));
        assert_eq!(("1".to_string(), true), frequency(1000));
        assert_eq!(("12.8".to_string(), true), frequency(12800));
    }
}
