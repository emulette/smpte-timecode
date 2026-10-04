//! MIDI Time Code encoding: exact bytes, and round trips through the decoder.

use core::time::Duration;

use smpte_timecode::{
    Direction, FrameRate, Motion, MtcDecoder, MtcEncoder, MtcError, Ratio, Timecode,
};

fn parse(text: &str, rate: FrameRate) -> Timecode {
    Timecode::parse(text, rate).unwrap()
}

#[test]
fn quarter_frames_carry_each_field_nibble_in_order() {
    let start = parse("17:42:59;28", FrameRate::FPS_29_97_DF);
    let messages: Vec<[u8; 2]> = MtcEncoder::new(start, Direction::Forward)
        .unwrap()
        .take(9)
        .collect();
    // 28 = 0x1C, 59 = 0x3B, 42 = 0x2A, 17 = 0x11; rate code 2 is 0b100 in piece 7.
    assert_eq!(
        messages,
        [
            [0xF1, 0x0C],
            [0xF1, 0x11],
            [0xF1, 0x2B],
            [0xF1, 0x33],
            [0xF1, 0x4A],
            [0xF1, 0x52],
            [0xF1, 0x61],
            [0xF1, 0x75],
            // The next cycle carries 17:43:00;02, two frames on across the dropped labels.
            [0xF1, 0x02],
        ]
    );
}

#[test]
fn reverse_streams_send_pieces_from_seven_down() {
    let start = parse("01:00:00:00", FrameRate::FPS_24);
    let mut encoder = MtcEncoder::new(start, Direction::Reverse).unwrap();
    let pieces: Vec<u8> = encoder
        .by_ref()
        .take(9)
        .map(|[_, data]| data >> 4)
        .collect();
    assert_eq!(pieces, [7, 6, 5, 4, 3, 2, 1, 0, 7]);
    assert_eq!(encoder.timecode().to_string(), "00:59:59:22");
    assert_eq!(encoder.direction(), Direction::Reverse);
}

#[test]
fn full_frames_carry_the_rate_code_in_the_hours_byte() {
    let cases = [
        (parse("01:02:03:04", FrameRate::FPS_24), 0x01),
        (parse("01:02:03:04", FrameRate::FPS_23_976), 0x01),
        (parse("01:02:03:04", FrameRate::FPS_25), 0x21),
        (parse("01:02:03;04", FrameRate::FPS_29_97_DF), 0x41),
        (parse("01:02:03;04", FrameRate::FPS_30_DF), 0x41),
        (parse("01:02:03:04", FrameRate::FPS_29_97), 0x61),
        (parse("23:02:03:04", FrameRate::FPS_30), 0x77),
    ];
    for (timecode, hours) in cases {
        let expected = [0xF0, 0x7F, 0x7F, 0x01, 0x01, hours, 0x02, 0x03, 0x04, 0xF7];
        assert_eq!(
            MtcEncoder::full_frame(timecode),
            Ok(expected),
            "{timecode:?}"
        );
    }
}

#[test]
fn rates_of_48_frames_and_faster_have_no_rate_code() {
    for rate in FrameRate::ALL
        .into_iter()
        .filter(|rate| rate.nominal() >= 48)
    {
        let start = Timecode::from_frames(0, rate).unwrap();
        assert_eq!(
            MtcEncoder::new(start, Direction::Forward).err(),
            Some(MtcError::NoRateCode)
        );
        assert_eq!(MtcEncoder::full_frame(start), Err(MtcError::NoRateCode));
    }
}

#[test]
fn quarter_frames_are_a_quarter_of_a_frame_apart() {
    let interval = |rate: FrameRate| {
        let start = Timecode::from_frames(0, rate).unwrap();
        MtcEncoder::new(start, Direction::Forward)
            .unwrap()
            .quarter_frame_interval()
    };
    assert_eq!(interval(FrameRate::FPS_25), Ratio::new(1, 100).unwrap());
    assert_eq!(interval(FrameRate::FPS_24), Ratio::new(1, 96).unwrap());
    assert_eq!(
        interval(FrameRate::FPS_29_97_DF),
        Ratio::new(1001, 120_000).unwrap()
    );
}

