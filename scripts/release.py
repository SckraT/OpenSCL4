import argparse
import hashlib
import json
import os
import platform
import re
import shutil
import subprocess
import sys
import tarfile
import tempfile
import time
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
APP_ID = "io.github.sckrat.openscl4"
ABIS = ("arm64-v8a", "armeabi-v7a", "x86_64", "x86")
BUILD_ENVIRONMENT = (
    "RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "CARGO_BUILD_JOBS", "CARGO_TARGET_DIR",
    "SOURCE_DATE_EPOCH", "CC", "CXX", "AR",
)


def configured_tools() -> dict[str, str]:
    return dict(line.split(maxsplit=1) for line in (ROOT / ".build-tool-versions").read_text().splitlines() if line.strip())


def version() -> str:
    with (ROOT / "Cargo.toml").open("rb") as source:
        value = tomllib.load(source)["workspace"]["package"]["version"]
    if not re.fullmatch(r"\d+\.\d+\.\d+", value):
        raise ValueError("Release version must have the form major.minor.patch")
    return value


def set_version(value: str) -> None:
    if not re.fullmatch(r"(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)", value):
        raise ValueError("Version must have the form major.minor.patch")
    major, minor, patch = map(int, value.split("."))
    if major > 2000 or minor > 999 or patch > 999 or major + minor + patch == 0:
        raise ValueError("Version does not fit the Android versionCode scheme")
    path = ROOT / "Cargo.toml"
    text = path.read_text(encoding="utf-8")
    before, marker, after = text.partition("[workspace.package]\n")
    if not marker:
        raise ValueError("Missing workspace.package section")
    section, next_section, rest = after.partition("\n[")
    section, count = re.subn(r'(?m)^version\s*=\s*"[^"\n]+"', f'version = "{value}"', section)
    if count != 1:
        raise ValueError("Expected exactly one product version")
    path.write_text(before + marker + section + next_section + rest, encoding="utf-8")
    changelog = ROOT / "CHANGELOG.md"
    if changelog.exists():
        text = changelog.read_text(encoding="utf-8")
        changelog.write_text(text.replace("## Unreleased", f"## v{value}", 1), encoding="utf-8")


def target_directory() -> Path:
    path = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target"))
    return (path if path.is_absolute() else ROOT / path).resolve()


def run(args: list[str], *, cwd: Path = ROOT, capture: bool = False) -> str:
    # Windows cannot execute .bat/.cmd images directly, so run them through the command interpreter.
    if sys.platform == "win32" and Path(args[0]).suffix.lower() in {".bat", ".cmd"}:
        args = ["cmd", "/c", *args]
    result = subprocess.run(args, cwd=cwd, check=True, text=True, capture_output=capture)
    return result.stdout.strip() if capture else ""


def sha256(path: Path) -> str:
    with path.open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def inno_compiler() -> Path:
    configured = os.environ.get("ISCC") or shutil.which("ISCC")
    candidates = [Path(configured)] if configured else [
        ROOT / ".local/iscc/ISCC.exe",
        Path(os.environ.get("ProgramFiles(x86)", "C:/Program Files (x86)")) / "Inno Setup 6/ISCC.exe",
    ]
    for candidate in candidates:
        if candidate.is_file():
            return candidate
    raise ValueError("Install the pinned Inno Setup compiler or set ISCC to its executable")


def build_windows(output: Path) -> list[Path]:
    if sys.platform != "win32":
        raise ValueError("Windows packaging must run on Windows")
    compiler = str(inno_compiler())
    run(["cargo", "build", "--locked", "--release", "--target", "x86_64-pc-windows-msvc", "-p", "openscq30-desktop"])
    with tempfile.TemporaryDirectory(prefix="openscl4-windows-", dir=ROOT / "target") as temporary:
        stage = Path(temporary).resolve()
        if not stage.is_relative_to((ROOT / "target").resolve()):
            raise ValueError("Invalid staging directory")
        shutil.copy2(target_directory() / "x86_64-pc-windows-msvc/release/openscl4.exe", stage / "openscl4.exe")
        shutil.copy2(ROOT / "LICENSE.txt", stage / "LICENSE.txt")
        run([compiler, f"/DAppVersion={version()}", f"/DSourceDir={stage}", f"/O{output}", str(ROOT / "packaging/windows/setup.iss")])
        portable = output / f"openscl4-{version()}-windows-x86_64.exe"
        shutil.copy2(stage / "openscl4.exe", portable)
    return [portable, output / f"openscl4-{version()}-windows-x86_64-setup.exe"]


