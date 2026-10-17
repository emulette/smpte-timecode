//! MIDI Time Code decoding from hand-built messages, independent of the encoder.

use core::time::Duration;

use smpte_timecode::{
    FrameRate, LockState, Motion, MtcDecoder, MtcError, MtcEvent, Ratio, TimecodeError,
};

/// The eight quarter-frame data nibbles for a position and rate code.
fn pieces(hours: u8, minutes: u8, seconds: u8, frames: u8, code: u8) -> [u8; 8] {
    [
        frames & 0x0F,
        frames >> 4,
        seconds & 0x0F,
        seconds >> 4,
        minutes & 0x0F,
        minutes >> 4,
        hours & 0x0F,
        (hours >> 4) | (code << 1),
    ]
}

fn full_frame(hours: u8, minutes: u8, seconds: u8, frames: u8, code: u8) -> [u8; 10] {
    [
        0xF0,
        0x7F,
        0x7F,
        0x01,
        0x01,
        (code << 5) | hours,
        minutes,
        seconds,
        frames,
        0xF7,
    ]
}

/// Feeds messages 10 ms apart, as a 25 fps source would, and keeps the clock between calls.
struct Source {
    decoder: MtcDecoder,
    now: Duration,
}

impl Source {
    fn new() -> Self {
        Self::with(MtcDecoder::new())
    }

    fn with(decoder: MtcDecoder) -> Self {
        Self {
            decoder,
            now: Duration::ZERO,
        }
    }

    fn feed(&mut self, message: &[u8]) -> Result<Option<MtcEvent>, MtcError> {
        self.now += Duration::from_millis(10);
        self.decoder.feed(message, self.now)
    }

    /// Feeds the listed pieces in order and returns the labels of the positions they produced.
    fn quarter(&mut self, values: [u8; 8], order: impl IntoIterator<Item = usize>) -> Vec<String> {
        order
            .into_iter()
            .filter_map(|piece| {
                let data = ((piece as u8) << 4) | values[piece];
                self.feed(&[0xF1, data]).unwrap()
            })
            .map(|event| event.timecode.to_string())
            .collect()
    }

    fn forward(&mut self, values: [u8; 8]) -> Vec<String> {
        self.quarter(values, 0..8)
    }

    fn events(&mut self, values: [u8; 8], order: impl IntoIterator<Item = usize>) -> Vec<MtcEvent> {
        order
            .into_iter()
            .filter_map(|piece| {
                self.feed(&[0xF1, ((piece as u8) << 4) | values[piece]])
                    .unwrap()
            })
            .collect()
    }
}

#[test]
fn rate_codes_name_24_25_2997_drop_and_30() {
    let mut source = Source::new();
    let rates: Vec<FrameRate> = (0..4)
        .map(|code| {
            source
                .feed(&full_frame(1, 0, 0, 0, code))
                .unwrap()
                .unwrap()
                .timecode
                .rate()
        })
        .collect();
    assert_eq!(
        rates,
        [
            FrameRate::FPS_24,
            FrameRate::FPS_25,
            FrameRate::FPS_29_97_DF,
            FrameRate::FPS_30
        ]
    );
}

#[test]
fn a_forward_cycle_yields_its_position_two_frames_later() {
    let mut source = Source::new();
    let events = source.events(pieces(1, 2, 3, 4, 2), 0..8);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].timecode.to_string(), "01:02:03;06");
    assert_eq!(events[0].motion, Motion::Forward);
    assert_eq!(events[0].at, Duration::from_millis(80));
}

