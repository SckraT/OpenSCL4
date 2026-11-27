use openscl4_session::{
    AutoPowerOff, AutoPowerOffSetting, CaseFeatures, CaseLanguage, CommandOutcome, ConnectionState,
    DeviceState, ModelDriver, SessionCommand, SessionError, SessionHandle, SessionRunner,
    SoundMode, SoundModeSetting, SpatialAudio, UnidentifiedReason, UnidentifiedState,
};
use openscl4_transport::{ConnectTarget, RedactedMac};
use openscl4_transport_mock::{
    VirtualA3028, VirtualA3029, VirtualA3062, VirtualA3949, VirtualIncompatibleA3954,
    VirtualSoundcoreDevice, VirtualUnknownIdentity,
};
use serde_json::{Value, json};

pub async fn handle(
    cycle_mode: bool,
    cycle_settings: bool,
    device: Option<&str>,
    mock_model: ModelDriver,
    scenario: Option<&str>,
) -> anyhow::Result<()> {
    let target = ConnectTarget {
        mac: RedactedMac::from_octets([0; 6]),
    };
    let handle = match device {
        Some(name) => start_bluetooth(name).await?,
        None => match scenario {
            Some("unknown-identity") => SessionRunner::start(VirtualUnknownIdentity, target),
            Some("incompatible-state") => SessionRunner::start(VirtualIncompatibleA3954, target),
            _ => match mock_model {
                ModelDriver::A3028 => SessionRunner::start(VirtualA3028, target),
                ModelDriver::A3029 => SessionRunner::start(VirtualA3029, target),
                ModelDriver::A3062 => SessionRunner::start(VirtualA3062, target),
                ModelDriver::A3949 => SessionRunner::start(VirtualA3949, target),
                ModelDriver::A3954 => SessionRunner::start(VirtualSoundcoreDevice, target),
            },
        },
    };
    let output = read_pilot(&handle, cycle_mode, cycle_settings).await;
    let closed = handle.close().await;
    let output = output?;
    closed?;

    println!("{output}");
    Ok(())
}

#[cfg(windows)]
async fn start_bluetooth(name: &str) -> anyhow::Result<SessionHandle> {
    use std::time::Duration;

    use openscl4_transport_windows::{WindowsRfcommTransport, connected_target};

    let target = tokio::time::timeout(Duration::from_secs(5), connected_target(name))
        .await
        .map_err(|_| SessionError::Timeout(openscl4_session::IoStage::Connect))??;
    Ok(SessionRunner::start(WindowsRfcommTransport, target))
}

#[cfg(target_os = "linux")]
async fn start_bluetooth(name: &str) -> anyhow::Result<SessionHandle> {
    use std::time::Duration;

    use openscl4_transport_bluez::BluezRfcommTransport;

    let (provider, target) = tokio::time::timeout(
        Duration::from_secs(5),
        BluezRfcommTransport::for_connected_device(name),
    )
    .await
    .map_err(|_| SessionError::Timeout(openscl4_session::IoStage::Connect))??;
    Ok(SessionRunner::start(provider, target))
}

#[cfg(not(any(windows, target_os = "linux")))]
async fn start_bluetooth(_name: &str) -> anyhow::Result<()> {
    anyhow::bail!("the Bluetooth pilot is available on Windows and Linux")
}

async fn read_pilot(
    handle: &SessionHandle,
    cycle_mode: bool,
    cycle_settings: bool,
) -> anyhow::Result<Value> {
    let snapshot = handle.confirmed_view().await?;
    let ConnectionState::Ready(initial) = snapshot.connection else {
        return match snapshot.connection {
            ConnectionState::Unidentified(_) if cycle_mode || cycle_settings => {
                anyhow::bail!("device is read-only: the pilot cannot change settings")
            }
            ConnectionState::Unidentified(state) => Ok(unidentified_json(state)),
            _ => Err(SessionError::Closed.into()),
        };
    };

    let mut output = snapshot_json(initial);

    if cycle_mode {
        let (change, restore) = mode_cycle_commands(initial)?;
        let changed = apply(handle, change).await?;
        let restored = apply(handle, restore).await?;
        output = snapshot_json(restored);
        output["modeCycle"] = json!({
            "changed": applied_json(changed),
            "restored": applied_json(restored),
        });
    }

    if cycle_settings {
        let mut entries = Vec::new();
        let mut restored_state = initial;
        for (setting, change, restore) in settings_cycle_commands(initial)? {
            apply(handle, change).await?;
            let restored = apply(handle, restore).await?;
            // Each result is `Applied` only after a fresh read-back matched.
            entries.push(json!({
                "setting": setting,
                "changed": "Applied",
                "restored": "Applied",
            }));
            restored_state = restored;
        }
        output = snapshot_json(restored_state);
        output["settingsCycle"] = json!(entries);
    }

    Ok(output)
}

