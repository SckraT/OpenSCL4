# BudsLink research: connection status, LDAC, transport, reusable Soundcore protocol

Researched 2026-10-21 against primary sources only (BudsLink source, README, PR/issue text). No OpenSCL4 code was changed except this file.

## What was inspected

- Repo: `https://github.com/maniacx/BudsLink`, shallow clone at commit `f3935512ec459229317e8b875c024af08ca7c2e9` (2026-10-20, "translations: update"), version `0.2.1` (`meson.build:3-4`).
- License: GPL-3.0-or-later (`meson.build:4`; `data/io.github.maniacx.BudsLink.metainfo.xml:50`; full text in `LICENSE`). This is compatible with OpenSCL4's GPL-3.0-or-later (`Cargo.toml:18`), subject to attribution if code is copied.
- **Premise correction:** BudsLink is not an Android app. It is a Linux GNOME/Flatpak app written in GJS (GTK4/libadwaita), using BlueZ over D-Bus and RFCOMM profile file descriptors. Its only Soundcore model is Liberty 4 NC (A3947) (`src/lib/devices/soundcore/soundcoreConfig.js:5-7`). Conclusions below are scoped accordingly.
- OpenSCL4 context read: `GLOSSARY.md`, `docs/development.md:12` ("The device is the source of truth"), `docs/adr/0001-compose-slint-shared-behavior.md`, plus the two architecture notes under `C:\Users\lunsa\AppData\Local\Temp\opencode\openscl4-arch-notes\`.
- Clone note: Git reported case-only filename collisions under `nothingBuds/deviceConfigs/`; this is unrelated to Soundcore and does not affect the inspected files.

The OpenSCL4 behaviors being compared are the ones in the two notes: (a) the list status is the app session, while the phone Bluetooth link is a separate fact; (b) an LDAC write can leave a value the device did not apply; (c) an RFCOMM drop is fatal, with no recovery state or automatic reconnect.

## Q1. How BudsLink decides that a saved device is connected

**BudsLink.** Status comes from the operating system's BlueZ `org.bluez.Device1.Connected` property, not from its own socket. `BluetoothClient` enumerates managed objects, keeps only `Paired` devices, reads `Connected`, and subscribes to `PropertiesChanged` for `Connected`/`Paired` (`src/lib/bluetoothClient.js:47-58,96-132`). The app window adds a row only while `dev.connected` is true and removes it when false (`src/app.js:280-341`). It never probes the vendor session for the status label.

App control is a second, implicit condition: `EnhancedDeviceSupportManager` creates the protocol object only when `type && connected` (`src/lib/devices/enhancedDeviceSupportManager.js:253-332`, Soundcore at `:316-320`), and the row is only attached after the first `0x01 0x01` state reply produced a `DataHandler` (`src/lib/devices/soundcore/soundcoreDevice.js:570-581`; `src/app.js:300-311`). So:
- OS link up + SPP handshake complete: device row appears.
- OS link up + SPP dead on first attempt: no row is created; if no other device works, the window says "No compatible device found" (`src/app.js:338-341`). If the row already existed, it can remain because socket destruction is not propagated to the device object (see Q3).
- There is no label that distinguishes "phone/OS-linked but not app-controlled"; the failure is hidden by removing the row or leaving it stale.

Multipoint is not modelled as a status. BlueZ lists only devices connected to this Linux host; each gets its own path and vendor socket (`src/lib/devices/profileManager.js:280-292`; `src/lib/devices/enhancedDeviceSupportManager.js:253-332`). Other hosts connected to the earbuds are visible only through the earbuds' own `0x0B 0x01` list, shown in Connection Management (`src/lib/devices/soundcore/soundcoreSocket.js:163-171`; `src/preferences/devices/soundcore/configureWindow.js:677-705`). The list's `connected` flag is the earbuds' view; BudsLink never reads which host is the active audio route. A second host is not an app-session concept.

**OpenSCL4.** The Android pill shows the app session: `DeviceConnectionController` emits `Connected` only after `connectWithBackends` returns, and `DeviceStatusPill` maps that state to "Connected"/"Not connected" (`android/app/src/main/java/com/oppzippy/openscq30/features/soundcoredevice/service/DeviceConnectionController.kt:34-68`; `.../ui/sound/DeviceHeroCard.kt:174-195`). Non-featured rows hard-code "Not connected" (`.../ui/deviceselection/screens/DeviceListingScreen.kt:270-276`). The device notes confirm no `BluetoothDevice.isConnected`/profile listener is used for the pill (`C:\Users\lunsa\AppData\Local\Temp\opencode\openscl4-arch-notes\device-status.md:7,30-38`).

**Does it solve the status bug?** No. It is the opposite policy. BudsLink trusts the OS link, which is precisely the signal that can stay true while the vendor channel is dead: its own issue #95 reports "the UI still shows the device connected while nothing works" because a peer stopped sending and no read error fired (`https://github.com/maniacx/BudsLink/issues/95`, section "Bug A"). Copying BudsLink's status source would not give OpenSCL4 a truthful app-session indicator.

