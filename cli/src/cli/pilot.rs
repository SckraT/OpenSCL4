use openscl4_session::{ConnectionState, SessionError, SessionRunner};
use openscl4_transport::{ConnectTarget, RedactedMac};
use openscl4_transport_mock::VirtualSoundcoreDevice;
use serde_json::json;

pub async fn handle() -> anyhow::Result<()> {
    let target = ConnectTarget {
        mac: RedactedMac::from_octets([0; 6]),
    };
    let handle = SessionRunner::start(VirtualSoundcoreDevice, target);
    let snapshot = handle.confirmed_view().await;
    let closed = handle.close().await;
    let snapshot = snapshot?;
    closed?;

    let ConnectionState::Ready(device) = snapshot.connection else {
        return Err(SessionError::Closed.into());
    };
    let output = json!({
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
    });
    println!("{output}");
    Ok(())
}
