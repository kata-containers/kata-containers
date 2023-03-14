#![deny(unsafe_code)]

use std::convert::{TryFrom, TryInto};
use std::fmt;
use std::num::{ParseIntError, TryFromIntError};
use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Sub, SubAssign};
use std::str::FromStr;

use libc::rlim_t;

/// Unsigned integer type used for limit values.
///
/// The actual type of [`RawRlim`] can be different on different platforms.
pub type RawRlim = rlim_t;

/// Unsigned integer type used for limit values.
///
/// Arithmetic operations with [`Rlim`] are delegated to the inner [`RawRlim`].
///
/// Arithmetic operation with [`usize`] converts the rhs to [`RawRlim`] and computes the result by two [`RawRlim`] values.
///
/// **Be careful**: The actual type of [`RawRlim`] can be different on different platforms.
///
/// # Panics
///
/// Panics if the usize operand can not be converted to [`RawRlim`].
///
/// Panics in debug mode if arithmetic overflow occurred .
///
/// # Features
/// Enables the feature `serde` to implement `Serialize` and `Deserialize` for [`Rlim`] with the attribute `serde(transparent)`.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Rlim(RawRlim);

impl Rlim {
    /// A value of Rlim indicating no limit.
    pub const INFINITY: Self = Self(libc::RLIM_INFINITY);

    #[cfg(any(
        target_os = "fuchsia",
        any(target_os = "openbsd", target_os = "netbsd"),
        target_os = "emscripten",
        target_os = "linux",
    ))]
    /// A value of type Rlim indicating an unrepresentable saved soft limit.
    pub const SAVED_CUR: Self = Self(libc::RLIM_SAVED_CUR);

    #[cfg(any(
        target_os = "fuchsia",
        any(target_os = "openbsd", target_os = "netbsd"),
        target_os = "emscripten",
        target_os = "linux",
    ))]
    /// A value of type Rlim indicating an unrepresentable saved hard limit.
    pub const SAVED_MAX: Self = Self(libc::RLIM_SAVED_MAX);
}

impl Rlim {
    /// Returns `true` if `self` indicates no limit.
    #[must_use]
    pub const fn is_infinity(self) -> bool {
        self.0 == Self::INFINITY.0
    }

    /// Wraps a raw value of limit as Rlim.
    ///
    /// # Example
    /// ```
    /// # use rlimit::Rlim;
    /// // The integer type is inferred by compiler.
    /// const DEFAULT_LIMIT: Rlim = Rlim::from_raw(42);
    /// ```
    #[inline]
    #[must_use]
    pub const fn from_raw(rlim: RawRlim) -> Self {
        Self(rlim)
    }

    /// Returns a raw value of limit.
    #[inline]
    #[must_use]
    pub const fn as_raw(self) -> RawRlim {
        self.0
    }

    /// Converts usize to Rlim
    /// # Panics
    /// Panics if the usize value can not be converted to [`RawRlim`].
    #[inline]
    #[must_use]
    pub fn from_usize(n: usize) -> Self {
        Self(usize_to_raw(n))
    }

    /// Converts Rlim to usize
    /// # Panics
    /// Panics if the wrapped [`RawRlim`] value can not be converted to usize.
    #[inline]
    #[must_use]
    pub fn as_usize(self) -> usize {
        raw_to_usize(self.0)
    }
}

impl fmt::Debug for Rlim {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        RawRlim::fmt(&(self.0), f)
    }
}

impl fmt::Display for Rlim {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        RawRlim::fmt(&(self.0), f)
    }
}

impl FromStr for Rlim {
    type Err = ParseIntError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(RawRlim::from_str(s)?))
    }
}

impl TryFrom<usize> for Rlim {
    type Error = TryFromIntError;
    fn try_from(n: usize) -> Result<Self, Self::Error> {
        Ok(Self(n.try_into()?))
    }
}

impl TryFrom<Rlim> for usize {
    type Error = TryFromIntError;
    fn try_from(r: Rlim) -> Result<Self, Self::Error> {
        Ok(r.0.try_into()?)
    }
}

