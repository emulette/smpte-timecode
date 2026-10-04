//! SMPTE timecode arithmetic with exact drop-frame handling, and a MIDI Time Code decoder and
//! encoder.
//!
//! - [`Timecode`] is a frame position within one 24-hour day at a [`FrameRate`]. It parses and
//!   formats labels, wraps at midnight, and converts to and from exact seconds and samples.
//! - [`Ratio`] and [`Pull`] keep NTSC rates, pulled sample rates and seconds exact.
//! - [`MtcDecoder`] turns quarter-frame and full-frame messages into checked positions with their
//!   direction and lock state. [`MtcEncoder`] produces the messages.
//!
//! The crate is `no_std`, does not allocate and has no dependencies. It does not open MIDI ports
//! or keep time: callers pass complete MIDI messages and the time each arrived.
//!
//! ```
//! use smpte_timecode::{FrameRate, Ratio, Rounding, Timecode};
//!
//! let tc = Timecode::parse("00:10:00;00", FrameRate::FPS_29_97_DF)?;
//! assert_eq!(tc.frames(), 17_982);
//! assert_eq!(tc.to_samples(Ratio::from_integer(48_000), Rounding::Nearest)?, 28_799_971);
//! # Ok::<(), smpte_timecode::TimecodeError>(())
//! ```

#![no_std]

mod mtc;
mod timecode;

pub use mtc::{Direction, LockState, Motion, MtcDecoder, MtcEncoder, MtcError, MtcEvent};
pub use timecode::{Fields, FrameRate, Pull, Ratio, Rounding, Timecode, TimecodeError};

/// Compiles and runs the README example as a doctest.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;
