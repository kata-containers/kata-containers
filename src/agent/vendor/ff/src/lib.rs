//! This crate provides traits for working with finite fields.

// Catch documentation errors caused by code changes.
#![no_std]
#![deny(intra_doc_link_resolution_failure)]
#![allow(unused_imports)]
#![forbid(unsafe_code)]

#[cfg(feature = "std")]
#[macro_use]
extern crate std;

#[cfg(feature = "derive")]
pub use ff_derive::*;

pub use bitvec::view::BitView;

use bitvec::{array::BitArray, order::Lsb0};
use core::convert::TryFrom;
use core::fmt;
use core::marker::PhantomData;
use core::ops::{Add, AddAssign, BitAnd, Mul, MulAssign, Neg, Shr, Sub, SubAssign};
use rand_core::RngCore;
#[cfg(feature = "std")]
use std::io::{self, Read, Write};
use subtle::{ConditionallySelectable, CtOption};

/// Bit representation of a field element.
pub type FieldBits<V> = BitArray<Lsb0, V>;

/// This trait represents an element of a field.
pub trait Field:
    Sized
    + Eq
    + Copy
    + Clone
    + Default
    + Send
    + Sync
    + fmt::Debug
    + 'static
    + ConditionallySelectable
    + Add<Output = Self>
    + Sub<Output = Self>
    + Mul<Output = Self>
    + Neg<Output = Self>
    + for<'a> Add<&'a Self, Output = Self>
    + for<'a> Mul<&'a Self, Output = Self>
    + for<'a> Sub<&'a Self, Output = Self>
    + MulAssign
    + AddAssign
    + SubAssign
    + for<'a> MulAssign<&'a Self>
    + for<'a> AddAssign<&'a Self>
    + for<'a> SubAssign<&'a Self>
{
    /// Returns an element chosen uniformly at random using a user-provided RNG.
    fn random(rng: impl RngCore) -> Self;

    /// Returns the zero element of the field, the additive identity.
    fn zero() -> Self;

    /// Returns the one element of the field, the multiplicative identity.
    fn one() -> Self;

    /// Returns true iff this element is zero.
    fn is_zero(&self) -> bool;

    /// Squares this element.
    #[must_use]
    fn square(&self) -> Self;

    /// Cubes this element.
    #[must_use]
    fn cube(&self) -> Self {
        self.square() * self
    }

    /// Doubles this element.
    #[must_use]
    fn double(&self) -> Self;

    /// Computes the multiplicative inverse of this element,
    /// failing if the element is zero.
    fn invert(&self) -> CtOption<Self>;

    /// Returns the square root of the field element, if it is
    /// quadratic residue.
    fn sqrt(&self) -> CtOption<Self>;

    /// Exponentiates `self` by `exp`, where `exp` is a little-endian order
    /// integer exponent.
    ///
    /// **This operation is variable time with respect to the exponent.** If the
    /// exponent is fixed, this operation is effectively constant time.
    fn pow_vartime<S: AsRef<[u64]>>(&self, exp: S) -> Self {
        let mut res = Self::one();
        for e in exp.as_ref().iter().rev() {
            for i in (0..64).rev() {
                res = res.square();

                if ((*e >> i) & 1) == 1 {
                    res.mul_assign(self);
                }
            }
        }

        res
    }
}

/// This represents an element of a prime field.
pub trait PrimeField: Field + From<u64> {
    /// The prime field can be converted back and forth into this binary
    /// representation.
    type Repr: Default + AsRef<[u8]> + AsMut<[u8]>;

    /// The backing store for a bit representation of a prime field element.
    type ReprBits: BitView + Send + Sync;

