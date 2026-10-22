use std::time::Duration;

use async_trait::async_trait;
use openscq30_lib_has::MaybeHas;
use tokio::sync::watch;
use tokio::time::Instant;

use crate::{
    api::{
        connection::ConnectionStatus,
        device::{self, SettingWriteOutcome, WriteOutcome},
        settings::{SettingId, SettingMode, Value},
    },
    client::availability::{AvailabilityReason, SettingAvailability, setting_availability},
    devices::soundcore::common::{
        modules::ModuleCollection,
        packet::{self, PacketIOController, outbound::ToPacket},
        settings_manager::SettingHandlerError,
        structures::Flag,
    },
};

pub const MIN_WRITE_INTERVAL: Duration = Duration::from_millis(250);
pub const DEFAULT_CONFIRMATION_TIMEOUT: Duration = Duration::from_secs(2);
pub const LDAC_CONFIRMATION_TIMEOUT: Duration = Duration::from_secs(15);

const POLL_INTERVAL: Duration = Duration::from_millis(500);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlagVerification {
    Bulk,
    Targeted { command: packet::Command },
}

#[derive(Default)]
pub struct WriteThrottle {
    last_command: Option<Instant>,
}

impl WriteThrottle {
    pub async fn wait(&mut self) {
        if self.last_command.is_some() {
            tokio::time::sleep(MIN_WRITE_INTERVAL).await;
        }
        self.last_command = Some(Instant::now());
    }
}

pub enum TargetedRead {
    Observed(Option<Value>),
    TimedOut,
    Disconnected,
}

#[async_trait]
pub trait FlagAccess<StateType>: Send + Sync {
    fn set_command(&self) -> packet::Command;
    fn raw_value(&self, state: &StateType) -> Option<bool>;
    fn set_raw_value(&self, state: &mut StateType, value: bool);
    fn verification(&self) -> FlagVerification;
    async fn verify_targeted(
        &self,
        packet_io: &PacketIOController,
        state_sender: &watch::Sender<StateType>,
    ) -> TargetedRead;
}

pub struct FlagAccessor<FlagT> {
    set_command: packet::Command,
    is_inverted: bool,
    verification: FlagVerification,
    _flag: std::marker::PhantomData<FlagT>,
}

impl<FlagT> FlagAccessor<FlagT> {
    pub fn new(
        set_command: packet::Command,
        is_inverted: bool,
        verification: FlagVerification,
    ) -> Self {
        Self {
            set_command,
            is_inverted,
            verification,
            _flag: std::marker::PhantomData,
        }
    }
}

#[async_trait]
impl<FlagT, StateType> FlagAccess<StateType> for FlagAccessor<FlagT>
where
    StateType: MaybeHas<FlagT> + Send + Sync,
    FlagT: Flag + Send + Sync,
{
    fn set_command(&self) -> packet::Command {
        self.set_command
    }

    fn raw_value(&self, state: &StateType) -> Option<bool> {
        state.maybe_get().map(Flag::get_bool)
    }

    fn set_raw_value(&self, state: &mut StateType, value: bool) {
        if let Some(flag) = state.maybe_get_mut() {
            flag.set_bool(value);
        }
    }

    fn verification(&self) -> FlagVerification {
        self.verification
    }

    async fn verify_targeted(
        &self,
        packet_io: &PacketIOController,
        state_sender: &watch::Sender<StateType>,
    ) -> TargetedRead {
        let FlagVerification::Targeted { command } = self.verification else {
            return TargetedRead::Observed(None);
        };
        let response = match packet_io
            .send_with_response(&packet::Outbound::new(command, Vec::new()))
            .await
        {
            Ok(response) => response,
            Err(device::Error::ActionTimedOut { .. }) => return TargetedRead::TimedOut,
            Err(_) => return TargetedRead::Disconnected,
        };
        let Some(raw) = parse_bool_body(&response.body) else {
            return TargetedRead::Observed(None);
        };
        let value = if self.is_inverted { !raw } else { raw };
        state_sender.send_if_modified(|state| {
            let current = self.raw_value(state);
            if current == Some(raw) {
                false
            } else {
                self.set_raw_value(state, raw);
                true
            }
        });
        TargetedRead::Observed(Some(Value::Bool(value)))
    }
}

fn parse_bool_body(body: &[u8]) -> Option<bool> {
    match body {
        [value] => Some(*value == 1),
        _ => None,
    }
}