## Q2. Codec toggles and write/confirm flow

LDAC is a `Настройка устройства`: its effective value is owned by the earbuds, while BudsLink's GSettings entry is only the client's copy.

### Soundcore LDAC (the directly relevant path)

- Commands: `GET_LDAC = [0x01,0x7F]`, `SET_LDAC = [0x01,0xFF]` (`src/lib/devices/soundcore/soundcoreConfig.js:22-23`). Payload is one byte, `1`/`0` (`src/lib/devices/soundcore/soundcoreSocket.js:191-193`).
- Read at connect: `postConnectInitialization()` sends only `GET_STATE` (`:40-42`). After every `0x01 0x01` state reply, if the model supports LDAC, the socket sends `GET_LDAC` (`:151-154`). BudsLink does not read LDAC from the 159-byte state packet; it always queries it separately.
- Write: the settings switch writes the new value to GSettings first (`src/preferences/devices/soundcore/configureWindow.js:294-305`), the GSettings change handler compares it with the last device-reported value and calls `setLdac` (`src/lib/devices/soundcore/soundcoreDevice.js:220-234`), and `setLdac` just enqueues the packet (`soundcoreSocket.js:191-193`). The UI reads GSettings, so it shows the requested value immediately.
- Confirmation: any inbound `0x017F` frame is treated as current state: `body[0] === 1` is written back to GSettings (`soundcoreSocket.js:115-120`; `soundcoreDevice.js:551-554`). This is better than OpenSCL4's "any same-command packet is success, body ignored", because the body is interpreted.
- But there is **no targeted read-back after SET**, no request/response correlation, no ACK deadline, no retry, and no NACK/status check. If the device ignores the write and sends no `0x017F`, the requested value persists until a later unsolicited `0x017F` or a reconnect. Whether `SET_LDAC` itself elicits a `0x017F` carrying the real state (rather than an echo) is **unconfirmed** from source alone.
- Gating: no TWS/one-earbud awareness at all. The Liberty 4 NC config begins at battery offset 2 and does not parse a TWS field (`src/lib/devices/soundcore/deviceConfigs/Liberty-4-NC.js:14-31`). Multipoint is gated only in one direction: the Connection Management row is made insensitive while LDAC is on (`src/preferences/devices/soundcore/configureWindow.js:133-135,702`), while the LDAC switch stays enabled when multipoint is on; its subtitle warns that LDAC turns off multiple connections and the earbuds reconnect (`:246-248`). That is a UI convention, not a protocol-level precondition check.
- What happens on an ignored write: nothing detects it. For ordinary flags, a later `0x01 0x01` state packet corrects them (`soundcoreSocket.js:224-243`); LDAC has no periodic state packet, so only a `0x017F` or reconnect corrects it.

### Other codec toggles in the same repository

