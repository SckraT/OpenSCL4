use std::str::FromStr;

use i18n_embed::unic_langid::subtags;
use openscq30_lib::{client, connection, device, settings, storage};

pub struct MacAddr6(pub macaddr::MacAddr6);
uniffi::custom_type!(MacAddr6, String, {
    try_lift: |val| Ok(MacAddr6(macaddr::MacAddr6::from_str(&val)?)),
    lower: |val| val.0.to_string(),
});

pub struct Uuid(pub uuid::Uuid);
uniffi::custom_type!(Uuid, String, {
    try_lift: |val| Ok(Uuid(uuid::Uuid::from_str(&val)?)),
    lower: |val| val.0.to_string(),
});

pub struct PairedDevice(pub storage::PairedDevice);
uniffi::custom_type!(PairedDevice, String, {
    try_lift: |json| Ok(PairedDevice(serde_json::from_str(&json)?)),
    lower: |val| serde_json::to_string(&val.0).expect("json serialization shouldn't fail"),
});

pub struct ConnectionDescriptor(pub connection::ConnectionDescriptor);
uniffi::custom_type!(ConnectionDescriptor, String, {
    try_lift: |json| Ok(ConnectionDescriptor(serde_json::from_str(&json)?)),
    lower: |val| serde_json::to_string(&val.0).expect("json serialization shouldn't fail"),
});

pub struct ConnectionStatus(pub connection::ConnectionStatus);
uniffi::custom_type!(ConnectionStatus, String, {
    try_lift: |json| Ok(ConnectionStatus(serde_json::from_str(&json)?)),
    lower: |val| serde_json::to_string(&val.0).expect("json serialization shouldn't fail"),
});

#[derive(Clone, Copy, Debug, uniffi::Enum)]
pub enum SessionFailureReason {
    TransportLost,
    ConnectionStatusDisconnected,
    ReadChannelClosed,
    WriteFailed,
    WriteTimedOut,
    SilenceWatchdogFailed,
    ConnectTimedOut,
    StateRefreshFailed,
    BluetoothAdapterUnavailable,
    DeviceNotFound,
    PermissionDenied,
    Unpaired,
    SecurityFailure,
    ConnectionFailed,
}

impl From<connection::SessionFailureReason> for SessionFailureReason {
    fn from(reason: connection::SessionFailureReason) -> Self {
        match reason {
            connection::SessionFailureReason::TransportLost => Self::TransportLost,
            connection::SessionFailureReason::ConnectionStatusDisconnected => {
                Self::ConnectionStatusDisconnected
            }
            connection::SessionFailureReason::ReadChannelClosed => Self::ReadChannelClosed,
            connection::SessionFailureReason::WriteFailed => Self::WriteFailed,
            connection::SessionFailureReason::WriteTimedOut => Self::WriteTimedOut,
            connection::SessionFailureReason::SilenceWatchdogFailed => Self::SilenceWatchdogFailed,
            connection::SessionFailureReason::ConnectTimedOut => Self::ConnectTimedOut,
            connection::SessionFailureReason::StateRefreshFailed => Self::StateRefreshFailed,
            connection::SessionFailureReason::BluetoothAdapterUnavailable => {
                Self::BluetoothAdapterUnavailable
            }
            connection::SessionFailureReason::DeviceNotFound => Self::DeviceNotFound,
            connection::SessionFailureReason::PermissionDenied => Self::PermissionDenied,
            connection::SessionFailureReason::Unpaired => Self::Unpaired,
            connection::SessionFailureReason::SecurityFailure => Self::SecurityFailure,
            connection::SessionFailureReason::ConnectionFailed => Self::ConnectionFailed,
        }
    }
}

#[derive(Clone, Debug, uniffi::Enum)]
pub enum SessionStatus {
    Connecting,
    Connected,
    Recovering {
        attempt: u32,
        reason: SessionFailureReason,
    },
    Lost {
        reason: SessionFailureReason,
    },
    Closed,
}

impl From<connection::SessionStatus> for SessionStatus {
    fn from(status: connection::SessionStatus) -> Self {
        match status {
            connection::SessionStatus::Connecting => Self::Connecting,
            connection::SessionStatus::Connected => Self::Connected,
            connection::SessionStatus::Recovering { attempt, reason } => Self::Recovering {
                attempt,
                reason: reason.into(),
            },
            connection::SessionStatus::Lost { reason } => Self::Lost {
                reason: reason.into(),
            },
            connection::SessionStatus::Closed => Self::Closed,
        }
    }
}

pub struct DeviceModel(pub openscq30_lib::DeviceModel);
uniffi::custom_type!(DeviceModel, String, {
    try_lift: |model| Ok(DeviceModel(openscq30_lib::DeviceModel::from_str(&model)?)),
    lower: |val| val.0.to_string(),
});

