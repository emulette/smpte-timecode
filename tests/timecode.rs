//! Labels, parsing and frame arithmetic through the public API.

use smpte_timecode::{Fields, FrameRate, Timecode, TimecodeError};

fn parse(text: &str, rate: FrameRate) -> Timecode {
    Timecode::parse(text, rate).unwrap()
}

/// Positions that follow from the drop-frame definition alone.
#[test]
fn drop_frame_labels_match_the_definition() {
    let cases = [
        (FrameRate::FPS_29_97_DF, "00:00:59;29", 1_799),
        (FrameRate::FPS_29_97_DF, "00:01:00;02", 1_800),
        (FrameRate::FPS_29_97_DF, "00:10:00;00", 17_982),
        (FrameRate::FPS_29_97_DF, "01:00:00;00", 107_892),
        (FrameRate::FPS_29_97_DF, "23:59:59;29", 2_589_407),
        (FrameRate::FPS_59_94_DF, "00:01:00;04", 3_600),
        (FrameRate::FPS_59_94_DF, "00:10:00;00", 35_964),
        (FrameRate::FPS_59_94_DF, "01:00:00;00", 215_784),
        (FrameRate::FPS_59_94_DF, "23:59:59;59", 5_178_815),
        (FrameRate::FPS_30_DF, "00:01:00;02", 1_800),
        (FrameRate::FPS_119_88_DF, "00:01:00;08", 7_200),
        (FrameRate::FPS_119_88_DF, "00:10:00;00", 71_928),
    ];
    for (rate, label, frames) in cases {
        assert_eq!(parse(label, rate).frames(), frames, "{label}");
        assert_eq!(
            Timecode::from_frames(frames, rate).unwrap().to_string(),
            label
        );
    }
}

#[test]
fn frames_per_day_counts_every_label_once() {
    assert_eq!(FrameRate::FPS_24.frames_per_day(), 2_073_600);
    assert_eq!(FrameRate::FPS_23_976.frames_per_day(), 2_073_600);
    assert_eq!(FrameRate::FPS_29_97.frames_per_day(), 2_592_000);
    assert_eq!(FrameRate::FPS_29_97_DF.frames_per_day(), 2_589_408);
    assert_eq!(FrameRate::FPS_59_94_DF.frames_per_day(), 5_178_816);
    assert_eq!(FrameRate::FPS_120.frames_per_day(), 10_368_000);
}

/// The next label by counting on a clock, independent of the crate's position arithmetic.
fn next_label(label: Fields, rate: FrameRate) -> Fields {
    let skipped = match (rate.is_drop_frame(), rate.nominal()) {
        (false, _) => 0,
        (true, 30) => 2,
        (true, 60) => 4,
        (true, 120) => 8,
        (true, nominal) => panic!("no drop-frame rule for {nominal}"),
    };
    let mut next = label;
    next.frames += 1;
    if next.frames == rate.nominal() {
        next.frames = 0;
        next.seconds += 1;
        if next.seconds == 60 {
            next.seconds = 0;
            next.minutes += 1;
            if next.minutes == 60 {
                next.minutes = 0;
                next.hours = (next.hours + 1) % 24;
            }
            if next.minutes % 10 != 0 {
                next.frames = skipped;
            }
        }
    }
    next
}

/// Every frame of a day at every rate: the label is the previous label counted on by one, and the
/// label converts back to the same frame.
#[test]
fn every_frame_of_the_day_counts_labels_and_round_trips() {
    for rate in FrameRate::ALL {
        let mut expected = Fields {
            hours: 0,
            minutes: 0,
            seconds: 0,
            frames: 0,
        };
        for frame in 0..rate.frames_per_day() {
            let tc = Timecode::from_frames(frame, rate).unwrap();
            let fields = tc.fields();
            assert_eq!(fields, expected, "{rate:?} frame {frame}");
            assert_eq!(Timecode::from_fields(fields, rate), Ok(tc));
            expected = next_label(fields, rate);
        }
        assert_eq!(expected.hours, 0, "{rate:?} must wrap to midnight");
        assert_eq!(
            (expected.minutes, expected.seconds, expected.frames),
            (0, 0, 0)
        );
    }
}

#[test]
fn text_round_trips_across_the_day() {
    for rate in FrameRate::ALL {
        for frame in (0..rate.frames_per_day()).step_by(7) {
            let tc = Timecode::from_frames(frame, rate).unwrap();
            assert_eq!(Timecode::parse(&tc.to_string(), rate), Ok(tc));
        }
    }
}

