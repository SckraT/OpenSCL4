//! Windows Bluetooth RFCOMM transport for the device pilot.

#![cfg(windows)]
#![forbid(unsafe_code)]

use async_trait::async_trait;
use bytes::Bytes;
use openscl4_transport::{
    ConnectTarget, LinkRx, LinkTx, RedactedMac, TransportError, TransportLink, TransportProvider,
};
use windows::{
    Devices::{
        Bluetooth::{
            BluetoothConnectionStatus, BluetoothDevice, BluetoothError, Rfcomm::RfcommDeviceService,
        },
        Enumeration::DeviceInformation,
    },
    Networking::Sockets::{SocketProtectionLevel, StreamSocket},
    Storage::Streams::{Buffer, DataReader, DataWriter, IInputStream, InputStreamOptions},
    core::{AgileReference, HSTRING},
};

const SERIAL_PORT_UUID: u128 = 0x00001101_0000_1000_8000_00805f9b34fb;
const SOUNDCORE_UUID: u128 = 0x0cf12d31_fac3_4553_bd80_d6832e700000;
const SOUNDCORE_UUID_MASK: u128 = 0xffffffff_ffff_ffff_ffff_fffffff00000;
const READ_CAPACITY: u32 = 4096;

/// Resolves an explicitly selected, connected Classic Bluetooth device by its Windows name.
pub async fn connected_target(name: &str) -> Result<ConnectTarget, TransportError> {
    let connected = BluetoothDevice::GetDeviceSelectorFromConnectionStatus(
        BluetoothConnectionStatus::Connected,
    )
    .map_err(classify_error)?;
    let filter = HSTRING::from(format!(
        "{connected} AND System.Devices.Aep.IsPresent:=System.StructuredQueryType.Boolean#True"
    ));
    let devices = DeviceInformation::FindAllAsyncAqsFilter(&filter)
        .map_err(classify_error)?
        .await
        .map_err(classify_error)?;

    let mut found = None;
    for device in devices {
        if device
            .Name()
            .map_err(classify_error)?
            .to_string_lossy()
            .eq_ignore_ascii_case(name)
        {
            let device = BluetoothDevice::FromIdAsync(&device.Id().map_err(classify_error)?)
                .map_err(classify_error)?
                .await
                .map_err(classify_error)?;
            let address = device.BluetoothAddress().map_err(classify_error)?;
            if found.is_some_and(|previous| previous != address) {
                return Err(TransportError::AmbiguousDevice);
            }
            found = Some(address);
        }
    }

    let address = found.ok_or(TransportError::DeviceNotFound)?;
    let service = soundcore_service_uuid(address).await;
    Ok(ConnectTarget {
        mac: RedactedMac::from_octets(
            address.to_be_bytes()[2..]
                .try_into()
                .map_err(|_| TransportError::Io)?,
        ),
        service,
    })
}

/// Returns the Soundcore RFCOMM service UUID seen during discovery, when the
/// device reports one. The D1202 family needs it to select its public code.
async fn soundcore_service_uuid(address: u64) -> Option<[u8; 16]> {
    let device = BluetoothDevice::FromBluetoothAddressAsync(address)
        .map_err(classify_error)
        .ok()?
        .await
        .ok()?;
    let service = soundcore_service(&device).await.ok()?;
    let uuid = service
        .ServiceId()
        .and_then(|id| id.Uuid())
        .map_err(classify_error)
        .ok()?;
    Some(uuid.to_u128().to_be_bytes())
}

/// Opens and owns a Windows socket without exposing device identifiers or raw packet data.
pub struct WindowsRfcommTransport;