- Cambridge Audio Melomania A100: `SET_CODECS = 0x0400`, `GET_CODECS = 0x0401`, response `0x0501`, LDAC id `0x09`, on-list `[00 01 02 03 04 09]`, off-list `[00 01]` (`src/lib/devices/cambridgeBuds/cambridgeBudsConfig.js:79-80,95,104-106`). Read once at connect (`.../cambridgeBudsSocket.js:57-62`), response checks `payload.includes(0x09)` (`:218-219`), set sends the list (`:296-297`). Same pattern: optimistic, passive correction.
- OpoBuds/Realme/OnePlus: "High-Res LHDC" is feature id `0x18` (`src/lib/devices/opoBuds/opoBudsConfig.js:120,135`); the write is a feature switch (`src/lib/devices/opoBuds/opoBudsSocket.js:897-900`) with a 500 ms response timeout (`:121-132`). Issue #95 shows this write was ACKed, then the vendor RFCOMM channel reset ~1.2 s later; after reconnect the channel went silent while the UI still showed connected.
- Read-only indicators only: Sony shows the active codec (SBC/AAC/LDAC/aptX/aptX HD/LC3) from `GetCodecIndicator` (`src/lib/devices/sony/sonyDevice.js:694-722`; `sonyConfig.js:201-210`); Sennheiser reports a codec label (`src/lib/devices/senhBuds/senhBudsDevice.js:1098-1104`). Edifier defines `LDAC_STATE_QUERY/SET 0x48/0x49` but no code path uses them (`src/lib/devices/edifierBuds/edifierBudsConfig.js:95-96`).
- **No aptX, AAC, SSC, or LE Audio write toggle exists anywhere in BudsLink.** There are no such setting ids or commands; only the read-only indicators above.

**OpenSCL4.** LDAC is a plain bool flag. `FlagStateModifier` sends `0x01 0xFF`, accepts any same-command response, discards the body, and writes the target into its own watch state (`lib/src/devices/soundcore/common/modules/flag/state_modifier.rs:37-62`); `PacketIOController` retries at 500/1000/1500 ms and keys responses only by command (`lib/src/devices/soundcore/common/packet/packet_io_controller.rs:110-138`). There is no state-availability rule, so one-earbud/multipoint do not disable LDAC. For A3947 specifically, LDAC is not even registered: the builder calls in `lib/src/devices/soundcore/a3947.rs:30-56` do not include `builder.ldac()`.

**Does it solve the stale-LDAC bug?** Partially, and not by its confirmation design. BudsLink proves the A3947 command pair and that LDAC is out-of-band from `0x01 0x01`. Its body interpretation is a real improvement, but "write GSettings first, correct only if the device volunteers `0x017F`" can reproduce the same lying UI. The applicable fix is body interpretation plus a targeted `GET_LDAC` after SET with a bounded outcome, not copying the optimistic update.

## Q3. Transport loss, timeouts, retries, reconnect, second connection

**BudsLink.** The vendor transport is a BlueZ `Profile1` RFCOMM profile registered with `AutoConnect: true` (`src/lib/devices/profileManager.js:82-86`). `acquireFd` calls `ConnectProfile` and retries it at 500 ms ticks 1, 2, 4 and 8, then resolves `-1` at about 4.5 s (`:294-342`). If that happens, `startSocket` simply returns; there is no retry (`src/lib/devices/socketByProfile.js:27-36`).

- Read deadline: none. `read_bytes_async(1024)` is bounded only by a cancellable (`socketByProfile.js:66-98`); EOF/zero bytes or an exception destroys the socket. A peer that stops sending without closing is not detected.
- Write deadline: none. `write_all_async` is bounded only by the same cancellable (`:100-131`); an exception destroys the socket.
- Retries: none at the Soundcore protocol level. `_send` only queues and paces packets; there is no response wait (`src/lib/devices/soundcore/soundcoreSocket.js:44-66`). The only soundcore timers are TX pacing (250 ms), dual-list refresh (3 s), a 10 s spinner timeout and a 400 ms EQ debounce (`:16-17,211-221`; `soundcoreDevice.js:365-386,481-491`). None is a transport deadline.
- Recovery: none in the app. `destroy()` closes streams/socket and calls `releaseFd(..., true)`, which calls `DisconnectProfile` (`socketByProfile.js:139-164`; `profileManager.js:346-375`). After a disconnect, a new enhanced device is constructed only when BlueZ reports `Connected` false then true (`enhancedDeviceSupportManager.js:253-332`). If the ACL stays up and only the vendor profile dies, the stale object and row can remain; on A3947 this is an **inference from code**, unconfirmed on hardware. Issue #95 documents the equivalent "connected but dead" state on the OpoBuds driver and notes there is no liveness watchdog in the shared base class.
- Codec change: `SET_LDAC` does not intentionally tear down the session. PR #130's hardware testing says the LDAC toggle "resets the link for a few seconds" (`https://github.com/maniacx/BudsLink/pull/130`, Testing section). If the firmware resets RFCOMM, the shared read loop destroys the socket and disconnects the vendor profile; the ACL/A2DP link may or may not drop.
- Second connection/host: dual commands are sent without waiting for a response, then the device list is refreshed after 3 s (`soundcoreSocket.js:187-222`). The app does not tear down its own session on a dual operation. It cannot prevent the firmware from resetting the link when a second host connects; it only disables the Connection Management UI while LDAC is on.

