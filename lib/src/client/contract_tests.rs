use std::str::FromStr;

use serde_json::Value as Json;

use super::{
    SettingMap, availability, battery, controls, equalizer, groups, presets, quick_toggles,
    sound_mode,
};
use crate::{
    settings::{Equalizer, Select, Setting, SettingId, Value},
    storage::{QuickPreset, QuickPresetField},
};

#[test]
fn shared_client_contract() {
    let fixtures: Json =
        serde_json::from_str(include_str!("../../tests/fixtures/client_rules.json")).unwrap();
    for case in fixtures["battery"].as_array().unwrap() {
        let actual = battery::battery_progress(case["raw"].as_str().unwrap());
        match case["progress"].as_f64() {
            Some(expected) => assert!((f64::from(actual.unwrap()) - expected).abs() < 0.0001),
            None => assert_eq!(None, actual),
        }
    }
    for case in fixtures["groups"].as_array().unwrap() {
        assert_eq!(
            case["group"].as_str().unwrap(),
            format!(
                "{:?}",
                groups::setting_group(case["id"].as_str().unwrap(), case["category"].as_str())
            )
        );
    }
    for scenario in fixtures["availability"].as_array().unwrap() {
        let mut settings = SettingMap::new();
        if let Some(tws_status) = scenario["twsStatus"].as_str() {
            settings.insert(
                SettingId::TwsStatus,
                Setting::Information {
                    value: tws_status.to_string(),
                    translated_value: tws_status.to_string(),
                },
            );
        }
        if scenario["dualConnections"].as_bool().unwrap_or(false) {
            settings.insert(SettingId::DualConnections, Setting::Toggle { value: true });
        }
        for case in scenario["cases"].as_array().unwrap() {
            let id = case["id"].as_str().unwrap();
            let reason = match case["reason"].as_str() {
                Some("RequiresBothEarbuds") => {
                    availability::AvailabilityReason::RequiresBothEarbuds
                }
                Some("NoReadBack") => availability::AvailabilityReason::NoReadBack,
                None => {
                    assert_eq!("Available", case["status"].as_str().unwrap());
                    assert_eq!(
                        availability::SettingAvailability::Available,
                        availability::setting_availability(&settings, id),
                        "{id}"
                    );
                    continue;
                }
                Some(other) => panic!("unknown reason {other}"),
            };
            let expected = match case["status"].as_str().unwrap() {
                "ReadOnly" => availability::SettingAvailability::ReadOnly { reason },
                "Unavailable" => availability::SettingAvailability::Unavailable { reason },
                other => panic!("unknown availability {other}"),
            };
            assert_eq!(
                expected,
                availability::setting_availability(&settings, id),
                "{id} with twsStatus {:?}",
                scenario["twsStatus"],
            );
        }
    }
    for scenario in fixtures["settingControls"].as_array().unwrap() {
        let settings = scenario["settings"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(id, value)| {
                let setting = if let Some(value) = value.as_bool() {
                    Setting::Toggle { value }
                } else {
                    let value = value.as_str().unwrap().to_owned();
                    Setting::Select {
                        setting: Select {
                            options: vec![value.clone().into()],
                            localized_options: vec![value.clone()],
                        },
                        value: value.into(),
                    }
                };
                (SettingId::from_str(id).unwrap(), setting)
            })
            .collect();
        for (id, expected) in scenario["cases"].as_object().unwrap() {
            assert_eq!(
                expected.as_str().unwrap(),
                format!("{:?}", controls::setting_control(&settings, id)),
                "{id} with settings {}",
                scenario["settings"],
            );
        }
    }
    for case in fixtures["isTws"].as_array().unwrap() {
        let mut settings = SettingMap::new();
        if let Some(tws_status) = case["twsStatus"].as_str() {
            settings.insert(
                SettingId::TwsStatus,
                Setting::Information {
                    value: tws_status.to_string(),
                    translated_value: tws_status.to_string(),
                },
            );
        }
        assert_eq!(
            case["tws"].as_bool().unwrap(),
            battery::is_tws(&settings),
            "{case}"
        );
    }
    for case in fixtures["scale"].as_array().unwrap() {
        let values =
            sound_mode::sound_mode_scale_settings(case["position"].as_i64().unwrap() as i32);
        assert_eq!(
            Value::String(case["mode"].as_str().unwrap().to_string().into()),
            values[0].1
        );
        if let Some(id) = case["strengthId"].as_str() {
            assert_eq!(id, <&str>::from(values[1].0));
            assert_eq!(
                Value::I32(case["strength"].as_i64().unwrap() as i32),
                values[1].1
            );
        } else {
            assert_eq!(1, values.len());
        }
    }
    let eq = Equalizer {
        band_hz: vec![100].into(),
        fraction_digits: 1,
        min: -120,
        max: 134,
    };
    for case in fixtures["equalizer"].as_array().unwrap() {
        let profile = equalizer::TransferProfile {
            name: "p".into(),
            volume_adjustments: vec![case["value"].as_f64().unwrap()],
        };
        assert_eq!(
            case["status"].as_str().unwrap(),
            format!(
                "{:?}",
                equalizer::preview_import(&[profile], &[], Some(&eq))[0]
            )
        );
    }
    for case in fixtures["equalizerInputs"].as_array().unwrap() {
        let eq = Equalizer {
            band_hz: vec![100].into(),
            fraction_digits: case["fractionDigits"].as_i64().unwrap() as i16,
            min: case["min"].as_i64().unwrap() as i16,
            max: case["max"].as_i64().unwrap() as i16,
        };
        assert_eq!(
            case["value"].as_i64().map(|value| value as i16),
            equalizer::parse_equalizer_value(case["text"].as_str().unwrap(), &eq),
            "{case}",
        );
    }
    for case in fixtures["equalizerFormats"].as_array().unwrap() {
        assert_eq!(
            case["text"].as_str().unwrap(),
            equalizer::format_equalizer_value(
                case["value"].as_i64().unwrap() as i16,
                case["fractionDigits"].as_i64().unwrap() as i16,
            ),
        );
    }
    for case in fixtures["equalizerFrequencies"].as_array().unwrap() {
        assert_eq!(
            (
                case["value"].as_str().unwrap().to_string(),
                case["khz"].as_bool().unwrap()
            ),
            equalizer::frequency(case["hz"].as_u64().unwrap() as u16),
        );
    }
    for case in fixtures["quickToggles"].as_array().unwrap() {
        let stored: Option<Vec<String>> = serde_json::from_value(case["stored"].clone()).unwrap();
        let available = case["available"]
            .as_array()
            .unwrap()
            .iter()
            .map(|id| id.as_str().unwrap())
            .collect::<Vec<_>>();
        let expected: Vec<String> = serde_json::from_value(case["expected"].clone()).unwrap();
        assert_eq!(
            expected,
            quick_toggles::quick_toggle_ids(stored.as_deref(), &available)
        );
    }
    for case in fixtures["presets"].as_array().unwrap() {
        let preset = QuickPreset {
            name: "p".into(),
            fields: case["enabled"]
                .as_array()
                .unwrap()
                .iter()
                .zip(case["values"].as_array().unwrap())
                .map(|(enabled, value)| QuickPresetField {
                    setting_id: SettingId::SpatialAudio,
                    is_enabled: enabled.as_bool().unwrap(),
                    value: Value::Bool(value.as_bool().unwrap()),
                })
                .collect(),
        };
        let values = SettingMap::from([(
            SettingId::SpatialAudio,
            Setting::Toggle {
                value: case["current"].as_bool().unwrap(),
            },
        )]);
        assert_eq!(
            case["status"].as_str().unwrap(),
            format!(
                "{:?}",
                presets::preset_status(&preset, &values, case["lastApplied"].as_str())
            )
        );
    }
    let mode_values = |case: &serde_json::Value| -> SettingMap {
        ["ambientSoundMode", "noiseCancelingMode", "transparencyMode"]
            .into_iter()
            .filter_map(|id| {
                let value = case[id].as_str()?.to_owned();
                Some((
                    SettingId::from_str(id).unwrap(),
                    Setting::Select {
                        setting: crate::settings::Select {
                            options: vec![value.clone().into()],
                            localized_options: vec![value.clone()],
                        },
                        value: value.into(),
                    },
                ))
            })
            .collect()
    };
    for case in fixtures["adaptiveLevel"].as_array().unwrap() {
        let mut settings = mode_values(case);
        settings.insert(
            SettingId::AdaptiveNoiseCanceling,
            Setting::Information {
                value: "1/5".to_owned(),
                translated_value: "1/5".to_owned(),
            },
        );
        assert_eq!(
            case["shown"].as_bool().unwrap(),
            sound_mode::adaptive_noise_canceling_display(&settings).is_some(),
            "{case}"
        );
    }
    for case in fixtures["soundModeRows"].as_array().unwrap() {
        let settings = mode_values(case);
        let scale = case["scale"].as_bool().unwrap_or_default();
        let ids = [
            "ambientSoundMode",
            "noiseCancelingMode",
            "customNoiseCanceling",
            "manualNoiseCanceling",
            "transparencyMode",
            "manualTransparency",
            "airplaneMode",
        ];
        let shown: Vec<_> = ids
            .into_iter()
            .filter(|id| sound_mode::mode_setting_is_visible(&settings, id, scale))
            .collect();
        let expected: Vec<_> = case["shown"]
            .as_array()
            .unwrap()
            .iter()
            .map(|id| id.as_str().unwrap())
            .collect();
        assert_eq!(expected, shown, "{case}");
    }
    assert_every_mode_name_is_exercised(&fixtures["soundModeRows"]);
}

/// A rule keyed on a setting value is only as good as the names the examples put in front of it: the Space One Pro
/// calls its user-set mode `Custom`, another device calls it `Manual`, and a rule written against one name silently
/// missed the other. Every name a rule may read therefore has to appear in the shared examples, which both clients
/// check against.
fn assert_every_mode_name_is_exercised(cases: &serde_json::Value) {
    let cases = cases.as_array().unwrap();
    let exercised =
        |key: &str, name: &str| cases.iter().any(|case| case[key].as_str() == Some(name));
    for name in sound_mode::ambient_mode::ALL {
        assert!(
            exercised("ambientSoundMode", name),
            "no shared example uses the ambient mode {name}"
        );
    }
    for name in sound_mode::noise_canceling_mode::ALL {
        assert!(
            exercised("noiseCancelingMode", name),
            "no shared example uses the noise canceling mode {name}"
        );
    }
}
