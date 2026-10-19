# Origin and historical material

OpenSCL4 is an independent fork of [OpenSCQ30](https://github.com/Oppzippy/OpenSCQ30), originally developed by Kyle Scheuing and contributors. SckraT maintains this fork at [SckraT/OpenSCL4](https://github.com/SckraT/OpenSCL4). Existing copyright notices and the [GPL-3.0-or-later license](../../LICENSE.txt) are retained; bundled dependencies and fonts keep their own licenses.

## What remains under upstream names

- Rust crates, the CLI executable and Kotlin source packages retain `openscq30` / `com.oppzippy.openscq30` names. Their names do not determine the installed application's identity.
- `gui/` is the inherited libcosmic GUI. Its `com.oppzippy.OpenSCQ30` AppStream/desktop resources, Windows version resources and install recipes describe that client, not the Slint release. Its AppStream CI check is a legacy metadata check.
- The old release record is preserved in [upstream-changelog.md](upstream-changelog.md). It includes the case-battery entry recovered during branch cleanup. Its version numbers do not prove that this fork published those releases.
- `fastlane/metadata/android/en-US/changelogs/` and its screenshots are inherited store history/assets. The current English title/descriptions identify OpenSCL4, but the directory is not a ready-to-publish store listing; see [its notes](../../fastlane/README.md).
- `screenshots/` contains legacy GUI illustrations. They do not represent the current Slint interface. Historical demonstrations: [upstream desktop video](https://github.com/user-attachments/assets/3df615f5-2e5d-44e8-9604-f5175c11ea5b) and [upstream Android video](https://github.com/user-attachments/assets/bf48a9f3-db73-4f26-b1e7-edac5f3fba32).
- Upstream URLs inside dependency patches and regression-test references identify the actual source of that dependency or evidence. They are not fork support links and should not be replaced mechanically.

## Current identity and support

The current clients install as OpenSCL4. The Android release ID is `io.github.sckrat.openscl4`; Windows installer/app identity and Linux desktop ID are separate from upstream. Desktop data lives under `openscl4/` in the platform configuration directory; Android uses its own application sandbox. There is no automatic data import from OpenSCQ30. See [ADR 0003](../adr/0003-separate-application-identity.md).

Report fork problems to [OpenSCL4 Issues](https://github.com/SckraT/OpenSCL4/issues), and submit translations as pull requests to this repository. Upstream stores, translation hosting, signing keys and the upstream maintainer's email are not this fork's distribution or support services. The inherited GitHub funding configuration has been removed rather than representing the upstream author's sponsorship as funding for OpenSCL4.

## Documentation snapshots

The [accepted plan](../cross-platform-release-plan.md) and [implementation log](../implementation-status-2026-10-19.md) retain earlier observations and measurements as dated history. Read their status notes and latest sections; older paragraphs about source state, signing preparation or unbuilt packages are not claims about the current checkout. The current operational instructions are [the release guide](../build-release.md) and its platform guides.
