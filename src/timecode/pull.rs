use super::Ratio;

/// An exact pull-up or pull-down factor for a frame rate or a sample rate.
///
/// Pulling a rate multiplies it by the factor. 0.1% pulls move between film and NTSC speeds and
/// 4% pulls between 24 and 25 frames per second. A pulled rate is usually not a whole number:
/// 48 kHz pulled down 0.1% is 48 000 000/1001 ≈ 47 952.05 Hz, which equipment labels 47.952 kHz.
///
/// ```
/// use smpte_timecode::{FrameRate, Pull, Ratio};
///
/// let pulled = Pull::DOWN_0_1_PERCENT.apply(Ratio::from_integer(48_000)).unwrap();
/// assert_eq!((pulled.numer(), pulled.denom()), (48_000_000, 1001));
///
/// let film_to_pal = Pull::UP_4_PERCENT.apply(FrameRate::FPS_24.frames_per_second()).unwrap();
/// assert_eq!(film_to_pal, FrameRate::FPS_25.frames_per_second());
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Pull(Ratio);

impl Pull {
    /// No change.
    pub const NONE: Self = Self(Ratio::from_integer(1));
    /// 1001/1000, from NTSC speed up to film speed.
    pub const UP_0_1_PERCENT: Self = Self(Ratio::reduced(1001, 1000));
    /// 1000/1001, from film speed down to NTSC speed.
    pub const DOWN_0_1_PERCENT: Self = Self(Ratio::reduced(1000, 1001));
    /// 25/24, from 24 up to 25 frames per second. The exact change is +4.1667%.
    pub const UP_4_PERCENT: Self = Self(Ratio::reduced(25, 24));
    /// 24/25, from 25 down to 24 frames per second.
    pub const DOWN_4_PERCENT: Self = Self(Ratio::reduced(24, 25));

    /// The exact factor the pull multiplies a rate by.
    pub const fn factor(self) -> Ratio {
        self.0
    }

    /// The pull that applies `self` and then `next`, or `None` if the combined factor does not fit
    /// in a [`Ratio`].
    pub const fn then(self, next: Self) -> Option<Self> {
        match self.0.checked_mul(next.0) {
            Some(factor) => Some(Self(factor)),
            None => None,
        }
    }

    /// Pulls `rate`, or returns `None` if the result does not fit in a [`Ratio`].
    pub const fn apply(self, rate: Ratio) -> Option<Ratio> {
        rate.checked_mul(self.0)
    }
}
