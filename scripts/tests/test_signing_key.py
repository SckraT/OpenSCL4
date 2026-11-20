import importlib.util
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
PROJECT_DIR = ROOT / "android" / "app"


def load_signing_key_script():
    """The script name contains a hyphen, so it is loaded by path rather than imported by name."""
    spec = importlib.util.spec_from_file_location("create_signing_key", ROOT / "scripts" / "create-signing-key.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


SIGNING_KEY = load_signing_key_script()


class StoreFileValueTest(unittest.TestCase):
    def test_a_key_inside_the_repository_is_relative_to_the_app_project(self):
        keystore = ROOT / ".local" / "signing" / "openscl4.p12"

        self.assertEqual(
            "../../.local/signing/openscl4.p12",
            SIGNING_KEY.store_file_value(keystore, PROJECT_DIR, ROOT),
        )

    def test_a_key_outside_the_repository_keeps_its_absolute_path(self):
        keystore = Path("/keys/openscl4.p12")

        self.assertEqual(keystore.as_posix(), SIGNING_KEY.store_file_value(keystore, PROJECT_DIR, ROOT))


if __name__ == "__main__":
    unittest.main()