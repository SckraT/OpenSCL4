import json
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import release


class ReleaseVerificationTest(unittest.TestCase):
    def setUp(self) -> None:
        (release.ROOT / "target").mkdir(exist_ok=True)
        self.temporary = tempfile.TemporaryDirectory(dir=release.ROOT / "target")
        self.output = Path(self.temporary.name).resolve()
        self.assertTrue(self.output.is_relative_to((release.ROOT / "target").resolve()))
        self.reports = []
        product_version = release.version()
        builds = [
            ("windows", "x86_64", [f"openscl4-{product_version}-windows-x86_64.exe", f"openscl4-{product_version}-windows-x86_64-setup.exe"]),
            ("linux", "x86_64", [f"openscl4-{product_version}-linux-x86_64.tar.gz", f"openscl4_{product_version}_amd64.deb"]),
            *(("android", abi, [f"openscl4-{product_version}-android-{abi}.apk"]) for abi in release.ABIS),
        ]
        for os_name, abi, filenames in builds:
            artifacts = []
            for filename in filenames:
                path = self.output / filename
                path.write_bytes(filename.encode())
                artifacts.append({"file": filename, "sha256": release.sha256(path)})
            report = {
                "application": "OpenSCL4",
                "applicationId": release.APP_ID,
                "version": product_version,
                "commit": "a" * 40,
                "configuredTools": release.configured_tools(),
                "dirty": False,
                "platform": os_name,
                "abi": abi,
                "signed": os_name == "android",
                "androidCertificateSha256": "b" * 64 if os_name == "android" else None,
                "artifacts": artifacts,
            }
            path = self.output / f"build-{os_name}-{abi}.json"
            self.reports.append((path, report))
        self.save_reports()
        self.evidence = self.output / "linux-qa.json"
        self.evidence.write_text(json.dumps({
            "commit": "a" * 40,
            "model": "Liberty 4 Pro",
            "connect": True,
            "settings": True,
            "reconnect": True,
            "suspendResume": True,
            "quit": True,
        }))

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def save_reports(self) -> None:
        for path, report in self.reports:
            path.write_text(json.dumps(report), encoding="utf-8")

    def test_complete_same_revision_set_has_checksums(self) -> None:
        reports = release.verify(self.output, self.evidence, expected_commit="a" * 40)
        self.assertEqual(6, len(reports))
        self.assertEqual(8, len((self.output / "SHA256SUMS").read_text().splitlines()))

    def add_linux_package(self, filename: str) -> None:
        path = self.output / filename
        path.write_bytes(filename.encode())
        report = next(report for _, report in self.reports if report["platform"] == "linux")
        report["artifacts"].append({"file": filename, "sha256": release.sha256(path)})
        self.save_reports()

    def test_preview_set_with_an_appimage_lists_it_in_the_checksums(self) -> None:
        appimage = f"openscl4-{release.version()}-linux-x86_64.AppImage"
        self.add_linux_package(appimage)

        release.verify(self.output, None, preview=True)

        checksums = (self.output / "SHA256SUMS").read_text().splitlines()
        self.assertIn(f"{release.sha256(self.output / appimage)}  {appimage}", checksums)
        self.assertEqual(9, len(checksums))

    def test_stable_set_neither_requires_nor_drops_the_appimage(self) -> None:
        release.verify(self.output, self.evidence, expected_commit="a" * 40)
        appimage = f"openscl4-{release.version()}-linux-x86_64.AppImage"
        self.add_linux_package(appimage)

        release.verify(self.output, self.evidence, expected_commit="a" * 40)

        self.assertIn(appimage, (self.output / "SHA256SUMS").read_text())

    def test_appimage_for_another_architecture_is_rejected(self) -> None:
        self.add_linux_package(f"openscl4-{release.version()}-linux-aarch64.AppImage")

        with self.assertRaisesRegex(ValueError, "package set"):
            release.verify(self.output, None, preview=True)

    def test_missing_platform_or_android_abi_is_rejected(self) -> None:
        self.reports[0][0].unlink()
        with self.assertRaisesRegex(ValueError, "complete"):
            release.verify(self.output, self.evidence)

    def test_mixed_source_revisions_are_rejected(self) -> None:
        self.reports[-1][1]["commit"] = "c" * 40
        self.save_reports()
        with self.assertRaisesRegex(ValueError, "one version, commit"):
            release.verify(self.output, self.evidence)

    def test_other_application_identity_is_rejected(self) -> None:
        self.reports[-1][1]["application"] = "OpenSCQ30"
        self.save_reports()
        with self.assertRaisesRegex(ValueError, "OpenSCL4 identity"):
            release.verify(self.output, self.evidence)

    def test_report_tool_pins_must_match_this_checkout(self) -> None:
        self.reports[0][1]["configuredTools"] = dict(release.configured_tools(), ndk="0.0.0")
        self.save_reports()
        with self.assertRaisesRegex(ValueError, "tool pins"):
            release.verify(self.output, None, preview=True)

    def test_other_checkout_revision_is_rejected(self) -> None:
        with self.assertRaisesRegex(ValueError, "source checkout"):
            release.verify(self.output, self.evidence, expected_commit="c" * 40)
        self.assertFalse((self.output / "SHA256SUMS").exists())

    def test_modified_package_is_rejected(self) -> None:
        filename = self.reports[0][1]["artifacts"][0]["file"]
        (self.output / filename).write_bytes(b"changed")
        with self.assertRaisesRegex(ValueError, "Checksum mismatch"):
            release.verify(self.output, self.evidence)

    def test_missing_installer_is_rejected(self) -> None:
        self.reports[0][1]["artifacts"].pop()
        self.save_reports()
        with self.assertRaisesRegex(ValueError, "package set"):
            release.verify(self.output, self.evidence)

    def test_dirty_tree_is_preview_only(self) -> None:
        self.reports[0][1]["dirty"] = True
        self.save_reports()
        with self.assertRaisesRegex(ValueError, "clean tree"):
            release.verify(self.output, self.evidence)
        release.verify(self.output, None, preview=True)

    def test_android_certificates_must_match(self) -> None:
        self.reports[-1][1]["androidCertificateSha256"] = "c" * 64
        self.save_reports()
        with self.assertRaisesRegex(ValueError, "signing certificate"):
            release.verify(self.output, self.evidence)

    def test_android_certificate_accepts_old_and_pinned_sdk_output(self) -> None:
        digest = "A1" * 32
        for prefix in ["Signer #1", "V2 Signer:", "V3.1 Signer:"]:
            with self.subTest(prefix=prefix), patch.dict(release.os.environ, {"ANDROID_HOME": str(self.output)}), patch.object(
                release, "run", return_value=f"{prefix} certificate SHA-256 digest: {digest}\n",
            ):
                self.assertEqual(digest.lower(), release.android_certificate(self.output / "app.apk"))

    def test_android_certificate_rejects_missing_invalid_or_different_signers(self) -> None:
        for output in [
            "Verified successfully",
            "V2 Signer: certificate SHA-256 digest: abc\n",
            f"V2 Signer: certificate SHA-256 digest: {'a' * 64}\nV3 Signer: certificate SHA-256 digest: {'b' * 64}\n",
        ]:
            with self.subTest(output=output), patch.dict(release.os.environ, {"ANDROID_HOME": str(self.output)}), patch.object(
                release, "run", return_value=output,
            ), self.assertRaisesRegex(ValueError, "exactly one signing certificate"):
                release.android_certificate(self.output / "app.apk")

    def _android_build_fixture(self, abi: str, *, signed: bool) -> None:
        """Lay out what build_android reads, relative to a patched ROOT."""
        (self.output / "Cargo.toml").write_text(
            f'[workspace.package]\nversion = "{release.version()}"\n',
        )
        apk_dir = self.output / f"android/app/build/outputs/apk/{abi}/release"
        apk_dir.mkdir(parents=True, exist_ok=True)
        suffix = "" if signed else "-unsigned"
        apk = apk_dir / f"openscl4-{release.version()}-android-{abi}{suffix}.apk"
        apk.write_bytes(b"apk")
        (apk_dir / "output-metadata.json").write_text(json.dumps({
            "applicationId": release.APP_ID,
            "elements": [{"versionName": release.version(), "outputFile": apk.name}],
        }))

    def test_android_build_asks_gradle_for_the_abi_release_task(self) -> None:
        self._android_build_fixture("arm64-v8a", signed=True)

        with patch.object(release.sys, "platform", "linux"), patch.object(
            release, "ROOT", self.output,
        ), patch.object(release, "run") as run:
            artifacts = release.build_android(self.output, "arm64-v8a", unsigned=False)

        self.assertEqual(
            ["./gradlew", "assembleArm64-v8aRelease", "-Popenscl4Unsigned=false"],
            run.call_args_list[0].args[0],
        )
        self.assertEqual(self.output / f"openscl4-{release.version()}-android-arm64-v8a.apk", artifacts[0])

    def test_android_build_rejects_a_mis_signed_apk(self) -> None:
        self._android_build_fixture("arm64-v8a", signed=False)

        with patch.object(release.sys, "platform", "linux"), patch.object(
            release, "ROOT", self.output,
        ), patch.object(release, "run"):
            with self.assertRaisesRegex(ValueError, "signing state"):
                release.build_android(self.output, "arm64-v8a", unsigned=False)

    def test_windows_resolves_a_bare_executable_against_its_working_directory(self) -> None:
        # Windows searches PATH for a bare name, not cwd, so the wrapper in android/ would never be found.
        completed = subprocess.CompletedProcess([], 0, stdout="ok\n")
        (self.output / "gradlew.bat").write_bytes(b"")

        with patch.object(release.sys, "platform", "win32"), patch.object(
            release.subprocess, "run", return_value=completed,
        ) as run:
            release.run(["gradlew.bat", "--version"], cwd=self.output)

        self.assertEqual(
            [str(self.output / "gradlew.bat"), "--version"],
            run.call_args.args[0],
        )

    def test_windows_leaves_absolute_and_path_lookup_commands_alone(self) -> None:
        completed = subprocess.CompletedProcess([], 0, stdout="ok\n")
        (self.output / "tool.bat").write_bytes(b"")
        absolute = str(self.output / "tool.bat")

        for command in (["cargo", "build"], [absolute, "verify"], ["./gradlew", "build"]):
            with self.subTest(command=command), patch.object(
                release.sys, "platform", "win32",
            ), patch.object(release.subprocess, "run", return_value=completed) as run:
                release.run(command, cwd=self.output)

            self.assertEqual(command, run.call_args.args[0])

    def test_other_platforms_start_the_wrapper_as_given(self) -> None:
        completed = subprocess.CompletedProcess([], 0, stdout="ok\n")
        (self.output / "tool.bat").write_bytes(b"")

        with patch.object(release.sys, "platform", "linux"), patch.object(
            release.subprocess, "run", return_value=completed,
        ) as run:
            release.run(["tool.bat", "build"], cwd=self.output)

        self.assertEqual(["tool.bat", "build"], run.call_args.args[0])

    def test_windows_resolution_keeps_a_path_with_spaces_intact(self) -> None:
        completed = subprocess.CompletedProcess([], 0, stdout="ok\n")
        spaced = self.output / "dir with space"
        spaced.mkdir(exist_ok=True)
        (spaced / "tool.bat").write_bytes(b"")

        with patch.object(release.sys, "platform", "win32"), patch.object(
            release.subprocess, "run", return_value=completed,
        ) as run:
            release.run(["tool.bat", "verify", "--print-certs", "app.apk"], cwd=spaced)

        # A single argv entry, so the space cannot split the executable path.
        self.assertEqual(
            [str(spaced / "tool.bat"), "verify", "--print-certs", "app.apk"],
            run.call_args.args[0],
        )

    def test_capture_returns_trimmed_stdout_for_a_batch_command(self) -> None:
        completed = subprocess.CompletedProcess([], 0, stdout="  gradle 9.8.1  \n")
        (self.output / "gradlew.bat").write_bytes(b"")

        with patch.object(release.sys, "platform", "win32"), patch.object(
            release.subprocess, "run", return_value=completed,
        ):
            self.assertEqual("gradle 9.8.1", release.run(["gradlew.bat", "--version"], cwd=self.output, capture=True))

    def test_a_failing_batch_command_raises_with_its_exit_code(self) -> None:
        (self.output / "gradlew.bat").write_bytes(b"")
        failed = subprocess.CalledProcessError(7, "gradlew.bat")

        with patch.object(release.sys, "platform", "win32"), patch.object(
            release.subprocess, "run", side_effect=failed,
        ):
            with self.assertRaises(subprocess.CalledProcessError) as raised:
                release.run(["gradlew.bat", "build"], cwd=self.output)

        self.assertEqual(7, raised.exception.returncode)

    def test_stable_requires_real_device_evidence_for_this_revision(self) -> None:
        with self.assertRaisesRegex(ValueError, "linux-evidence"):
            release.verify(self.output, None)
        evidence = json.loads(self.evidence.read_text())
        evidence["commit"] = "c" * 40
        self.evidence.write_text(json.dumps(evidence))
        with self.assertRaisesRegex(ValueError, "real-device"):
            release.verify(self.output, self.evidence)

    def test_setting_version_changes_only_product_source_and_changelog(self) -> None:
        manifest = self.output / "Cargo.toml"
        manifest.write_text('[workspace.package]\nversion = "1.2.3"\n\n[dependencies]\nserde = { version = "1" }\n')
        (self.output / "CHANGELOG.md").write_text('## Unreleased\n\nChanges\n')
        with patch.object(release, "ROOT", self.output):
            release.set_version("2.14.1")
            self.assertEqual("2.14.1", release.version())
        self.assertIn('serde = { version = "1" }', manifest.read_text())
        self.assertIn('## v2.14.1', (self.output / "CHANGELOG.md").read_text())

    def test_invalid_versions_do_not_change_the_source(self) -> None:
        manifest = self.output / "Cargo.toml"
        original = '[workspace.package]\nversion = "1.2.3"\n'
        manifest.write_text(original)
        with patch.object(release, "ROOT", self.output):
            for value in ("1.2", "1.2.3-rc1", "01.2.3", "2001.0.0", "1.1000.0", "0.0.0"):
                with self.assertRaises(ValueError):
                    release.set_version(value)
        self.assertEqual(original, manifest.read_text())


if __name__ == "__main__":
    unittest.main()