**OpenSCL4.** A read EOF/`IOException` or write failure marks the session `Disconnected`; `DeviceService` closes the controller and stops, with no `Recovering` state and no automatic reconnect after an RFCOMM-only loss (`.../DeviceService.kt:249-269`; `C:\Users\lunsa\AppData\Local\Temp\opencode\openscl4-arch-notes\settings-and-transport.md:52-57`). ACK retries are 500/1000/1500 ms; `write` and `wait_for_start` are unbounded; Android `socket.connect()` is unbounded (`.../settings-and-transport.md:56`).

**Does it solve the drop bug?** No. For Soundcore, BudsLink has weaker session-level guarantees than OpenSCL4: no ACK wait, no write/read deadline, no watchdog, no app-level reconnect. Its only recovery mechanism is BlueZ's ACL auto-connect plus the earbuds reconnecting. That mechanism does not exist in OpenSCL4's Android RFCOMM model and should not be imported as a substitute for session recovery.

## Comparison table

| Concern | BudsLink | OpenSCL4 | Consequence |
|---|---|---|---|
| Status source | BlueZ `Device1.Connected` (OS link) | App session (`DeviceConnectionController` state) | Opposite semantics; BudsLink hides SPP failure by removing the row |
| Phone-link vs app-control | Implicit: row needs both; no separate state/label | Session state only; no phone-link state | Neither exposes both axes; BudsLink's issue #95 shows the false-positive risk |
| Multipoint visibility | Host list from earbuds (`0x0B 01`), not OS status | `DualConnections` from device, per model | Different data source, same device-owned truth |
| Codec confirmation | Interpret `0x017F` body, no post-write read, no deadline | Same-command ACK, body ignored, local overwrite | Both can show a value the device did not apply |
| Ignored codec write | No detection; persists until `0x017F`/reconnect | No detection; persists until state update/reconnect | Neither is a confirmed write |
| Codec gating | Multipoint switch insensitive under LDAC; no TWS gate | No state availability for LDAC at all | BudsLink is closer, but only one direction |
| Connect deadline | ~4.5 s fd acquisition; `ConnectProfile` retries | Android `socket.connect()` unbounded | BudsLink is better here |
| Read/write deadline | None | Write unbounded; ACK bounded; read unbounded | Both weak |
| Auto reconnect | None in app; BlueZ `AutoConnect` + ACL transitions | None after RFCOMM loss | Neither solves recovery |
| Codec change | Expected link reset; socket destroyed | Session fatal on RFCOMM reset | Both drop the control session |

## Q4. Reusable Soundcore protocol knowledge

BudsLink implements exactly one Soundcore model, **Liberty 4 NC (A3947)**, vendor UUID prefix `0cf12d31-fac3-4553-bd80-d6832e7b` + model code (`src/lib/devices/soundcore/soundcoreConfig.js:9-13`; `.../deviceConfigs/Liberty-4-NC.js:3-6`). PR #130 states the RFCOMM vendor UUID is `...3947` on channel 7 (`https://github.com/maniacx/BudsLink/pull/130`, Protocol notes); the channel number is a PR claim, not visible in code.

OpenSCL4 already supports A3947 (`lib/src/devices/soundcore/a3947.rs:30-56`), and most BudsLink commands already exist there. The genuine gaps are the commands/offsets BudsLink wires for A3947 but OpenSCL4's A3947 module does not:

