use openscl4_session::{
    A3949_DISABLED_ACTION, A3949Button, CommandOutcome, ConnectionState, DeviceState,
    EqualizerSetting, ModelDriver, QuickPresetValues, SessionCommand, SessionError, SessionHandle,
    SessionRunner, SoundMode, SoundModeSetting, UnidentifiedReason, UnidentifiedState,
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
async fn start_bluetooth(_name: &str) -> anyhow::Result<SessionHandle> {
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
        let changed = apply_mode(handle, change).await?;
        let restored = apply_mode(handle, restore).await?;
        output = snapshot_json(restored);
        output["modeCycle"] = json!({
            "changed": applied_json(changed),
            "restored": applied_json(restored),
        });
    }
    if cycle_settings {
        output["settingsCycle"] = settings_cycle(handle).await?;
    }
    Ok(output)
}

/// The measured SoundcoreSignature preset the pilot capture reports.
const SIGNATURE_PRESET: EqualizerSetting = EqualizerSetting {
    preset_id: 0,
    volume_adjustments: [0, 0, 0, 0, 0, 0, 0, 0, 0, -120],
};

/// The legacy Piano preset body from `r50i_set_eq_matches_soundcore_app`.
const PIANO_PRESET: EqualizerSetting = EqualizerSetting {
    preset_id: 0x000e,
    volume_adjustments: [0, 30, 30, 20, 40, 50, 30, 40, 0, -120],
};

/// Switches and restores every A3949 setting the session can confirm.
async fn settings_cycle(handle: &SessionHandle) -> anyhow::Result<Value> {
    let snapshot = handle.confirmed_view().await?;
    let ConnectionState::Ready(state) = snapshot.connection else {
        anyhow::bail!("device is not ready for the settings cycle");
    };
    let mut cycle = serde_json::Map::new();

    if state.model.equalizer_write_available(&state) {
        let original = state
            .equalizer
            .ok_or_else(|| anyhow::anyhow!("device reports no equalizer curve"))?;
        let target = if original == PIANO_PRESET {
            SIGNATURE_PRESET
        } else {
            PIANO_PRESET
        };
        let changed = apply_setting(
            handle,
            SessionCommand::ApplyQuickPreset(QuickPresetValues {
                equalizer: Some(target),
                ..QuickPresetValues::default()
            }),
        )
        .await?;
        let restored = apply_setting(
            handle,
            SessionCommand::ApplyQuickPreset(QuickPresetValues {
                equalizer: Some(original),
                ..QuickPresetValues::default()
            }),
        )
        .await?;
        cycle.insert(
            "equalizer".into(),
            json!({
                "changed": equalizer_json(changed, target),
                "restored": equalizer_json(restored, original),
            }),
        );
    }

    if let Some(original) = state.touch_tone {
        let changed = apply_setting(handle, SessionCommand::SetTouchTone(!original)).await?;
        let restored = apply_setting(handle, SessionCommand::SetTouchTone(original)).await?;
        cycle.insert(
            "touchTone".into(),
            json!({
                "changed": touch_tone_json(changed, !original),
                "restored": touch_tone_json(restored, original),
            }),
        );
    }

    if let Some(tws) = state.tws
        && let Some(buttons) = state.buttons
        && state.model.capabilities().button_configuration_write
    {
        let button = A3949Button::LeftSinglePress;
        let original = buttons.status(button).action(tws);
        let original = (original != A3949_DISABLED_ACTION).then_some(original);
        let target = if original == Some(2) { None } else { Some(2) };
        let changed = apply_setting(
            handle,
            SessionCommand::SetButtonAction {
                button,
                action: target,
            },
        )
        .await?;
        let restored = apply_setting(
            handle,
            SessionCommand::SetButtonAction {
                button,
                action: original,
            },
        )
        .await?;
        cycle.insert(
            "buttonAction".into(),
            json!({
                "changed": button_json(changed, button, target),
                "restored": button_json(restored, button, original),
            }),
        );
    }

    if state.model.capabilities().button_reset {
        match handle
            .dispatch(SessionCommand::ResetButtons)?
            .wait()
            .await?
        {
            CommandOutcome::Applied(_) | CommandOutcome::Accepted(_) => {
                cycle.insert("resetButtons".into(), json!({"result": "Accepted"}));
            }
        }
    }

    Ok(Value::Object(cycle))
}

async fn apply_setting(
    handle: &SessionHandle,
    command: SessionCommand,
) -> anyhow::Result<DeviceState> {
    Ok(match handle.dispatch(command)?.wait().await? {
        CommandOutcome::Applied(state) | CommandOutcome::Accepted(state) => state,
    })
}

fn equalizer_json(state: DeviceState, requested: EqualizerSetting) -> Value {
    json!({
        "result": if state.equalizer == Some(requested) { "Applied" } else { "Unconfirmed" },
        "presetId": state.equalizer.map(|equalizer| equalizer.preset_id),
        "bands": state.equalizer.map(|equalizer| equalizer.volume_adjustments.to_vec()),
    })
}

fn touch_tone_json(state: DeviceState, requested: bool) -> Value {
    json!({
        "result": if state.touch_tone == Some(requested) { "Applied" } else { "Unconfirmed" },
        "touchTone": state.touch_tone,
    })
}

fn button_json(state: DeviceState, button: A3949Button, requested: Option<u8>) -> Value {
    let observed = state
        .buttons
        .zip(state.tws)
        .map(|(buttons, tws)| buttons.status(button).action(tws))
        .and_then(|action| (action != A3949_DISABLED_ACTION).then_some(action));
    json!({
        "result": if observed == requested { "Applied" } else { "Unconfirmed" },
        "button": button.code(),
        "action": observed,
    })
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
    match handle.dispatch(command)?.wait().await? {
        CommandOutcome::Applied(state) | CommandOutcome::Accepted(state) => Ok(state),
    }
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
    if let Some(enabled) = device.gaming_mode {
        snapshot["gamingMode"] = json!(enabled);
    }
    snapshot
}