fn mode_cycle_commands(initial: DeviceState) -> anyhow::Result<(SessionCommand, SessionCommand)> {
    if let Some(original) = initial.gaming_mode
        && initial.model.capabilities().gaming_mode_write
    {
        return Ok((
            SessionCommand::SetGamingMode(!original),
            SessionCommand::SetGamingMode(original),
        ));
    }
    let Some(original_mode) = initial.sound_mode else {
        anyhow::bail!("device model is read-only in the pilot");
    };
    let target = match original_mode {
        SoundMode::NoiseCanceling | SoundMode::Transparency => SoundModeSetting::NORMAL,
        SoundMode::Normal => SoundModeSetting::NOISE_CANCELING,
        _ => anyhow::bail!("the pilot cycles normal and noise canceling modes"),
    };
    if let Some(original) = initial.sound_mode_setting() {
        initial.sound_mode_write(original)?;
        Ok((
            SessionCommand::SetSoundMode(target),
            SessionCommand::SetSoundMode(original),
        ))
    } else {
        Ok((
            SessionCommand::SetAmbientSoundMode(target.mode),
            SessionCommand::SetAmbientSoundMode(original_mode),
        ))
    }
}

/// The settings cycle writes each supported A3954 setting to a different value
/// and restores the confirmed original, one read-back at a time.
fn settings_cycle_commands(
    initial: DeviceState,
) -> anyhow::Result<Vec<(&'static str, SessionCommand, SessionCommand)>> {
    if initial.model != ModelDriver::A3954 {
        anyhow::bail!("the settings cycle is available for the A3954 pilot only");
    }
    let mut commands = Vec::new();

    let features = initial
        .case_features
        .ok_or_else(|| anyhow::anyhow!("the device did not report its case features"))?;
    let toggled = CaseFeatures {
        atmospheric: !features.atmospheric,
        ..features
    };
    commands.push((
        "caseFeatures",
        SessionCommand::SetCaseFeatures(toggled),
        SessionCommand::SetCaseFeatures(features),
    ));

    let spatial_audio = initial
        .spatial_audio
        .ok_or_else(|| anyhow::anyhow!("the device did not report its spatial audio"))?;
    let toggled_spatial_audio = SpatialAudio {
        enabled: !spatial_audio.enabled,
        ..spatial_audio
    };
    commands.push((
        "spatialAudio",
        SessionCommand::SetSpatialAudio(toggled_spatial_audio),
        SessionCommand::SetSpatialAudio(spatial_audio),
    ));

    let sound_leak_compensation = initial
        .sound_leak_compensation
        .ok_or_else(|| anyhow::anyhow!("the device did not report sound leak compensation"))?;
    commands.push((
        "soundLeakCompensation",
        SessionCommand::SetSoundLeakCompensation(!sound_leak_compensation),
        SessionCommand::SetSoundLeakCompensation(sound_leak_compensation),
    ));

    let auto_power_off = initial
        .auto_power_off
        .ok_or_else(|| anyhow::anyhow!("the device did not report auto power off"))?;
    commands.push((
        "autoPowerOff",
        SessionCommand::SetAutoPowerOff(AutoPowerOffSetting::Enabled(
            (auto_power_off.duration + 1) % 4,
        )),
        SessionCommand::SetAutoPowerOff(restore_auto_power_off(auto_power_off)),
    ));

    let case_language = initial
        .case_language
        .ok_or_else(|| anyhow::anyhow!("the device did not report its case language"))?;
    commands.push((
        "caseLanguage",
        SessionCommand::SetCaseLanguage(next_case_language(case_language)),
        SessionCommand::SetCaseLanguage(case_language),
    ));

    Ok(commands)
}

