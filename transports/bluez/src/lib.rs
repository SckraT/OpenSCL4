//! Linux RFCOMM transport for the device pilot, using the BlueZ system bus.

#![cfg(target_os = "linux")]
#![forbid(unsafe_code)]

use std::{future::Future, io, pin::Pin};

use async_trait::async_trait;
use bluer::{
    Address, Device, ErrorKind, InternalErrorKind, Session,
    rfcomm::{
        Profile, ProfileHandle, ReqError, Role, Stream,
        stream::{OwnedReadHalf, OwnedWriteHalf},
    },
};
use bytes::Bytes;
use futures::StreamExt;
use openscl4_transport::{
    ConnectTarget, LinkRx, LinkTx, RedactedMac, TransportError, TransportLink, TransportProvider,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    sync::watch,
};
use uuid::{Uuid, uuid};

const SERIAL_PORT_UUID: Uuid = uuid!("00001101-0000-1000-8000-00805f9b34fb");
const SOUNDCORE_UUID: u128 = 0x0cf12d31_fac3_4553_bd80_d6832e700000;
const SOUNDCORE_UUID_MASK: u128 = 0xffffffff_ffff_ffff_ffff_fffffff00000;
const READ_CAPACITY: usize = 4096;

type ProfileClosed = Pin<Box<dyn Future<Output = ()> + Send>>;

/// Opens the Soundcore control profile of one explicitly selected BlueZ device.
pub struct BluezRfcommTransport {
    session: Session,
    device: Device,
}

impl BluezRfcommTransport {
    /// Resolves the name shown in Linux Bluetooth settings to one paired, connected device.
    pub async fn for_connected_device(name: &str) -> Result<(Self, ConnectTarget), TransportError> {
        let session = Session::new().await.map_err(classify_bluez)?;
        let names = session.adapter_names().await.map_err(classify_bluez)?;
        if names.is_empty() {
            return Err(TransportError::Unavailable);
        }

        let mut found = None;
        for name_of_adapter in names {
            let adapter = session.adapter(&name_of_adapter).map_err(classify_bluez)?;
            for address in adapter.device_addresses().await.map_err(classify_bluez)? {
                let device = adapter.device(address).map_err(classify_bluez)?;
                match matches_device(&device, name).await {
                    Ok(true) if found.is_some() => return Err(TransportError::AmbiguousDevice),
                    Ok(true) => found = Some(device),
                    Ok(false) | Err(TransportError::DeviceNotFound) => {}
                    Err(error) => return Err(error),
                }
            }
        }
        let device = found.ok_or(TransportError::DeviceNotFound)?;
        let target = ConnectTarget {
            mac: RedactedMac::from_octets(device.address().0),
        };
        Ok((Self { session, device }, target))
    }
}

async fn matches_device(device: &Device, name: &str) -> Result<bool, TransportError> {
    Ok(device
        .alias()
        .await
        .map_err(classify_bluez)?
        .eq_ignore_ascii_case(name)
        && device.is_paired().await.map_err(classify_bluez)?
        && device.is_connected().await.map_err(classify_bluez)?)
}

#[async_trait]
impl TransportProvider for BluezRfcommTransport {
    async fn connect(
        &self,
        target: &ConnectTarget,
    ) -> Result<Box<dyn TransportLink>, TransportError> {
        if self.device.address().0 != target.mac.octets() {
            return Err(TransportError::DeviceNotFound);
        }
        if !self.device.is_connected().await.map_err(classify_bluez)? {
            return Err(TransportError::Disconnected);
        }
        let uuids = self.device.uuids().await.map_err(classify_bluez)?;
        let uuid =
            select_service(uuids.into_iter().flatten()).ok_or(TransportError::Unavailable)?;
        let mut profile = self
            .session
            .register_profile(Profile {
                uuid,
                role: Some(Role::Client),
                require_authentication: Some(true),
                ..Profile::default()
            })
            .await
            .map_err(classify_bluez)?;

        // BlueZ calls NewConnection while ConnectProfile is still pending.
        let connect = async {
            self.device
                .connect_profile(&uuid)
                .await
                .map_err(classify_bluez)
        };
        let accept = accept_profile(&mut profile, self.device.address());
        let ((), (stream, peer_closed)) = tokio::try_join!(connect, accept)?;
        Ok(Box::new(BluezLink {
            stream,
            profile,
            peer_closed,
        }))
    }
}

fn select_service(uuids: impl IntoIterator<Item = Uuid>) -> Option<Uuid> {
    let mut fallback = None;
    for uuid in uuids {
        if uuid.as_u128() & SOUNDCORE_UUID_MASK == SOUNDCORE_UUID {
            return Some(uuid);
        }
        if uuid == SERIAL_PORT_UUID {
            fallback = Some(uuid);
        }
    }
    fallback
}

async fn accept_profile(
    profile: &mut ProfileHandle,
    address: Address,
) -> Result<(Stream, ProfileClosed), TransportError> {
    while let Some(request) = profile.next().await {
        if request.device() == address {
            let peer_closed = Box::pin(request.closed());
            let stream = request.accept().map_err(classify_bluez)?;
            return Ok((stream, peer_closed));
        }
        request.reject(ReqError::Rejected);
    }
    Err(TransportError::Disconnected)
}

struct BluezLink {
    stream: Stream,
    profile: ProfileHandle,
    peer_closed: ProfileClosed,
}

