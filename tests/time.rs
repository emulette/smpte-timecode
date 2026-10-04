//! Exact seconds, samples and pull factors through the public API.

use proptest::prelude::*;
use smpte_timecode::{FrameRate, Pull, Ratio, Rounding, Timecode, TimecodeError};

fn ratio(numer: u64, denom: u64) -> Ratio {
    Ratio::new(numer, denom).unwrap()
}

fn parse(text: &str, rate: FrameRate) -> Timecode {
    Timecode::parse(text, rate).unwrap()
}

#[test]
fn seconds_are_exact_at_ntsc_rates() {
    assert_eq!(
        parse("01:00:00:00", FrameRate::FPS_23_976).to_seconds(),
        ratio(18_018, 5)
    );
    assert_eq!(
        parse("01:00:00:00", FrameRate::FPS_24).to_seconds(),
        ratio(3_600, 1)
    );
    // 107 892 frames × 1001/30 000 s.
    assert_eq!(
        parse("01:00:00;00", FrameRate::FPS_29_97_DF).to_seconds(),
        ratio(107_892 * 1001, 30_000)
    );
    assert_eq!(
        FrameRate::FPS_29_97.frames_per_second(),
        ratio(30_000, 1001)
    );
    assert_eq!(
        FrameRate::FPS_119_88_DF.frames_per_second(),
        ratio(120_000, 1001)
    );
}

#[test]
fn samples_round_as_requested() {
    let hour = parse("01:00:00;00", FrameRate::FPS_29_97_DF);
    let rate = Ratio::from_integer(48_000);
    // Exactly 172 799 827.2 samples.
    assert_eq!(hour.to_samples(rate, Rounding::Down), Ok(172_799_827));
    assert_eq!(hour.to_samples(rate, Rounding::Nearest), Ok(172_799_827));
    assert_eq!(hour.to_samples(rate, Rounding::Up), Ok(172_799_828));

    // One 29.97 frame at 48 kHz is 1601.6 samples; five frames are exactly 8008.
    let five = Timecode::from_frames(5, FrameRate::FPS_29_97).unwrap();
    for rounding in [Rounding::Down, Rounding::Nearest, Rounding::Up] {
        assert_eq!(five.to_samples(rate, rounding), Ok(8_008));
    }

    // Half a sample rounds up under Nearest: one frame of 25 fps at 25/2 Hz is 0.5 samples.
    let one = Timecode::from_frames(1, FrameRate::FPS_25).unwrap();
    assert_eq!(one.to_samples(ratio(25, 2), Rounding::Nearest), Ok(1));
    assert_eq!(one.to_samples(ratio(25, 2), Rounding::Down), Ok(0));
}

#[test]
fn samples_and_seconds_convert_back_to_positions() {
    let rate = FrameRate::FPS_29_97_DF;
    let sample_rate = Ratio::from_integer(48_000);
    assert_eq!(
        Timecode::from_samples(172_799_827, sample_rate, rate, Rounding::Up),
        Ok(parse("01:00:00;00", rate))
    );
    assert_eq!(
        Timecode::from_samples(172_799_827, sample_rate, rate, Rounding::Down),
        Ok(parse("00:59:59;29", rate))
    );
    assert_eq!(
        Timecode::from_seconds(ratio(18_018, 5), FrameRate::FPS_23_976, Rounding::Down),
        Ok(parse("01:00:00:00", FrameRate::FPS_23_976))
    );
    assert_eq!(
        Timecode::from_seconds(
            Ratio::from_integer(86_400),
            FrameRate::FPS_24,
            Rounding::Down
        ),
        Err(TimecodeError::FrameOutOfRange)
    );
    assert_eq!(
        Timecode::from_samples(1, Ratio::from_integer(0), rate, Rounding::Down),
        Err(TimecodeError::ZeroSampleRate)
    );
    assert_eq!(
        Timecode::from_samples(u64::MAX, ratio(1, u64::MAX), rate, Rounding::Down),
        Err(TimecodeError::Overflow)
    );
    let late = parse("23:00:00:00", FrameRate::FPS_24);
    assert_eq!(
        late.to_samples(Ratio::from_integer(u64::MAX), Rounding::Down),
        Err(TimecodeError::Overflow)
    );
}