def build_linux(output: Path) -> list[Path]:
    if sys.platform != "linux" or platform.machine() != "x86_64":
        raise ValueError("Linux packaging must run on x86_64 Linux")
    distribution = platform.freedesktop_os_release()
    if distribution.get("ID") != "ubuntu" or distribution.get("VERSION_ID") != "24.04":
        raise ValueError("Build Linux release packages on Ubuntu 24.04 for the supported glibc baseline")
    run(["cargo", "build", "--locked", "--release", "--target", "x86_64-unknown-linux-gnu", "-p", "openscq30-desktop", "--features", "openscq30-lib/bundled-dependencies"])
    with tempfile.TemporaryDirectory(prefix="openscl4-linux-", dir=ROOT / "target") as temporary:
        stage = Path(temporary).resolve()
        if not stage.is_relative_to((ROOT / "target").resolve()):
            raise ValueError("Invalid staging directory")
        files = {
            target_directory() / "x86_64-unknown-linux-gnu/release/openscl4": stage / "usr/bin/openscl4",
            ROOT / "packaging/linux/io.github.sckrat.openscl4.desktop": stage / "usr/share/applications/io.github.sckrat.openscl4.desktop",
            ROOT / "desktop/ui/icons/app.svg": stage / "usr/share/icons/hicolor/scalable/apps/io.github.sckrat.openscl4.svg",
            ROOT / "LICENSE.txt": stage / "usr/share/doc/openscl4/copyright",
            ROOT / "packaging/linux/README.md": stage / "usr/share/doc/openscl4/README.md",
        }
        for source, destination in files.items():
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(source, destination)
        archive = output / f"openscl4-{version()}-linux-x86_64.tar.gz"
        with tarfile.open(archive, "w:gz") as target:
            target.add(stage / "usr", arcname=f"openscl4-{version()}/usr")
        control = stage / "DEBIAN/control"
        control.parent.mkdir()
        template = (ROOT / "packaging/linux/control").read_text(encoding="utf-8")
        control.write_text(template.replace("@VERSION@", version()), encoding="utf-8")
        deb = output / f"openscl4_{version()}_amd64.deb"
        run(["dpkg-deb", "--root-owner-group", "--build", str(stage), str(deb)])
        run(["dpkg-deb", "--info", str(deb)])
    return [archive, deb]


def build_android(output: Path, abi: str, unsigned: bool) -> list[Path]:
    task = f"assemble{abi[0].upper()}{abi[1:]}Release"
    wrapper = "gradlew.bat" if sys.platform == "win32" else "./gradlew"
    run([wrapper, task, f"-Popenscl4Unsigned={str(unsigned).lower()}"], cwd=ROOT / "android")
    apk_dir = ROOT / f"android/app/build/outputs/apk/{abi}/release"
    metadata = json.loads((apk_dir / "output-metadata.json").read_text(encoding="utf-8"))
    if metadata["applicationId"] != APP_ID:
        raise ValueError("APK has the wrong application ID")
    element = metadata["elements"][0]
    if element["versionName"] != version():
        raise ValueError("APK version does not match Cargo.toml")
    source = apk_dir / element["outputFile"]
    if unsigned != ("unsigned" in source.name):
        raise ValueError("APK signing state does not match the requested build")
    suffix = "-unsigned" if unsigned else ""
    target = output / f"openscl4-{version()}-android-{abi}{suffix}.apk"
    shutil.copy2(source, target)
    return [target]


def android_certificate(apk: Path) -> str:
    sdk = os.environ.get("ANDROID_HOME") or os.environ.get("ANDROID_SDK_ROOT")
    if sdk is None:
        raise ValueError("Set ANDROID_HOME to verify the signed APK")
    executable = "apksigner.bat" if sys.platform == "win32" else "apksigner"
    signer = Path(sdk) / "build-tools" / configured_tools()["android-build-tools"] / executable
    details = run([str(signer), "verify", "--print-certs", str(apk)], capture=True)
    certificates = {
        value.lower() for value in re.findall(
            r"(?m)^(?:Signer #\d+|V\d+(?:\.\d+)? Signer:) certificate SHA-256 digest: ([0-9a-fA-F]{64})\s*$",
            details,
        )
    }
    if len(certificates) != 1:
        raise ValueError("APK verification did not report exactly one signing certificate")
    return certificates.pop()


