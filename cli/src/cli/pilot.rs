use openscl4_session::{
    CommandOutcome, ConnectionState, DeviceState, SessionCommand, SessionError, SessionHandle,
    SessionRunner, SoundMode, SoundModeSetting,
};
use openscl4_transport::{ConnectTarget, RedactedMac};
use openscl4_transport_mock::VirtualSoundcoreDevice;
use serde_json::{Value, json};

pub async fn handle(cycle_mode: bool) -> anyhow::Result<()> {
    let target = ConnectTarget {
        mac: RedactedMac::from_octets([0; 6]),
    };
    let handle = SessionRunner::start(VirtualSoundcoreDevice, target);
    let output = read_pilot(&handle, cycle_mode).await;
    let closed = handle.close().await;
    let output = output?;
    closed?;

    println!("{output}");
    Ok(())
}

async fn read_pilot(handle: &SessionHandle, cycle_mode: bool) -> anyhow::Result<Value> {
    let snapshot = handle.confirmed_view().await?;
    let ConnectionState::Ready(initial) = snapshot.connection else {
        return Err(SessionError::Closed.into());
    };

    if !cycle_mode {
        return Ok(snapshot_json(initial));
    }

    let original = initial.sound_mode_setting();
    let target = match original.mode {
        SoundMode::NoiseCanceling => SoundModeSetting::NORMAL,
        SoundMode::Normal => SoundModeSetting::NOISE_CANCELING,
        _ => anyhow::bail!("the pilot cycles normal and noise canceling modes"),
    };
    let changed = apply_mode(handle, target).await?;
    let restored = apply_mode(handle, original).await?;

    let mut output = snapshot_json(restored);
    output["modeCycle"] = json!({
        "changed": applied_json(changed),
        "restored": applied_json(restored),
    });
    Ok(output)
}

async fn apply_mode(
    handle: &SessionHandle,
    setting: SoundModeSetting,
) -> anyhow::Result<DeviceState> {
    let CommandOutcome::Applied(state) = handle
        .dispatch(SessionCommand::SetSoundMode(setting))?
        .wait()
        .await?;
    Ok(state)
}

fn applied_json(state: DeviceState) -> Value {
    let setting = state.sound_mode_setting();
    json!({
        "result": "Applied",
        "soundMode": setting.mode.code(),
        "position": setting.position,
    })
}

fn snapshot_json(device: DeviceState) -> Value {
    json!({
        "model": device.model.code(),
        "firmware": {
            "left": device.firmware_left.map(|version| version.to_string()),
            "right": device.firmware_right.map(|version| version.to_string()),
        },
        "battery": {
            "leftPercent": device.battery.left,
            "rightPercent": device.battery.right,
            "casePercent": device.battery.case,
        },
        "soundMode": device.sound_mode.code(),
    })
}
