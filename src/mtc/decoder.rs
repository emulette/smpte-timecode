use core::time::Duration;

use super::{
    Direction, Motion, MtcError, QUARTER_FRAME, SUB_ID_FULL_FRAME, SUB_ID_TIME_CODE, SYSEX_END,
    SYSEX_START, UNIVERSAL_REAL_TIME, stream_rate,
};
use crate::{Fields, FrameRate, Timecode};

/// Every quarter-frame piece, one bit each.
const ALL_PIECES: u8 = 0xFF;
/// Forward cycles complete two frames after the frame they encode (RFC 6295 B.4).
const FORWARD_CYCLE_LATENCY: i64 = 2;

/// A position decoded from MIDI Time Code.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MtcEvent {
    /// The position. Forward positions are already compensated for the cycle latency.
    pub timecode: Timecode,
    /// How the position was reached.
    pub motion: Motion,
    /// The time the completing message was fed, as passed to [`MtcDecoder::feed`].
    pub at: Duration,
}

/// Whether a decoder is following a timecode source.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LockState {
    /// No position has been decoded since creation or the last [`MtcDecoder::reset`].
    Unlocked,
    /// The last position is recent. Holds the motion that produced it.
    Locked(Motion),
    /// The last position is older than the stale limit, as when the source stops sending.
    Stale,
}

/// Turns MIDI Time Code messages into checked positions.
///
/// Feed every complete MIDI message with the time it arrived. Quarter frames form a position only
/// when all eight pieces arrive consecutively in one direction, so a locate, a lost or repeated
/// piece, a direction change or joining mid-cycle never yields a position that mixes two cycles.
/// Full frames reset the cycle and yield a [`Motion::Located`] position.
///
/// Times are durations since any fixed instant the caller chooses, such as a MIDI backend's
/// timestamp or a monotonic clock. Use one clock per decoder. The decoder never reads a clock,
/// allocates or blocks, so it can run inside a MIDI callback.
///
/// ```
/// use core::time::Duration;
/// use smpte_timecode::{Direction, FrameRate, Motion, MtcDecoder, MtcEncoder, Timecode};
///
/// let start = Timecode::parse("01:00:00:00", FrameRate::FPS_25)?;
/// let mut encoder = MtcEncoder::new(start, Direction::Forward)?;
/// let mut decoder = MtcDecoder::new();
/// let mut events = Vec::new();
/// for (index, message) in encoder.by_ref().take(8).enumerate() {
///     let at = Duration::from_millis(10 * index as u64);
///     events.extend(decoder.feed(&message, at)?);
/// }
/// assert_eq!(events.len(), 1);
/// assert_eq!(events[0].timecode.to_string(), "01:00:00:02");
/// assert_eq!(events[0].motion, Motion::Forward);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Debug)]
pub struct MtcDecoder {
    quarter_frame_timeout: Duration,
    stale_after: Duration,
    rate_hint: Option<FrameRate>,
    pieces: [u8; 8],
    received: u8,
    last_piece: Option<(u8, Duration)>,
    direction: Option<Direction>,
    last_event: Option<(Motion, Duration)>,
}

impl MtcDecoder {
    /// The default longest gap between consecutive quarter frames of one cycle.
    pub const DEFAULT_QUARTER_FRAME_TIMEOUT: Duration = Duration::from_millis(500);
    /// The default age after which the last position is [`LockState::Stale`].
    pub const DEFAULT_STALE_AFTER: Duration = Duration::from_secs(1);

    /// A decoder with the default limits and no rate hint.
    pub const fn new() -> Self {
        Self {
            quarter_frame_timeout: Self::DEFAULT_QUARTER_FRAME_TIMEOUT,
            stale_after: Self::DEFAULT_STALE_AFTER,
            rate_hint: None,
            pieces: [0; 8],
            received: 0,
            last_piece: None,
            direction: None,
            last_event: None,
        }
    }

