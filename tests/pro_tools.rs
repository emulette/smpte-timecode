//! Replay original Pro Tools MIDI captures through the public codec API.

use core::time::Duration;

use smpte_timecode::{Direction, FrameRate, LockState, Motion, MtcDecoder, MtcEncoder, Timecode};

const MINUTE_BOUNDARY: &str = include_str!("fixtures/pro-tools-2026.4.1/24-minute-boundary.txt");
const LOCATE_JUMP: &str = include_str!("fixtures/pro-tools-2026.4.1/24-locate-jump.txt");

fn capture(text: &str) -> Vec<(Duration, Vec<u8>)> {
    text.lines()
        .map(|line| {
            let mut fields = line.split_whitespace();
            let at = Duration::from_micros(fields.next().unwrap().parse().unwrap());
            let bytes = fields
                .map(|field| u8::from_str_radix(field, 16).unwrap())
                .collect();
            (at, bytes)
        })
        .collect()
}

#[test]
fn pro_tools_24_crosses_the_minute_boundary() {
    let mut decoder = MtcDecoder::new();
    let start = Timecode::parse("00:00:59:04", FrameRate::FPS_24).unwrap();
    let mut actual = Vec::new();
    for (at, bytes) in capture(MINUTE_BOUNDARY) {
        if let Some(event) = decoder.feed(&bytes, at).unwrap() {
            assert_eq!(event.motion, Motion::Forward);
            actual.push(event.timecode);
        }
    }
    let expected: Vec<Timecode> = (0..14)
        .map(|cycle| start.wrapping_add_frames(2 * cycle))
        .collect();
    assert_eq!(actual, expected);
    assert!(
        actual
            .iter()
            .any(|position| position.to_string() == "00:01:00:00")
    );
}

#[test]
fn pro_tools_24_stop_restart_and_live_jump_do_not_mix_positions() {
    let mut decoder = MtcDecoder::new();
    let mut actual = Vec::new();
    for (at, bytes) in capture(LOCATE_JUMP) {
        if let Some(event) = decoder.feed(&bytes, at).unwrap() {
            assert_eq!(event.motion, Motion::Forward);
            actual.push(event.timecode);
        }
    }

    // Consecutive original messages: the final two pre-stop cycles, a stopped
    // locate to 01:02:03:04 followed by playback, and a live jump to 00:05:00:00.
    // PT encodes the target plus two; decoded positions add two more.
    let segments = [("00:01:15:04", 2), ("01:02:03:08", 9), ("00:05:00:04", 3)];
    let expected: Vec<Timecode> = segments
        .into_iter()
        .flat_map(|(label, cycles)| {
            let start = Timecode::parse(label, FrameRate::FPS_24).unwrap();
            (0..cycles).map(move |index| start.wrapping_add_frames(2 * index))
        })
        .collect();
    assert_eq!(actual, expected);
}

struct RecordedRun {
    raw_start: &'static str,
    first_message: usize,
    cycles: usize,
    // At a live 1 -> 0 -> 1 transition, direction changes twice. The decoder
    // discards that first cycle and resumes on the next complete forward cycle.
    decoded_cycles_to_skip: usize,
}

struct TransportCapture {
    name: &'static str,
    data: &'static str,
    session_rate: FrameRate,
    decoded_rate: FrameRate,
    messages: usize,
    runs: &'static [RecordedRun],
}