def java_version() -> str:
    java_home = os.environ.get("JAVA_HOME")
    executable = "java.exe" if sys.platform == "win32" else "java"
    java = str(Path(java_home) / "bin" / executable) if java_home else executable
    return run([java, "--version"], capture=True)


def build(args: argparse.Namespace) -> None:
    revision = run(["git", "rev-parse", "HEAD"], capture=True)
    dirty = bool(run(["git", "status", "--porcelain"], capture=True))
    if dirty and not args.allow_dirty:
        raise ValueError("Working tree is dirty; use --allow-dirty for a local preview only")
    tools = configured_tools()
    with (ROOT / "rust-toolchain.toml").open("rb") as source:
        if tomllib.load(source)["toolchain"]["channel"] != tools["rust"]:
            raise ValueError("Rust toolchain pins disagree")
    compiler_version = run(["rustc", "--version"], capture=True)
    if compiler_version.split()[1] != tools["rust"]:
        raise ValueError("Installed Rust compiler does not match the release pin")
    java = java_version() if args.platform == "android" else None
    cargo_ndk = run(["cargo", "ndk", "--version"], capture=True) if args.platform == "android" else None
    if java is not None and re.search(r'(?:openjdk|java) (?:version )?"?' + re.escape(tools["jdk"]) + r'[.\s"]', java) is None:
        raise ValueError("JAVA_HOME does not match the release JDK pin")
    if cargo_ndk is not None and cargo_ndk.split()[1] != tools["cargo-ndk"]:
        raise ValueError("Installed cargo-ndk does not match the release pin")
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    (ROOT / "target").mkdir(exist_ok=True)
    started = time.perf_counter()
    if args.platform == "windows":
        artifacts = build_windows(output)
    elif args.platform == "linux":
        artifacts = build_linux(output)
    else:
        artifacts = build_android(output, args.abi, args.unsigned)
    name = f"android-{args.abi}" if args.platform == "android" else args.platform
    report = {
        "application": "OpenSCL4",
        "applicationId": APP_ID,
        "version": version(),
        "commit": revision,
        "dirty": dirty,
        "preview": args.allow_dirty or args.unsigned,
        "platform": args.platform,
        "abi": args.abi if args.platform == "android" else "x86_64",
        "profile": "release-android" if args.platform == "android" else "release",
        "signed": args.platform == "android" and not args.unsigned,
        "androidCertificateSha256": android_certificate(artifacts[0]) if args.platform == "android" and not args.unsigned else None,
        "host": platform.platform(),
        "rustc": compiler_version,
        "configuredTools": tools,
        "buildEnvironment": {name: os.environ[name] for name in BUILD_ENVIRONMENT if name in os.environ},
        "java": java,
        "cargoNdk": cargo_ndk,
        "innoCompilerSha256": sha256(inno_compiler()) if args.platform == "windows" else None,
        "durationSeconds": round(time.perf_counter() - started, 3),
        "artifacts": [{"file": item.name, "sha256": sha256(item)} for item in artifacts],
    }
    (output / f"build-{name}.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")


