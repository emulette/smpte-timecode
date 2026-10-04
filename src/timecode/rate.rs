use super::{Ratio, TimecodeError};

/// A timecode frame rate: a nominal frame count, whether frames run 1000/1001 slower (the NTSC
/// family), and whether labels use drop-frame counting.
///
/// Only the 19 combinations in [`FrameRate::ALL`] exist. Drop-frame counting needs a nominal rate
/// of 30, 60 or 120 and skips 2, 4 or 8 labels at the start of every minute not divisible by ten.
/// 29.97 DF and 59.94 DF follow SMPTE ST 12-1. 30 DF, 60 DF, 120 DF and 119.88 DF follow the same
/// rule as Pro Tools labels them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FrameRate {
    nominal: u8,
    ntsc: bool,
    drop_frame: bool,
}

impl FrameRate {
    /// 24000/1001 frames per second, counted as 24.
    pub const FPS_23_976: Self = Self::known(24, true, false);
    /// 24 frames per second.
    pub const FPS_24: Self = Self::known(24, false, false);
    /// 25 frames per second.
    pub const FPS_25: Self = Self::known(25, false, false);
    /// 30000/1001 frames per second, counted as 30 without dropping labels.
    pub const FPS_29_97: Self = Self::known(30, true, false);
    /// 30000/1001 frames per second with drop-frame labels.
    pub const FPS_29_97_DF: Self = Self::known(30, true, true);
    /// 30 frames per second.
    pub const FPS_30: Self = Self::known(30, false, false);
    /// 30 frames per second with drop-frame labels.
    pub const FPS_30_DF: Self = Self::known(30, false, true);
    /// 48000/1001 frames per second, counted as 48.
    pub const FPS_47_952: Self = Self::known(48, true, false);
    /// 48 frames per second.
    pub const FPS_48: Self = Self::known(48, false, false);
    /// 50 frames per second.
    pub const FPS_50: Self = Self::known(50, false, false);
    /// 60000/1001 frames per second, counted as 60 without dropping labels.
    pub const FPS_59_94: Self = Self::known(60, true, false);
    /// 60000/1001 frames per second with drop-frame labels.
    pub const FPS_59_94_DF: Self = Self::known(60, true, true);
    /// 60 frames per second.
    pub const FPS_60: Self = Self::known(60, false, false);
    /// 60 frames per second with drop-frame labels.
    pub const FPS_60_DF: Self = Self::known(60, false, true);
    /// 100 frames per second.
    pub const FPS_100: Self = Self::known(100, false, false);
    /// 120000/1001 frames per second, counted as 120 without dropping labels.
    pub const FPS_119_88: Self = Self::known(120, true, false);
    /// 120000/1001 frames per second with drop-frame labels.
    pub const FPS_119_88_DF: Self = Self::known(120, true, true);
    /// 120 frames per second.
    pub const FPS_120: Self = Self::known(120, false, false);
    /// 120 frames per second with drop-frame labels.
    pub const FPS_120_DF: Self = Self::known(120, false, true);

    /// Every supported frame rate.
    pub const ALL: [Self; 19] = [
        Self::FPS_23_976,
        Self::FPS_24,
        Self::FPS_25,
        Self::FPS_29_97,
        Self::FPS_29_97_DF,
        Self::FPS_30,
        Self::FPS_30_DF,
        Self::FPS_47_952,
        Self::FPS_48,
        Self::FPS_50,
        Self::FPS_59_94,
        Self::FPS_59_94_DF,
        Self::FPS_60,
        Self::FPS_60_DF,
        Self::FPS_100,
        Self::FPS_119_88,
        Self::FPS_119_88_DF,
        Self::FPS_120,
        Self::FPS_120_DF,
    ];

    /// Looks up the rate with `nominal` labels per second, running 1000/1001 slower when `ntsc`
    /// is set, and counting drop-frame labels when `drop_frame` is set.
    ///
    /// # Errors
    ///
    /// [`TimecodeError::UnsupportedFrameRate`] when the combination is not in [`FrameRate::ALL`],
    /// such as 25 DF or 50 with the 1000/1001 factor.
    pub const fn new(nominal: u8, ntsc: bool, drop_frame: bool) -> Result<Self, TimecodeError> {
        let mut index = 0;
        while index < Self::ALL.len() {
            let rate = Self::ALL[index];
            if rate.nominal == nominal && rate.ntsc == ntsc && rate.drop_frame == drop_frame {
                return Ok(rate);
            }
            index += 1;
        }
        Err(TimecodeError::UnsupportedFrameRate)
    }

    /// The number of frame labels in one timecode second.
    pub const fn nominal(self) -> u8 {
        self.nominal
    }

    /// Whether frames run at 1000/1001 of the nominal rate, as 23.976 and 29.97 do.
    pub const fn is_ntsc(self) -> bool {
        self.ntsc
    }

    /// Whether labels use drop-frame counting.
    pub const fn is_drop_frame(self) -> bool {
        self.drop_frame
    }

    /// The exact number of frames per real-time second.
    pub const fn frames_per_second(self) -> Ratio {
        if self.ntsc {
            Ratio::reduced(self.nominal as u64 * 1000, 1001)
        } else {
            Ratio::from_integer(self.nominal as u64)
        }
    }

    /// The number of frames from `00:00:00:00` up to, but not including, the next midnight.
    pub const fn frames_per_day(self) -> u32 {
        self.frames_per_ten_minutes() * 6 * 24
    }

    /// Labels skipped at the start of each minute not divisible by ten. Zero without drop-frame.
    pub(crate) const fn dropped_per_minute(self) -> u32 {
        if self.drop_frame {
            self.nominal as u32 / 15
        } else {
            0
        }
    }

    pub(crate) const fn frames_per_minute(self) -> u32 {
        self.nominal as u32 * 60 - self.dropped_per_minute()
    }

    pub(crate) const fn frames_per_ten_minutes(self) -> u32 {
        // The first minute of every ten keeps all of its labels.
        self.nominal as u32 * 600 - self.dropped_per_minute() * 9
    }

    const fn known(nominal: u8, ntsc: bool, drop_frame: bool) -> Self {
        Self {
            nominal,
            ntsc,
            drop_frame,
        }
    }
}
