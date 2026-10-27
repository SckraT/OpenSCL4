use std::collections::HashMap;

use crate::devices::soundcore::{
    a3028::{packets::A3028StateUpdatePacket, state::A3028State},
    common::{
        self,
        device::fetch_state_from_state_update_packet,
        macros::soundcore_device,
        modules::{equalizer, sound_modes::AvailableSoundModes},
        packet::outbound::{RequestState, ToPacket},
        structures::{AmbientSoundMode, NoiseCancelingMode},
    },
};

mod packets;
mod state;

soundcore_device!(
    A3028State,
    async |packet_io| {
        fetch_state_from_state_update_packet::<A3028State, A3028StateUpdatePacket>(packet_io).await
    },
    async |builder| {
        builder.module_collection().add_state_update();
        builder.sound_modes(AvailableSoundModes {
            ambient_sound_modes: vec![
                AmbientSoundMode::Normal,
                AmbientSoundMode::Transparency,
                AmbientSoundMode::NoiseCanceling,
            ],
            transparency_modes: vec![],
            noise_canceling_modes: vec![
                NoiseCancelingMode::Transport,
                NoiseCancelingMode::Indoor,
                NoiseCancelingMode::Outdoor,
            ],
        });
        builder.equalizer(equalizer::common_settings()).await;
        builder.auto_power_off(
            common::modules::auto_power_off::AutoPowerOffDuration::half_hour_increments(),
        );
        builder.single_battery(5);
        builder.serial_number_and_firmware_version();
    },
    {
        HashMap::from([(
            RequestState::COMMAND,
            A3028StateUpdatePacket::default().to_packet(),
        )])
    },
);

#[cfg(test)]
mod tests {
    use std::{borrow::Cow, sync::Arc, time::Duration};

    use macaddr::MacAddr6;
    use tokio::sync::mpsc;

    use crate::{
        api::{
            device::{OpenSCQ30Device, OpenSCQ30DeviceRegistry, SettingWriteOutcome, WriteOutcome},
            settings::{SettingId, Value},
        },
        connection_backend::mock::rfcomm::MockRfcommBackend,
        devices::{
            DeviceModel,
            soundcore::common::{
                packet,
                structures::{AmbientSoundMode, NoiseCancelingMode},
            },
        },
        storage::OpenSCQ30Database,
    };

    fn example_state_update_packet() -> Vec<u8> {
        vec![
            0x09, 0xff, 0x00, 0x00, 0x01, 0x01, 0x01, 0x46, 0x00, 0x05, 0x00, 0xfe, 0xfe, 0x3c,
            0xb4, 0x8f, 0xa0, 0x8e, 0xb4, 0x74, 0x88, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x02, 0x00, 0x01, 0x00, 0x30, 0x32, 0x2e, 0x33, 0x30, 0x33, 0x30, 0x32,
            0x39, 0x30, 0x38, 0x36, 0x45, 0x43, 0x38, 0x32, 0x46, 0x31, 0x32, 0x41, 0x43, 0x30,
        ]
    }

    async fn set_settings_with_acks(
        device: Arc<dyn OpenSCQ30Device + Send + Sync>,
        settings: Vec<(SettingId, Value)>,
        inbound_sender: mpsc::Sender<Vec<u8>>,
        outbound_receiver: &mut mpsc::Receiver<Vec<u8>>,
    ) -> (Vec<SettingWriteOutcome>, usize) {
        let mut join =
            tokio::spawn(async move { device.set_setting_values(settings).await.unwrap() });
        let mut set_commands = 0;
        loop {
            tokio::select! {
                result = &mut join => return (result.unwrap(), set_commands),
                bytes = outbound_receiver.recv() => {
                    let Some(bytes) = bytes else { break };
                    let packet = packet::Outbound::take::<nom_language::error::VerboseError<_>>(
                        packet::ChecksumKind::Suffix,
                    )(&bytes)
                    .unwrap()
                    .1;
                    if packet.command == packet::outbound::SetSoundModes::COMMAND
                        || packet.command == packet::outbound::SET_EQUALIZER_COMMAND
                    {
                        set_commands += 1;
                    }
                    inbound_sender
                        .send(packet.command.ack::<packet::InboundMarker>().bytes_with_checksum())
                        .await
                        .unwrap();
                }
            }
        }
        (join.await.unwrap(), set_commands)
    }

    async fn create_test_connection() -> (
        impl OpenSCQ30DeviceRegistry,
        mpsc::Sender<Vec<u8>>,
        mpsc::Receiver<Vec<u8>>,
    ) {
        let (inbound_sender, inbound_receiver) = mpsc::channel(10);
        let (outbound_sender, outbound_receiver) = mpsc::channel(10);
        let database = Arc::new(OpenSCQ30Database::new_in_memory().await.unwrap());
        let registry = super::device_registry(
            Arc::new(MockRfcommBackend::new(inbound_receiver, outbound_sender)),
            database,
            DeviceModel::SoundcoreA3028,
        );
        (registry, inbound_sender, outbound_receiver)
    }

