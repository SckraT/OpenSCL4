import json
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
                "applicationId": release.APP_ID,
                "version": product_version,
                "commit": "a" * 40,
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

    def test_missing_platform_or_android_abi_is_rejected(self) -> None:
        self.reports[0][0].unlink()
        with self.assertRaisesRegex(ValueError, "complete"):
            release.verify(self.output, self.evidence)

    def test_mixed_source_revisions_are_rejected(self) -> None:
        self.reports[-1][1]["commit"] = "c" * 40
        self.save_reports()
        with self.assertRaisesRegex(ValueError, "one version, commit"):
            release.verify(self.output, self.evidence)

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
