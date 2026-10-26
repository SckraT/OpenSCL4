# Building and releasing OpenSCL4

OpenSCL4 uses Compose on Android and Slint in `desktop/` on Windows and Linux. `gui/` is the older desktop implementation and is not a release target. macOS is a later stage.

This guide describes implemented release tooling and required gates, not a completed stable release. At the 2026-10-19 audit, local Android ARM64 and Windows x64 preview packages had been built, no GitHub releases were published, and Linux session/hardware QA remained open. See the [current platform status](../README.md#platform-status).

## Inputs and tools

Use Python 3.11 or newer. Product version comes from `[workspace.package].version` in `Cargo.toml`; `Cargo.lock`, `rust-toolchain.toml`, `.build-tool-versions`, the Gradle wrapper and Gradle version catalog fix the other inputs. CI and local packaging call `scripts/release.py` on the appropriate operating system. This controls inputs and records provenance; it does not promise byte-identical packages.

Use the exact versions in [`.build-tool-versions`](../.build-tool-versions), not the newest system defaults. Install the Rust target for the chosen platform. Android also needs the corresponding NDK targets and SDK licences; set `JAVA_HOME` and `ANDROID_HOME`. Windows needs Visual Studio C++ build tools and the pinned Inno compiler, discoverable through `ISCC`, `.local/iscc/ISCC.exe` or its normal installation path. Platform setup is covered in [Windows](build-windows.md), [Android](build-android.md) and [Linux](build-linux.md).

Build Linux packages on x86_64 Ubuntu 24.04, including when targeting Ubuntu 26.04. Install the build dependencies listed in `.github/workflows/build.yml`; the release script bundles the library's supported native dependencies, but the package still declares desktop/Bluetooth runtime dependencies. A newer build host can raise the minimum glibc requirement and is rejected.

For desktop development:

```text
cargo build --locked -p openscq30-desktop
cargo test --locked -p openscq30-lib -p openscq30-desktop -p openscq30-android
```

The executable is `target/debug/openscl4[.exe]`. Set `OPENSCL4_CONFIG_DIR` to isolate test data. The normal data directory is the platform configuration directory followed by `openscl4`. Android's release application ID is `io.github.sckrat.openscl4`, with `.debug` appended for debug builds. OpenSCQ30 data is not imported automatically.

## Version and platform builds

Choose the first published version during release preparation; the source version in `Cargo.toml` is not proof of publication. Do not reset it without accounting for Android's monotonically increasing version codes. Change the common version with:

```text
python scripts/release.py set-version 2.14.0
cargo check -p openscq30-desktop -p openscq30-android
```

Review and include the resulting Cargo lockfile changes with the source. The setter also replaces the first `Unreleased` changelog heading. Commit the complete source before a release build and use one `v<version>` tag. A platform hotfix increments the common patch version and rebuilds every platform.

Run each command on its matching host, from the same commit:

```text
python scripts/release.py build windows
python scripts/release.py build linux
python scripts/release.py build android --abi arm64-v8a
python scripts/release.py build android --abi armeabi-v7a
python scripts/release.py build android --abi x86_64
python scripts/release.py build android --abi x86
```

Outputs go to `build-output/`. Windows produces a portable EXE and a per-user installer; Linux produces `.deb` and `.tar.gz`; Android produces one APK per ABI. Each job records the source revision, version, actual/configured tools, build parameters, duration and package hashes in `build-*.json`. For PowerShell Android builds, make the Gradle wrapper discoverable as described in [the Android guide](build-android.md#release-or-preview-apks).

For a local source preview, append `--allow-dirty`. For an unsigned Android preview, append `--unsigned`. These flags mark the build as preview; stable verification rejects it. A dirty manifest's commit identifies its base, not all uncommitted contents.

## Android signing

Create the new line's permanent key once with `python scripts/create-signing-key.py`. It refuses to overwrite existing material. It creates the private `.local/signing/openscl4.p12`, ignored `android/openscl4-keystore.properties`, and `.local/signing/openscl4-signing-backup.zip`. Do not run this to rotate a released key. Keep this key for subsequent OpenSCL4 updates.

Copy the backup to separate protected storage before publishing or removing the worktree. A copy on the same disk is not disaster recovery. Do not commit keys, passwords or backup archives. The old application's keystore is not used.

For signed CI builds, configure repository Actions secrets `OPENSCL4_KEYSTORE_BASE64` (the new PKCS12 file encoded as base64) and `OPENSCL4_KEY_PASSWORD` (its generated password). Alias is `openscl4`. Transfer these through a private secret-management interface; do not print them to a shared terminal or chat. CI reconstructs ignored local signing configuration. Signed APKs are checked with `apksigner`, and their public certificate SHA-256 must agree across all ABIs. Windows initially ships without Authenticode.

## CI and verification

The `OpenSCL4 release builds` workflow supports manual preview runs and `v*` tags. A manual run with `platforms: linux` builds and checks only the Linux packages, without the complete bundle; [the Linux guide](build-linux.md#development-and-packaging) shows the commands. All platform jobs check out the source job's exact commit. Its Linux package checks install/remove the `.deb` on Ubuntu 24.04 and 26.04, exercise X11 and nested Wayland, and test closing without a tray. These checks do not replace GNOME/KDE session and real-headphone QA. Failed jobs block assembling the complete preview bundle; successful individual jobs retain their artifacts.

Download the platform packages and all six build reports into one output directory. Verify an unsigned or otherwise preliminary complete set with:

```text
python scripts/release.py verify --preview
```

For stable verification, use signed Android packages, clean source builds and a real Linux Liberty 4 Pro check for the exact commit. Record truthful results in a JSON file of this form; change a check to `true` only after it passes:

```json
{
    "commit": "FULL_SOURCE_COMMIT",
    "model": "Liberty 4 Pro",
    "connect": false,
    "settings": false,
    "reconnect": false,
    "suspendResume": false,
    "quit": false
}
```

Run `python scripts/release.py verify --linux-evidence path/to/linux-qa.json`. Verification checks the complete package set, matching version/revision/identity/tool pins/certificate and package hashes, then writes `SHA256SUMS`. It checks manifests and recorded evidence; it does not run the physical checks or authenticate a manually edited report.

Before first stable publication, verify these sessions on x86_64: Ubuntu 24.04 GNOME Wayland and X11; Kubuntu 24.04 X11; Ubuntu 26.04 GNOME Wayland; Kubuntu 26.04 Wayland. Check install/start/remove, no-tray close, hiding only with a registered tray, recovery when the tray disappears, and clean shutdown. Record session details alongside the JSON. Repeat real-device tests after Bluetooth or connection-lifecycle changes. Test installation alongside OpenSCQ30 and upgrades within the OpenSCL4 line with its own data.

Publication remains manual in [SckraT/OpenSCL4 Releases](https://github.com/SckraT/OpenSCL4/releases), after all mandatory jobs and checks pass. Upload the verified packages, build reports, checksums and release notes for the same tag. The workflow never publishes a stable release automatically. macOS, stores, AppImage, Flatpak and an updater are not part of this first stage.
