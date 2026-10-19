mod android
mod cli
mod desktop
mod gui
mod i18n
mod i18n-macros
mod lib
mod lib-macros
mod lib-has

build-output-dir := "build-output"

[default]
[doc("List available recipes")]
list:
    @just --list

[doc("Run a fully optimized release build")]
[group("build")]
build-gui features='': create-build-output-dir
    just gui::build release '{{ features }}'
    cp target/release/openscq30-gui '{{ build-output-dir }}/'

[doc("Run a release build with excessively slow optimizations disabled")]
[group("build")]
build-gui-fast features='': create-build-output-dir
    just gui::build release-fast '{{ features }}'
    cp target/release-fast/openscq30-gui '{{ build-output-dir }}/'

[doc("Build the current OpenSCL4 Windows installer through the common release script.")]
[group("build")]
[windows]
build-gui-installer: create-build-output-dir
    python scripts/release.py build windows

[doc("Run a fully optimized release build")]
[group("build")]
build-cli features='': create-build-output-dir
    just cli::build release '{{ features }}'
    cp target/release/openscq30 '{{ build-output-dir }}/'

[doc("Run a release build with excessively slow optimizations disabled")]
[group("build")]
build-cli-fast features='': create-build-output-dir
    just cli::build release-fast '{{ features }}'
    cp target/release-fast/openscq30 '{{ build-output-dir }}/'

[doc("Build apks for all supported architectures")]
[group("build")]
build-android: build-android-x86 build-android-x86_64 build-android-arm64-v8a build-android-armeabi-v7a

[doc("Build an x86 apk")]
[group("build")]
build-android-x86: create-build-output-dir
    python scripts/release.py build android --abi x86

[doc("Build an x86_64 apk")]
[group("build")]
build-android-x86_64: create-build-output-dir
    python scripts/release.py build android --abi x86_64

[doc("Build an arm64-v8a apk")]
[group("build")]
build-android-arm64-v8a: create-build-output-dir
    python scripts/release.py build android --abi arm64-v8a

[doc("Build an armeabi-v7a apk")]
[group("build")]
build-android-armeabi-v7a: create-build-output-dir
    python scripts/release.py build android --abi armeabi-v7a

[group("build")]
[private]
create-build-output-dir:
    mkdir -p build-output

[doc("Run all tests")]
test: lib::test cli::test gui::test desktop::test android::test

test-cov: lib::test-cov cli::test-cov gui::test-cov desktop::test-cov android::test-cov

llvm-cov-clean:
    cargo llvm-cov clean --workspace

[arg("format", pattern="lcov|html")]
[script("bash")]
test-cov-report format='lcov':
    set -euo pipefail

    case '{{ format }}' in
        lcov)
            format_args="--lcov --output-path lcov.info"
            ;;
        html)
            format_args="--html"
            ;;
        *)
            echo Invalid format
            exit 1
            ;;
    esac

    cargo llvm-cov report $format_args

[doc("Install openscq30-gui and openscq30-cli to the specified path such as '/usr/local' or '.local'. Requires building both first using either build or build-fast. This will also install opnescq30-cli shell completions for all shells installed on the system. This can be disabled by setting OPENSCQ30_SKIP_SHELL_COMPLETIONS=1.")]
[linux]
install path:
    just gui::install '{{ path }}'
    just cli::install '{{ path }}'

[doc("Uninstall openscq30-gui and openscq30-cli from the specified path.")]
[linux]
uninstall path:
    just gui::uninstall '{{ path }}'
    just cli::uninstall '{{ path }}'

alias fmt := format

[parallel]
format: android::format cli::format desktop::format gui::format i18n::format i18n-macros::format lib::format lib-macros::format lib-has::format format-docs

[private]
[script("bash")]
format-docs:
    set -euo pipefail
    if command -v prettier > /dev/null; then
        prettier --write README.md CHANGELOG.md docs/*.md tools/soundcore-device-faker/README.md
    else
        echo "Prettier not installed, skipping markdown formatting"
    fi

[parallel]
format-check: android::format-check cli::format-check desktop::format-check gui::format-check i18n::format-check i18n-macros::format-check lib::format-check lib-macros::format-check lib-has::format-check format-check-docs

[private]
[script("bash")]
format-check-docs:
    set -euo pipefail
    if command -v prettier > /dev/null; then
        prettier --check README.md CHANGELOG.md docs/*.md tools/soundcore-device-faker/README.md
    else
        echo "Prettier not installed, skipping markdown format check"
    fi

[script("bash")]
shellcheck:
    set -euxo pipefail

    if command -v fdfind >/dev/null; then
        fd_command=fdfind
    else
        fd_command=fd
    fi
    $fd_command --type file --extension sh --exec shellcheck {}
