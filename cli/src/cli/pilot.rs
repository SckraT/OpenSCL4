use openscl4_session::{
    CommandOutcome, ConnectionState, DeviceState, ModelDriver, SessionCommand, SessionError,
    SessionHandle, SessionRunner, SoundMode, SoundModeSetting,
};
use openscl4_transport::{ConnectTarget, RedactedMac};
use openscl4_transport_mock::{VirtualA3062, VirtualA3949, VirtualSoundcoreDevice};
use serde_json::{Value, json};

pub async fn handle(
    cycle_mode: bool,
    device: Option<&str>,
    mock_model: ModelDriver,
) -> anyhow::Result<()> {
    let handle = match device {
        Some(name) => start_bluetooth(name).await?,
        None => {
            let target = ConnectTarget {
                mac: RedactedMac::from_octets([0; 6]),
            };
            match mock_model {
                ModelDriver::A3062 => SessionRunner::start(VirtualA3062, target),
                ModelDriver::A3949 => SessionRunner::start(VirtualA3949, target),
                ModelDriver::A3954 => SessionRunner::start(VirtualSoundcoreDevice, target),
            }
        }
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
        return Err(SessionError::Closed.into());
    };

    if !cycle_mode {
        return Ok(snapshot_json(initial));
    }

    let Some(original_mode) = initial.sound_mode else {
        anyhow::bail!("device model is read-only in the pilot");
    };
    let target = match original_mode {
        SoundMode::NoiseCanceling | SoundMode::Transparency => SoundModeSetting::NORMAL,
        SoundMode::Normal => SoundModeSetting::NOISE_CANCELING,
        _ => anyhow::bail!("the pilot cycles normal and noise canceling modes"),
    };
    let (change, restore) = if let Some(original) = initial.sound_mode_setting() {
        initial.sound_mode_write(original)?;
        (
            SessionCommand::SetSoundMode(target),
            SessionCommand::SetSoundMode(original),
        )
    } else {
        initial.ambient_sound_mode_write(original_mode)?;
        (
            SessionCommand::SetAmbientSoundMode(target.mode),
            SessionCommand::SetAmbientSoundMode(original_mode),
        )
    };
    let changed = apply_mode(handle, change).await?;
    let restored = apply_mode(handle, restore).await?;

    let mut output = snapshot_json(restored);
    output["modeCycle"] = json!({
        "changed": applied_json(changed),
        "restored": applied_json(restored),
    });
    Ok(output)
}

async fn apply_mode(
    handle: &SessionHandle,
    command: SessionCommand,
) -> anyhow::Result<DeviceState> {
    let CommandOutcome::Applied(state) = handle.dispatch(command)?.wait().await?;
    Ok(state)
}

fn applied_json(state: DeviceState) -> Value {
    let mut applied = json!({
        "result": "Applied",
        "soundMode": state.sound_mode.map(SoundMode::code),
    });
    if let Some(setting) = state.sound_mode_setting() {
        applied["position"] = json!(setting.position);
    }
    applied
}

fn snapshot_json(device: DeviceState) -> Value {
    let (firmware, battery) = match device.model {
        ModelDriver::A3062 => (
            json!({"device": device.firmware_device.map(|version| version.to_string())}),
            json!({"devicePercent": device.battery.device}),
        ),
        ModelDriver::A3949 | ModelDriver::A3954 => {
            let mut battery = json!({
                "leftPercent": device.battery.left,
                "rightPercent": device.battery.right,
            });
            if device.model == ModelDriver::A3954 {
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
    snapshot
}
