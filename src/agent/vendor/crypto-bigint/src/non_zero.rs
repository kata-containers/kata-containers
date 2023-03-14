//! Wrapper type for non-zero integers.

use crate::{Encoding, Integer, Limb, UInt, Zero};
use core::{
    fmt,
    num::{NonZeroU128, NonZeroU16, NonZeroU32, NonZeroU64, NonZeroU8},
    ops::Deref,
};
use subtle::{Choice, ConditionallySelectable, ConstantTimeEq, CtOption};

#[cfg(feature = "generic-array")]
use crate::{ArrayEncoding, ByteArray};

#[cfg(feature = "rand_core")]
use {
    crate::Random,
    rand_core::{CryptoRng, RngCore},
};

/// Wrapper type for non-zero integers.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, PartialOrd, Ord)]
pub struct NonZero<T: Zero>(T);

impl<T> NonZero<T>
where
    T: Zero,
{
    /// Create a new non-zero integer.
    pub fn new(n: T) -> CtOption<Self> {
        let is_zero = n.is_zero();
        CtOption::new(Self(n), !is_zero)
    }
}

impl<T> NonZero<T>
where
    T: Integer,
{
    /// The value `1`.
    pub const ONE: Self = Self(T::ONE);

    /// Maximum value this integer can express.
    pub const MAX: Self = Self(T::MAX);
}

impl<T> NonZero<T>
where
    T: Encoding + Zero,
{
    /// Decode from big endian bytes.
    pub fn from_be_bytes(bytes: T::Repr) -> CtOption<Self> {
        Self::new(T::from_be_bytes(bytes))
    }

    /// Decode from little endian bytes.
    pub fn from_le_bytes(bytes: T::Repr) -> CtOption<Self> {
        Self::new(T::from_le_bytes(bytes))
    }
}

#[cfg(feature = "generic-array")]
#[cfg_attr(docsrs, doc(cfg(feature = "generic-array")))]
impl<T> NonZero<T>
where
    T: ArrayEncoding + Zero,
{
    /// Decode a non-zero integer from big endian bytes.
    pub fn from_be_byte_array(bytes: ByteArray<T>) -> CtOption<Self> {
        Self::new(T::from_be_byte_array(bytes))
    }

    /// Decode a non-zero integer from big endian bytes.
    pub fn from_le_byte_array(bytes: ByteArray<T>) -> CtOption<Self> {
        Self::new(T::from_be_byte_array(bytes))
    }
}

impl<T> AsRef<T> for NonZero<T>
where
    T: Zero,
{
    fn as_ref(&self) -> &T {
        &self.0
    }
}

impl<T> ConditionallySelectable for NonZero<T>
where
    T: ConditionallySelectable + Zero,
{
    fn conditional_select(a: &Self, b: &Self, choice: Choice) -> Self {
        Self(T::conditional_select(&a.0, &b.0, choice))
    }
}

impl<T> ConstantTimeEq for NonZero<T>
where
    T: Zero,
{
    fn ct_eq(&self, other: &Self) -> Choice {
        self.0.ct_eq(&other.0)
    }
}

impl<T> Deref for NonZero<T>
where
    T: Zero,
{
    type Target = T;

    fn deref(&self) -> &T {
        &self.0
    }
}

#[cfg(feature = "rand_core")]
#[cfg_attr(docsrs, doc(cfg(feature = "rand_core")))]
impl<T> Random for NonZero<T>
where
    T: Random + Zero,
{
    /// Generate a random `NonZero<T>`.
    fn random(mut rng: impl CryptoRng + RngCore) -> Self {
        // Use rejection sampling to eliminate zero values.
        // While this method isn't constant-time, the attacker shouldn't learn
        // anything about unrelated outputs so long as `rng` is a secure `CryptoRng`.
        loop {
            if let Some(result) = Self::new(T::random(&mut rng)).into() {
                break result;
            }
        }
    }
}

impl<T> fmt::Display for NonZero<T>
where
    T: fmt::Display + Zero,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl<T> fmt::Binary for NonZero<T>
where
    T: fmt::Binary + Zero,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Binary::fmt(&self.0, f)
    }
}

impl<T> fmt::Octal for NonZero<T>
where
    T: fmt::Octal + Zero,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Octal::fmt(&self.0, f)
    }
}

impl<T> fmt::LowerHex for NonZero<T>
where
    T: fmt::LowerHex + Zero,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::LowerHex::fmt(&self.0, f)
    }
}

impl<T> fmt::UpperHex for NonZero<T>
where
    T: fmt::UpperHex + Zero,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::UpperHex::fmt(&self.0, f)
    }
}

