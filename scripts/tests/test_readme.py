import re
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]

READMES = [
    ("README.md.in", "README.md", ("Model", "Name")),
    ("README.ru.md.in", "README.ru.md", ("Модель", "Название")),
]


class ReadmeTest(unittest.TestCase):
    def test_generated_readme_matches_template_and_device_catalog(self):
        source = (ROOT / "lib/src/devices/device_model.rs").read_text(encoding="utf-8")
        model_enum = source.split("pub enum DeviceModel {", 1)[1].split("\n}", 1)[0]
        models = re.findall(r"\bSoundcore(\w+),", model_enum)
        translations = (ROOT / "lib/i18n/en/openscq30-lib.ftl").read_text(encoding="utf-8")
        names = dict(re.findall(r"(?m)^soundcore-([a-z0-9]+) = (.+)$", translations))
        expected = {model: names[model.lower()] for model in models if model != "Development"}

        for template_name, readme_name, headers in READMES:
            with self.subTest(readme=readme_name):
                template = (ROOT / template_name).read_text(encoding="utf-8")
                readme = (ROOT / readme_name).read_text(encoding="utf-8")
                marker = "@SUPPORTED_DEVICES_TABLE@"
                self.assertEqual(template.count(marker), 1)
                before, _, after = template.partition(marker)
                self.assertTrue(
                    readme.startswith(before),
                    f"Regenerate {readme_name} from {template_name}",
                )
                self.assertTrue(
                    readme.endswith(after),
                    f"Regenerate {readme_name} from {template_name}",
                )
                table = readme[len(before):len(readme) - len(after)]
                rows = [line.split("|")[1:-1] for line in table.splitlines()]
                self.assertEqual([cell.strip() for cell in rows[0]], list(headers))
                actual = {cells[0].strip(): cells[1].strip() for cells in rows[2:]}
                self.assertEqual(len(actual), len(rows) - 2, "Duplicate device rows")
                self.assertEqual(actual, expected, "Regenerate the supported-device catalog")
