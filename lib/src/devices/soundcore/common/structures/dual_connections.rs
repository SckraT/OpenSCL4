use macaddr::MacAddr6;
use nom::{
    IResult, Parser,
    bytes::complete::take,
    error::{ContextError, ParseError, context},
    number::complete::le_u8,
};

use crate::devices::soundcore::common::packet::parsing::take_bool;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DualConnections {
    pub is_enabled: bool,
    pub devices: Vec<DualConnectionsDevice>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DualConnectionsDevice {
    pub is_connected: bool,
    pub mac_address: MacAddr6,
    pub name: String,
}

impl DualConnectionsDevice {
    pub fn take<'a, E: ParseError<&'a [u8]> + ContextError<&'a [u8]>>(
        input: &'a [u8],
    ) -> IResult<&'a [u8], Self, E> {
        context("dual connection device", |input| {
            let (input, (length, is_connected, mac_address_bytes)) = (
                le_u8, // number of bytes from here (including this byte) to the end. name is remaining_length-8.
                take_bool,
                (le_u8, le_u8, le_u8, le_u8, le_u8, le_u8),
            )
                .parse_complete(input)?;
            // Name, right padded with 0s. Clamped to what's actually left: real devices have been
            // observed sending a lone trailing entry (alone in the final packet of a multi-packet
            // response) padded a few bytes short of what its own length byte implies.
            let declared_name_length = usize::from(length.saturating_sub(8));
            let name_length = declared_name_length.min(input.len());
            if name_length < declared_name_length {
                tracing::warn!(
                    "dual connections device length byte claims {declared_name_length} name bytes, only {name_length} remain, truncating"
                );
            }
            let (input, name_bytes) = take(name_length).parse_complete(input)?;

            let mac_address = MacAddr6::from(<[u8; 6]>::from(mac_address_bytes));

            // Name without 0 padding
            let name_str_bytes = name_bytes
                .iter()
                .position(|b| *b == 0)
                .map_or(name_bytes, |index| &name_bytes[..index]);
            // Bluetooth device names are supposed to be utf8, but the device
            // pads them to the 32-byte name field and can cut a multi-byte
            // character in half. Keep the valid prefix: dropping the whole
            // packet loses every device in the list for one truncated name.
            let name = match str::from_utf8(name_str_bytes) {
                Ok(name) => name.to_owned(),
                Err(error) => {
                    tracing::debug!(
                        "device name ends mid-UTF-8 character at byte {}, truncating",
                        error.valid_up_to()
                    );
                    String::from_utf8_lossy(&name_str_bytes[..error.valid_up_to()]).into_owned()
                }
            };

            Ok((
                input,
                Self {
                    is_connected,
                    mac_address,
                    name,
                },
            ))
        })
        .parse_complete(input)
    }

    pub fn bytes(&self) -> impl Iterator<Item = u8> {
        [0x28, self.is_connected as u8]
            .into_iter()
            .chain(self.mac_address.into_array())
            .chain(self.name.as_bytes().iter().take(32).copied())
            .chain(std::iter::repeat_n(
                0,
                32usize.saturating_sub(self.name.len()),
            ))
    }
}
