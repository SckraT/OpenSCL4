# OpenSCL4 on Windows

The current desktop client is `desktop/` (Slint), packaged as `openscl4.exe`. The libcosmic application in `gui/` is [legacy](../gui/README.md).

## Prerequisites

Install Rust via rustup, the MSVC C++ build tools with a Windows SDK, and Python 3.11+. `rust-toolchain.toml` selects the required Rust toolchain. Windows release packaging targets `x86_64-pc-windows-msvc`; add that target with rustup if it is missing.

For an installer, install the exact Inno Setup version from [`.build-tool-versions`](../.build-tool-versions). The release script searches `ISCC`, `.local/iscc/ISCC.exe`, and the usual Inno Setup 6 installation path. `ISCC` can name the compiler executable explicitly. Bash and `just` are not needed for the commands below.

## Development

From the repository root in PowerShell:

```powershell
cargo build --locked -p openscq30-desktop
.\target\debug\openscl4.exe
```

For an optimized executable without packaging:

```powershell
cargo build --locked --release -p openscq30-desktop
```

The output is `target/release/openscl4.exe`. `--profile release-fast` uses the workspace's thin-LTO profile; it is a development option, not the profile used by the release workflow.

## Packages

```powershell
python scripts/release.py build windows
```

This builds the explicit Windows x64 target and writes a portable EXE, per-user installer and `build-windows.json` under `build-output/`. The names include the workspace version: `openscl4-VERSION-windows-x86_64.exe` and `openscl4-VERSION-windows-x86_64-setup.exe`. Use `--allow-dirty` only for a local preview.

The installer and application have their own OpenSCL4 identity. Initial Windows packages do not have Authenticode signatures. See [release preparation](build-release.md) for versioning, checksums and publication gates.

## Data and testing

The default data directory is `%APPDATA%/openscl4`; `OPENSCL4_CONFIG_DIR` overrides it for isolated tests. The Slint settings file is `openscl4-config.toml`, alongside `database.sqlite`. No automatic import from OpenSCQ30 is performed.

The release CI uses a Windows runner and tests the Rust library and current desktop client. A successful build does not establish physical headphone behavior or installer upgrade compatibility; record these separately before a stable release.