#[async_trait]
impl TransportProvider for WindowsRfcommTransport {
    async fn connect(
        &self,
        target: &ConnectTarget,
    ) -> Result<Box<dyn TransportLink>, TransportError> {
        let mut address = [0; 8];
        address[2..].copy_from_slice(&target.mac.octets());
        let device = BluetoothDevice::FromBluetoothAddressAsync(u64::from_be_bytes(address))
            .map_err(classify_error)?
            .await
            .map_err(classify_error)?;
        tracing::debug!("resolved connected Bluetooth device");
        let service = soundcore_service(&device).await?;
        tracing::debug!("selected Bluetooth RFCOMM service");
        let socket = Socket(StreamSocket::new().map_err(classify_error)?);
        socket
            .0
            .ConnectWithProtectionLevelAsync(
                &service.ConnectionHostName().map_err(classify_error)?,
                &service.ConnectionServiceName().map_err(classify_error)?,
                SocketProtectionLevel::BluetoothEncryptionAllowNullAuthentication,
            )
            .map_err(classify_error)?
            .await
            .map_err(classify_error)?;
        tracing::debug!("opened Bluetooth RFCOMM socket");
        Ok(Box::new(WindowsLink::new(socket)?))
    }
}

async fn soundcore_service(
    device: &BluetoothDevice,
) -> Result<RfcommDeviceService, TransportError> {
    let result = device
        .GetRfcommServicesAsync()
        .map_err(classify_error)?
        .await
        .map_err(classify_error)?;
    match result.Error().map_err(classify_error)? {
        BluetoothError::Success => {}
        BluetoothError::DeviceNotConnected => return Err(TransportError::Disconnected),
        BluetoothError::ResourceInUse => return Err(TransportError::Busy),
        BluetoothError::ConsentRequired | BluetoothError::DisabledByPolicy => {
            return Err(TransportError::AccessDenied);
        }
        _ => return Err(TransportError::Unavailable),
    }

    let mut serial_port = None;
    for service in result.Services().map_err(classify_error)? {
        let uuid = service
            .ServiceId()
            .and_then(|id| id.Uuid())
            .map_err(classify_error)?
            .to_u128();
        if uuid & SOUNDCORE_UUID_MASK == SOUNDCORE_UUID {
            return Ok(service);
        }
        if uuid == SERIAL_PORT_UUID {
            serial_port = Some(service);
        }
    }
    serial_port.ok_or(TransportError::Unavailable)
}

struct Socket(StreamSocket);

impl Drop for Socket {
    fn drop(&mut self) {
        // Closing the socket also aborts pending reads, including on connect cancellation.
        let _ = self.0.Close();
    }
}

struct WindowsLink {
    socket: Socket,
    input: AgileReference<IInputStream>,
}

impl WindowsLink {
    fn new(socket: Socket) -> Result<Self, TransportError> {
        let input = socket
            .0
            .InputStream()
            .and_then(|stream| AgileReference::new(&stream))
            .map_err(classify_error)?;
        Ok(Self { socket, input })
    }
}

impl TransportLink for WindowsLink {
    fn split(self: Box<Self>) -> (Box<dyn LinkTx>, Box<dyn LinkRx>) {
        (
            Box::new(WindowsTx(Some(self.socket))),
            Box::new(WindowsRx(self.input)),
        )
    }
}

struct WindowsTx(Option<Socket>);

#[async_trait]
impl LinkTx for WindowsTx {
    async fn send(&mut self, data: Bytes) -> Result<(), TransportError> {
        let socket = self.0.as_ref().ok_or(TransportError::Closed)?;
        let operation = {
            let stream = socket.0.OutputStream().map_err(classify_error)?;
            let writer = DataWriter::new().map_err(classify_error)?;
            writer.WriteBytes(&data).map_err(classify_error)?;
            let buffer = writer.DetachBuffer().map_err(classify_error)?;
            stream.WriteAsync(&buffer).map_err(classify_error)?
        };
        let written = operation.await.map_err(classify_error)?;
        if written as usize != data.len() {
            return Err(TransportError::Io);
        }
        Ok(())
    }

    async fn close(&mut self) {
        drop(self.0.take());
    }
}

struct WindowsRx(AgileReference<IInputStream>);