impl TransportLink for BluezLink {
    fn split(self: Box<Self>) -> (Box<dyn LinkTx>, Box<dyn LinkRx>) {
        let (reader, writer) = self.stream.into_split();
        let (close, closing) = watch::channel(false);
        (
            Box::new(BluezTx {
                // The daemon can request disconnection while the RFCOMM stream is active.
                link: Some((writer, self.profile)),
                close,
            }),
            Box::new(BluezRx {
                reader,
                closing,
                peer_closed: self.peer_closed,
                disconnected: false,
            }),
        )
    }
}

struct BluezTx {
    link: Option<(OwnedWriteHalf, ProfileHandle)>,
    close: watch::Sender<bool>,
}

#[async_trait]
impl LinkTx for BluezTx {
    async fn send(&mut self, data: Bytes) -> Result<(), TransportError> {
        let (writer, _) = self.link.as_mut().ok_or(TransportError::Closed)?;
        writer.write_all(&data).await.map_err(classify_io)
    }

    async fn close(&mut self) {
        self.close.send_replace(true);
        if let Some((mut writer, _profile)) = self.link.take() {
            let _ = writer.shutdown().await;
        }
    }
}

struct BluezRx {
    reader: OwnedReadHalf,
    closing: watch::Receiver<bool>,
    peer_closed: ProfileClosed,
    disconnected: bool,
}

#[async_trait]
impl LinkRx for BluezRx {
    async fn recv(&mut self) -> Result<Bytes, TransportError> {
        if self.disconnected {
            return Err(TransportError::Disconnected);
        }
        let mut data = vec![0; READ_CAPACITY];
        let length = tokio::select! {
            biased;
            _ = self.closing.wait_for(|closed| *closed) => return Err(TransportError::Closed),
            () = &mut self.peer_closed => {
                self.disconnected = true;
                return Err(TransportError::Disconnected);
            },
            result = self.reader.read(&mut data) => result.map_err(classify_io)?,
        };
        if length == 0 {
            return Err(TransportError::Disconnected);
        }
        data.truncate(length);
        Ok(Bytes::from(data))
    }
}

fn classify_bluez(error: bluer::Error) -> TransportError {
    match error.kind {
        ErrorKind::DoesNotExist | ErrorKind::NotFound => TransportError::DeviceNotFound,
        ErrorKind::NotAuthorized
        | ErrorKind::NotPermitted
        | ErrorKind::AuthenticationFailed
        | ErrorKind::AuthenticationRejected => TransportError::AccessDenied,
        ErrorKind::AlreadyConnected | ErrorKind::AlreadyExists | ErrorKind::InProgress => {
            TransportError::Busy
        }
        ErrorKind::NotReady | ErrorKind::NotAvailable | ErrorKind::NotSupported => {
            TransportError::Unavailable
        }
        ErrorKind::ConnectionAttemptFailed => TransportError::Disconnected,
        ErrorKind::Failed if error.message == "br-connection-busy" => TransportError::Busy,
        ErrorKind::Failed if error.message == "br-connection-profile-unavailable" => {
            TransportError::Unavailable
        }
        ErrorKind::Internal(InternalErrorKind::Io(kind)) => classify_io(kind.into()),
        ErrorKind::Internal(InternalErrorKind::DBus(name)) => match name.as_str() {
            "org.freedesktop.DBus.Error.AccessDenied" => TransportError::AccessDenied,
            "org.freedesktop.DBus.Error.ServiceUnknown"
            | "org.freedesktop.DBus.Error.NameHasNoOwner" => TransportError::Unavailable,
            _ => TransportError::Io,
        },
        _ => TransportError::Io,
    }
}

fn classify_io(error: io::Error) -> TransportError {
    match error.kind() {
        io::ErrorKind::ConnectionAborted
        | io::ErrorKind::ConnectionReset
        | io::ErrorKind::BrokenPipe
        | io::ErrorKind::NotConnected
        | io::ErrorKind::UnexpectedEof => TransportError::Disconnected,
        io::ErrorKind::AddrInUse | io::ErrorKind::ResourceBusy => TransportError::Busy,
        io::ErrorKind::PermissionDenied => TransportError::AccessDenied,
        _ => TransportError::Io,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selects_serial_port_only_as_a_fallback() {
        let vendor = uuid!("0cf12d31-fac3-4553-bd80-d6832e739540");
        let audio = uuid!("0000110b-0000-1000-8000-00805f9b34fb");
        assert_eq!(
            select_service([SERIAL_PORT_UUID, audio, vendor]),
            Some(vendor)
        );
        assert_eq!(
            select_service([audio, SERIAL_PORT_UUID]),
            Some(SERIAL_PORT_UUID)
        );
        assert_eq!(select_service([audio]), None);
    }

    #[test]
    fn classifies_bluez_and_socket_failures() {
        for (kind, message, expected) in [
            (
                ErrorKind::Failed,
                "br-connection-busy",
                TransportError::Busy,
            ),
            (ErrorKind::NotAuthorized, "", TransportError::AccessDenied),
            (ErrorKind::NotFound, "", TransportError::DeviceNotFound),
            (ErrorKind::NotAvailable, "", TransportError::Unavailable),
            (
                ErrorKind::ConnectionAttemptFailed,
                "",
                TransportError::Disconnected,
            ),
        ] {
            assert_eq!(
                classify_bluez(bluer::Error {
                    kind,
                    message: message.into()
                }),
                expected
            );
        }
        assert_eq!(
            classify_io(io::ErrorKind::BrokenPipe.into()),
            TransportError::Disconnected
        );
    }
}