#[test]
fn display_uses_the_drop_frame_separator_and_three_digit_frames_only_when_needed() {
    assert_eq!(
        parse("01:02:03:04", FrameRate::FPS_29_97).to_string(),
        "01:02:03:04"
    );
    assert_eq!(
        parse("01:02:03:04", FrameRate::FPS_29_97_DF).to_string(),
        "01:02:03;04"
    );
    assert_eq!(
        parse("01:02:03:04", FrameRate::FPS_120).to_string(),
        "01:02:03:04"
    );
    assert_eq!(
        parse("01:02:03:119", FrameRate::FPS_120).to_string(),
        "01:02:03:119"
    );
    assert_eq!(
        parse("01:02:03:004", FrameRate::FPS_25).to_string(),
        "01:02:03:04"
    );
}

#[test]
fn parse_rejects_malformed_text() {
    let rate = FrameRate::FPS_25;
    for text in [
        "",
        "1:02:03:04",
        "01:02:03:4",
        "01:02:03:0004",
        "01-02-03-04",
        "01:02:03.04",
        " 01:02:03:04",
        "01:02:03:04 ",
        "0a:02:03:04",
        "01:02:03:+4",
        "０1:02:03:04",
    ] {
        assert_eq!(
            Timecode::parse(text, rate),
            Err(TimecodeError::InvalidSyntax),
            "{text:?}"
        );
    }
}

#[test]
fn parse_checks_fields_against_the_rate() {
    let df = FrameRate::FPS_29_97_DF;
    assert_eq!(
        Timecode::parse("01:02:03;04", FrameRate::FPS_29_97),
        Err(TimecodeError::DropFrameSeparator)
    );
    assert_eq!(
        Timecode::parse("01:02:03:04", df),
        Ok(parse("01:02:03;04", df))
    );
    assert_eq!(
        Timecode::parse("00:01:00;00", df),
        Err(TimecodeError::DroppedLabel)
    );
    assert_eq!(
        Timecode::parse("00:01:00;01", df),
        Err(TimecodeError::DroppedLabel)
    );
    assert_eq!(
        Timecode::parse("00:01:00;03", FrameRate::FPS_59_94_DF),
        Err(TimecodeError::DroppedLabel)
    );
    assert!(Timecode::parse("00:10:00;00", df).is_ok());
    for text in [
        "24:00:00:00",
        "00:60:00:00",
        "00:00:60:00",
        "00:00:00:25",
        "00:00:00:999",
    ] {
        assert_eq!(
            Timecode::parse(text, FrameRate::FPS_25),
            Err(TimecodeError::FieldOutOfRange),
            "{text}"
        );
    }
}

#[test]
fn from_frames_rejects_positions_past_midnight() {
    let rate = FrameRate::FPS_29_97_DF;
    assert!(Timecode::from_frames(rate.frames_per_day() - 1, rate).is_ok());
    assert_eq!(
        Timecode::from_frames(rate.frames_per_day(), rate),
        Err(TimecodeError::FrameOutOfRange)
    );
}

#[test]
fn arithmetic_wraps_at_midnight_in_both_directions() {
    let rate = FrameRate::FPS_29_97_DF;
    let midnight = parse("00:00:00;00", rate);
    assert_eq!(midnight.wrapping_add_frames(-1).to_string(), "23:59:59;29");
    assert_eq!(
        parse("23:59:59;29", rate)
            .wrapping_add_frames(3)
            .to_string(),
        "00:00:00;02"
    );
    let day = i64::from(rate.frames_per_day());
    assert_eq!(
        midnight.wrapping_add_frames(day * 5 + 1800),
        parse("00:01:00;02", rate)
    );
    assert_eq!(
        midnight.wrapping_add_frames(i64::MAX).frames(),
        (i64::MAX % day) as u32
    );
    assert_eq!(
        midnight.wrapping_add_frames(i64::MIN).frames(),
        i64::MIN.rem_euclid(day) as u32
    );
}

#[test]
fn frame_rates_exist_only_for_the_supported_combinations() {
    for rate in FrameRate::ALL {
        assert_eq!(
            FrameRate::new(rate.nominal(), rate.is_ntsc(), rate.is_drop_frame()),
            Ok(rate)
        );
    }
    for (nominal, ntsc, drop_frame) in [
        (25, false, true),
        (50, true, false),
        (24, false, true),
        (100, true, false),
        (29, true, false),
    ] {
        assert_eq!(
            FrameRate::new(nominal, ntsc, drop_frame),
            Err(TimecodeError::UnsupportedFrameRate)
        );
    }
}
