# Soundcore framing corpus

[conformance.json](conformance.json) contains 503 complete outbound Soundcore frames for framing tests.

## Provenance

The input was `core/data/conformance.json` from SoundcoreNext `v2.0.0`, commit
`c1bf3e6881d2806972bf1fbe7f52fd5589997462`. The source file SHA-256 is
`269ded36ec86eecdc8f2768f97ce27e1c2efaa490ae030ca750bb37f30b38fbf`. It contains 500 settings-builder frames
labelled `settings-frames.csv` and three measured A3062 sound-mode frames labelled
`a3062-anc-modes.json`. [Source inventory](https://github.com/SckraT/OpenSCL4/issues/135#issuecomment-7132811721)

Copyright (C) 2026 Soundcore research contributors. Licensed under GPL-3.0-or-later
under the terms of [the repository license](../../LICENSE.txt).

## Privacy review

Each record was checked against its reviewed source label, command and payload width.
The 500 settings-builder payloads carry one to four bytes of settings data. A raw
six-byte Bluetooth address or 16-byte Soundcore serial cannot fit in those fields.
The `0B 84` command in the corpus carries a one-byte enable flag. The related
commands that take a Bluetooth address are absent. [MAC-bearing command layouts](../../lib/src/devices/soundcore/common/packet/outbound/dual_connections.rs) · [Serial number layout](../../lib/src/devices/soundcore/common/structures/serial_number.rs)

The three six-byte payloads are `06 81` sound-mode selections. All six bytes match
the measured A3062 command builder at the source revision. The mode byte matches
the label, and the remaining fields encode sound-mode controls. [A3062 layout](../../lib/src/devices/soundcore/a3062/structures.rs)

The export keeps the reviewed record labels, basenames and frame bytes. Its
top-level source and selection strings were replaced with a revision reference
and a description of the framing scope.

## Integrity check

All 503 frames are unique and retain their original order. They span 36 labels,
26 commands and eight command groups. Each little-endian declared length matches
the frame size and each additive checksum matches the preceding bytes. The
SHA-256 of the concatenated decoded frames in file order is
`475919f1ed95f162cd722244919311aa54c6ad787fef2e19601ca643e50b31d6`.

## CI verification

The protocol integration test decodes all 503 records and runs in the Windows
and Linux CI jobs. Run it locally with

```sh
cargo test --locked -p openscl4-protocol --test conformance
```

A failing record is identified by its index and reviewed label. The full corpus
run is part of first-wave acceptance. [First-wave acceptance criteria](https://github.com/SckraT/OpenSCL4/issues/136#issuecomment-7133141246)

## A3954 pilot state

[a3954-pilot-state.bin](a3954-pilot-state.bin) is the 175-byte inbound `01 01`
frame from `core/data/a3954-state.json` at the SoundcoreNext revision above.
The source JSON SHA-256 is `a58f290aedfc9e06de4bec1d63c5d3ac7bdaf4e84ed6724ea2b1bbe509379947`.
The 16-byte device serial and six-byte case serial were masked again at body
offsets 16 and 38. The frame checksum was recalculated. The resulting frame
SHA-256 is `6025c240c672fb296e50b5e84d8bc18f14aa1dcf6809f030bbd6bb447ebad037`.
The virtual transport supplies a synthetic identity response with the A3954
model prefix.

The physical A3954 reports a 31-byte `01 05` identity body. It contains two
five-byte earbud firmware versions, a 16-byte serial beginning with `3954`,
and a five-byte case firmware version. The model tests use a synthetic serial.

The Windows hardware pilot confirmed four-byte `06 81` writes. The body is mode,
position, airplane mode and wind-noise suppression. Normal uses position 6
(`02 06 00 01` with wind-noise suppression enabled), and the pilot's
noise-canceling setting uses position 11 (`00 0B 00 01`). The SoundcoreNext
fixture appended `FF`. With that fifth byte, the hardware run reported the
previous mode after the write. [Windows hardware run](https://github.com/SckraT/OpenSCL4/issues/154)
[SoundcoreNext measurements](https://github.com/SckraT/SoundcoreNext/blob/c1bf3e6881d2806972bf1fbe7f52fd5589997462/core/data/a3954-sound-modes.json)

## A3062 pilot state

[a3062-pilot-state.bin](a3062-pilot-state.bin) contains the 105-byte inbound
`01 01` frame from `core/data/a3062-state.json` at the SoundcoreNext revision
above. The source JSON SHA-256 is
`f4a9c23808cad91278a0a8bf85282159b7f93ad3d236c84736638ba068e33ed9`.

The original capture had already masked the serial at body offsets 7–22. This
fixture substitutes a synthetic serial with the model prefix at the same offsets
and recalculates the checksum. The resulting frame SHA-256 is
`1624f62a147f7bf480b560bb5866b084b4c37afca6bbddadaad3225b70ad5ecb`.
The fixture exercises the single battery at offset 0 (raw step 2, displayed as
30% using the existing A3062 scale), firmware 04.39 at offsets 2–6 and normal mode
at offset 69. The payload carries no multipoint peer addresses.
[Source capture](https://github.com/SckraT/SoundcoreNext/blob/c1bf3e6881d2806972bf1fbe7f52fd5589997462/core/data/a3062-state.json)

## A3062 sound-mode read-back

The [noise canceling](a3062-mode-noise-canceling.bin),
[transparency](a3062-mode-transparency.bin), and [normal](a3062-mode-normal.bin)
fixtures are independent 105-byte `01 01` responses captured over RFCOMM on
Space One Pro firmware 04.39 after the corresponding `06 81` writes. The source
JSON SHA-256 is
`f02ca21f9497ad324d5f633efa2671b37cff827ed82ed265ea6de541d78e89e8`.
[Source capture](https://github.com/SckraT/SoundcoreNext/blob/c1bf3e6881d2806972bf1fbe7f52fd5589997462/core/data/a3062-sound-modes.json)

The fixtures replace the already masked serial at body offsets 7–22 with the
synthetic A3062 serial and recalculate the checksum. Concatenated in the order
shown above, their SHA-256 is
`63bf74ff20775569dbd8bc827fdf477c3639a60f7c457adb338b4ccb8b53e205`.
When the virtual device accepts a matching `06 81` body, it copies the mode and
its controls from the corresponding capture into the current state. State reads
keep the device's current EQ curve and other settings.
[Sound-mode commands](conformance.json)

## A3062 equalizer write and read-back

These frames come from the Windows RFCOMM capture on A3062 firmware 04.39 in
`core/data/a3062-eq-custom.json` at the SoundcoreNext revision above. The source
JSON SHA-256 is `0b771d93b50d0a933f24c553bc7ed708128dd7c08ad1cc660d76d7d0386eacc3`.
[Source capture](https://github.com/SckraT/SoundcoreNext/blob/c1bf3e6881d2806972bf1fbe7f52fd5589997462/core/data/a3062-eq-custom.json)

| Fixture                                    | Source field                                | SHA-256                                                            |
| ------------------------------------------ | ------------------------------------------- | ------------------------------------------------------------------ |
| [a3062-eq-write.bin](a3062-eq-write.bin)   | `frame_hex`                                 | `838f3fea1ce46e7912266210f3a1df27aa6144e5e2d24ffe8ede895fdf0ff93d` |
| [a3062-eq-before.bin](a3062-eq-before.bin) | `before_frame_hex` and `restored_frame_hex` | `2b14e731828a86feb817389e911ca12298746257a3dce250877f44205abda775` |
| [a3062-eq-custom.bin](a3062-eq-custom.bin) | `custom_frame_hex`                          | `132a798f9ae46431fc0a2ff86748ddd5bdf8a63f2fe674f7712cd603f168f97c` |

The two `01 01` fixtures substitute a synthetic serial at body offsets 7–22
and recalculate the checksum. The before and restored frames have identical
bytes. The outbound `03 87` frame contains EQ and HearID settings.

The session replay compares the custom write to the captured outbound frame
and confirms both curves from the captured state replies.

## A3028 and A3029 state reference

[a3028-a3029-mode-reference.json](a3028-a3029-mode-reference.json) records one legacy
A3028 driver test whose state carries an A3029 serial prefix. Its `01 01` state
body is 60 bytes: a single battery at 0, the eight-band equalizer at 4–11, the
sound modes at 35–38, firmware `02.30` at 39–43 and a 16-byte serial at 44–59.
The reference replaces the full serial with the synthetic model-specific
`3028TEST00000000` or `3029TEST00000000` value and recomputes the checksum, so
the two published variants exercise the same layout.

The `06 81` cycle preserves the other sound-mode bytes: Normal uses
`02 00 01 00` and ANC Transport uses `00 00 01 00`. The optional 7-byte extra
fields block at body 60–66 (unknown1, touch control, dual connections, auto power
off, and the two prompt tones) is documented by
[the legacy parser](../../lib/src/devices/soundcore/a3028/packets/state_update_packet.rs)
but **no capture of it is recorded**. The 67-byte state used by the session tests
is reconstructed from that legacy layout, not measured over RFCOMM: the tests
append `0x00 0x00 0x00 0x01 0x01 0x01 0x01` to the same body and use it as a
`protocolFixture`. A body without the block reports auto power off as
unavailable. An unrecognized flag or a duration index above 3 is reported as
unavailable, not as a confirmed disabled value.

The eight-band write bodies are the fixed examples from the legacy
`set_equalizer` tests: the custom curve is
`08 EE 00 00 00 02 81 14 00 FE FE 3C B4 8F A0 8E B4 74 88 E6` and the
signature curve is
`08 EE 00 00 00 02 81 14 00 00 00 78 78 78 78 78 78 78 78 4D`.
**No physical A3028 or A3029 capture exists** for these sessions; the virtual
transport replays the reference bodies with their checksums recomputed.

## Separate-identity TWS reference

The A3909, A3926, A3930 and A3951 drivers are the `separateIdentity` family: they
read the state with `01 01` and read the two firmware versions and the serial with
a separate `01 05` response. The state body carries no serial, so the new session
selects the model from the identity response only and keeps the firmware versions
from that same response.

The **A3909** identity and state frames come from the legacy driver test
`with_packet_from_issue_264` in
[`lib/src/devices/soundcore/a3909.rs`](../../lib/src/devices/soundcore/a3909.rs),
added with the driver in commit `f47d0a77` and referring to the upstream issue 264
capture. The test registers the driver with a different `DeviceModel` label, but
the captured `01 05` serial carries the 3909 model prefix, so the frames are used
as A3909. The test's `01 05` body is `01.2801.28` followed by a real serial
number; [a3909-identity.bin](a3909-identity.bin) replaces the serial with the
synthetic `3909TEST00000000` value and recomputes the checksum.
[a3909-state.bin](a3909-state.bin) keeps the captured 36-byte body and recomputes
the checksum. The legacy parser consumes 35 bytes and leaves the last byte unread;
the fixture keeps it and the session accepts both lengths. The body contains no
serial, MAC or peer data that would need masking.

**No physical A3926, A3930 or A3951 capture exists.** Their identity and state
frames are derived from the documented layout of the legacy parsers and
serializers at the same revision. The state body is TwsStatus (2), DualBattery
(4), `CommonEqualizerConfiguration<2, 8>` (18), Gender (1), AgeRange (1), then the
model block: the basic hear-id block (21) for the A3926 or the custom hear-id
block (39) for the A3930 and A3951, `ButtonStatusCollection<6>` (12), and for the
A3930 and A3951 SoundModes (4) and the side tone (1), plus wearing detection (1)
and touch tone (1) for the A3951. The derived bodies were checked by parsing them with the
legacy parsers and every value except the layout is a synthetic example. The firmware strings
are synthetic (`01.60`, `01.70`, `01.80`), and the serials are
`3926TEST00000000`, `3930TEST00000000` and `3951TEST00000000`. Checksums are the
documented additive checksums.

| Fixture                                  | Length | SHA-256                                                            |
| ---------------------------------------- | ------ | ------------------------------------------------------------------ |
| [a3909-identity.bin](a3909-identity.bin) | 36     | `97d4e26800e17451da1ad6ce8f41df6b747ca483f0da96fa75862cde852f177a` |
| [a3909-state.bin](a3909-state.bin)       | 46     | `66997d55b6d3d5f6259ec888cd979b93b1121148fcc2eb174f5c40e271f7086a` |
| [a3926-identity.bin](a3926-identity.bin) | 36     | `d8b1f149f556ca29d92a70050a93d24070a3cdf32500a9957e434d05dbe1328e` |
| [a3926-state.bin](a3926-state.bin)       | 69     | `87b353c356a96054b6b2a8d4e23bb9073a341f702c7406dcbdcf0137c1b16088` |
| [a3930-identity.bin](a3930-identity.bin) | 36     | `35dee0de0cc3d6e0d734bd43906064d0799b4039d1952f3f3883bb4dbba2cfd4` |
| [a3930-state.bin](a3930-state.bin)       | 92     | `4c942577b64192db70c2e8ae518fa332e9bddc3a904586225329ead6ab1a43ab` |
| [a3930-ldac.bin](a3930-ldac.bin)         | 11     | `9ba4f5f99489540db90fd1ef482bdf1e34e2956a0ef89596cb0b6c2c0f071d15` |
| [a3951-identity.bin](a3951-identity.bin) | 36     | `36e4a32bebc2377d71303dababf2a2e76f854283d7d0759016dd7f3cd89b683e` |
| [a3951-state.bin](a3951-state.bin)       | 94     | `8c5f1a0874cbc571263e85c3793c3f82c7d6e731bb7a5c27d692d3a7eedb61dd` |
| [a3951-ldac.bin](a3951-ldac.bin)         | 11     | `d660ad97957993b54ac99d654d17977abfcdc27d3f170c5ed57dd24a35d68d0d` |

The LDAC fixtures are the `01 7F` replies the A3930 and A3951 drivers request
after the state: the A3930 fixture reports the flag off and the A3951 fixture
reports it on. The A3909 and A3926 drivers do not read LDAC.

The equalizer writes are derived from the legacy builders, not from a capture:

- The A3926 basic hear-id variant writes the preset, the two eight-band channels,
  the gender, the age range and the basic hear-id volume and time. The legacy
  `EqualizerWithBasicHearIdStateModifier` adapts the basic block into the shared
  `SetEqualizerAndCustomHearId` builder, which is the source of the field order.
- The A3930 and A3951 custom hear-id variant writes the preset, the favorite music
  genre, the two channels, the gender, the age range, the custom hear-id volume,
  time, type and custom channel, and ends with the active compressed curve.
- Both variants select `03 86` instead of `03 87` and replace the hear-id blocks
  with absent markers when the state reports the age range `0xFF`.
- The trailing active curve is the legacy dynamic-range-compression output, so the
  models crate carries that algorithm. The legacy test
  `set_equalizer_and_custom_hear_id.rs::it_matches_a_generated_packet_with_hear_id_set`
  fixes a full `03 87` frame, and the models test
  `reproduces_the_legacy_golden_equalizer_body` rebuilds that body from a state
  with the same hear-id fields. The state carries the genre as one byte, so the
  published two-byte genre `FE FE` becomes `FE 00` in a write built from a state.
- The `06 81` sound-mode write repeats the state's noise-canceling, transparency
  and custom fields and changes only the ambient byte. The A3930 driver offers the
  normal and transparency ambient modes, the A3951 also offers noise canceling.
- The A3930 driver explicitly selects the trailing checksum. The modular core
  frames every model with that trailing checksum, so the fixture and identity
  frames keep it without a separate mode.

The session replay compares each outgoing write with the published body and
confirms the result from a fresh `01 01` reply. The A3909 equalizer is not ported:
its state carries only the preset id and the legacy driver infers the curve from a
fixed preset table, so a curve write cannot be confirmed by read-back.

## A3949 pilot state

[a3949-pilot-state.bin](a3949-pilot-state.bin) is a 98-byte `01 01` state response
captured from a P20i. The source JSON SHA-256 is
`e69afee67c36b283420fda162dc0d5b877bf6dd7d188c1ed0336c037e0914af8`.
[Source capture](https://github.com/SckraT/SoundcoreNext/blob/c1bf3e6881d2806972bf1fbe7f52fd5589997462/core/data/a3949-state.json)

The fixture replaces the masked serial at body offsets 16–31 with a synthetic
serial with the A3949 model prefix and recalculates the checksum. Its SHA-256 is
`26b4102a741f34f9bf2ffa85b312231aa45ac3b6c13dff9268303ff592cf64b3`.
The left and right levels are 5 and 4 on the five-step scale at body offsets 2
and 3. Both firmware versions are `14.43` at body offsets 6–10 and 11–15.

## A3949 gaming-mode read-back

[a3949-gaming-off.bin](a3949-gaming-off.bin) and
[a3949-gaming-on.bin](a3949-gaming-on.bin) are 98-byte inbound `01 01` responses
captured over Windows RFCOMM from the paired Soundcore P20i (A3949, firmware
`14.43` on both earbuds) on 2026-11-26. The off frame is the state before and after
the cycle; the on frame followed the measured write
`08 EE 00 00 00 01 87 0B 00 01 8A`, and the mode returned with
`08 EE 00 00 00 01 87 0B 00 00 89`.
[Measurement ticket](https://github.com/SckraT/OpenSCL4/issues/174)

The capture substituted the device serial at body offsets 16–31 with the synthetic
`3949TEST00000000` serial and recalculated the checksums. The frames differ only in
the gaming flag at body offset 71 (`00` off, `01` on) and the trailing checksum
(`D0` off, `D1` on). The SHA-256 of the concatenated off and on frames in that
order is
`fd7324ef5cf2df5203074b59384e9c5a24deae33bc967114082d934294331577`.

The measurement separated the `01 87` acknowledgements from the state responses it
counted. The device kept reporting the previous mode for four state reads before
the on write appeared, 1022 ms after the write, and for five reads before the off
write appeared, 1247 ms after the write. A session test that confirms A3949
gaming-mode writes by read-back must hold its window past both delays.