pub struct CategoryId(pub settings::CategoryId);
uniffi::custom_type!(CategoryId, String, {
    try_lift: |category_id| Ok(CategoryId(settings::CategoryId::from_str(&category_id)?)),
    lower: |val| val.0.to_string(),
});

#[derive(Hash, PartialEq, Eq)]
pub struct SettingId(pub settings::SettingId);
uniffi::custom_type!(SettingId, String, {
    try_lift: |setting_id| Ok(SettingId(settings::SettingId::from_str(&setting_id)?)),
    lower: |val| val.0.to_string(),
});

pub struct Setting(pub settings::Setting);
uniffi::custom_type!(Setting, String, {
    try_lift: |json| Ok(Setting(serde_json::from_str(&json)?)),
    lower: |val| serde_json::to_string(&val.0).expect("json serialization shouldn't fail"),
});

pub struct Value(pub settings::Value);
uniffi::custom_type!(Value, String, {
    try_lift: |json| Ok(Value(serde_json::from_str(&json)?)),
    lower: |val| serde_json::to_string(&val.0).expect("json serialization shouldn't fail"),
});

#[derive(uniffi::Record)]
pub struct SettingWriteOutcome {
    pub setting_id: SettingId,
    pub outcome: WriteOutcome,
}

#[derive(uniffi::Enum)]
pub enum WriteOutcome {
    Applied { observed: Value },
    UnsupportedInState { reason: AvailabilityReason },
    Rejected { observed: Option<Value> },
    TimedOut,
    Disconnected,
}

#[derive(Clone, Copy, Debug, uniffi::Enum)]
pub enum AvailabilityReason {
    RequiresBothEarbuds,
    NoReadBack,
}

impl From<device::SettingWriteOutcome> for SettingWriteOutcome {
    fn from(outcome: device::SettingWriteOutcome) -> Self {
        Self {
            setting_id: SettingId(outcome.setting_id),
            outcome: outcome.outcome.into(),
        }
    }
}

impl From<device::WriteOutcome> for WriteOutcome {
    fn from(outcome: device::WriteOutcome) -> Self {
        match outcome {
            device::WriteOutcome::Applied { observed } => Self::Applied {
                observed: Value(observed),
            },
            device::WriteOutcome::UnsupportedInState { reason } => Self::UnsupportedInState {
                reason: reason.into(),
            },
            device::WriteOutcome::Rejected { observed } => Self::Rejected {
                observed: observed.map(Value),
            },
            device::WriteOutcome::TimedOut => Self::TimedOut,
            device::WriteOutcome::Disconnected => Self::Disconnected,
        }
    }
}

impl From<client::availability::AvailabilityReason> for AvailabilityReason {
    fn from(reason: client::availability::AvailabilityReason) -> Self {
        match reason {
            client::availability::AvailabilityReason::RequiresBothEarbuds => {
                Self::RequiresBothEarbuds
            }
            client::availability::AvailabilityReason::NoReadBack => Self::NoReadBack,
        }
    }
}

pub struct QuickPreset(pub openscq30_lib::storage::QuickPreset);
uniffi::custom_type!(QuickPreset, String, {
    try_lift: |json| Ok(QuickPreset(serde_json::from_str(&json)?)),
    lower: |val| serde_json::to_string(&val.0).expect("json serialization shouldn't fail"),
});

pub struct QuickPresetField(pub storage::QuickPresetField);
uniffi::custom_type!(QuickPresetField, String, {
    try_lift: |json| Ok(QuickPresetField(serde_json::from_str(&json)?)),
    lower: |val| serde_json::to_string(&val.0).expect("json serialization shouldn't fail"),
});

pub struct Equalizer(pub settings::Equalizer);
uniffi::custom_type!(Equalizer, String, {
    try_lift: |json| Ok(Equalizer(serde_json::from_str(&json)?)),
    lower: |val| serde_json::to_string(&val.0).expect("json serialization shouldn't fail"),
});

#[derive(Debug, uniffi::Record)]
pub struct LanguageIdentifier {
    language: String,
    script: Option<String>,
    region: Option<String>,
    variants: Vec<String>,
}

impl TryFrom<&LanguageIdentifier> for i18n_embed::unic_langid::LanguageIdentifier {
    type Error = i18n_embed::unic_langid::parser::ParserError;

    fn try_from(value: &LanguageIdentifier) -> Result<Self, Self::Error> {
        Ok(Self::from_parts(
            subtags::Language::from_str(&value.language)?,
            value
                .script
                .as_ref()
                .map(|script| subtags::Script::from_str(script))
                .transpose()?,
            value
                .region
                .as_ref()
                .map(|region| subtags::Region::from_str(region))
                .transpose()?,
            &value
                .variants
                .iter()
                .map(|variant| subtags::Variant::from_str(variant))
                .collect::<Result<Vec<_>, _>>()?,
        ))
    }
}
