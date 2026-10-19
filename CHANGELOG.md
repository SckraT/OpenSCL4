# OpenSCL4 changelog

Changes below describe this fork. This is source history, not proof of publication; published packages belong in [OpenSCL4 Releases](https://github.com/SckraT/OpenSCL4/releases). The inherited OpenSCQ30 release notes are preserved in [upstream history](docs/history/upstream-changelog.md).

## Unreleased

### Clients and identity

- Use a Slint desktop client alongside the Compose Android app, with shared Rust device protocols and client rules.
- Install as OpenSCL4 with separate application identity, desktop data and Android signing key; no automatic import from OpenSCQ30.
- Add desktop tray controls and refine tooltips, controls and the Windows application icon.

### Shared behavior

- Share setting availability, battery interpretation, sound mode scales, equalizer validation and preset matching across clients.
- Share equalizer profile transfer and quick preset activation. Remember the last successful activation only for the current connection.
- Read grouped settings from one captured device-state snapshot and share Android UI reads.
- Retain A3954 sound-leak command, spatial-audio/equalizer and sound-mode fixes.
- Omit unavailable case battery level `0xFF` instead of overflowing or displaying a nonsensical value.

### Builds and documentation

- Prepare one version/commit-based release script for Android, Windows and experimental Linux `.deb`/`.tar.gz` packages.
- Build signed Android ARM64 and Windows x64 preview packages locally; Linux session and hardware validation remain open.
- Document current architecture, platform setup, support links and the boundary between OpenSCL4 and inherited upstream history.
