# macOS Bluetooth transport: feasibility and minimal port path

**Status: research only.** This document identifies a feasible transport and a bounded path to
validate it. It is **not** evidence that a macOS client exists, builds, connects to headphones, or is
supported. No macOS host or Apple SDK was available while preparing it, so nothing here was compiled
or run on macOS.

- Research date: 2026-10-20.
- Source revision: `5b6ed28f` (`impl/ticket-15`, based on `spec/remaining-openscl4`).
- Tracking issue: SckraT/OpenSCL4#15. Parent scope: #1.
- Current public position remains "macOS is a later goal" with no validated support matrix [S27].

## 1. The gap in the current architecture

The shared library already abstracts RFCOMM, but it has no macOS implementation.

- `lib/src/connection_backend.rs` selects exactly `linux`, `windows`, or `none`; there is no
  `target_os = "macos"` arm [S1].
- `default_backends()` returns `Some(...)` only for Linux and Windows. Every other OS, including
  macOS, gets `None` [S1].
- `lib/src/api/session.rs` calls `default_backends().expect("no default backends available")` in both
  `list_devices()` and `connect()` [S2]. On macOS this **panics before any Bluetooth call**.
- The `none` fallback is not a usable transport: `rfcomm()` is `unimplemented!()`, as are
  `devices()` and `connect()` [S3].
- The trait surface a macOS backend must satisfy is small [S4]:
    - `RfcommBackend::devices() -> HashSet<ConnectionDescriptor>`
    - `RfcommBackend::connect(mac, RfcommServiceSelectionStrategy) -> Arc<dyn RfcommConnection>`
    - `RfcommConnection::{write, read_channel, connection_status}`

The device protocol requires **classic RFCOMM/SPP**, not BLE GATT:

- `lib/src/devices/soundcore.rs` defines the SPP UUID `00001101-0000-1000-8000-00805f9b34fb` and a
  Soundcore vendor RFCOMM UUID base `0cf12d31-fac3-4553-bd80-d6832e700000` matched by mask [S5].
- The default `SoundcoreDeviceConfig` uses a dynamic strategy: prefer the Soundcore vendor RFCOMM
  UUID, otherwise fall back to SPP [S6].
- The registry opens the connection through `RfcommBackend::connect` and then constructs
  `SoundcoreDeviceBuilder`, whose `new()` immediately performs `RequestState` and parses the state
  update packet [S6]. A successful `connect()` therefore already exercises one settings read.

Android is the existing precedent for supplying a native transport outside `lib`: Kotlin implements
RFCOMM and injects `ManualConnectionBackends` through UniFFI [S7]. For the Slint desktop, a direct
Rust macOS backend is simpler than adding a Swift/Objective-C boundary.

## 2. Transport options

### Option A — IOBluetooth classic RFCOMM (recommended)

Apple's `IOBluetooth` framework provides user-space access to Bluetooth devices, including
Objective-C classes for RFCOMM and SDP [S8]. The relevant calls are documented:

- `IOBluetoothDevice.pairedDevices()` returns paired devices. Apple notes that the list is **not
  stored per user**; it contains devices paired by any user on the Mac [S13].
- `performSDPQuery(_:)` is always asynchronous and returns all L2CAP-derived service records; when a
  target is supplied, `sdpQueryComplete:status:` is called on it [S10].
- `IOBluetoothSDPServiceRecord.getRFCOMMChannelID(_:)` extracts the channel ID from the protocol
  descriptor list [S11].
- `openRFCOMMChannelAsync(_:withChannelID:delegate:)` starts opening a channel, opens the baseband
  connection if needed, and explicitly recommends an SDP query first because channel assignments can
  change [S9].
- `IOBluetoothRFCOMMChannel` provides `writeAsync`/`writeSync`, a delegate, `close()`, `isOpen()`,
  and disconnect/close notification registration [S12].

