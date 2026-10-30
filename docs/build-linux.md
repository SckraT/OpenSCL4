# OpenSCL4 on Linux (experimental)

The current desktop client is `desktop/` (Slint). Linux packaging and CI checks exist, but the release session matrix and real-headphone QA have not been established as complete. The older libcosmic GUI and its `cosmic-icons` instructions are [legacy](../gui/README.md), not OpenSCL4 runtime requirements.

## Target systems

The first target is x86_64 Ubuntu/Kubuntu 24.04 and 26.04. Before stable publication, check Ubuntu 24.04 GNOME on Wayland and X11; Kubuntu 24.04 on X11; Ubuntu 26.04 GNOME and Kubuntu 26.04 on Wayland. Other architectures and distributions are later work, not a tested support promise.

## Build prerequisites

Install Python 3.11+, Rust via rustup, a C/C++ build toolchain, and the development packages used by [release CI](../.github/workflows/build.yml). For Ubuntu:

```sh
sudo apt-get update
sudo apt-get install build-essential pkg-config libdbus-1-dev libxkbcommon-dev libfontconfig1-dev libsqlite3-dev desktop-file-utils file dbus
```

The Rust version is selected by `rust-toolchain.toml`. For packaging, add `x86_64-unknown-linux-gnu` with rustup if needed. Release packages must be built on **Ubuntu 24.04 x86_64**, including packages intended for 26.04, to preserve the glibc baseline; the release script rejects a different build host.

The AppImage is made with `appimagetool` and the AppImage runtime at the versions in [`.build-tool-versions`](../.build-tool-versions). The release script downloads both from their GitHub releases into `.local/appimage/` on the first build and deletes and refuses a file whose SHA-256 differs from the one pinned there, so that build needs access to GitHub. `appimagetool` runs extracted, so the build host needs no FUSE; it does need `file` and `desktop-file-validate` from the packages above.

## Development and packaging

```sh
cargo build --locked -p openscq30-desktop
./target/debug/openscl4
python3 scripts/release.py build linux
```

The last command produces `openscl4_VERSION_amd64.deb`, `openscl4-VERSION-linux-x86_64.tar.gz`, `openscl4-VERSION-linux-x86_64.AppImage` and `build-linux.json` in `build-output/`. The tarball and the AppImage contain the same staged application files as the Debian package. Neither bundles the system libraries, so neither is a distribution-independent runtime. `build-linux.json` lists all three packages with their SHA-256 and records the hashes of the two AppImage tools. Use `--allow-dirty` only for a source preview.

Without an Ubuntu 24.04 host, build the same files in CI from a pushed branch. The `linux` platforms choice skips Windows, Android and the preview bundle, and still runs the package checks below. The AppImage is a separate artifact, `release-linux-appimage`, next to `release-linux` with the other packages and the build report:

```sh
gh workflow run build.yml --repo SckraT/OpenSCL4 --ref BRANCH -f platforms=linux
gh run download RUN_ID --repo SckraT/OpenSCL4 --name release-linux
gh run download RUN_ID --repo SckraT/OpenSCL4 --name release-linux-appimage
```

Artifact downloads do not keep the executable bit, so run `chmod +x` on the downloaded AppImage.

## Install and run

Once a matching package is available:

```sh
sudo apt install ./openscl4_VERSION_amd64.deb
```

APT installs the runtime dependencies declared in [`packaging/linux/control`](../packaging/linux/control). Pair the headphones using the system Bluetooth settings, then open OpenSCL4 from the launcher.

For the archive, install those same runtime dependencies, extract it and run:

```sh
tar -xf openscl4-VERSION-linux-x86_64.tar.gz
./openscl4-VERSION/usr/bin/openscl4
```

### AppImage (preview)

The AppImage is a single file that runs without installation:

```sh
chmod +x openscl4-VERSION-linux-x86_64.AppImage
./openscl4-VERSION-linux-x86_64.AppImage
```

It is a later format, not part of the first release stage, and does not replace the `.deb` as the main package. CI builds, checksums and starts it, as described [below](#verification), but it has not been through the `.deb` package checks or the session checks. Its limits:

- **Dependencies.** It needs the same system libraries as the `.tar.gz`: the runtime dependencies in [`packaging/linux/control`](../packaging/linux/control), including glibc 2.39 or newer, BlueZ and `xdg-desktop-portal`. Systems older than Ubuntu 24.04 do not have a new enough glibc.
- **FUSE.** The AppImage mounts itself with FUSE through `fusermount3` or `fusermount`, from the `fuse3` package that Ubuntu Desktop installs; the older `libfuse2` is not needed. Without FUSE, for example in a container, run it with `--appimage-extract-and-run`, which unpacks it to a temporary folder on each start.
- **No automatic updates.** It carries no update information, and OpenSCL4 does not update itself. To update, download the newer AppImage from the release and check it against `SHA256SUMS`.
- **No desktop integration of its own.** It does not add a launcher entry or icon to the system; start it from a terminal or the file manager, or let a desktop integration tool add the entry.

All Linux packages use the same data folder, described below, so a running copy from one package also counts as the running instance for another.

A working tray is optional. Without a registered tray, closing the window exits. Hiding is allowed only when the tray is available; losing it should restore a hidden window. This behavior is implemented and still needs validation on the target sessions. GNOME may need an AppIndicator extension to provide a tray host; it is not required for windowed operation.

Data is kept in the system configuration directory under `openscl4/`, separately from OpenSCQ30. Use `OPENSCL4_CONFIG_DIR` for an isolated test.

Starting the app again for the same data directory asks the running window to show and then exits; a different data directory runs beside it. This uses a name on the D-Bus session bus, which the bus releases when the app exits or crashes, so there is no stale lock. If no session bus is available, the app still starts and single-instance enforcement is skipped. No tray icon is required.

## Verification

The release workflow installs/removes the `.deb` on Ubuntu 24.04/26.04 and runs X11/nested-Wayland smoke checks with [`scripts/test-linux-package.sh`](../scripts/test-linux-package.sh). These cover the application's own behaviour: one instance per data folder, separate data folders, recovery after a crash and a start without a session bus.

On the same releases, the `linux-appimage-check` job installs the runtime dependencies from `packaging/linux/control`, but no FUSE, and starts the AppImage artifact with `--appimage-extract-and-run` through [`scripts/test-linux-appimage.sh`](../scripts/test-linux-appimage.sh). Under X11 the application's window must open and closing it without a tray must end the AppImage with status 0. Under nested Wayland the application must open its window and still run after eight seconds. The check does not repeat the `.deb` checks. When it fails, it prints the failed session's logs in the job log and keeps the log folder as the `linux-appimage-check-logs-UBUNTU-attempt-N` artifact. It does not hold back the bundle, since the AppImage is not a release gate. To run it on an Ubuntu host, install the packages that job installs and run `bash scripts/test-linux-appimage.sh path/to/openscl4-VERSION-linux-x86_64.AppImage LOG_FOLDER`.

These are configured CI checks, not evidence that the full GNOME/KDE matrix or Bluetooth hardware checks have passed. See [the release gates](build-release.md#ci-and-verification) for the required session and Liberty 4 Pro evidence.