// Literal positions and run lengths were inspected from the original PT nibbles,
// independently of this codec. The frame digits in high-rate MTC are half the UI digits.
const TRANSPORT_CAPTURES: &[TransportCapture] = &[
    TransportCapture {
        name: "23.976",
        data: include_str!("fixtures/pro-tools-2026.4.1/23.976-transport.txt"),
        session_rate: FrameRate::FPS_23_976,
        decoded_rate: FrameRate::FPS_23_976,
        messages: 482,
        runs: &[
            RecordedRun {
                raw_start: "00:00:59:02",
                first_message: 0,
                cycles: 33,
                decoded_cycles_to_skip: 0,
            },
            RecordedRun {
                raw_start: "01:02:03:06",
                first_message: 270,
                cycles: 23,
                decoded_cycles_to_skip: 0,
            },
            RecordedRun {
                raw_start: "00:05:00:02",
                first_message: 458,
                cycles: 3,
                decoded_cycles_to_skip: 0,
            },
        ],
    },
    TransportCapture {
        name: "25",
        data: include_str!("fixtures/pro-tools-2026.4.1/25-transport.txt"),
        session_rate: FrameRate::FPS_25,
        decoded_rate: FrameRate::FPS_25,
        messages: 491,
        runs: &[
            RecordedRun {
                raw_start: "00:00:59:01",
                first_message: 0,
                cycles: 35,
                decoded_cycles_to_skip: 0,
            },
            RecordedRun {
                raw_start: "01:02:03:05",
                first_message: 281,
                cycles: 23,
                decoded_cycles_to_skip: 0,
            },
            RecordedRun {
                raw_start: "00:05:00:01",
                first_message: 467,
                cycles: 3,
                decoded_cycles_to_skip: 1,
            },
        ],
    },
    TransportCapture {
        name: "29.97",
        data: include_str!("fixtures/pro-tools-2026.4.1/29.97-transport.txt"),
        session_rate: FrameRate::FPS_29_97,
        decoded_rate: FrameRate::FPS_29_97,
        messages: 588,
        runs: &[
            RecordedRun {
                raw_start: "00:00:59:02",
                first_message: 0,
                cycles: 42,
                decoded_cycles_to_skip: 0,
            },
            RecordedRun {
                raw_start: "01:02:03:06",
                first_message: 339,
                cycles: 28,
                decoded_cycles_to_skip: 0,
            },
            RecordedRun {
                raw_start: "00:05:00:02",
                first_message: 564,
                cycles: 3,
                decoded_cycles_to_skip: 0,
            },
        ],
    },
    TransportCapture {
        name: "29.97-df",
        data: include_str!("fixtures/pro-tools-2026.4.1/29.97-df-transport.txt"),
        session_rate: FrameRate::FPS_29_97_DF,
        decoded_rate: FrameRate::FPS_29_97_DF,
        messages: 588,
        runs: &[
            RecordedRun {
                raw_start: "00:00:59;02",
                first_message: 0,
                cycles: 41,
                decoded_cycles_to_skip: 0,
            },
            RecordedRun {
                raw_start: "01:02:03;06",
                first_message: 333,
                cycles: 28,
                decoded_cycles_to_skip: 0,
            },
            RecordedRun {
                raw_start: "00:05:00;06",
                first_message: 564,
                cycles: 3,
                decoded_cycles_to_skip: 0,
            },
        ],
    },
    TransportCapture {
        name: "30",
        data: include_str!("fixtures/pro-tools-2026.4.1/30-transport.txt"),
        session_rate: FrameRate::FPS_30,
        decoded_rate: FrameRate::FPS_30,
        messages: 594,
        runs: &[
            RecordedRun {
                raw_start: "00:00:59:02",
                first_message: 0,
                cycles: 42,
                decoded_cycles_to_skip: 0,
            },
            RecordedRun {
                raw_start: "01:02:03:06",
                first_message: 339,
                cycles: 28,
                decoded_cycles_to_skip: 0,
            },
            RecordedRun {
                raw_start: "00:05:00:02",
                first_message: 570,
                cycles: 3,
                decoded_cycles_to_skip: 0,
            },
        ],
    },
    TransportCapture {
        name: "30-df",
        data: include_str!("fixtures/pro-tools-2026.4.1/30-df-transport.txt"),
        session_rate: FrameRate::FPS_30_DF,
        decoded_rate: FrameRate::FPS_30_DF,
        messages: 594,
        runs: &[
            RecordedRun {
                raw_start: "00:00:59;02",
                first_message: 0,
                cycles: 42,
                decoded_cycles_to_skip: 0,
            },
            RecordedRun {
                raw_start: "01:02:03;06",
                first_message: 339,
                cycles: 28,
                decoded_cycles_to_skip: 0,
            },
            RecordedRun {
                raw_start: "00:05:00;06",
                first_message: 570,
                cycles: 3,
                decoded_cycles_to_skip: 0,
            },
        ],
    },
    TransportCapture {
        name: "48",
        data: include_str!("fixtures/pro-tools-2026.4.1/48-transport.txt"),
        session_rate: FrameRate::FPS_48,
        decoded_rate: FrameRate::FPS_24,
        messages: 472,
        runs: &[
            RecordedRun {
                raw_start: "00:00:59:02",
                first_message: 0,
                cycles: 33,
                decoded_cycles_to_skip: 0,
            },
            RecordedRun {
                raw_start: "01:02:03:04",
                first_message: 270,
                cycles: 22,
                decoded_cycles_to_skip: 0,
            },
            RecordedRun {
                raw_start: "00:05:00:02",
                first_message: 448,
                cycles: 3,
                decoded_cycles_to_skip: 1,
            },
        ],
    },
    TransportCapture {
        name: "50",
        data: include_str!("fixtures/pro-tools-2026.4.1/50-transport.txt"),
        session_rate: FrameRate::FPS_50,
        decoded_rate: FrameRate::FPS_25,
        messages: 509,
        runs: &[
            RecordedRun {
                raw_start: "00:00:59:01",
                first_message: 0,
                cycles: 35,
                decoded_cycles_to_skip: 0,
            },
            RecordedRun {
                raw_start: "01:02:03:03",
                first_message: 285,
                cycles: 25,
                decoded_cycles_to_skip: 0,
            },
            RecordedRun {
                raw_start: "00:05:00:01",
                first_message: 485,
                cycles: 3,
                decoded_cycles_to_skip: 0,
            },
        ],
    },
    TransportCapture {
        name: "59.94",
        data: include_str!("fixtures/pro-tools-2026.4.1/59.94-transport.txt"),
        session_rate: FrameRate::FPS_59_94,
        decoded_rate: FrameRate::FPS_29_97,
        messages: 618,
        runs: &[
            RecordedRun {
                raw_start: "00:00:59:02",
                first_message: 0,
                cycles: 43,
                decoded_cycles_to_skip: 0,
            },
            RecordedRun {
                raw_start: "01:02:03:04",
                first_message: 345,
                cycles: 31,
                decoded_cycles_to_skip: 0,
            },
            RecordedRun {
                raw_start: "00:05:00:02",
                first_message: 594,
                cycles: 3,
                decoded_cycles_to_skip: 0,
            },
        ],
    },
    TransportCapture {
        name: "60",
        data: include_str!("fixtures/pro-tools-2026.4.1/60-transport.txt"),
        session_rate: FrameRate::FPS_60,
        decoded_rate: FrameRate::FPS_30,
        messages: 584,
        runs: &[
            RecordedRun {
                raw_start: "00:00:59:02",
                first_message: 0,
                cycles: 41,
                decoded_cycles_to_skip: 0,
            },
            RecordedRun {
                raw_start: "01:02:03:04",
                first_message: 334,
                cycles: 28,
                decoded_cycles_to_skip: 0,
            },
            RecordedRun {
                raw_start: "00:05:00:02",
                first_message: 560,
                cycles: 3,
                decoded_cycles_to_skip: 1,
            },
        ],
    },
];

