#![allow(clippy::inherent_to_string)]

mod api;
mod client;
pub mod connection;
mod device;
pub mod i18n;
pub mod quick_presets;
pub mod serializable;

use i18n_embed::unic_langid::LanguageIdentifier;
use log::LevelFilter;
use openscq30_lib::DeviceModel;
use strum::IntoEnumIterator;
use tracing::instrument;

uniffi::setup_scaffolding!();

#[uniffi::export]
pub fn init_native_logging() {
    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(LevelFilter::Trace)
            .with_tag("openscq30-lib"),
    );
}

#[uniffi::export]
#[instrument]
pub fn init_native_i18n(languages: Vec<serializable::LanguageIdentifier>) {
    let languages = languages
        .into_iter()
        .filter_map(|language| match LanguageIdentifier::try_from(&language) {
            Ok(language) => Some(language),
            Err(err) => {
                tracing::error!("failed to parse language, skipping: {err:?}: {language:?}");
                None
            }
        })
        .collect::<Vec<_>>();
    if languages.is_empty() {
        tracing::error!("initializing i18n with no languages");
    }
    openscq30_lib::i18n::init(&languages);
}

#[derive(thiserror::Error, Debug, uniffi::Error)]
#[uniffi(flat_error)]
pub enum OpenSCQ30Error {
    #[error("JsonError: {0:?}")]
    JsonError(#[from] serde_json::Error),
    #[error("DeviceError: {0:?}")]
    DeviceError(#[from] openscq30_lib::device::Error),
    #[error("StorageError: {0:?}")]
    StorageError(#[from] openscq30_lib::storage::Error),
}

#[derive(uniffi::Error, thiserror::Error, Debug)]
pub enum AndroidError {
    #[error("{0}")]
    Other(String),
    #[error("bluetooth permission denied")]
    PermissionDenied,
    #[error("device is not paired")]
    Unpaired,
    #[error("bluetooth adapter unavailable")]
    BluetoothAdapterUnavailable,
    #[error("connection timed out")]
    TimedOut,
}

#[uniffi::export]
pub fn device_models() -> Vec<serializable::DeviceModel> {
    DeviceModel::iter().map(serializable::DeviceModel).collect()
}

/// The battery percentage Android reports for a connected Classic audio device.
///
/// `BluetoothDevice.getBatteryLevel()` is a hidden API: reflection is blocked on recent Android releases, but
/// native code may still call it. Returns -1 when Android has no reading for the device.
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_oppzippy_openscq30_android_SystemBattery_nativeGetBatteryLevel<
    'caller,
>(
    mut env: jni::EnvUnowned<'caller>,
    _class: jni::objects::JClass<'caller>,
    device: jni::objects::JObject<'caller>,
) -> jni::sys::jint {
    env.with_env(|env| -> jni::errors::Result<jni::sys::jint> {
        let level = env
            .call_method(
                device,
                jni::jni_str!("getBatteryLevel"),
                jni::jni_sig!("()I"),
                &[],
            )?
            .i()?;
        Ok(if (0..=100).contains(&level) {
            level
        } else {
            -1
        })
    })
    .resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}
