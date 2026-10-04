//! Cross-checks every label of the day against talw-timecode, an independent implementation.
//! Agreement is evidence, not truth: the definition tables in `tests/timecode.rs` are the truth.

use smpte_timecode::{FrameRate, Timecode};
use talw_timecode::FrameRate as Talw;

const SHARED_RATES: [(FrameRate, Talw); 11] = [
    (FrameRate::FPS_23_976, Talw::Fps23_976),
    (FrameRate::FPS_24, Talw::Fps24),
    (FrameRate::FPS_25, Talw::Fps25),
    (FrameRate::FPS_29_97, Talw::Fps29_97Ndf),
    (FrameRate::FPS_29_97_DF, Talw::Fps29_97Df),
    (FrameRate::FPS_30, Talw::Fps30),
    (FrameRate::FPS_48, Talw::Fps48),
    (FrameRate::FPS_50, Talw::Fps50),
    (FrameRate::FPS_59_94, Talw::Fps59_94Ndf),
    (FrameRate::FPS_59_94_DF, Talw::Fps59_94Df),
    (FrameRate::FPS_60, Talw::Fps60),
];

#[test]
fn every_label_of_the_day_matches_talw_timecode() {
    for (rate, talw) in SHARED_RATES {
        for frame in 0..rate.frames_per_day() {
            let ours = Timecode::from_frames(frame, rate).unwrap().fields();
            let theirs = talw_timecode::Timecode::from_frames(i64::from(frame), talw).components();
            assert_eq!(
                (ours.hours, ours.minutes, ours.seconds, ours.frames),
                theirs,
                "{rate:?} frame {frame}"
            );
        }
    }
}