pub async fn set_setting_values<StateType>(
    module_collection: &ModuleCollection<StateType>,
    state_sender: &watch::Sender<StateType>,
    packet_io: &PacketIOController,
    setting_values: impl IntoIterator<Item = (SettingId, Value)>,
    demo: bool,
) -> device::Result<Vec<SettingWriteOutcome>>
where
    StateType: Clone,
{
    let setting_values = coalesce(setting_values.into_iter().collect());
    if setting_values.is_empty() {
        return Ok(Vec::new());
    }
    if !is_connected(packet_io) {
        return Ok(setting_values
            .into_iter()
            .map(|(setting_id, _)| SettingWriteOutcome {
                setting_id,
                outcome: WriteOutcome::Disconnected,
            })
            .collect());
    }

    let mut throttle = module_collection.write_lock.lock().await;
    let current_state = state_sender.borrow().clone();
    let current_settings = module_collection
        .setting_manager
        .settings_map(&current_state);
    let mut target_state = current_state.clone();

    let mut entries = Vec::<Entry>::with_capacity(setting_values.len());
    for (setting_id, requested) in setting_values {
        if !module_collection.setting_manager.has_handler(&setting_id) {
            return Err(local_error(setting_id, SettingHandlerError::DoesNotExist));
        }
        if let SettingAvailability::ReadOnly { reason }
        | SettingAvailability::Unavailable { reason } =
            setting_availability(&current_settings, &setting_id.to_string())
        {
            entries.push(Entry::unsupported(setting_id, reason));
            continue;
        }
        if let Some(current_setting) = current_settings.get(&setting_id)
            && matches!(current_setting.mode(), SettingMode::ReadOnly)
        {
            return Err(local_error(setting_id, SettingHandlerError::ReadOnly));
        }

        let current_value = current_settings
            .get(&setting_id)
            .map(|s| Value::from(s.clone()));
        match module_collection
            .setting_manager
            .set(&mut target_state, &setting_id, requested.clone())
            .await
        {
            None => return Err(local_error(setting_id, SettingHandlerError::DoesNotExist)),
            Some(Err(err)) => return Err(err.into_settings_error(setting_id).into()),
            Some(Ok(())) => {}
        }
        let Some(target_setting) = module_collection
            .setting_manager
            .get(&target_state, &setting_id)
        else {
            entries.push(Entry::unsupported(
                setting_id,
                AvailabilityReason::NoReadBack,
            ));
            continue;
        };
        match target_setting.mode() {
            SettingMode::ReadWrite => {}
            SettingMode::ReadOnly => {
                return Err(local_error(setting_id, SettingHandlerError::ReadOnly));
            }
            SettingMode::WriteOnly => {}
        }
        let target_is_readable = target_setting.mode().is_readable();
        let target_value = Value::from(target_setting);
        let target_observed = if target_is_readable {
            target_value.clone()
        } else {
            requested.clone()
        };
        let is_flag = module_collection.flags.contains_key(&setting_id);
        let flag = module_collection.flags.get(&setting_id).and_then(|access| {
            Some(FlagPlan {
                set_command: access.set_command(),
                current_raw: access.raw_value(&current_state)?,
                target_raw: access.raw_value(&target_state)?,
                verification: access.verification(),
            })
        });
        if is_flag && flag.is_none() {
            entries.push(Entry::unsupported(
                setting_id,
                AvailabilityReason::NoReadBack,
            ));
            continue;
        }

        let readable = current_settings
            .get(&setting_id)
            .is_some_and(|setting| setting.mode().is_readable());
        let already_equal =
            readable && current_value.as_ref() == Some(&requested) && target_value == requested;
        if already_equal {
            entries.push(Entry::applied(setting_id, requested));
            continue;
        }
        if readable && current_value.as_ref() == Some(&target_value) {
            entries.push(Entry::resolved(
                setting_id,
                WriteOutcome::Rejected {
                    observed: current_value,
                },
            ));
            continue;
        }

        let timeout = if is_flag && setting_id == SettingId::Ldac {
            LDAC_CONFIRMATION_TIMEOUT
        } else {
            DEFAULT_CONFIRMATION_TIMEOUT
        };
        entries.push(Entry {
            setting_id,
            requested,
            target_value: Some(target_observed),
            flag,
            legacy: !is_flag,
            deadline: Instant::now() + timeout,
            last_observed: None,
            outcome: None,
        });
    }

    for index in 0..entries.len() {
        if entries[index].outcome.is_some() {
            continue;
        }
        if !is_connected(packet_io) {
            mark_pending_disconnected(&mut entries[index..]);
            return Ok(finish(entries));
        }
        let Some(flag) = &entries[index].flag else {
            continue;
        };
        if flag.current_raw == flag.target_raw {
            continue;
        }
        throttle.wait().await;
        let packet = packet::Outbound::new(flag.set_command, vec![flag.target_raw.into()]);
        if let Err(err) = packet_io.send_without_response(&packet).await
            && (is_disconnection(&err) || !is_connected(packet_io))
        {
            mark_pending_disconnected(&mut entries[index..]);
            return Ok(finish(entries));
        }
    }
    for modifier in &module_collection.state_modifiers {
        if !is_connected(packet_io) {
            mark_pending_disconnected(&mut entries);
            return Ok(finish(entries));
        }
        let modifier_result = modifier.move_to_state(state_sender, &target_state).await;
        if let Err(err) = modifier_result {
            if is_disconnection(&err) || !is_connected(packet_io) {
                mark_pending_disconnected(&mut entries);
                return Ok(finish(entries));
            }
            tracing::debug!("write modifier failed: {err:?}");
        }
    }
    if demo {
        state_sender.send_if_modified(|state| {
            let mut modified = false;
            for entry in &entries {
                let Some(flag) = &entry.flag else {
                    continue;
                };
                let Some(access) = module_collection.flags.get(&entry.setting_id) else {
                    continue;
                };
                if access.raw_value(state) != Some(flag.target_raw) {
                    access.set_raw_value(state, flag.target_raw);
                    modified = true;
                }
            }
            modified
        });
        for entry in &mut entries {
            if entry.outcome.is_none() {
                entry.outcome = Some(WriteOutcome::Applied {
                    observed: entry.requested.clone(),
                });
            }
        }
        return Ok(finish(entries));
    }

    for entry in &mut entries {
        if entry.outcome.is_none() && entry.legacy {
            entry.outcome = Some(WriteOutcome::Applied {
                observed: entry
                    .target_value
                    .clone()
                    .unwrap_or_else(|| entry.requested.clone()),
            });
        }
    }

    for index in 0..entries.len() {
        if entries[index].outcome.is_some() {
            continue;
        }
        let Some(flag) = &entries[index].flag else {
            continue;
        };
        if !matches!(flag.verification, FlagVerification::Targeted { .. }) {
            continue;
        }
        let Some(access) = module_collection.flags.get(&entries[index].setting_id) else {
            entries[index].outcome = Some(WriteOutcome::UnsupportedInState {
                reason: AvailabilityReason::NoReadBack,
            });
            continue;
        };
        let outcome = verify_targeted(
            access.as_ref(),
            packet_io,
            state_sender,
            &mut throttle,
            &entries[index],
        )
        .await;
        entries[index].outcome = Some(outcome);
        if matches!(entries[index].outcome, Some(WriteOutcome::Disconnected)) {
            mark_remaining_disconnected(&mut entries, index + 1);
            return Ok(finish(entries));
        }
    }

    let has_bulk = entries.iter().any(|entry| {
        entry.outcome.is_none()
            && !entry.legacy
            && !entry
                .flag
                .as_ref()
                .is_some_and(|flag| matches!(flag.verification, FlagVerification::Targeted { .. }))
    });
    if has_bulk {
        verify_bulk(
            module_collection,
            packet_io,
            state_sender,
            &mut throttle,
            &mut entries,
        )
        .await;
    }
    Ok(finish(entries))
}

