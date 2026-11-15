use macaddr::MacAddr6;
use nom::{
    IResult, Parser,
    error::{ContextError, ParseError, context},
    number::complete::le_u8,
};

use crate::devices::soundcore::common::{
    self,
    packet::{self, Command, outbound::ToPacket},
    structures::DualConnectionsDevice,
};

use super::FromPacketBody;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DualConnectionsDevicePacket {
    pub total_packets: u8,
    /// Index starts from 1
    pub current_packet_index: u8,
    pub devices: Vec<common::structures::DualConnectionsDevice>,
}

impl DualConnectionsDevicePacket {
    pub const COMMAND: Command = Command([0x0b, 0x01]);

    pub fn demo() -> Self {
        Self {
            total_packets: 1,
            current_packet_index: 1,
            devices: vec![
                DualConnectionsDevice {
                    is_connected: true,
                    mac_address: MacAddr6::new(0, 0, 0, 0, 0, 0),
                    name: "Device 1".to_string(),
                },
                DualConnectionsDevice {
                    is_connected: false,
                    mac_address: MacAddr6::new(0, 0, 0, 0, 0, 1),
                    name: "Device 2".to_string(),
                },
                DualConnectionsDevice {
                    is_connected: false,
                    mac_address: MacAddr6::new(0, 0, 0, 0, 0, 2),
                    name: "Device 3".to_string(),
                },
            ],
        }
    }
}

impl ToPacket for DualConnectionsDevicePacket {
    type DirectionMarker = packet::InboundMarker;

    fn command(&self) -> packet::Command {
        Self::COMMAND
    }

    fn body(&self) -> Vec<u8> {
        [self.total_packets, self.current_packet_index]
            .into_iter()
            .chain(self.devices.iter().flat_map(|device| device.bytes()))
            .collect()
    }
}

impl FromPacketBody for DualConnectionsDevicePacket {
    type DirectionMarker = packet::InboundMarker;

    fn take<'a, E: ParseError<&'a [u8]> + ContextError<&'a [u8]>>(
        input: &'a [u8],
    ) -> IResult<&'a [u8], Self, E> {
        context("dual connection device", |input: &'a [u8]| {
            let (mut input, (total_packets, current_packet_index)) =
                (le_u8, le_u8).parse_complete(input)?;

            let mut devices = Vec::new();
            while !input.is_empty() {
                let (remaining, device) = DualConnectionsDevice::take(input)?;
                devices.push(device);
                input = remaining;
            }

            Ok((
                &[] as &[u8], // name extends all the way to the end of the packet, so this empties out input
                Self {
                    total_packets,
                    current_packet_index,
                    devices,
                },
            ))
        })
        .parse_complete(input)
    }
}

#[cfg(test)]
mod tests {
    use nom_language::error::VerboseError;

    use super::*;

    #[test]
    fn round_trip() {
        let initial = [
            0x6, 0x1, 0x28, 0x1, 0x66, 0x14, 0x92, 0x2c, 0x3a, 0xd4, 0x50, 0x69, 0x78, 0x65, 0x6c,
            0x20, 0x36, 0x61, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
            0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
        ];
        let (remaining, parsed) =
            DualConnectionsDevicePacket::take::<VerboseError<_>>(&initial).unwrap();
        assert_eq!(remaining.len(), 0);
        assert_eq!(parsed.to_packet().body, initial);
    }

    #[test]
    fn no_string_null_termination() {
        let initial = [
            0x6, 0x1, 0x28, 0x1, 0x66, 0x14, 0x92, 0x2c, 0x3a, 0xd4, 0x50, 0x69, 0x78, 0x65, 0x6c,
            0x20, 0x36, 0x61, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65,
            65, 65, 65, 65, 65, 65, 65,
        ];
        let (remaining, parsed) =
            DualConnectionsDevicePacket::take::<VerboseError<_>>(&initial).unwrap();
        assert_eq!(remaining.len(), 0);
        assert_eq!(parsed.devices.len(), 1);
        assert_eq!(parsed.devices[0].name, "Pixel 6aAAAAAAAAAAAAAAAAAAAAAAAA");
    }

    #[test]
    fn parses_a_name_truncated_mid_utf8_character() {
        // The device pads each remembered name to the 32-byte name field and
        // can cut a multi-byte character in half. This used to fail strict
        // UTF-8 decoding, which discarded the whole device list. Device names
        // and addresses here are synthetic; the packet layout is from an
        // A3954 capture.
        let initial = [
            0x01, 0x01, 0x28, 0x01, 0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x4c, 0x61, 0x70, 0x74,
            0x6f, 0x70, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x28, 0x01, 0x00, 0x11, 0x22, 0x33, 0x44, 0x56, 0x50, 0x68, 0x6f, 0x6e, 0x65, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x28, 0x00,
            0x00, 0x11, 0x22, 0x33, 0x44, 0x57, 0x44, 0x65, 0x76, 0x69, 0x63, 0x65, 0x20, 0xd0,
            0xbf, 0xd0, 0xbe, 0xd0, 0xbb, 0xd1, 0x8c, 0xd0, 0xb7, 0xd0, 0xbe, 0xd0, 0xb2, 0xd0,
            0xb0, 0xd1, 0x82, 0xd0, 0xb5, 0xd0, 0xbb, 0xd1, 0x8c, 0xd0, 0x28, 0x00, 0x00, 0x11,
            0x22, 0x33, 0x44, 0x58, 0x54, 0x61, 0x62, 0x6c, 0x65, 0x74, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];
        let (remaining, parsed) =
            DualConnectionsDevicePacket::take::<VerboseError<_>>(&initial).unwrap();
        assert_eq!(remaining.len(), 0);
        assert_eq!(parsed.devices.len(), 4);
        assert_eq!(parsed.devices[0].name, "Laptop");
        assert!(parsed.devices[0].is_connected);
        assert_eq!(parsed.devices[1].name, "Phone");
        assert!(parsed.devices[1].is_connected);
        // The next character starts at byte 32 and is cut off.
        assert_eq!(parsed.devices[2].name, "Device пользователь");
        assert!(!parsed.devices[2].is_connected);
        assert_eq!(parsed.devices[3].name, "Tablet");
    }

    #[test]
    fn multiple_devices_in_one_packet() {
        let initial = [
            0x1, 0x1, // current/total
            0x28, 0x1, 0x66, 0x14, 0x92, 0x2c, 0x3a, 0xd4, 0x50, 0x69, 0x78, 0x65, 0x6c, 0x20,
            0x36, 0x61, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
            0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, // device 1
            0x28, 0x1, 0x67, 0x14, 0x92, 0x2c, 0x3a, 0xd4, 0x50, 0x69, 0x78, 0x65, 0x6c, 0x20,
            0x37, 0x61, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
            0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, // device 2
        ];
        let (remaining, parsed) =
            DualConnectionsDevicePacket::take::<VerboseError<_>>(&initial).unwrap();
        assert_eq!(remaining.len(), 0);
        assert_eq!(parsed.devices[0].name, "Pixel 6a");
        assert_eq!(parsed.devices[1].name, "Pixel 7a");
    }
}
