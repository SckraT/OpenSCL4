---
name: device-protocol
description: "Add or change Soundcore device support in lib/src/devices: capture and parse state packets, register the DeviceModel, build state and packet structs, keep shared client rules, and verify with tests and the CLI. Use for a new device model, a new setting, packet parsing, or device-specific behavior."
---

# Soundcore device protocol

`docs/device-development.md` is the full guide. This skill is the order and the invariants that are easy to get wrong. Finish with the `openscl4-checks` skill.

## 1. Start from a captured packet

Get a state update packet before writing the model:

- Wireshark with the Soundcore plugin `tools/wireshark/plugins/soundcore.lua` (recommended).
- `tools/soundcore-device-faker/` to act as the device: change packet bytes in `tools/soundcore-device-faker/devices/<model>.toml` and watch the app react (recommended when you have no device).
- The "Soundcore Development Device" model in OpenSCL4 shows the raw packet (fallback).

Change one setting at a time and diff the packets. Done when every field you plan to expose has a byte-level explanation from captured data, not a guess.

## 2. Register the model

- Add the variant to `DeviceModel` in `lib/src/devices/device_model.rs`.
- Add its name to the English catalog `lib/i18n/en/openscq30-lib.ftl`.
- Add match arms in `device_registry` and `demo_device_registry`.

Done when `openscq30 list-models --json` contains the new model.

## 3. Implement state and packets

Create the module under `lib/src/devices/soundcore/<model>/`, following a neighbouring device:

- State struct with `#[derive(Has, Clone)]` (see `lib/src/devices/soundcore/a3035/state.rs`).
- State update packet implementing `Default`, `InboundPacket` and `OutboundPacket`; the `OutboundPacket` implementation is what demo devices replay.
- Conversion from the state update packet into the state struct.
- The `soundcore_device!` macro, with settings added through its builder. Category order in the UI follows the first builder call that adds a category, so order the calls deliberately.

Done when a demo device for the model connects, reads its settings, and writes one setting back.

## 4. Keep shared behavior in lib/src/client

Availability, grouping, battery interpretation, sound-mode scales, equalizer validation, quick toggles and preset matching live in `lib/src/client/` and are consumed by both clients. When a shared rule changes, update `lib/tests/fixtures/client_rules.json` and the contract tests in `lib/src/client/contract_tests.rs`; both sides of the Android boundary are checked against that fixture.

Done when the fixture, the Rust contract tests and the Kotlin side agree.

## 5. Verify

```sh
cargo test --locked -p openscq30-lib
cargo clippy --locked -p openscq30-lib -p openscq30-desktop --all-targets -- -D warnings
```

Then exercise the CLI against the demo device with an isolated database:

```sh
cargo run -p openscq30-cli -- list-models
OPENSCQ30_DATABASE_PATH=<temp-dir>/openscq30.db cargo run -p openscq30-cli -- <subcommand>
```

The CLI's default database is the legacy `openscq30` directory; set `OPENSCQ30_DATABASE_PATH` whenever you test so current desktop data is untouched. Done when the settings round-trip on the demo device and the tests pass.

## Invariants

- The device is the source of truth: read through `OpenSCQ30Device`, subscribe with `watch_for_changes`, write with `set_setting_values`.
- "The preset was applied successfully" and "current settings match the preset" are different states; the applied mark lasts only for the current connection and clears on disconnect.
- Native types and Kotlin wrappers move together; setting values still cross the UniFFI boundary as JSON.
- One-earbud TWS states gate some settings; enforce availability in the shared client rules, not inside a single client.
- Protocol work is test-first: call the `tdd` skill and put the seam at the packet parser or the shared client rule.
