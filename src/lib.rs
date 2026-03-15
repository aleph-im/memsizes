use std::fmt;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// How to round when a conversion isn't an exact integer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rounding {
    Floor,   // toward zero
    Ceil,    // away from zero
    Nearest, // ties to even
}

/// Error for failed conversions (overflow, rounding disallowed, etc.)
#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)]
pub enum MemConvError {
    #[error("conversion overflowed u64 range")]
    Overflow,
    #[error("byte count is not exactly divisible by target unit size")]
    Inexact,
}

mod sealed {
    pub trait Sealed {}
}

/// Core trait all memory-size newtypes implement.
/// The *semantic value* is "count of this unit", stored as an integer.
/// Each type specifies its bytes-per-unit as a `u64` constant.
///
/// This trait is sealed and cannot be implemented outside this crate.
pub trait MemorySize: Sized + Copy + sealed::Sealed {
    /// Number of units (e.g., "5 MiB" => 5).
    fn count(self) -> u64;

    /// Convert value to `f64` (for convenience).
    fn to_f64(self) -> f64 {
        self.count() as f64
    }

    /// Construct from a count of units (no validation besides overflow domain).
    fn from_units(units: u64) -> Self;

    /// Exact bytes per 1 unit of this type (e.g., MiB = 1_048_576).
    const BYTES_PER_UNIT: u64;

    /// Convert to raw bytes as a `Bytes` newtype (checked multiply).
    fn to_bytes(self) -> Result<Bytes, MemConvError> {
        let bytes = self
            .count()
            .checked_mul(Self::BYTES_PER_UNIT)
            .ok_or(MemConvError::Overflow)?;
        Ok(Bytes::from_units(bytes))
    }

    /// Convert to another memory unit with **rounding**.
    ///
    /// This performs: `self.bytes() / T::BYTES_PER_UNIT`, rounded as requested.
    fn to_rounded<T: MemorySize>(self, mode: Rounding) -> Result<T, MemConvError> {
        let b = self.to_bytes()?.count();
        let d = T::BYTES_PER_UNIT;

        let (q, r) = (b / d, b % d);
        let add = match mode {
            Rounding::Floor => 0,
            Rounding::Ceil => (r > 0) as u64,
            Rounding::Nearest => {
                // ties-to-even on q
                let twice_r = r.checked_mul(2).ok_or(MemConvError::Overflow)?;
                if twice_r > d || (twice_r == d && q % 2 == 1) {
                    1
                } else {
                    0
                }
            }
        };

        let units = q.checked_add(add).ok_or(MemConvError::Overflow)?;
        Ok(T::from_units(units))
    }

    /// Convert to another unit **only if exact** (no remainder).
    fn to_exact<T: MemorySize>(self) -> Result<T, MemConvError> {
        let b = self.to_bytes()?.count();
        if b % T::BYTES_PER_UNIT == 0 {
            Ok(T::from_units(b / T::BYTES_PER_UNIT))
        } else {
            Err(MemConvError::Inexact)
        }
    }

    fn checked_add(self, rhs: Self) -> Option<Self> {
        self.count().checked_add(rhs.count()).map(Self::from_units)
    }

    fn checked_sub(self, rhs: Self) -> Option<Self> {
        self.count().checked_sub(rhs.count()).map(Self::from_units)
    }

    fn saturating_add(self, rhs: Self) -> Self {
        Self::from_units(self.count().saturating_add(rhs.count()))
    }

    fn saturating_sub(self, rhs: Self) -> Self {
        Self::from_units(self.count().saturating_sub(rhs.count()))
    }
}

/// Canonical base type: raw bytes.
#[must_use]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Bytes(u64);

impl sealed::Sealed for Bytes {}

impl MemorySize for Bytes {
    #[inline]
    fn count(self) -> u64 {
        self.0
    }
    #[inline]
    fn from_units(units: u64) -> Self {
        Self(units)
    }
    const BYTES_PER_UNIT: u64 = 1;
}

impl fmt::Display for Bytes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} B", self.0)
    }
}

impl From<u64> for Bytes {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

/// Helper macro to declare a new memory unit and implement `MemorySize` + basic From/TryFrom.
macro_rules! mem_unit {
    ($name:ident, $bytes_per_unit:expr, $suffix:expr) => {
        #[must_use]
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
        pub struct $name(u64);
        impl sealed::Sealed for $name {}
        impl MemorySize for $name {
            #[inline]
            fn count(self) -> u64 {
                self.0
            }
            #[inline]
            fn from_units(units: u64) -> Self {
                Self(units)
            }
            const BYTES_PER_UNIT: u64 = $bytes_per_unit;
        }
        impl TryFrom<$name> for Bytes {
            type Error = MemConvError;
            fn try_from(v: $name) -> Result<Bytes, MemConvError> {
                v.to_bytes()
            }
        }
        impl TryFrom<Bytes> for $name {
            type Error = MemConvError;
            fn try_from(b: Bytes) -> Result<Self, Self::Error> {
                if b.count() % Self::BYTES_PER_UNIT == 0 {
                    Ok(Self(b.count() / Self::BYTES_PER_UNIT))
                } else {
                    Err(MemConvError::Inexact)
                }
            }
        }
        impl From<u64> for $name {
            fn from(v: u64) -> Self {
                Self(v)
            }
        }
        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{} {}", self.0, $suffix)
            }
        }
    };
}

