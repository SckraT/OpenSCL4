#![cfg(target_os = "linux")]
#![forbid(unsafe_code)]

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        os::fd::OwnedFd,
        os::unix::net::UnixStream as StdUnixStream,
        process::Command,
        sync::{Arc, Mutex},
        time::Duration,
    };

    use bytes::Bytes;
    use openscl4_transport::{TransportError, TransportProvider};
    use openscl4_transport_bluez::BluezRfcommTransport;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::UnixStream,
        sync::mpsc,
        time::timeout,
    };
    use zbus::{
        Connection,
        fdo::{self, ObjectManager},
        message::Header,
        zvariant::{ObjectPath, OwnedValue},
    };

    const TEST_BUS: &str = "OPEN_SCL4_BLUEZ_TEST_BUS";
    const DEVICE_NAME: &str = "Test Liberty 4 Pro";
    const DEVICE_PATH: &str = "/org/bluez/hci0/dev_12_23_34_45_56_67";
    const VENDOR_SERVICE: &str = "0cf12d31-fac3-4553-bd80-d6832e739540";
    const SERIAL_PORT: &str = "00001101-0000-1000-8000-00805f9b34fb";

    #[test]
    fn opens_a_profile_on_a_private_bus_and_releases_its_socket() {
        if std::env::var_os(TEST_BUS).is_none() {
            let status = Command::new("dbus-run-session")
                .args([
                    "--",
                    "sh",
                    "-c",
                    "DBUS_SYSTEM_BUS_ADDRESS=\"$DBUS_SESSION_BUS_ADDRESS\" exec \"$@\"",
                    "openscl4-bluez-test",
                ])
                .arg(std::env::current_exe().expect("test executable"))
                .args([
                    "--exact",
                    "tests::opens_a_profile_on_a_private_bus_and_releases_its_socket",
                    "--nocapture",
                ])
                .env(TEST_BUS, "1")
                .status()
                .expect("start a private D-Bus session");
            assert!(status.success());
            return;
        }

        assert!(std::env::var_os("DBUS_SYSTEM_BUS_ADDRESS").is_some());
        assert_eq!(
            std::env::var_os("DBUS_SYSTEM_BUS_ADDRESS"),
            std::env::var_os("DBUS_SESSION_BUS_ADDRESS"),
        );
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("test runtime")
            .block_on(async {
                timeout(Duration::from_secs(8), exchange())
                    .await
                    .expect("profile exchange completes")
                    .expect("private BlueZ profile works");
            });
    }

    async fn exchange() -> Result<(), Box<dyn std::error::Error>> {
        let registration = Registration::default();
        let (peer_sender, mut peers) = mpsc::channel(1);
        let server = zbus::connection::Builder::system()?
            .name("org.bluez")?
            .serve_at("/", ObjectManager)?
            .serve_at("/org/bluez", ProfileManager(registration.clone()))?
            .serve_at("/org/bluez/hci0", FakeAdapter)?
            .serve_at(
                DEVICE_PATH,
                FakeDevice {
                    registration: registration.clone(),
                    peer_sender,
                },
            )?
            .build()
            .await?;

        let (provider, target) = BluezRfcommTransport::for_connected_device(DEVICE_NAME).await?;
        let (mut tx, mut rx) = provider.connect(&target).await?.split();
        let peer = peers.recv().await.expect("receive the other socket half");
        peer.set_nonblocking(true)?;
        let mut peer = UnixStream::from_std(peer)?;

        tx.send(Bytes::from_static(b"request")).await?;
        let mut request = [0; 7];
        peer.read_exact(&mut request).await?;
        assert_eq!(&request, b"request");
        peer.write_all(b"x").await?;
        assert_eq!(rx.recv().await?, Bytes::from_static(b"x"));

        let pending = tokio::spawn(async move { rx.recv().await });
        tokio::task::yield_now().await;
        tx.close().await;
        assert_eq!(pending.await?, Err(TransportError::Closed));
        assert_eq!(
            tx.send(Bytes::from_static(b"x")).await,
            Err(TransportError::Closed)
        );
        let mut remaining = [0];
        assert_eq!(peer.read(&mut remaining).await?, 0);

        while registration.lock().expect("registration lock").is_some() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        let (mut tx, mut rx) = provider.connect(&target).await?.split();
        let _remote_peer = peers.recv().await.expect("second peer socket");
        let (owner, profile) = registration
            .lock()
            .expect("registration lock")
            .clone()
            .expect("new profile");
        server
            .call_method(
                Some(owner.as_str()),
                profile.as_str(),
                Some("org.bluez.Profile1"),
                "RequestDisconnection",
                &(ObjectPath::try_from(DEVICE_PATH)?,),
            )
            .await?;
        assert_eq!(rx.recv().await, Err(TransportError::Disconnected));
        assert_eq!(rx.recv().await, Err(TransportError::Disconnected));
        tx.close().await;
        Ok(())
    }

    type Registration = Arc<Mutex<Option<(String, String)>>>;

    struct ProfileManager(Registration);

    #[zbus::interface(name = "org.bluez.ProfileManager1")]
    impl ProfileManager {
        fn register_profile(
            &self,
            path: ObjectPath<'_>,
            uuid: &str,
            options: HashMap<String, OwnedValue>,
            #[zbus(header)] header: Header<'_>,
        ) {
            assert_eq!(uuid, VENDOR_SERVICE);
            assert_eq!(
                options
                    .get("Role")
                    .and_then(|role| <&str>::try_from(role).ok()),
                Some("client")
            );
            assert_eq!(
                options
                    .get("RequireAuthentication")
                    .and_then(|value| bool::try_from(value).ok()),
                Some(true)
            );
            let owner = header.sender().expect("profile owner").as_str().to_owned();
            *self.0.lock().expect("registration lock") = Some((owner, path.to_string()));
        }

        fn unregister_profile(&self, _path: ObjectPath<'_>) {
            self.0.lock().expect("registration lock").take();
        }
    }

    struct FakeAdapter;

    #[zbus::interface(name = "org.bluez.Adapter1")]
    impl FakeAdapter {
        #[zbus(property)]
        fn powered(&self) -> bool {
            true
        }
    }

    struct FakeDevice {
        registration: Registration,
        peer_sender: mpsc::Sender<StdUnixStream>,
    }

    #[zbus::interface(name = "org.bluez.Device1")]
    impl FakeDevice {
        #[zbus(property)]
        fn alias(&self) -> &str {
            DEVICE_NAME
        }

        #[zbus(property)]
        fn paired(&self) -> bool {
            true
        }

        #[zbus(property)]
        fn connected(&self) -> bool {
            true
        }

        #[zbus(property, name = "UUIDs")]
        fn uuids(&self) -> Vec<&str> {
            vec![SERIAL_PORT, VENDOR_SERVICE]
        }

        async fn connect_profile(
            &self,
            uuid: &str,
            #[zbus(connection)] bus: &Connection,
        ) -> fdo::Result<()> {
            assert_eq!(uuid, VENDOR_SERVICE);
            let (owner, profile) = self
                .registration
                .lock()
                .expect("registration lock")
                .clone()
                .expect("registered profile");
            let (local, peer) = StdUnixStream::pair().expect("connected Unix socket pair");
            let fd: OwnedFd = local.into();
            bus.call_method(
                Some(owner.as_str()),
                profile.as_str(),
                Some("org.bluez.Profile1"),
                "NewConnection",
                &(
                    ObjectPath::try_from(DEVICE_PATH).expect("fixture path"),
                    zbus::zvariant::OwnedFd::from(fd),
                    HashMap::<String, OwnedValue>::new(),
                ),
            )
            .await?;
            self.peer_sender
                .send(peer)
                .await
                .expect("deliver the peer socket");
            Ok(())
        }
    }
}
