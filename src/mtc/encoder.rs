use super::{
    ALL_DEVICES, Direction, MtcError, QUARTER_FRAME, SUB_ID_FULL_FRAME, SUB_ID_TIME_CODE,
    SYSEX_END, SYSEX_START, UNIVERSAL_REAL_TIME, rate_code,
};
use crate::{Ratio, Timecode};

/// Each quarter-frame cycle spans two frames.
const CYCLE_FRAMES: i64 = 2;

/// Produces the quarter-frame messages of a running MIDI Time Code stream.
///
/// The encoder is an endless iterator of two-byte messages. Forward streams send pieces 0 to 7 and
/// reverse streams 7 to 0. Each cycle of eight pieces carries one position, and the next cycle
/// carries the position two frames further in the direction of travel, wrapping at midnight.
///
/// The encoder only builds bytes. Send one message every
/// [`quarter_frame_interval`](MtcEncoder::quarter_frame_interval) on whatever clock and port
/// the application uses, and send [`MtcEncoder::full_frame`] after a locate.
///
/// ```
/// use smpte_timecode::{Direction, FrameRate, MtcEncoder, Timecode};
///
/// let start = Timecode::parse("01:02:03:04", FrameRate::FPS_25)?;
/// let messages: Vec<[u8; 2]> = MtcEncoder::new(start, Direction::Forward)?.take(8).collect();
/// assert_eq!(messages[0], [0xF1, 0x04]); // piece 0: frames, low nibble
/// assert_eq!(messages[7], [0xF1, 0x72]); // piece 7: rate code 1 (25 fps), hours high bit
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Debug)]
pub struct MtcEncoder {
    timecode: Timecode,
    direction: Direction,
    code: u8,
    piece: u8,
}

impl MtcEncoder {
    /// An encoder whose first cycle carries `start`.
    ///
    /// # Errors
    ///
    /// [`MtcError::NoRateCode`] when the rate of `start` has 48 or more frames per second.
    pub const fn new(start: Timecode, direction: Direction) -> Result<Self, MtcError> {
        let code = match rate_code(start.rate()) {
            Ok(code) => code,
            Err(error) => return Err(error),
        };
        let piece = match direction {
            Direction::Forward => 0,
            Direction::Reverse => 7,
        };
        Ok(Self {
            timecode: start,
            direction,
            code,
            piece,
        })
    }

    /// The position the cycle in progress carries. A new cycle starts with the next message
    /// after the last piece of the previous one.
    pub const fn timecode(&self) -> Timecode {
        self.timecode
    }

    /// The direction of the stream.
    pub const fn direction(&self) -> Direction {
        self.direction
    }

    /// The exact time between quarter frames in seconds: a quarter of a frame.
    pub const fn quarter_frame_interval(&self) -> Ratio {
        let fps = self.timecode.rate().frames_per_second();
        Ratio::reduced(fps.denom(), fps.numer() * 4)
    }

    /// The full-frame SysEx message for `timecode`, addressed to all devices.
    ///
    /// # Errors
    ///
    /// [`MtcError::NoRateCode`] when the rate of `timecode` has 48 or more frames per second.
    pub const fn full_frame(timecode: Timecode) -> Result<[u8; 10], MtcError> {
        let code = match rate_code(timecode.rate()) {
            Ok(code) => code,
            Err(error) => return Err(error),
        };
        let fields = timecode.fields();
        Ok([
            SYSEX_START,
            UNIVERSAL_REAL_TIME,
            ALL_DEVICES,
            SUB_ID_TIME_CODE,
            SUB_ID_FULL_FRAME,
            (code << 5) | fields.hours,
            fields.minutes,
            fields.seconds,
            fields.frames,
            SYSEX_END,
        ])
    }
}

impl Iterator for MtcEncoder {
    type Item = [u8; 2];

    fn next(&mut self) -> Option<Self::Item> {
        let fields = self.timecode.fields();
        let value = match self.piece {
            0 => fields.frames & 0x0F,
            1 => fields.frames >> 4,
            2 => fields.seconds & 0x0F,
            3 => fields.seconds >> 4,
            4 => fields.minutes & 0x0F,
            5 => fields.minutes >> 4,
            6 => fields.hours & 0x0F,
            _ => (fields.hours >> 4) | (self.code << 1),
        };
        let message = [QUARTER_FRAME, (self.piece << 4) | value];
        match (self.direction, self.piece) {
            (Direction::Forward, 7) => {
                self.piece = 0;
                self.timecode = self.timecode.wrapping_add_frames(CYCLE_FRAMES);
            }
            (Direction::Forward, _) => self.piece += 1,
            (Direction::Reverse, 0) => {
                self.piece = 7;
                self.timecode = self.timecode.wrapping_add_frames(-CYCLE_FRAMES);
            }
            (Direction::Reverse, _) => self.piece -= 1,
        }
        Some(message)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (usize::MAX, None)
    }
}
