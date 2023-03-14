use std::cmp;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

/// Number represents either an integral or a floating point value. It
/// needs to be accompanied with a source of NumberKind that describes
/// the actual type of the value stored within Number.
#[derive(Clone, Debug, Default)]
pub struct Number(u64);

impl Number {
    /// Create an atomic version of the current number
    pub fn to_atomic(&self) -> AtomicNumber {
        AtomicNumber(AtomicU64::new(self.0))
    }

    /// Compares this number to the given other number. Both should be of the same kind.
    pub fn partial_cmp(&self, number_kind: &NumberKind, other: &Number) -> Option<cmp::Ordering> {
        match number_kind {
            NumberKind::I64 => (self.0 as i64).partial_cmp(&(other.0 as i64)),
            NumberKind::F64 => {
                let current = u64_to_f64(self.0);
                let other = u64_to_f64(other.0);
                current.partial_cmp(&other)
            }
            NumberKind::U64 => self.0.partial_cmp(&other.0),
        }
    }

    /// Casts the number to `i64`. May result in data/precision loss.
    pub fn to_i64(&self, number_kind: &NumberKind) -> i64 {
        match number_kind {
            NumberKind::F64 => u64_to_f64(self.0) as i64,
            NumberKind::U64 | NumberKind::I64 => self.0 as i64,
        }
    }

    /// Casts the number to `f64`. May result in data/precision loss.
    pub fn to_f64(&self, number_kind: &NumberKind) -> f64 {
        match number_kind {
            NumberKind::I64 => (self.0 as i64) as f64,
            NumberKind::F64 => u64_to_f64(self.0),
            NumberKind::U64 => self.0 as f64,
        }
    }

    /// Casts the number to `u64`. May result in data/precision loss.
    pub fn to_u64(&self, number_kind: &NumberKind) -> u64 {
        match number_kind {
            NumberKind::F64 => u64_to_f64(self.0) as u64,
            NumberKind::U64 | NumberKind::I64 => self.0,
        }
    }

    /// Checks if this value ia an f64 nan value. Do not use on non-f64 values.
    pub fn is_nan(&self) -> bool {
        u64_to_f64(self.0).is_nan()
    }

    /// `true` if the actual value is less than zero.
    pub fn is_negative(&self, number_kind: &NumberKind) -> bool {
        match number_kind {
            NumberKind::I64 => (self.0 as i64).is_negative(),
            NumberKind::F64 => u64_to_f64(self.0).is_sign_negative(),
            NumberKind::U64 => false,
        }
    }

    /// Return loaded data for debugging purposes
    pub fn to_debug(&self, kind: &NumberKind) -> Box<dyn fmt::Debug> {
        match kind {
            NumberKind::I64 => Box::new(self.0 as i64),
            NumberKind::F64 => Box::new(u64_to_f64(self.0)),
            NumberKind::U64 => Box::new(self.0),
        }
    }
}

/// An atomic version of `Number`
#[derive(Debug, Default)]
pub struct AtomicNumber(AtomicU64);

impl AtomicNumber {
    /// Stores a `Number` into the atomic number.
    pub fn store(&self, val: &Number) {
        self.0.store(val.0, Ordering::Relaxed)
    }

    /// Adds to the current number. Both numbers must be of the same kind.
    ///
    /// This operation wraps around on overflow for `u64` and `i64` types and is
    /// `inf` for `f64`.
    pub fn fetch_add(&self, number_kind: &NumberKind, val: &Number) {
        match number_kind {
            NumberKind::I64 => {
                let mut old = self.0.load(Ordering::Acquire);
                loop {
                    let new = (old as i64).wrapping_add(val.0 as i64) as u64;
                    match self.0.compare_exchange_weak(
                        old,
                        new,
                        Ordering::AcqRel,
                        Ordering::Acquire,
                    ) {
                        Ok(_) => break,
                        Err(x) => old = x,
                    };
                }
            }
            NumberKind::F64 => {
                let mut old = self.0.load(Ordering::Acquire);
                loop {
                    let new = u64_to_f64(old) + u64_to_f64(val.0);
                    match self.0.compare_exchange_weak(
                        old,
                        f64_to_u64(new),
                        Ordering::AcqRel,
                        Ordering::Acquire,
                    ) {
                        Ok(_) => break,
                        Err(x) => old = x,
                    };
                }
            }
            NumberKind::U64 => {
                self.0.fetch_add(val.0, Ordering::AcqRel);
            }
        }
    }

