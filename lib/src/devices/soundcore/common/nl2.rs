//! Hearing-test calculation used by the Soundcore mobile application.
//!
//! The application receives eight values per ear from the device, passes them
//! to a small fully connected network that ships in the APK
//! (`assets/model_parameters/fc{1..4}_{weight,bias}.txt`), and uses the eight
//! integers that come back: they are added to the first eight equalizer bands
//! and are also sent to the device.
//!
//! This module is the pure calculation only. It does not read the APK, does not
//! know about the serial port, and makes no claim about which products accept
//! the result.
//!
//! The contract was taken from the original `libscnativelib.so` and checked
//! against it: 59 input pairs and 944 integer outputs match the native library
//! loaded from the installed APK, including the JNI list marshalling and the
//! `Map` return value.
//!
//! # Contract
//!
//! * The native entry point takes a matrix and uses **only the first row**; this
//!   module takes that row directly. At least nine values are required.
//! * The network reads five of them, at zero-based positions `1, 2, 4, 6, 8`.
//!   The application builds a nine-element row from its eight device values with
//!   a `999` filler at position `3`, which [`nine_slot_row`] reproduces.
//! * If none of the three last selected values reaches `35`, the result is eight
//!   zeros without running the network.
//! * Four fully connected layers (`64x5`, `128x64`, `64x128`, `19x64`), the
//!   first three followed by ReLU, the last one without an activation.
//! * Output values are truncated toward zero and clamped from below at zero;
//!   this is not rounding. Eight of the nineteen outputs are returned, at
//!   positions `0, 2, 5, 8, 11, 14, 17, 18`.

use std::fmt;
use std::fs;
use std::io;
use std::path::Path;

/// Positions of the first row that the network reads.
pub const INPUT_SLOTS: [usize; 5] = [1, 2, 4, 6, 8];

/// Positions of the network output that become the result, in order.
pub const OUTPUT_SLOTS: [usize; 8] = [0, 2, 5, 8, 11, 14, 17, 18];

/// Value the application writes into the slot that is not a measurement.
pub const APPLICATION_FILLER: f64 = 999.0;

/// The network starts only when one of the three last selected values reaches
/// this value.
pub const HEARING_LOSS_THRESHOLD: f64 = 35.0;

/// Rows and columns of every layer, in order.
const LAYER_SHAPES: [(usize, usize); 4] = [(64, 5), (128, 64), (64, 128), (19, 64)];

/// Count of values the last layer produces.
const NETWORK_OUTPUTS: usize = 19;

/// Count of values the native entry point reads from the first row.
const REQUIRED_ROW_VALUES: usize = 9;

/// One fully connected layer, weights stored row by row.
#[derive(Debug, Clone, PartialEq)]
struct Layer {
    columns: usize,
    weights: Vec<f64>,
    bias: Vec<f64>,
}

impl Layer {
    fn forward(&self, input: &[f64]) -> Vec<f64> {
        let rows = self.bias.len();
        let mut output = Vec::with_capacity(rows);
        for row in 0..rows {
            let weights = &self.weights[row * self.columns..(row + 1) * self.columns];
            let mut total = self.bias[row];
            for (weight, value) in weights.iter().zip(input) {
                total += weight * value;
            }
            output.push(total);
        }
        output
    }
}

/// The four layers of the shipped network.
#[derive(Debug, Clone, PartialEq)]
pub struct Network {
    layers: [Layer; 4],
}

impl Network {
    /// Loads the eight parameter files from the directory that holds them.
    ///
    /// The directory is the one the application copies out of its assets, so the
    /// names are fixed: `fc1_weight.txt`, `fc1_bias.txt`, and so on.
    pub fn from_directory(directory: &Path) -> Result<Self, Error> {
        let mut layers = Vec::with_capacity(LAYER_SHAPES.len());
        for (index, (rows, columns)) in LAYER_SHAPES.into_iter().enumerate() {
            let number = index + 1;
            let weights = read_numbers(
                &directory.join(format!("fc{number}_weight.txt")),
                rows * columns,
            )?;
            let bias = read_numbers(&directory.join(format!("fc{number}_bias.txt")), rows)?;
            layers.push(Layer {
                columns,
                weights,
                bias,
            });
        }
        let layers: [Layer; 4] = layers
            .try_into()
            .map_err(|_| Error::Shape("expected four layers"))?;
        Ok(Self { layers })
    }