Rust bindings already exist: `objc2-io-bluetooth` 0.3.2 documents both `aarch64-apple-darwin` and
`x86_64-apple-darwin`, exposes `IOBluetoothDevice` (`pairedDevices`, `performSDPQuery`,
`openRFCOMMChannelAsync_withChannelID_delegate`), `IOBluetoothRFCOMMChannel`, and
`IOBluetoothSDPServiceRecord` [S14]. The relevant feature flags are enabled by default.

Important constraint: `IOBluetoothDevice` is **`!Send` and `!Sync`** [S14]. All Objective-C objects
and delegate callbacks must stay on one dedicated thread, bridged to Tokio through channels.

**Why it fits:** it preserves the existing SPP/vendor-UUID protocol and needs no device-protocol
rewrite. It is the only documented macOS route to SPP/RFCOMM found.

### Option B — CoreBluetooth

CoreBluetooth covers LE and BR/EDR, but its API is GATT-centric: `CBCentralManager`, `CBPeripheral`,
`CBService`, and `CBCharacteristic` [S15]. No RFCOMM/SPP API appears in the framework. Apple's
"Using Core Bluetooth Classic" sample is scoped to iOS 13/Xcode 11 metadata, not a macOS RFCOMM API
[S16]. Using CoreBluetooth would require reverse-engineering a BLE command protocol for Soundcore
devices and validating it per model/firmware. That is a product-level protocol project, not a
transport backend.

### Option C — existing Rust crates

- `bluer` 0.17.4 is the official BlueZ interface and depends on the Linux Bluetooth stack; its
  RFCOMM module is Linux/BlueZ-specific [S18]. Not applicable.
- `btleplug` 0.13.5 is a cross-platform **BLE central** library (macOS via `objc2-core-bluetooth`)
  with GATT services/characteristics, not RFCOMM/SPP [S17]. Not applicable without a BLE protocol.
- `objc2-io-bluetooth` is the credible Rust route to classic RFCOMM [S14].
- A newer crate claiming BLE + classic support was not audited and should not be adopted without a
  dedicated review.

### Option D — Objective-C/Swift transport shim behind UniFFI

Technically viable and follows the Android shape, but it adds a second toolchain, another binding
surface, and a cross-language async bridge. It is a fallback if `objc2::define_class!` delegates prove
unworkable, not the first choice.

### Comparison

| Option                               | SPP/RFCOMM | Reuses protocol | Rust-only | Main risk                                   | Verdict        |
| ------------------------------------ | ---------- | --------------- | --------- | ------------------------------------------- | -------------- |
| IOBluetooth via `objc2-io-bluetooth` | Yes        | Yes             | Yes       | Legacy ObjC API, run-loop/delegate bridging | **Recommend**  |
| CoreBluetooth                        | No (GATT)  | No              | Yes       | New BLE protocol per device                 | Out of scope   |
| `bluer` / `btleplug`                 | No         | No              | Yes       | Wrong OS / wrong transport                  | Not applicable |
| Swift/ObjC shim + UniFFI             | Yes        | Yes             | No        | Extra toolchain and binding layer           | Fallback       |

## 3. Recommended design

Add, entirely behind `#[cfg(target_os = "macos")]`:

1. `lib/src/connection_backend/macos.rs`: `PlatformConnectionBackends`, analogous to the Linux and
   Windows modules.
2. `lib/src/connection_backend/macos/rfcomm.rs`:
    - `devices()`: list `IOBluetoothDevice::pairedDevices()` (possibly also `recentDevices()`), map
      `addressString` to `MacAddr6`, use `nameOrAddress`, and include connected/paired state as the
      trait allows. The trait explicitly permits a superset of connected devices [S4].
    - `connect(mac, strategy)`:
        1. Resolve the device by `deviceWithAddressString`.
        2. Run `performSDPQuery` and await `sdpQueryComplete:status:`.
        3. Collect service-record UUIDs and apply `RfcommServiceSelectionStrategy` exactly as the other
           backends do, including the vendor UUID preference.
        4. Extract the channel ID with `getRFCOMMChannelID`.
        5. Open with `openRFCOMMChannelAsync` and a channel delegate. Apple requires the data listener
           to be registered before opening completes [S9].
    - `RfcommConnection`:
        - `write()` -> `writeAsync`/`writeSync`, respecting the `UInt16` length parameter.
        - `read_channel()` -> `tokio::sync::mpsc` fed by delegate data callbacks.
        - `connection_status()` -> `tokio::sync::watch` fed by channel-close/disconnect notifications,
          with polling as a fallback if notifications are unreliable.
