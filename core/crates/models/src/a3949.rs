//! Measured A3949 (P20i / P25i / R50i) state and write formats.
//!
//! The layouts come from the legacy driver at
//! `lib/src/devices/soundcore/a3949/**` and the saved P20i captures in
//! `core/data/`. The body offsets are shared by the legacy state parser and the
//! measured 88-byte state response; the captured body carries 15 trailing
//! bytes that the legacy parser does not describe and this decoder keeps as
//! unassigned state.

use crate::{
    BatteryLevels, ButtonConfiguration, ButtonStatus, DeviceState, EqualizerSetting,
    FirmwareVersion, HostDevice, ModelDriver, ModelError, RedactedSerial, TwsStatus,
    battery_percent,
};

/// Body length of the measured A3949 pilot state.
pub const STATE_LENGTH: usize = 88;

/// Two-byte TWS status: host device and connection flag.
pub const TWS_STATUS: std::ops::Range<usize> = 0..2;
/// Left earbud battery level on the five-step scale.
pub const BATTERY_LEFT: usize = 2;
/// Right earbud battery level on the five-step scale.
pub const BATTERY_RIGHT: usize = 3;
/// Left and right firmware strings, five bytes each.
pub const FIRMWARE: usize = 6;
/// Serial number, also read by the measured identity layout.
pub const SERIAL_NUMBER: usize = 16;
/// Equalizer preset identifier, little-endian.
pub const A3949_EQUALIZER_PRESET: std::ops::Range<usize> = 32..34;
/// Ten equalizer band values, one byte per band.
pub const A3949_EQUALIZER_BANDS: std::ops::Range<usize> = 34..44;
/// Six button status entries, two bytes each: enabled flag and action byte.
pub const BUTTONS: std::ops::Range<usize> = 55..67;
/// Number of button entries in the state and the button configuration.
pub const BUTTON_COUNT: usize = 6;
/// Body offset of the measured A3949 gaming-mode flag.
pub const A3949_GAMING_MODE: usize = 71;
/// Body offset of the measured A3949 touch-tone flag.
pub const A3949_TOUCH_TONE: usize = 72;

/// Preset bytes of the measured equalizer write body.
pub const A3949_EQUALIZER_WRITE_PRESET: std::ops::Range<usize> = 0..2;
/// Ten raw band bytes of the measured equalizer write body.
pub const A3949_EQUALIZER_WRITE_BANDS: std::ops::Range<usize> = 2..12;
/// Ten DRC bytes of the measured equalizer write body.
pub const A3949_EQUALIZER_WRITE_DRC: std::ops::Range<usize> = 12..22;

/// Equalizer write command of the DRC devices.
pub const EQUALIZER_COMMAND: [u8; 2] = [0x02, 0x83];
/// Touch-tone write command.
pub const TOUCH_TONE_COMMAND: [u8; 2] = [0x01, 0x83];
/// Single-button action write command.
pub const BUTTON_COMMAND: [u8; 2] = [0x04, 0x81];
/// Reset-all-buttons action command.
pub const RESET_BUTTONS_COMMAND: [u8; 2] = [0x04, 0x82];

const BATTERY_SCALE: u8 = 5;
const BUTTON_ENTRY_LENGTH: usize = 2;
/// Action byte value of a disabled button in the TWS low-bits encoding.
pub const DISABLED_ACTION: u8 = 0x0F;
const PRESET_BYTES: usize = 2;
const DRC_BAND_COUNT: usize = 8;
const DRC_DEFAULT_BANDS: [i16; 10] = [0, 0, 0, 0, 0, 0, 0, 0, 0, -120];

/// One physical button whose action can be reassigned.
///
/// The order is the measured state order: left and right single presses,
/// double presses, then long presses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
    /// Left single press.
    LeftSinglePress,
    /// Right single press.
    RightSinglePress,
    /// Left double press.
    LeftDoublePress,
    /// Right double press.
    RightDoublePress,
    /// Left long press.
    LeftLongPress,
    /// Right long press.
    RightLongPress,
}

impl Button {
    /// The position of this button's entry in the state response.
    pub const fn index(self) -> usize {
        match self {
            Self::LeftSinglePress => 0,
            Self::RightSinglePress => 1,
            Self::LeftDoublePress => 2,
            Self::RightDoublePress => 3,
            Self::LeftLongPress => 4,
            Self::RightLongPress => 5,
        }
    }

    /// The wire side byte: left is zero and right is one.
    pub const fn side(self) -> u8 {
        match self {
            Self::LeftSinglePress | Self::LeftDoublePress | Self::LeftLongPress => 0,
            Self::RightSinglePress | Self::RightDoublePress | Self::RightLongPress => 1,
        }
    }

    /// The wire button id of this button's press kind.
    pub const fn button_id(self) -> u8 {
        match self {
            Self::LeftSinglePress | Self::RightSinglePress => 2,
            Self::LeftDoublePress | Self::RightDoublePress => 0,
            Self::LeftLongPress | Self::RightLongPress => 1,
        }
    }