    /// Builds a network from parameters that were already parsed.
    pub fn from_parameters(layers: [[Vec<f64>; 2]; 4]) -> Result<Self, Error> {
        let mut built = Vec::with_capacity(LAYER_SHAPES.len());
        for (index, [weights, bias]) in layers.into_iter().enumerate() {
            let (rows, columns) = LAYER_SHAPES[index];
            if weights.len() != rows * columns || bias.len() != rows {
                return Err(Error::Shape(
                    "parameter count does not match the layer shape",
                ));
            }
            if weights.iter().chain(&bias).any(|value| !value.is_finite()) {
                return Err(Error::Shape("parameters must be finite"));
            }
            built.push(Layer {
                columns,
                weights,
                bias,
            });
        }
        let built: [Layer; 4] = built
            .try_into()
            .map_err(|_| Error::Shape("expected four layers"))?;
        Ok(Self { layers: built })
    }

    /// Whether the row activates the network.
    ///
    /// The two first selected values never activate it on their own; one of the
    /// three last ones has to reach [`HEARING_LOSS_THRESHOLD`].
    pub fn is_active(row: &[f64]) -> bool {
        if row.len() < REQUIRED_ROW_VALUES {
            return false;
        }
        INPUT_SLOTS[2..]
            .iter()
            .any(|slot| row[*slot] >= HEARING_LOSS_THRESHOLD)
    }

    /// Calculates the eight result values for one ear from a single input row.
    ///
    /// Only the first row of the matrix is ever used by the native entry point,
    /// so pass that row; anything after the ninth value is ignored.
    pub fn calculate(&self, row: &[f64]) -> Result<[i32; 8], Error> {
        if row.len() < REQUIRED_ROW_VALUES {
            return Err(Error::ShortRow { found: row.len() });
        }
        let selected: [f64; 5] = INPUT_SLOTS.map(|slot| row[slot]);
        if selected.iter().any(|value| !value.is_finite()) {
            return Err(Error::NonFiniteInput);
        }

        let active = Self::is_active(row);
        let values = if active {
            let mut values = selected.to_vec();
            for (index, layer) in self.layers.iter().enumerate() {
                values = layer.forward(&values);
                if index + 1 < self.layers.len() {
                    for value in &mut values {
                        *value = value.max(0.0);
                    }
                }
            }
            values
        } else {
            vec![0.0; NETWORK_OUTPUTS]
        };

        if values.len() != NETWORK_OUTPUTS {
            return Err(Error::Shape("unexpected output length"));
        }
        let quantized: Vec<i32> = values.into_iter().map(quantize).collect::<Result<_, _>>()?;
        Ok(OUTPUT_SLOTS.map(|slot| quantized[slot]))
    }

    /// Calculates both ears from the eight device values of each ear.
    pub fn calculate_hearing_test(
        &self,
        left: &[f64; 8],
        right: &[f64; 8],
    ) -> Result<[[i32; 8]; 2], Error> {
        Ok([
            self.calculate(&nine_slot_row(left))?,
            self.calculate(&nine_slot_row(right))?,
        ])
    }
}

/// Builds the nine-slot row the application passes to the native entry point.
///
/// The eight device values are kept in order and one filler takes the fourth
/// slot, which the network never reads.
pub fn nine_slot_row(values: &[f64; 8]) -> [f64; 9] {
    let [first, second, third, fourth, fifth, sixth, seventh, eighth] = *values;
    [
        first,
        second,
        third,
        APPLICATION_FILLER,
        fourth,
        fifth,
        sixth,
        seventh,
        eighth,
    ]
}

fn quantize(value: f64) -> Result<i32, Error> {
    if !value.is_finite() || value <= i32::MIN as f64 || value >= i32::MAX as f64 {
        return Err(Error::UnsupportedOutput);
    }
    Ok(value.trunc().max(0.0) as i32)
}

fn read_numbers(path: &Path, expected: usize) -> Result<Vec<f64>, Error> {
    let text = fs::read_to_string(path).map_err(Error::Io)?;
    let numbers = text
        .split_whitespace()
        .map(|token| {
            token.parse::<f64>().map_err(|_| Error::InvalidParameters {
                file: path.display().to_string(),
            })
        })
        .collect::<Result<Vec<f64>, _>>()?;
    if numbers.len() != expected {
        return Err(Error::ParameterCount {
            file: path.display().to_string(),
            expected,
            found: numbers.len(),
        });
    }
    if numbers.iter().any(|value| !value.is_finite()) {
        return Err(Error::InvalidParameters {
            file: path.display().to_string(),
        });
    }
    Ok(numbers)
}