| Feature | BudsLink command / evidence | OpenSCL4 A3947 status |
|---|---|---|
| LDAC | get `01 7F`, set `01 FF`, one-byte bool (`soundcoreConfig.js:22-23`; `soundcoreSocket.js:151-154,191-193`) | generic flag exists (`common/device.rs:702`), **not registered for A3947** |
| Dual/multipoint enable | `0B 84` + `[0/1]`, max 2 hosts (`soundcoreConfig.js:32-38`; `soundcoreSocket.js:187-189`; `Liberty-4-NC.js:27-28`) | generic module exists (`common/device.rs:684-690`), **not registered for A3947** |
| Dual device list | `0B 01`; multi-packet body: total, index, then length/connected/6-byte MAC/name (`soundcoreConfig.js:113-136`; `soundcoreSocket.js:163-171`) | not parsed for A3947 |
| Dual connect/disconnect/forget | `0B 82` / `0B 81` / `0B 83` + 6 MAC bytes (`soundcoreConfig.js:32-38`; `soundcoreSocket.js:199-222`) | not registered for A3947 |
| Ambient cycle | `06 82`, body = mask repeated in both nibbles, state offset 125 (`soundcoreConfig.js:28`; `soundcoreSocket.js:298-300`; `Liberty-4-NC.js:59`) | parsed in state but **not exposed**: A3947 never calls `builder.ambient_sound_mode_cycle()` (`a3947.rs:30-56`) |
| Dual enabled state | state offset 147, `body[147] === 1` (`Liberty-4-NC.js:27-29`; `soundcoreSocket.js:143-144`) | not parsed for A3947 |
| Sound modes | set `06 81`, 7-byte body (`soundcoreConfig.js:21,86-111`) | present (`a3947/modules/sound_modes.rs`) |
| Equalizer | set `03 87`; 71-byte block at offset 37; DRC ported from OpenSCQ30 (`soundcoreConfig.js:29,231-363`; `Liberty-4-NC.js:61-66`) | present (`a3947/modules/equalizer.rs`) |
| Buttons | set `04 81`; button ids single 2/double 0/triple 5/long 1; sides left 0/right 1; high nibble for in-case, low nibble for out-of-case (`soundcoreConfig.js:27,142-171`; `soundcoreSocket.js:281-296`) | present with `TwsLowBits` (`a3947.rs:66-130`) |
| Flags | sound leak `10 86`, gaming `10 85`, touch tone `01 83`, surround `02 86`, auto play/pause `01 81`, wearing tone `01 8C`, touch lock `04 86`, low-battery `10 82`; offsets 141-157 (`Liberty-4-NC.js:34-43`) | present (`a3947.rs:40-47`) |
| Auto power off | `01 86`, enabled byte + duration index (10/20/30/60) (`soundcoreConfig.js:24`; `Liberty-4-NC.js:44-45`) | present |
| Volume limit | `20 82` enabled+dB, `20 81` refresh rate (`soundcoreConfig.js:25-26`; `soundcoreSocket.js:258-264`) | present |
| Firmware/serial/battery | parsed from state offsets 6/11, 16, 2 and 139 (`Liberty-4-NC.js:16-24`; `soundcoreSocket.js:323-376`) | present |

Protocol facts demonstrated by the source:

- Frame: host→device `08 EE 00 00 00`, device→host `09 FF 00 00 01`, command 2 bytes, length little-endian including header and checksum, checksum = sum of all preceding bytes mod 256 (`soundcoreConfig.js:15-17,71-84`).
- A3947 state body is 159 bytes (`Liberty-4-NC.js:15`); body offsets above are from that packet.
- Commands must be spaced by at least 250 ms; "the earbuds drop commands that arrive in quick succession" (`soundcoreSocket.js:16-17,44-66`). This is also in PR #130.
- The earbuds ignore `GET_LDAC` while the dual-device list is being sent. BudsLink therefore queries LDAC first and delays the list by 1500 ms (or 100 ms after the LDAC reply) (`soundcoreSocket.js:150-157,173-185`). That ordering is documented only in the code comment; the 250 ms pacing rule is also in PR #130.
- LDAC and multiple connections are mutually exclusive on A3947 (`Liberty-4-NC.js:30-31`; `configureWindow.js:246-248`).
- Wearing detection is **not** implemented; only the "wearing tone" flag exists. There is no ear-tip fit test, no find-device command, no firmware-update command, and no in-app diagnostic viewer. Logging is a runtime file with hex TX/RX (`src/lib/devices/logger.js:8-85`), not a protocol feature.
- Other Soundcore models are not implemented, so nothing here proves protocol facts for A3936, A3952, A3954, etc.

