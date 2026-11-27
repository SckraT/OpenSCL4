use openscl4_session::{
    CommandOutcome, ConnectionState, DeviceState, ModelDriver, SessionCommand, SessionError,
    SessionHandle, SessionRunner, SoundMode, SoundModeSetting, UnidentifiedReason,
    UnidentifiedState,
};
use openscl4_transport::{ConnectTarget, RedactedMac};
use openscl4_transport_mock::{
    VirtualA3028, VirtualA3029, VirtualA3062, VirtualA3949, VirtualA3953, VirtualA3957,
    VirtualIncompatibleA3954, VirtualSoundcoreDevice, VirtualUnknownIdentity,
};
use serde_json::{Value, json};

pub async fn handle(
    cycle_mode: bool,
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
                ModelDriver::A3953 => SessionRunner::start(VirtualA3953, target),
                ModelDriver::A3954 => SessionRunner::start(VirtualSoundcoreDevice, target),
                ModelDriver::A3957 => SessionRunner::start(VirtualA3957, target),
            },
        },
    };
    let output = read_pilot(&handle, cycle_mode).await;
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
async fn start_bluetooth(_name: &str) -> anyhow::Result<SessionHandle> {
    anyhow::bail!("the Bluetooth pilot is available on Windows and Linux")
}

async fn read_pilot(handle: &SessionHandle, cycle_mode: bool) -> anyhow::Result<Value> {
    let snapshot = handle.confirmed_view().await?;
    let ConnectionState::Ready(initial) = snapshot.connection else {
        return match snapshot.connection {
            ConnectionState::Unidentified(_) if cycle_mode => {
                anyhow::bail!("device is read-only: the pilot cannot change settings")
            }
            ConnectionState::Unidentified(state) => Ok(unidentified_json(state)),
            _ => Err(SessionError::Closed.into()),
        };
    };

    if !cycle_mode {
        return Ok(snapshot_json(initial));
    }

    let (change, restore) = mode_cycle_commands(initial)?;
    let changed = apply_mode(handle, change).await?;
    let restored = apply_mode(handle, restore).await?;

    let mut output = snapshot_json(restored);
    output["modeCycle"] = json!({
        "changed": applied_json(changed),
        "restored": applied_json(restored),
    });
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

async fn apply_mode(
    handle: &SessionHandle,
    command: SessionCommand,
) -> anyhow::Result<DeviceState> {
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
        ModelDriver::A3949 | ModelDriver::A3953 | ModelDriver::A3954 | ModelDriver::A3957 => {
            let mut battery = json!({
                "leftPercent": device.battery.left,
                "rightPercent": device.battery.right,
            });
            if matches!(
                device.model,
                ModelDriver::A3953 | ModelDriver::A3954 | ModelDriver::A3957
            ) {
                battery["casePercent"] = json!(device.battery.case);
            }
            (
                json!({
                    "left": device.firmware_left.map(|version| version.to_string()),
                    "right": device.firmware_right.map(|version| version.to_string()),
                }),
                battery,
            )
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
    if let Some(spatial_audio) = device.spatial_audio {
        snapshot["spatialAudio"] = json!({
            "enabled": spatial_audio.enabled,
            "mode": spatial_audio.mode.code(),
            "contentMode": spatial_audio.content.code(),
        });
    }
    if let Some(press_sensitivity) = device.press_sensitivity {
        snapshot["pressSensitivity"] = json!(press_sensitivity);
    }
    snapshot
}
