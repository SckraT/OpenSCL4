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

## A3944, A3945, A3948 and A3959 state reference

[a3944-a3945-a3948-a3959-state-reference.json](a3944-a3945-a3948-a3959-state-reference.json)
records the derived bodies and the golden write frames of the four
`dualState`/`dualWithPeers` drivers. Each `01 01` body comes from a legacy driver
test example documented by that driver's state parser. The 16-byte serial at
body offsets 16–31 is replaced with the synthetic model-prefixed
`3944TEST00000000`, `3945TEST00000000`, `3948TEST00000000` or
`3959TEST00000000` value, and the additive frame checksum is recomputed. **No
physical capture of these models is recorded**; the virtual transport replays
the derived frames.

| Fixture                                                                 | Body | SHA-256                                                            |
| ----------------------------------------------------------------------- | ---- | ------------------------------------------------------------------ |
| [a3944-pilot-state.bin](a3944-pilot-state.bin)                           | 88   | `f4229f6f9449cecd8f483e985e6d41fc9b31e1697d05df1f606c96c1a383f64c` |
| [a3945-pilot-state.bin](a3945-pilot-state.bin)                           | 72   | `cb3956e551d585e79072980d9e8e67cd64be75ee225b67b76fb56161ca1ce91d` |
| [a3948-pilot-state.bin](a3948-pilot-state.bin)                           | 88   | `002c1f6bf007aefd1abcbb85bd1915a11bc3754c506afe247e3e9f52db38aa0e` |
| [a3959-pilot-state.bin](a3959-pilot-state.bin)                           | 91   | `7bb620e70a87b5ee10b56522e05ef6effd86109e5ff5d71437cd782db05cea64` |
| [a3959-gaming-unavailable-state.bin](a3959-gaming-unavailable-state.bin) | 91   | `078a978829b855c2b13356f725ff75925a2f9ba06e606b27326970c04e6ca63d` |

The family shares the serial, firmware and battery fields at offsets 16–31,
6–10, 11–15, 2 and 3. A3944, A3945 and A3948 read the five-step battery scale;
A3959 reads the ten-step scale at the same offsets. A3945 additionally reports
the case battery at offset 69 on the five-step scale. The equalizer preset id is
at offset 32–33 in every layout. A3944, A3948 and A3959 carry one ten-band curve
at 34–43; A3945 carries a second channel at 44–53. A3944's parser documents 73
bytes and ignores the example's trailing 15 bytes, while A3948's parser
documents the same trailing block. A3945 documents 72 bytes. The gaming-mode
flag is at A3945 offset 68 and A3959 offset 78. A3959 sound modes are at 64–70,
auto power off at 75–76, and the P30i firmware boundary for gaming mode is
`01.60`: the captured 01.59 body has no flag and the captured 01.64 body does.

The A3944 and A3948 writes use `02 83`, the A3959 write uses `02 83`, and the
A3945 two-channel write uses `02 81`. The DRC writes append the legacy
`apply_drc` copy of the curve; the A3944 Acoustic and A3959 legacy equalizer
bodies are reproduced byte for byte by the new driver. The A3959 `06 81`
ambient-mode write repeats the mode around the five preserved controls. Gaming
mode writes `01 87` with one byte, and auto power off writes `01 86` with the
enabled flag and the ten-minute-step duration index.

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