// Binary IEC units
mem_unit!(KiB, 1024u64, "KiB");
mem_unit!(MiB, 1024u64 * 1024, "MiB");
mem_unit!(GiB, 1024u64 * 1024 * 1024, "GiB");
mem_unit!(TiB, 1024u64 * 1024 * 1024 * 1024, "TiB");
mem_unit!(PiB, 1024u64 * 1024 * 1024 * 1024 * 1024, "PiB");
mem_unit!(EiB, 1024u64 * 1024 * 1024 * 1024 * 1024 * 1024, "EiB");

// Decimal SI units
mem_unit!(KB, 1000u64, "KB");
mem_unit!(MB, 1000u64 * 1000, "MB");
mem_unit!(GB, 1000u64 * 1000 * 1000, "GB");
mem_unit!(TB, 1000u64 * 1000 * 1000 * 1000, "TB");
mem_unit!(PB, 1000u64 * 1000 * 1000 * 1000 * 1000, "PB");
mem_unit!(EB, 1000u64 * 1000 * 1000 * 1000 * 1000 * 1000, "EB");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_bytes() {
        let m = MiB::from_units(5);
        let b = m.to_bytes().unwrap();
        assert_eq!(b.count(), 5 * MiB::BYTES_PER_UNIT);
        assert_eq!(m, b.to_exact::<MiB>().unwrap());
    }

    #[test]
    fn to_exact_and_rounded() {
        let g = GiB::from_units(2); // 2 GiB
        assert_eq!(g.to_exact::<MiB>().unwrap().count(), 2048);

        let two_gib_in_mb_floor = g.to_rounded::<MB>(Rounding::Floor).unwrap();
        let two_gib_in_mb_ceil = g.to_rounded::<MB>(Rounding::Ceil).unwrap();
        assert!(two_gib_in_mb_ceil.count() >= two_gib_in_mb_floor.count());
    }

    #[test]
    fn overflow_guard() {
        // A very large value that would overflow when multiplied by BYTES_PER_UNIT
        let big = GiB::from_units(u64::MAX / GiB::BYTES_PER_UNIT + 1);
        assert!(matches!(big.to_bytes(), Err(MemConvError::Overflow)));
    }

    #[test]
    fn rounding_nearest() {
        // Rounds down: 1500 bytes / 1024 = 1.46..., nearest is 1
        let b = Bytes::from_units(1500);
        assert_eq!(b.to_rounded::<KiB>(Rounding::Nearest).unwrap().count(), 1);

        // Rounds up: 1800 bytes / 1024 = 1.76..., nearest is 2
        let b = Bytes::from_units(1800);
        assert_eq!(b.to_rounded::<KiB>(Rounding::Nearest).unwrap().count(), 2);

        // Tie, q even (stays): 2560 bytes / 1024 = 2.5, q=2 is even → 2
        let b = Bytes::from_units(2560);
        assert_eq!(b.to_rounded::<KiB>(Rounding::Nearest).unwrap().count(), 2);

        // Tie, q odd (rounds up): 1536 bytes / 1024 = 1.5, q=1 is odd → 2
        let b = Bytes::from_units(1536);
        assert_eq!(b.to_rounded::<KiB>(Rounding::Nearest).unwrap().count(), 2);
    }

    #[test]
    fn try_from_bytes_inexact() {
        let b = Bytes::from_units(1025); // not divisible by 1024
        assert!(matches!(KiB::try_from(b), Err(MemConvError::Inexact)));
    }

    #[test]
    fn try_from_unit_to_bytes_overflow() {
        let big = GiB::from_units(u64::MAX);
        assert!(matches!(Bytes::try_from(big), Err(MemConvError::Overflow)));
    }

    #[test]
    fn test_bytes_checked_add() {
        let size = Bytes::from_units(100);

        // Test adding zero
        assert_eq!(
            size.checked_add(Bytes::from_units(0)),
            Some(Bytes::from_units(100))
        );

        // Test adding non-zero
        assert_eq!(
            size.checked_add(Bytes::from_units(50)),
            Some(Bytes::from_units(150))
        );

        // Test overflow
        let max_size = Bytes::from_units(u64::MAX);
        assert_eq!(max_size.checked_add(Bytes::from_units(1)), None);
    }

    #[test]
    fn test_bytes_checked_sub() {
        let size = Bytes::from_units(100);

        // Test subtracting zero
        assert_eq!(
            size.checked_sub(Bytes::from_units(0)),
            Some(Bytes::from_units(100))
        );

        // Test subtracting non-zero
        assert_eq!(
            size.checked_sub(Bytes::from_units(50)),
            Some(Bytes::from_units(50))
        );

        // Test underflow
        assert_eq!(size.checked_sub(Bytes::from_units(150)), None);
    }

    #[test]
    fn test_bytes_saturating_add() {
        let size = Bytes::from_units(100);

        // Test normal addition
        assert_eq!(
            size.saturating_add(Bytes::from_units(50)),
            Bytes::from_units(150)
        );

        // Test overflow
        let max_size = Bytes::from_units(u64::MAX);
        assert_eq!(
            max_size.saturating_add(Bytes::from_units(1)),
            Bytes::from_units(u64::MAX)
        );
    }

    #[test]
    fn test_bytes_saturating_sub() {
        let size = Bytes::from_units(100);

        // Test normal subtraction
        assert_eq!(
            size.saturating_sub(Bytes::from_units(50)),
            Bytes::from_units(50)
        );

        // Test underflow
        assert_eq!(
            size.saturating_sub(Bytes::from_units(150)),
            Bytes::from_units(0)
        );
    }
}