    #[tokio::test(start_paused = true)]
    async fn new_with_example_state_update_packet() {
        let (registry, inbound_sender, mut outbound_receiver) = create_test_connection().await;
        inbound_sender
            .send(example_state_update_packet())
            .await
            .unwrap();
        let device = registry.connect(MacAddr6::nil()).await.unwrap();
        _ = outbound_receiver
            .recv()
            .await
            .expect("state update packet request");
        assert_eq!(
            AmbientSoundMode::Normal,
            Value::from(device.setting(&SettingId::AmbientSoundMode).unwrap())
                .try_as_enum_variant()
                .unwrap()
        );
        assert_eq!(
            NoiseCancelingMode::Transport,
            Value::from(device.setting(&SettingId::NoiseCancelingMode).unwrap())
                .try_as_enum_variant()
                .unwrap()
        );
        assert_eq!(
            None,
            Value::from(device.setting(&SettingId::PresetEqualizerProfile).unwrap())
                .try_as_optional_str()
                .unwrap()
        );
        assert_eq!(
            vec![-60, 60, 23, 40, 22, 60, -4, 16],
            Value::from(device.setting(&SettingId::VolumeAdjustments).unwrap())
                .try_into_i16_vec()
                .unwrap()
        );
    }

    #[tokio::test(start_paused = true)]
    async fn new_with_retry() {
        let (registry, inbound_sender, mut outbound_receiver) = create_test_connection().await;
        tokio::spawn(async move {
            outbound_receiver
                .recv()
                .await
                .expect("state update packet request");
            tokio::time::sleep(Duration::from_millis(1500)).await;
            inbound_sender
                .send(example_state_update_packet())
                .await
                .unwrap();
        });
        registry
            .connect(MacAddr6::nil())
            .await
            .expect("should not time out");
    }

    #[tokio::test(start_paused = true)]
    async fn new_max_retries() {
        let (registry, _inbound_sender, _outbound_receiver) = create_test_connection().await;
        registry
            .connect(MacAddr6::nil())
            .await
            .err()
            .expect("should time out");
    }

    #[tokio::test(start_paused = true)]
    async fn ambient_sound_mode_update_packet() {
        let (registry, inbound_sender, mut _outbound_receiver) = create_test_connection().await;
        inbound_sender
            .send(example_state_update_packet())
            .await
            .unwrap();
        let device = registry.connect(MacAddr6::nil()).await.unwrap();

        // alert from device that sound mode changed
        inbound_sender
            .send(vec![
                0x09, 0xff, 0x00, 0x00, 0x01, 0x06, 0x01, 0x0e, 0x00, 0x00, 0x01, 0x01, 0x00, 0x20,
            ])
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(1)).await;

        assert_eq!(
            AmbientSoundMode::NoiseCanceling,
            Value::from(device.setting(&SettingId::AmbientSoundMode).unwrap())
                .try_as_enum_variant()
                .unwrap()
        );
        assert_eq!(
            NoiseCancelingMode::Outdoor,
            Value::from(device.setting(&SettingId::NoiseCancelingMode).unwrap())
                .try_as_enum_variant()
                .unwrap()
        );
    }

    #[tokio::test(start_paused = true)]
    async fn set_sound_mode_resends_when_the_device_does_not_confirm() {
        let (registry, inbound_sender, mut outbound_receiver) = create_test_connection().await;
        inbound_sender
            .send(example_state_update_packet())
            .await
            .unwrap();
        let device = registry.connect(MacAddr6::nil()).await.unwrap();
        outbound_receiver
            .recv()
            .await
            .expect("state update packet request");

        let (first, sets) = set_settings_with_acks(
            device.clone(),
            vec![(
                SettingId::AmbientSoundMode,
                Cow::from("Transparency").into(),
            )],
            inbound_sender.clone(),
            &mut outbound_receiver,
        )
        .await;
        assert_eq!(1, sets);
        assert!(matches!(first[0].outcome, WriteOutcome::TimedOut));

        // Without a state read-back the write is not Applied, so asking for the same value
        // again is a new write, not a no-op.
        let (second, sets) = set_settings_with_acks(
            device,
            vec![(
                SettingId::AmbientSoundMode,
                Cow::from("Transparency").into(),
            )],
            inbound_sender,
            &mut outbound_receiver,
        )
        .await;
        assert_eq!(1, sets);
        assert!(matches!(second[0].outcome, WriteOutcome::TimedOut));
    }

    #[tokio::test(start_paused = true)]
    async fn set_equalizer_configuration_resends_when_the_device_does_not_confirm() {
        let (registry, inbound_sender, mut outbound_receiver) = create_test_connection().await;
        inbound_sender
            .send(example_state_update_packet())
            .await
            .unwrap();
        let device = registry.connect(MacAddr6::nil()).await.unwrap();
        outbound_receiver
            .recv()
            .await
            .expect("state update packet request");

        let (first, sets) = set_settings_with_acks(
            device.clone(),
            vec![(
                SettingId::PresetEqualizerProfile,
                Cow::from("Acoustic").into(),
            )],
            inbound_sender.clone(),
            &mut outbound_receiver,
        )
        .await;
        assert_eq!(1, sets);
        assert!(matches!(first[0].outcome, WriteOutcome::TimedOut));

        let (second, sets) = set_settings_with_acks(
            device,
            vec![(
                SettingId::PresetEqualizerProfile,
                Cow::from("Acoustic").into(),
            )],
            inbound_sender,
            &mut outbound_receiver,
        )
        .await;
        assert_eq!(1, sets);
        assert!(matches!(second[0].outcome, WriteOutcome::TimedOut));
    }
}
