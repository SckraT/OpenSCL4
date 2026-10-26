---
name: openscl4-checks
description: "Run the checks that gate OpenSCL4 changes: Rust tests, clippy, formatting, docs-consistency scripts and Android JVM tests, per host. Use after editing Rust in lib/, desktop/ or android/, before committing, or when CI fails."
---

# OpenSCL4 checks

`docs/development.md` defines the gates. This skill is the order to run them in, the host split, and the traps that make a green run meaningless. Run everything from the repository root.

## 1. Narrow loop while editing

```sh
cargo test --locked -p openscq30-lib -p openscq30-desktop
cargo clippy --locked -p openscq30-lib -p openscq30-desktop --all-targets -- -D warnings
cargo fmt --all --check
```

Add `-p openscq30-android` to both cargo commands when `android/src/`, `lib/src/api/` or an exported UniFFI type changed. CI runs that package's host checks on Linux, so a Windows-only pass is not the full gate for it.

Done when every command exits 0.

## 2. Full gate on Windows

```powershell
cargo test --locked -p openscq30-lib -p openscq30-desktop
cargo clippy --locked -p openscq30-lib -p openscq30-desktop -p openscq30-android --all-targets -- -D warnings
cargo fmt --all --check
python -m unittest discover -s scripts/tests -v
```

```powershell
cd android; .\gradlew.bat testX86_64DebugUnitTest
```

Use `python`, not `python3`, on Windows. The Gradle task needs JDK 21 and the Android SDK; pinned versions are in `.build-tool-versions`, and `rust-toolchain.toml` is selected by rustup automatically. `cargo test` respects `--locked`, so commit `Cargo.lock` changes before running it.

Done when all commands exit 0. If only `openscq30-android` fails on Windows, run that check on Linux or dispatch CI — keep the package in the gate.

## 3. Full gate on Linux/CI

```sh
cargo test --locked -p openscq30-lib -p openscq30-desktop -p openscq30-android
cargo clippy --locked -p openscq30-lib -p openscq30-desktop -p openscq30-android --all-targets -- -D warnings
cargo fmt --all --check
python3 -m unittest discover -s scripts/tests -v
just format-check
```

`just format-check` adds ktlint (Android/Kotlin) and Prettier (Markdown) on top of rustfmt; `just shellcheck` checks shell scripts. Both recipes need bash, so on Windows run them from Git Bash/WSL or rely on CI. Use the Prettier version pinned in `.github/workflows/lint.yml`: CI fails without Prettier, while a local run skips Markdown when it is missing. A Windows checkout with `core.autocrlf=true` has CRLF files that Prettier rejects; check them by hand with `--end-of-line auto`.

Done when every command exits 0.

## 4. Docs and generated artifacts

- Changed `README.md.in` or a `DeviceModel` variant: edit the template first, then regenerate with `bash scripts/generate-readme` (needs bash, `jq`, `sd` and Prettier). On Windows update both files and let the Python suite check consistency.
- Changed `scripts/release.py` or its tests: the Python suite is the gate.
- Changed the exported UniFFI API: regenerate Kotlin bindings through the Android Gradle build (`generate-uniffi-bindings-*` tasks) and rebuild the app; native types and Kotlin wrappers must agree.

Done when `python -m unittest discover -s scripts/tests -v` is green and `git status` shows no regenerated-but-uncommitted artifact you did not intend.

## 5. CI dispatch when local checks are not enough

Dispatch only when the push or pull request did not already start the run for that revision:

```sh
gh workflow run lint.yml --repo SckraT/OpenSCL4 --ref <branch>
gh workflow run test.yml --repo SckraT/OpenSCL4 --ref <branch>
gh workflow run test-android.yml --repo SckraT/OpenSCL4 --ref <branch>
```

Android instrumentation (`connectedX86_64DebugAndroidTest`) is manual; CI does not run it.

Done when the run for the exact revision is queued or green.

## Traps

- `cargo test --workspace` and `just test` also build the legacy `gui/` and `cli/` targets. The release-client suite is the explicit three-package list above.
- A green narrow loop does not cover `android/app/`: Kotlin-only changes need the Gradle unit tests, and instrumentation only covers them on a device or emulator.
- The pinned toolchain (`.build-tool-versions`, `rust-toolchain.toml`) is part of the gate: reproduce CI with it rather than the newest system defaults.
