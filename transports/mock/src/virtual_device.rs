use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use bytes::{Bytes, BytesMut};
use openscl4_models::{
    A3028_AUTO_POWER_OFF, A3028_EQUALIZER_BANDS, A3028_EQUALIZER_COMMAND, A3028_EQUALIZER_PRESET,
    A3028_SOUND_MODE_FIELDS, A3062_EQUALIZER_BANDS, A3062_EQUALIZER_PRESET,
    A3062_SOUND_MODE_FIELDS, A3116_AUTO_POWER_OFF_COMMAND, A3116_AUTO_POWER_OFF_FIELD,
    A3116_EQUALIZER_BANDS, A3116_EQUALIZER_BANDS_COMMAND, A3116_EQUALIZER_PRESET,
    A3116_EQUALIZER_PRESET_COMMAND, A3116_POWER_OFF_COMMAND, A3116_VOICE_PROMPT_COMMAND,
    A3116_VOICE_PROMPT_READ_COMMAND, A3116_VOLUME_COMMAND, A3116_VOLUME_FIELD, A3949_GAMING_MODE,
    AUTO_POWER_OFF_COMMAND, EQUALIZER_COMMAND, GAMING_MODE_COMMAND, IDENTITY_COMMAND, ModelDriver,
    SOUND_MODE_COMMAND, STATE_COMMAND, SoundModeWrite,
};
use openscl4_protocol::{Direction, FrameDecoder, encode_frame, encode_frame_with};
use openscl4_transport::{ConnectTarget, TransportError, TransportLink, TransportProvider};

use crate::{MockDisconnect, WithoutSoundModeAck, open_link};

const IDENTITY_REPLY: &[u8] = b"\x09\xff\0\0\x01\x01\x05\x24\x0004.2904.29A3954DEMO0000000\xb8";
const A3949_STATE_REPLY: &[u8] = include_bytes!("../../../core/data/a3949-pilot-state.bin");
const A3949_GAMING_REPLIES: [&[u8]; 2] = [
    include_bytes!("../../../core/data/a3949-gaming-off.bin"),
    include_bytes!("../../../core/data/a3949-gaming-on.bin"),
];
const A3954_STATE_REPLY: &[u8] = include_bytes!("../../../core/data/a3954-pilot-state.bin");
const A3062_MODE_REPLIES: [&[u8]; 3] = [
    include_bytes!("../../../core/data/a3062-mode-noise-canceling.bin"),
    include_bytes!("../../../core/data/a3062-mode-transparency.bin"),
    include_bytes!("../../../core/data/a3062-mode-normal.bin"),
];
/// A3116 state responses derived from the legacy driver fixture. The frames
/// carry no checksum, and the firmware variant decides whether the targeted
/// voice prompt read is available.
const A3116_STATE_REPLY: &[u8] = include_bytes!("../../../core/data/a3116-pilot-state.bin");
const A3116_OLD_FIRMWARE_STATE_REPLY: &[u8] =
    include_bytes!("../../../core/data/a3116-old-firmware-state.bin");
/// A3028 and A3029 state bodies from `core/data/a3028-a3029-mode-reference.json`.
/// The reference starts from one legacy example with an A3029 serial prefix and
/// replaces the full serial at body offsets 44–59 with the synthetic
/// model-specific value `3028TEST00000000` or `3029TEST00000000`, then
/// recomputes the checksum. Neither body carries the optional extra fields.
const A3028_STATE_BODY: &[u8] = &[
    0x05, 0x00, 0xfe, 0xfe, 0x3c, 0xb4, 0x8f, 0xa0, 0x8e, 0xb4, 0x74, 0x88, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x02, 0x00, 0x01, 0x00, 0x30, 0x32, 0x2e, 0x33, 0x30, 0x33, 0x30, 0x32, 0x38,
    0x54, 0x45, 0x53, 0x54, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30,
];
const A3029_STATE_BODY: &[u8] = &[
    0x05, 0x00, 0xfe, 0xfe, 0x3c, 0xb4, 0x8f, 0xa0, 0x8e, 0xb4, 0x74, 0x88, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x02, 0x00, 0x01, 0x00, 0x30, 0x32, 0x2e, 0x33, 0x30, 0x33, 0x30, 0x32, 0x39,
    0x54, 0x45, 0x53, 0x54, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30,
];
/// The documented extra fields appended to the same state body: unknown1, touch
/// control, dual connections, auto power off (enabled, duration index 1) and the
/// ambient and battery alert prompt tones. The matrix lists this block as
/// derived from the known layout, not a separate capture.
const AUTO_POWER_OFF_EXTRA_FIELDS: [u8; 7] = [0x00, 0x00, 0x00, 0x01, 0x01, 0x01, 0x01];
const DUAL_CONNECTIONS_COMMAND: [u8; 2] = [0x0b, 0x01];
const SOUND_MODE_ACK: &[u8] = b"\x09\xff\0\0\x01\x06\x81\x0a\0\x9a";

