//! MIDI Time Code quarter-frame and full-frame messages.
//!
//! Rate codes map as 0 → 24, 1 → 25, 2 → 29.97 DF and 3 → 30. The encoder sends 23.976 as code 0,
//! 30 DF as code 2 and 29.97 as code 3, because those rates have no code of their own. The
//! decoder recovers them through [`MtcDecoder::set_rate_hint`], which also reads the half-count
//! MTC of sessions at 48 frames per second and faster at the half rate, such as 59.94 as 29.97.

mod decoder;
mod encoder;
mod errors;

pub use decoder::{LockState, MtcDecoder, MtcEvent};
pub use encoder::MtcEncoder;
pub use errors::MtcError;

use crate::FrameRate;

const QUARTER_FRAME: u8 = 0xF1;
const SYSEX_START: u8 = 0xF0;
const SYSEX_END: u8 = 0xF7;
const UNIVERSAL_REAL_TIME: u8 = 0x7F;
const ALL_DEVICES: u8 = 0x7F;
const SUB_ID_TIME_CODE: u8 = 0x01;
const SUB_ID_FULL_FRAME: u8 = 0x01;

/// Which way quarter frames run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Direction {
    /// Pieces 0 to 7, with time moving forward.
    Forward,
    /// Pieces 7 to 0, with time moving backward.
    Reverse,
}

/// How a decoded position was reached.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Motion {
    /// A forward quarter-frame cycle. The position includes the two-frame cycle latency.
    Forward,
    /// A reverse quarter-frame cycle.
    Reverse,
    /// A full-frame message: a locate or a stopped position.
    Located,
}

/// The rate a two-bit code names on its own.
const fn stream_rate(code: u8) -> FrameRate {
    match code {
        0 => FrameRate::FPS_24,
        1 => FrameRate::FPS_25,
        2 => FrameRate::FPS_29_97_DF,
        _ => FrameRate::FPS_30,
    }
}

/// The two-bit code that carries `rate`, grouping rates by nominal frames and drop-frame counting.
const fn rate_code(rate: FrameRate) -> Result<u8, MtcError> {
    match (rate.nominal(), rate.is_drop_frame()) {
        (24, _) => Ok(0),
        (25, _) => Ok(1),
        (30, true) => Ok(2),
        (30, false) => Ok(3),
        _ => Err(MtcError::NoRateCode),
    }
}