    /// Subtracts from the current number. Both numbers must be of the same kind.
    ///
    /// This operation wraps around on overflow for `u64` and `i64` types and is
    /// `-inf` for `f64`.
    pub fn fetch_sub(&self, number_kind: &NumberKind, val: &Number) {
        match number_kind {
            NumberKind::I64 => {
                let mut old = self.0.load(Ordering::Acquire);
                loop {
                    let new = (old as i64).wrapping_sub(val.0 as i64) as u64;
                    match self.0.compare_exchange_weak(
                        old,
                        new,
                        Ordering::AcqRel,
                        Ordering::Relaxed,
                    ) {
                        Ok(_) => break,
                        Err(x) => old = x,
                    };
                }
            }
            NumberKind::F64 => {
                let mut old = self.0.load(Ordering::Acquire);
                loop {
                    let new = u64_to_f64(old) - u64_to_f64(val.0);
                    match self.0.compare_exchange_weak(
                        old,
                        f64_to_u64(new),
                        Ordering::AcqRel,
                        Ordering::Acquire,
                    ) {
                        Ok(_) => break,
                        Err(x) => old = x,
                    };
                }
            }
            NumberKind::U64 => {
                self.0.fetch_sub(val.0, Ordering::AcqRel);
            }
        }
    }

    /// Loads the current `Number`.
    pub fn load(&self) -> Number {
        Number(self.0.load(Ordering::Relaxed))
    }
}

impl Clone for AtomicNumber {
    fn clone(&self) -> Self {
        AtomicNumber(AtomicU64::new(self.0.load(Ordering::Relaxed)))
    }
}

impl From<f64> for Number {
    fn from(f: f64) -> Self {
        Number(f64_to_u64(f))
    }
}

impl From<i64> for Number {
    fn from(i: i64) -> Self {
        Number(i as u64)
    }
}

impl From<u64> for Number {
    fn from(u: u64) -> Self {
        Number(u)
    }
}

/// A descriptor for the encoded data type of a `Number`
#[derive(Clone, Debug, PartialEq, Hash)]
pub enum NumberKind {
    /// A Number that stores `i64` values.
    I64,
    /// A Number that stores `f64` values.
    F64,
    /// A Number that stores `u64` values.
    U64,
}

impl NumberKind {
    /// Returns the zero value for each kind
    pub fn zero(&self) -> Number {
        match self {
            NumberKind::I64 => 0i64.into(),
            NumberKind::F64 => 0f64.into(),
            NumberKind::U64 => 0u64.into(),
        }
    }

    /// Returns the max value for each kind
    pub fn max(&self) -> Number {
        match self {
            NumberKind::I64 => std::i64::MAX.into(),
            NumberKind::F64 => std::f64::MAX.into(),
            NumberKind::U64 => std::u64::MAX.into(),
        }
    }

    /// Returns the min value for each kind
    pub fn min(&self) -> Number {
        match self {
            NumberKind::I64 => std::i64::MIN.into(),
            NumberKind::F64 => std::f64::MIN.into(),
            NumberKind::U64 => std::u64::MIN.into(),
        }
    }
}

#[inline]
fn u64_to_f64(val: u64) -> f64 {
    f64::from_bits(val)
}

#[inline]
fn f64_to_u64(val: f64) -> u64 {
    f64::to_bits(val)
}
