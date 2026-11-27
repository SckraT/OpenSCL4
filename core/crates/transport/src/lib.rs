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
    /// The Soundcore RFCOMM service seen during discovery, when the platform
    /// reports it.
    ///
    /// The D1202 family needs this connection metadata to select between its
    /// two public codes: both report the same state serial and the same
    /// protocol formats.
    pub service: Option<[u8; 16]>,
}

impl ConnectTarget {
    /// Creates a target with the address alone.
    pub const fn from_mac(mac: RedactedMac) -> Self {
        Self { mac, service: None }
    }

    /// Adds the Soundcore RFCOMM service seen during discovery.
    pub const fn with_service(self, service: [u8; 16]) -> Self {
        Self {
            mac: self.mac,
            service: Some(service),
        }
    }
}

/// A transport failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum TransportError {
    /// The connection was lost.
    #[error("transport link disconnected")]
    Disconnected,
    /// The link has been closed.
    #[error("transport link closed")]
    Closed,
    /// No connected device matched the selection.
    #[error("Bluetooth device not found")]
    DeviceNotFound,
    /// More than one connected device matched the selection.
    #[error("more than one connected Bluetooth device has that name")]
    AmbiguousDevice,
    /// Windows or the device refused Bluetooth access.
    #[error("Bluetooth access denied")]
    AccessDenied,
    /// The requested Bluetooth connection is already in use.
    #[error("Bluetooth connection is in use")]
    Busy,
    /// The requested Bluetooth transport or service is unavailable.
    #[error("Bluetooth transport unavailable")]
    Unavailable,
    /// An unclassified transport I/O failure.
    #[error("transport I/O failed")]
    Io,
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