    /// The stable CLI identifier.
    pub const fn code(self) -> &'static str {
        match self {
            Self::LeftSinglePress => "LeftSinglePress",
            Self::RightSinglePress => "RightSinglePress",
            Self::LeftDoublePress => "LeftDoublePress",
            Self::RightDoublePress => "RightDoublePress",
            Self::LeftLongPress => "LeftLongPress",
            Self::RightLongPress => "RightLongPress",
        }
    }

    /// Resolves a button from its wire side and button id.
    pub const fn from_wire(side: u8, button_id: u8) -> Option<Self> {
        let left = side == 0;
        if side > 1 {
            return None;
        }
        match (button_id, left) {
            (2, true) => Some(Self::LeftSinglePress),
            (2, false) => Some(Self::RightSinglePress),
            (0, true) => Some(Self::LeftDoublePress),
            (0, false) => Some(Self::RightDoublePress),
            (1, true) => Some(Self::LeftLongPress),
            (1, false) => Some(Self::RightLongPress),
            _ => None,
        }
    }
}

/// The layout of the measured A3949 serial at body offsets 16-31.
pub fn state_serial(body: &[u8]) -> Result<RedactedSerial<'_>, ModelError> {
    if body.len() < STATE_LENGTH {
        return Err(ModelError::IncompleteState);
    }
    RedactedSerial::parse(&body[SERIAL_NUMBER..SERIAL_NUMBER + 16])
}

/// Reads the two-byte TWS status of an A3949 state body.
pub fn tws_status(body: &[u8]) -> Option<TwsStatus> {
    if body.len() < STATE_LENGTH {
        return None;
    }
    let host_device = HostDevice::from_byte(body[TWS_STATUS.start])?;
    Some(TwsStatus {
        host_device,
        connected: body[TWS_STATUS.start + 1] == 1,
    })
}

/// Reads the six button status entries of an A3949 state body.
pub fn buttons(body: &[u8]) -> Option<[ButtonStatus; BUTTON_COUNT]> {
    if body.len() < STATE_LENGTH {
        return None;
    }
    Some(std::array::from_fn(|index| {
        let entry = BUTTONS.start + index * BUTTON_ENTRY_LENGTH;
        ButtonStatus {
            enabled: body[entry] == 1,
            connected_action: body[entry + 1] & 0x0F,
            disconnected_action: body[entry + 1] >> 4,
        }
    }))
}

/// Whether the measured state reports both earbuds, which gates equalizer writes.
pub fn both_earbuds_connected(body: &[u8]) -> bool {
    body.len() == STATE_LENGTH && body[TWS_STATUS.start + 1] == 1
}

pub fn decode_state(body: &[u8]) -> Result<DeviceState, ModelError> {
    if body.len() < STATE_LENGTH {
        return Err(ModelError::IncompleteState);
    }
    if !state_serial(body)?.identifies(ModelDriver::A3949) {
        return Err(ModelError::UnknownModel);
    }
    let tws = tws_status(body).ok_or(ModelError::InvalidStateField)?;
    Ok(DeviceState {
        model: ModelDriver::A3949,
        battery: BatteryLevels {
            device: None,
            left: battery_percent(body[BATTERY_LEFT], BATTERY_SCALE, 0)?,
            right: battery_percent(body[BATTERY_RIGHT], BATTERY_SCALE, 0)?,
            case: None,
        },
        sound_mode: None,
        sound_mode_controls: None,
        equalizer: Some(equalizer(body)),
        gaming_mode: match (body.len(), body[A3949_GAMING_MODE]) {
            (STATE_LENGTH, 0) => Some(false),
            (STATE_LENGTH, 1) => Some(true),
            _ => None,
        },
        auto_power_off: None,
        firmware_device: None,
        firmware_left: FirmwareVersion::parse(&body[FIRMWARE..FIRMWARE + 5])?,
        firmware_right: FirmwareVersion::parse(&body[FIRMWARE + 5..FIRMWARE + 10])?,
        tws: Some(tws),
        buttons: buttons(body).map(ButtonConfiguration::new),
        touch_tone: Some(body[A3949_TOUCH_TONE] == 1),
    })
}

/// The reported equalizer curve. The state stores the raw band values; the
/// device applies its DRC layer internally and does not report it.
fn equalizer(body: &[u8]) -> EqualizerSetting {
    let preset_id = u16::from_le_bytes([
        body[A3949_EQUALIZER_PRESET.start],
        body[A3949_EQUALIZER_PRESET.start + 1],
    ]);
    EqualizerSetting {
        preset_id,
        volume_adjustments: std::array::from_fn(|band| {
            (i16::from(body[A3949_EQUALIZER_BANDS.start + band]) - 120).clamp(-120, 134)
        }),
    }
}

