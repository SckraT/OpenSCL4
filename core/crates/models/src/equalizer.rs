use crate::EQUALIZER_BANDS;

/// Number of bands that the DRC transform covers; the last two keep their
/// fixed defaults.
const DRC_BANDS: usize = 8;

const SMALLER_COEFFICIENT: f64 = 0.85;
const LARGER_COEFFICIENT: f64 = 0.95;

/// The measured band coefficients of the Soundcore DRC transform.
///
/// The values come from the legacy `VolumeAdjustments::apply_drc`, where the
/// small and large coefficients scale the neighbouring band weights.
const BAND_COEFFICIENTS: [[f64; DRC_BANDS]; DRC_BANDS] = [
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

/// The measured curve with the DRC transform applied.
///
/// The equalizer write formats of the Liberty 4 NC and P40i carry this copy as
/// their active curve. The transform covers the first eight bands; the last two
/// bands keep their fixed defaults, exactly like the legacy builder.
pub fn drc(gains: [i16; EQUALIZER_BANDS]) -> [i16; EQUALIZER_BANDS] {
    let adjustments = gains.map(|gain| f64::from(gain) / 10.0);
    let lows_subtraction = adjustments[2] * 0.81 * SMALLER_COEFFICIENT;
    let highs_subtraction = adjustments[5] * 0.81 * SMALLER_COEFFICIENT;

    let multiplied: [f64; DRC_BANDS] = std::array::from_fn(|band| {
        let coefficients = BAND_COEFFICIENTS[band];
        (0..DRC_BANDS).fold(0.0, |acc, index| {
            // The adjacent low and high bands are replaced by their subtraction
            // term for the same band pairs the legacy algorithm special-cases.
            if matches!(band, 1 | 3) && index == 2 {
                acc - lows_subtraction
            } else if matches!(band, 4 | 6) && index == 5 {
                acc - highs_subtraction
            } else {
                acc + adjustments[index] * coefficients[index]
            }
        })
    });

    let mut result = [0; EQUALIZER_BANDS];
    for (band, multiplied) in multiplied.iter().enumerate() {
        // The legacy code divides by ten, scales back to tenths of a decibel and
        // rounds, then clamps to the model range.
        let scaled = (multiplied / 10.0) * 10.0;
        result[band] = (scaled.round() as i16).clamp(-120, 134);
    }
    result[DRC_BANDS] = 0;
    result[DRC_BANDS + 1] = -120;
    result
}

/// Encodes the DRC-transformed curve in the measured wire scale.
pub fn drc_bytes(gains: [i16; EQUALIZER_BANDS]) -> [u8; EQUALIZER_BANDS] {
    drc(gains).map(crate::EqualizerSetting::band_byte)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_the_legacy_drc_examples() {
        // The reference values are the legacy `apply_drc` unit-test examples,
        // converted from decibels to tenths of a decibel.
        let examples: [([i16; EQUALIZER_BANDS], [i16; EQUALIZER_BANDS]); 3] = [
            (
                [-60, 60, 23, 120, 22, -120, -4, 16, 0, 0],
                [-11, 14, -8, 16, 3, -18, 6, 1, 0, -120],
            ),
            (
                [120, 120, 120, 120, 120, 120, 120, 120, 0, 0],
                [10, 6, 7, 7, 7, 7, 7, 13, 0, -120],
            ),
            (
                [-120, -120, -120, -120, -120, -120, -120, -120, 0, 0],
                [-10, -6, -7, -7, -7, -7, -7, -13, 0, -120],
            ),
        ];
        for (curve, expected) in examples {
            assert_eq!(drc(curve), expected, "{curve:?}");
        }
    }

    #[test]
    fn flat_curves_keep_their_measured_bytes() {
        // A flat curve stays flat; the fixed last band is the absent marker.
        assert_eq!(
            drc_bytes([0; EQUALIZER_BANDS]),
            [120, 120, 120, 120, 120, 120, 120, 120, 120, 0]
        );
        // The eighth band rounds to +1 tenth of a decibel for a small ramp,
        // which the legacy golden test records for the measured write.
        assert_eq!(
            drc_bytes([0, 1, 2, 3, 4, 5, 6, 7, 0, 0]),
            [120, 120, 120, 120, 120, 120, 120, 121, 120, 0]
        );
    }
}
