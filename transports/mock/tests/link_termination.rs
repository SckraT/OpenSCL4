#![forbid(unsafe_code)]

#[cfg(test)]
mod tests {
    use std::{
        future::{Future, poll_fn},
        pin::pin,
        time::Duration,
    };

    use bytes::Bytes;
    use openscl4_transport::{ConnectTarget, RedactedMac, TransportError, TransportProvider};
    use openscl4_transport_mock::MockTransport;
    use tokio::{sync::oneshot, task::JoinHandle, time::timeout};

    const DEADLINE: Duration = Duration::from_secs(5);

    #[tokio::test]
    async fn simulated_disconnect_releases_pending_read() {
        let (link, disconnect) = MockTransport.connect_with_fault_injection();
        let (_tx, mut rx) = link.split();
        let read = start_pending(async move { rx.recv().await }).await;

        disconnect.disconnect();

        let result = timeout(DEADLINE, read)
            .await
            .expect("read completes after disconnect")
            .expect("read task succeeds");
        assert_eq!(result, Err(TransportError::Disconnected));
    }

    #[tokio::test]
    async fn simulated_disconnect_releases_a_send_waiting_for_capacity() {
        let (link, disconnect) = MockTransport.connect_with_fault_injection();
        let (mut tx, _rx) = link.split();
        let send = start_pending(async move {
            for _ in 0..32 {
                tx.send(Bytes::new()).await?;
            }
            Ok(())
        })
        .await;

        disconnect.disconnect();

        let result = timeout(DEADLINE, send)
            .await
            .expect("send completes after disconnect")
            .expect("send task succeeds");
        assert_eq!(result, Err(TransportError::Disconnected));
    }

    #[tokio::test]
    async fn explicit_close_releases_pending_read_and_rejects_sends() {
        let target = ConnectTarget {
            mac: RedactedMac::from_octets([0; 6]),
        };
        let link = MockTransport
            .connect(&target)
            .await
            .expect("open mock link");
        let (mut tx, mut rx) = link.split();
        let read = start_pending(async move { rx.recv().await }).await;

        timeout(DEADLINE, tx.close())
            .await
            .expect("close completes");

        let result = timeout(DEADLINE, read)
            .await
            .expect("read completes after close")
            .expect("read task succeeds");
        assert_eq!(result, Err(TransportError::Closed));
        assert_eq!(tx.send(Bytes::new()).await, Err(TransportError::Closed));
    }

    async fn start_pending<T: Send + 'static>(
        operation: impl Future<Output = T> + Send + 'static,
    ) -> JoinHandle<T> {
        let (pending, ready) = oneshot::channel();
        let task = tokio::spawn(async move {
            let mut operation = pin!(operation);
            let mut pending = Some(pending);
            poll_fn(|context| {
                let result = operation.as_mut().poll(context);
                if let Some(pending) = pending.take() {
                    assert!(result.is_pending(), "operation must initially wait");
                    pending.send(()).expect("notify the test");
                }
                result
            })
            .await
        });
        timeout(DEADLINE, ready)
            .await
            .expect("operation becomes pending")
            .expect("operation reached its first poll");
        task
    }
}