/// A virtual Soundcore Liberty 4 Pro with per-connection sound-mode state.
pub struct VirtualSoundcoreDevice;

impl VirtualSoundcoreDevice {
    /// Applies writes and answers state requests while withholding the write ACK.
    pub fn without_sound_mode_ack() -> impl TransportProvider {
        WithoutSoundModeAck
    }

    /// Creates a virtual device that retains its settings across links.
    pub fn with_shared_state() -> SharedVirtualSoundcoreDevice {
        SharedVirtualSoundcoreDevice {
            state: initial_state(ModelDriver::A3954),
        }
    }
}

/// A virtual Space One Pro that responds with independently captured state frames.
pub struct VirtualA3062;

impl VirtualA3062 {
    /// Continues to answer state reads while ignoring equalizer writes.
    pub fn without_equalizer_write() -> impl TransportProvider {
        WithoutEqualizerWrite
    }
}

/// Opens a virtual A3062 link with the given responder options.
fn a3062_link(options: ResponderOptions) -> Box<dyn TransportLink> {
    let model = ModelDriver::A3062;
    open_link(Some(Responder::with_state(
        model,
        options,
        initial_state(model),
    )))
    .0
}

#[async_trait]
impl TransportProvider for VirtualA3062 {
    async fn connect(
        &self,
        _target: &ConnectTarget,
    ) -> Result<Box<dyn TransportLink>, TransportError> {
        Ok(a3062_link(ResponderOptions {
            acknowledge_writes: true,
            apply_equalizer: true,
        }))
    }
}

/// A virtual Space One Pro that never applies an equalizer write.
struct WithoutEqualizerWrite;

#[async_trait]
impl TransportProvider for WithoutEqualizerWrite {
    async fn connect(
        &self,
        _target: &ConnectTarget,
    ) -> Result<Box<dyn TransportLink>, TransportError> {
        Ok(a3062_link(ResponderOptions {
            acknowledge_writes: true,
            apply_equalizer: false,
        }))
    }
}

/// A virtual A3116 speaker whose frames carry no checksum.
pub struct VirtualA3116;

impl VirtualA3116 {
    /// Opens a link with a handle for simulating the link loss after power off.
    pub fn connect_with_disconnect(&self) -> (Box<dyn TransportLink>, MockDisconnect) {
        a3116_link(A3116_STATE_REPLY)
    }

    /// Serves firmware 00.99, which does not expose the voice prompt flag.
    pub fn with_old_firmware() -> impl TransportProvider {
        A3116OldFirmware
    }
}

fn a3116_link(state: &[u8]) -> (Box<dyn TransportLink>, MockDisconnect) {
    open_link(Some(Responder::with_state(
        ModelDriver::A3116,
        ResponderOptions {
            acknowledge_writes: true,
            apply_equalizer: true,
        },
        Arc::new(Mutex::new(fixture_body(ModelDriver::A3116, state))),
    )))
}

#[async_trait]
impl TransportProvider for VirtualA3116 {
    async fn connect(
        &self,
        _target: &ConnectTarget,
    ) -> Result<Box<dyn TransportLink>, TransportError> {
        Ok(a3116_link(A3116_STATE_REPLY).0)
    }
}