#[test]
fn pulls_follow_their_definitions() {
    let khz48 = Ratio::from_integer(48_000);
    assert_eq!(
        Pull::DOWN_0_1_PERCENT.apply(khz48),
        Some(ratio(48_000_000, 1001))
    );
    assert_eq!(
        Pull::UP_0_1_PERCENT.apply(khz48),
        Some(Ratio::from_integer(48_048))
    );
    assert_eq!(Pull::NONE.apply(khz48), Some(khz48));
    assert_eq!(
        Pull::UP_4_PERCENT.apply(FrameRate::FPS_24.frames_per_second()),
        Some(FrameRate::FPS_25.frames_per_second())
    );
    assert_eq!(
        Pull::DOWN_0_1_PERCENT.apply(FrameRate::FPS_30.frames_per_second()),
        Some(FrameRate::FPS_29_97.frames_per_second())
    );
    let both = Pull::UP_4_PERCENT.then(Pull::DOWN_0_1_PERCENT).unwrap();
    assert_eq!(both.factor(), ratio(25_000, 24_024));
    assert_eq!(
        Pull::UP_0_1_PERCENT.then(Pull::DOWN_0_1_PERCENT),
        Some(Pull::NONE)
    );
    assert_eq!(
        Pull::UP_4_PERCENT.then(Pull::DOWN_4_PERCENT),
        Some(Pull::NONE)
    );
}

#[test]
fn a_pulled_sample_rate_places_the_same_time_at_fewer_samples() {
    // 48 kHz audio played 0.1% slow: one hour of 23.976 video spans 3603.6 s of the pulled clock.
    let pulled = Pull::DOWN_0_1_PERCENT
        .apply(Ratio::from_integer(48_000))
        .unwrap();
    let hour = parse("01:00:00:00", FrameRate::FPS_23_976);
    assert_eq!(hour.to_samples(pulled, Rounding::Down), Ok(172_800_000));
    assert_eq!(
        Timecode::from_samples(172_800_000, pulled, FrameRate::FPS_23_976, Rounding::Down),
        Ok(hour)
    );
}

#[test]
fn ratios_reduce_compare_and_report_overflow() {
    assert_eq!(Ratio::new(1, 0), Err(TimecodeError::ZeroDenominator));
    let half = ratio(2, 4);
    assert_eq!((half.numer(), half.denom()), (1, 2));
    assert_eq!(ratio(0, 7), Ratio::from_integer(0));
    assert!(ratio(1001, 1000) > Ratio::from_integer(1));
    assert!(ratio(30_000, 1001) < Ratio::from_integer(30));
    assert_eq!(ratio(u64::MAX, 1).checked_mul(ratio(2, 1)), None);
    assert_eq!(
        ratio(u64::MAX, 3).checked_mul(ratio(3, u64::MAX)),
        Some(Ratio::from_integer(1))
    );
    assert_eq!(half.to_f64(), 0.5);
}

fn any_rate() -> impl Strategy<Value = FrameRate> {
    proptest::sample::select(FrameRate::ALL.to_vec())
}

fn any_position() -> impl Strategy<Value = Timecode> {
    any_rate().prop_flat_map(|rate| {
        (0..rate.frames_per_day())
            .prop_map(move |frame| Timecode::from_frames(frame, rate).unwrap())
    })
}

fn any_sample_rate() -> impl Strategy<Value = Ratio> {
    let base = proptest::sample::select(vec![32_000_u64, 44_100, 48_000, 88_200, 96_000, 192_000]);
    let pull = proptest::sample::select(vec![
        Pull::NONE,
        Pull::UP_0_1_PERCENT,
        Pull::DOWN_0_1_PERCENT,
        Pull::UP_4_PERCENT,
        Pull::DOWN_4_PERCENT,
    ]);
    (base, pull).prop_map(|(base, pull)| pull.apply(Ratio::from_integer(base)).unwrap())
}

proptest! {
    /// With at least one sample per frame, rounding the sample one way and the frame the other way
    /// recovers the position.
    #[test]
    fn sample_positions_round_trip(tc in any_position(), sample_rate in any_sample_rate()) {
        let rate = tc.rate();
        for (to, back) in [
            (Rounding::Down, Rounding::Up),
            (Rounding::Up, Rounding::Down),
            (Rounding::Nearest, Rounding::Nearest),
        ] {
            let samples = tc.to_samples(sample_rate, to).unwrap();
            prop_assert_eq!(Timecode::from_samples(samples, sample_rate, rate, back), Ok(tc));
        }
        prop_assert_eq!(Timecode::from_seconds(tc.to_seconds(), rate, Rounding::Down), Ok(tc));
    }

    #[test]
    fn frame_arithmetic_is_associative_and_reversible(
        tc in any_position(),
        a in any::<i32>(),
        b in any::<i32>(),
    ) {
        let (a, b) = (i64::from(a), i64::from(b));
        prop_assert_eq!(tc.wrapping_add_frames(a).wrapping_add_frames(b), tc.wrapping_add_frames(a + b));
        prop_assert_eq!(tc.wrapping_add_frames(a).wrapping_add_frames(-a), tc);
    }
}