struct Entry {
    setting_id: SettingId,
    requested: Value,
    target_value: Option<Value>,
    flag: Option<FlagPlan>,
    legacy: bool,
    deadline: Instant,
    last_observed: Option<Option<Value>>,
    outcome: Option<WriteOutcome>,
}

impl Entry {
    fn unsupported(setting_id: SettingId, reason: AvailabilityReason) -> Self {
        Self::resolved(setting_id, WriteOutcome::UnsupportedInState { reason })
    }

    fn applied(setting_id: SettingId, observed: Value) -> Self {
        Self::resolved(setting_id, WriteOutcome::Applied { observed })
    }

    fn resolved(setting_id: SettingId, outcome: WriteOutcome) -> Self {
        Self {
            setting_id,
            requested: Value::Bool(false),
            target_value: None,
            flag: None,
            legacy: false,
            deadline: Instant::now(),
            last_observed: None,
            outcome: Some(outcome),
        }
    }
}

struct FlagPlan {
    set_command: packet::Command,
    current_raw: bool,
    target_raw: bool,
    verification: FlagVerification,
}

fn coalesce(setting_values: Vec<(SettingId, Value)>) -> Vec<(SettingId, Value)> {
    let mut order = Vec::with_capacity(setting_values.len());
    let mut latest = std::collections::HashMap::with_capacity(setting_values.len());
    for (setting_id, value) in setting_values {
        if latest.insert(setting_id, value).is_none() {
            order.push(setting_id);
        }
    }
    order
        .into_iter()
        .filter_map(|setting_id| latest.remove(&setting_id).map(|value| (setting_id, value)))
        .collect()
}

fn finish(entries: Vec<Entry>) -> Vec<SettingWriteOutcome> {
    entries
        .into_iter()
        .map(|entry| SettingWriteOutcome {
            setting_id: entry.setting_id,
            outcome: entry.outcome.unwrap_or(WriteOutcome::TimedOut),
        })
        .collect()
}

fn local_error(setting_id: SettingId, error: SettingHandlerError) -> device::Error {
    error.into_settings_error(setting_id).into()
}

fn mark_pending_disconnected(entries: &mut [Entry]) {
    for entry in entries {
        if entry.outcome.is_none() {
            entry.outcome = Some(WriteOutcome::Disconnected);
        }
    }
}

fn mark_remaining_disconnected(entries: &mut [Entry], start: usize) {
    for entry in &mut entries[start..] {
        if entry.outcome.is_none() {
            entry.outcome = Some(WriteOutcome::Disconnected);
        }
    }
}

fn is_connected(packet_io: &PacketIOController) -> bool {
    *packet_io.connection_status().borrow() == ConnectionStatus::Connected
}

fn is_disconnection(error: &device::Error) -> bool {
    matches!(
        error,
        device::Error::ConnectionError { .. } | device::Error::Disconnected
    )
}

async fn verify_targeted<StateType>(
    access: &(dyn FlagAccess<StateType> + Send + Sync),
    packet_io: &PacketIOController,
    state_sender: &watch::Sender<StateType>,
    throttle: &mut WriteThrottle,
    entry: &Entry,
) -> WriteOutcome {
    let mut last_observed: Option<Option<Value>> = None;
    let mut saw_response = false;
    loop {
        if !is_connected(packet_io) {
            return WriteOutcome::Disconnected;
        }
        let now = Instant::now();
        if now >= entry.deadline {
            break;
        }
        throttle.wait().await;
        match access.verify_targeted(packet_io, state_sender).await {
            TargetedRead::Disconnected => return WriteOutcome::Disconnected,
            TargetedRead::TimedOut => {}
            TargetedRead::Observed(observed) => {
                saw_response = true;
                if observed.as_ref() == Some(&entry.requested) {
                    return WriteOutcome::Applied {
                        observed: entry.requested.clone(),
                    };
                }
                last_observed = Some(observed);
            }
        }
        let now = Instant::now();
        if now >= entry.deadline {
            break;
        }
        tokio::time::sleep((entry.deadline - now).min(POLL_INTERVAL)).await;
    }
    if saw_response {
        WriteOutcome::Rejected {
            observed: last_observed.flatten(),
        }
    } else {
        WriteOutcome::TimedOut
    }
}

