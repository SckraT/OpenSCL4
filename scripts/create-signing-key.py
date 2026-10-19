import json
import os
import secrets
import shutil
import subprocess
import sys
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def protect(path: Path) -> None:
    if sys.platform == "win32":
        account = subprocess.run(["whoami"], check=True, capture_output=True, text=True).stdout.strip()
        access = "(OI)(CI)F" if path.is_dir() else "F"
        subprocess.run(["icacls", str(path), "/inheritance:r", "/grant:r", f"{account}:{access}"], check=True, capture_output=True)
    else:
        path.chmod(0o700 if path.is_dir() else 0o600)


def main() -> None:
    directory = ROOT / ".local/signing"
    properties = ROOT / "android/openscl4-keystore.properties"
    keystore = directory / "openscl4.p12"
    backup = directory / "openscl4-signing-backup.zip"
    if properties.exists() or keystore.exists() or backup.exists():
        raise ValueError("OpenSCL4 signing material already exists; refusing to replace it")
    keytool = shutil.which("keytool")
    if keytool is None and os.environ.get("JAVA_HOME"):
        keytool = str(Path(os.environ["JAVA_HOME"]) / "bin" / ("keytool.exe" if sys.platform == "win32" else "keytool"))
    if keytool is None:
        raise ValueError("Set JAVA_HOME to a JDK with keytool")
    directory.mkdir(parents=True, exist_ok=True)
    protect(directory)
    password = secrets.token_urlsafe(36)
    environment = dict(os.environ, OPENSCL4_KEY_PASSWORD=password)
    subprocess.run([
        keytool, "-genkeypair", "-noprompt", "-alias", "openscl4", "-keyalg", "RSA",
        "-keysize", "4096", "-validity", "10000", "-dname", "CN=OpenSCL4",
        "-storetype", "PKCS12", "-keystore", str(keystore),
        "-storepass:env", "OPENSCL4_KEY_PASSWORD", "-keypass:env", "OPENSCL4_KEY_PASSWORD",
    ], check=True, env=environment, capture_output=True)
    protect(keystore)
    with properties.open("x", encoding="utf-8") as target:
        protect(properties)
        store_file = json.dumps(keystore.as_posix(), ensure_ascii=True)[1:-1]
        target.write(f"storeFile={store_file}\nstorePassword={password}\nkeyPassword={password}\nkeyAlias=openscl4\n")
    with zipfile.ZipFile(backup, "x", compression=zipfile.ZIP_DEFLATED) as target:
        target.write(keystore, "openscl4.p12")
        target.write(properties, "openscl4-keystore.properties")
    protect(backup)
    print("Created the OpenSCL4 release key and a private recovery bundle in .local/signing/.")
    print("Keep a protected copy of the recovery bundle on separate storage before publishing.")


if __name__ == "__main__":
    try:
        main()
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        sys.exit(str(error))
