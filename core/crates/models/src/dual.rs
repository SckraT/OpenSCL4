//! Shared decoding for dual-battery models that keep the serial at body offset 16.
//!
//! A3936, A3952 and A3968 share the legacy layout: TWS status, two battery
//! levels and charging flags, a dual firmware version, the serial, an equalizer
//! configuration, a 16-byte hear-id block and a six-byte sound-mode block. The
//! fields this module reads are the ones whose offsets the legacy parsers fix;
//! every offset lives in the model's [`DualLayout`].

use std::ops::Range;

use crate::{
    AUTO_POWER_OFF_DURATION_COUNT, AmbientSoundModeCycle, AutoPowerOff, BatteryLevels, DeviceState,
    EQUALIZER_BANDS, EqualizerSetting, FirmwareVersion, ModelDriver, ModelError, SoundMode,
    SoundModeControls, SoundModeCycle, TwsStatus, battery_percent,
};

/// Serial offset shared by the dual-battery state layouts.
pub const SERIAL: Range<usize> = 16..32;
/// Length of the six-byte sound-mode block these models write as a whole.
pub const SOUND_MODE_BLOCK: usize = 6;
/// Length of a firmware version string on the wire.
const FIRMWARE_LENGTH: usize = 5;

/// Where a dual-battery model keeps each field in its state body.
pub struct DualLayout {
    /// Shortest confirmed body: the length the legacy serializer writes.
    pub state_length: usize,
    /// Bodies whose layout is confirmed, including recorded trailing data.
    pub confirmed_lengths: &'static [usize],
    /// First battery level byte; the second earbud follows it.
    pub battery: usize,
    /// Full-scale value of one battery step.
    pub battery_scale: u8,
    /// First firmware version byte; the right earbud version follows it.
    pub firmware: usize,
    /// First preset id byte of the equalizer configuration.
    pub equalizer_preset: usize,
    /// First band byte of the first equalizer channel.
    pub equalizer_bands: usize,
    /// Charging-case level byte.
    pub case_battery: usize,
    /// Full-scale value of one charging-case step.
    pub case_battery_scale: u8,
    /// Offset added to a charging-case step before scaling.
    pub case_battery_offset: u8,
    /// Host-device byte; the TWS-connected flag follows it.
    pub tws_status: usize,
    /// First byte of the six-byte sound-mode block.
    pub sound_modes: usize,
    /// Ambient sound mode cycle byte.
    pub ambient_cycle: usize,
    /// Whether the cycle carries separate TWS-connected and disconnected values.
    pub tws_cycle: bool,
    /// First byte of the auto power off pair.
    pub auto_power_off: usize,
    /// Gaming-mode flag, for a model whose state carries one.
    pub gaming_mode: Option<usize>,
    /// Value the legacy writer fixes for the sound-mode block's last byte.
    pub sound_mode_tail: Option<u8>,
}

/// Reads the model-specific serial from the shared dual-battery offset.
pub fn state_serial(body: &[u8]) -> Result<crate::RedactedSerial<'_>, ModelError> {
    let serial = body.get(SERIAL).ok_or(ModelError::IncompleteState)?;
    let serial: &[u8; 16] = serial.try_into().map_err(|_| ModelError::IncompleteState)?;
    crate::RedactedSerial::parse(serial)
}

/// Whether the body is one of the layouts recorded for this model.
pub fn has_confirmed_layout(body: &[u8], layout: &DualLayout) -> bool {
    layout.confirmed_lengths.contains(&body.len())
}

/// Decodes the state fields shared by every dual-battery model.
pub fn decode_state(
    model: ModelDriver,
    body: &[u8],
    layout: &DualLayout,
) -> Result<DeviceState, ModelError> {
    if body.len() < layout.state_length {
        return Err(ModelError::IncompleteState);
    }
    if !state_serial(body)?.identifies(model) {
        return Err(ModelError::UnknownModel);
    }
    let sound_modes = sound_mode_block(body, layout)?;
    let sound_mode = SoundMode::decode(sound_modes[0])?;
    if sound_mode == SoundMode::AirplaneMode {
        return Err(ModelError::UnknownSoundMode);
    }
    Ok(DeviceState {
        model,
        battery: BatteryLevels {
            device: None,
            left: battery_percent(body[layout.battery], layout.battery_scale, 0)?,
            right: battery_percent(body[layout.battery + 1], layout.battery_scale, 0)?,
            case: case_battery(body, layout)?,
        },
        sound_mode: Some(sound_mode),
        sound_mode_controls: Some(SoundModeControls::Dual(sound_modes)),
        equalizer: equalizer_setting(body, layout),
        gaming_mode: gaming_mode(body, layout),
        auto_power_off: auto_power_off(body, layout),
        firmware_device: None,
        firmware_left: FirmwareVersion::parse(&body[layout.firmware..][..FIRMWARE_LENGTH])?,
        firmware_right: FirmwareVersion::parse(
            &body[layout.firmware + FIRMWARE_LENGTH..][..FIRMWARE_LENGTH],
        )?,
        tws_status: Some(tws_status(body, layout)),
        ambient_sound_mode_cycle: Some(ambient_sound_mode_cycle(body, layout)),
    })
}