async fn verify_bulk<StateType>(
    module_collection: &ModuleCollection<StateType>,
    packet_io: &PacketIOController,
    state_sender: &watch::Sender<StateType>,
    throttle: &mut WriteThrottle,
    entries: &mut [Entry],
) where
    StateType: Clone,
{
    let mut receiver = state_sender.subscribe();
    let mut saw_response = false;
    let mut next_request = Instant::now();
    loop {
        let now = Instant::now();
        match pending_deadline(entries) {
            Some(deadline) if now < deadline => {}
            Some(_) | None => break,
        }
        if !is_connected(packet_io) {
            mark_pending_disconnected(entries);
            return;
        }

        {
            let state = state_sender.borrow().clone();
            let mut any_pending = false;
            for entry in entries.iter_mut() {
                if entry.outcome.is_some() || entry.legacy {
                    continue;
                }
                if entry.flag.as_ref().is_some_and(|flag| {
                    matches!(flag.verification, FlagVerification::Targeted { .. })
                }) {
                    any_pending = true;
                    continue;
                }
                let observed = module_collection
                    .setting_manager
                    .get(&state, &entry.setting_id)
                    .map(Value::from);
                if observed.as_ref() == Some(&entry.requested) {
                    entry.outcome = Some(WriteOutcome::Applied {
                        observed: entry.requested.clone(),
                    });
                } else {
                    entry.last_observed = Some(observed);
                    any_pending = true;
                }
            }
            if !any_pending {
                return;
            }
        }

        if Instant::now() >= next_request {
            throttle.wait().await;
            if let Err(error) = packet_io
                .send_without_response(&packet::outbound::RequestState.to_packet())
                .await
                && (is_disconnection(&error) || !is_connected(packet_io))
            {
                mark_pending_disconnected(entries);
                return;
            }
            next_request = Instant::now() + POLL_INTERVAL;
        }

        let wake_at = pending_deadline(entries)
            .unwrap_or(next_request)
            .min(next_request);
        let wait = wake_at.saturating_duration_since(Instant::now());
        tokio::select! {
            result = receiver.changed() => {
                if result.is_ok() {
                    saw_response = true;
                }
            }
            () = tokio::time::sleep(wait) => ()
        }
    }

    for entry in entries.iter_mut() {
        if entry.outcome.is_some() || entry.legacy {
            continue;
        }
        if entry
            .flag
            .as_ref()
            .is_some_and(|flag| matches!(flag.verification, FlagVerification::Targeted { .. }))
        {
            continue;
        }
        if entry.last_observed.as_ref() == Some(&Some(entry.requested.clone())) {
            entry.outcome = Some(WriteOutcome::Applied {
                observed: entry.requested.clone(),
            });
        } else if saw_response {
            entry.outcome = Some(WriteOutcome::Rejected {
                observed: entry.last_observed.clone().flatten(),
            });
        } else {
            entry.outcome = Some(WriteOutcome::TimedOut);
        }
    }
}

