# Developing OpenSCL4

Start with the [README](../README.md) for product scope and platform status. Build setup is in [Windows](build-windows.md), [Android](build-android.md), [Linux](build-linux.md) and [the common release guide](build-release.md). Current development uses Android Compose and `desktop/` Slint; `gui/` is the [legacy libcosmic client](../gui/README.md). The [macOS Bluetooth transport research](macos-bluetooth-transport.md) records a proposed path and risks; it is not an implemented backend.

## Boundaries and device state

- `lib/src/devices/` implements device protocols, packet parsing and per-model settings. `lib/src/api/` exposes device operations, equalizer transfer and quick preset activation; `lib/src/storage/` owns persisted data.
- `lib/src/client/` contains shared availability/grouping rules, battery interpretation, sound-mode scales, equalizer input validation, quick toggles and preset matching. Both clients consume these rules; Bluetooth transports, permissions and UI lifecycle remain platform-specific.
- `android/src/` is the UniFFI adapter; `android/app/` contains Kotlin wrappers, Compose UI, services, notifications and widgets. Setting values still cross the binding as JSON inside the native API records.
- `desktop/src/controller.rs` reads device snapshots and writes settings; `desktop/src/app/` applies them to Slint models, and `desktop/ui/` defines the interface.

The device is the source of truth. Read settings through `OpenSCQ30Device`, subscribe with `watch_for_changes`, and write through `set_setting_values`. A successful preset activation and a preset matching current settings are different concepts. The successful-activation mark lasts only for the current connection; saved presets persist. See the [glossary](../GLOSSARY.md) and [accepted architecture decisions](adr/0001-compose-slint-shared-behavior.md).

Keep native types and Kotlin wrappers synchronized. Regenerate bindings through Gradle when the exported API changes. Shared examples in `lib/tests/fixtures/client_rules.json` check behavior on both sides of the Android boundary.

The `openscq30` CLI remains useful for protocol work. Its default database is still in the legacy `openscq30` configuration directory; set `OPENSCQ30_DATABASE_PATH` to an isolated database when testing. It does not manage the current desktop client's data directory by default.

## Checks

With the relevant host prerequisites installed, the current Rust-client suite is:

```sh
cargo test --locked -p openscq30-lib -p openscq30-desktop -p openscq30-android
cargo clippy --locked -p openscq30-lib -p openscq30-desktop -p openscq30-android -- -D warnings
cargo fmt --all --check
python3 -m unittest discover -s scripts/tests -v
```

On Windows use `python`; the release workflow runs the Rust library/desktop tests there and the library/desktop/Android host checks on Linux. Android JVM tests use `./gradlew testX86_64DebugUnitTest` from `android/` (or `gradlew.bat`). CI does not currently run Android instrumentation; those checks are manual. See [Android validation](android-interface.md#validation).

CI runs for pull requests and for pushes to `master`; a push to any other branch does not start it on its own. A push and a pull request for the same branch would share one concurrency group, and the later run would cancel the other, so branches are checked through their pull request only. The Rust, Windows and Android jobs are path-filtered, so a documentation-only change does not start them. For a branch without a pull request, or when automatic triggering is unavailable, run a check for an existing revision directly from the repository root: `gh workflow run lint.yml --repo SckraT/OpenSCL4 --ref <branch>`, `gh workflow run test.yml --repo SckraT/OpenSCL4 --ref <branch>` for the Rust job, or `gh workflow run test-android.yml --repo SckraT/OpenSCL4 --ref <branch>` for the Android JVM job. Do not dispatch a workflow again for a revision whose push or pull request already started it.

`cargo test --workspace` and `just test` also include legacy CLI/GUI targets. They are broader than the current release-client checks and may need legacy GUI dependencies. Most `just` recipes require bash; the current release entry point is Python and does not require `just`.

## Documentation and model catalog

`README.md.in` is the source of the prose in `README.md`. Change the template first. From the repository root, regenerate the catalog and README with:

```sh
bash scripts/generate-readme
```

This existing generator requires bash, Rust/Cargo, `jq`, `sd` and Prettier. It runs the CLI's `list-models --json`, removes the development-only model, substitutes `@SUPPORTED_DEVICES_TABLE@` and formats Markdown. Its input is this checkout's template, not an upstream README download. On Windows use an environment with those tools, or update both files and run the documentation consistency checks; the device table must still match the current enum and English model names.

The lightweight Python suite checks that the checked-in README matches its template and that its catalog matches the core model declarations. It also checks release scripts and workflow triggers, so a push to a pull request branch cannot cancel the pull request's run (see the CI rules above). Documentation CI runs it without rebuilding the apps.

Device implementation work is described in [the protocol development guide](device-development.md). Feature/bug reports belong in [OpenSCL4 Issues](https://github.com/SckraT/OpenSCL4/issues); translation updates arrive as pull requests here. Upstream's translation host is not automatically connected to this fork.

## Historical material

[Origin and historical assets](history/upstream.md) distinguish inherited screenshots, store metadata, dependencies and release records from current OpenSCL4 deliverables. Preserve copyright and license notices when editing inherited code. Keep release keys and local data out of Git.