/// What can go wrong while loading parameters or calculating.
#[derive(Debug)]
pub enum Error {
    /// A parameter file could not be read.
    Io(io::Error),
    /// A parameter file holds a value that is not a number.
    InvalidParameters {
        /// File that failed to parse.
        file: String,
    },
    /// A parameter file holds a different number of values than the layer needs.
    ParameterCount {
        /// File with the wrong count.
        file: String,
        /// Number of values the layer needs.
        expected: usize,
        /// Number of values the file holds.
        found: usize,
    },
    /// The recorded layer shapes and the supplied parameters disagree.
    Shape(&'static str),
    /// The input row is shorter than the nine values the native entry point reads.
    ShortRow {
        /// Length of the supplied row.
        found: usize,
    },
    /// The input row holds a value that is not finite.
    NonFiniteInput,
    /// A network output cannot be represented as a result value.
    UnsupportedOutput,
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "cannot read the model parameters: {error}"),
            Self::InvalidParameters { file } => {
                write!(formatter, "{file} holds a value that is not a number")
            }
            Self::ParameterCount {
                file,
                expected,
                found,
            } => write!(
                formatter,
                "{file} holds {found} values, the layer needs {expected}"
            ),
            Self::Shape(message) => write!(formatter, "invalid network shape: {message}"),
            Self::ShortRow { found } => write!(
                formatter,
                "the input row holds {found} values, at least {REQUIRED_ROW_VALUES} are required"
            ),
            Self::NonFiniteInput => {
                write!(formatter, "the input row holds a value that is not finite")
            }
            Self::UnsupportedOutput => {
                write!(formatter, "a network output is outside the supported range")
            }
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat_network(value: f64) -> Network {
        let weights = vec![
            value;
            LAYER_SHAPES
                .iter()
                .map(|(rows, columns)| rows * columns)
                .sum()
        ];
        let mut offset = 0;
        let layers = LAYER_SHAPES.map(|(rows, columns)| {
            let layer = (
                weights[offset..offset + rows * columns].to_vec(),
                vec![0.0; rows],
            );
            offset += rows * columns;
            [layer.0, layer.1]
        });
        Network::from_parameters(layers).unwrap()
    }

    #[test]
    fn nine_slot_row_keeps_device_order_and_the_filler() {
        let values = [10.0, 20.0, 30.0, 40.0, 50.0, 60.0, 70.0, 80.0];
        assert_eq!(
            nine_slot_row(&values),
            [
                10.0,
                20.0,
                30.0,
                APPLICATION_FILLER,
                40.0,
                50.0,
                60.0,
                70.0,
                80.0
            ]
        );
    }

    #[test]
    fn short_row_is_rejected() {
        let network = flat_network(0.0);
        assert!(matches!(
            network.calculate(&[1.0; 8]),
            Err(Error::ShortRow { .. })
        ));
    }

    #[test]
    fn network_stays_off_just_below_the_threshold() {
        let mut row = [0.0; 9];
        row[4] = HEARING_LOSS_THRESHOLD - 0.1;
        row[1] = 1000.0;
        row[2] = 1000.0;
        assert!(!Network::is_active(&row));
    }

    #[test]
    fn threshold_is_inclusive() {
        let mut row = [0.0; 9];
        row[8] = HEARING_LOSS_THRESHOLD;
        assert!(Network::is_active(&row));
    }

    #[test]
    fn ignored_slots_never_activate_the_network() {
        let mut row = [0.0; 9];
        for slot in [0, 1, 2, 3, 5, 7] {
            row[slot] = 1000.0;
        }
        assert!(!Network::is_active(&row));
    }

    #[test]
    fn results_are_truncated_toward_zero_and_clamped() {
        assert_eq!(quantize(1.99).unwrap(), 1);
        assert_eq!(quantize(-3.5).unwrap(), 0);
        assert_eq!(quantize(0.0).unwrap(), 0);
        assert!(quantize(f64::NAN).is_err(), "NaN has no integer result");
    }
}
