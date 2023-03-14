//! # Traits to represent generic nonzero integer types
//! [![Build Status](https://travis-ci.com/antifuchs/nonzero_ext.svg?branch=master)](https://travis-ci.com/antifuchs/nonzero_ext) [![Docs](https://docs.rs/nonzero_ext/badge.svg)](https://docs.rs/nonzero_ext)
//!
//! Rust ships with non-zero integer types now, which let programmers
//! promise (memory-savingly!) that a number can never be zero. That's
//! great, but sadly the standard library has not got a whole lot of
//! tools to help you use them ergonomically.
//!
//! ## A macro for non-zero constant literals
//!
//! Creating and handling constant literals is neat, but the standard
//! library (and the rust parser at the moment) have no affordances to
//! easily create values of `num::NonZeroU*` types from constant
//! literals. This crate ships a `nonzero!` macro that lets you write
//! `nonzero!(20u32)`, which checks at compile time that the constant
//! being converted is non-zero, instead of the cumbersome (and
//! runtime-checked!)  `NonZeroU32::new(20).unwrap()`.
//!
//! ## Traits for generic non-zeroness
//!
//! The stdlib `num::NonZeroU*` types do not implement any common
//! traits (and neither do their zeroable equivalents).  Where this
//! lack of traits in the standard library becomes problematic is if
//! you want to write a function that takes a vector of integers, and
//! that returns a vector of the corresponding non-zero integer types,
//! minus any elements that were zero in the original. You can write
//! that with the standard library quite easily for concrete types:
//!
//! ```rust
//! # use std::num::NonZeroU8;
//! fn only_nonzeros(v: Vec<u8>) -> Vec<NonZeroU8>
//! {
//!     v.into_iter()
//!         .filter_map(|n| NonZeroU8::new(n))
//!         .collect::<Vec<NonZeroU8>>()
//! }
//! # #[macro_use] extern crate nonzero_ext;
//! # fn main() {
//! let expected: Vec<NonZeroU8> = vec![nonzero!(20u8), nonzero!(5u8)];
//! assert_eq!(expected, only_nonzeros(vec![0, 20, 5]));
//! # }
//! ```
//!
//! But what if you want to allow this function to work with any
//! integer type that has a corresponding non-zero type? This crate
//! can help:
//!
//! ```rust
//! # use std::num::{NonZeroU8, NonZeroU32};
//! # use nonzero_ext::{NonZeroAble};
//! fn only_nonzeros<I>(v: Vec<I>) -> Vec<I::NonZero>
//! where
//!     I: Sized + NonZeroAble,
//! {
//!     v.into_iter()
//!         .filter_map(|n| n.as_nonzero())
//!         .collect::<Vec<I::NonZero>>()
//! }
//!
//! # #[macro_use] extern crate nonzero_ext;
//! # fn main() {
//! // It works for `u8`:
//! let input_u8: Vec<u8> = vec![0, 20, 5];
//! let expected_u8: Vec<NonZeroU8> = vec![nonzero!(20u8), nonzero!(5u8)];
//! assert_eq!(expected_u8, only_nonzeros(input_u8));
//!
//! // And it works for `u32`:
//! let input_u32: Vec<u32> = vec![0, 20, 5];
//! let expected_u32: Vec<NonZeroU32> = vec![nonzero!(20u32), nonzero!(5u32)];
//! assert_eq!(expected_u32, only_nonzeros(input_u32));
//! # }
//! ```
//!

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(unknown_lints)]
// Unfortunately necessary, otherwise features aren't supported in doctests:
#![allow(clippy::needless_doctest_main)]

mod lib {
    mod core {
        #[cfg(feature = "std")]
        pub use std::*;

        #[cfg(not(feature = "std"))]
        pub use core::*;
    }
    pub use self::core::num::{
        NonZeroI128, NonZeroI16, NonZeroI32, NonZeroI64, NonZeroI8, NonZeroIsize,
    };
    pub use self::core::num::{
        NonZeroU128, NonZeroU16, NonZeroU32, NonZeroU64, NonZeroU8, NonZeroUsize,
    };
}

use self::lib::*;

pub mod literals;