#[test]
fn forward_compensation_crosses_second_minute_drop_and_midnight_boundaries() {
    let cases = [
        (0, (0, 0, 9, 22), "00:00:10:00"),
        (1, (0, 0, 59, 24), "00:01:00:01"),
        (3, (0, 59, 59, 29), "01:00:00:01"),
        (0, (23, 59, 59, 23), "00:00:00:01"),
        (2, (0, 0, 59, 29), "00:01:00;03"),
        (2, (0, 9, 59, 29), "00:10:00;01"),
        (2, (0, 59, 59, 29), "01:00:00;01"),
        (2, (23, 59, 59, 29), "00:00:00;01"),
    ];
    for (code, (hours, minutes, seconds, frames), expected) in cases {
        let mut source = Source::new();
        assert_eq!(
            source.forward(pieces(hours, minutes, seconds, frames, code)),
            [expected]
        );
    }
}

#[test]
fn a_missing_quarter_frame_never_builds_a_mixed_position() {
    let mut source = Source::new();
    assert!(source.quarter(pieces(1, 0, 0, 0, 0), 0..6).is_empty());
    assert!(
        source
            .quarter(pieces(1, 0, 5, 0, 0), [0, 1, 2, 3, 4, 6, 7])
            .is_empty()
    );
    assert_eq!(source.forward(pieces(1, 0, 6, 0, 0)), ["01:00:06:02"]);
}

#[test]
fn a_repeated_quarter_frame_restarts_the_cycle() {
    let mut source = Source::new();
    assert!(
        source
            .quarter(pieces(1, 0, 0, 0, 0), [0, 1, 2, 3, 3, 4, 5, 6, 7])
            .is_empty()
    );
    assert_eq!(source.forward(pieces(1, 0, 0, 2, 0)), ["01:00:00:04"]);
}

#[test]
fn joining_mid_cycle_waits_for_the_next_complete_cycle() {
    // Pieces 4-7 of 01:59:59:24 and pieces 0-3 of 02:00:00:01 would mix into 01:59:00:01.
    let mut source = Source::new();
    assert!(source.quarter(pieces(1, 59, 59, 24, 1), 4..8).is_empty());
    assert_eq!(source.forward(pieces(2, 0, 0, 1, 1)), ["02:00:00:03"]);
}

#[test]
fn a_full_frame_locate_discards_the_incomplete_cycle() {
    let mut source = Source::new();
    assert!(source.quarter(pieces(1, 0, 0, 0, 0), 0..6).is_empty());
    let located = source.feed(&full_frame(1, 0, 12, 0, 0)).unwrap().unwrap();
    assert_eq!(located.timecode.to_string(), "01:00:12:00");
    assert_eq!(located.motion, Motion::Located);
    assert!(source.quarter(pieces(1, 0, 0, 0, 0), [6, 7]).is_empty());
}

#[test]
fn motions_distinguish_forward_reverse_and_locate() {
    let mut source = Source::new();
    let mut events = source.events(pieces(1, 0, 7, 0, 0), 0..8);
    events.extend(source.events(pieces(1, 0, 6, 0, 0), (0..8).rev()));
    events.extend(source.feed(&full_frame(1, 0, 12, 0, 0)).unwrap());
    let summary: Vec<(String, Motion)> = events
        .iter()
        .map(|event| (event.timecode.to_string(), event.motion))
        .collect();
    assert_eq!(
        summary,
        [
            ("01:00:07:02".to_owned(), Motion::Forward),
            ("01:00:06:00".to_owned(), Motion::Reverse),
            ("01:00:12:00".to_owned(), Motion::Located),
        ]
    );
}

#[test]
fn turning_around_mid_cycle_yields_nothing_mixed() {
    let mut source = Source::new();
    assert!(
        source
            .quarter(pieces(1, 0, 0, 0, 0), [0, 1, 2, 3, 2, 1, 0])
            .is_empty()
    );
    assert_eq!(
        source.quarter(pieces(1, 0, 0, 0, 0), (0..8).rev()),
        ["01:00:00:00"]
    );
}

#[test]
fn a_gap_longer_than_the_timeout_restarts_the_cycle() {
    let decoder = MtcDecoder::new().with_quarter_frame_timeout(Duration::from_millis(100));
    let mut source = Source::with(decoder);
    let values = pieces(1, 0, 0, 0, 0);
    assert!(source.quarter(values, 0..4).is_empty());
    source.now += Duration::from_millis(100);
    assert!(source.quarter(values, 4..8).is_empty());
    assert_eq!(source.forward(values), ["01:00:00:02"]);
}

