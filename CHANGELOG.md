# OpenSCL4 changelog

Changes below describe this fork. This is source history, not proof of publication; published packages belong in [OpenSCL4 Releases](https://github.com/SckraT/OpenSCL4/releases). The inherited OpenSCQ30 release notes are preserved in [upstream history](docs/history/upstream-changelog.md).

## Unreleased

### Shared behavior

- Keep the dual-connections list when a remembered device's name is cut in the middle of a UTF-8 character. The headphones pad names to 32 bytes; the parser now keeps the valid prefix instead of discarding the whole response, so the connected devices stay visible.
- Refresh the device state after 30 s without inbound packets instead of only proving the device is alive. The silence watchdog now lets its `RequestState` response reach the packet handlers, so battery level and charging flags no longer stay at their connect-time values on a long, otherwise idle connection — the Soundcore A3949 (P20i/P25i/R50i) is the reported case, and it never sends these on its own.
- Show the battery percentage Android reports as an experimental setting that is off by default and only replaces what it improves: a single-battery device always gains, a pair of earbuds loses the per-earbud and case levels and needs a second switch, and a pair that reports exact percentages (the A3954) keeps its own values because one number would be coarser. The percentage comes from a native call to the hidden `BluetoothDevice.getBatteryLevel`, since the public SDK exposes no battery reading; when Android has none, the device levels stay. Windows and Linux are unchanged.
- Wait longer for a device to report a toggle before declaring the change unapplied. Some devices reflect a flag only a second or two after the command — the Soundcore A3949 takes about 1 s to enter gaming mode and about 1.5 s to leave it — which did not fit the 2 s window together with the write and read-back pacing, so leaving gaming mode failed with "the device did not apply this setting" in both clients. A confirmed write still returns as soon as the device reports it.

## v2.14.0

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
