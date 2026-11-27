# Developing OpenSCL4

Start with the [README](../README.md) for product scope and platform status. Build setup is in [Windows](build-windows.md), [Android](build-android.md), [Linux](build-linux.md) and [the common release guide](build-release.md). Current development uses Android Compose and `desktop/` Slint; `gui/` is the [legacy libcosmic client](../gui/README.md).

## Boundaries and device state

- `lib/src/devices/` implements device protocols, packet parsing and per-model settings. `lib/src/api/` exposes device operations, equalizer transfer and quick preset activation; `lib/src/storage/` owns persisted data.
- `lib/src/client/` contains shared availability/grouping rules, battery interpretation, sound-mode scales, equalizer input validation, quick toggles, preset matching, and the mapping from each model to its device art and kind (`lib/src/client/device_art.rs`). Both clients consume it; the illustration files, Bluetooth transports, permissions and UI lifecycle remain platform-specific.
- `android/src/` is the UniFFI adapter; `android/app/` contains Kotlin wrappers, Compose UI, services, notifications and widgets. Setting values still cross the binding as JSON inside the native API records.
- `desktop/src/controller.rs` reads device snapshots and writes settings; `desktop/src/app/` applies them to Slint models, and `desktop/ui/` defines the interface. The desktop client ships on Windows and Linux: text that names the system, its settings or where a device is paired has each platform's wording through `AppState.linux`, and the system's own settings pages open through `desktop/src/system_settings.rs`.

The device is the source of truth. In the release clients, read settings through `OpenSCQ30Device`, subscribe with `watch_for_changes`, and write through `set_setting_values`. A successful preset activation and a preset matching current settings are different concepts. The successful-activation mark lasts for the current connection. Saved presets persist. [Glossary](../GLOSSARY.md) · [Shared behavior decision](adr/0001-compose-slint-shared-behavior.md)

Keep native types and Kotlin wrappers synchronized. Regenerate bindings through Gradle when the exported API changes. Shared examples in `lib/tests/fixtures/client_rules.json` check behavior on both sides of the Android boundary.

## Device control during the migration

Trace the issue to its client or pilot entry point before editing, and verify the result through that path.

- **Release clients.** Desktop and Android still control devices through `OpenSCQ30Device` in `lib`. Follow the release path for changes intended to reach those clients.
- **Modular pilot.** `openscq30 pilot` starts a `SessionHandle` through `SessionRunner` from the modular core. Follow that path for pilot and session changes.
- **Profile migration.** `lib` exposes `migrate_legacy_profiles` in `lib/src/storage/profile_migration.rs` to import legacy SQLite data into `openscl4-profiles`. Verify compatibility through the migration API and its integration tests.