struct A3116OldFirmware;

#[async_trait]
impl TransportProvider for A3116OldFirmware {
    async fn connect(
        &self,
        _target: &ConnectTarget,
    ) -> Result<Box<dyn TransportLink>, TransportError> {
        Ok(a3116_link(A3116_OLD_FIRMWARE_STATE_REPLY).0)
    }
}

/// A virtual P20i with a gaming-mode flag backed by measured responses.
pub struct VirtualA3949;

/// A virtual A3028 that answers state reads from the family reference.
pub struct VirtualA3028;

impl VirtualA3028 {
    /// Serves the same state with the documented auto power off extra fields.
    pub fn with_auto_power_off() -> impl TransportProvider {
        FamilyWithAutoPowerOff(ModelDriver::A3028)
    }
}

/// A virtual A3029 that answers state reads from the family reference.
pub struct VirtualA3029;

fn family_link(model: ModelDriver) -> Box<dyn TransportLink> {
    family_link_with_state(model, initial_state(model))
}

fn family_link_with_state(
    model: ModelDriver,
    state: Arc<Mutex<Vec<u8>>>,
) -> Box<dyn TransportLink> {
    open_link(Some(Responder::with_state(
        model,
        ResponderOptions {
            acknowledge_writes: true,
            apply_equalizer: true,
        },
        state,
    )))
    .0
}

#[async_trait]
impl TransportProvider for VirtualA3028 {
    async fn connect(
        &self,
        _target: &ConnectTarget,
    ) -> Result<Box<dyn TransportLink>, TransportError> {
        Ok(family_link(ModelDriver::A3028))
    }
}

#[async_trait]
impl TransportProvider for VirtualA3029 {
    async fn connect(
        &self,
        _target: &ConnectTarget,
    ) -> Result<Box<dyn TransportLink>, TransportError> {
        Ok(family_link(ModelDriver::A3029))
    }
}

struct FamilyWithAutoPowerOff(ModelDriver);

#[async_trait]
impl TransportProvider for FamilyWithAutoPowerOff {
    async fn connect(
        &self,
        _target: &ConnectTarget,
    ) -> Result<Box<dyn TransportLink>, TransportError> {
        let mut body = initial_state(self.0)
            .lock()
            .expect("virtual state lock")
            .clone();
        body.extend_from_slice(&AUTO_POWER_OFF_EXTRA_FIELDS);
        Ok(family_link_with_state(self.0, Arc::new(Mutex::new(body))))
    }
}

#[async_trait]
impl TransportProvider for VirtualA3949 {
    async fn connect(
        &self,
        _target: &ConnectTarget,
    ) -> Result<Box<dyn TransportLink>, TransportError> {
        let model = ModelDriver::A3949;
        Ok(open_link(Some(Responder::with_state(
            model,
            ResponderOptions {
                acknowledge_writes: true,
                apply_equalizer: false,
            },
            initial_state(model),
        )))
        .0)
    }
}

/// A virtual device whose state is shared by successive connections.
pub struct SharedVirtualSoundcoreDevice {
    state: Arc<Mutex<Vec<u8>>>,
}

impl SharedVirtualSoundcoreDevice {
    /// Opens a link with fault injection and no sound-mode ACK.
    pub fn connect_with_fault_injection(&self) -> (Box<dyn TransportLink>, MockDisconnect) {
        open_link(Some(Responder::with_state(
            ModelDriver::A3954,
            ResponderOptions {
                acknowledge_writes: false,
                apply_equalizer: false,
            },
            Arc::clone(&self.state),
        )))
    }
}

/// How the virtual responder treats device writes.
#[derive(Clone, Copy)]
struct ResponderOptions {
    /// Whether a successful write is answered with an empty ACK.
    acknowledge_writes: bool,
    /// Whether an A3062 equalizer write changes the virtual device's curve.
    apply_equalizer: bool,
}

pub struct Responder {
    inner: ResponderKind,
}

enum ResponderKind {
    Device(DeviceResponder),
    ReadOnly(ReadOnlyResponder),
}