fn pending_deadline(entries: &[Entry]) -> Option<Instant> {
    entries
        .iter()
        .filter(|entry| entry.outcome.is_none() && !entry.legacy)
        .map(|entry| entry.deadline)
        .max()
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use async_trait::async_trait;
    use nom_language::error::VerboseError;
    use tokio::sync::{mpsc, watch};
    use tokio::time::Duration;

    use crate::{
        api::{
            connection::{ConnectionStatus, RfcommConnection},
            settings::{CategoryId, Setting, SettingId, Value},
        },
        devices::soundcore::common::{
            modules::{
                ModuleCollection, ModuleCollectionSpawnPacketHandlerExt, flag::FlagConfiguration,
            },
            packet::{
                self, ChecksumKind, PacketIOController, inbound::LdacState,
                outbound::SET_WEARING_DETECTION_COMMAND,
            },
            settings_manager::{SettingHandler, SettingHandlerResult},
            structures::{Ldac, TouchTone, TwsStatus, WearingDetection, WearingTone},
        },
    };

    use super::*;

    #[derive(Clone, Default)]
    struct TestState {
        ldac: Ldac,
        wearing_detection: WearingDetection,
        wearing_tone: WearingTone,
        touch_tone: TouchTone,
        tws_status: TwsStatus,
    }

    macro_rules! has {
        ($field:ident, $ty:ty) => {
            impl openscq30_lib_has::Has<$ty> for TestState {
                fn get(&self) -> &$ty {
                    &self.$field
                }

                fn get_mut(&mut self) -> &mut $ty {
                    &mut self.$field
                }
            }
        };
    }

    has!(ldac, Ldac);
    has!(wearing_detection, WearingDetection);
    has!(wearing_tone, WearingTone);
    has!(touch_tone, TouchTone);
    has!(tws_status, TwsStatus);

    struct TestConnection {
        status: watch::Sender<ConnectionStatus>,
        inbound: Mutex<Option<mpsc::Receiver<Vec<u8>>>>,
        outbound: mpsc::Sender<Vec<u8>>,
    }

    #[async_trait]
    impl RfcommConnection for TestConnection {
        async fn write(&self, data: &[u8]) -> crate::api::connection::Result<()> {
            self.outbound.send(data.to_vec()).await.unwrap();
            Ok(())
        }

        fn read_channel(&self) -> mpsc::Receiver<Vec<u8>> {
            self.inbound
                .lock()
                .unwrap()
                .take()
                .expect("read_channel should only be called once")
        }

        fn connection_status(&self) -> watch::Receiver<ConnectionStatus> {
            self.status.subscribe()
        }
    }

    struct BulkStateHandler;

    #[async_trait]
    impl crate::devices::soundcore::common::packet_manager::PacketHandler<TestState>
        for BulkStateHandler
    {
        async fn handle_packet(
            &self,
            state: &watch::Sender<TestState>,
            packet: &packet::Inbound,
        ) -> crate::api::device::Result<()> {
            let [ldac, wearing_detection, wearing_tone, touch_tone] = packet.body.as_slice() else {
                return Ok(());
            };
            state.send_modify(|state| {
                state.ldac.0 = *ldac == 1;
                state.wearing_detection.0 = *wearing_detection == 1;
                state.wearing_tone.0 = *wearing_tone == 1;
                state.touch_tone.0 = *touch_tone == 1;
            });
            Ok(())
        }
    }

    struct Harness {
        modules: Arc<ModuleCollection<TestState>>,
        state_sender: watch::Sender<TestState>,
        packet_io: Arc<PacketIOController>,
        inbound: mpsc::Sender<Vec<u8>>,
        outbound: mpsc::Receiver<Vec<u8>>,
        status: watch::Sender<ConnectionStatus>,
    }

    impl Harness {
        async fn new(state: TestState) -> Self {
            let (inbound_sender, inbound_receiver) = mpsc::channel(100);
            let (outbound_sender, outbound_receiver) = mpsc::channel(100);
            let status = watch::channel(ConnectionStatus::Connected).0;
            let connection = Arc::new(TestConnection {
                status: status.clone(),
                inbound: Mutex::new(Some(inbound_receiver)),
                outbound: outbound_sender,
            });
            let (packet_io, packet_receiver) =
                PacketIOController::new(connection, ChecksumKind::None)
                    .await
                    .unwrap();
            let packet_io = Arc::new(packet_io);
            let mut modules = ModuleCollection::<TestState>::default();
            modules.add_flag::<Ldac>(
                packet_io.clone(),
                FlagConfiguration {
                    category_id: CategoryId::Miscellaneous,
                    setting_id: SettingId::Ldac,
                    set_command: packet::Command([0x01, 0xFF]),
                    update_command: Some(LdacState::COMMAND),
                    is_inverted: false,
                    verification: FlagVerification::Targeted {
                        command: LdacState::COMMAND,
                    },
                },
            );
            modules.add_flag::<WearingDetection>(
                packet_io.clone(),
                FlagConfiguration {
                    category_id: CategoryId::Miscellaneous,
                    setting_id: SettingId::WearingDetection,
                    set_command: SET_WEARING_DETECTION_COMMAND,
                    update_command: None,
                    is_inverted: false,
                    verification: FlagVerification::Bulk,
                },
            );
            modules.add_flag::<WearingTone>(
                packet_io.clone(),
                FlagConfiguration {
                    category_id: CategoryId::Miscellaneous,
                    setting_id: SettingId::WearingTone,
                    set_command: packet::outbound::SET_WEARING_TONE_COMMAND,
                    update_command: None,
                    is_inverted: false,
                    verification: FlagVerification::Bulk,
                },
            );
            modules.add_flag::<TouchTone>(
                packet_io.clone(),
                FlagConfiguration {
                    category_id: CategoryId::Miscellaneous,
                    setting_id: SettingId::TouchTone,
                    set_command: packet::outbound::SET_TOUCH_TONE_COMMAND,
                    update_command: None,
                    is_inverted: false,
                    verification: FlagVerification::Bulk,
                },
            );
            modules
                .packet_handlers
                .set_handler(packet::inbound::STATE_COMMAND, Box::new(BulkStateHandler));
            modules.add_tws_status();
            let modules = Arc::new(modules);
            let state_sender = watch::channel(state).0;
            modules
                .spawn_packet_handler(
                    state_sender.clone(),
                    packet_receiver,
                    Arc::new(tokio::sync::Semaphore::new(0)),
                )
                .await;
            Self {
                modules,
                state_sender,
                packet_io,
                inbound: inbound_sender,
                outbound: outbound_receiver,
                status,
            }
        }

        fn spawn_write(
            &self,
            values: Vec<(SettingId, Value)>,
        ) -> tokio::task::JoinHandle<device::Result<Vec<SettingWriteOutcome>>> {
            let modules = self.modules.clone();
            let state_sender = self.state_sender.clone();
            let packet_io = self.packet_io.clone();
            tokio::spawn(async move {
                modules
                    .set_setting_values(&state_sender, &packet_io, values)
                    .await
            })
        }

        async fn next_packet(&mut self) -> packet::Outbound {
            let bytes = tokio::time::timeout(Duration::from_secs(5), self.outbound.recv())
                .await
                .expect("outbound packet")
                .expect("outbound channel");
            packet::Outbound::take::<VerboseError<_>>(ChecksumKind::None)(&bytes)
                .unwrap()
                .1
        }

        async fn respond(&self, command: packet::Command, body: Vec<u8>) {
            self.inbound
                .send(packet::Inbound::new(command, body).bytes(ChecksumKind::None))
                .await
                .unwrap();
        }
    }

    #[tokio::test(start_paused = true)]
    async fn ack_without_read_back_is_rejected_and_does_not_publish_the_target() {
        let mut harness = Harness::new(TestState {
            ldac: Ldac(true),
            ..Default::default()
        })
        .await;
        let changes = harness.state_sender.subscribe();
        let mut write = harness.spawn_write(vec![(SettingId::Ldac, false.into())]);

        let set = harness.next_packet().await;
        assert_eq!(packet::Command([0x01, 0xFF]), set.command);
        assert_eq!(vec![0], set.body);
        harness.respond(set.command, Vec::new()).await;

        let outcomes = loop {
            tokio::select! {
                result = &mut write => break result.unwrap().unwrap(),
                packet = harness.outbound.recv() => {
                    let bytes = packet.expect("outbound channel");
                    let get = packet::Outbound::take::<VerboseError<_>>(ChecksumKind::None)(&bytes)
                        .unwrap()
                        .1;
                    assert_eq!(LdacState::COMMAND, get.command);
                    harness.respond(get.command, vec![1]).await;
                }
            }
        };

        assert_eq!(
            vec![SettingWriteOutcome {
                setting_id: SettingId::Ldac,
                outcome: WriteOutcome::Rejected {
                    observed: Some(Value::Bool(true))
                },
            }],
            outcomes
        );
        assert!(harness.state_sender.borrow().ldac.0);
        assert!(!changes.has_changed().unwrap());
    }

    #[tokio::test(start_paused = true)]
    async fn a_targeted_read_back_applies_and_publishes_the_observed_value() {
        let mut harness = Harness::new(TestState {
            ldac: Ldac(true),
            ..Default::default()
        })
        .await;
        let changes = harness.state_sender.subscribe();
        let mut write = harness.spawn_write(vec![(SettingId::Ldac, false.into())]);

        let set = harness.next_packet().await;
        assert_eq!(packet::Command([0x01, 0xFF]), set.command);
        harness.respond(set.command, Vec::new()).await;

        let get = harness.next_packet().await;
        assert_eq!(LdacState::COMMAND, get.command);
        harness.respond(get.command, vec![0]).await;

        let outcomes = write.await.unwrap().unwrap();
        assert_eq!(
            vec![SettingWriteOutcome {
                setting_id: SettingId::Ldac,
                outcome: WriteOutcome::Applied {
                    observed: Value::Bool(false)
                },
            }],
            outcomes
        );
        assert!(!harness.state_sender.borrow().ldac.0);
        assert!(changes.has_changed().unwrap());
    }

    #[tokio::test(start_paused = true)]
    async fn a_slow_applying_setting_is_polled_until_it_matches() {
        let mut harness = Harness::new(TestState {
            ldac: Ldac(true),
            ..Default::default()
        })
        .await;
        let started = tokio::time::Instant::now();
        let mut write = harness.spawn_write(vec![(SettingId::Ldac, false.into())]);

        let set = harness.next_packet().await;
        harness.respond(set.command, Vec::new()).await;

        let mut reads = 0;
        let outcomes = loop {
            tokio::select! {
                result = &mut write => break result.unwrap().unwrap(),
                packet = harness.outbound.recv() => {
                    let bytes = packet.expect("outbound channel");
                    let get = packet::Outbound::take::<VerboseError<_>>(ChecksumKind::None)(&bytes)
                        .unwrap()
                        .1;
                    assert_eq!(LdacState::COMMAND, get.command);
                    reads += 1;
                    harness.respond(get.command, vec![u8::from(reads < 3)]).await;
                }
            }
        };

        assert_eq!(
            WriteOutcome::Applied {
                observed: Value::Bool(false)
            },
            outcomes[0].outcome
        );
        assert!(reads >= 3);
        assert!(started.elapsed() >= Duration::from_millis(500));
    }

    #[tokio::test(start_paused = true)]
    async fn no_read_back_times_out_without_publishing_and_the_next_write_does_not_hang() {
        let mut harness = Harness::new(TestState {
            ldac: Ldac(true),
            ..Default::default()
        })
        .await;
        let changes = harness.state_sender.subscribe();
        let mut write = harness.spawn_write(vec![(SettingId::Ldac, false.into())]);

        let set = harness.next_packet().await;
        harness.respond(set.command, Vec::new()).await;

        let outcomes = loop {
            tokio::select! {
                result = &mut write => break result.unwrap().unwrap(),
                _ = harness.outbound.recv() => {}
            }
        };
        assert_eq!(WriteOutcome::TimedOut, outcomes[0].outcome);
        assert!(harness.state_sender.borrow().ldac.0);
        assert!(!changes.has_changed().unwrap());

        let mut write = harness.spawn_write(vec![(SettingId::Ldac, false.into())]);
        let set = harness.next_packet().await;
        harness.respond(set.command, Vec::new()).await;
        let get = harness.next_packet().await;
        harness.respond(get.command, vec![0]).await;
        let outcomes = write.await.unwrap().unwrap();
        assert_eq!(
            WriteOutcome::Applied {
                observed: Value::Bool(false)
            },
            outcomes[0].outcome
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_disconnect_during_verification_is_reported_and_publishes_nothing() {
        let mut harness = Harness::new(TestState {
            ldac: Ldac(true),
            ..Default::default()
        })
        .await;
        let changes = harness.state_sender.subscribe();
        let mut write = harness.spawn_write(vec![(SettingId::Ldac, false.into())]);

        let set = harness.next_packet().await;
        harness.respond(set.command, Vec::new()).await;
        harness.status.send_replace(ConnectionStatus::Disconnected);

        let outcomes = write.await.unwrap().unwrap();
        assert_eq!(WriteOutcome::Disconnected, outcomes[0].outcome);
        assert!(harness.state_sender.borrow().ldac.0);
        assert!(!changes.has_changed().unwrap());
    }

    #[tokio::test(start_paused = true)]
    async fn a_bulk_read_back_applies_and_publishes_the_observed_value() {
        let mut harness = Harness::new(TestState::default()).await;
        let changes = harness.state_sender.subscribe();
        let mut write = harness.spawn_write(vec![(SettingId::WearingDetection, true.into())]);

        let set = harness.next_packet().await;
        assert_eq!(SET_WEARING_DETECTION_COMMAND, set.command);
        harness.respond(set.command, Vec::new()).await;

        let get = harness.next_packet().await;
        assert_eq!(packet::inbound::STATE_COMMAND, get.command);
        harness.respond(get.command, vec![0, 1, 0, 0]).await;

        let outcomes = write.await.unwrap().unwrap();
        assert_eq!(
            WriteOutcome::Applied {
                observed: Value::Bool(true)
            },
            outcomes[0].outcome
        );
        assert!(harness.state_sender.borrow().wearing_detection.0);
        assert!(changes.has_changed().unwrap());
    }

    #[tokio::test(start_paused = true)]
    async fn a_bulk_read_back_of_the_old_value_is_rejected() {
        let mut harness = Harness::new(TestState::default()).await;
        let mut write = harness.spawn_write(vec![(SettingId::WearingDetection, true.into())]);

        let set = harness.next_packet().await;
        harness.respond(set.command, Vec::new()).await;

        let outcomes = loop {
            tokio::select! {
                result = &mut write => break result.unwrap().unwrap(),
                packet = harness.outbound.recv() => {
                    let bytes = packet.expect("outbound channel");
                    let get = packet::Outbound::take::<VerboseError<_>>(ChecksumKind::None)(&bytes)
                        .unwrap()
                        .1;
                    assert_eq!(packet::inbound::STATE_COMMAND, get.command);
                    harness.respond(get.command, vec![0, 0, 0, 0]).await;
                }
            }
        };

        assert_eq!(
            WriteOutcome::Rejected {
                observed: Some(Value::Bool(false))
            },
            outcomes[0].outcome
        );
        assert!(!harness.state_sender.borrow().wearing_detection.0);
    }

    #[tokio::test(start_paused = true)]
    async fn a_bulk_read_back_that_never_arrives_is_timed_out() {
        let mut harness = Harness::new(TestState::default()).await;
        let changes = harness.state_sender.subscribe();
        let mut write = harness.spawn_write(vec![(SettingId::WearingDetection, true.into())]);

        let set = harness.next_packet().await;
        harness.respond(set.command, Vec::new()).await;

        let outcomes = loop {
            tokio::select! {
                result = &mut write => break result.unwrap().unwrap(),
                _ = harness.outbound.recv() => {}
            }
        };

        assert_eq!(WriteOutcome::TimedOut, outcomes[0].outcome);
        assert!(!harness.state_sender.borrow().wearing_detection.0);
        assert!(!changes.has_changed().unwrap());
    }

    #[tokio::test(start_paused = true)]
    async fn a_batch_reports_every_setting_in_request_order_without_rollback() {
        let mut harness = Harness::new(TestState::default()).await;
        let mut write = harness.spawn_write(vec![
            (SettingId::WearingDetection, true.into()),
            (SettingId::WearingTone, true.into()),
            (SettingId::TouchTone, true.into()),
        ]);

        let mut set_commands = Vec::new();
        for _ in 0..3 {
            let set = harness.next_packet().await;
            set_commands.push(set.command);
            harness.respond(set.command, Vec::new()).await;
        }
        assert_eq!(
            vec![
                SET_WEARING_DETECTION_COMMAND,
                packet::outbound::SET_WEARING_TONE_COMMAND,
                packet::outbound::SET_TOUCH_TONE_COMMAND,
            ],
            set_commands
        );

        let outcomes = loop {
            tokio::select! {
                result = &mut write => break result.unwrap().unwrap(),
                packet = harness.outbound.recv() => {
                    let bytes = packet.expect("outbound channel");
                    let get = packet::Outbound::take::<VerboseError<_>>(ChecksumKind::None)(&bytes)
                        .unwrap()
                        .1;
                    assert_eq!(packet::inbound::STATE_COMMAND, get.command);
                    harness.respond(get.command, vec![0, 1, 0, 1]).await;
                }
            }
        };

        assert_eq!(
            vec![
                (
                    SettingId::WearingDetection,
                    WriteOutcome::Applied {
                        observed: Value::Bool(true)
                    }
                ),
                (
                    SettingId::WearingTone,
                    WriteOutcome::Rejected {
                        observed: Some(Value::Bool(false))
                    }
                ),
                (
                    SettingId::TouchTone,
                    WriteOutcome::Applied {
                        observed: Value::Bool(true)
                    }
                ),
            ],
            outcomes
                .into_iter()
                .map(|outcome| (outcome.setting_id, outcome.outcome))
                .collect::<Vec<_>>()
        );
        let state = harness.state_sender.borrow().clone();
        assert!(state.wearing_detection.0);
        assert!(!state.wearing_tone.0);
        assert!(state.touch_tone.0);
    }

    #[tokio::test(start_paused = true)]
    async fn setting_commands_are_paced_and_read_backs_come_after_all_writes() {
        let mut harness = Harness::new(TestState::default()).await;
        let mut write = harness.spawn_write(vec![
            (SettingId::WearingDetection, true.into()),
            (SettingId::WearingTone, true.into()),
        ]);

        let first = harness.next_packet().await;
        let first_at = tokio::time::Instant::now();
        harness.respond(first.command, Vec::new()).await;
        let second = harness.next_packet().await;
        let second_at = tokio::time::Instant::now();
        harness.respond(second.command, Vec::new()).await;
        let read_back = harness.next_packet().await;

        assert_eq!(SET_WEARING_DETECTION_COMMAND, first.command);
        assert_eq!(packet::outbound::SET_WEARING_TONE_COMMAND, second.command);
        assert_eq!(packet::inbound::STATE_COMMAND, read_back.command);
        assert!(second_at - first_at >= MIN_WRITE_INTERVAL);

        harness.respond(read_back.command, vec![0, 1, 1, 0]).await;
        let outcomes = write.await.unwrap().unwrap();
        assert!(
            outcomes
                .iter()
                .all(|outcome| matches!(outcome.outcome, WriteOutcome::Applied { .. }))
        );
    }

    #[tokio::test(start_paused = true)]
    async fn one_earbud_ldac_is_unsupported_without_sending_anything() {
        let mut harness = Harness::new(TestState {
            ldac: Ldac(true),
            tws_status: TwsStatus {
                is_connected: false,
                ..Default::default()
            },
            ..Default::default()
        })
        .await;

        let outcomes = harness
            .spawn_write(vec![(SettingId::Ldac, false.into())])
            .await
            .unwrap()
            .unwrap();

        assert_eq!(
            WriteOutcome::UnsupportedInState {
                reason: AvailabilityReason::RequiresBothEarbuds
            },
            outcomes[0].outcome
        );
        assert!(harness.state_sender.borrow().ldac.0);
        assert!(
            tokio::time::timeout(Duration::from_millis(10), harness.outbound.recv())
                .await
                .is_err()
        );
    }

    #[tokio::test(start_paused = true)]
    async fn an_unknown_setting_is_a_local_error() {
        let harness = Harness::new(TestState::default()).await;

        let error = harness
            .spawn_write(vec![(SettingId::GamingMode, true.into())])
            .await
            .unwrap()
            .unwrap_err();

        assert!(matches!(error, device::Error::SettingError { .. }));
    }

    #[tokio::test(start_paused = true)]
    async fn a_setting_that_is_already_equal_is_applied_without_a_write() {
        let mut harness = Harness::new(TestState::default()).await;

        let outcomes = harness
            .spawn_write(vec![(SettingId::WearingDetection, false.into())])
            .await
            .unwrap()
            .unwrap();

        assert_eq!(
            WriteOutcome::Applied {
                observed: Value::Bool(false)
            },
            outcomes[0].outcome
        );
        assert!(
            tokio::time::timeout(Duration::from_millis(10), harness.outbound.recv())
                .await
                .is_err()
        );
    }

    #[tokio::test(start_paused = true)]
    async fn writes_while_recovering_are_disconnected_without_queueing() {
        let mut harness = Harness::new(TestState {
            ldac: Ldac(true),
            ..Default::default()
        })
        .await;
        let mut status = harness.packet_io.connection_status();
        harness.status.send_replace(ConnectionStatus::Disconnected);
        while *status.borrow() == ConnectionStatus::Connected {
            status.changed().await.unwrap();
        }

        let outcomes = harness
            .spawn_write(vec![(SettingId::WearingDetection, true.into())])
            .await
            .unwrap()
            .unwrap();
        assert_eq!(WriteOutcome::Disconnected, outcomes[0].outcome);
        assert!(
            !matches!(
                tokio::time::timeout(Duration::from_millis(10), harness.outbound.recv()).await,
                Ok(Some(_))
            ),
            "no packet should be written while recovering"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn repeated_slider_values_are_coalesced_to_the_last_value() {
        let mut harness = Harness::new(TestState::default()).await;

        let outcomes = harness
            .spawn_write(vec![
                (SettingId::WearingDetection, true.into()),
                (SettingId::WearingDetection, false.into()),
            ])
            .await
            .unwrap()
            .unwrap();

        assert_eq!(1, outcomes.len());
        assert_eq!(
            WriteOutcome::Applied {
                observed: Value::Bool(false)
            },
            outcomes[0].outcome
        );
        assert!(
            tokio::time::timeout(Duration::from_millis(10), harness.outbound.recv())
                .await
                .is_err()
        );
    }
}
