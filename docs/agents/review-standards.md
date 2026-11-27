# Review standards

Sources for the **Standards** axis of `code-review`. Read the cited file before reporting a violation; this document lists what this repository actually holds reviewers to.

## Read before judging

- `AGENTS.md` (imported by `CLAUDE.md`) — current project boundaries and which client is the release target.
- `docs/development.md`, `docs/build-release.md`, `docs/device-development.md`, `docs/android-interface.md`.
- `docs/adr/` — accepted decisions; cite the ADR number when a change contradicts one.
- `GLOSSARY.md` — domain vocabulary (Russian); use its terms in findings.
- `Cargo.toml` `[workspace.lints.clippy]`, `rust-toolchain.toml`, `.editorconfig`, `justfile`.

## Tooling already enforces this — do not report it

- `cargo fmt --check`; ktlint with `.editorconfig` (`android_studio` style, 120 columns, trailing commas); Prettier for Markdown; `shellcheck`.
- `cargo clippy --all-targets -- -D warnings` (tests included) with the workspace lint set (`use_self`, `cloned_instead_of_copied`, `map_unwrap_or`, `try_err`, `get_unwrap`, `assertions_on_result_states`, and the rest of `[workspace.lints.clippy]`).
- `python3 -m unittest discover -s scripts/tests` for README/release-script consistency, for the release workflow's Linux artifacts and their checks (the AppImage and its report in their own artifact, built in a step whose failure leaves the Linux packages and the bundle, started on both Ubuntu releases, its logs kept on failure, no job waiting for it), and for workflow concurrency: push, pull request and manual runs of one branch fall into different concurrency groups.
- `cargo test -p openscq30-desktop`: every `@tr` string is translated with the same placeholders and the same named systems (Windows, Linux), and `ms-settings:` stays out of `desktop/ui`.
- The Rust type checker and the generated UniFFI headers.

A finding must name something the tools cannot decide. Run the gate first; "formatting" or "clippy would complain" is not a finding.

## Project rules (cite the source)

**Shared behavior.** Availability/grouping rules, battery interpretation, sound-mode scales, equalizer validation, quick toggles, preset matching and the mapping from each model to its device art and kind (`lib/src/client/device_art.rs`) live in `lib/src/client/` and are consumed by both clients (`docs/development.md`, ADR-0001). Duplicating them in `android/app/` or `desktop/src/` is a violation; the illustration files, platform transport, permissions and UI lifecycle staying platform-specific is not.

**Device state.** The device is the source of truth. Release clients read through `OpenSCQ30Device`, subscribe with `watch_for_changes`, and write with `set_setting_values` (`docs/development.md`). A cached copy that can drift from the device is a finding. "Preset applied successfully" and "current settings match the preset" are different states. The applied mark clears on disconnect (`GLOSSARY.md`).

**Session ordering.** In the modular core, resolve conflicts between quick presets and pending live EQ by order of acceptance within the session, including calls through cloned handles. A valid preset with EQ supersedes earlier unsent live EQ. A preset without EQ keeps the pending curve. Live EQ accepted after a preset waits until the preset finishes, including confirmation of its fields. [Session contract](https://github.com/SckraT/OpenSCL4/issues/137#issuecomment-7133425212)

**Session confirmation.** Report a write to a readable setting as applied once fresh device state from the current connection matches the request. When a preset stops after confirming a field, return the independently confirmed snapshot and the typed cause. Keep the last confirmed snapshot available after link loss. A new connection accepts writes after fresh synchronization. Retrying an interrupted write takes a new user action. [Session contract](https://github.com/SckraT/OpenSCL4/issues/137#issuecomment-7133425212)

**Session write admission.** Enable writes once the session identifies a driver and receives a fresh response in that driver's measured state layout. Require the measured layout for the read before a write and for read-back. Distinguish an explicit unknown or conflicting identity from an unavailable identity. An unavailable identity permits discovery from measured state for models with evidence for that path. An unknown model or incompatible initial state leaves the session read-only with safe reads, sanitized diagnostics and typed rejection before transmission. Ignore unrelated frames during an active command, preserving the confirmed state and reply deadline until a matching response or timeout. [Parity criteria](https://github.com/SckraT/OpenSCL4/issues/141#issuecomment-7134509776)

**Write validation.** In the modular core, validate model capabilities and the wire range of every requested EQ gain before writing any field of a quick preset. Reject unsupported live EQ and out-of-range gains at submission. Use typed errors and preserve the current settings on these validation failures.

**Model write evidence.** Verify the outbound command and the outcome separately. Confirm the outcome with device read-back or an independent measured response. Record the fixture's source revision or capture reference, known model and firmware, and how device and peer identifiers were masked. In the virtual transport, apply the write's documented effects and preserve unrelated settings. [Parity criteria](https://github.com/SckraT/OpenSCL4/issues/141#issuecomment-7134509776) · [Capture provenance](../../core/data/README.md)

**Native boundary.** Setting values cross UniFFI as JSON; native types and Kotlin wrappers move together, and exported API changes require regenerated bindings (`docs/development.md`). Demo devices replay `OutboundPacket`, so a setting without an outbound implementation is incomplete.

**Data and text.** `lib/src/storage/` owns persisted data; Room changes keep the checked-in `android/app/schemas` JSON and migration tests for old data. User-visible text comes from i18n resources (`lib/i18n/*/openscq30-lib.ftl`, Android resources), not hardcoded English.

**Desktop platforms.** The Slint client ships on Windows and Linux. Text that names the system, its settings or where a device is paired gives each platform its own wording through `AppState.linux`, and system settings pages open through `desktop/src/system_settings.rs` (`docs/development.md`). Windows-only wording or behaviour reachable on Linux, or the reverse, is a violation.

**Identity and release.** The application id is `io.github.sckrat.openscl4`; OpenSCL4 stays a separate installation from OpenSCQ30 (ADR-0003). Do not rename inherited module identities without a stated reason. One product version and one tag per stable release; a platform hotfix bumps the common patch version and rebuilds every platform (ADR-0002). Keys, passwords and signing archives stay out of commits and review output (`docs/build-release.md`).

**Documentation.** `README.md` is generated from `README.md.in`: change the template first (`docs/development.md`). `gui/` is the legacy client, not a release target; work there only when the issue is about it.

**Vendored skills.** On addition or update, verify the applicable upstream license at the copied revision. Record the source and revision, full license text and required copyright notices in the license registry. [Third-party licenses](../../.agents/THIRD_PARTY_LICENSES.txt)

## Classifying findings

- **Violation** — the diff contradicts a rule above or a cited document. Quote the rule and the line.
- **Judgement call** — a design smell the repo has no explicit position on. State it as a possible refactor, never as a breached standard.
- **Out of scope** — taste with no repo rule behind it. Leave it out; do not invent standards.

When a standard changes, update the file that owns it and this list together.
