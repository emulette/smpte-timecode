use core::fmt;

use crate::TimecodeError;

const MALFORMED_MESSAGE: &str = "malformed MIDI Time Code message";
const INVALID_TIMECODE: &str = "MIDI Time Code carries an invalid timecode";
const NO_RATE_CODE: &str = "frame rate has no MIDI Time Code rate code";

/// An error from decoding or encoding MIDI Time Code.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum MtcError {
    /// A quarter-frame or full-frame message has the wrong length, framing or a data byte with
    /// its high bit set.
    MalformedMessage,
    /// A complete message names a position that does not exist at its frame rate, such as hour 24
    /// or a skipped drop-frame label.
    InvalidTimecode(TimecodeError),
    /// MIDI Time Code has rate codes only for 24, 25 and 30 frame labels per second, so 48 and
    /// faster rates cannot be sent.
    NoRateCode,
}

impl fmt::Display for MtcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::MalformedMessage => MALFORMED_MESSAGE,
            Self::InvalidTimecode(_) => INVALID_TIMECODE,
            Self::NoRateCode => NO_RATE_CODE,
        })
    }
}

impl core::error::Error for MtcError {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::InvalidTimecode(error) => Some(error),
            Self::MalformedMessage | Self::NoRateCode => None,
        }
    }
}