#[test]
fn pro_tools_session_rate_matrix_decodes_minute_boundary_restart_and_live_jump() {
    for recording in TRANSPORT_CAPTURES {
        let messages = capture(recording.data);
        assert_eq!(messages.len(), recording.messages, "{}", recording.name);
        let mut decoder = MtcDecoder::new();
        decoder.set_rate_hint(Some(recording.session_rate));
        let mut actual = Vec::new();
        for (at, bytes) in &messages {
            assert_eq!(bytes.len(), 2, "{}", recording.name);
            assert_eq!(bytes[0], 0xF1, "{}", recording.name);
            if let Some(event) = decoder.feed(bytes, *at).unwrap() {
                assert_eq!(event.motion, Motion::Forward, "{}", recording.name);
                assert_eq!(event.at, *at, "{}", recording.name);
                actual.push(event.timecode);
            }
        }
        let expected: Vec<Timecode> = recording
            .runs
            .iter()
            .flat_map(|run| {
                let raw = Timecode::parse(run.raw_start, recording.decoded_rate).unwrap();
                (run.decoded_cycles_to_skip..run.cycles)
                    .map(move |cycle| raw.wrapping_add_frames(2 + 2 * cycle as i64))
            })
            .collect();
        assert_eq!(
            actual.len(),
            expected.len(),
            "{} event count",
            recording.name
        );
        for (index, (position, expected_position)) in actual.iter().zip(&expected).enumerate() {
            assert_eq!(
                position, expected_position,
                "{} event {index}",
                recording.name
            );
        }
        assert!(
            actual.iter().any(|position| {
                let fields = position.fields();
                fields.hours == 0 && fields.minutes == 1 && fields.seconds == 0
            }),
            "{} did not cross the minute boundary",
            recording.name
        );
        let stopped_at = messages.last().unwrap().0;
        assert_eq!(
            decoder.lock_state(stopped_at),
            LockState::Locked(Motion::Forward)
        );
        assert_eq!(
            decoder.lock_state(stopped_at + Duration::from_secs(2)),
            LockState::Stale
        );
    }
}

#[test]
fn pro_tools_positions_advance_with_the_recorded_time() {
    // A position read at the wrong rate drifts 0.1% from the recording, 2.8 ms over the longest
    // runs; the timestamps keep their pace far closer than that.
    for recording in TRANSPORT_CAPTURES {
        let mut decoder = MtcDecoder::new();
        decoder.set_rate_hint(Some(recording.session_rate));
        let events: Vec<_> = capture(recording.data)
            .iter()
            .filter_map(|(at, bytes)| decoder.feed(bytes, *at).unwrap())
            .collect();
        // Consecutive forward cycles advance two frames.
        let runs = events.chunk_by(|a, b| b.timecode.frames() == a.timecode.frames() + 2);
        for run in runs.filter(|run| run.len() >= 20) {
            let (first, last) = (run[0], run[run.len() - 1]);
            let positions =
                last.timecode.to_seconds().to_f64() - first.timecode.to_seconds().to_f64();
            let recorded = (last.at - first.at).as_secs_f64();
            assert!(
                (positions - recorded).abs() < 0.000_5,
                "{}: positions advanced {positions} s in {recorded} s",
                recording.name
            );
        }
    }
}

#[test]
fn pro_tools_session_rate_matrix_matches_encoder_bytes_for_every_complete_run() {
    for recording in TRANSPORT_CAPTURES {
        let messages = capture(recording.data);
        for run in recording.runs {
            let raw = Timecode::parse(run.raw_start, recording.decoded_rate).unwrap();
            let encoder = MtcEncoder::new(raw, Direction::Forward).unwrap();
            let recorded = &messages[run.first_message..run.first_message + run.cycles * 8];
            for ((_, captured), encoded) in recorded.iter().zip(encoder) {
                assert_eq!(
                    captured.as_slice(),
                    encoded,
                    "{} at {}",
                    recording.name,
                    run.raw_start
                );
            }
        }
    }
}