Use the epic decisions to identify the intended migration stage and required evidence. [Migration roadmap](https://github.com/SckraT/OpenSCL4/issues/134) · [Session contract](https://github.com/SckraT/OpenSCL4/issues/137#issuecomment-7133425212) · [Model parity criteria](https://github.com/SckraT/OpenSCL4/issues/141#issuecomment-7134509776)

The `openscq30` CLI remains useful for protocol work. Its default database is still in the legacy `openscq30` configuration directory; set `OPENSCQ30_DATABASE_PATH` to an isolated database when testing. It does not manage the current desktop client's data directory by default.

Run the read-only CLI pilot to start a virtual Liberty 4 Pro through the new core.

```sh
cargo run --locked -p openscq30-cli -- pilot
```

The command prints the confirmed model, firmware, battery levels and sound mode as JSON after closing the link.
Pass `--cycle-mode` after `pilot` to switch between normal and noise canceling modes, then restore the original mode and position.
The JSON includes the separately confirmed `changed` and `restored` results.

To read the virtual Space One Pro, run:

```sh
cargo run --locked -p openscq30-cli -- pilot --mock-model A3062
```

The A3062 pilot reports one device battery and firmware. The session can identify
it from the state response if the separate identity probe is unavailable. Add
`--cycle-mode` to change the mode and restore the original. Each result comes
from a fresh state response. The JSON reports the mode without a scale position.
A state that reports multipoint enabled is followed by the captured `0B 01` peer
read before the session becomes writable; the snapshot exposes the masked peers
that read reports, and diagnostics never export their addresses.

The A3028 and A3029 pilots use the same driver and are discovered from their
state response because no identity response is recorded for the family. Both
report firmware 02.30, one device battery and normal mode from the reference
state in [`core/data`](../core/data/README.md#a3028-and-a3029-state-reference).
Add `--cycle-mode` to write the reference Normal → ANC Transport → Normal
sequence, each confirmed by a new state response.

```sh
cargo run --locked -p openscq30-cli -- pilot --mock-model A3028
cargo run --locked -p openscq30-cli -- pilot --mock-model A3029
```

The P20i pilot reports both earbud batteries, firmware versions and gaming mode
from a captured A3949 state response. Add `--cycle-mode` to switch gaming mode
and restore its original value. Each result requires a fresh state response.

```sh
cargo run --locked -p openscq30-cli -- pilot --mock-model A3949
```

An unrecognized identity or a registered model with an incompatible state
switch the pilot to a read-only result. An identity naming no registered model
sends only the verified identity read; a registered model with an incompatible
state uses the verified identity and state reads. Settings writes and
`--cycle-mode` are rejected before reaching the transport, and the JSON reports
safe metadata without raw identifiers.

```sh
cargo run --locked -p openscq30-cli -- pilot --mock-scenario unknown-identity
cargo run --locked -p openscq30-cli -- pilot --mock-scenario incompatible-state
```

On Windows and Linux, select a connected Liberty 4 Pro by its name in Bluetooth settings to run the same pilot over RFCOMM. Linux uses BlueZ on the system D-Bus.

```sh
cargo run --locked -p openscq30-cli -- pilot --bluetooth "soundcore Liberty 4 Pro"
```

Start with the read-only run. Add `--cycle-mode` to test the writes once the read-only run succeeds.
A hardware run uses a separate connection to the device. Close any other app that holds a Soundcore control connection before starting it.

The Linux CI job also uploads `openscl4-a3954-pilot-linux-x86_64`, built on Ubuntu 24.04. Extract its archive and run `./openscq30 pilot --bluetooth "soundcore Liberty 4 Pro"`. Add `--cycle-mode` for the write check.

## Checks

With the relevant host prerequisites installed, the current Rust-client suite is:

```sh
cargo test --locked -p openscq30-lib -p openscq30-desktop -p openscq30-android
cargo clippy --locked -p openscq30-lib -p openscq30-desktop -p openscq30-android --all-targets -- -D warnings
cargo fmt --all --check
python3 -m unittest discover -s scripts/tests -v
```

For modular core, transport, protocol fixture and CLI pilot changes, follow the core checks on Windows and Linux. For device writes and vendored skills, use the repository review rules. [Core and pilot checks](../.agents/skills/openscl4-checks/SKILL.md#2-full-core-and-pilot-gate) · [Review standards](agents/review-standards.md)

Install the pre-commit hook once per clone with `bash scripts/git-hooks/install.sh` (or `just install-hooks`); it serves every worktree of the clone and leaves other hooks in place. Before each commit it checks only the staged files: rustfmt for Rust, ktlint for Kotlin, Prettier for the Markdown that `just format-check` covers, shellcheck for shell scripts, and the Python suite when its inputs change. A tool that is not installed is reported and skipped, and CI still runs the full gate. Skip the hook once with `git commit --no-verify`.

On Windows use `python`; the release workflow runs the Rust library/desktop tests there and the library/desktop/Android host checks on Linux. The desktop single-instance test needs a D-Bus session bus on Linux: CI runs the tests under `dbus-run-session` and fails the test without one, while a local run without a bus skips it. Android JVM tests use `./gradlew testX86_64DebugUnitTest` from `android/` (or `gradlew.bat`); tests for the Gradle helpers in `android/buildSrc` use `./gradlew :buildSrc:test`. CI does not currently run Android instrumentation; those checks are manual. See [Android validation](android-interface.md#validation).

To check Linux-only desktop code from Windows before CI, `just desktop::check-linux` (or `bash scripts/check-desktop-linux.sh`) runs the desktop clippy and tests on Ubuntu 24.04 in Docker, the tests under `dbus-run-session` as in CI. The toolchain, crates and build stay in the `openscl4-desktop-linux` Docker volume, so only the first run builds everything; `docker volume rm openscl4-desktop-linux` clears it.

CI runs for pull requests and pushes to `master`. A concurrency group covers one workflow, event and branch. A newer run cancels older runs in the same group. Push, pull request and manual runs use separate groups. Rust, Windows and Android workflows use path filters. If a required workflow has no run for your commit, use the manual check steps. [CI dispatch](../.agents/skills/openscl4-checks/SKILL.md#6-ci-dispatch-when-local-checks-are-not-enough)

`cargo test --workspace` and `just test` also include legacy CLI/GUI targets. They are broader than the current release-client checks and may need legacy GUI dependencies. Most `just` recipes require bash; the current release entry point is Python and does not require `just`.

## Documentation and model catalog

`README.md.in` and `README.ru.md.in` are the sources of the prose in `README.md` and `README.ru.md`. Change the templates first. From the repository root, regenerate both catalogs and READMEs with:

```sh
bash scripts/generate-readme
```

This existing generator requires bash, Rust/Cargo, `jq`, `sd` and Prettier. It runs the CLI's `list-models --json`, removes the development-only model, substitutes `@SUPPORTED_DEVICES_TABLE@` in each template (with English or Russian column headers) and formats Markdown. Its input is this checkout's templates, not an upstream README download. On Windows use an environment with those tools, or update the four files and run the documentation consistency checks; the device table must still match the current enum and English model names.

The lightweight Python suite checks that each checked-in README matches its template and that both catalogs match the core model declarations. It also checks release scripts, how the release workflow hands the Linux packages to their checks, and that push, pull request and manual runs of one branch fall into different concurrency groups (see the CI rules above). Documentation CI runs it without rebuilding the apps.

Screenshots in `docs/images/` are captures from the release packages, not generated assets; replace them when the interface or the shown version changes, and keep the version in the README caption. Device implementation work is described in [the protocol development guide](device-development.md). Feature/bug reports belong in [OpenSCL4 Issues](https://github.com/SckraT/OpenSCL4/issues); translation updates arrive as pull requests here. Upstream's translation host is not automatically connected to this fork.

## Historical material

[Origin and historical assets](history/upstream.md) distinguish inherited screenshots, store metadata, dependencies and release records from current OpenSCL4 deliverables. Preserve copyright and license notices when editing inherited code. Keep release keys and local data out of Git.