3. Threading: one dedicated thread owning a `CFRunLoop` and all `IOBluetoothDevice` /
   `IOBluetoothRFCOMMChannel` objects. Delegate callbacks push into Tokio channels. Never hold an
   Objective-C object across `.await`.
4. Delegate implementation: `objc2::define_class!` for `IOBluetoothRFCOMMChannelDelegate` and the
   SDP callback target. This is the highest-risk integration point and should be prototyped first.
5. Cargo: add `objc2`, `objc2-foundation`, `objc2-core-foundation`, and `objc2-io-bluetooth` under
   `[target.'cfg(target_os = "macos")'.dependencies]`.
6. `default_backends()`: add a macOS arm returning `Some(macos::PlatformConnectionBackends)`; keep
   the `none` fallback only for other operating systems. The `expect` in `session.rs` then becomes
   reachable only on genuinely unsupported platforms.

Do not change shared protocol code, and do not remove the `expect` as a shortcut: on an unsupported
platform, failing loudly is preferable to silently returning no devices.

## 4. Permissions, TCC, and sandbox

- **Usage description is mandatory.** `NSBluetoothAlwaysUsageDescription` (macOS 11.0+) "is required
  if your app uses the device's Bluetooth interface" [S19]. For CoreBluetooth, Apple states the app
  crashes if required usage keys are missing [S15]. Generate it from `Info.plist` in the app bundle;
  a bare Cargo binary has no `Info.plist`.
- **TCC consent.** macOS records the user's Bluetooth decision per responsible app identity and shows
  the usage string in the prompt. A denied prompt blocks access until changed in System Settings.
  Unsigned/ad-hoc builds can acquire an unstable identity, so a stable bundle identifier and a
  Developer ID signature are strongly advisable even for hardware testing. Verify the exact
  IOBluetooth prompt behavior in the first run; do not assume CoreBluetooth's documented crash
  behavior applies to IOBluetooth.
- **App Sandbox.** If the app is sandboxed (for example, Mac App Store), it needs
  `com.apple.security.device.bluetooth` (macOS 10.7+), enabled through App Sandbox > Hardware >
  Bluetooth [S20]. A direct, non-sandboxed Developer ID build does not use that sandbox entitlement;
  it relies on the usage description and TCC.
- Bundle identity should remain `io.github.sckrat.openscl4` per ADR-0003. Desktop data remains
  `dirs::config_dir()/openscl4` (on macOS this resolves below `~/Library/Application Support`;
  verify at runtime with `OPENSCL4_CONFIG_DIR` isolation).

## 5. Build targets and distribution

- Rust targets: `aarch64-apple-darwin` is Tier 1; `x86_64-apple-darwin` is Tier 2. Rust's minimum is
  macOS 11 on ARM64, raisable with `MACOSX_DEPLOYMENT_TARGET` [S25].
- Slint's tested desktop matrix is **aarch64 only**: macOS 14 Sonoma, 15 Sequoia, and 26 [S23].
  Intel and universal binaries are not in that matrix.
- **Recommendation:** first validate `aarch64-apple-darwin` with a macOS 14+ deployment target.
  Shipping arm64-only is lower risk. Adding `x86_64-apple-darwin` and `lipo` later is a separate
  product decision that requires Intel validation.
- Packaging requires a Mac with Xcode, XcodeGen, the Rust Apple target, and an Apple Developer
  Program membership for signing/notarization/Mac App Store distribution [S24]. Without membership,
  only local self-signing is possible.
