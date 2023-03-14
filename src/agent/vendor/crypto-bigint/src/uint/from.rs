//! `From`-like conversions for [`UInt`].

use crate::{Limb, LimbUInt, Split, UInt, U128, U64};

impl<const LIMBS: usize> UInt<LIMBS> {
    /// Create a [`UInt`] from a `u8` (const-friendly)
    // TODO(tarcieri): replace with `const impl From<u8>` when stable
    pub const fn from_u8(n: u8) -> Self {
        const_assert!(LIMBS >= 1, "number of limbs must be greater than zero");
        let mut limbs = [Limb::ZERO; LIMBS];
        limbs[0].0 = n as LimbUInt;
        Self { limbs }
    }

    /// Create a [`UInt`] from a `u16` (const-friendly)
    // TODO(tarcieri): replace with `const impl From<u16>` when stable
    pub const fn from_u16(n: u16) -> Self {
        const_assert!(LIMBS >= 1, "number of limbs must be greater than zero");
        let mut limbs = [Limb::ZERO; LIMBS];
        limbs[0].0 = n as LimbUInt;
        Self { limbs }
    }

    /// Create a [`UInt`] from a `u32` (const-friendly)
    // TODO(tarcieri): replace with `const impl From<u32>` when stable
    #[allow(trivial_numeric_casts)]
    pub const fn from_u32(n: u32) -> Self {
        const_assert!(LIMBS >= 1, "number of limbs must be greater than zero");
        let mut limbs = [Limb::ZERO; LIMBS];
        limbs[0].0 = n as LimbUInt;
        Self { limbs }
    }

    /// Create a [`UInt`] from a `u64` (const-friendly)
    // TODO(tarcieri): replace with `const impl From<u64>` when stable
    #[cfg(target_pointer_width = "32")]
    pub const fn from_u64(n: u64) -> Self {
        const_assert!(LIMBS >= 2, "number of limbs must be two or greater");
        let mut limbs = [Limb::ZERO; LIMBS];
        limbs[0].0 = (n & 0xFFFFFFFF) as u32;
        limbs[1].0 = (n >> 32) as u32;
        Self { limbs }
    }

    /// Create a [`UInt`] from a `u64` (const-friendly)
    // TODO(tarcieri): replace with `const impl From<u64>` when stable
    #[cfg(target_pointer_width = "64")]
    pub const fn from_u64(n: u64) -> Self {
        const_assert!(LIMBS >= 1, "number of limbs must be greater than zero");
        let mut limbs = [Limb::ZERO; LIMBS];
        limbs[0].0 = n;
        Self { limbs }
    }

    /// Create a [`UInt`] from a `u128` (const-friendly)
    // TODO(tarcieri): replace with `const impl From<u128>` when stable
    pub const fn from_u128(n: u128) -> Self {
        const_assert!(
            LIMBS >= (128 / Limb::BIT_SIZE),
            "number of limbs must be greater than zero"
        );

        let lo = U64::from_u64((n & 0xffff_ffff_ffff_ffff) as u64);
        let hi = U64::from_u64((n >> 64) as u64);

        let mut limbs = [Limb::ZERO; LIMBS];

        let mut i = 0;
        while i < lo.limbs.len() {
            limbs[i] = lo.limbs[i];
            i += 1;
        }

        let mut j = 0;
        while j < hi.limbs.len() {
            limbs[i + j] = hi.limbs[j];
            j += 1;
        }

        Self { limbs }
    }

    /// Create a [`UInt`] from an array of the [`LimbUInt`]
    /// unsigned integer type.
    // TODO(tarcieri): replace with `const impl From<[LimbUInt; LIMBS]>` when stable
    #[inline]
    pub const fn from_uint_array(arr: [LimbUInt; LIMBS]) -> Self {
        let mut limbs = [Limb::ZERO; LIMBS];
        let mut i = 0;

        while i < LIMBS {
            limbs[i] = Limb(arr[i]);
            i += 1;
        }

        Self { limbs }
    }

    /// Create an array of [`LimbUInt`] unsigned integers from a [`UInt`].
    #[inline]
    // TODO(tarcieri): replace with `const impl From<Self> for [limb::Inner; LIMBS]` when stable
    pub const fn to_uint_array(self) -> [LimbUInt; LIMBS] {
        let mut arr = [0; LIMBS];
        let mut i = 0;

        while i < LIMBS {
            arr[i] = self.limbs[i].0;
            i += 1;
        }

        arr
    }
}

