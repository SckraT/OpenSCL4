# Android connection reliability and availability spec

Status: draft for implementation, 2026-10-21.
Scope: Android client and the shared Rust core. This spec is the result of the wayfinder map
"Карта: статус, LDAC и восстановление Android-сессии" (issues #18-#27). It does not implement code.

## 1. Principles

- The device is the source of truth. A value is shown as applied only after the device reports it.
- Two independent facts per device: whether the phone has a Bluetooth link, and whether OpenSCL4
  controls the device. Never collapse them into one status.
- Availability is evidence-based. The official Soundcore app's disabled list is not evidence.
  Settings are blocked only when the device ignores them or cannot expose a usable value.
- Shared behavior lives in the Rust core (ADR-0001). Android-specific facts (Bluetooth link,
  permissions, activity lifecycle) stay in Android adapters.
- Planning stays in the issue tracker. Implementation is tracked by the issues listed in section 7.

## 2. Domain terms

`Возможность устройства` (device capability): a feature of the model/firmware, independent of the
current state. Example: A3954 has EQ, LDAC and multipoint; a model without `dualConnections` has no
multipoint.

`Доступность настройки` (setting availability): whether a setting can be changed in the current
device state. `Available`, `ReadOnly` or `Unavailable`, always with a reason for the latter two.
Availability is not `SettingMode`: `SettingMode` is static (read-write, information, action).

Phone link: any Bluetooth link between this phone and the device (classic profiles, ACL, or an
active vendor session). It is not the same as "audio is routed through this phone".

App control: an active OpenSCL4 RFCOMM session for the device. There is at most one controlled
device per client.

Multipoint: the earbuds are connected to more than one source device. TWS: the two earbuds of one
pair are connected to each other. Do not use one word for both.

## 3. Device status (decision #19)

### 3.1 Model

```kotlin
enum class PhoneBluetoothLink { Linked, NotLinked, Unknown }
enum class AppControl { Controlling, Connecting, NotControlling }
enum class BluetoothAccess { Ready, Off, PermissionDenied, Unsupported, Unknown }

data class DeviceRowState(
    val device: PairedDevice,
    val link: PhoneBluetoothLink,
    val control: AppControl,
)

data class DeviceListState(
    val access: BluetoothAccess,
    val featured: DeviceRowState?,
    val others: List<DeviceRowState>,
    val isLoading: Boolean = false,
)
```

The state is produced by a pure presenter. The Android `PhoneBluetoothLink` adapter is thin: it
provides a snapshot plus a flow of updates, and performs no presentation decisions.

### 3.2 Phone link source

The effective link set is the union of:

1. a profile scan seed on start/resume. Classic profiles are read through
   `BluetoothAdapter.getProfileProxy` (A2DP, HEADSET): on Android 16
   `BluetoothManager.getConnectedDevices` throws `IllegalArgumentException: Profile not
   supported` for them. `BluetoothManager.getConnectedDevices` is used only for
   GATT/GATT_SERVER. Lowercased MAC addresses; unsupported profiles skipped;
2. live `ACTION_ACL_CONNECTED` / `ACTION_ACL_DISCONNECTED` broadcasts;
3. evidence from an active app session (`AppControl.Controlling` implies a link).

Bluetooth off, permission denied, or every profile read failing means `BluetoothAccess != Ready`
and `Unknown` for every row. It never means `NotLinked`. No polling: refresh on monitor start and
`ON_RESUME`; update from broadcasts while the list is visible. If ACL delivery proves unreliable on
the target phone, fall back to profile-state broadcasts plus resume refresh; verify this on the
hardware.

Invariants:

- If `control == Controlling`, `link` must not be rendered as `NotLinked`; coerce to `Unknown`.
- MAC comparison is case-insensitive.
- Demo devices have no real link/control axis and keep the "Демо" state.

### 3.3 Presentation

Featured card: two badges (link and app control). Other rows: one combined line that does not hide a
divergence.

| Link | Control | Featured badges | Other row | Tap |
|---|---|---|---|---|
| Linked | Controlling | «По Bluetooth подключено» + «Управление активно» | «Подключено» | Open |
| Linked | Connecting | «По Bluetooth подключено» + «Подключение…» | «Подключение…» | None |
| Linked | NotControlling | «По Bluetooth подключено» + «Управление недоступно» | «По Bluetooth · без управления» | Connect |
| NotLinked | NotControlling | «По Bluetooth не подключено» + «Управление недоступно» | «Не подключено» | Connect |
| Unknown | NotControlling | «Bluetooth: состояние неизвестно» + «Управление недоступно» | «Bluetooth-статус неизвестен» | Connect / grant |
| Unknown | Controlling | «Bluetooth не подтверждён» + «Управление активно» | «Подключено» | Open |

Featured order: controlling > phone-linked > last used > first saved. Keep the order stable while
the list is open; do not re-sort on every link event.

Tap on a linked but uncontrolled device: stay on the list, show the row as `Connecting`, navigate to
the device screen only after `Connected`. If access is not `Ready`, run the banner action instead.
Switching control closes the previous session (existing controller behavior).

Banner for `BluetoothAccess`: off ("Bluetooth выключен"), permission denied, unsupported, unknown;
reuse the existing Bluetooth settings/permission flows.

### 3.4 Tests

JVM presenter tests: all four link × control combinations on the featured card and a row; MAC
case-fold; two linked devices with one controlled; link changes while the list is open; control lost
while link remains; Bluetooth off/permission denied/all profiles unsupported; profile scan empty but
session active; demo device; featured precedence; labels/actions. The Android adapter gets a small
wiring test for profile union, ACL add/remove, adapter-off clear and permission revocation. Compose
UI coverage is optional.

## 4. Confirmed setting writes (decision #20)

### 4.1 Contract

```rust
struct SettingWriteOutcome {
    setting_id: SettingId,
    outcome: WriteOutcome,
}

enum WriteOutcome {
    Applied { observed: Value },
    UnsupportedInState { reason: AvailabilityReason },
    Rejected { observed: Option<Value> },
    TimedOut,
    Disconnected,
}

async fn set_setting_values(
    values: Vec<(SettingId, Value)>,
) -> Result<Vec<SettingWriteOutcome>, Error>;
```

- One outcome per requested setting, in request order. `Err` is only for local errors (unknown
  setting, invalid value, API misuse, internal error).
- `Applied` is the only outcome that may move the UI to the new value. An ACK is never `Applied`.
- `Applied` requires a device-reported value equal to the request (read-back or a fresh state update).
- `Rejected` means the device reported a different value; publish `observed` if present.
- `TimedOut` and `Disconnected` publish nothing; the last confirmed value stays.

### 4.2 Confirmation

- SET ACKs are empty on Liberty 4 Pro and must not be parsed as success.
- LDAC: targeted GET `0x01 0x7F`, body parsed as a boolean.
- `VoicePrompt`: targeted `0x01 0x10` only on A3116 (proven request). Other models use bulk.
- All other flags/settings: bulk `0x01 0x01` through the existing state-update handler, compare the
  public `Setting` values (respect inverted flags).
- A writable setting with no verifier is `UnsupportedInState { NoReadBack }` and is not sent.
- Default confirmation deadline 2 s; LDAC 15 s (mode switch ≈10 s). If the link drops, return
  `Disconnected` immediately.
- The write path never calls `send_modify(target)`. It publishes only observed values. Delete the
  optimistic publication in `FlagStateModifier` and equivalent state modifiers.
- One write mutex per device; at least 250 ms between outbound setting commands; coalesce slider
  values to the latest value per setting.

### 4.3 Batch and presets

Batches are ordered, best-effort, non-atomic and without rollback. `UnsupportedInState` is reported
per setting and does not abort the rest. `TimedOut`/`Rejected` do not stop other settings.
`Disconnected` stops the batch and marks the remaining requested settings `Disconnected`.

Preset activation: apply all fields, retry `Rejected` fields once in order, and mark the preset as
applied only if every requested setting ends `Applied` (or was already equal). A blocked field
returns `UnsupportedInState`; no silent skip, no deferred application.

### 4.4 UI

Android keeps a per-setting pending map `{ requested value, generation }`. The control shows the
requested value disabled/spinner until the confirmed state arrives. `Applied` clears the pending
entry after the confirmed value is visible. `Rejected`/`TimedOut` revert to the last confirmed value
with an inline error. `UnsupportedInState` shows the current value and the availability reason.
`Disconnected` hands over to the connection UI. A newer write supersedes an older pending entry;
stale outcomes are ignored. Quick tile and widget read confirmed state only.

Desktop/Slint emits the confirmed snapshot after every write and a typed failure event for non-
`Applied` outcomes. CLI prints one line per outcome and exits non-zero if any is not `Applied`.
Legacy GUI maps non-`Applied` to its existing error path.

### 4.5 API impact

Breaking change is allowed. Update the trait in `lib/src/api/device.rs`, the implementation, UniFFI
(`android/src/device.rs` and serializable records), Android view model/editor/tile, desktop
controller/settings event handling, CLI and legacy GUI. Update model/demo tests, `SettingTest.kt`
(remove optimistic assertions) and every test that maps SET commands to empty ACKs.

## 5. Setting availability (decisions #21, #26, #27)

### 5.1 Model

```rust
enum SettingAvailability {
    Available,
    ReadOnly { reason: AvailabilityReason },
    Unavailable { reason: AvailabilityReason },
}

enum AvailabilityReason {
    RequiresBothEarbuds,
}
```

- `ReadOnly`: the device still reports a meaningful value, but it cannot be changed now. Show the
  value, disable the control, show the reason.
- `Unavailable`: the device does not report a usable value, or the function does not work in this
  state. Show the reason; do not show a stale value.
- A setting the model does not have is not `Unavailable`; it is absent (no `Возможность`).

The source is device-reported state only: `TwsStatus` and `DualConnections`. Phone audio routing is
not an input.

### 5.2 Rules with one earbud

| Setting | Availability | Reason |
|---|---|---|
| ANC, transparency, auto power off, wearing detection, safe volume, quick chat, leak compensation, multipoint | `Available` | — |
| EQ (`presetEqualizerProfile`, `customEqualizerProfile`, `volumeAdjustments`) | `Unavailable` | `RequiresBothEarbuds` |
| LDAC | `ReadOnly` | `RequiresBothEarbuds` |
| Untested settings (buttons, case, voice/tone, find device, fit test) | `Available` by default | — |

Models without `TwsStatus`: everything `Available` (the one-earbud condition cannot be determined).
`DualConnections` never gates LDAC or EQ. LDAC and multipoint are compatible.

The one-earbud condition is exactly `TwsStatus == "Disconnected"`, not a missing battery level or a
channel count.

### 5.3 UI and presets

Disabled controls keep the row and show the reason. Do not hide a setting because of one earbud.
The one-earbud banner text must be corrected: EQ is unavailable and LDAC cannot be switched; other
settings can still be changed. The old "эквалайзер и некоторые другие настройки" text is wrong.

Presets use each field's availability: apply available fields, return `UnsupportedInState` for
blocked fields, and do not mark the preset applied. Quick toggles and the LDAC tile use the same
availability: show the current LDAC state with a reason, not `STATE_UNAVAILABLE`, and do not send a
write while `ReadOnly`. `STATE_UNAVAILABLE` remains only for "no connected device with LDAC".

### 5.4 Glossary

Update `GLOSSARY.md`:

- `Возможность устройства`: a feature of the model/firmware, not the current state.
- `Доступность настройки`: whether a setting can be changed in the current state; one of
  `Available`, `ReadOnly`, `Unavailable`, with a reason.

## 6. Connection recovery (decision #22)

### 6.1 Module and states

Add a shared Rust `ConnectionSession` between `RfcommConnection` and `PacketIOController`. It owns
the durable session across sockets: state machine, reconnect, backoff, deadlines, stable read
channel and status.

States: `Connecting`, `Connected`, `Recovering { attempt, reason }`, `Lost { reason }`, `Closed`.

- Unexpected transport loss always enters `Recovering`; never `Connected -> Lost` directly.
- `Lost` only after the recovery budget or a terminal cause (unpaired, adapter unavailable,
  permission/security failure, explicit close).
- `Closed` only for explicit disconnect/switch/unpair/service destroy.

### 6.2 Budget and deadlines

- Recovery window 30 s; attempts at 0/1/3/7/15/23 s; backoff 1/2/4/8/8 s with ±20% jitter.
- Per-attempt connect deadline 5 s; absolute cutoff at 30 s aborts an in-flight attempt.
- Write deadline 2 s; `send_with_response` total 5 s; `wait_for_start` 3 s; keep the ACK budget
  500/1000/1500 ms (do not raise it as a fix).
- Silence watchdog: 30 s with no inbound packet -> `RequestState`; probe 3 s; one retry after 1 s;
  two failures -> `Recovering`.
- On recovery, keep the `OpenSCQ30Device` object and re-run `fetch_state` before announcing
  `Connected`. If the fetch fails, stay in `Recovering` within the budget.

Initial connect uses the same 30 s budget (product decision; differs from the original brief
recommendation). After `Lost`, a new 30 s budget starts on a link-available event, foreground/service
start or manual Retry; not a tight infinite loop.

### 6.3 Queue and transport fixes

- `MultiQueue::cancel` closes the cancelled handle's semaphore.
- Bound `wait_for_start`; cancel the handle on timeout.
- On multi-response timeout, cancel the popped head and return an error/partial result, never
  `Ok(())`.
- Use a drop guard so a cancelled send always removes/closes its handle.
- On transport loss, fail all pending commands; do not replay stale commands after reconnect.
- Android adapter: bounded `socket.connect`, propagate write errors, report EOF/IO, no retry loop.
- `set_setting_values` while `Recovering`/`Lost` returns `Disconnected` immediately.

### 6.4 UI

`Recovering`: keep the current screen and last-known values; show a non-blocking "Переподключение…"
banner after 1 s; controls read-only/fail fast; do not clear settings or navigate away. The ~10 s
LDAC/AAC switch must appear as `Recovering`, not "Связь потеряна".

`Lost`: show after the budget with a reason and Retry; keep the service and foreground notification
for 5 minutes; keep the last snapshot dimmed. `Closed`: clear settings, unbind, stop the service.

Desktop maps `Recovering` to a banner with disabled controls and `Lost` to its existing lost UI.

### 6.5 Tests

Rust session tests with a scripted backend: drop at t=0 and reconnect at t=10; budget exhaustion at
30 s; terminal error; explicit close; write/ACK/queue timeouts; cancel-head regression; watchdog;
state re-fetch failure; writes during `Recovering`. Android JVM tests for service/controller
lifecycle, notification/widget states and no duplicate session during `Recovering`. Hardware test:
LDAC/AAC switch, TWS reconnect, multipoint host switch.

## 7. Implementation task graph

All issues are normal implementation issues, not wayfinder tickets. Labels: `enhancement` plus
`ready-for-agent`, except hardware QA which is `ready-for-human`. Dependencies are native GitHub
blocking links.

| Issue | Scope | Blocked by |
|---|---|---|
| A. PhoneBluetoothLink + DeviceListState | Android link adapter, pure presenter, JVM tests | — |
| B. Two-axis device list UI | Compose badges/rows/banner, tap flow, strings | A |
| C. Setting availability core | `SettingAvailability`, reasons, one-earbud rules, glossary | — |
| D. Confirmed write core | `WriteOutcome`, read-back, no optimistic publish, mutex/pacing, tests | C |
| E. WriteOutcome client migration | UniFFI, Android, desktop, CLI, legacy GUI, tests | D |
| F. Availability and write UI | disabled+reason, one-earbud banner, presets, tile, desktop parity | E |
| G. ConnectionSession core | state machine, backoff, deadlines, watchdog, queue fixes, re-fetch | — |
| H. Android recovery integration | ConnectionStatus, service lifecycle, banner, notification, Retry | G |
| I. Hardware QA | Pixel 8 Pro / Liberty 4 Pro: status, writes, availability, recovery | B, F, H |

Frontier: A, C, G. Hardware QA is last. The spec is the source for all issue bodies.

## 8. Open risks

- Android 16: `BluetoothManager.getConnectedDevices` is not supported for classic A2DP/HEADSET.
  The monitor and the `AutoConnectService` start reconcile share `BluetoothProfileDevices`, which
  uses `getProfileProxy` (#38). Re-checked on Pixel 8 Pro (Android 16) at `52eb82ea`: the start
  reconcile sees already-connected headphones and starts the device once, and the monitor shows
  the link after a cold start. After Bluetooth off/on the monitor stays at an unknown link until a
  later re-read (#52; `f714e6cd` behaves the same).
- ACL broadcast delivery is OEM/Android-version dependent; verify on Pixel 8 Pro and keep the
  profile/resume fallback.
- EQ device acceptance at one earbud is inferred from the core tests, not measured directly; the
  matrix confirms the UI is blocked in both clients.
- Fit test, button configuration, case settings, voice/tone and find device were not tested at one
  earbud. They default to `Available` under allow-by-evidence; add a specific reason only if a test
  proves otherwise.
- Some flags have no proven GET command; they rely on bulk state read-back. A future model without
  that path must return `UnsupportedInState { NoReadBack }` rather than lie.
- The LDAC/AAC switch timing is ~10 s on A3954; the 30 s recovery window has margin, but logs from
  other models may require tuning.
- Brand-new transport drops during `Recovering` must not replay stale writes; the state re-fetch is
  authoritative.

## 9. References

- Map: https://github.com/SckraT/OpenSCL4/issues/18
- Decisions: #19, #20, #21, #22, #23, #25, #26, #27
- Briefs: `brief-19.md`, `brief-20.md`, `brief-22.md`, `brief-27.md` in the architecture notes dir.
- Research: `docs/budslink-research.md`.
- ADR-0001: shared behavior in the Rust core; Android keeps platform lifecycle.