macro_rules! impl_nonzeroness {
    ($trait_name:ident, $nonzero_type:ty, $wrapped:ty) => {
        impl $trait_name for $nonzero_type {
            type Primitive = $wrapped;

            #[inline]
            #[cfg_attr(feature = "cargo-clippy", allow(clippy::new_ret_no_self))]
            fn new(n: $wrapped) -> Option<Self> {
                Self::new(n)
            }

            #[inline]
            fn get(self) -> Self::Primitive {
                <$nonzero_type>::get(self)
            }
        }
    };
}

/// A trait identifying a non-zero integral type. It is useful mostly
/// in order to give to genericized helper functions as `impl NonZero`
/// arguments.
pub trait NonZero {
    /// The primitive type (e.g. `u8`) underlying this integral type.
    type Primitive;

    /// Creates a new non-zero object from an integer that might be
    /// zero.
    fn new(n: Self::Primitive) -> Option<Self>
    where
        Self: Sized;

    /// Returns the value as a primitive type.
    fn get(self) -> Self::Primitive;
}

impl_nonzeroness!(NonZero, NonZeroU8, u8);
impl_nonzeroness!(NonZero, NonZeroU16, u16);
impl_nonzeroness!(NonZero, NonZeroU32, u32);
impl_nonzeroness!(NonZero, NonZeroU64, u64);
impl_nonzeroness!(NonZero, NonZeroU128, u128);
impl_nonzeroness!(NonZero, NonZeroUsize, usize);

impl_nonzeroness!(NonZero, NonZeroI8, i8);
impl_nonzeroness!(NonZero, NonZeroI16, i16);
impl_nonzeroness!(NonZero, NonZeroI32, i32);
impl_nonzeroness!(NonZero, NonZeroI64, i64);
impl_nonzeroness!(NonZero, NonZeroI128, i128);
impl_nonzeroness!(NonZero, NonZeroIsize, isize);

/// A trait identifying integral types that have a non-zeroable
/// equivalent.
pub trait NonZeroAble {
    /// The concrete non-zero type represented by an implementation of
    /// this trait. For example, for `u8`'s implementation, it is
    /// `NonZeroU8`.
    type NonZero: NonZero;

    //noinspection RsSelfConvention
    /// Converts the integer to its non-zero equivalent.
    ///
    /// # Examples
    ///
    /// ### Trying to convert zero
    /// ``` rust
    /// # use nonzero_ext::NonZeroAble;
    /// let n: u16 = 0;
    /// assert_eq!(n.as_nonzero(), None);
    /// ```
    ///
    /// ### Converting a non-zero value
    /// ``` rust
    /// # use nonzero_ext::NonZeroAble;
    /// # use std::num::NonZeroUsize;
    /// let n: usize = 20;
    /// let non0n: NonZeroUsize = n.as_nonzero().expect("should result in a converted value");
    /// assert_eq!(non0n.get(), 20);
    /// ```
    #[deprecated(since = "0.2.0", note = "Renamed to `into_nonzero`")]
    #[allow(clippy::wrong_self_convention)]
    fn as_nonzero(self) -> Option<Self::NonZero>
    where
        Self: Sized,
    {
        self.into_nonzero()
    }

    /// Converts the integer to its non-zero equivalent.
    ///
    /// # Examples
    ///
    /// ### Trying to convert zero
    /// ``` rust
    /// # use nonzero_ext::NonZeroAble;
    /// let n: u16 = 0;
    /// assert_eq!(n.into_nonzero(), None);
    /// ```
    ///
    /// ### Converting a non-zero value
    /// ``` rust
    /// # use nonzero_ext::NonZeroAble;
    /// # use std::num::NonZeroUsize;
    /// let n: usize = 20;
    /// let non0n: NonZeroUsize = n.into_nonzero().expect("should result in a converted value");
    /// assert_eq!(non0n.get(), 20);
    /// ```
    fn into_nonzero(self) -> Option<Self::NonZero>;

    //noinspection RsSelfConvention
    /// Converts the integer to its non-zero equivalent without
    /// checking for zeroness.
    ///
    /// This corresponds to the `new_unchecked` function on the
    /// corresponding NonZero type.
    ///
    /// # Safety
    /// The value must not be zero.
    #[deprecated(since = "0.2.0", note = "Renamed to `into_nonzero_unchecked`")]
    #[allow(clippy::wrong_self_convention)]
    unsafe fn as_nonzero_unchecked(self) -> Self::NonZero
    where
        Self: Sized,
    {
        self.into_nonzero_unchecked()
    }