impl<const LIMBS: usize> From<u8> for UInt<LIMBS> {
    fn from(n: u8) -> Self {
        // TODO(tarcieri): const where clause when possible
        debug_assert!(LIMBS > 0, "limbs must be non-zero");
        Self::from_u8(n)
    }
}

impl<const LIMBS: usize> From<u16> for UInt<LIMBS> {
    fn from(n: u16) -> Self {
        // TODO(tarcieri): const where clause when possible
        debug_assert!(LIMBS > 0, "limbs must be non-zero");
        Self::from_u16(n)
    }
}

impl<const LIMBS: usize> From<u32> for UInt<LIMBS> {
    fn from(n: u32) -> Self {
        // TODO(tarcieri): const where clause when possible
        debug_assert!(LIMBS > 0, "limbs must be non-zero");
        Self::from_u32(n)
    }
}

impl<const LIMBS: usize> From<u64> for UInt<LIMBS> {
    fn from(n: u64) -> Self {
        // TODO(tarcieri): const where clause when possible
        debug_assert!(LIMBS >= (64 / Limb::BIT_SIZE), "not enough limbs");
        Self::from_u64(n)
    }
}

impl<const LIMBS: usize> From<u128> for UInt<LIMBS> {
    fn from(n: u128) -> Self {
        // TODO(tarcieri): const where clause when possible
        debug_assert!(LIMBS >= (128 / Limb::BIT_SIZE), "not enough limbs");
        Self::from_u128(n)
    }
}

#[cfg(target_pointer_width = "32")]
#[cfg_attr(docsrs, doc(cfg(target_pointer_width = "32")))]
impl From<U64> for u64 {
    fn from(n: U64) -> u64 {
        (n.limbs[0].0 as u64) | ((n.limbs[1].0 as u64) << 32)
    }
}

#[cfg(target_pointer_width = "64")]
#[cfg_attr(docsrs, doc(cfg(target_pointer_width = "64")))]
impl From<U64> for u64 {
    fn from(n: U64) -> u64 {
        n.limbs[0].into()
    }
}

impl From<U128> for u128 {
    fn from(n: U128) -> u128 {
        let (hi, lo) = n.split();
        (u64::from(hi) as u128) << 64 | (u64::from(lo) as u128)
    }
}

impl<const LIMBS: usize> From<[LimbUInt; LIMBS]> for UInt<LIMBS> {
    fn from(arr: [LimbUInt; LIMBS]) -> Self {
        Self::from_uint_array(arr)
    }
}

impl<const LIMBS: usize> From<UInt<LIMBS>> for [LimbUInt; LIMBS] {
    fn from(n: UInt<LIMBS>) -> [LimbUInt; LIMBS] {
        n.to_uint_array()
    }
}

impl<const LIMBS: usize> From<[Limb; LIMBS]> for UInt<LIMBS> {
    fn from(limbs: [Limb; LIMBS]) -> Self {
        Self { limbs }
    }
}

impl<const LIMBS: usize> From<UInt<LIMBS>> for [Limb; LIMBS] {
    fn from(n: UInt<LIMBS>) -> [Limb; LIMBS] {
        n.limbs
    }
}

impl<const LIMBS: usize> From<Limb> for UInt<LIMBS> {
    fn from(limb: Limb) -> Self {
        limb.0.into()
    }
}

#[cfg(test)]
mod tests {
    use crate::{Limb, LimbUInt, U128};

    #[cfg(target_pointer_width = "32")]
    use crate::U64 as UIntEx;

    #[cfg(target_pointer_width = "64")]
    use crate::U128 as UIntEx;

    #[test]
    fn from_u8() {
        let n = UIntEx::from(42u8);
        assert_eq!(n.limbs(), &[Limb(42), Limb(0)]);
    }

    #[test]
    fn from_u16() {
        let n = UIntEx::from(42u16);
        assert_eq!(n.limbs(), &[Limb(42), Limb(0)]);
    }

    #[test]
    fn from_u64() {
        let n = UIntEx::from(42u64);
        assert_eq!(n.limbs(), &[Limb(42), Limb(0)]);
    }

    #[test]
    fn from_u128() {
        let n = U128::from(42u128);
        assert_eq!(&n.limbs()[..2], &[Limb(42), Limb(0)]);
        assert_eq!(u128::from(n), 42u128);
    }

    #[test]
    fn array_round_trip() {
        let arr1 = [1, 2];
        let n = UIntEx::from(arr1);
        let arr2: [LimbUInt; 2] = n.into();
        assert_eq!(arr1, arr2);
    }
}