#[async_trait]
impl LinkRx for WindowsRx {
    async fn recv(&mut self) -> Result<Bytes, TransportError> {
        let operation = {
            let stream = self.0.resolve().map_err(classify_error)?;
            let buffer = Buffer::Create(READ_CAPACITY).map_err(classify_error)?;
            stream
                .ReadAsync(&buffer, READ_CAPACITY, InputStreamOptions::Partial)
                .map_err(classify_error)?
        };
        let payload = operation.await.map_err(classify_error)?;
        let length = payload.Length().map_err(classify_error)? as usize;
        if length == 0 {
            return Err(TransportError::Disconnected);
        }
        let mut data = vec![0; length];
        DataReader::FromBuffer(&payload)
            .and_then(|reader| reader.ReadBytes(&mut data))
            .map_err(classify_error)?;
        Ok(Bytes::from(data))
    }
}

fn classify_error(error: windows::core::Error) -> TransportError {
    let status = error.code().0 as u32;
    tracing::debug!(
        hresult = format_args!("{status:#010x}"),
        "Windows RFCOMM I/O error"
    );
    match status {
        0x8000_0013 | 0x8000_4004 | 0x8007_03e3 => TransportError::Closed, // RO_E_CLOSED, ERROR_OPERATION_ABORTED
        0x8007_0005 => TransportError::AccessDenied,
        0x8007_0490 => TransportError::DeviceNotFound,
        0x8007_2740 => TransportError::Busy, // WSAEADDRINUSE
        // Device not connected, connection aborted, or a Winsock connection loss.
        0x8007_048f | 0x8007_04d4 | 0x8007_2745 | 0x8007_2746 | 0x8007_2749 | 0x8007_274a => {
            TransportError::Disconnected
        }
        _ => TransportError::Io,
    }
}

#[cfg(test)]
mod tests {
    use std::{net::Ipv4Addr, time::Duration};

    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
        time::timeout,
    };
    use windows::{Networking::HostName, core::HRESULT};

    use super::*;

    #[test]
    fn classifies_closed_disconnect_and_access_failures_without_native_details() {
        for (code, expected) in [
            (0x8000_0013u32, TransportError::Closed),
            (0x8007_048f, TransportError::Disconnected),
            (0x8007_2746, TransportError::Disconnected),
            (0x8007_0005, TransportError::AccessDenied),
            (0x8007_0490, TransportError::DeviceNotFound),
            (0x8007_2740, TransportError::Busy),
            (0x8000_4005, TransportError::Io),
        ] {
            let error = windows::core::Error::from_hresult(HRESULT(code as i32));
            assert_eq!(classify_error(error), expected);
        }
    }

    #[tokio::test]
    async fn shares_socket_bytes_and_releases_a_pending_read_on_close() {
        timeout(Duration::from_secs(5), async {
            // TCP exercises the same WinRT stream operations without a Bluetooth radio.
            let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
            let socket = Socket(StreamSocket::new()?);
            socket
                .0
                .ConnectAsync(
                    &HostName::CreateHostName(&HSTRING::from("127.0.0.1"))?,
                    &HSTRING::from(listener.local_addr()?.port().to_string()),
                )?
                .await?;
            let (mut peer, _) = listener.accept().await?;
            let (mut tx, mut rx) = Box::new(WindowsLink::new(socket)?).split();

            tx.send(Bytes::from_static(b"request")).await?;
            let mut request = [0; 7];
            peer.read_exact(&mut request).await?;
            assert_eq!(&request, b"request");
            peer.write_all(b"x").await?;
            assert_eq!(rx.recv().await?, Bytes::from_static(b"x"));

            let pending_read = tokio::spawn(async move { rx.recv().await });
            tokio::task::yield_now().await;
            tx.close().await;
            assert!(matches!(
                pending_read.await?,
                Err(TransportError::Closed | TransportError::Disconnected)
            ));
            assert_eq!(
                tx.send(Bytes::from_static(b"x")).await,
                Err(TransportError::Closed)
            );
            let mut remaining = [0];
            assert_eq!(peer.read(&mut remaining).await?, 0);
            Ok::<_, Box<dyn std::error::Error>>(())
        })
        .await
        .expect("WinRT socket operations finish")
        .expect("WinRT socket exchanges bytes and closes");
    }
}
