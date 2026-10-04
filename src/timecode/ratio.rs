use core::cmp::Ordering;

use super::TimecodeError;

/// An exact non-negative rational number, always stored in lowest terms.
///
/// Seconds, frame rates, sample rates and pull factors are ratios so that NTSC rates such as
/// 30000/1001 frames per second never pick up floating-point error.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Ratio {
    numer: u64,
    denom: u64,
}

impl Ratio {
    /// Creates `numer / denom` in lowest terms.
    ///
    /// # Errors
    ///
    /// [`TimecodeError::ZeroDenominator`] when `denom` is zero.
    pub const fn new(numer: u64, denom: u64) -> Result<Self, TimecodeError> {
        if denom == 0 {
            return Err(TimecodeError::ZeroDenominator);
        }
        Ok(Self::reduced(numer, denom))
    }

    /// Creates the whole number `value`.
    pub const fn from_integer(value: u64) -> Self {
        Self {
            numer: value,
            denom: 1,
        }
    }

    /// The numerator in lowest terms.
    pub const fn numer(self) -> u64 {
        self.numer
    }

    /// The denominator in lowest terms. Never zero.
    pub const fn denom(self) -> u64 {
        self.denom
    }

    /// Multiplies two ratios, or returns `None` when the reduced result does not fit in `u64`.
    pub const fn checked_mul(self, rhs: Self) -> Option<Self> {
        // Cross-reducing first keeps every exact product that fits from overflowing on the way.
        let left = gcd(self.numer, rhs.denom);
        let right = gcd(rhs.numer, self.denom);
        let numer = (self.numer / left) as u128 * (rhs.numer / right) as u128;
        let denom = (self.denom / right) as u128 * (rhs.denom / left) as u128;
        if numer > u64::MAX as u128 || denom > u64::MAX as u128 {
            return None;
        }
        Some(Self {
            numer: numer as u64,
            denom: denom as u64,
        })
    }

    /// The nearest `f64`, for display and plotting. Arithmetic in this crate never uses it.
    pub fn to_f64(self) -> f64 {
        self.numer as f64 / self.denom as f64
    }

    pub(crate) const fn reduced(numer: u64, denom: u64) -> Self {
        let divisor = gcd(numer, denom);
        Self {
            numer: numer / divisor,
            denom: denom / divisor,
        }
    }
}

impl Ord for Ratio {
    fn cmp(&self, other: &Self) -> Ordering {
        (u128::from(self.numer) * u128::from(other.denom))
            .cmp(&(u128::from(other.numer) * u128::from(self.denom)))
    }
}

impl PartialOrd for Ratio {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// How a conversion picks a whole number when the exact result falls between two.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Rounding {
    /// The largest whole number not above the exact result.
    Down,
    /// The smallest whole number not below the exact result.
    Up,
    /// The closest whole number, with exact halves rounded up.
    Nearest,
}

impl Rounding {
    /// Divides `numer` by a non-zero `denom` with this rounding.
    pub(crate) const fn divide(self, numer: u128, denom: u128) -> u128 {
        let quotient = numer / denom;
        let remainder = numer % denom;
        let round_up = match self {
            Self::Down => false,
            Self::Up => remainder != 0,
            // remainder >= denom - remainder, written so neither side can overflow.
            Self::Nearest => remainder >= denom - remainder,
        };
        if round_up { quotient + 1 } else { quotient }
    }
}

const fn gcd(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        let rest = a % b;
        a = b;
        b = rest;
    }
    a
}