/// Every rate MIDI Time Code can carry, with the hint a decoder needs to tell it apart.
const SENDABLE: [(FrameRate, Option<FrameRate>); 7] = [
    (FrameRate::FPS_23_976, Some(FrameRate::FPS_23_976)),
    (FrameRate::FPS_24, None),
    (FrameRate::FPS_25, None),
    (FrameRate::FPS_29_97, Some(FrameRate::FPS_29_97)),
    (FrameRate::FPS_29_97_DF, None),
    (FrameRate::FPS_30, None),
    (FrameRate::FPS_30_DF, Some(FrameRate::FPS_30_DF)),
];

/// Runs `cycles` cycles from the encoder through a decoder and returns every decoded event.
fn round_trip(
    encoder: MtcEncoder,
    hint: Option<FrameRate>,
    cycles: usize,
) -> Vec<(Timecode, Motion)> {
    let mut decoder = MtcDecoder::new();
    decoder.set_rate_hint(hint);
    encoder
        .take(cycles * 8)
        .enumerate()
        .filter_map(|(index, message)| {
            let at = Duration::from_millis(10 * index as u64);
            decoder.feed(&message, at).unwrap()
        })
        .map(|event| (event.timecode, event.motion))
        .collect()
}

#[test]
fn forward_streams_decode_to_the_compensated_position() {
    for (rate, hint) in SENDABLE {
        for boundary in boundaries(rate) {
            let start = boundary.wrapping_add_frames(-4);
            let encoder = MtcEncoder::new(start, Direction::Forward).unwrap();
            let events = round_trip(encoder, hint, 6);
            let expected: Vec<(Timecode, Motion)> = (1..=6)
                .map(|cycle| (start.wrapping_add_frames(2 * cycle), Motion::Forward))
                .collect();
            assert_eq!(events, expected, "{rate:?} from {start}");
            assert_eq!(events[1].0, boundary);
        }
    }
}

#[test]
fn reverse_streams_decode_to_the_encoded_position() {
    for (rate, hint) in SENDABLE {
        for boundary in boundaries(rate) {
            let start = boundary.wrapping_add_frames(4);
            let encoder = MtcEncoder::new(start, Direction::Reverse).unwrap();
            let events = round_trip(encoder, hint, 6);
            let expected: Vec<(Timecode, Motion)> = (0..6)
                .map(|cycle| (start.wrapping_add_frames(-2 * cycle), Motion::Reverse))
                .collect();
            assert_eq!(events, expected, "{rate:?} from {start}");
            assert_eq!(events[2].0, boundary);
        }
    }
}

/// First valid labels of a drop minute, a ten-minute block, and the day at sendable rates.
fn boundaries(rate: FrameRate) -> [Timecode; 3] {
    let minute = if rate.is_drop_frame() {
        "00:01:00;02"
    } else {
        "00:01:00:00"
    };
    [
        parse(minute, rate),
        parse("00:10:00:00", rate),
        parse("00:00:00:00", rate),
    ]
}

#[test]
fn full_frames_decode_to_the_same_position() {
    for (rate, hint) in SENDABLE {
        let mut decoder = MtcDecoder::new();
        decoder.set_rate_hint(hint);
        for frame in (0..rate.frames_per_day()).step_by(9_973) {
            let timecode = Timecode::from_frames(frame, rate).unwrap();
            let message = MtcEncoder::full_frame(timecode).unwrap();
            let event = decoder.feed(&message, Duration::ZERO).unwrap().unwrap();
            assert_eq!((event.timecode, event.motion), (timecode, Motion::Located));
        }
    }
}
