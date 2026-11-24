#![forbid(unsafe_code)]

#[cfg(test)]
mod tests {
    use openscl4_protocol::{DecodeError, Direction, Frame, FrameDecoder};

    const INBOUND: [u8; 11] = [
        0x09, 0xff, 0x00, 0x00, 0x01, 0x01, 0x03, 0x0b, 0x00, 0x2a, 0x42,
    ];
    const OUTBOUND: [u8; 10] = [0x08, 0xee, 0x00, 0x00, 0x00, 0x01, 0x01, 0x0a, 0x00, 0x02];

    #[test]
    fn assembles_a_frame_across_multiple_reads() {
        let mut decoder = FrameDecoder::default();

        for chunk in [&INBOUND[..4], &INBOUND[4..9], &INBOUND[9..10]] {
            decoder.push(chunk);
            assert!(decoder.next_frame().expect("valid partial frame").is_none());
        }

        decoder.push(&INBOUND[10..]);
        let frame = read_frame(&mut decoder);
        assert_eq!(frame.direction, Direction::Inbound);
        assert_eq!(frame.command, [0x01, 0x03]);
        assert_eq!(frame.body.as_ref(), &[0x2a]);
        assert!(decoder.next_frame().expect("end of stream").is_none());
    }

    #[test]
    fn returns_frames_from_one_read_in_wire_order() {
        let mut decoder = FrameDecoder::default();
        decoder.push(&[OUTBOUND.as_slice(), INBOUND.as_slice()].concat());

        let first = read_frame(&mut decoder);
        assert_eq!(first.direction, Direction::Outbound);
        assert_eq!(first.command, [0x01, 0x01]);
        assert!(first.body.is_empty());

        let second = read_frame(&mut decoder);
        assert_eq!(second.direction, Direction::Inbound);
        assert_eq!(second.command, [0x01, 0x03]);
        assert_eq!(second.body.as_ref(), &[0x2a]);

        assert!(decoder.next_frame().expect("end of stream").is_none());
    }

    #[test]
    fn discards_a_bad_checksum_and_decodes_the_following_frame() {
        let mut damaged = OUTBOUND;
        damaged[9] ^= 1;

        let mut decoder = FrameDecoder::default();
        decoder.push(&[damaged.as_slice(), INBOUND.as_slice()].concat());

        let Err(error) = decoder.next_frame() else {
            panic!("the frame with a bad checksum was not rejected");
        };
        assert_eq!(error, DecodeError::ChecksumMismatch);

        let frame = read_frame(&mut decoder);
        assert_eq!(frame.direction, Direction::Inbound);
        assert_eq!(frame.command, [0x01, 0x03]);
        assert_eq!(frame.body.as_ref(), &[0x2a]);
        assert!(decoder.next_frame().expect("end of stream").is_none());
    }

    fn read_frame(decoder: &mut FrameDecoder) -> Frame {
        decoder
            .next_frame()
            .expect("decode valid bytes")
            .expect("receive a complete frame")
    }
}
