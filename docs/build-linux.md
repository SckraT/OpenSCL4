# OpenSCL4 on Linux (experimental)

The current desktop client is `desktop/` (Slint). Linux packaging and CI checks exist, but the release session matrix and real-headphone QA have not been established as complete. The older libcosmic GUI and its `cosmic-icons` instructions are [legacy](../gui/README.md), not OpenSCL4 runtime requirements.

## Target systems

The first target is x86_64 Ubuntu/Kubuntu 24.04 and 26.04. Before stable publication, check Ubuntu 24.04 GNOME on Wayland and X11; Kubuntu 24.04 on X11; Ubuntu 26.04 GNOME and Kubuntu 26.04 on Wayland. Other architectures and distributions are later work, not a tested support promise.

## Build prerequisites

Install Python 3.11+, Rust via rustup, a C/C++ build toolchain, and the development packages used by [release CI](../.github/workflows/build.yml). For Ubuntu:

```sh
sudo apt-get update
sudo apt-get install build-essential pkg-config libdbus-1-dev libxkbcommon-dev libfontconfig1-dev libsqlite3-dev desktop-file-utils
```

The Rust version is selected by `rust-toolchain.toml`. For packaging, add `x86_64-unknown-linux-gnu` with rustup if needed. Release packages must be built on **Ubuntu 24.04 x86_64**, including packages intended for 26.04, to preserve the glibc baseline; the release script rejects a different build host.

## Development and packaging

```sh
cargo build --locked -p openscq30-desktop
./target/debug/openscl4
python3 scripts/release.py build linux
```

The last command produces `openscl4_VERSION_amd64.deb`, `openscl4-VERSION-linux-x86_64.tar.gz`, and `build-linux.json` in `build-output/`. The tarball contains the same staged application files as the Debian package; it is not an AppImage or a distribution-independent runtime. Use `--allow-dirty` only for a source preview.

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

A working tray is optional. Without a registered tray, closing the window exits. Hiding is allowed only when the tray is available; losing it should restore a hidden window. This behavior is implemented and still needs validation on the target sessions. GNOME may need an AppIndicator extension to provide a tray host; it is not required for windowed operation.

Data is kept in the system configuration directory under `openscl4/`, separately from OpenSCQ30. Use `OPENSCL4_CONFIG_DIR` for an isolated test.

Starting the app again for the same data directory asks the running window to show and then exits; a different data directory runs beside it. This uses a name on the D-Bus session bus, which the bus releases when the app exits or crashes, so there is no stale lock. If no session bus is available, the app still starts and single-instance enforcement is skipped. No tray icon is required.

## Verification

The release workflow installs/removes the `.deb` on Ubuntu 24.04/26.04 and runs X11/nested-Wayland smoke checks with [`scripts/test-linux-package.sh`](../scripts/test-linux-package.sh). These are configured CI checks, not evidence that the full GNOME/KDE matrix or Bluetooth hardware checks have passed. See [the release gates](build-release.md#ci-and-verification) for the required session and Liberty 4 Pro evidence.
