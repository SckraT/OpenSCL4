# Review standards

Sources for the **Standards** axis of `code-review`. Read the cited file before reporting a violation; this document lists what this repository actually holds reviewers to.

## Read before judging

- `CLAUDE.md` — current project boundaries and which client is the release target.
- `docs/development.md`, `docs/build-release.md`, `docs/device-development.md`, `docs/android-interface.md`.
- `docs/adr/` — accepted decisions; cite the ADR number when a change contradicts one.
- `GLOSSARY.md` — domain vocabulary (Russian); use its terms in findings.
- `Cargo.toml` `[workspace.lints.clippy]`, `rust-toolchain.toml`, `.editorconfig`, `justfile`.

## Tooling already enforces this — do not report it

- `cargo fmt --check`; ktlint with `.editorconfig` (`android_studio` style, 120 columns, trailing commas); Prettier for Markdown; `shellcheck`.
- `cargo clippy -- -D warnings` with the workspace lint set (`use_self`, `cloned_instead_of_copied`, `map_unwrap_or`, `try_err`, `get_unwrap`, `assertions_on_result_states`, and the rest of `[workspace.lints.clippy]`).
- `python3 -m unittest discover -s scripts/tests` for README/release-script consistency and for workflow concurrency: push, pull request and manual runs of one branch fall into different concurrency groups.
- `cargo test -p openscq30-desktop`: every `@tr` string is translated with the same placeholders and the same named systems (Windows, Linux), and `ms-settings:` stays out of `desktop/ui`.
- The Rust type checker and the generated UniFFI headers.

A finding must name something the tools cannot decide. Run the gate first; "formatting" or "clippy would complain" is not a finding.

## Project rules (cite the source)

**Shared behavior.** Availability/grouping rules, battery interpretation, sound-mode scales, equalizer validation, quick toggles and preset matching live in `lib/src/client/` and are consumed by both clients (`docs/development.md`, ADR-0001). Duplicating them in `android/app/` or `desktop/src/` is a violation; platform transport, permissions and UI lifecycle staying platform-specific is not.

**Device state.** The device is the source of truth: read through `OpenSCQ30Device`, subscribe with `watch_for_changes`, write with `set_setting_values` (`docs/development.md`). A cached copy that can drift from the device is a finding. "Preset applied successfully" and "current settings match the preset" are different states; the applied mark clears on disconnect (`GLOSSARY.md`).

**Native boundary.** Setting values cross UniFFI as JSON; native types and Kotlin wrappers move together, and exported API changes require regenerated bindings (`docs/development.md`). Demo devices replay `OutboundPacket`, so a setting without an outbound implementation is incomplete.

**Data and text.** `lib/src/storage/` owns persisted data; Room changes keep the checked-in `android/app/schemas` JSON and migration tests for old data. User-visible text comes from i18n resources (`lib/i18n/*/openscq30-lib.ftl`, Android resources), not hardcoded English.

**Desktop platforms.** The Slint client ships on Windows and Linux. Text that names the system, its settings or where a device is paired gives each platform its own wording through `AppState.linux`, and system settings pages open through `desktop/src/system_settings.rs`. Windows-only wording or behaviour reachable on Linux, or the reverse, is a violation.

**Identity and release.** The application id is `io.github.sckrat.openscl4`; OpenSCL4 stays a separate installation from OpenSCQ30 (ADR-0003). Do not rename inherited module identities without a stated reason. One product version and one tag per stable release; a platform hotfix bumps the common patch version and rebuilds every platform (ADR-0002). Keys, passwords and signing archives stay out of commits and review output (`docs/build-release.md`).

**Documentation.** `README.md` is generated from `README.md.in`: change the template first (`docs/development.md`). `gui/` is the legacy client, not a release target; work there only when the issue is about it.

## Classifying findings

- **Violation** — the diff contradicts a rule above or a cited document. Quote the rule and the line.
- **Judgement call** — a design smell the repo has no explicit position on. State it as a possible refactor, never as a breached standard.
- **Out of scope** — taste with no repo rule behind it. Leave it out; do not invent standards.

When a standard changes, update the file that owns it and this list together.
