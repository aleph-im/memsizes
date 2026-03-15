//! Type-safe memory size newtypes with checked conversions and arithmetic.
//!
//! Each unit (`Bytes`, `KiB`, `MiB`, `GiB`, etc.) is a distinct type so the
//! compiler prevents mixing up binary and decimal sizes or raw byte counts.
//!
//! ```
//! use memsizes::{GiB, MiB, MB, MemorySize, Rounding};
//!
//! let mem = GiB::from(2);
//!
//! // Exact conversion (binary → binary)
//! let mib: MiB = mem.to_exact().unwrap();
//! assert_eq!(mib.count(), 2048);
//!
//! // Rounded conversion (binary → decimal)
//! let mb = mem.to_rounded::<MB>(Rounding::Ceil).unwrap();
//!
//! // Checked arithmetic (both operands must be the same type)
//! let total = mib.checked_add(MiB::from(512)).unwrap();
//! assert_eq!(total.count(), 2560);
//! ```

#![warn(missing_docs)]

use std::fmt;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// How to round when a conversion isn't an exact integer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Rounding {
    /// Round toward zero.
    Floor,
    /// Round away from zero.
    Ceil,
    /// Round to nearest, ties to even.
    Nearest,
}

/// Error for failed conversions (overflow, rounding disallowed, etc.)
#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)]
pub enum MemConvError {
    /// The intermediate byte value overflowed `u64`.
    #[error("conversion overflowed u64 range")]
    Overflow,
    /// The byte count is not exactly divisible by the target unit size.
    #[error("byte count is not exactly divisible by target unit size")]
    Inexact,
}

mod sealed {
    /// Sealing trait — prevents external implementations of [`super::MemorySize`].
    pub trait Sealed {}
}

/// Core trait all memory-size newtypes implement.
/// The *semantic value* is "count of this unit", stored as an integer.
/// Each type specifies its bytes-per-unit as a `u64` constant.
///
/// This trait is sealed and cannot be implemented outside this crate.
pub trait MemorySize: Sized + Copy + From<u64> + sealed::Sealed {
    /// Number of units (e.g., "5 MiB" => 5).
    fn count(self) -> u64;

    /// Convert value to `f64` (for convenience).
    fn to_f64(self) -> f64 {
        self.count() as f64
    }

    /// Exact bytes per 1 unit of this type (e.g., MiB = 1_048_576).
    const BYTES_PER_UNIT: u64;

    /// Convert to raw bytes as a `Bytes` newtype (checked multiply).
    fn to_bytes(self) -> Result<Bytes, MemConvError> {
        let bytes = self
            .count()
            .checked_mul(Self::BYTES_PER_UNIT)
            .ok_or(MemConvError::Overflow)?;
        Ok(Bytes::from(bytes))
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
        Ok(T::from(units))
    }

    /// Convert to another unit **only if exact** (no remainder).
    fn to_exact<T: MemorySize>(self) -> Result<T, MemConvError> {
        let b = self.to_bytes()?.count();
        if b % T::BYTES_PER_UNIT == 0 {
            Ok(T::from(b / T::BYTES_PER_UNIT))
        } else {
            Err(MemConvError::Inexact)
        }
    }

    /// Checked addition. Returns `None` on overflow.
    fn checked_add(self, rhs: Self) -> Option<Self> {
        self.count().checked_add(rhs.count()).map(Self::from)
    }

    /// Checked subtraction. Returns `None` on underflow.
    fn checked_sub(self, rhs: Self) -> Option<Self> {
        self.count().checked_sub(rhs.count()).map(Self::from)
    }

    /// Saturating addition. Clamps at `u64::MAX` on overflow.
    fn saturating_add(self, rhs: Self) -> Self {
        Self::from(self.count().saturating_add(rhs.count()))
    }

    /// Saturating subtraction. Clamps at zero on underflow.
    fn saturating_sub(self, rhs: Self) -> Self {
        Self::from(self.count().saturating_sub(rhs.count()))
    }
}

/// Canonical base type: raw bytes.
#[must_use]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Default)]
pub struct Bytes(u64);

impl Bytes {
    /// Returns the number of bytes.
    #[inline]
    pub fn count(self) -> u64 {
        self.0
    }
}

impl sealed::Sealed for Bytes {}

impl MemorySize for Bytes {
    #[inline]
    fn count(self) -> u64 {
        self.0
    }
    const BYTES_PER_UNIT: u64 = 1;