impl Responder {
    pub fn new(acknowledge_writes: bool) -> Self {
        Self {
            inner: ResponderKind::Device(DeviceResponder::new(acknowledge_writes)),
        }
    }

    fn with_state(
        model: ModelDriver,
        options: ResponderOptions,
        state: Arc<Mutex<Vec<u8>>>,
    ) -> Self {
        Self {
            inner: ResponderKind::Device(DeviceResponder::with_state(model, options, state)),
        }
    }

    fn read_only(identity: Bytes, state: Vec<u8>) -> Self {
        Self {
            inner: ResponderKind::ReadOnly(ReadOnlyResponder::new(identity, state)),
        }
    }

    pub fn reply(&mut self, chunk: &[u8]) -> Option<Bytes> {
        match &mut self.inner {
            ResponderKind::Device(responder) => responder.reply(chunk),
            ResponderKind::ReadOnly(responder) => responder.reply(chunk),
        }
    }
}

struct DeviceResponder {
    model: ModelDriver,
    decoder: FrameDecoder,
    state: Arc<Mutex<Vec<u8>>>,
    options: ResponderOptions,
    /// The A3116 voice prompt flag, which is read by its own command.
    voice_prompt: bool,
}

impl DeviceResponder {
    fn new(acknowledge_writes: bool) -> Self {
        let model = ModelDriver::A3954;
        Self::with_state(
            model,
            ResponderOptions {
                acknowledge_writes,
                apply_equalizer: false,
            },
            initial_state(model),
        )
    }

    fn with_state(
        model: ModelDriver,
        options: ResponderOptions,
        state: Arc<Mutex<Vec<u8>>>,
    ) -> Self {
        Self {
            model,
            decoder: FrameDecoder::new(model.framing()),
            state,
            options,
            voice_prompt: true,
        }
    }

