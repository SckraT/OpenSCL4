//! Equalizer write codec shared by the A3004 and A3005.
//!
//! Both models write a ten-band curve followed by a separate ten-band DRC block.
//! The block repeats the legacy `VolumeAdjustments::apply_drc` transform: the
//! first eight bands are recomputed from the same eight bands and the last two
//! keep the fixed defaults. The transform is a port of the legacy algorithm, so
//! the derived bytes match the legacy `set_equalizer_with_drc` reference frame.

use crate::{EqualizerSetting, ModelError};

/// Number of equalizer bands carried in the models' curves.
pub const BAND_COUNT: usize = crate::EQUALIZER_BANDS;

/// Rewrites the first eight bands through the legacy DRC transform.
///
/// The legacy code converts the tenths-of-a-decibel gains to decibels, applies
/// the fixed coefficient matrix and rounds the result back to tenths. The last
/// two bands are not part of the matrix and keep the family defaults
/// (`0 dB` and the lowest gain).
pub fn drc_gains(gains: &[i16; BAND_COUNT]) -> [i16; BAND_COUNT] {
    const SMALLER_COEFFICIENT: f64 = 0.85;
    const LARGER_COEFFICIENT: f64 = 0.95;

    let adjustments = gains.map(|gain| f64::from(gain) / 10.0);
    let lows_subtraction = adjustments[2] * 0.81 * SMALLER_COEFFICIENT;
    let highs_subtraction = adjustments[5] * 0.81 * SMALLER_COEFFICIENT;

    // The matrix from the legacy `apply_drc`. Coefficients that the legacy code
    // pre-multiplies are written as such so the arithmetic order stays identical.
    const BAND_COEFFICIENTS: [[f64; 8]; 8] = [
        [
            1.26,
            -0.71 * SMALLER_COEFFICIENT,
            0.177,
            -0.0494,
            0.0345,
            -0.0197,
            0.0075,
            -0.00217,
        ],
        [
            -0.71 * SMALLER_COEFFICIENT,
            1.73 * LARGER_COEFFICIENT,
            0.0,
            0.204,
            -0.068,
            0.045,
            -0.0235,
            0.0075,
        ],
        [
            0.177,
            -0.81 * SMALLER_COEFFICIENT,
            1.73 * LARGER_COEFFICIENT,
            -0.81 * SMALLER_COEFFICIENT,
            0.208,
            -0.07,
            0.045,
            -0.0197,
        ],
        [
            -0.0494,
            0.204,
            0.0,
            1.73 * LARGER_COEFFICIENT,
            -0.82 * SMALLER_COEFFICIENT,
            0.208,
            -0.068,
            0.0345,
        ],
        [
            0.0345,
            -0.068,
            0.208,
            -0.82 * SMALLER_COEFFICIENT,
            1.73 * LARGER_COEFFICIENT,
            0.0,
            0.204,
            -0.0494,
        ],
        [
            -0.0197,
            0.045,
            -0.07,
            0.208,
            -0.81 * SMALLER_COEFFICIENT,
            1.73 * LARGER_COEFFICIENT,
            -0.81 * SMALLER_COEFFICIENT,
            0.177,
        ],
        [
            0.0075,
            -0.0235,
            0.045,
            -0.068,
            0.204,
            0.0,
            1.83 * LARGER_COEFFICIENT,
            -0.71 * SMALLER_COEFFICIENT,
        ],
        [
            -0.00217,
            0.0075,
            -0.0197,
            0.0345,
            -0.0494,
            0.177,
            -0.71 * SMALLER_COEFFICIENT,
            1.5,
        ],
    ];
    const BAND_DEFAULTS: [i16; BAND_COUNT] = [0, 0, 0, 0, 0, 0, 0, 0, 0, -120];

    let transformed: [f64; 8] = std::array::from_fn(|band| {
        BAND_COEFFICIENTS[band]
            .iter()
            .enumerate()
            .fold(0.0, |sum, (index, coefficient)| {
                if (band == 1 || band == 3) && index == 2 {
                    sum - lows_subtraction
                } else if (band == 4 || band == 6) && index == 5 {
                    sum - highs_subtraction
                } else {
                    sum + adjustments[index] * coefficient
                }
            })
    });

    std::array::from_fn(|band| {
        let gain = transformed.get(band).map_or_else(
            || f64::from(BAND_DEFAULTS[band]),
            // The legacy code divides by ten and multiplies by ten again before
            // rounding; keep that order so the rounding matches exactly.
            |value| (value / 10.0) * 10.0,
        );
        (gain.round() as i16).clamp(crate::EQUALIZER_MIN_GAIN, crate::EQUALIZER_MAX_GAIN)
    })
}

/// Builds the ten-band curve followed by its DRC block.
pub fn write_body(setting: EqualizerSetting) -> Result<Vec<u8>, ModelError> {
    if !setting.is_writable() {
        return Err(ModelError::InvalidEqualizerWrite);
    }
    let bands = setting.volume_adjustments;
    let drc = drc_gains(&bands);
    let mut body = Vec::with_capacity(2 + 2 * BAND_COUNT);
    body.extend_from_slice(&setting.preset_id.to_le_bytes());
    body.extend(bands.iter().map(|gain| EqualizerSetting::band_byte(*gain)));
    body.extend(drc.iter().map(|gain| EqualizerSetting::band_byte(*gain)));
    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The fixed example curve from the legacy `set_equalizer_with_drc` test.
    const LEGACY_CURVE: [i16; BAND_COUNT] = [-60, 60, 23, 120, 22, -120, -4, 16, 0, -120];

    #[test]
    fn matches_the_legacy_drc_reference_bytes() {
        // The legacy test frame carries the first eight transformed bands as
        // 6D 86 70 88 7B 66 7E 79; the last two are the documented defaults.
        let drc = drc_gains(&LEGACY_CURVE);
        assert_eq!(
            drc.map(EqualizerSetting::band_byte),
            [0x6d, 0x86, 0x70, 0x88, 0x7b, 0x66, 0x7e, 0x79, 0x78, 0x00]
        );
        assert_eq!(drc[8..], [0, -120]);
    }

    #[test]
    fn writes_the_curve_followed_by_the_drc_block() {
        let body = write_body(EqualizerSetting {
            preset_id: 0xfefe,
            volume_adjustments: LEGACY_CURVE,
        })
        .expect("measured write");
        assert_eq!(
            body,
            vec![
                0xfe, 0xfe, // preset
                0x3c, 0xb4, 0x8f, 0xf0, 0x8e, 0x00, 0x74, 0x88, 0x78, 0x00, // curve
                0x6d, 0x86, 0x70, 0x88, 0x7b, 0x66, 0x7e, 0x79, 0x78, 0x00, // DRC
            ]
        );
    }

    #[test]
    fn rejects_gains_outside_the_representable_range() {
        let mut setting = EqualizerSetting {
            preset_id: 0,
            volume_adjustments: [0; BAND_COUNT],
        };
        for invalid in [i16::MIN, -121, 135, i16::MAX] {
            setting.volume_adjustments[9] = invalid;
            assert_eq!(write_body(setting), Err(ModelError::InvalidEqualizerWrite));
        }
    }
}
