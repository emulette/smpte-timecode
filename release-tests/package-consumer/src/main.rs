//! Runs the public API of the packaged crate from a project outside the source tree.

use core::time::Duration;

use smpte_timecode::{
    Direction, FrameRate, LockState, Motion, MtcDecoder, MtcEncoder, Pull, Ratio, Rounding,
    Timecode,
};

fn main() {
    let rate = FrameRate::FPS_29_97_DF;
    let hour = Timecode::parse("01:00:00;00", rate).expect("label parses");
    assert_eq!(hour.frames(), 107_892);
    assert_eq!(hour.wrapping_add_frames(-1).to_string(), "00:59:59;29");

    let khz48 = Ratio::from_integer(48_000);
    assert_eq!(hour.to_samples(khz48, Rounding::Down), Ok(172_799_827));
    let pulled = Pull::DOWN_0_1_PERCENT.apply(khz48).expect("pull fits");
    assert_eq!((pulled.numer(), pulled.denom()), (48_000_000, 1001));

    let mut decoder = MtcDecoder::new();
    let encoder = MtcEncoder::new(hour, Direction::Forward).expect("29.97 DF has a rate code");
    let mut last = None;
    for (index, message) in encoder.take(16).enumerate() {
        let at = Duration::from_millis(8 * index as u64);
        if let Some(event) = decoder.feed(&message, at).expect("encoder output decodes") {
            last = Some(event);
        }
    }
    let event = last.expect("two complete cycles");
    assert_eq!(event.timecode.to_string(), "01:00:00;04");
    assert_eq!(event.motion, Motion::Forward);
    assert_eq!(decoder.lock_state(event.at), LockState::Locked(Motion::Forward));
}