- Direct distribution: Developer ID Application certificate, Hardened Runtime, secure timestamp,
  `notarytool` submission, stapled ticket, signed DMG, Gatekeeper check. Apple stopped accepting
  `altool` uploads on 2023-11-01 [S21]; Hardened Runtime is required for notarization [S22].
- The repository's `scripts/release.py` currently has no macOS build path [S26]. A macOS release job
  is net-new work, not a flag flip.
- Version must stay sourced from `[workspace.package].version`; do not hard-code it into the bundle.

## 6. Minimal connect + read-settings path

The smallest meaningful proof is: list a paired Soundcore device, connect over RFCOMM, complete the
existing `RequestState` exchange, and read one setting through the existing API.

1. Build a local `.app` bundle with `NSBluetoothAlwaysUsageDescription`, bundle id
   `io.github.sckrat.openscl4`, and a deployment target matching the test Mac.
2. Pair the headphones in **System Settings > Bluetooth** and confirm they appear as paired.
3. `OpenSCQ30Session::list_devices(model)` returns a `ConnectionDescriptor` whose MAC matches the
   headphones. This proves `default_backends()` and `devices()` work.
4. `OpenSCQ30Session::connect(mac)` performs the SDP query, selects the vendor RFCOMM UUID (or SPP),
   opens the channel, and constructs the device.
5. `SoundcoreDeviceBuilder::new()` sends `RequestState` and parses the state update packet [S6].
   Reaching this point already proves a write plus a read.
6. Read one stable setting (for example battery level or ambient sound mode) from the returned
   device and compare it with the value shown by the Android or Windows client on the same firmware.
7. Disconnect or quit. The channel must close, the process must exit cleanly, and audio playback must
   not break.
8. Repeat connect after disconnect at least once to expose a stale channel or run-loop leak.

### Required test environment

| Item       | Requirement                                                                                               |
| ---------- | --------------------------------------------------------------------------------------------------------- |
| Mac        | Apple Silicon (M1 or later), macOS 14 Sonoma / 15 Sequoia / 26; record exact version and build            |
| Toolchain  | Xcode with a macOS SDK, XcodeGen, Rust with `aarch64-apple-darwin`                                        |
| Signing    | Developer ID Application certificate recommended; stable bundle id; local self-sign only if no membership |
| Headphones | Soundcore Liberty 4 Pro, both earbuds connected, one recorded firmware version                            |
| Pairing    | Paired in System Settings; not connected to a phone or another Mac during the test                        |
| Reference  | Android or Windows client at the same commit/firmware for value comparison                                |
| Evidence   | App commit, macOS build, headphone MAC and firmware, logs, setting value, audio check                     |

### Acceptance evidence for the spike

- The device appears with the correct MAC address.
- SDP exposes the expected RFCOMM service; the selected UUID and channel ID are logged.
- The channel opens and `RequestState` receives a parseable response.
- One setting matches the reference client.
- Disconnect/quit releases the channel; audio still plays; a second connect succeeds.

## 7. Risks and unknowns

1. **No macOS environment was available.** Compile success, runtime behavior, and IOBluetooth
   availability on the user's macOS version are unverified.
2. **IOBluetooth is a legacy Objective-C API.** It remains documented (macOS 10.2+ [S8]) and carries
   no deprecation annotation in the current metadata, but Apple has not positioned it as the modern
   API, and there is no documented modern RFCOMM replacement.
3. **Delegate/run-loop bridging.** `!Send`/`!Sync` objects plus asynchronous callbacks are the main
   implementation risk. A dedicated run-loop thread and strict channel boundaries are required.
4. **SDP and channel selection.** Channel IDs can change; the vendor-UUID preference must be
   replicated or devices exposing only the vendor channel may fail.
5. **TCC identity.** Denied or unstable consent can block the app; unsigned development builds may
   need manual resets in System Settings.
6. **Audio coexistence.** Headphones are normally connected to macOS for audio. Opening SPP
   concurrently may conflict on some firmware, or macOS may own the ACL link. This is unverified and
   must be tested with playback.
