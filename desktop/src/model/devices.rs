//! Saved devices, as android/app/src/main/java/com/oppzippy/openscq30/ui/deviceselection/ActiveDevice.kt.

use macaddr::MacAddr6;
use openscq30_lib::storage::PairedDevice;

/// The device at the top of the list: the one the app is connected or connecting to, otherwise the one the user
/// opened last, otherwise the first saved device.
pub fn featured_device(
    devices: &[PairedDevice],
    active: Option<MacAddr6>,
    last: Option<MacAddr6>,
) -> Option<&PairedDevice> {
    let find = |mac: Option<MacAddr6>| {
        let mac = mac?;
        devices.iter().find(|d| d.mac_address == mac)
    };
    find(active)
        .or_else(|| find(last))
        .or_else(|| devices.first())
}

/// Model names start with the brand, which the app shows separately: "Soundcore Liberty 4 Pro".
pub fn split_brand(model_name: &str) -> (Option<&str>, &str) {
    match model_name.strip_prefix("Soundcore ") {
        Some(name) if !name.is_empty() => (Some("Soundcore"), name),
        _ => (None, model_name),
    }
}

#[cfg(test)]
mod tests {
    use openscq30_lib::DeviceModel;

    use super::*;

    // made-up addresses
    const LIBERTY: MacAddr6 = MacAddr6::new(0, 0, 0, 0, 0, 1);
    const Q30: MacAddr6 = MacAddr6::new(0, 0, 0, 0, 0, 2);

    fn devices() -> Vec<PairedDevice> {
        vec![
            PairedDevice {
                mac_address: Q30,
                model: DeviceModel::SoundcoreA3028,
                is_demo: false,
            },
            PairedDevice {
                mac_address: LIBERTY,
                model: DeviceModel::SoundcoreA3954,
                is_demo: false,
            },
        ]
    }

    #[test]
    fn the_connected_device_is_featured_even_if_another_was_used_last() {
        assert_eq!(
            Some(LIBERTY),
            featured_device(&devices(), Some(LIBERTY), Some(Q30)).map(|d| d.mac_address)
        );
    }

    #[test]
    fn the_last_used_device_is_featured_when_nothing_is_connected() {
        assert_eq!(
            Some(LIBERTY),
            featured_device(&devices(), None, Some(LIBERTY)).map(|d| d.mac_address)
        );
    }

    #[test]
    fn the_first_device_is_featured_without_history() {
        assert_eq!(
            Some(Q30),
            featured_device(&devices(), None, None).map(|d| d.mac_address)
        );
        assert_eq!(
            Some(Q30),
            featured_device(&devices(), None, Some(MacAddr6::nil())).map(|d| d.mac_address)
        );
    }

    #[test]
    fn nothing_is_featured_without_devices() {
        assert_eq!(None, featured_device(&[], Some(LIBERTY), Some(LIBERTY)));
    }

    #[test]
    fn the_brand_is_split_from_the_model_name() {
        assert_eq!(
            (Some("Soundcore"), "Liberty 4 Pro"),
            split_brand("Soundcore Liberty 4 Pro")
        );
        assert_eq!(
            (None, "Development Device"),
            split_brand("Development Device")
        );
        assert_eq!((None, "Soundcore"), split_brand("Soundcore"));
    }
}