    fn reply(&mut self, chunk: &[u8]) -> Option<Bytes> {
        self.decoder.push(chunk);
        let mut replies = BytesMut::new();
        let checksum_kind = self.model.framing();
        loop {
            let frame = match self.decoder.next_frame() {
                Ok(Some(frame)) => frame,
                Ok(None) => break,
                Err(_) => continue,
            };
            if frame.direction != Direction::Outbound {
                continue;
            }
            if frame.command == IDENTITY_COMMAND && frame.body.is_empty() {
                if self.model == ModelDriver::A3954 {
                    replies.extend_from_slice(IDENTITY_REPLY);
                }
            } else if frame.command == self.model.state_command() && frame.body.is_empty() {
                let state = self.state.lock().expect("virtual state lock");
                if let Ok(state) =
                    encode_frame_with(checksum_kind, Direction::Inbound, frame.command, &state)
                {
                    replies.extend_from_slice(&state);
                }
            } else if self.model == ModelDriver::A3954
                && frame.command == DUAL_CONNECTIONS_COMMAND
                && frame.body.is_empty()
            {
                // The fixture has no stored peers. Reply with one empty list segment.
                if let Ok(peers) = encode_frame(Direction::Inbound, frame.command, &[1, 1]) {
                    replies.extend_from_slice(&peers);
                }
            } else if frame.command == SOUND_MODE_COMMAND
                && self.apply_sound_mode(&frame.body)
                && self.options.acknowledge_writes
            {
                replies.extend_from_slice(SOUND_MODE_ACK);
            } else if self.model == ModelDriver::A3062
                && frame.command == EQUALIZER_COMMAND
                && self.options.acknowledge_writes
                && self.equalizer_applied(&frame.body)
                && let Ok(ack) = encode_frame(Direction::Inbound, EQUALIZER_COMMAND, &[])
            {
                replies.extend_from_slice(&ack);
            } else if matches!(self.model, ModelDriver::A3028 | ModelDriver::A3029)
                && frame.command == A3028_EQUALIZER_COMMAND
                && self.options.acknowledge_writes
                && self.equalizer_applied(&frame.body)
                && let Ok(ack) = encode_frame(Direction::Inbound, A3028_EQUALIZER_COMMAND, &[])
            {
                replies.extend_from_slice(&ack);
            } else if self.model == ModelDriver::A3949
                && frame.command == GAMING_MODE_COMMAND
                && self.apply_gaming_mode(&frame.body)
                && self.options.acknowledge_writes
                && let Ok(ack) = encode_frame(Direction::Inbound, GAMING_MODE_COMMAND, &[])
            {
                replies.extend_from_slice(&ack);
            } else if matches!(self.model, ModelDriver::A3028 | ModelDriver::A3029)
                && frame.command == AUTO_POWER_OFF_COMMAND
                && self.auto_power_off_applied(&frame.body)
                && self.options.acknowledge_writes
                && let Ok(ack) = encode_frame(Direction::Inbound, AUTO_POWER_OFF_COMMAND, &[])
            {
                replies.extend_from_slice(&ack);
            } else if self.model == ModelDriver::A3116
                && frame.command == A3116_VOLUME_COMMAND
                && self.a3116_volume_applied(&frame.body)
                && self.options.acknowledge_writes
                && let Ok(ack) =
                    encode_frame_with(checksum_kind, Direction::Inbound, A3116_VOLUME_COMMAND, &[])
            {
                replies.extend_from_slice(&ack);
            } else if self.model == ModelDriver::A3116
                && frame.command == A3116_EQUALIZER_PRESET_COMMAND
                && self.a3116_equalizer_preset_applied(&frame.body)
                && self.options.acknowledge_writes
                && let Ok(ack) = encode_frame_with(
                    checksum_kind,
                    Direction::Inbound,
                    A3116_EQUALIZER_PRESET_COMMAND,
                    &[],
                )
            {
                replies.extend_from_slice(&ack);
            } else if self.model == ModelDriver::A3116
                && frame.command == A3116_EQUALIZER_BANDS_COMMAND
                && self.a3116_equalizer_bands_applied(&frame.body)
                && self.options.acknowledge_writes
                && let Ok(ack) = encode_frame_with(
                    checksum_kind,
                    Direction::Inbound,
                    A3116_EQUALIZER_BANDS_COMMAND,
                    &[],
                )
            {
                replies.extend_from_slice(&ack);
            } else if self.model == ModelDriver::A3116
                && frame.command == A3116_AUTO_POWER_OFF_COMMAND
                && self.a3116_auto_power_off_applied(&frame.body)
                && self.options.acknowledge_writes
                && let Ok(ack) = encode_frame_with(
                    checksum_kind,
                    Direction::Inbound,
                    A3116_AUTO_POWER_OFF_COMMAND,
                    &[],
                )
            {
                replies.extend_from_slice(&ack);
            } else if self.model == ModelDriver::A3116
                && frame.command == A3116_VOICE_PROMPT_READ_COMMAND
                && frame.body.is_empty()
                && let Ok(reply) = encode_frame_with(
                    checksum_kind,
                    Direction::Inbound,
                    A3116_VOICE_PROMPT_READ_COMMAND,
                    &[u8::from(self.voice_prompt)],
                )
            {
                replies.extend_from_slice(&reply);
            } else if self.model == ModelDriver::A3116
                && frame.command == A3116_VOICE_PROMPT_COMMAND
                && self.a3116_voice_prompt_applied(&frame.body)
                && self.options.acknowledge_writes
                && let Ok(ack) = encode_frame_with(
                    checksum_kind,
                    Direction::Inbound,
                    A3116_VOICE_PROMPT_COMMAND,
                    &[],
                )
            {
                replies.extend_from_slice(&ack);
            } else if self.model == ModelDriver::A3116
                && frame.command == A3116_POWER_OFF_COMMAND
                && frame.body.is_empty()
                && self.options.acknowledge_writes
                && let Ok(ack) = encode_frame_with(
                    checksum_kind,
                    Direction::Inbound,
                    A3116_POWER_OFF_COMMAND,
                    &[],
                )
            {
                replies.extend_from_slice(&ack);
            }
        }
        (!replies.is_empty()).then(|| replies.freeze())
    }