7. **Distribution gates.** Notarization requires a paid Apple Developer Program membership, a
   Developer ID certificate, hardened runtime, a secure timestamp, and CI secrets. Gatekeeper blocks
   unnotarized downloads by default.
8. **Architecture.** Slint tests only aarch64 macOS 14+. Intel/universal needs extra targets and
   separate validation.
9. **Parity gaps.** Single-instance enforcement exists for Windows and Linux; macOS has none
   (`single_instance.rs` returns `true` there), and tray/close/launch-at-login behavior is
   Windows/Linux-shaped and unverified on macOS.
10. **Repository claims.** README, `build-release.md`, and the cross-platform plan still describe
    macOS as a later stage [S27][S28]. Do not change that wording until hardware and packaging
    evidence exists.

## 8. Product decisions for the user

These are not decided by this research:

1. **Transport:** commit to IOBluetooth/RFCOMM now, or invest in a BLE GATT protocol for a
   future-proof path?
2. **Distribution:** notarized Developer ID DMG outside the store, or Mac App Store (sandbox,
   Bluetooth entitlement, review)?
3. **Minimum OS and architecture:** arm64-only macOS 14+ (matches Slint's tested matrix), universal
   arm64+x86_64 with Intel validation, or an older deployment target?
4. **Scope:** a read-only one-setting spike first, or full parity (writes, EQ, presets, tray,
   reconnect) in the first macOS release?
5. **Apple account and CI:** who owns the Developer ID certificate, notary credentials, and CI
   secrets? Is the annual Apple Developer Program fee acceptable?
6. **UX:** menu-bar-first app, close-to-tray behavior, single instance, launch at login?
7. **Support label:** keep macOS "experimental / no validated support" until a real-device matrix
   passes?

## 9. Bounded follow-up tasks

These are proposals, not created issues.

1. **macOS compile spike (no behavior change).** On a Mac: install `aarch64-apple-darwin`, run
   `cargo check --locked -p openscq30-desktop --target aarch64-apple-darwin` and the library tests.
   Record every error. Acceptance: results attached; no source changes beyond CI/docs.
2. **Backend skeleton and delegate prototype.** Implement `macos.rs` plus a minimal SDP -> channel
   -> delegate -> channel path. Unit-test the backend trait with a mock; no hardware required.
   Acceptance: `cargo test -p openscq30-lib` and `cargo clippy -D warnings` on macOS; no
   Linux/Windows regressions.
3. **Local `.app` bundle.** Add `Info.plist` with the usage description, bundle id, icon, and minimum
   OS. Acceptance: launches from Finder on a Mac.
4. **Hardware spike.** On Liberty 4 Pro: list -> connect -> read one setting -> disconnect, with
   logs. Acceptance: evidence defined in section 6.
5. **Signing and notarization pipeline.** Developer ID, hardened runtime, `notarytool`, staple, DMG,
   `codesign --verify --deep --strict`, `spctl -a -vv`. Gate on secrets.
6. **macOS CI job.** `macos-14`/`macos-15` Rust tests plus `cargo check`. Do not call macOS supported
   before hardware and packaging evidence exists.
7. **Status documentation.** Only after tasks 4 and 5: add a macOS row with exact tested
   macOS/architecture/firmware and remaining gaps.

## 10. Status against issue #15

| Acceptance criterion                                                            | Status in this document          |
| ------------------------------------------------------------------------------- | -------------------------------- |
| Transport, permissions, build, system integration, and target limits researched | Covered in sections 2-5 and 7    |
| Minimal connect + read path and Mac/headphone requirements defined              | Covered in section 6             |
| Justified choice and risks saved; product decisions surfaced, not invented      | Sections 3, 7, 8                 |
| Bounded follow-up sequence; macOS support not declared implemented              | Sections 9 and the status banner |

The research criteria are addressed. The implementation criteria are **not** met: no macOS backend,
build, package, or device test exists.

## 11. Sources

External sources were accessed on 2026-10-20. Repository references are at `5b6ed28f`.

- [S1] `lib/src/connection_backend.rs` (cfg selection and `default_backends`).
- [S2] `lib/src/api/session.rs` (`expect("no default backends available")`).
- [S3] `lib/src/connection_backend/none.rs`, `lib/src/connection_backend/none/rfcomm.rs`.
- [S4] `lib/src/api/connection.rs` (`RfcommBackend`, `RfcommConnection`,
  `ConnectionDescriptor`).
- [S5] `lib/src/devices/soundcore.rs` (SPP and vendor RFCOMM UUIDs).
- [S6] `lib/src/devices/soundcore/common/device.rs` (dynamic service strategy, `RequestState` fetch
  in `SoundcoreDeviceBuilder::new`).
- [S7] `android/src/connection.rs`, `android/src/api.rs` (Kotlin transport injected via UniFFI).
- [S8] Apple, IOBluetooth framework:
  <https://developer.apple.com/documentation/iobluetooth>
- [S9] Apple, `openRFCOMMChannelAsync(_:withChannelID:delegate:)`:
  <https://developer.apple.com/documentation/iobluetooth/iobluetoothdevice/openrfcommchannelasync(_:withchannelid:delegate:)>
- [S10] Apple, `performSDPQuery(_:)`:
  <https://developer.apple.com/documentation/iobluetooth/iobluetoothdevice/performsdpquery(_:)>
- [S11] Apple, `getRFCOMMChannelID(_:)`:
  <https://developer.apple.com/documentation/iobluetooth/iobluetoothsdpservicerecord/getrfcommchannelid(_:)>
- [S12] Apple, `IOBluetoothRFCOMMChannel`:
  <https://developer.apple.com/documentation/iobluetooth/iobluetoothrfcommchannel>
- [S13] Apple, `pairedDevices()`:
  <https://developer.apple.com/documentation/iobluetooth/iobluetoothdevice/paireddevices()>
- [S14] `objc2-io-bluetooth` 0.3.2:
  <https://docs.rs/objc2-io-bluetooth/latest/objc2_io_bluetooth/>;
  `IOBluetoothDevice`:
  <https://docs.rs/objc2-io-bluetooth/latest/objc2_io_bluetooth/struct.IOBluetoothDevice.html>
- [S15] Apple, Core Bluetooth:
  <https://developer.apple.com/documentation/corebluetooth>
- [S16] Apple, "Using Core Bluetooth Classic":
  <https://developer.apple.com/documentation/corebluetooth/using-core-bluetooth-classic>
- [S17] `btleplug` 0.13.5: <https://docs.rs/btleplug/latest/btleplug/>
- [S18] `bluer` 0.17.4: <https://docs.rs/bluer/latest/bluer/>
- [S19] Apple, `NSBluetoothAlwaysUsageDescription`:
  <https://developer.apple.com/documentation/bundleresources/information-property-list/nsbluetoothalwaysusagedescription>
- [S20] Apple, `com.apple.security.device.bluetooth`:
  <https://developer.apple.com/documentation/bundleresources/entitlements/com.apple.security.device.bluetooth>
- [S21] Apple, "Notarizing macOS software before distribution":
  <https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution>
- [S22] Apple, "Hardened Runtime":
  <https://developer.apple.com/documentation/security/hardened-runtime>
- [S23] Slint, macOS platform and test matrix:
  <https://docs.slint.dev/latest/docs/slint/guide/platforms/desktop/macos/general/>
- [S24] Slint, macOS packaging (Rust):
  <https://docs.slint.dev/latest/docs/slint/guide/platforms/desktop/macos/packaging/>
- [S25] Rust, `*-apple-darwin` platform support:
  <https://doc.rust-lang.org/nightly/rustc/platform-support/apple-darwin.html>
- [S26] `scripts/release.py` (Windows/Linux/Android build functions only).
- [S27] `README.md` platform status table (macOS is a later goal, no validated matrix).
- [S28] `docs/cross-platform-release-plan.md` (macOS has no standard Bluetooth backend yet).