#[test]
fn reserved_bits_are_ignored() {
    let mut values = pieces(1, 2, 3, 4, 3);
    values[1] |= 0b1110;
    values[3] |= 0b1100;
    values[5] |= 0b1100;
    values[7] |= 0b1000;
    let mut source = Source::new();
    assert_eq!(source.forward(values), ["01:02:03:06"]);

    let mut message = full_frame(1, 2, 3, 4, 3);
    message[6] |= 0x40;
    message[7] |= 0x40;
    message[8] |= 0x60;
    assert_eq!(
        source.feed(&message).unwrap().unwrap().timecode.to_string(),
        "01:02:03:04"
    );
}

#[test]
fn positions_that_do_not_exist_are_errors() {
    let mut source = Source::new();
    assert_eq!(
        source.feed(&full_frame(24, 0, 0, 0, 1)),
        Err(MtcError::InvalidTimecode(TimecodeError::FieldOutOfRange))
    );
    assert_eq!(
        source.feed(&full_frame(0, 1, 0, 0, 2)),
        Err(MtcError::InvalidTimecode(TimecodeError::DroppedLabel))
    );
    let values = pieces(1, 0, 0, 25, 1);
    let results: Vec<_> = (0..8)
        .map(|piece| source.feed(&[0xF1, ((piece as u8) << 4) | values[piece]]))
        .collect();
    assert_eq!(
        results[7],
        Err(MtcError::InvalidTimecode(TimecodeError::FieldOutOfRange))
    );
    assert_eq!(source.decoder.lock_state(source.now), LockState::Unlocked);
}

#[test]
fn malformed_time_code_messages_discard_partial_cycles_and_recover() {
    let mut truncated = full_frame(1, 0, 0, 0, 1).to_vec();
    truncated.remove(8);
    let mut unterminated = full_frame(1, 0, 0, 0, 1);
    unterminated[9] = 0x00;
    let mut high_bit = full_frame(1, 0, 0, 0, 1);
    high_bit[7] = 0x80;
    let malformed: [&[u8]; 6] = [
        &[0xF1],
        &[0xF1, 0x80],
        &[0xF1, 0x00, 0x00],
        &truncated,
        &unterminated,
        &high_bit,
    ];
    for message in malformed {
        for reverse in [false, true] {
            let mut source = Source::new();
            source.feed(&full_frame(1, 0, 0, 0, 1)).unwrap();
            let last_position_at = source.now;
            let order: Vec<usize> = if reverse {
                (0..8).rev().collect()
            } else {
                (0..8).collect()
            };
            assert!(
                source
                    .quarter(pieces(1, 2, 3, 4, 1), order[..4].iter().copied())
                    .is_empty()
            );
            assert_eq!(
                source.feed(message),
                Err(MtcError::MalformedMessage),
                "{message:02X?}"
            );
            assert!(
                source
                    .quarter(pieces(2, 30, 40, 8, 1), order[4..].iter().copied())
                    .is_empty(),
                "mixed position after {message:02X?}, reverse={reverse}"
            );
            // Errors and incomplete cycles neither replace nor refresh the last valid position.
            assert_eq!(
                source.decoder.lock_state(source.now),
                LockState::Locked(Motion::Located)
            );
            assert_eq!(
                source
                    .decoder
                    .lock_state(last_position_at + Duration::from_millis(1001)),
                LockState::Stale
            );
            let events = source.events(pieces(2, 30, 40, 8, 1), order);
            assert_eq!(events.len(), 1);
            assert_eq!(
                events[0].timecode.to_string(),
                if reverse {
                    "02:30:40:08"
                } else {
                    "02:30:40:10"
                }
            );
            assert_eq!(
                events[0].motion,
                if reverse {
                    Motion::Reverse
                } else {
                    Motion::Forward
                }
            );
            assert_eq!(
                source.decoder.lock_state(source.now),
                LockState::Locked(events[0].motion)
            );
        }
    }
}