/// Builds the six-byte sound-mode write from the latest measured block.
///
/// Only the ambient mode changes; the legacy migration planner writes the whole
/// block from the target state, and a single ambient-mode change leaves every
/// other field as the device reported it. The last byte is kept for the models
/// whose writer preserves it, and fixed for the model whose writer replaces an
/// unexposed byte.
pub fn sound_mode_write(
    mode: SoundMode,
    block: [u8; SOUND_MODE_BLOCK],
    layout: &DualLayout,
) -> Result<[u8; SOUND_MODE_BLOCK], ModelError> {
    if mode == SoundMode::AirplaneMode {
        return Err(ModelError::InvalidSoundModeWrite);
    }
    let mut body = block;
    body[0] = mode.wire();
    if let Some(tail) = layout.sound_mode_tail {
        body[SOUND_MODE_BLOCK - 1] = tail;
    }
    Ok(body)
}

fn sound_mode_block(
    body: &[u8],
    layout: &DualLayout,
) -> Result<[u8; SOUND_MODE_BLOCK], ModelError> {
    body.get(layout.sound_modes..layout.sound_modes + SOUND_MODE_BLOCK)
        .and_then(|block| block.try_into().ok())
        .ok_or(ModelError::IncompleteState)
}

/// Reads the first channel's curve. A band block that starts with the absent
/// marker reports no curve. The second channel is not part of the shared curve.
fn equalizer_setting(body: &[u8], layout: &DualLayout) -> Option<EqualizerSetting> {
    let preset: [u8; 2] = body
        .get(layout.equalizer_preset..layout.equalizer_preset + 2)?
        .try_into()
        .ok()?;
    let bands: [u8; EQUALIZER_BANDS] = body
        .get(layout.equalizer_bands..layout.equalizer_bands + EQUALIZER_BANDS)?
        .try_into()
        .ok()?;
    if bands[0] == u8::MAX {
        return None;
    }
    Some(EqualizerSetting {
        preset_id: u16::from_le_bytes(preset),
        volume_adjustments: bands.map(EqualizerSetting::band_gain),
    })
}

fn case_battery(body: &[u8], layout: &DualLayout) -> Result<Option<u8>, ModelError> {
    let raw = *body
        .get(layout.case_battery)
        .ok_or(ModelError::IncompleteState)?;
    battery_percent(raw, layout.case_battery_scale, layout.case_battery_offset)
}

/// Reads the auto power off pair. An unrecognized flag or a duration index
/// outside the model's table is not reported as a confirmed value.
fn auto_power_off(body: &[u8], layout: &DualLayout) -> Option<AutoPowerOff> {
    let enabled = match *body.get(layout.auto_power_off)? {
        0 => false,
        1 => true,
        _ => return None,
    };
    let duration = *body.get(layout.auto_power_off + 1)?;
    if duration >= AUTO_POWER_OFF_DURATION_COUNT {
        return None;
    }
    Some(AutoPowerOff { enabled, duration })
}

fn gaming_mode(body: &[u8], layout: &DualLayout) -> Option<bool> {
    match *body.get(layout.gaming_mode?)? {
        0 => Some(false),
        1 => Some(true),
        _ => None,
    }
}

fn tws_status(body: &[u8], layout: &DualLayout) -> TwsStatus {
    // The legacy parser defaults an unknown host value to the left earbud.
    TwsStatus {
        host_is_left: body[layout.tws_status] != 1,
        connected: body[layout.tws_status + 1] == 1,
    }
}

fn ambient_sound_mode_cycle(body: &[u8], layout: &DualLayout) -> AmbientSoundModeCycle {
    let byte = body[layout.ambient_cycle];
    if layout.tws_cycle {
        // The legacy TWS cycle keeps the connected modes in the low nibble.
        AmbientSoundModeCycle::Tws {
            enabled: SoundModeCycle::from_bits(byte & 0x0f),
            disconnected: SoundModeCycle::from_bits(byte >> 4),
        }
    } else {
        AmbientSoundModeCycle::Shared(SoundModeCycle::from_bits(byte))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LAYOUT: DualLayout = DualLayout {
        state_length: 40,
        confirmed_lengths: &[40],
        battery: 2,
        battery_scale: 5,
        tws_status: 0,
        firmware: 6,
        equalizer_preset: 32,
        equalizer_bands: 34,
        case_battery: 37,
        case_battery_scale: 5,
        case_battery_offset: 0,
        sound_modes: 0,
        ambient_cycle: 1,
        tws_cycle: false,
        auto_power_off: 38,
        gaming_mode: None,
        sound_mode_tail: None,
    };

    #[test]
    fn reads_the_cycle_bits_from_the_low_three_bits() {
        assert_eq!(
            SoundModeCycle::from_bits(0b101),
            SoundModeCycle {
                noise_canceling: true,
                transparency: false,
                normal: true,
            }
        );
        assert_eq!(SoundModeCycle::from_bits(0), SoundModeCycle::default());
    }

    #[test]
    fn the_sport_model_keeps_a_fixed_sound_mode_tail() {
        let layout = DualLayout {
            sound_mode_tail: Some(0xff),
            ..LAYOUT
        };
        assert_eq!(
            sound_mode_write(
                SoundMode::Transparency,
                [0x02, 0x30, 0, 0, 1, 0x36],
                &layout
            ),
            Ok([0x01, 0x30, 0, 0, 1, 0xff])
        );
        assert_eq!(
            sound_mode_write(
                SoundMode::AirplaneMode,
                [0x02, 0x30, 0, 0, 1, 0x36],
                &layout
            ),
            Err(ModelError::InvalidSoundModeWrite)
        );
    }

    #[test]
    fn the_space_model_keeps_every_other_sound_mode_byte() {
        assert_eq!(
            sound_mode_write(SoundMode::NoiseCanceling, [0x02, 0x30, 0, 0, 1, 3], &LAYOUT),
            Ok([0x00, 0x30, 0, 0, 1, 3])
        );
    }
}