## Applicable changes for OpenSCL4, ranked

**Proven by BudsLink source/PR, safe to adopt as protocol facts**

1. **Wire LDAC for A3947 in the Rust core.** Add `Ldac` to A3947 state registration, use `01 7F`/`01 FF`, and fetch once on connect, because BudsLink treats LDAC as out-of-band from `01 01`. This is the largest concrete gap: OpenSCL4 currently has no LDAC setting for this model (`a3947.rs:30-56`). Keep it in `lib/` so both clients share it, per ADR-0001.
2. **Add A3947 multipoint/dual-connection support.** `0B 84` enable, `0B 01` list, `0B 81/82/83` disconnect/connect/forget, state offset 147, multipart list format from `soundcoreConfig.js:113-136`. Represent LDAC × multipoint as state-dependent availability: LDAC on should make multipoint unavailable, because the device treats them as mutually exclusive.
3. **Adopt the firmware pacing/ordering rules.** Enforce ≥250 ms between writes and serialize the LDAC read before the multipoint list (the device drops bursts and ignores queries during the list). These are cheap, model-proven robustness fixes; they belong in the Soundcore packet layer, not in UI code.
4. **Expose ambient cycle for A3947.** `06 82`, body `mask << 4 | mask`, state offset 125. OpenSCL4 already parses `AmbientSoundModeCycle` in A3947's state packet but never registers the setting. This is a `Возможность устройства` already reported by the device, not a new capability invented by BudsLink.
5. **Interpret the LDAC response body.** Treat `0x017F body[0]` as a boolean state, not as an opaque ACK. This is a strict improvement over `FlagStateModifier` ignoring the body, and it is proven by BudsLink's parser (`soundcoreSocket.js:115-120`).

**Inferred, needs a hardware test before relying on it**

6. **After SET_LDAC, issue a targeted `GET_LDAC` and bound the outcome.** BudsLink does not do this; it only waits passively for `0x017F`. The inference is that a post-write read is the direct way to distinguish "applied" from "ACKed/ignored". Give it `Applied/Rejected/TimedOut/Disconnected` semantics rather than a bool. This aligns with `docs/development.md:12` and the existing "confirmed setting-write" proposal in the transport notes.
7. **Add a session watchdog and a `Recovering` state with bounded reconnect.** BudsLink does not solve this, but issue #95 is direct evidence that an open, silent RFCOMM channel is indistinguishable from an idle one and that the UI then lies. Do not copy BudsLink's no-deadline model; add read/write deadlines, a silence watchdog and retry/backoff at the `RfcommConnection`/`ConnectionSession` seam.
8. **Gate codec changes on one-earbud/TWS state.** BudsLink has no TWS parsing, so this is not proven for LDAC. OpenSCL4 already parses `TwsStatus` for A3947 (`a3947/state.rs:19`), so adding an availability rule is feasible, but the exact firmware rule (does LDAC require both buds?) is **unconfirmed** and should be tested.

## What BudsLink does not solve, and what cannot be copied

- **Not an Android client.** GJS + GTK4 + BlueZ `Profile1` + GIO fd passing; no `BluetoothAdapter`, ACL broadcasts, GATT, `CompanionDeviceManager` or Android RFCOMM. The transport code is not portable; only the protocol facts are.
- **No host-side connection lifecycle.** BudsLink does not initiate or retry the ACL link; it waits for BlueZ `Connected` and only calls `ConnectProfile` for its vendor profile (`profileManager.js:294-342`). OpenSCL4's user-selected connect, `Connecting` state and retry have no counterpart, so BudsLink's recovery model cannot be transplanted.
- **OS-link status is not a health signal.** BudsLink shows a device because BlueZ says `Connected`; issue #95 shows the UI can remain "connected" while every command times out. Adopting this would move OpenSCL4 backwards from an app-session indicator to a link indicator, and would not distinguish "phone-linked" from "app-controlled".
- **No confirmed codec write.** There is no post-write read, deadline, retry or NACK handling for Soundcore. Its `0x017F` parser is useful; its optimism is not.
- **No recovery.** No `Recovering` state, no read/write timeout, no silence watchdog, no automatic reconnect. It relies on BlueZ `AutoConnect` and the earbuds reconnecting; Android has no equivalent contract.
- **Only A3947.** No evidence for other Soundcore models, no LE Audio/aptX/AAC/SSC toggles, no one-earbud gating, no wearing detection, no firmware update/fit test/find-device.
- **License is compatible but code reuse is impractical.** Both projects are GPL-3.0-or-later; a port would require attribution and GPL compliance. Copying GJS idioms into Rust would be a rewrite, and the EQ code BudsLink itself credits to OpenSCQ30 (`PR #130`, "Credits / licensing").
- **Contradicts "device is the source of truth" if copied literally.** BudsLink writes GSettings before the device confirms and only corrects on a volunteered `0x017F`. OpenSCL4's stated rule is to read through the device and publish only device-owned state (`docs/development.md:12`); a port must keep that rule, not BudsLink's optimistic GSettings path.