    /// Sets the longest gap between consecutive quarter frames that still continues a cycle.
    /// A longer gap starts a new cycle.
    #[must_use]
    pub const fn with_quarter_frame_timeout(mut self, timeout: Duration) -> Self {
        self.quarter_frame_timeout = timeout;
        self
    }

    /// Sets the age after which [`MtcDecoder::lock_state`] reports [`LockState::Stale`].
    #[must_use]
    pub const fn with_stale_after(mut self, stale_after: Duration) -> Self {
        self.stale_after = stale_after;
        self
    }

    /// Sets the rate the source is known to run at, such as a DAW session rate.
    ///
    /// The two-bit rate code cannot tell 29.97 from 30, 23.976 from 24, or mark 30 as drop-frame.
    /// While the hint has the same nominal frame count as the incoming code, positions use the
    /// hint. A hint at twice the code's count is a session that sends half-count MTC, as Pro Tools
    /// sends 48, 50, 59.94 and 60 sessions with the codes of 24, 25 and 30: positions then use the
    /// half rate with the hint's 1000/1001 factor and drop-frame counting, such as 29.97 for
    /// 59.94, 29.97 DF for 59.94 DF, 23.976 for 47.952 and 30 DF for 60 DF. They keep the
    /// half-count labels the stream carries; converting them to labels at the session rate is the
    /// caller's. Otherwise the stream wins, because reading its frames at another count would
    /// shift positions or reject frame numbers the hint cannot hold.
    pub const fn set_rate_hint(&mut self, hint: Option<FrameRate>) {
        self.rate_hint = hint;
    }

    /// Decodes one complete MIDI message received at `at`.
    ///
    /// Returns a position when the message completes a quarter-frame cycle or is a full frame,
    /// and `None` for a partial cycle or a message that is not MIDI Time Code.
    ///
    /// # Errors
    ///
    /// [`MtcError::MalformedMessage`] for a quarter frame or full frame with the wrong shape, and
    /// [`MtcError::InvalidTimecode`] when a complete message names a position that does not exist.
    /// A malformed message discards the partial cycle. An invalid complete position consumes the
    /// cycle as a valid one would. Neither error replaces or refreshes the last valid position.
    pub fn feed(&mut self, message: &[u8], at: Duration) -> Result<Option<MtcEvent>, MtcError> {
        match message {
            [QUARTER_FRAME, data] if data & 0x80 == 0 => self.quarter_frame(*data, at),
            [QUARTER_FRAME, ..] => {
                self.clear_cycle();
                Err(MtcError::MalformedMessage)
            }
            [
                SYSEX_START,
                UNIVERSAL_REAL_TIME,
                _,
                SUB_ID_TIME_CODE,
                SUB_ID_FULL_FRAME,
                ..,
            ] => self.full_frame(message, at).map(Some),
            _ => Ok(None),
        }
    }

    /// Whether the decoder is following a source at `now`, on the same clock as
    /// [`MtcDecoder::feed`].
    pub const fn lock_state(&self, now: Duration) -> LockState {
        match self.last_event {
            None => LockState::Unlocked,
            Some((_, at)) if now.saturating_sub(at).as_nanos() > self.stale_after.as_nanos() => {
                LockState::Stale
            }
            Some((motion, _)) => LockState::Locked(motion),
        }
    }

    /// Discards any partial cycle and the last position, as when the source changes. Limits and
    /// the rate hint stay.
    pub const fn reset(&mut self) {
        self.clear_cycle();
        self.last_event = None;
    }

