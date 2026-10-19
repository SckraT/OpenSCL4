# OpenSCL4 on Android

The Android app uses Jetpack Compose with the Rust core through UniFFI. Minimum Android version is 8.0 (API 26). The release application ID is `io.github.sckrat.openscl4`; debug builds append `.debug`. The Kotlin namespace remains `com.oppzippy.openscq30`.

## Tools

Use Rust via rustup, Python 3.11+, and the JDK, Android SDK/build-tools, NDK and cargo-ndk versions in [`.build-tool-versions`](../.build-tool-versions). The Gradle wrapper in `android/` is authoritative; a separate system Gradle installation is unnecessary. The same tool setup is recorded in [the CI action](../.github/actions/install-android-build-tools/action.yml).

Install the specified SDK platform, build-tools and NDK with Android Studio's SDK manager or the command-line SDK manager, and accept SDK licenses. Set `ANDROID_HOME` to the SDK directory and `JAVA_HOME` to the pinned JDK. Set `ANDROID_NDK_HOME` to the selected SDK `ndk/VERSION` directory if invoking native tooling directly.

Install cargo-ndk at its pinned version. Add Rust targets for the ABIs being built:

| Android ABI   | Rust target               |
| ------------- | ------------------------- |
| `arm64-v8a`   | `aarch64-linux-android`   |
| `armeabi-v7a` | `armv7-linux-androideabi` |
| `x86_64`      | `x86_64-linux-android`    |
| `x86`         | `i686-linux-android`      |

## Debug builds

Open `android/` in Android Studio, or run from that directory:

```sh
./gradlew assembleArm64-v8aDebug
./gradlew testX86_64DebugUnitTest
```

Use `gradlew.bat` in PowerShell. Use `assembleX86_64Debug` for an x86_64 emulator. Gradle builds the native library and generates its bindings; do not edit generated Kotlin wrappers. APK outputs are under `android/app/build/outputs/apk/ABI/debug/`.

The four ABI product flavors are defined in `android/app/build.gradle.kts`. The old `assembleRelease-arm64-v8a` tasks and `build-android-universal` recipe are not current commands; the common release produces one APK per ABI.

## Release or preview APKs

Run from the repository root, choosing the ABI:

```sh
python3 scripts/release.py build android --abi arm64-v8a
```

On Windows, use `python`. Make the wrapper discoverable to the Python subprocess before invoking the script from PowerShell:

```powershell
$env:PATH = "$(Resolve-Path android);$env:JAVA_HOME\bin;$env:PATH"
python scripts/release.py build android --abi arm64-v8a
```

A signed build uses the ignored `android/openscl4-keystore.properties`, not the upstream `keystore.properties`. Create a permanent OpenSCL4 key only if one does not already exist; retain it for future updates. See [signing and backup](build-release.md#android-signing).

Append `--unsigned` for an unsigned preview and `--allow-dirty` for uncommitted source. These do not qualify as stable builds. Outputs are `openscl4-VERSION-android-ABI.apk` (or `-unsigned.apk`) and `build-android-ABI.json` in `build-output/`.

OpenSCL4 installs separately from upstream and does not automatically migrate its data. A debug APK and a release APK also have separate identities. Instrumented UI checks are distinct from JVM tests and require an emulator/device; see [Android interface validation](android-interface.md#validation). Bluetooth behavior and upgrades with the retained signing key need physical-device QA.
