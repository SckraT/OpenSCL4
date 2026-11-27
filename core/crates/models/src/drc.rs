//! DRC curve transform carried by the equalizer writes of some models.
//!
//! The A3944, A3948 and A3959 writes repeat the requested curve as a
//! DRC-adjusted copy. The coefficients, the order of the arithmetic and the
//! reset of the invisible bands follow the legacy
//! `lib/src/devices/soundcore/common/structures/volume_adjustments.rs::apply_drc`.

use crate::{EQUALIZER_BANDS, EQUALIZER_MAX_GAIN, EQUALIZER_MIN_GAIN};

const SMALLER_COEFFICIENT: f64 = 0.85;
const LARGER_COEFFICIENT: f64 = 0.95;

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
        0.0, // subtraction
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
        0.0, // subtraction
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
        0.0, // subtraction
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
        0.0, // subtraction
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

/// The default tenths-of-a-decibel values the legacy transform keeps for the
/// two invisible bands.
const INVISIBLE_BANDS: [i16; 2] = [0, EQUALIZER_MIN_GAIN];

/// Applies the legacy DRC transform to one curve.
pub fn apply_drc(gains: [i16; EQUALIZER_BANDS]) -> [i16; EQUALIZER_BANDS] {
    let adjustments = gains.map(|gain| f64::from(gain) / 10.0);
    let lows_subtraction = adjustments[2] * 0.81 * SMALLER_COEFFICIENT;
    let highs_subtraction = adjustments[5] * 0.81 * SMALLER_COEFFICIENT;

    let mut transformed = [0_i16; EQUALIZER_BANDS];
    for (band_index, coefficients) in BAND_COEFFICIENTS.iter().enumerate() {
        let multiplied =
            adjustments[..8]
                .iter()
                .enumerate()
                .fold(0.0, |accumulated, (index, adjustment)| {
                    // The legacy transform replaces two coefficients with the
                    // subtracted low/high band values.
                    if (band_index == 1 || band_index == 3) && index == 2 {
                        accumulated - lows_subtraction
                    } else if (band_index == 4 || band_index == 6) && index == 5 {
                        accumulated - highs_subtraction
                    } else {
                        accumulated + adjustment * coefficients[index]
                    }
                });
        // The legacy code divides by ten and multiplies back after rounding, so
        // the published values are reproduced exactly.
        transformed[band_index] = ((multiplied / 10.0) * 10.0).round() as i16;
    }
    transformed[8] = INVISIBLE_BANDS[0];
    transformed[9] = INVISIBLE_BANDS[1];
    transformed.map(|gain| gain.clamp(EQUALIZER_MIN_GAIN, EQUALIZER_MAX_GAIN))
}