/// Builds the measured `02 83` equalizer-with-DRC write body.
///
/// The body is the preset id (little-endian), the ten raw band bytes, and the
/// ten DRC values the device expects next to them. The DRC curve is the
/// legacy driver's documented transform of the raw curve.
pub fn equalizer_write_body(
    setting: EqualizerSetting,
    state: &[u8],
) -> Result<Vec<u8>, ModelError> {
    if !both_earbuds_connected(state) || !setting.is_writable() {
        return Err(ModelError::InvalidEqualizerWrite);
    }
    let mut body = Vec::with_capacity(PRESET_BYTES + 2 * 10);
    body.extend_from_slice(&setting.preset_id.to_le_bytes());
    body.extend(
        setting
            .volume_adjustments
            .iter()
            .map(|gain| EqualizerSetting::band_byte(*gain)),
    );
    body.extend(
        drc_curve(setting.volume_adjustments)
            .iter()
            .map(|gain| EqualizerSetting::band_byte(*gain)),
    );
    Ok(body)
}

/// The documented DRC transform of a raw curve for the DRC write command.
///
/// Only the first eight bands are transformed; the last two keep the driver's
/// fixed defaults. See `lib/src/devices/soundcore/common/structures/volume_adjustments.rs`.
fn drc_curve(volume_adjustments: [i16; 10]) -> [i16; 10] {
    const SMALLER_COEFFICIENT: f64 = 0.85;
    const LARGER_COEFFICIENT: f64 = 0.95;
    const BAND_COEFFICIENTS: [[f64; DRC_BAND_COUNT]; DRC_BAND_COUNT] = [
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

    let adjustments: [f64; 10] = volume_adjustments.map(|adjustment| f64::from(adjustment) / 10.0);
    let lows_subtraction = adjustments[2] * 0.81 * SMALLER_COEFFICIENT;
    let highs_subtraction = adjustments[5] * 0.81 * SMALLER_COEFFICIENT;

    let mut transformed = [0i16; 10];
    for (band, coefficients) in BAND_COEFFICIENTS.iter().enumerate() {
        let mut accumulated = 0.0;
        for (index, adjustment) in adjustments.iter().take(DRC_BAND_COUNT).enumerate() {
            if (band == 1 || band == 3) && index == 2 {
                accumulated -= lows_subtraction;
            } else if (band == 4 || band == 6) && index == 5 {
                accumulated -= highs_subtraction;
            } else {
                accumulated += adjustment * coefficients[index];
            }
        }
        transformed[band] = accumulated.round().clamp(-120.0, 134.0) as i16;
    }
    for (band, default) in DRC_DEFAULT_BANDS.iter().enumerate().skip(DRC_BAND_COUNT) {
        transformed[band] = *default;
    }
    transformed
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The legacy R50i request body for the Piano preset:
    /// `lib/src/devices/soundcore/a3949.rs::r50i_set_eq_matches_soundcore_app`.
    const PIANO_WRITE_BODY: &[u8] = &[
        14, 0, 120, 150, 150, 140, 160, 170, 150, 160, 120, 0, 119, 123, 122, 120, 123, 124, 120,
        125, 120, 0,
    ];
    const PIANO_CURVE: [i16; 10] = [0, 30, 30, 20, 40, 50, 30, 40, 0, -120];

    #[test]
    fn drc_curve_matches_the_legacy_r50i_write_body() {
        let setting = EqualizerSetting {
            preset_id: 0x000e,
            volume_adjustments: PIANO_CURVE,
        };
        let state = include_bytes!("../../../data/a3949-pilot-state.bin");
        let body = &state[9..state.len() - 1];
        assert_eq!(
            equalizer_write_body(setting, body).expect("measured layout"),
            PIANO_WRITE_BODY
        );
    }

    #[test]
    fn button_writes_use_the_legacy_side_and_button_ids() {
        let state = include_bytes!("../../../data/a3949-pilot-state.bin");
        let body = &state[9..state.len() - 1];
        let decoded = decode_state(body).expect("measured layout");
        let cases = [
            (Button::LeftSinglePress, [0, 2]),
            (Button::RightSinglePress, [1, 2]),
            (Button::LeftDoublePress, [0, 0]),
            (Button::RightDoublePress, [1, 0]),
            (Button::LeftLongPress, [0, 1]),
            (Button::RightLongPress, [1, 1]),
        ];
        for (button, [side, button_id]) in cases {
            let write = decoded
                .button_write(button, Some(2))
                .expect("measured layout");
            assert_eq!([write[0], write[1]], [side, button_id]);
            assert_eq!(write[2], 2);
            assert_eq!(
                decoded.button_write(button, None).expect("measured layout")[2],
                DISABLED_ACTION
            );
        }
        assert_eq!(
            decoded.button_write(Button::LeftSinglePress, Some(DISABLED_ACTION)),
            Err(ModelError::InvalidButtonWrite)
        );
    }

    #[test]
    fn touch_tone_uses_the_measured_flag_body() {
        let state = include_bytes!("../../../data/a3949-pilot-state.bin");
        let body = &state[9..state.len() - 1];
        let decoded = decode_state(body).expect("measured layout");
        assert_eq!(
            decoded.touch_tone_write(true).expect("measured layout"),
            [1]
        );
        assert_eq!(
            decoded.touch_tone_write(false).expect("measured layout"),
            [0]
        );
    }
}
