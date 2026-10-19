#[cfg(target_os = "linux")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TrayStatus {
    pub host: bool,
    pub registered: bool,
}

fn is_our_item(item: &str, pid: u32) -> bool {
    item.starts_with(&format!("org.kde.StatusNotifierItem-{pid}-"))
}

#[cfg(target_os = "linux")]
async fn status(connection: &zbus::Connection) -> zbus::Result<TrayStatus> {
    let proxy = zbus::Proxy::new(
        connection,
        "org.kde.StatusNotifierWatcher",
        "/StatusNotifierWatcher",
        "org.kde.StatusNotifierWatcher",
    )
    .await?;
    let host = proxy
        .get_property::<bool>("IsStatusNotifierHostRegistered")
        .await?;
    let items = proxy
        .get_property::<Vec<String>>("RegisteredStatusNotifierItems")
        .await?;
    Ok(TrayStatus {
        host,
        registered: host
            && items
                .iter()
                .any(|item| is_our_item(item, std::process::id())),
    })
}

#[cfg(target_os = "linux")]
pub async fn watch(emit: impl Fn(TrayStatus)) {
    use std::time::Duration;

    let mut interval = tokio::time::interval(Duration::from_secs(2));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut connection = None;
    loop {
        interval.tick().await;
        if connection.is_none() {
            connection = tokio::time::timeout(Duration::from_secs(1), zbus::Connection::session())
                .await
                .ok()
                .and_then(Result::ok);
        }
        let current = if let Some(conn) = &connection {
            match tokio::time::timeout(Duration::from_secs(1), status(conn)).await {
                Ok(Ok(status)) => status,
                _ => {
                    connection = None;
                    TrayStatus::default()
                }
            }
        } else {
            TrayStatus::default()
        };
        emit(current);
    }
}

#[cfg(test)]
mod tests {
    use super::is_our_item;

    #[test]
    fn only_this_process_registration_counts() {
        assert!(is_our_item(
            "org.kde.StatusNotifierItem-42-1/StatusNotifierItem",
            42
        ));
        assert!(is_our_item("org.kde.StatusNotifierItem-42-3", 42));
        assert!(!is_our_item(
            "org.kde.StatusNotifierItem-420-1/StatusNotifierItem",
            42
        ));
        assert!(!is_our_item("org.kde.StatusNotifierItem-43-1", 42));
    }
}
