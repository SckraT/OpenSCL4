use serde::{Deserialize, Serialize};

use crate::settings::Equalizer;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferProfile {
    pub name: String,
    pub volume_adjustments: Vec<f64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProfileConstraints {
    pub band_count: usize,
    pub fraction_digits: i16,
    pub min: i16,
    pub max: i16,
}

impl From<&Equalizer> for ProfileConstraints {
    fn from(equalizer: &Equalizer) -> Self {
        Self {
            band_count: equalizer.band_hz.len(),
            fraction_digits: equalizer.fraction_digits,
            min: equalizer.min,
            max: equalizer.max,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ProfileValidationError {
    #[error("profile \"{name}\" has {actual} bands, but this device's equalizer has {expected}")]
    WrongBandCount {
        name: String,
        actual: usize,
        expected: usize,
    },
    #[error(
        "profile \"{name}\" has {value} for band {band}, but values must be between {min} and {max}"
    )]
    OutOfRange {
        name: String,
        band: usize,
        value: f64,
        min: f64,
        max: f64,
    },
}

pub fn parse_profiles(text: &str) -> serde_json::Result<Vec<TransferProfile>> {
    serde_json::from_str(text)
}

pub fn profiles_json(profiles: &[TransferProfile]) -> String {
    serde_json::to_string(profiles).expect("profiles are plain data")
}

fn scaled_to_device(value: f64, fraction_digits: i16) -> f64 {
    (value * 10f64.powi(i32::from(fraction_digits))).round()
}

pub fn validate_profile(
    profile: &TransferProfile,
    constraints: &ProfileConstraints,
) -> Result<Vec<i16>, ProfileValidationError> {
    if profile.volume_adjustments.len() != constraints.band_count {
        return Err(ProfileValidationError::WrongBandCount {
            name: profile.name.clone(),
            actual: profile.volume_adjustments.len(),
            expected: constraints.band_count,
        });
    }
    let scale = 10f64.powi(i32::from(constraints.fraction_digits));
    let allowed_range = f64::from(constraints.min)..=f64::from(constraints.max);
    profile
        .volume_adjustments
        .iter()
        .enumerate()
        .map(|(index, &value)| {
            let scaled = scaled_to_device(value, constraints.fraction_digits);
            if !allowed_range.contains(&scaled) {
                return Err(ProfileValidationError::OutOfRange {
                    name: profile.name.clone(),
                    band: index + 1,
                    value,
                    min: f64::from(constraints.min) / scale,
                    max: f64::from(constraints.max) / scale,
                });
            }
            Ok(scaled as i16)
        })
        .collect()
}

pub fn device_values(profile: &TransferProfile, fraction_digits: i16) -> Vec<i16> {
    profile
        .volume_adjustments
        .iter()
        .map(|&value| scaled_to_device(value, fraction_digits) as i16)
        .collect()
}

pub fn decibels(values: &[i16], fraction_digits: i16) -> Vec<f64> {
    values
        .iter()
        .map(|&value| f64::from(value) / 10f64.powi(i32::from(fraction_digits)))
        .collect()
}