#[test]
fn other_midi_messages_are_ignored() {
    let mut source = Source::new();
    let user_bits = [
        0xF0, 0x7F, 0x7F, 0x01, 0x02, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xF7,
    ];
    let others: [&[u8]; 5] = [
        &[],
        &[0x90, 60, 100],
        &[0xF8],
        &user_bits,
        &[0xF0, 0x7E, 0x7F, 0x06, 0x01, 0xF7],
    ];
    for message in others {
        assert_eq!(source.feed(message), Ok(None), "{message:02X?}");
    }
}

#[test]
fn reset_discards_pending_pieces_and_the_last_position() {
    let mut source = Source::new();
    source.feed(&full_frame(1, 0, 0, 0, 0)).unwrap();
    assert!(source.quarter(pieces(1, 0, 0, 0, 0), 0..6).is_empty());
    source.decoder.reset();
    assert_eq!(source.decoder.lock_state(source.now), LockState::Unlocked);
    assert!(source.quarter(pieces(1, 0, 12, 0, 0), [6, 7]).is_empty());
    assert_eq!(source.forward(pieces(1, 0, 12, 0, 0)), ["01:00:12:02"]);
}

#[test]
fn lock_state_follows_the_age_of_the_last_position() {
    let decoder = MtcDecoder::new().with_stale_after(Duration::from_millis(200));
    let mut source = Source::with(decoder);
    assert_eq!(source.decoder.lock_state(source.now), LockState::Unlocked);
    source.forward(pieces(1, 0, 0, 0, 1));
    let at = source.now;
    assert_eq!(
        source.decoder.lock_state(at),
        LockState::Locked(Motion::Forward)
    );
    assert_eq!(
        source.decoder.lock_state(at + Duration::from_millis(200)),
        LockState::Locked(Motion::Forward)
    );
    assert_eq!(
        source.decoder.lock_state(at + Duration::from_millis(201)),
        LockState::Stale
    );
    source.feed(&full_frame(1, 0, 0, 0, 1)).unwrap();
    assert_eq!(
        source.decoder.lock_state(source.now),
        LockState::Locked(Motion::Located)
    );
}

#[test]
fn the_default_limits_are_half_a_second_and_one_second() {
    let mut decoder = MtcDecoder::default();
    decoder
        .feed(&full_frame(1, 0, 0, 0, 1), Duration::ZERO)
        .unwrap();
    assert_eq!(
        decoder.lock_state(Duration::from_secs(1)),
        LockState::Locked(Motion::Located)
    );
    assert_eq!(
        decoder.lock_state(Duration::from_millis(1001)),
        LockState::Stale
    );

    let values = pieces(1, 0, 0, 0, 1);
    let mut at = Duration::from_secs(5);
    let mut events = Vec::new();
    for (piece, value) in (0_u8..).zip(values) {
        at += if piece == 4 {
            Duration::from_millis(499)
        } else {
            Duration::from_millis(10)
        };
        events.extend(decoder.feed(&[0xF1, (piece << 4) | value], at).unwrap());
    }
    assert_eq!(events.len(), 1);
}

#[test]
fn a_rate_hint_refines_what_the_rate_code_cannot_express() {
    let cases = [
        (FrameRate::FPS_29_97, 3, "01:00:00:12"),
        (FrameRate::FPS_30_DF, 2, "01:00:00;12"),
        (FrameRate::FPS_23_976, 0, "01:00:00:12"),
    ];
    for (hint, code, expected) in cases {
        let mut source = Source::new();
        source.decoder.set_rate_hint(Some(hint));
        let events = source.events(pieces(1, 0, 0, 10, code), 0..8);
        assert_eq!(events[0].timecode.rate(), hint);
        assert_eq!(events[0].timecode.to_string(), expected);
    }

    let mut source = Source::new();
    source.decoder.set_rate_hint(Some(FrameRate::FPS_29_97));
    let located = source.feed(&full_frame(1, 2, 3, 4, 2)).unwrap().unwrap();
    assert_eq!(located.timecode.to_string(), "01:02:03:04");
}