    /// Interpret a string of numbers as a (congruent) prime field element.
    /// Does not accept unnecessary leading zeroes or a blank string.
    fn from_str(s: &str) -> Option<Self> {
        if s.is_empty() {
            return None;
        }

        if s == "0" {
            return Some(Self::zero());
        }

        let mut res = Self::zero();

        let ten = Self::from(10);

        let mut first_digit = true;

        for c in s.chars() {
            match c.to_digit(10) {
                Some(c) => {
                    if first_digit {
                        if c == 0 {
                            return None;
                        }

                        first_digit = false;
                    }

                    res.mul_assign(&ten);
                    res.add_assign(&Self::from(u64::from(c)));
                }
                None => {
                    return None;
                }
            }
        }

        Some(res)
    }

    /// Attempts to convert a byte representation of a field element into an element of
    /// this prime field, failing if the input is not canonical (is not smaller than the
    /// field's modulus).
    ///
    /// The byte representation is interpreted with the same endianness as elements
    /// returned by [`PrimeField::to_repr`].
    fn from_repr(_: Self::Repr) -> Option<Self>;

    /// Converts an element of the prime field into the standard byte representation for
    /// this field.
    ///
    /// The endianness of the byte representation is implementation-specific. Generic
    /// encodings of field elements should be treated as opaque.
    fn to_repr(&self) -> Self::Repr;

    /// Converts an element of the prime field into a little-endian sequence of bits.
    fn to_le_bits(&self) -> FieldBits<Self::ReprBits>;

    /// Returns true iff this element is odd.
    fn is_odd(&self) -> bool;

    /// Returns true iff this element is even.
    #[inline(always)]
    fn is_even(&self) -> bool {
        !self.is_odd()
    }

    /// Returns the bits of the field characteristic (the modulus) in little-endian order.
    fn char_le_bits() -> FieldBits<Self::ReprBits>;

    /// How many bits are needed to represent an element of this field.
    const NUM_BITS: u32;

    /// How many bits of information can be reliably stored in the field element.
    ///
    /// This is usually `Self::NUM_BITS - 1`.
    const CAPACITY: u32;

    /// Returns a fixed multiplicative generator of `modulus - 1` order. This element must
    /// also be a quadratic nonresidue.
    ///
    /// It can be calculated using [SageMath] as `GF(modulus).primitive_element()`.
    ///
    /// Implementations of this method MUST ensure that this is the generator used to
    /// derive `Self::root_of_unity`.
    ///
    /// [SageMath]: https://www.sagemath.org/
    fn multiplicative_generator() -> Self;

    /// An integer `s` satisfying the equation `2^s * t = modulus - 1` with `t` odd.
    ///
    /// This is the number of leading zero bits in the little-endian bit representation of
    /// `modulus - 1`.
    const S: u32;

    /// Returns the `2^s` root of unity.
    ///
    /// It can be calculated by exponentiating `Self::multiplicative_generator` by `t`,
    /// where `t = (modulus - 1) >> Self::S`.
    fn root_of_unity() -> Self;
}

pub use self::arith_impl::*;

mod arith_impl {
    /// Calculate a - b - borrow, returning the result and modifying
    /// the borrow value.
    #[inline(always)]
    pub fn sbb(a: u64, b: u64, borrow: &mut u64) -> u64 {
        let tmp = (1u128 << 64) + u128::from(a) - u128::from(b) - u128::from(*borrow);

        *borrow = if tmp >> 64 == 0 { 1 } else { 0 };

        tmp as u64
    }

    /// Calculate a + b + carry, returning the sum and modifying the
    /// carry value.
    #[inline(always)]
    pub fn adc(a: u64, b: u64, carry: &mut u64) -> u64 {
        let tmp = u128::from(a) + u128::from(b) + u128::from(*carry);

        *carry = (tmp >> 64) as u64;

        tmp as u64
    }

    /// Calculate a + (b * c) + carry, returning the least significant digit
    /// and setting carry to the most significant digit.
    #[inline(always)]
    pub fn mac_with_carry(a: u64, b: u64, c: u64, carry: &mut u64) -> u64 {
        let tmp = (u128::from(a)) + u128::from(b) * u128::from(c) + u128::from(*carry);

        *carry = (tmp >> 64) as u64;

        tmp as u64
    }
}
