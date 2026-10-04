use core::fmt;

use super::{FrameRate, Ratio, Rounding, TimecodeError};

/// The four fields of a timecode label.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Fields {
    /// Hours, 0 to 23.
    pub hours: u8,
    /// Minutes, 0 to 59.
    pub minutes: u8,
    /// Seconds, 0 to 59.
    pub seconds: u8,
    /// Frames, 0 up to the nominal frame rate.
    pub frames: u8,
}

/// A position within one 24-hour timecode day at a [`FrameRate`].
///
/// The position is a frame count from `00:00:00:00`, so arithmetic is exact and drop-frame labels
/// only matter when parsing and formatting. Arithmetic wraps at midnight in both directions.
///
/// ```
/// use smpte_timecode::{FrameRate, Timecode};
///
/// let start = Timecode::parse("00:00:59;29", FrameRate::FPS_29_97_DF)?;
/// assert_eq!(start.frames(), 1799);
/// assert_eq!(start.wrapping_add_frames(1).to_string(), "00:01:00;02");
/// # Ok::<(), smpte_timecode::TimecodeError>(())
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Timecode {
    frames: u32,
    rate: FrameRate,
}

impl Timecode {
    /// The position `frames` frames after `00:00:00:00`.
    ///
    /// # Errors
    ///
    /// [`TimecodeError::FrameOutOfRange`] when `frames` is not below
    /// [`FrameRate::frames_per_day`].
    pub const fn from_frames(frames: u32, rate: FrameRate) -> Result<Self, TimecodeError> {
        if frames >= rate.frames_per_day() {
            return Err(TimecodeError::FrameOutOfRange);
        }
        Ok(Self { frames, rate })
    }

    /// The position that `fields` label at `rate`.
    ///
    /// # Errors
    ///
    /// [`TimecodeError::FieldOutOfRange`] for a field outside its range and
    /// [`TimecodeError::DroppedLabel`] for a label drop-frame counting skips.
    pub const fn from_fields(fields: Fields, rate: FrameRate) -> Result<Self, TimecodeError> {
        let Fields {
            hours,
            minutes,
            seconds,
            frames,
        } = fields;
        if hours > 23 || minutes > 59 || seconds > 59 || frames >= rate.nominal() {
            return Err(TimecodeError::FieldOutOfRange);
        }
        let dropped = rate.dropped_per_minute();
        if minutes % 10 != 0 && seconds == 0 && (frames as u32) < dropped {
            return Err(TimecodeError::DroppedLabel);
        }
        let total_minutes = hours as u32 * 60 + minutes as u32;
        let labels = (total_minutes * 60 + seconds as u32) * rate.nominal() as u32 + frames as u32;
        let skipped = dropped * (total_minutes - total_minutes / 10);
        Ok(Self {
            frames: labels - skipped,
            rate,
        })
    }

    /// Parses `HH:MM:SS:FF`. Drop-frame rates also accept `HH:MM:SS;FF`. The frame field has two
    /// or three digits. Surrounding whitespace is not accepted.
    ///
    /// # Errors
    ///
    /// [`TimecodeError::InvalidSyntax`] for any other shape, [`TimecodeError::DropFrameSeparator`]
    /// for `;` at a non-drop-frame rate, and the errors of [`Timecode::from_fields`].
    pub const fn parse(text: &str, rate: FrameRate) -> Result<Self, TimecodeError> {
        let bytes = text.as_bytes();
        if (bytes.len() != 11 && bytes.len() != 12) || bytes[2] != b':' || bytes[5] != b':' {
            return Err(TimecodeError::InvalidSyntax);
        }
        let drop_separator = match bytes[8] {
            b':' => false,
            b';' => true,
            _ => return Err(TimecodeError::InvalidSyntax),
        };
        let (Some(hours), Some(minutes), Some(seconds), Some(frames)) = (
            digits(bytes, 0, 2),
            digits(bytes, 3, 5),
            digits(bytes, 6, 8),
            digits(bytes, 9, bytes.len()),
        ) else {
            return Err(TimecodeError::InvalidSyntax);
        };
        if drop_separator && !rate.is_drop_frame() {
            return Err(TimecodeError::DropFrameSeparator);
        }
        if frames > u8::MAX as u16 {
            return Err(TimecodeError::FieldOutOfRange);
        }
        Self::from_fields(
            Fields {
                hours: hours as u8,
                minutes: minutes as u8,
                seconds: seconds as u8,
                frames: frames as u8,
            },
            rate,
        )
    }

    /// The position at `seconds` after midnight, rounded to a whole frame.
    ///
    /// # Errors
    ///
    /// [`TimecodeError::FrameOutOfRange`] when the rounded frame falls outside the day.
    pub const fn from_seconds(
        seconds: Ratio,
        rate: FrameRate,
        rounding: Rounding,
    ) -> Result<Self, TimecodeError> {
        let fps = rate.frames_per_second();
        let numer = seconds.numer() as u128 * fps.numer() as u128;
        let denom = seconds.denom() as u128 * fps.denom() as u128;
        Self::from_frame_count(rounding.divide(numer, denom), rate)
    }