    fn quarter_frame(&mut self, data: u8, at: Duration) -> Result<Option<MtcEvent>, MtcError> {
        let piece = (data >> 4) & 0x07;
        let direction = match self.last_piece {
            Some((last, last_at)) if at.saturating_sub(last_at) < self.quarter_frame_timeout => {
                match (piece + 8 - last) % 8 {
                    1 => Some(Direction::Forward),
                    7 => Some(Direction::Reverse),
                    _ => None,
                }
            }
            _ => None,
        };
        // A gap, skip, repeat or turn around ends the cycle so far.
        if direction.is_none() || (self.direction.is_some() && direction != self.direction) {
            self.received = 0;
        }
        self.direction = direction;
        self.last_piece = Some((piece, at));
        self.pieces[usize::from(piece)] = data & 0x0F;
        self.received |= 1 << piece;

        let (motion, cycle_end) = match direction {
            Some(Direction::Forward) => (Motion::Forward, 7),
            Some(Direction::Reverse) => (Motion::Reverse, 0),
            None => return Ok(None),
        };
        if piece != cycle_end || self.received != ALL_PIECES {
            return Ok(None);
        }
        self.received = 0;

        // Bits the specification reserves are masked: 0000000f, 00ssssss, 00mmmmmm, 0rrh.
        let p = self.pieces;
        let fields = Fields {
            hours: p[6] | ((p[7] & 0x01) << 4),
            minutes: p[4] | ((p[5] & 0x03) << 4),
            seconds: p[2] | ((p[3] & 0x03) << 4),
            frames: p[0] | ((p[1] & 0x01) << 4),
        };
        let encoded = self.timecode(fields, (p[7] >> 1) & 0x03)?;
        let timecode = match motion {
            Motion::Forward => encoded.wrapping_add_frames(FORWARD_CYCLE_LATENCY),
            Motion::Reverse | Motion::Located => encoded,
        };
        Ok(Some(self.record(timecode, motion, at)))
    }

    fn full_frame(&mut self, message: &[u8], at: Duration) -> Result<MtcEvent, MtcError> {
        // A locate interrupts the running stream even when its payload is malformed.
        self.clear_cycle();
        // F0 7F <device> 01 01 0rrhhhhh 00mmmmmm 00ssssss 000fffff F7
        let [
            _,
            _,
            device,
            _,
            _,
            hours,
            minutes,
            seconds,
            frames,
            SYSEX_END,
        ] = *message
        else {
            return Err(MtcError::MalformedMessage);
        };
        if [device, hours, minutes, seconds, frames]
            .iter()
            .any(|byte| byte & 0x80 != 0)
        {
            return Err(MtcError::MalformedMessage);
        }
        let fields = Fields {
            hours: hours & 0x1F,
            minutes: minutes & 0x3F,
            seconds: seconds & 0x3F,
            frames: frames & 0x1F,
        };
        let timecode = self.timecode(fields, (hours >> 5) & 0x03)?;
        Ok(self.record(timecode, Motion::Located, at))
    }

    fn timecode(&self, fields: Fields, code: u8) -> Result<Timecode, MtcError> {
        let stream = stream_rate(code);
        let rate = self
            .rate_hint
            .and_then(|hint| hinted_rate(hint, stream))
            .unwrap_or(stream);
        Timecode::from_fields(fields, rate).map_err(MtcError::InvalidTimecode)
    }

    const fn record(&mut self, timecode: Timecode, motion: Motion, at: Duration) -> MtcEvent {
        self.last_event = Some((motion, at));
        MtcEvent {
            timecode,
            motion,
            at,
        }
    }

    const fn clear_cycle(&mut self) {
        self.received = 0;
        self.last_piece = None;
        self.direction = None;
    }
}

/// The rate positions of a `stream` code use under `hint`: the hint when it counts the stream's
/// frames, or for a hint at twice that count, the stream's count with the hint's 1000/1001 factor
/// and drop-frame counting. `None` when the hint is another session's.
fn hinted_rate(hint: FrameRate, stream: FrameRate) -> Option<FrameRate> {
    if hint.nominal() == stream.nominal() {
        Some(hint)
    } else if u16::from(hint.nominal()) == 2 * u16::from(stream.nominal()) {
        FrameRate::new(stream.nominal(), hint.is_ntsc(), hint.is_drop_frame()).ok()
    } else {
        None
    }
}

impl Default for MtcDecoder {
    fn default() -> Self {
        Self::new()
    }
}
