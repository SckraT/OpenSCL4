import json
import re
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
MATRIX_PATH = ROOT / "docs/model-parity-1.2.json"
REFERENCE_PATH = ROOT / "core/data/a3028-a3029-mode-reference.json"
A3931_REFERENCE_PATH = ROOT / "core/data/a3931-a3935-state-reference.json"
A3933_REFERENCE_PATH = ROOT / "core/data/a3933-a3939-state-reference.json"


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

    def test_a3931_a3935_reference_frames(self):
        reference = json.loads(A3931_REFERENCE_PATH.read_text(encoding="utf-8"))
        outgoing = reference["outbound"]
        for name, command in (
            ("readState", bytes.fromhex("01 01")),
            ("readIdentity", bytes.fromhex("01 05")),
            ("writeNoiseCanceling", bytes.fromhex("06 81")),
            ("restoreNormal", bytes.fromhex("06 81")),
            ("writeCustomEqualizer", bytes.fromhex("02 83")),
            ("writeSignatureEqualizer", bytes.fromhex("02 83")),
            ("writeAutoPowerOffDisabled", bytes.fromhex("01 86")),
            ("writeAutoPowerOffEnabledDuration3", bytes.fromhex("01 86")),
        ):
            with self.subTest(command=name):
                self.assert_frame(outgoing[name], bytes.fromhex("08 ee"), command)

        offsets = reference["stateBodyOffsets"]
        state = self.assert_frame(
            reference["inbound"]["state"], bytes.fromhex("09 ff"), bytes.fromhex("01 01")
        )
        body = state[9:-1]
        self.assertEqual(len(body), reference["stateLength"])
        self.assertEqual(
            body[offsets["equalizerPreset"]:offsets["equalizerPreset"] + 2],
            bytes.fromhex("00 00"),
        )
        self.assertEqual(
            body[offsets["soundModes"]:offsets["soundModes"] + 4],
            bytes.fromhex("00 00 01 00"),
        )
        self.assertEqual(
            body[offsets["autoPowerOff"]:offsets["autoPowerOff"] + 2],
            bytes.fromhex("00 00"),
        )

        equalizer = self.assert_frame(
            outgoing["writeCustomEqualizer"], bytes.fromhex("08 ee"), bytes.fromhex("02 83")
        )
        equalizer_body = equalizer[9:-1]
        self.assertEqual(len(equalizer_body), 34)
        self.assertEqual(equalizer_body[0:2], bytes.fromhex("fe fe"))
        self.assertEqual(equalizer_body[2:10], equalizer_body[10:18])
        self.assertEqual(equalizer_body[18:26], equalizer_body[26:34])

        serials = {}
        for code, identity_hex in reference["inbound"]["identities"].items():
            identity = self.assert_frame(
                identity_hex, bytes.fromhex("09 ff"), bytes.fromhex("01 05")
            )
            identity_body = identity[9:-1]
            self.assertEqual(len(identity_body), 26)
            self.assertEqual(identity_body[:10], b"00.0000.00")
            serial = identity_body[10:26]
            serials[code] = serial
            self.assertEqual(serial, f"{code[1:]}TEST00000000".encode("ascii"))
        self.assertEqual(set(serials), {"A3931", "A3935"})

    def test_a3933_a3939_reference_frames(self):
        reference = json.loads(A3933_REFERENCE_PATH.read_text(encoding="utf-8"))
        outgoing = reference["outbound"]
        for name, command in (
            ("readState", bytes.fromhex("01 01")),
            ("writeNoiseCanceling", bytes.fromhex("06 81")),
            ("restoreNormal", bytes.fromhex("06 81")),
            ("writeCustomEqualizer", bytes.fromhex("02 81")),
            ("writeTrebleReducerEqualizer", bytes.fromhex("02 81")),
            ("writeGamingModeOff", bytes.fromhex("01 87")),
            ("writeGamingModeOn", bytes.fromhex("01 87")),
        ):
            with self.subTest(command=name):
                self.assert_frame(outgoing[name], bytes.fromhex("08 ee"), command)

        offsets = reference["stateBodyOffsets"]
        self.assertEqual(set(reference["inbound"]), {"A3933", "A3939"})
        for code, state_hex in reference["inbound"].items():
            with self.subTest(model=code):
                expected_serial = f"{code[1:]}TEST00000000".encode("ascii")
                state = self.assert_frame(
                    state_hex, bytes.fromhex("09 ff"), bytes.fromhex("01 01")
                )
                body = state[9:-1]
                self.assertEqual(len(body), reference["stateLength"])
                self.assertEqual(
                    body[offsets["serial"]:offsets["serial"] + 16], expected_serial
                )
                self.assertEqual(
                    body[offsets["firmware"]:offsets["firmware"] + 10], b"00.0000.00"
                )
                self.assertEqual(
                    body[offsets["soundModes"]:offsets["soundModes"] + 4],
                    bytes.fromhex("00 00 01 00"),
                )
                self.assertEqual(body[offsets["gamingMode"]], 0)

        equalizer = self.assert_frame(
            outgoing["writeCustomEqualizer"], bytes.fromhex("08 ee"), bytes.fromhex("02 81")
        )
        equalizer_body = equalizer[9:-1]
        self.assertEqual(len(equalizer_body), 22)
        self.assertEqual(equalizer_body[0:2], bytes.fromhex("fe fe"))
        self.assertEqual(equalizer_body[2:12], equalizer_body[12:22])

        for name, enabled in (("writeGamingModeOff", 0), ("writeGamingModeOn", 1)):
            frame = self.assert_frame(
                outgoing[name], bytes.fromhex("08 ee"), bytes.fromhex("01 87")
            )
            self.assertEqual(frame[9:-1], bytes([enabled]))

    def assert_frame(self, hex_frame, prefix, command):
        frame = bytes.fromhex(hex_frame)
        self.assertEqual(frame[:2], prefix)
        self.assertEqual(frame[5:7], command)
        self.assertEqual(int.from_bytes(frame[7:9], "little"), len(frame))
        self.assertEqual(frame[-1], sum(frame[:-1]) & 0xff)
        return frame
