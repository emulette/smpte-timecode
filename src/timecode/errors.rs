use core::fmt;

const UNSUPPORTED_FRAME_RATE: &str = "unsupported frame rate";
const INVALID_SYNTAX: &str = "timecode must be written as HH:MM:SS:FF";
const DROP_FRAME_SEPARATOR: &str = "only a drop-frame rate takes a ';' frame separator";
const FIELD_OUT_OF_RANGE: &str = "timecode field is out of range for the frame rate";
const DROPPED_LABEL: &str = "drop-frame timecode skips this label";
const FRAME_OUT_OF_RANGE: &str = "frame position is outside the 24-hour day";
const ZERO_DENOMINATOR: &str = "ratio denominator is zero";
const ZERO_SAMPLE_RATE: &str = "sample rate is zero";
const OVERFLOW: &str = "arithmetic overflow";

/// An error from building, parsing or converting a timecode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum TimecodeError {
    /// The nominal rate, 1000/1001 and drop-frame combination is not one of
    /// [`FrameRate::ALL`](crate::FrameRate::ALL).
    UnsupportedFrameRate,
    /// The text is not `HH:MM:SS:FF` with two-digit fields and a two- or three-digit frame field.
    InvalidSyntax,
    /// A non-drop-frame timecode used the `;` frame separator.
    DropFrameSeparator,
    /// Hours exceed 23, minutes or seconds exceed 59, or frames reach the nominal rate.
    FieldOutOfRange,
    /// The label is one drop-frame counting skips, such as `00:01:00;00` at 29.97 DF.
    DroppedLabel,
    /// The frame position does not fall within one 24-hour day.
    FrameOutOfRange,
    /// A ratio was given a zero denominator.
    ZeroDenominator,
    /// A conversion from samples was given a zero sample rate.
    ZeroSampleRate,
    /// An intermediate or final value does not fit the result type.
    Overflow,
}

impl TimecodeError {
    const fn message(self) -> &'static str {
        match self {
            Self::UnsupportedFrameRate => UNSUPPORTED_FRAME_RATE,
            Self::InvalidSyntax => INVALID_SYNTAX,
            Self::DropFrameSeparator => DROP_FRAME_SEPARATOR,
            Self::FieldOutOfRange => FIELD_OUT_OF_RANGE,
            Self::DroppedLabel => DROPPED_LABEL,
            Self::FrameOutOfRange => FRAME_OUT_OF_RANGE,
            Self::ZeroDenominator => ZERO_DENOMINATOR,
            Self::ZeroSampleRate => ZERO_SAMPLE_RATE,
            Self::Overflow => OVERFLOW,
        }
    }
}

impl fmt::Display for TimecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.message())
    }
}

impl core::error::Error for TimecodeError {}