    fn apply_gaming_mode(&self, body: &[u8]) -> bool {
        let [enabled @ 0..=1] = body else {
            return false;
        };
        let captured = fixture_body(self.model, A3949_GAMING_REPLIES[usize::from(*enabled)]);
        let mut state = self.state.lock().expect("virtual state lock");
        state[A3949_GAMING_MODE] = captured[A3949_GAMING_MODE];
        true
    }

    fn apply_sound_mode(&self, body: &[u8]) -> bool {
        let mut state = self.state.lock().expect("virtual state lock");
        match self.model {
            ModelDriver::A3954 => SoundModeWrite::decode(body)
                .and_then(|write| write.apply_to_state(&mut state))
                .is_ok(),
            ModelDriver::A3062 => {
                // The outgoing golden fixtures and the measured responses use this control profile.
                let [mode @ 0..=2, 0x51, 1, 1, 0, 5] = body else {
                    return false;
                };
                let captured = fixture_body(self.model, A3062_MODE_REPLIES[usize::from(*mode)]);
                state[A3062_SOUND_MODE_FIELDS].copy_from_slice(&captured[A3062_SOUND_MODE_FIELDS]);
                true
            }
            ModelDriver::A3028 | ModelDriver::A3029 => {
                let [_mode @ 0..=2, _nc @ 0..=3, _transparency @ 0..=1, _custom] = body else {
                    return false;
                };
                state[A3028_SOUND_MODE_FIELDS].copy_from_slice(body);
                true
            }
            ModelDriver::A3949 | ModelDriver::A3116 => false,
        }
    }

    /// Applies an equalizer write; a withheld application answers nothing.
    fn equalizer_applied(&self, body: &[u8]) -> bool {
        if !self.options.apply_equalizer {
            return false;
        }
        let mut state = self.state.lock().expect("virtual state lock");
        match self.model {
            ModelDriver::A3062 => {
                // The write body starts with the preset, the hear-id genre, and the bands.
                let (Some(preset), Some(bands)) = (body.get(0..2), body.get(4..14)) else {
                    return false;
                };
                // The state carries the curve at the offsets the models crate measured.
                state[A3062_EQUALIZER_PRESET].copy_from_slice(preset);
                state[A3062_EQUALIZER_BANDS].copy_from_slice(bands);
                true
            }
            ModelDriver::A3028 | ModelDriver::A3029 => {
                // The family write is the preset followed directly by eight bands.
                let (Some(preset), Some(bands)) = (body.get(0..2), body.get(2..10)) else {
                    return false;
                };
                state[A3028_EQUALIZER_PRESET].copy_from_slice(preset);
                state[A3028_EQUALIZER_BANDS].copy_from_slice(bands);
                true
            }
            ModelDriver::A3949 | ModelDriver::A3954 | ModelDriver::A3116 => false,
        }
    }

    /// Applies an auto power off write only when the state carries extra fields.
    fn auto_power_off_applied(&self, body: &[u8]) -> bool {
        let [_enabled @ 0..=1, _duration] = body else {
            return false;
        };
        let mut state = self.state.lock().expect("virtual state lock");
        let Some(value) = state.get_mut(A3028_AUTO_POWER_OFF) else {
            return false;
        };
        value.copy_from_slice(body);
        true
    }

    fn a3116_volume_applied(&self, body: &[u8]) -> bool {
        let [volume @ 0..=16] = body else {
            return false;
        };
        let mut state = self.state.lock().expect("virtual state lock");
        state[A3116_VOLUME_FIELD] = *volume;
        true
    }

    fn a3116_equalizer_preset_applied(&self, body: &[u8]) -> bool {
        let [preset] = body else {
            return false;
        };
        let mut state = self.state.lock().expect("virtual state lock");
        state[A3116_EQUALIZER_PRESET] = *preset;
        true
    }

    fn a3116_equalizer_bands_applied(&self, body: &[u8]) -> bool {
        if body.len() != A3116_EQUALIZER_BANDS.len() {
            return false;
        }
        let mut state = self.state.lock().expect("virtual state lock");
        state[A3116_EQUALIZER_BANDS].copy_from_slice(body);
        true
    }

