//! Turns what the app knows into what the window shows.

use macaddr::MacAddr6;
use openscq30_i18n::Translate;
use openscq30_lib::{DeviceModel, storage::PairedDevice};
use strum::VariantArray;

use crate::{
    DeviceCard, ModelOption, Page, art,
    controller::DeviceSnapshot,
    model::{
        device_art::{self, DeviceArt, DeviceKind, EarbudsColor},
        devices::split_brand,
        groups::{SettingGroup, group_settings},
    },
};

/// "SoundcoreA3954" as "A3954".
pub fn model_code(model: DeviceModel) -> String {
    let name: &'static str = model.into();
    name.trim_start_matches("Soundcore").to_string()
}

/// The model's name without the brand: "Liberty 4 Pro".
pub fn model_name(model: DeviceModel) -> String {
    let full = model.translate();
    split_brand(&full).1.to_string()
}

pub fn model_art(model: DeviceModel) -> Option<DeviceArt> {
    device_art::device_art(model.into())
}

pub fn art_image(model: DeviceModel, color: EarbudsColor) -> slint::Image {
    model_art(model).map_or_else(
        || art::image(DeviceArt::Headphones, color),
        |art| art::image(art, color),
    )
}

pub fn is_speaker(model: DeviceModel) -> bool {
    model_art(model).is_some_and(|art| art.kind() == DeviceKind::Speaker)
}

pub fn device_cards(
    paired: &[PairedDevice],
    current: Option<MacAddr6>,
    color: EarbudsColor,
) -> Vec<DeviceCard> {
    paired
        .iter()
        .map(|device| DeviceCard {
            mac: device.mac_address.to_string().into(),
            name: model_name(device.model).into(),
            code: model_code(device.model).into(),
            art: art_image(device.model, color),
            current: Some(device.mac_address) == current,
            demo: device.is_demo,
        })
        .collect()
}

/// The models one can add, filtered by a name or code and by kind (0 all, then earbuds, over-ear, open-ear,
/// speakers), in the library's order.
pub fn model_options(query: &str, kind: i32, color: EarbudsColor) -> Vec<ModelOption> {
    let query = query.trim().to_lowercase();
    let wanted = match kind {
        1 => Some(DeviceKind::Earbuds),
        2 => Some(DeviceKind::OverEar),
        3 => Some(DeviceKind::OpenEar),
        4 => Some(DeviceKind::Speaker),
        _ => None,
    };
    DeviceModel::VARIANTS
        .iter()
        .copied()
        .filter(|model| *model != DeviceModel::SoundcoreDevelopment)
        .filter(|model| wanted.is_none() || model_art(*model).map(DeviceArt::kind) == wanted)
        .map(|model| (model, model_name(model), model_code(model)))
        .filter(|(_, name, code)| {
            query.is_empty()
                || name.to_lowercase().contains(&query)
                || code.to_lowercase().contains(&query)
        })
        .map(|(model, name, code)| ModelOption {
            code: code.into(),
            name: name.into(),
            art: art_image(model, color),
        })
        .collect()
}

pub fn find_model(code: &str) -> Option<DeviceModel> {
    DeviceModel::VARIANTS
        .iter()
        .copied()
        .find(|model| model_code(*model) == code)
}

pub fn page_of(group: SettingGroup) -> Page {
    match group {
        SettingGroup::SoundModes => Page::SoundModes,
        SettingGroup::Equalizer => Page::Equalizer,
        SettingGroup::SoundEffects => Page::Effects,
        SettingGroup::Controls => Page::Controls,
        SettingGroup::Comfort => Page::Comfort,
        SettingGroup::Power => Page::Power,
        SettingGroup::Connections => Page::Connections,
        SettingGroup::Case => Page::Case,
        SettingGroup::Sleep => Page::Sleep,
        SettingGroup::Lights => Page::Lights,
        SettingGroup::Alarms => Page::Alarms,
        SettingGroup::About => Page::About,
        SettingGroup::Other => Page::Other,
    }
}

/// The device's settings by group, each with the category it was found in.
pub fn groups(snapshot: &DeviceSnapshot) -> Vec<(SettingGroup, Vec<&'static str>)> {
    // categories are named by Display, as "deviceInformation"
    let settings: Vec<(&'static str, String)> = snapshot
        .categories
        .iter()
        .flat_map(|(category, ids)| {
            let category = category.to_string();
            ids.iter()
                .map(move |id| (<&'static str>::from(id), category.clone()))
        })
        .collect();
    let borrowed: Vec<(&'static str, &str)> = settings
        .iter()
        .map(|(id, category)| (*id, category.as_str()))
        .collect();
    group_settings(&borrowed)
}

/// The device's pages under "Sound" and under "Device" in the sidebar.
pub fn pages(snapshot: &DeviceSnapshot, one_earbud_only: bool) -> (Vec<Page>, Vec<Page>) {
    let mut present: Vec<SettingGroup> = groups(snapshot)
        .into_iter()
        .map(|(group, _)| group)
        .collect();
    // with one earbud the equalizer offers nothing, but its page says why and still imports and exports profiles
    let has_equalizer_category = snapshot
        .categories
        .iter()
        .any(|(category, _)| category.to_string() == "equalizer");
    if one_earbud_only && has_equalizer_category && !present.contains(&SettingGroup::Equalizer) {
        present.push(SettingGroup::Equalizer);
    }
    let sound_groups = [
        SettingGroup::SoundModes,
        SettingGroup::Equalizer,
        SettingGroup::SoundEffects,
    ];
    let mut sound: Vec<Page> = sound_groups
        .iter()
        .filter(|g| present.contains(g))
        .map(|g| page_of(*g))
        .collect();
    sound.push(Page::Presets);
    let device = SettingGroup::ALL
        .iter()
        .filter(|g| !sound_groups.contains(g) && present.contains(g))
        .map(|g| page_of(*g))
        .collect();
    (sound, device)
}