#[test]
fn a_rate_hint_at_twice_the_code_count_gives_the_half_rate() {
    // Sessions at 48 frames per second and faster send half-count MTC. The positions keep the
    // labels the stream carries, at the half rate with the hint's 1000/1001 factor and counting.
    let cases = [
        (FrameRate::FPS_59_94, 3, FrameRate::FPS_29_97, "01:00:00:12"),
        (
            FrameRate::FPS_59_94_DF,
            2,
            FrameRate::FPS_29_97_DF,
            "01:00:00;12",
        ),
        (
            FrameRate::FPS_59_94_DF,
            3,
            FrameRate::FPS_29_97_DF,
            "01:00:00;12",
        ),
        (
            FrameRate::FPS_47_952,
            0,
            FrameRate::FPS_23_976,
            "01:00:00:12",
        ),
        (FrameRate::FPS_60_DF, 3, FrameRate::FPS_30_DF, "01:00:00;12"),
        (FrameRate::FPS_48, 0, FrameRate::FPS_24, "01:00:00:12"),
        (FrameRate::FPS_50, 1, FrameRate::FPS_25, "01:00:00:12"),
        (FrameRate::FPS_60, 3, FrameRate::FPS_30, "01:00:00:12"),
    ];
    for (hint, code, rate, expected) in cases {
        let mut source = Source::new();
        source.decoder.set_rate_hint(Some(hint));
        let events = source.events(pieces(1, 0, 0, 10, code), 0..8);
        assert_eq!(events[0].timecode.rate(), rate, "{hint:?}");
        assert_eq!(events[0].timecode.to_string(), expected, "{hint:?}");
    }

    // A 59.94 session's hour runs 1000/1001 slower than its labels count.
    let mut source = Source::new();
    source.decoder.set_rate_hint(Some(FrameRate::FPS_59_94));
    let located = source.feed(&full_frame(1, 0, 0, 0, 3)).unwrap().unwrap();
    assert_eq!(located.timecode.rate(), FrameRate::FPS_29_97);
    assert_eq!(
        located.timecode.to_seconds(),
        Ratio::new(18_018, 5).unwrap()
    );
}

#[test]
fn the_stream_rate_wins_when_the_hint_counts_different_frames() {
    let mut source = Source::new();
    source.decoder.set_rate_hint(Some(FrameRate::FPS_24));
    let labels: Vec<String> = (25..30)
        .flat_map(|frames| source.forward(pieces(1, 0, 0, frames, 3)))
        .collect();
    assert_eq!(
        labels,
        [
            "01:00:00:27",
            "01:00:00:28",
            "01:00:00:29",
            "01:00:01:00",
            "01:00:01:01"
        ]
    );

    source.decoder.set_rate_hint(Some(FrameRate::FPS_25));
    let located = source.feed(&full_frame(1, 2, 3, 4, 2)).unwrap().unwrap();
    assert_eq!(located.timecode.rate(), FrameRate::FPS_29_97_DF);

    // Twice another code's count, or four times the code's, is not the stream's session rate.
    for (hint, code, rate) in [
        (FrameRate::FPS_48, 1, FrameRate::FPS_25),
        (FrameRate::FPS_100, 1, FrameRate::FPS_25),
        (FrameRate::FPS_119_88, 3, FrameRate::FPS_30),
    ] {
        source.decoder.set_rate_hint(Some(hint));
        let located = source.feed(&full_frame(1, 2, 3, 4, code)).unwrap().unwrap();
        assert_eq!(located.timecode.rate(), rate, "{hint:?}");
    }

    source.decoder.set_rate_hint(None);
    assert_eq!(source.forward(pieces(1, 0, 0, 10, 3))[0], "01:00:00:12");
}