impl NonZero<Limb> {
    /// Create a [`NonZero<Limb>`] from a [`NonZeroU8`] (const-friendly)
    // TODO(tarcieri): replace with `const impl From<NonZeroU8>` when stable
    pub const fn from_u8(n: NonZeroU8) -> Self {
        Self(Limb::from_u8(n.get()))
    }

    /// Create a [`NonZero<Limb>`] from a [`NonZeroU16`] (const-friendly)
    // TODO(tarcieri): replace with `const impl From<NonZeroU16>` when stable
    pub const fn from_u16(n: NonZeroU16) -> Self {
        Self(Limb::from_u16(n.get()))
    }

    /// Create a [`NonZero<Limb>`] from a [`NonZeroU32`] (const-friendly)
    // TODO(tarcieri): replace with `const impl From<NonZeroU32>` when stable
    pub const fn from_u32(n: NonZeroU32) -> Self {
        Self(Limb::from_u32(n.get()))
    }

    /// Create a [`NonZero<Limb>`] from a [`NonZeroU64`] (const-friendly)
    // TODO(tarcieri): replace with `const impl From<NonZeroU64>` when stable
    #[cfg(target_pointer_width = "64")]
    #[cfg_attr(docsrs, doc(cfg(target_pointer_width = "64")))]
    pub const fn from_u64(n: NonZeroU64) -> Self {
        Self(Limb::from_u64(n.get()))
    }
}

impl From<NonZeroU8> for NonZero<Limb> {
    fn from(integer: NonZeroU8) -> Self {
        Self::from_u8(integer)
    }
}

impl From<NonZeroU16> for NonZero<Limb> {
    fn from(integer: NonZeroU16) -> Self {
        Self::from_u16(integer)
    }
}

impl From<NonZeroU32> for NonZero<Limb> {
    fn from(integer: NonZeroU32) -> Self {
        Self::from_u32(integer)
    }
}

#[cfg(target_pointer_width = "64")]
#[cfg_attr(docsrs, doc(cfg(target_pointer_width = "64")))]
impl From<NonZeroU64> for NonZero<Limb> {
    fn from(integer: NonZeroU64) -> Self {
        Self::from_u64(integer)
    }
}

impl<const LIMBS: usize> NonZero<UInt<LIMBS>> {
    /// Create a [`NonZero<UInt>`] from a [`NonZeroU8`] (const-friendly)
    // TODO(tarcieri): replace with `const impl From<NonZeroU8>` when stable
    pub const fn from_u8(n: NonZeroU8) -> Self {
        Self(UInt::from_u8(n.get()))
    }

    /// Create a [`NonZero<UInt>`] from a [`NonZeroU16`] (const-friendly)
    // TODO(tarcieri): replace with `const impl From<NonZeroU16>` when stable
    pub const fn from_u16(n: NonZeroU16) -> Self {
        Self(UInt::from_u16(n.get()))
    }

    /// Create a [`NonZero<UInt>`] from a [`NonZeroU32`] (const-friendly)
    // TODO(tarcieri): replace with `const impl From<NonZeroU32>` when stable
    pub const fn from_u32(n: NonZeroU32) -> Self {
        Self(UInt::from_u32(n.get()))
    }

    /// Create a [`NonZero<UInt>`] from a [`NonZeroU64`] (const-friendly)
    // TODO(tarcieri): replace with `const impl From<NonZeroU64>` when stable
    pub const fn from_u64(n: NonZeroU64) -> Self {
        Self(UInt::from_u64(n.get()))
    }

    /// Create a [`NonZero<UInt>`] from a [`NonZeroU128`] (const-friendly)
    // TODO(tarcieri): replace with `const impl From<NonZeroU128>` when stable
    pub const fn from_u128(n: NonZeroU128) -> Self {
        Self(UInt::from_u128(n.get()))
    }
}

impl<const LIMBS: usize> From<NonZeroU8> for NonZero<UInt<LIMBS>> {
    fn from(integer: NonZeroU8) -> Self {
        Self::from_u8(integer)
    }
}

impl<const LIMBS: usize> From<NonZeroU16> for NonZero<UInt<LIMBS>> {
    fn from(integer: NonZeroU16) -> Self {
        Self::from_u16(integer)
    }
}

impl<const LIMBS: usize> From<NonZeroU32> for NonZero<UInt<LIMBS>> {
    fn from(integer: NonZeroU32) -> Self {
        Self::from_u32(integer)
    }
}

impl<const LIMBS: usize> From<NonZeroU64> for NonZero<UInt<LIMBS>> {
    fn from(integer: NonZeroU64) -> Self {
        Self::from_u64(integer)
    }
}

impl<const LIMBS: usize> From<NonZeroU128> for NonZero<UInt<LIMBS>> {
    fn from(integer: NonZeroU128) -> Self {
        Self::from_u128(integer)
    }
}
