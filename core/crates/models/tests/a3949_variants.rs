//! The four A3949 state variants recorded by the legacy driver tests.
//!
//! These are the example frames in `lib/src/devices/soundcore/a3949.rs`. They
//! document the firmware and TWS differences between the public variant names
//! while keeping the same 88-byte measured layout. No device identifiers are
//! present: the serials were masked in the legacy fixtures.

#![forbid(unsafe_code)]

#[cfg(test)]
mod tests {
    use openscl4_models::{
        A3949_DISABLED_ACTION, A3949_GAMING_MODE, A3949_TOUCH_TONE, A3949Button, HostDevice,
        ModelDriver, registry,
    };

    const EXAMPLE: &[u8] = &[
        0x00, 0x01, 0x03, 0x04, 0x00, 0x00, 0x31, 0x34, 0x2e, 0x34, 0x33, 0x31, 0x34, 0x2e, 0x34,
        0x33, 0x33, 0x39, 0x34, 0x39, 0x42, 0x39, 0x41, 0x45, 0x43, 0x37, 0x32, 0x43, 0x39, 0x43,
        0x31, 0x38, 0x00, 0x00, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x0e, 0x01, 0x00, 0x01, 0x11, 0x01,
        0x22, 0x01, 0x33, 0x01, 0x55, 0x01, 0x66, 0xff, 0xff, 0xff, 0x61, 0x00, 0x01, 0xff, 0xff,
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    ];

    const P20I: &[u8] = &[
        0x01, 0x00, 0xff, 0x04, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x31, 0x34, 0x2e, 0x34,
        0x33, 0x33, 0x39, 0x34, 0x39, 0x42, 0x39, 0x41, 0x45, 0x43, 0x37, 0x32, 0x43, 0x39, 0x43,
        0x31, 0x38, 0x00, 0x00, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x0e, 0x01, 0xff, 0x01, 0xff, 0x01,
        0xf5, 0x01, 0x66, 0x01, 0xf2, 0x01, 0x33, 0xff, 0xff, 0xff, 0x61, 0x00, 0x01, 0xff, 0xff,
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    ];

    const R50I: &[u8] = &[
        0x00, 0x01, 0x03, 0x03, 0x00, 0x00, 0x31, 0x39, 0x2e, 0x34, 0x33, 0x31, 0x39, 0x2e, 0x34,
        0x33, 0x33, 0x39, 0x34, 0x39, 0x33, 0x46, 0x32, 0x34, 0x39, 0x45, 0x32, 0x44, 0x42, 0x36,
        0x46, 0x34, 0x00, 0x00, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x0e, 0x01, 0xf6, 0x01, 0xf6, 0x01,
        0x62, 0x01, 0x63, 0x01, 0x33, 0x01, 0x33, 0xff, 0xff, 0xff, 0x31, 0x00, 0x00, 0xff, 0xff,
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    ];

    const P25I: &[u8] = &[
        0x00, 0x01, 0x05, 0x05, 0x00, 0x00, 0x30, 0x38, 0x2e, 0x33, 0x30, 0x30, 0x38, 0x2e, 0x33,
        0x30, 0x33, 0x39, 0x34, 0x39, 0x46, 0x33, 0x46, 0x46, 0x42, 0x32, 0x36, 0x36, 0x41, 0x45,
        0x46, 0x30, 0x0e, 0x00, 0x78, 0x96, 0x96, 0x8c, 0xa0, 0xaa, 0x96, 0xa0, 0x78, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x0e, 0x01, 0xff, 0x01, 0xff, 0x01,
        0x65, 0x01, 0x66, 0x01, 0x32, 0x01, 0x33, 0xff, 0xff, 0xff, 0x31, 0x00, 0x01, 0xff, 0xff,
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    ];

    const VARIANTS: [(&str, &[u8], &str, &str); 4] = [
        ("example", EXAMPLE, "14.43", "14.43"),
        ("p20i", P20I, "", "14.43"),
        ("r50i", R50I, "19.43", "19.43"),
        ("p25i", P25I, "08.30", "08.30"),
    ];