    /// The position of sample `samples` at `sample_rate` samples per second, rounded to a whole
    /// frame. Pass a pulled sample rate from [`Pull::apply`](crate::Pull::apply) for pulled audio.
    ///
    /// # Errors
    ///
    /// [`TimecodeError::ZeroSampleRate`] for a zero `sample_rate`, [`TimecodeError::Overflow`]
    /// when the exact product does not fit in 128 bits, and [`TimecodeError::FrameOutOfRange`]
    /// when the rounded frame falls outside the day.
    pub const fn from_samples(
        samples: u64,
        sample_rate: Ratio,
        rate: FrameRate,
        rounding: Rounding,
    ) -> Result<Self, TimecodeError> {
        if sample_rate.numer() == 0 {
            return Err(TimecodeError::ZeroSampleRate);
        }
        let fps = rate.frames_per_second();
        let Some(numer) =
            (samples as u128 * sample_rate.denom() as u128).checked_mul(fps.numer() as u128)
        else {
            return Err(TimecodeError::Overflow);
        };
        let denom = sample_rate.numer() as u128 * fps.denom() as u128;
        Self::from_frame_count(rounding.divide(numer, denom), rate)
    }

    /// The frame count from `00:00:00:00`.
    pub const fn frames(self) -> u32 {
        self.frames
    }

    /// The frame rate.
    pub const fn rate(self) -> FrameRate {
        self.rate
    }

    /// The label fields of this position.
    pub const fn fields(self) -> Fields {
        let rate = self.rate;
        let nominal = rate.nominal() as u32;
        let dropped = rate.dropped_per_minute();
        let mut labels = self.frames;
        if dropped > 0 {
            let tens = labels / rate.frames_per_ten_minutes();
            let remaining = labels % rate.frames_per_ten_minutes();
            // The first minute of each ten-minute block has no skipped labels.
            let skipped_minutes = if remaining < dropped {
                0
            } else {
                (remaining - dropped) / rate.frames_per_minute()
            };
            labels += dropped * (tens * 9 + skipped_minutes);
        }
        let seconds = labels / nominal;
        let minutes = seconds / 60;
        Fields {
            hours: (minutes / 60) as u8,
            minutes: (minutes % 60) as u8,
            seconds: (seconds % 60) as u8,
            frames: (labels % nominal) as u8,
        }
    }

    /// The position `delta` frames later, or earlier for a negative `delta`, wrapping at midnight.
    pub const fn wrapping_add_frames(self, delta: i64) -> Self {
        let day = self.rate.frames_per_day() as i64;
        let frames = (self.frames as i64 + delta.rem_euclid(day)) % day;
        Self {
            frames: frames as u32,
            rate: self.rate,
        }
    }

    /// The exact time since midnight in seconds.
    ///
    /// ```
    /// use smpte_timecode::{FrameRate, Ratio, Timecode};
    ///
    /// let hour = Timecode::parse("01:00:00:00", FrameRate::FPS_23_976)?;
    /// assert_eq!(hour.to_seconds(), Ratio::new(18_018, 5)?); // 3603.6 s
    /// # Ok::<(), smpte_timecode::TimecodeError>(())
    /// ```
    pub const fn to_seconds(self) -> Ratio {
        let fps = self.rate.frames_per_second();
        Ratio::reduced(self.frames as u64 * fps.denom(), fps.numer())
    }

    /// The sample at this position for `sample_rate` samples per second, rounded to a whole
    /// sample. NTSC rates rarely land on a whole sample, so the rounding is part of the call.
    ///
    /// ```
    /// use smpte_timecode::{FrameRate, Ratio, Rounding, Timecode};
    ///
    /// let hour = Timecode::parse("01:00:00;00", FrameRate::FPS_29_97_DF)?;
    /// let rate = Ratio::from_integer(48_000);
    /// // Exactly 172 799 827.2 samples.
    /// assert_eq!(hour.to_samples(rate, Rounding::Down)?, 172_799_827);
    /// assert_eq!(hour.to_samples(rate, Rounding::Up)?, 172_799_828);
    /// # Ok::<(), smpte_timecode::TimecodeError>(())
    /// ```
    ///
    /// # Errors
    ///
    /// [`TimecodeError::Overflow`] when the sample does not fit in `u64`.
    pub const fn to_samples(
        self,
        sample_rate: Ratio,
        rounding: Rounding,
    ) -> Result<u64, TimecodeError> {
        let fps = self.rate.frames_per_second();
        // At most 2^24 frames × 2^10 × 2^64 and 2^17 × 2^64, so neither product overflows.
        let numer = self.frames as u128 * fps.denom() as u128 * sample_rate.numer() as u128;
        let denom = fps.numer() as u128 * sample_rate.denom() as u128;
        let samples = rounding.divide(numer, denom);
        if samples > u64::MAX as u128 {
            return Err(TimecodeError::Overflow);
        }
        Ok(samples as u64)
    }

    const fn from_frame_count(frames: u128, rate: FrameRate) -> Result<Self, TimecodeError> {
        if frames >= rate.frames_per_day() as u128 {
            return Err(TimecodeError::FrameOutOfRange);
        }
        Ok(Self {
            frames: frames as u32,
            rate,
        })
    }
}

impl fmt::Display for Timecode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Fields {
            hours,
            minutes,
            seconds,
            frames,
        } = self.fields();
        let separator = if self.rate.is_drop_frame() { ';' } else { ':' };
        write!(
            f,
            "{hours:02}:{minutes:02}:{seconds:02}{separator}{frames:02}"
        )
    }
}

/// The decimal value of `bytes[start..end]`, or `None` if any byte is not an ASCII digit.
const fn digits(bytes: &[u8], start: usize, end: usize) -> Option<u16> {
    let mut value = 0;
    let mut index = start;
    while index < end {
        let byte = bytes[index];
        if !byte.is_ascii_digit() {
            return None;
        }
        value = value * 10 + (byte - b'0') as u16;
        index += 1;
    }
    Some(value)
}
