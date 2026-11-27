import hashlib
import json
import re
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
MATRIX_PATH = ROOT / "docs/model-parity-1.2.json"
REFERENCE_PATH = ROOT / "core/data/a3028-a3029-mode-reference.json"
D1301_REFERENCE_PATH = ROOT / "core/data/d1301-sleep-reference.json"


class ModelParityTest(unittest.TestCase):
    def assert_source_exists(self, reference):
        path, _, symbol = reference.partition("::")
        source = ROOT / path
        self.assertTrue(source.is_file(), reference)
        if symbol:
            self.assertIn(symbol, source.read_text(encoding="utf-8"), reference)

    def test_every_public_model_resolves_its_operations(self):
        matrix = json.loads(MATRIX_PATH.read_text(encoding="utf-8"))
        source = (ROOT / matrix["source"]).read_text(encoding="utf-8")
        enum = source.split("pub enum DeviceModel {", 1)[1].split("\n}", 1)[0]
        codes = set(re.findall(r"\bSoundcore([AD]\d+\w*),", enum))
        models = {entry["code"]: entry for entry in matrix["models"]}
        self.assertEqual(len(models), len(matrix["models"]), "Duplicate model code")
        self.assertEqual(set(models), codes)

        startup = matrix["startupOperations"]
        modules = matrix["moduleOperations"]
        for name, operation in startup.items():
            with self.subTest(startup=name):
                self.assert_source_exists(operation["outbound"])
                if operation["inbound"] != "model.stateParserSource":
                    self.assert_source_exists(operation["inbound"])
        self.assertEqual(
            set(modules), {name for model in models.values() for name in model["modules"]}
        )
        for code, model in models.items():
            with self.subTest(model=code):
                self.assert_source_exists(model["legacyDriver"])
                self.assert_source_exists(model["stateParserSource"])
                reads = set(model["startup"]) | set(model.get("conditionalStartup", {}))
                self.assertLessEqual(reads, set(startup))
                overrides = model.get("operationSources", {})
                self.assertLessEqual(set(overrides), set(model["modules"]))
                for name in model["modules"]:
                    operation = modules[name] | overrides.get(name, {})
                    self.assertTrue(reads.intersection(operation["outboundRead"]), name)
                    inbound = operation["inboundState"]
                    if inbound.startswith("model."):
                        inbound = model[inbound.removeprefix("model.")]
                    self.assert_source_exists(inbound)
        for name, operation in modules.items():
            with self.subTest(operation=name):
                self.assertLessEqual(set(operation["outboundRead"]), set(startup))
                for field in ("registration", "availability", "inboundState"):
                    if not operation[field].startswith("model."):
                        self.assert_source_exists(operation[field])
                for reference in operation["outboundWrite"] + operation.get("inboundUpdates", []):
                    self.assert_source_exists(reference)
                self.assertTrue(operation["evidence"])
                self.assertTrue(operation["result"])
        for variant in matrix["operationVariants"]:
            with self.subTest(variant=variant["operation"], models=variant["models"]):
                for code in variant["models"]:
                    self.assertIn(variant["operation"], models[code]["modules"])
                for field in ("inbound", "outbound", "availabilitySource", "reference"):
                    if field in variant:
                        self.assert_source_exists(variant[field])
                self.assertTrue(variant["evidence"])
                self.assertTrue(variant["result"])

    def test_a3028_a3029_reference_frames(self):
        reference = json.loads(REFERENCE_PATH.read_text(encoding="utf-8"))
        outgoing = reference["outbound"]
        for name, command in (
            ("readState", bytes.fromhex("01 01")),
            ("writeNoiseCanceling", bytes.fromhex("06 81")),
            ("restoreNormal", bytes.fromhex("06 81")),
        ):
            with self.subTest(command=name):
                self.assert_frame(outgoing[name], bytes.fromhex("08 ee"), command)
        serial_offset = reference["stateBodyOffsets"]["serial"]
        mode_offset = reference["stateBodyOffsets"]["soundModes"]
        for code, replies in reference["inbound"].items():
            with self.subTest(model=code):
                expected_serial = f"{code[1:]}TEST00000000".encode("ascii")
                states = []
                for reply in ("initial", "noiseCanceling"):
                    frame = self.assert_frame(
                        replies[reply], bytes.fromhex("09 ff"), bytes.fromhex("01 01")
                    )
                    body = frame[9:-1]
                    self.assertEqual(body[serial_offset:serial_offset + 16], expected_serial)
                    states.append(body)
                initial, noise_canceling = states
                self.assertEqual(initial[mode_offset], 2)
                self.assertEqual(noise_canceling[mode_offset], 0)
                self.assertEqual(initial[:mode_offset], noise_canceling[:mode_offset])
                self.assertEqual(initial[mode_offset + 1:], noise_canceling[mode_offset + 1:])
        self.assertEqual(set(reference["inbound"]), {"A3028", "A3029"})

    def assert_frame(self, hex_frame, prefix, command):
        frame = bytes.fromhex(hex_frame)
        self.assertEqual(frame[:2], prefix)
        self.assertEqual(frame[5:7], command)
        self.assertEqual(int.from_bytes(frame[7:9], "little"), len(frame))
        self.assertEqual(frame[-1], sum(frame[:-1]) & 0xff)
        return frame

    def test_d1301_sleep_reference_frames(self):
        reference = json.loads(D1301_REFERENCE_PATH.read_text(encoding="utf-8"))
        self.assertEqual(reference["sourceModelPrefix"], "1301")
        offsets = reference["stateBodyOffsets"]
        self.assertEqual(offsets["serial"], 14)
        self.assertEqual(offsets["postSleepAudio"], 144)
        self.assertEqual(offsets["autoSwitchOnceAsleep"], 148)

        commands = {
            "postSleepKeep": bytes.fromhex("01 01"),
            "postSleepPause": bytes.fromhex("01 01"),
            "postSleepPlayLocal": bytes.fromhex("01 01"),
            "autoStopTimer": bytes.fromhex("15 03"),
            "alarmsEmpty": bytes.fromhex("14 01"),
            "alarmsFilled": bytes.fromhex("14 01"),
            "alarmsChanged": bytes.fromhex("14 01"),
        }
        frames = {}
        for name, expected in commands.items():
            entry = reference["inbound"][name]
            path = ROOT / "core/data" / entry["file"]
            self.assertTrue(path.is_file(), entry["file"])
            self.assertEqual(
                hashlib.sha256(path.read_bytes()).hexdigest(),
                entry["sha256"],
                entry["file"],
            )
            frame = path.read_bytes()
            frames[name] = self.assert_frame(frame.hex(" "), bytes.fromhex("09 ff"), expected)

        for name, expected in (
            ("postSleepKeep", (1, 0)),
            ("postSleepPause", (0, 1)),
            ("postSleepPlayLocal", (1, 1)),
        ):
            body = frames[name][9:-1]
            self.assertEqual(len(body), 150, name)
            self.assertEqual(
                body[offsets["serial"]:offsets["serial"] + 16],
                b"1301TEST00000000",
                name,
            )
            self.assertEqual(
                (
                    body[offsets["postSleepAudio"]],
                    body[offsets["autoSwitchOnceAsleep"]],
                ),
                expected,
                name,
            )
            self.assertEqual(body[offsets["firmware"]:offsets["firmware"] + 5], b"01.91")

        timer = frames["autoStopTimer"][9:-1]
        self.assertEqual(timer, bytes.fromhex("00 1e 00 01 3c 00 00 00"))
        self.assertEqual(len(frames["alarmsEmpty"][9:-1]), 0)
        filled = frames["alarmsFilled"][9:-1]
        changed = frames["alarmsChanged"][9:-1]
        self.assertEqual(len(filled), 16)
        self.assertEqual(len(changed), 16)
        self.assertEqual(filled[8:], changed[8:], "only the first alarm changes")
        self.assertNotEqual(filled[:8], changed[:8])
        self.assertEqual(len(reference["alarmRecords"]["filled"]), 2)
        self.assertEqual(len(reference["alarmRecords"]["changed"]), 2)

        captured = {
            "setAlarm": bytes.fromhex("14 81"),
            "updateAlarm": bytes.fromhex("14 81"),
            "deleteAlarm": bytes.fromhex("14 82"),
        }
        for name, command in captured.items():
            self.assert_frame(reference["outbound"][name], bytes.fromhex("08 ee"), command)

    def test_d1301_core_coverage_uses_the_published_fixtures(self):
        matrix = json.loads(MATRIX_PATH.read_text(encoding="utf-8"))
        model = next(entry for entry in matrix["models"] if entry["code"] == "D1301")
        coverage = {entry["operation"]: entry for entry in model["coreCoverage"]}
        for operation in (
            "identifyFromState",
            "readState",
            "readAutoStopTimer",
            "readAlarms",
            "writePostSleepAudio",
            "writeAutoSwitchOnceAsleep",
            "writeListeningMode",
            "writeDefaultListeningMode",
            "writeNoiseCanceling",
        ):
            with self.subTest(operation=operation):
                self.assertIn(operation, coverage)
        for entry in coverage.values():
            for field in ("inbound", "outbound", "test"):
                reference = entry.get(field)
                if reference:
                    self.assert_source_exists(reference)
        self.assertIn("d1301-sleep-reference", model["variants"])
        self.assertEqual(model["reference"], "core/data/d1301-sleep-reference.json")
