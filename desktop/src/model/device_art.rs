pub use openscq30_lib::client::device_art::{DeviceArt, DeviceKind, device_art};

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "camelCase")]
pub enum EarbudsColor {
    #[default]
    Silver,
    Black,
}

/// Follow the desktop theme when the user did not choose a color.
pub fn art_color(dark_theme: bool, chosen: Option<EarbudsColor>) -> EarbudsColor {
    chosen.unwrap_or(if dark_theme {
        EarbudsColor::Black
    } else {
        EarbudsColor::Silver
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_theme_picks_the_color_unless_one_is_chosen() {
        assert_eq!(EarbudsColor::Silver, art_color(false, None));
        assert_eq!(EarbudsColor::Black, art_color(true, None));
        assert_eq!(
            EarbudsColor::Black,
            art_color(false, Some(EarbudsColor::Black))
        );
        assert_eq!(
            EarbudsColor::Silver,
            art_color(true, Some(EarbudsColor::Silver))
        );
    }
}