fn restore_auto_power_off(value: AutoPowerOff) -> AutoPowerOffSetting {
    if value.enabled {
        AutoPowerOffSetting::Enabled(value.duration)
    } else {
        AutoPowerOffSetting::Disabled
    }
}

fn next_case_language(language: CaseLanguage) -> CaseLanguage {
    match language {
        CaseLanguage::Japanese => CaseLanguage::English,
        CaseLanguage::English => CaseLanguage::Chinese,
        CaseLanguage::Chinese => CaseLanguage::Japanese,
    }
}

async fn apply(handle: &SessionHandle, command: SessionCommand) -> anyhow::Result<DeviceState> {
    let CommandOutcome::Applied(state) = handle.dispatch(command)?.wait().await?;
    Ok(state)
}

fn applied_json(state: DeviceState) -> Value {
    if let Some(enabled) = state.gaming_mode {
        return json!({"result": "Applied", "gamingMode": enabled});
    }
    let mut applied = json!({
        "result": "Applied",
        "soundMode": state.sound_mode.map(SoundMode::code),
    });
    if let Some(setting) = state.sound_mode_setting() {
        applied["position"] = json!(setting.position);
    }
    applied
}

fn unidentified_json(state: UnidentifiedState) -> Value {
    json!({
        "model": state.model.map(ModelDriver::code),
        "readOnly": true,
        "reason": match state.reason {
            UnidentifiedReason::UnknownIdentity => "unknownIdentity",
            UnidentifiedReason::IncompatibleState => "incompatibleState",
        },
    })
}

fn snapshot_json(device: DeviceState) -> Value {
    let (firmware, battery) = match device.model {
        ModelDriver::A3028 | ModelDriver::A3029 | ModelDriver::A3062 => (
            json!({"device": device.firmware_device.map(|version| version.to_string())}),
            json!({"devicePercent": device.battery.device}),
        ),
        ModelDriver::A3949 | ModelDriver::A3954 => {
            let mut battery = json!({
                "leftPercent": device.battery.left,
                "rightPercent": device.battery.right,
            });
            let mut firmware = json!({
                "left": device.firmware_left.map(|version| version.to_string()),
                "right": device.firmware_right.map(|version| version.to_string()),
            });
            if device.model == ModelDriver::A3954 {
                battery["casePercent"] = json!(device.battery.case);
                firmware["case"] = json!(device.firmware_case.map(|version| version.to_string()));
            }
            (firmware, battery)
        }
    };
    let mut snapshot = json!({
        "model": device.model.code(),
        "firmware": firmware,
        "battery": battery,
    });
    if let Some(mode) = device.sound_mode {
        snapshot["soundMode"] = json!(mode.code());
    }
    if let Some(enabled) = device.gaming_mode {
        snapshot["gamingMode"] = json!(enabled);
    }
    if device.model == ModelDriver::A3954 {
        a3954_settings_json(&mut snapshot, device);
    }
    snapshot
}

/// Reports the A3954 settings that the session reads from the measured state.
fn a3954_settings_json(snapshot: &mut Value, device: DeviceState) {
    snapshot["twsConnected"] = json!(device.tws_connected);
    if let Some(equalizer) = device.equalizer {
        snapshot["equalizer"] = json!({
            "presetId": equalizer.preset_id,
            "volumeAdjustments": equalizer.volume_adjustments,
        });
    }
    if let Some(auto_power_off) = device.auto_power_off {
        snapshot["autoPowerOff"] = json!({
            "enabled": auto_power_off.enabled,
            "duration": auto_power_off.duration,
        });
    }
    if let Some(features) = device.case_features {
        snapshot["caseFeatures"] = json!({
            "atmospheric": features.atmospheric,
            "remoteCamera": features.remote_camera,
            "findDevice": features.find_device,
            "caseSpatialAudio": features.case_spatial_audio,
        });
    }
    if let Some(spatial_audio) = device.spatial_audio {
        snapshot["spatialAudio"] = json!({
            "enabled": spatial_audio.enabled,
            "mode": spatial_audio.mode.code(),
            "musicMode": spatial_audio.music_mode.code(),
        });
    }
    if let Some(enabled) = device.sound_leak_compensation {
        snapshot["soundLeakCompensation"] = json!(enabled);
    }
    if let Some(language) = device.case_language {
        snapshot["caseLanguage"] = json!(language.code());
    }
}