#[track_caller]
fn usize_to_raw(n: usize) -> RawRlim {
    match n.try_into() {
        Ok(r) => r,
        Err(e) => panic!(
            "can not convert usize to {}, the number is {}, the error is {}",
            std::any::type_name::<RawRlim>(),
            n,
            e
        ),
    }
}

#[track_caller]
fn raw_to_usize(n: RawRlim) -> usize {
    match n.try_into() {
        Ok(r) => r,
        Err(e) => panic!(
            "can not convert {} to usize, the number is {}, the error is {}",
            std::any::type_name::<RawRlim>(),
            n,
            e
        ),
    }
}

macro_rules! arithmetic_panic {
    ($method:tt, $lhs:expr,$rhs:expr) => {
        panic!(
            "Rlim: arithmetic overflow: method = {}, lhs = {}, rhs = {}, type = {}",
            stringify!($method),
            $lhs,
            $rhs,
            std::any::type_name::<RawRlim>(),
        )
    };
}

macro_rules! impl_arithmetic {
    ($tr:tt, $method:tt,$check:tt) => {
        impl $tr<Rlim> for Rlim {
            type Output = Self;

            #[track_caller]
            fn $method(self, rhs: Self) -> Self::Output {
                if cfg!(debug_assertions) {
                    match (self.0).$check(rhs.0) {
                        Some(x) => Self(x),
                        None => arithmetic_panic!($method, (self.0), rhs.0),
                    }
                } else {
                    Self((self.0).$method(rhs.0))
                }
            }
        }

        impl $tr<usize> for Rlim {
            type Output = Self;

            #[track_caller]
            fn $method(self, rhs: usize) -> Self::Output {
                let rhs = usize_to_raw(rhs);

                if cfg!(debug_assertions) {
                    match (self.0).$check(rhs) {
                        Some(x) => Self(x),
                        None => arithmetic_panic!($method, (self.0), rhs),
                    }
                } else {
                    Self((self.0).$method(rhs))
                }
            }
        }
    };
}

macro_rules! impl_arithmetic_assign{
    ($tr:tt, $method:tt,$op:tt) => {
        impl $tr<Rlim> for Rlim {
            #[track_caller]
            fn $method(&mut self, rhs: Self) {
                *self = *self $op rhs;
            }
        }

        impl $tr<usize> for Rlim {
            #[track_caller]
            fn $method(&mut self, rhs: usize) {
                *self = *self $op rhs;
            }
        }
    }
}

macro_rules! delegate_arithmetic{
    {@checked $($check:tt,)+} => {
        impl Rlim{
            $(
                /// Checked integer arithmetic. Returns None if overflow occurred.
                pub fn $check(self, rhs: Self) -> Option<Self>{
                    (self.0).$check(rhs.0).map(Self)
                }
            )+
        }
    };

    {@wrapping $($wrap:tt,)+} => {
        impl Rlim{
            $(
                /// Wrapping (modular) arithmetic. Wraps around at the boundary of the inner [`RawRlim`].
                #[must_use]
                #[allow(clippy::missing_const_for_fn)] // FIXME: `core::num::<impl u64>::wrapping_div` is not yet stable as a const fn
                pub fn $wrap(self, rhs: Self) -> Self{
                    Self((self.0).$wrap(rhs.0))
                }
            )+
        }
    }
}

impl_arithmetic!(Add, add, checked_add);
impl_arithmetic!(Sub, sub, checked_sub);
impl_arithmetic!(Mul, mul, checked_mul);
impl_arithmetic!(Div, div, checked_div);

impl_arithmetic_assign!(AddAssign, add_assign, +);
impl_arithmetic_assign!(SubAssign, sub_assign, -);
impl_arithmetic_assign!(MulAssign, mul_assign, *);
impl_arithmetic_assign!(DivAssign, div_assign, /);

delegate_arithmetic! {@checked
    checked_add,
    checked_sub,
    checked_mul,
    checked_div,
}

delegate_arithmetic! {@wrapping
    wrapping_add,
    wrapping_sub,
    wrapping_mul,
    wrapping_div,
}