## Sources

BudsLink code (pinned to `f3935512ec459229317e8b875c024af08ca7c2e9`):

- https://github.com/maniacx/BudsLink/blob/f3935512ec459229317e8b875c024af08ca7c2e9/src/lib/bluetoothClient.js
- https://github.com/maniacx/BudsLink/blob/f3935512ec459229317e8b875c024af08ca7c2e9/src/app.js
- https://github.com/maniacx/BudsLink/blob/f3935512ec459229317e8b875c024af08ca7c2e9/src/lib/devices/enhancedDeviceSupportManager.js
- https://github.com/maniacx/BudsLink/blob/f3935512ec459229317e8b875c024af08ca7c2e9/src/lib/devices/profileManager.js
- https://github.com/maniacx/BudsLink/blob/f3935512ec459229317e8b875c024af08ca7c2e9/src/lib/devices/socketByProfile.js
- https://github.com/maniacx/BudsLink/blob/f3935512ec459229317e8b875c024af08ca7c2e9/src/lib/devices/soundcore/soundcoreConfig.js
- https://github.com/maniacx/BudsLink/blob/f3935512ec459229317e8b875c024af08ca7c2e9/src/lib/devices/soundcore/soundcoreSocket.js
- https://github.com/maniacx/BudsLink/blob/f3935512ec459229317e8b875c024af08ca7c2e9/src/lib/devices/soundcore/soundcoreDevice.js
- https://github.com/maniacx/BudsLink/blob/f3935512ec459229317e8b875c024af08ca7c2e9/src/lib/devices/soundcore/deviceConfigs/Liberty-4-NC.js
- https://github.com/maniacx/BudsLink/blob/f3935512ec459229317e8b875c024af08ca7c2e9/src/preferences/devices/soundcore/configureWindow.js
- https://github.com/maniacx/BudsLink/blob/f3935512ec459229317e8b875c024af08ca7c2e9/src/lib/devices/cambridgeBuds/cambridgeBudsConfig.js
- https://github.com/maniacx/BudsLink/blob/f3935512ec459229317e8b875c024af08ca7c2e9/src/lib/devices/opoBuds/opoBudsConfig.js

BudsLink project text:

- README: https://github.com/maniacx/BudsLink/blob/f3935512ec459229317e8b875c024af08ca7c2e9/README.md
- License: https://github.com/maniacx/BudsLink/blob/f3935512ec459229317e8b875c024af08ca7c2e9/LICENSE
- PR #130 (A3947 support, protocol notes, LDAC resets link): https://github.com/maniacx/BudsLink/pull/130
- Issue #95 (codec switch resets vendor channel; connected-but-dead; no watchdog): https://github.com/maniacx/BudsLink/issues/95
- Issue #30 (Soundcore model request; `bluetoothctl` UUID evidence): https://github.com/maniacx/BudsLink/issues/30

OpenSCL4 context:

- `GLOSSARY.md:14-18` (`Возможность устройства`, `Настройка устройства`)
- `docs/development.md:12` (device is the source of truth)
- `docs/adr/0001-compose-slint-shared-behavior.md` (shared behavior in Rust)
- `C:\Users\lunsa\AppData\Local\Temp\opencode\openscl4-arch-notes\device-status.md`
- `C:\Users\lunsa\AppData\Local\Temp\opencode\openscl4-arch-notes\settings-and-transport.md`
