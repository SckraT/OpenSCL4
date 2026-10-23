use anyhow::{Context, anyhow};
use clap::ArgMatches;
use indexmap::IndexMap;
use macaddr::MacAddr6;
use openscq30_lib::{
    device::{OpenSCQ30Device, SettingWriteOutcome, WriteOutcome},
    settings::{self, CategoryId, Setting, SettingId},
};
use serde::Serialize;
use strum::VariantArray;
use tabled::{Table, Tabled};

use crate::{
    fmt::{CustomDisplaySetting, DisplayableValue},
    openscq30_session,
};

pub async fn handle(matches: &ArgMatches) -> anyhow::Result<()> {
    let session = openscq30_session().await?;
    let mac_address = matches
        .get_one::<MacAddr6>("mac-address")
        .unwrap()
        .to_owned();
    let device = session.connect(mac_address).await?;
    match matches.subcommand().unwrap() {
        ("list-settings", matches) => {
            handle_list_settings(matches, device.as_ref()).await?;
        }
        ("setting", matches) => {
            handle_setting(matches, device.as_ref()).await?;
        }
        _ => unreachable!(),
    }
    Ok(())
}

async fn handle_list_settings(
    matches: &ArgMatches,
    device: &dyn OpenSCQ30Device,
) -> anyhow::Result<()> {
    let json = matches.get_flag("json");
    let no_categories = matches.get_flag("no-categories");
    let no_extended_info = matches.get_flag("no-extended-info");

    if json {
        let settings_by_category = device.settings_by_category();
        match (no_categories, no_extended_info) {
            // all information
            (false, false) => {
                let settings = JsonCategory::from_openscq30_lib(settings_by_category);
                println!("{}", serde_json::to_string_pretty(&settings)?);
            }
            // no categories
            (true, false) => {
                let settings = settings_by_category
                    .into_iter()
                    .flat_map(|(_, settings)| {
                        settings
                            .into_iter()
                            .map(|(setting_id, setting)| (setting_id, JsonSetting::from(setting)))
                    })
                    .collect::<IndexMap<_, _>>();
                println!("{}", serde_json::to_string_pretty(&settings)?);
            }
            // no extended info
            (false, true) => {
                let setting_ids_by_category = device
                    .categories()
                    .into_iter()
                    .map(|category_id| JsonCategoryNoExtendedInfo {
                        category_id,
                        setting_ids: device.settings_in_category(&category_id),
                    })
                    .collect::<Vec<_>>();
                println!(
                    "{}",
                    serde_json::to_string_pretty(&setting_ids_by_category)?
                );
            }
            // no categories or extended info
            (true, true) => {
                let setting_ids = device
                    .categories()
                    .into_iter()
                    .flat_map(|category_id| device.settings_in_category(&category_id))
                    .collect::<Vec<_>>();
                println!("{}", serde_json::to_string_pretty(&setting_ids)?);
            }
        }
    } else {
        for category_id in device.categories() {
            if !no_categories {
                println!("-- {category_id} --");
            }
            for setting_id in device.settings_in_category(&category_id) {
                // Settings that are currently unavailable should not be listed. They may be temporarily unavailabe such
                // as when TWS is disconnected, but they could also be never available such as when a device's firmware
                // is too old for a particular feature.
                if let Some(setting) = device.setting(&setting_id) {
                    if no_extended_info {
                        println!("{setting_id}");
                    } else {
                        println!("{setting_id}: {}", CustomDisplaySetting(setting));
                    }
                }
            }
        }
    }

    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonCategoryNoExtendedInfo {
    category_id: CategoryId,
    setting_ids: Vec<SettingId>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonCategory {
    category_id: CategoryId,
    settings: Vec<JsonSettingWithId>,
}

impl JsonCategory {
    fn from_openscq30_lib(
        categories: IndexMap<CategoryId, IndexMap<SettingId, settings::Setting>>,
    ) -> Vec<Self> {
        categories
            .into_iter()
            .map(|(category_id, settings)| {
                let json_settings = settings
                    .into_iter()
                    .map(|(setting_id, setting)| JsonSettingWithId {
                        setting_id,
                        setting: setting.into(),
                    })
                    .collect::<Vec<_>>();
                Self {
                    category_id,
                    settings: json_settings,
                }
            })
            .collect()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonSettingWithId {
    setting_id: SettingId,
    #[serde(flatten)]
    setting: JsonSetting,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
enum JsonSetting {
    Toggle,
    I32Range { setting: settings::Range<i32> },
    Select { setting: settings::Select },
    OptionalSelect { setting: settings::Select },
    ModifiableSelect { setting: settings::Select },
    MultiSelect { setting: settings::Select },
    MultiSelectWithRemove { setting: settings::Select },
    Equalizer { setting: settings::Equalizer },
    HueColorPicker { hue: f32 },
    Information,
    ImportString,
    Action,
    TimeOfDay { hours: i32, minutes: i32 },
}

impl From<settings::Setting> for JsonSetting {
    fn from(setting: settings::Setting) -> Self {
        match setting {
            settings::Setting::Toggle { .. } => Self::Toggle,
            settings::Setting::I32Range { setting, .. } => Self::I32Range { setting },
            settings::Setting::Select { setting, .. } => Self::Select { setting },
            settings::Setting::OptionalSelect { setting, .. }
            | settings::Setting::PresetEqualizerProfileSelect {
                select: setting, ..
            } => Self::OptionalSelect { setting },
            settings::Setting::ModifiableSelect { setting, .. } => {
                Self::ModifiableSelect { setting }
            }
            settings::Setting::MultiSelect { setting, .. } => Self::MultiSelect { setting },
            settings::Setting::MultiSelectWithRemove { setting, .. } => {
                Self::MultiSelectWithRemove { setting }
            }
            settings::Setting::Equalizer { setting, .. } => Self::Equalizer { setting },
            settings::Setting::HueColorPicker { hue } => Self::HueColorPicker { hue },
            settings::Setting::Information { .. } => Self::Information,
            settings::Setting::ImportString { .. } => Self::ImportString,
            settings::Setting::Action => Self::Action,
            settings::Setting::TimeOfDay {
                minutes_after_midnight,
            } => Self::TimeOfDay {
                hours: minutes_after_midnight / 60,
                minutes: minutes_after_midnight % 60,
            },
        }
    }
}

async fn handle_setting(matches: &ArgMatches, device: &dyn OpenSCQ30Device) -> anyhow::Result<()> {
    let json = matches.get_flag("json");

    let commands = collect_commands(matches)?;

    // Whether this fails at any point or not, we still want to print the table, so make sure to
    // do that before returning the error.
    let mut command_results = Vec::new();
    let mut write_outcomes = Vec::new();
    let result =
        execute_commands(device, commands, &mut command_results, &mut write_outcomes).await;

    if json {
        let table_items = command_results
            .into_iter()
            .map(|(setting_id, setting)| SettingIdValue {
                setting_id,
                value: setting.into(),
            })
            .collect::<Vec<_>>();
        println!("{}", serde_json::to_string_pretty(&table_items)?);
        // Keep stdout valid json; the outcomes are still shown, but on stderr.
        for outcome in &write_outcomes {
            eprintln!("{}", format_write_outcome(outcome));
        }
    } else {
        for outcome in &write_outcomes {
            println!("{}", format_write_outcome(outcome));
        }
        if !command_results.is_empty() {
            let mut table =
                Table::new(command_results.into_iter().map(|(setting_id, setting)| {
                    SettingIdValueTableItem::new(setting_id, setting)
                }));
            crate::fmt::apply_tabled_settings(&mut table);
            println!("{table}");
        } else if result.is_ok() && write_outcomes.is_empty() {
            println!("OK");
        }
    }

    result?;
    check_all_applied(&write_outcomes)
}

fn check_all_applied(outcomes: &[SettingWriteOutcome]) -> anyhow::Result<()> {
    if let Some(outcome) = outcomes
        .iter()
        .find(|outcome| !matches!(outcome.outcome, WriteOutcome::Applied { .. }))
    {
        anyhow::bail!("{}", format_write_outcome(outcome));
    }
    Ok(())
}

fn format_write_outcome(outcome: &SettingWriteOutcome) -> String {
    let result = match &outcome.outcome {
        WriteOutcome::Applied { observed } => format!("applied ({observed:?})"),
        WriteOutcome::Rejected {
            observed: Some(observed),
        } => {
            format!("rejected (observed: {observed:?})")
        }
        WriteOutcome::Rejected { observed: None } => "rejected".to_string(),
        WriteOutcome::UnsupportedInState { reason } => {
            format!("unsupported in the current state ({reason:?})")
        }
        WriteOutcome::TimedOut => "timed out".to_string(),
        WriteOutcome::Disconnected => "disconnected".to_string(),
    };
    format!("{}: {result}", outcome.setting_id)
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct ExecCommandWithIndex {
    index: usize,
    command: ExecCommand,
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
enum ExecCommand {
    Get(SettingId),
    Set(SettingId, Option<String>),
}

fn collect_commands(matches: &ArgMatches) -> anyhow::Result<Vec<ExecCommand>> {
    let get_iter = matches
        .indices_of("get")
        .unwrap_or_default()
        .zip(matches.get_many::<String>("get").unwrap_or_default())
        .map(|(index, setting_id)| {
            Ok(ExecCommandWithIndex {
                index,
                command: ExecCommand::Get(setting_id_from_str(setting_id)?),
            })
        });

    let set_iter = matches
        .indices_of("set")
        .unwrap_or_default()
        .zip(matches.get_many::<String>("set").unwrap_or_default())
        .map(|(index, setting_id_to_value)| {
            let (setting_id, unparsed_value) = setting_id_to_value.split_once("=").map_or(
                (setting_id_to_value.as_str(), None),
                |(setting_id, unparsed_value)| (setting_id, Some(unparsed_value)),
            );
            Ok(ExecCommandWithIndex {
                index,
                command: ExecCommand::Set(
                    setting_id_from_str(setting_id)?,
                    unparsed_value.map(ToString::to_string),
                ),
            })
        });

    let mut commands = get_iter
        .chain(set_iter)
        .collect::<anyhow::Result<Vec<_>>>()?;
    commands.sort();
    Ok(commands
        .into_iter()
        .map(|with_index| with_index.command)
        .collect())
}

fn setting_id_from_str(setting_id: &str) -> anyhow::Result<SettingId> {
    Ok(*SettingId::VARIANTS
        .iter()
        .find(|variant| <&'static str>::from(*variant).eq_ignore_ascii_case(setting_id))
        .ok_or_else(|| anyhow!("setting id {setting_id} does not exist"))?)
}

async fn execute_commands(
    device: &dyn OpenSCQ30Device,
    commands: Vec<ExecCommand>,
    table_items: &mut Vec<(SettingId, settings::Setting)>,
    write_outcomes: &mut Vec<SettingWriteOutcome>,
) -> anyhow::Result<()> {
    for command in commands {
        match command {
            ExecCommand::Get(setting_id) => {
                let setting = device.setting(&setting_id).ok_or(anyhow!(
                    "{} does not use setting id {setting_id}.",
                    device.model(),
                ))?;
                table_items.push((setting_id, setting));
            }
            ExecCommand::Set(setting_id, unparsed_value) => {
                let setting = device.setting(&setting_id).ok_or(anyhow!(
                    "{} does not use setting id {setting_id}.",
                    device.model(),
                ))?;
                let outcomes = device
                    .set_setting_values(vec![(
                        setting_id,
                        crate::parse::setting_value(&setting, unparsed_value)
                            .context(setting_id)?,
                    )])
                    .await?;
                write_outcomes.extend(outcomes);
            }
        }
    }
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SettingIdValue {
    setting_id: SettingId,
    value: settings::Value,
}

#[derive(Tabled)]
struct SettingIdValueTableItem {
    #[tabled(rename = "Setting ID")]
    setting_id: SettingId,
    #[tabled(rename = "Value")]
    value: DisplayableValue,
}

impl SettingIdValueTableItem {
    pub fn new(setting_id: SettingId, setting: settings::Setting) -> Self {
        let displayable_value = match setting {
            Setting::TimeOfDay {
                minutes_after_midnight,
            } => DisplayableValue::MinutesAfterMidnight(minutes_after_midnight),
            _ => DisplayableValue::Passthrough(setting.into()),
        };
        Self {
            setting_id,
            value: displayable_value,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openscq30_lib::settings::Value;

    #[test]
    fn a_rejected_setting_fails_the_command() {
        let outcomes = vec![SettingWriteOutcome {
            setting_id: SettingId::WearingDetection,
            outcome: WriteOutcome::Rejected {
                observed: Some(Value::Bool(false)),
            },
        }];

        let error = check_all_applied(&outcomes).unwrap_err();

        assert!(error.to_string().contains("wearingDetection"));
        assert!(error.to_string().contains("rejected"));
    }

    #[test]
    fn a_timed_out_setting_fails_the_command() {
        let outcomes = vec![SettingWriteOutcome {
            setting_id: SettingId::Ldac,
            outcome: WriteOutcome::TimedOut,
        }];

        let error = check_all_applied(&outcomes).unwrap_err();

        assert!(error.to_string().contains("ldac"));
        assert!(error.to_string().contains("timed out"));
    }

    #[test]
    fn applied_settings_succeed() {
        let outcomes = vec![SettingWriteOutcome {
            setting_id: SettingId::Ldac,
            outcome: WriteOutcome::Applied {
                observed: Value::Bool(true),
            },
        }];

        assert!(check_all_applied(&outcomes).is_ok());
    }
}