    fn a3116_auto_power_off_applied(&self, body: &[u8]) -> bool {
        let [duration @ 1..=4] = body else {
            return false;
        };
        let mut state = self.state.lock().expect("virtual state lock");
        state[A3116_AUTO_POWER_OFF_FIELD] = *duration;
        true
    }

    fn a3116_voice_prompt_applied(&mut self, body: &[u8]) -> bool {
        let [enabled @ 0..=1] = body else {
            return false;
        };
        self.voice_prompt = *enabled == 1;
        true
    }
}

/// A virtual device whose identity is outside the measured registry.
pub struct VirtualUnknownIdentity;

#[async_trait]
impl TransportProvider for VirtualUnknownIdentity {
    async fn connect(
        &self,
        _target: &ConnectTarget,
    ) -> Result<Box<dyn TransportLink>, TransportError> {
        let identity = encode_frame(
            Direction::Inbound,
            IDENTITY_COMMAND,
            b"04.3004.309999UNKNOWN00000",
        )
        .expect("valid unknown identity frame");
        let state = fixture_body(ModelDriver::A3954, A3954_STATE_REPLY);
        Ok(open_link(Some(Responder::read_only(identity, state))).0)
    }
}

/// A virtual A3954 that answers its state request with an unconfirmed layout.
pub struct VirtualIncompatibleA3954;

#[async_trait]
impl TransportProvider for VirtualIncompatibleA3954 {
    async fn connect(
        &self,
        _target: &ConnectTarget,
    ) -> Result<Box<dyn TransportLink>, TransportError> {
        let mut state = fixture_body(ModelDriver::A3954, A3954_STATE_REPLY);
        state.extend_from_slice(b"UNTRUSTED-STATE");
        Ok(open_link(Some(Responder::read_only(
            Bytes::from_static(IDENTITY_REPLY),
            state,
        )))
        .0)
    }
}

struct ReadOnlyResponder {
    identity: Bytes,
    state: Vec<u8>,
    decoder: FrameDecoder,
}

impl ReadOnlyResponder {
    fn new(identity: Bytes, state: Vec<u8>) -> Self {
        Self {
            identity,
            state,
            decoder: FrameDecoder::default(),
        }
    }

    fn reply(&mut self, chunk: &[u8]) -> Option<Bytes> {
        self.decoder.push(chunk);
        let mut replies = BytesMut::new();
        loop {
            let frame = match self.decoder.next_frame() {
                Ok(Some(frame)) => frame,
                Ok(None) => break,
                Err(_) => continue,
            };
            if frame.direction != Direction::Outbound {
                continue;
            }
            if frame.command == IDENTITY_COMMAND && frame.body.is_empty() {
                replies.extend_from_slice(&self.identity);
            } else if frame.command == STATE_COMMAND
                && frame.body.is_empty()
                && let Ok(state) = encode_frame(Direction::Inbound, STATE_COMMAND, &self.state)
            {
                replies.extend_from_slice(&state);
            }
        }
        (!replies.is_empty()).then(|| replies.freeze())
    }
}

fn initial_state(model: ModelDriver) -> Arc<Mutex<Vec<u8>>> {
    let bytes = match model {
        ModelDriver::A3028 => A3028_STATE_BODY.to_vec(),
        ModelDriver::A3029 => A3029_STATE_BODY.to_vec(),
        ModelDriver::A3062 => fixture_body(model, A3062_MODE_REPLIES[2]),
        ModelDriver::A3116 => fixture_body(model, A3116_STATE_REPLY),
        ModelDriver::A3949 => fixture_body(model, A3949_STATE_REPLY),
        ModelDriver::A3954 => fixture_body(model, A3954_STATE_REPLY),
    };
    Arc::new(Mutex::new(bytes))
}

fn fixture_body(model: ModelDriver, bytes: &[u8]) -> Vec<u8> {
    let mut decoder = FrameDecoder::new(model.framing());
    decoder.push(bytes);
    let state = decoder
        .next_frame()
        .expect("valid virtual state fixture")
        .expect("complete virtual state fixture");
    assert_eq!(state.command, model.state_command());
    assert_eq!(state.direction, Direction::Inbound);
    state.body.to_vec()
}
