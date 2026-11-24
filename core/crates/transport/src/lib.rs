//! Platform-independent byte transport for device connections.

#![forbid(unsafe_code)]

use std::fmt;

use async_trait::async_trait;
use bytes::Bytes;

/// A device address whose diagnostic formatting hides the address.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct RedactedMac([u8; 6]);

impl RedactedMac {
    /// Wraps the six bytes returned by Bluetooth discovery.
    pub const fn from_octets(octets: [u8; 6]) -> Self {
        Self(octets)
    }

    /// Returns the bytes needed by a transport to address the device.
    pub const fn octets(self) -> [u8; 6] {
        self.0
    }
}

impl fmt::Debug for RedactedMac {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<redacted-mac>")
    }
}

impl fmt::Display for RedactedMac {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

/// The device to connect to.
pub struct ConnectTarget {
    /// Bluetooth address of the target device.
    pub mac: RedactedMac,
}

/// A transport failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum TransportError {
    /// The link has been closed.
    #[error("transport link closed")]
    Closed,
}

/// Opens a full-duplex byte link to a device.
#[async_trait]
pub trait TransportProvider: Send + Sync + 'static {
    /// Opens a new link for the target.
    async fn connect(
        &self,
        target: &ConnectTarget,
    ) -> Result<Box<dyn TransportLink>, TransportError>;
}

/// An open device connection.
pub trait TransportLink: Send + 'static {
    /// Splits the connection into independently usable send and receive halves.
    fn split(self: Box<Self>) -> (Box<dyn LinkTx>, Box<dyn LinkRx>);
}

/// The sending half of a connection.
#[async_trait]
pub trait LinkTx: Send + 'static {
    /// Sends a chunk of bytes.
    async fn send(&mut self, data: Bytes) -> Result<(), TransportError>;

    /// Closes the link and releases any pending receive.
    async fn close(&mut self);
}

/// The receiving half of a connection.
#[async_trait]
pub trait LinkRx: Send + 'static {
    /// Receives a byte chunk. Chunk boundaries are transport-specific.
    async fn recv(&mut self) -> Result<Bytes, TransportError>;
}

#[cfg(test)]
mod tests {
    use super::RedactedMac;

    #[test]
    fn device_address_is_redacted_when_formatted() {
        let address = RedactedMac::from_octets([2, 17, 34, 51, 68, 85]);

        assert_eq!(format!("{address:?}"), "<redacted-mac>");
        assert_eq!(format!("{address}"), "<redacted-mac>");
    }
}