def verify(output: Path, linux_evidence: Path | None, *, preview: bool = False, expected_commit: str | None = None) -> list[dict]:
    reports = [json.loads(path.read_text(encoding="utf-8")) for path in sorted(output.glob("build-*.json"))]
    required = {("windows", "x86_64"), ("linux", "x86_64"), *(("android", abi) for abi in ABIS)}
    if {(report["platform"], report["abi"]) for report in reports} != required or len(reports) != len(required):
        raise ValueError("A complete Windows/Linux/four-ABI Android build set is required")
    if len({(report["version"], report["commit"], report["applicationId"]) for report in reports}) != 1:
        raise ValueError("Builds must have one version, commit and application ID")
    if reports[0]["applicationId"] != APP_ID or reports[0]["version"] != version():
        raise ValueError("Build identity does not match this source checkout")
    if expected_commit is not None and reports[0]["commit"] != expected_commit:
        raise ValueError("Bundle commit does not match this source checkout")
    if any(report.get("application") != "OpenSCL4" for report in reports):
        raise ValueError("Build reports must carry the OpenSCL4 identity")
    pins = configured_tools()
    if any(report.get("configuredTools") != pins for report in reports):
        raise ValueError("Build reports must use this checkout's tool pins")
    certificates = {report.get("androidCertificateSha256") for report in reports if report["platform"] == "android" and report["signed"]}
    if len(certificates) > 1 or any(not certificate or not re.fullmatch(r"[0-9a-f]{64}", certificate) for certificate in certificates):
        raise ValueError("Signed Android builds must have one verified signing certificate")
    seen = set()
    for report in reports:
        if not preview and (report["dirty"] or report.get("preview", False) or (report["platform"] == "android" and not report["signed"])):
            raise ValueError("Stable builds require a clean tree and signed Android APKs")
        if report["platform"] == "windows":
            expected = {f'openscl4-{version()}-windows-x86_64.exe', f'openscl4-{version()}-windows-x86_64-setup.exe'}
        elif report["platform"] == "linux":
            expected = {f'openscl4-{version()}-linux-x86_64.tar.gz', f'openscl4_{version()}_amd64.deb'}
        else:
            suffix = "" if report["signed"] else "-unsigned"
            expected = {f'openscl4-{version()}-android-{report["abi"]}{suffix}.apk'}
        if {artifact["file"] for artifact in report["artifacts"]} != expected:
            raise ValueError("Build manifest has an incomplete or unexpected package set")
        for artifact in report["artifacts"]:
            name = artifact["file"]
            if Path(name).name != name or name in seen:
                raise ValueError("Invalid artifact filename")
            seen.add(name)
            if sha256(output / name) != artifact["sha256"]:
                raise ValueError(f"Checksum mismatch: {name}")
    if not preview:
        if linux_evidence is None:
            raise ValueError("Stable verification requires --linux-evidence from the real-device check")
        evidence = json.loads(linux_evidence.read_text(encoding="utf-8"))
        checks = ("connect", "settings", "reconnect", "suspendResume", "quit")
        if evidence.get("commit") != reports[0]["commit"] or evidence.get("model") != "Liberty 4 Pro" or not all(evidence.get(check) is True for check in checks):
            raise ValueError("Linux evidence must match the commit and pass all real-device checks")
    lines = sorted(f'{artifact["sha256"]}  {artifact["file"]}' for report in reports for artifact in report["artifacts"])
    (output / "SHA256SUMS").write_text("\n".join(lines) + "\n", encoding="utf-8")
    return reports


def main() -> None:
    parser = argparse.ArgumentParser(description="Build and verify an OpenSCL4 release from one revision")
    commands = parser.add_subparsers(dest="command", required=True)
    commands.add_parser("version")
    setter = commands.add_parser("set-version")
    setter.add_argument("version")
    builder = commands.add_parser("build")
    builder.add_argument("platform", choices=("windows", "linux", "android"))
    builder.add_argument("--abi", choices=ABIS, default="arm64-v8a")
    builder.add_argument("--unsigned", action="store_true")
    builder.add_argument("--allow-dirty", action="store_true")
    builder.add_argument("--output", type=Path, default=ROOT / "build-output")
    verifier = commands.add_parser("verify")
    verifier.add_argument("--output", type=Path, default=ROOT / "build-output")
    verifier.add_argument("--linux-evidence", type=Path)
    verifier.add_argument("--preview", action="store_true")
    args = parser.parse_args()
    if args.command == "version":
        print(version())
    elif args.command == "set-version":
        set_version(args.version)
    elif args.command == "build":
        build(args)
    else:
        revision = run(["git", "rev-parse", "HEAD"], capture=True)
        reports = verify(args.output, args.linux_evidence, preview=args.preview, expected_commit=revision)
        print(f"Verified OpenSCL4 {reports[0]['version']} at {reports[0]['commit']}")


if __name__ == "__main__":
    try:
        main()
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        sys.exit(str(error))