    #[inline]
    fn to_bytes(self) -> Result<Bytes, MemConvError> {
        Ok(self)
    }
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

impl From<Bytes> for u64 {
    fn from(b: Bytes) -> Self {
        b.0
    }
}

/// Helper macro to declare a new memory unit and implement `MemorySize` + basic From/TryFrom.
macro_rules! mem_unit {
    ($name:ident, $bytes_per_unit:expr, $suffix:expr, $doc:expr) => {
        #[doc = $doc]
        #[must_use]
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
        pub struct $name(u64);
        impl $name {
            /// Returns the unit count (e.g., `MiB::from(5u64).count() == 5`).
            #[inline]
            pub fn count(self) -> u64 {
                self.0
            }
        }
        impl sealed::Sealed for $name {}
        impl MemorySize for $name {
            #[inline]
            fn count(self) -> u64 {
                self.0
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
        impl From<$name> for u64 {
            fn from(v: $name) -> Self {
                v.0
            }
        }
        impl Default for $name {
            fn default() -> Self {
                Self(0)
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
mem_unit!(KiB, 1024u64, "KiB", "Kibibytes (1024 bytes).");
mem_unit!(
    MiB,
    1024u64 * 1024,
    "MiB",
    "Mebibytes (1024\u{00B2} bytes)."
);
mem_unit!(
    GiB,
    1024u64 * 1024 * 1024,
    "GiB",
    "Gibibytes (1024\u{00B3} bytes)."
);
mem_unit!(
    TiB,
    1024u64 * 1024 * 1024 * 1024,
    "TiB",
    "Tebibytes (1024\u{2074} bytes)."
);
mem_unit!(
    PiB,
    1024u64 * 1024 * 1024 * 1024 * 1024,
    "PiB",
    "Pebibytes (1024\u{2075} bytes)."
);
mem_unit!(
    EiB,
    1024u64 * 1024 * 1024 * 1024 * 1024 * 1024,
    "EiB",
    "Exbibytes (1024\u{2076} bytes)."
);

// Decimal SI units
mem_unit!(KB, 1000u64, "KB", "Kilobytes (1000 bytes).");
mem_unit!(MB, 1000u64 * 1000, "MB", "Megabytes (1000\u{00B2} bytes).");
mem_unit!(
    GB,
    1000u64 * 1000 * 1000,
    "GB",
    "Gigabytes (1000\u{00B3} bytes)."
);
mem_unit!(
    TB,
    1000u64 * 1000 * 1000 * 1000,
    "TB",
    "Terabytes (1000\u{2074} bytes)."
);
mem_unit!(
    PB,
    1000u64 * 1000 * 1000 * 1000 * 1000,
    "PB",
    "Petabytes (1000\u{2075} bytes)."
);
mem_unit!(
    EB,
    1000u64 * 1000 * 1000 * 1000 * 1000 * 1000,
    "EB",
    "Exabytes (1000\u{2076} bytes)."
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_bytes() {
        let m = MiB::from(5);
        let b = m.to_bytes().unwrap();
        assert_eq!(b.count(), 5 * MiB::BYTES_PER_UNIT);
        assert_eq!(m, b.to_exact::<MiB>().unwrap());
    }

    #[test]
    fn to_exact_and_rounded() {
        let g = GiB::from(2); // 2 GiB
        assert_eq!(g.to_exact::<MiB>().unwrap().count(), 2048);

        let two_gib_in_mb_floor = g.to_rounded::<MB>(Rounding::Floor).unwrap();
        let two_gib_in_mb_ceil = g.to_rounded::<MB>(Rounding::Ceil).unwrap();
        assert!(two_gib_in_mb_ceil.count() >= two_gib_in_mb_floor.count());
    }

    #[test]
    fn overflow_guard() {
        // A very large value that would overflow when multiplied by BYTES_PER_UNIT
        let big = GiB::from(u64::MAX / GiB::BYTES_PER_UNIT + 1);
        assert!(matches!(big.to_bytes(), Err(MemConvError::Overflow)));
    }

    #[test]
    fn rounding_nearest() {
        // Rounds down: 1500 bytes / 1024 = 1.46..., nearest is 1
        let b = Bytes::from(1500);
        assert_eq!(b.to_rounded::<KiB>(Rounding::Nearest).unwrap().count(), 1);

        // Rounds up: 1800 bytes / 1024 = 1.76..., nearest is 2
        let b = Bytes::from(1800);
        assert_eq!(b.to_rounded::<KiB>(Rounding::Nearest).unwrap().count(), 2);

        // Tie, q even (stays): 2560 bytes / 1024 = 2.5, q=2 is even → 2
        let b = Bytes::from(2560);
        assert_eq!(b.to_rounded::<KiB>(Rounding::Nearest).unwrap().count(), 2);

        // Tie, q odd (rounds up): 1536 bytes / 1024 = 1.5, q=1 is odd → 2
        let b = Bytes::from(1536);
        assert_eq!(b.to_rounded::<KiB>(Rounding::Nearest).unwrap().count(), 2);
    }

    #[test]
    fn try_from_bytes_inexact() {
        let b = Bytes::from(1025); // not divisible by 1024
        assert!(matches!(KiB::try_from(b), Err(MemConvError::Inexact)));
    }

    #[test]
    fn try_from_unit_to_bytes_overflow() {
        let big = GiB::from(u64::MAX);
        assert!(matches!(Bytes::try_from(big), Err(MemConvError::Overflow)));
    }

    #[test]
    fn test_bytes_checked_add() {
        let size = Bytes::from(100);

        // Test adding zero
        assert_eq!(size.checked_add(Bytes::from(0)), Some(Bytes::from(100)));

        // Test adding non-zero
        assert_eq!(size.checked_add(Bytes::from(50)), Some(Bytes::from(150)));

        // Test overflow
        let max_size = Bytes::from(u64::MAX);
        assert_eq!(max_size.checked_add(Bytes::from(1)), None);
    }

    #[test]
    fn test_bytes_checked_sub() {
        let size = Bytes::from(100);

        // Test subtracting zero
        assert_eq!(size.checked_sub(Bytes::from(0)), Some(Bytes::from(100)));

        // Test subtracting non-zero
        assert_eq!(size.checked_sub(Bytes::from(50)), Some(Bytes::from(50)));

        // Test underflow
        assert_eq!(size.checked_sub(Bytes::from(150)), None);
    }

    #[test]
    fn test_bytes_saturating_add() {
        let size = Bytes::from(100);

        // Test normal addition
        assert_eq!(size.saturating_add(Bytes::from(50)), Bytes::from(150));

        // Test overflow
        let max_size = Bytes::from(u64::MAX);
        assert_eq!(
            max_size.saturating_add(Bytes::from(1)),
            Bytes::from(u64::MAX)
        );
    }

    #[test]
    fn test_bytes_saturating_sub() {
        let size = Bytes::from(100);

        // Test normal subtraction
        assert_eq!(size.saturating_sub(Bytes::from(50)), Bytes::from(50));

        // Test underflow
        assert_eq!(size.saturating_sub(Bytes::from(150)), Bytes::from(0));
    }

    #[test]
    fn decimal_units_smoke() {
        let mb = MB::from(5);
        assert_eq!(mb.to_bytes().unwrap().count(), 5_000_000);

        let gb = GB::from(2);
        assert_eq!(gb.to_exact::<MB>().unwrap().count(), 2_000);

        let tb = TB::from(1);
        assert_eq!(tb.to_exact::<GB>().unwrap().count(), 1_000);

        let kb = KB::from(3_000);
        assert_eq!(kb.to_exact::<MB>().unwrap().count(), 3);
    }

    #[test]
    fn display_formatting() {
        assert_eq!(format!("{}", Bytes::from(42)), "42 B");
        assert_eq!(format!("{}", KiB::from(10)), "10 KiB");
        assert_eq!(format!("{}", MiB::from(5)), "5 MiB");
        assert_eq!(format!("{}", GiB::from(1)), "1 GiB");
        assert_eq!(format!("{}", TiB::from(2)), "2 TiB");
        assert_eq!(format!("{}", PiB::from(3)), "3 PiB");
        assert_eq!(format!("{}", EiB::from(4)), "4 EiB");
        assert_eq!(format!("{}", KB::from(7)), "7 KB");
        assert_eq!(format!("{}", MB::from(8)), "8 MB");
        assert_eq!(format!("{}", GB::from(9)), "9 GB");
        assert_eq!(format!("{}", TB::from(10)), "10 TB");
        assert_eq!(format!("{}", PB::from(11)), "11 PB");
        assert_eq!(format!("{}", EB::from(12)), "12 EB");
    }

    #[test]
    fn default_is_zero() {
        assert_eq!(Bytes::default().count(), 0);
        assert_eq!(KiB::default().count(), 0);
        assert_eq!(MiB::default().count(), 0);
        assert_eq!(GiB::default().count(), 0);
        assert_eq!(MB::default().count(), 0);
        assert_eq!(GB::default().count(), 0);
    }

    #[test]
    fn into_u64() {
        assert_eq!(u64::from(Bytes::from(99)), 99);
        assert_eq!(u64::from(MiB::from(42)), 42);
        assert_eq!(u64::from(GB::from(7)), 7);
    }

    #[test]
    fn identity_conversion() {
        assert_eq!(GiB::from(5).to_exact::<GiB>().unwrap().count(), 5);
        assert_eq!(MB::from(100).to_exact::<MB>().unwrap().count(), 100);
        assert_eq!(Bytes::from(1024).to_exact::<Bytes>().unwrap().count(), 1024);
    }
}
