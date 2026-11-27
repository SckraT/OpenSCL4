---
name: openscl4-checks
description: "Run OpenSCL4 checks for clients, core, transports, protocol fixtures, CLI pilots and vendored skills. Use for changes in these areas, before committing, or when CI fails."
---

# OpenSCL4 checks

Run the applicable checks from the repository root. The development guide describes the client checks and workflow triggers. [Development guide](../../../docs/development.md#checks)

## 1. Narrow loop while editing

### Release clients

```sh
cargo test --locked -p openscq30-lib -p openscq30-desktop
cargo clippy --locked -p openscq30-lib -p openscq30-desktop --all-targets -- -D warnings
cargo fmt --all --check
```

Add `-p openscq30-android` to both cargo commands when `android/src/`, `lib/src/api/` or an exported UniFFI type changed. CI runs that package's host checks on Linux, so a Windows-only pass is not the full gate for it.

Done when every command exits 0.

### Core, transports and CLI pilots

Start with tests for the changed package on its supported host. For model, session or virtual transport changes, test them together.

```sh
cargo test --locked -p openscl4-models -p openscl4-session -p openscl4-transport-mock
```

For framing or conformance fixture changes, run `cargo test --locked -p openscl4-protocol --test conformance`. For pilot changes, use the pilot command in the full core gate below.

For changes to session writes, cover the affected cases at the public `SessionHandle` boundary with virtual time. [Review standards](../../../docs/agents/review-standards.md#project-rules-cite-the-source)

- Submit a preset and a live EQ value back to back in both orders. Assert the final curve, including the case of a preset without EQ.
- Disconnect between two preset fields under both reconnect policies. Assert the typed interruption and confirmed state in the outcome and the session view.
- Check the gain bounds, typed rejections for invalid curves and unsupported live EQ, and unchanged device settings after an invalid mixed preset. Confirm that changing ambient mode on the virtual A3062 preserves its applied EQ curve.
- Verify the outbound bytes and confirm the result from an independent measured reply. The A3062 test replays captured EQ state before the custom write, after it and after restoration. [A3062 capture](../../../core/data/README.md#a3062-equalizer-write-and-read-back)

Done when the relevant package and scenario tests pass. The session suites `live_eq_presets` and `a3062_equalizer_capture` run in the command above.

For changes to model discovery or session write admission, use the same package command and exercise the affected paths.

- Through `SessionHandle` and the virtual transport, assert the typed rejection and record the outbound commands. Each read-only scenario sends only the verified reads used to identify or synchronize the device. Keep the measured known-model path working.
- In `SessionMachine` with virtual time, deliver a state frame received before the request and assert that it cannot complete synchronization. During a write, assert that an unrelated frame preserves confirmed state and the existing deadline. Deliver the matching reply and assert completion for the original command ID. Include events from a prior connection generation when changing generation handling.
- For diagnostic capture changes, check each unrecognized inbound record in `frames.jsonl` for `metadata_only` and the absence of `frame_hex`. Check the archive and the exposed state or events for raw MAC addresses and serial numbers, including their hexadecimal representations.

## 2. Full core and pilot gate

For changes to the core, transports, their fixtures or CLI pilots, run `cargo test --locked` with the `openscl4-*` package flags from each platform's Rust test step. Include `openscl4-transport-windows` on Windows and `openscl4-transport-bluez` on Linux. On Linux also run the core and CLI Clippy commands from the lint workflow with `--all-targets` or the explicit CLI targets shown there and `-D warnings`. [Windows tests](../../../.github/workflows/test-windows.yml) · [Linux tests](../../../.github/workflows/test.yml) · [Lint](../../../.github/workflows/lint.yml)

Run the virtual CLI pilots on both hosts.

```sh
cargo test --locked -p openscq30-cli --test pilot_a3954 --test pilot_a3062 --test pilot_a3949 --test pilot_a3028 --test pilot_a3931 --test pilot_a3933
```

Use CI for any host unavailable locally. Done when the tests on both hosts and the applicable lint checks pass on the submitted commit.

## 3. Release-client gate on Windows

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

## 4. Release-client gate on Linux/CI

```sh
dbus-run-session -- cargo test --locked -p openscq30-lib -p openscq30-desktop -p openscq30-android
cargo clippy --locked -p openscq30-lib -p openscq30-desktop -p openscq30-android --all-targets -- -D warnings
cargo fmt --all --check
python3 -m unittest discover -s scripts/tests -v
just format-check
```

`just format-check` adds ktlint (Android/Kotlin) and Prettier (Markdown) on top of rustfmt; `just shellcheck` checks shell scripts. Both recipes need bash, so on Windows run them from Git Bash/WSL or rely on CI. Use the Prettier version pinned in `.github/workflows/lint.yml`: CI fails without Prettier, while a local run skips Markdown when it is missing. A Windows checkout with `core.autocrlf=true` has CRLF files that Prettier rejects; check them by hand with `--end-of-line auto`.

Done when every command exits 0.

## 5. Docs and generated artifacts

- Changed `README.md.in` or a `DeviceModel` variant: edit the template first, then regenerate with `bash scripts/generate-readme` (needs bash, `jq`, `sd` and Prettier). On Windows update both files and let the Python suite check consistency.
- Changed `scripts/release.py` or its tests: the Python suite is the gate.
- Changed the exported UniFFI API: regenerate Kotlin bindings through the Android Gradle build (`generate-uniffi-bindings-*` tasks) and rebuild the app; native types and Kotlin wrappers must agree.
- For Markdown changes under `docs/agents/` or `.agents/skills/`, pass the changed files directly to the pinned Prettier and verify their relative links.
- For vendored skill additions or updates, use `skills-lock.json` and the copied source revision to verify the applicable license text and notices against the registry. [Third-party licenses](../../THIRD_PARTY_LICENSES.txt)

Done when `python -m unittest discover -s scripts/tests -v` is green and `git status` shows no regenerated-but-uncommitted artifact you did not intend.

## 6. CI dispatch when local checks are not enough

Dispatch only when the push or pull request did not already start the run for that revision:

```sh
gh workflow run lint.yml --repo SckraT/OpenSCL4 --ref <branch>
gh workflow run test.yml --repo SckraT/OpenSCL4 --ref <branch>
gh workflow run test-windows.yml --repo SckraT/OpenSCL4 --ref <branch>
gh workflow run test-android.yml --repo SckraT/OpenSCL4 --ref <branch>
```

Android instrumentation (`connectedX86_64DebugAndroidTest`) is manual; CI does not run it.

Use `gh pr checks <number> --repo SckraT/OpenSCL4 --watch` for a PR or `gh run watch <run-id> --repo SckraT/OpenSCL4 --exit-status` for a manual run. Check the revision against the submitted SHA using `gh pr view <number> --repo SckraT/OpenSCL4 --json headRefOid` or `gh run view <run-id> --repo SckraT/OpenSCL4 --json headSha`. Report queued and running checks as pending. The CI gate passes when the applicable checks succeed for that SHA.

## 7. Interface check on Windows

`scripts/ui.ps1` drives the running desktop window instead of hand-written automation:

```powershell
pwsh scripts/ui.ps1 -Action rect                        # where the window is, in physical pixels
pwsh scripts/ui.ps1 -Action click -X 1430 -Y 140        # coordinates are window-relative
pwsh scripts/ui.ps1 -Action shot -Out .local/page.png -Width 1200 -Height 760
pwsh scripts/ui.ps1 -Action wheel -X 600 -Y 400 -Delta -480
pwsh scripts/ui.ps1 -Action key -Key PAGE_DOWN
```

It makes the process DPI aware before reading any coordinate. Without that a 125% display reports every window rectangle 1.25 times too small, so clicks land beside their target and appear to do nothing at all. It also focuses the window through `AttachThreadInput`, because `SetForegroundWindow` alone is refused while another process owns the foreground.

Capture cropped: the window is 1600×1072 on a typical display, and a full capture costs an agent far more to read than the region it needs.

Done when the capture shows the state you meant to reach, not merely that a command exited 0.

## Traps

- `cargo test --workspace` and `just test` also build the legacy `gui/` and `cli/` targets. Use the explicit release-client packages and test targets for the virtual pilots above.
- A green narrow loop does not cover `android/app/`: Kotlin-only changes need the Gradle unit tests, and instrumentation only covers them on a device or emulator.
- The pinned toolchain (`.build-tool-versions`, `rust-toolchain.toml`) is part of the gate: reproduce CI with it rather than the newest system defaults.
