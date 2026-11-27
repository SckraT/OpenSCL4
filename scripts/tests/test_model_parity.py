import json
import re
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
MATRIX_PATH = ROOT / "docs/model-parity-1.2.json"
REFERENCE_PATH = ROOT / "core/data/a3028-a3029-mode-reference.json"
Q20I_Q11I_LIFE2NEO_PATH = ROOT / "core/data/a3004-a3005-a3033-reference.json"


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

    def test_a3004_a3005_a3033_reference_frames(self):
        reference = json.loads(Q20I_Q11I_LIFE2NEO_PATH.read_text(encoding="utf-8"))
        self.assertEqual(set(reference["models"]), {"A3004", "A3005", "A3033"})
        expected_lengths = {"A3004": 61, "A3005": 55, "A3033": 34}
        for code, model in reference["models"].items():
            with self.subTest(model=code):
                offsets = model["stateBodyOffsets"]
                body = bytes.fromhex(model["stateBody"])
                self.assertEqual(len(body), expected_lengths[code])
                self.assertEqual(len(body), len(bytes.fromhex(model["inbound"]["initial"])) - 10)
                for name, hex_frame in model["inbound"].items():
                    frame = self.assert_frame(
                        hex_frame, bytes.fromhex("09 ff"), bytes.fromhex("01 01")
                    )
                    frame_body = frame[9:-1]
                    serial = frame_body[offsets["serial"]:offsets["serial"] + 16]
                    self.assertEqual(
                        serial, f"{code[1:]}TEST00000000".encode("ascii"), name
                    )
                    if name == "initial":
                        self.assertEqual(frame_body, body)
                if code == "A3004":
                    self.assertEqual(len(body), offsets["soundModes"] + 4 + 22)
                if code == "A3005":
                    self.assertEqual(len(body), offsets["autoPowerOff"] + 2 + 7)
                for name, hex_frame in model["outbound"].items():
                    self.assert_frame(hex_frame, bytes.fromhex("08 ee"), None)
        drc = reference["drc"]
        self.assert_frame(
            drc["outboundFrame"], bytes.fromhex("08 ee"), bytes.fromhex("02 83")
        )
        self.assertEqual(
            bytes.fromhex(drc["writeBody"]),
            bytes.fromhex(drc["outboundFrame"])[9:-1],
        )
        self.assertEqual(
            bytes.fromhex(reference["models"]["A3033"]["outbound"]["writeEqualizer"]),
            bytes.fromhex(
                "08 EE 00 00 00 02 81 14 00 FE FE 3C B4 8F A0 8E B4 74 88 E6"
            ),
            "the Life 2 Neo equalizer frame stays the legacy set_equalizer example",
        )

    def assert_frame(self, hex_frame, prefix, command):
        frame = bytes.fromhex(hex_frame)
        self.assertEqual(frame[:2], prefix)
        if command is not None:
            self.assertEqual(frame[5:7], command)
        self.assertEqual(int.from_bytes(frame[7:9], "little"), len(frame))
        self.assertEqual(frame[-1], sum(frame[:-1]) & 0xff)
        return frame
