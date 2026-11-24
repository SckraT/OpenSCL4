#![forbid(unsafe_code)]

#[cfg(test)]
mod tests {
    use openscl4_protocol::{Direction, FrameDecoder};
    use serde::Deserialize;

    const CORPUS: &str = include_str!("../../../data/conformance.json");
    const FRAME_COUNT: usize = 503;
    const HEADER_LEN: usize = 9;
    const CHECKSUM_LEN: usize = 1;

    #[derive(Deserialize)]
    struct Corpus {
        count: usize,
        frames: Vec<Record>,
    }

    #[derive(Deserialize)]
    struct Record {
        label: String,
        frame_hex: String,
    }

    #[test]
    fn decodes_all_outbound_conformance_frames() {
        let corpus: Corpus = serde_json::from_str(CORPUS).unwrap_or_else(|error| {
            panic!(
                "invalid conformance corpus JSON at line {}, column {}",
                error.line(),
                error.column()
            )
        });
        assert_eq!(corpus.count, FRAME_COUNT, "incorrect declared frame count");
        assert_eq!(
            corpus.frames.len(),
            FRAME_COUNT,
            "incorrect actual frame count"
        );

        for (index, record) in corpus.frames.into_iter().enumerate() {
            let label = &record.label;
            let number = index + 1;
            let hex = record
                .frame_hex
                .split_ascii_whitespace()
                .collect::<String>();
            let bytes = hex::decode(hex)
                .unwrap_or_else(|_| panic!("record {number} ({label}): invalid hex encoding"));
            let mut decoder = FrameDecoder::default();
            decoder.push(&bytes);

            let frame = decoder
                .next_frame()
                .unwrap_or_else(|error| panic!("record {number} ({label}): {error}"))
                .unwrap_or_else(|| panic!("record {number} ({label}): incomplete frame"));
            assert_eq!(
                frame.direction,
                Direction::Outbound,
                "record {number} ({label}): incorrect direction"
            );
            assert_eq!(
                frame.body.len() + HEADER_LEN + CHECKSUM_LEN,
                bytes.len(),
                "record {number} ({label}): incorrect complete-frame length"
            );
            assert!(
                frame.command == bytes[5..7],
                "record {number} ({label}): command mismatch"
            );
            assert!(
                frame.body.as_ref() == &bytes[HEADER_LEN..bytes.len() - CHECKSUM_LEN],
                "record {number} ({label}): body mismatch"
            );
        }
    }
}