    #[test]
    fn every_variant_identifies_as_a3949_and_keeps_the_measured_layout() {
        for (name, body, _, _) in VARIANTS {
            assert_eq!(body.len(), 88, "{name}");
            assert_eq!(
                registry::identify_state(body),
                Ok(ModelDriver::A3949),
                "{name}"
            );
            assert!(
                ModelDriver::A3949.decode_confirmed_state(body).is_some(),
                "{name}"
            );
        }
    }

    #[test]
    fn variants_record_the_confirmed_firmware_and_tws_differences() {
        for (name, body, left, right) in VARIANTS {
            let state = ModelDriver::A3949
                .decode_confirmed_state(body)
                .expect("measured variant");
            let left = (!left.is_empty()).then(|| left.to_owned());
            assert_eq!(
                state.firmware_left.map(|firmware| firmware.to_string()),
                left,
                "{name}"
            );
            assert_eq!(
                state.firmware_right.map(|firmware| firmware.to_string()),
                Some(right.to_owned()),
                "{name}"
            );
            assert_eq!(state.gaming_mode, Some(false), "{name}");
        }

        let p20i = ModelDriver::A3949.decode_state(P20I).expect("P20i state");
        assert_eq!(p20i.tws.map(|tws| tws.host_device), Some(HostDevice::Right));
        assert_eq!(p20i.tws.map(|tws| tws.connected), Some(false));

        let r50i = ModelDriver::A3949.decode_state(R50I).expect("R50i state");
        assert_eq!(r50i.tws.map(|tws| tws.host_device), Some(HostDevice::Left));
        assert_eq!(r50i.tws.map(|tws| tws.connected), Some(true));
        assert_eq!(r50i.touch_tone, Some(false));
    }

    #[test]
    fn touch_tone_and_gaming_flags_differ_between_variants() {
        let example = ModelDriver::A3949.decode_state(EXAMPLE).expect("example");
        let p20i = ModelDriver::A3949.decode_state(P20I).expect("P20i");
        let r50i = ModelDriver::A3949.decode_state(R50I).expect("R50i");
        let p25i = ModelDriver::A3949.decode_state(P25I).expect("P25i");

        assert_eq!(example.touch_tone, Some(true));
        assert_eq!(p20i.touch_tone, Some(true));
        assert_eq!(r50i.touch_tone, Some(false));
        assert_eq!(p25i.touch_tone, Some(true));
        for body in [EXAMPLE, P20I, R50I, P25I] {
            assert_eq!(
                body[A3949_TOUCH_TONE] != 0,
                ModelDriver::A3949
                    .decode_state(body)
                    .expect("variant")
                    .touch_tone
                    .expect("flag")
            );
            assert_eq!(body[A3949_GAMING_MODE], 0);
        }
    }

    #[test]
    fn p25i_reports_the_piano_curve_and_a_disabled_left_single_press() {
        let state = ModelDriver::A3949.decode_state(P25I).expect("P25i");
        let equalizer = state.equalizer.expect("reported curve");
        assert_eq!(equalizer.preset_id, 0x000e);
        assert_eq!(
            equalizer.volume_adjustments,
            [0, 30, 30, 20, 40, 50, 30, 40, 0, -120]
        );
        let tws = state.tws.expect("TWS status");
        assert_eq!(
            state
                .buttons
                .expect("buttons")
                .status(A3949Button::LeftSinglePress)
                .action(tws),
            A3949_DISABLED_ACTION
        );
        assert_eq!(
            state
                .buttons
                .expect("buttons")
                .status(A3949Button::LeftDoublePress)
                .action(tws),
            5
        );
    }

    #[test]
    fn example_variant_records_the_legacy_button_actions() {
        let state = ModelDriver::A3949.decode_state(EXAMPLE).expect("example");
        let tws = state.tws.expect("TWS status");
        let buttons = state.buttons.expect("buttons");
        for (button, action) in [
            (A3949Button::LeftSinglePress, 0),
            (A3949Button::RightSinglePress, 1),
            (A3949Button::LeftDoublePress, 2),
            (A3949Button::RightDoublePress, 3),
            (A3949Button::LeftLongPress, 5),
            (A3949Button::RightLongPress, 6),
        ] {
            assert_eq!(buttons.status(button).action(tws), action, "{button:?}");
        }
    }
}