    /// Converts the integer to its non-zero equivalent without
    /// checking for zeroness.
    ///
    /// This corresponds to the `new_unchecked` function on the
    /// corresponding NonZero type.
    ///
    /// # Safety
    /// The value must not be zero.
    unsafe fn into_nonzero_unchecked(self) -> Self::NonZero;
}

macro_rules! impl_nonzeroable {
    ($trait_name:ident, $nonzero_type: ty, $nonzeroable_type:ty) => {
        impl $trait_name for $nonzeroable_type {
            type NonZero = $nonzero_type;

            fn into_nonzero(self) -> Option<$nonzero_type> {
                Self::NonZero::new(self)
            }

            unsafe fn into_nonzero_unchecked(self) -> $nonzero_type {
                Self::NonZero::new_unchecked(self)
            }
        }
        impl literals::NonZeroLiteral<$nonzeroable_type> {
            /// Converts the wrapped value to its non-zero equivalent.
            /// # Safety
            /// The wrapped value must be non-zero.
            pub const unsafe fn into_nonzero(self) -> $nonzero_type {
                <$nonzero_type>::new_unchecked(self.0)
            }
        }
        impl literals::sealed::IntegerLiteral for $nonzeroable_type {}
    };
}

impl_nonzeroable!(NonZeroAble, NonZeroU8, u8);
impl_nonzeroable!(NonZeroAble, NonZeroU16, u16);
impl_nonzeroable!(NonZeroAble, NonZeroU32, u32);
impl_nonzeroable!(NonZeroAble, NonZeroU64, u64);
impl_nonzeroable!(NonZeroAble, NonZeroU128, u128);
impl_nonzeroable!(NonZeroAble, NonZeroUsize, usize);

impl_nonzeroable!(NonZeroAble, NonZeroI8, i8);
impl_nonzeroable!(NonZeroAble, NonZeroI16, i16);
impl_nonzeroable!(NonZeroAble, NonZeroI32, i32);
impl_nonzeroable!(NonZeroAble, NonZeroI64, i64);
impl_nonzeroable!(NonZeroAble, NonZeroI128, i128);
impl_nonzeroable!(NonZeroAble, NonZeroIsize, isize);

/// Create non-zero values from constant literals easily.
///
/// This macro issues a compile-time check and, if it passes, creates
/// the corresponding non-zero numeric value from the given
/// constant. Since the type of constant literals needs to be exactly
/// known, `nonzero!` requires that you annotate the constant with the
/// type, so instead of `nonzero!(20)` you must write `nonzero!(20 as
/// u16)`.
///
/// Note that this macro only works with [integer
/// literals](https://doc.rust-lang.org/reference/tokens.html#integer-literals),
/// it isn't possible to use the `nonzero!` macro with types other
/// than the built-in ones.
///
/// # Determining the output type
///
/// Use a suffix on the input value to determine the output type:
/// `nonzero!(1_usize)` will return a [`NonZeroUsize`], and
/// `nonzero!(-1_i32)` will return a [`NonZeroI32`].
///
/// # Const expressions
///
/// This macro can be used in const expressions.
///
/// # Examples
/// ```
/// # #[macro_use]
/// # extern crate nonzero_ext;
/// # fn main() {
/// nonzero!(20usize);  // => NonZeroUsize
/// nonzero!(20u32);    // => NonZeroU32
/// nonzero!(20 as u8); // => NonZeroU8
/// # }
/// ```
///
/// and passing a zero of any type will fail:
///
/// ``` # compile_fail
/// # #[macro_use]
/// # extern crate nonzero_ext;
/// # fn main() {
/// nonzero!(0u8);
/// # }
/// ```
///
#[macro_export]
macro_rules! nonzero {
    ($n:expr) => {{
        #[allow(unknown_lints, eq_op)]
        let _ = [(); ($n.count_ones() as usize) - 1];
        let lit = $crate::literals::NonZeroLiteral($n);
        unsafe { lit.into_nonzero() }
    }};
}
