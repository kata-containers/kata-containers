//! Field arithmetic modulo p = 2^{224}(2^{32} − 1) + 2^{192} + 2^{96} − 1

use crate::FieldBytes;
use core::convert::TryInto;
use core::ops::{Add, AddAssign, Mul, MulAssign, Neg, Sub, SubAssign};
use elliptic_curve::{
    rand_core::{CryptoRng, RngCore},
    subtle::{Choice, ConditionallySelectable, ConstantTimeEq, CtOption},
    util::{adc64, mac64, sbb64},
};

#[cfg(feature = "zeroize")]
use elliptic_curve::zeroize::Zeroize;

/// The number of 64-bit limbs used to represent a [`FieldElement`].
const LIMBS: usize = 4;

/// Constant representing the modulus
/// p = 2^{224}(2^{32} − 1) + 2^{192} + 2^{96} − 1
pub const MODULUS: FieldElement = FieldElement([
    0xffff_ffff_ffff_ffff,
    0x0000_0000_ffff_ffff,
    0x0000_0000_0000_0000,
    0xffff_ffff_0000_0001,
]);

/// R = 2^256 mod p
const R: FieldElement = FieldElement([
    0x0000_0000_0000_0001,
    0xffff_ffff_0000_0000,
    0xffff_ffff_ffff_ffff,
    0x0000_0000_ffff_fffe,
]);

/// R^2 = 2^512 mod p
const R2: FieldElement = FieldElement([
    0x0000_0000_0000_0003,
    0xffff_fffb_ffff_ffff,
    0xffff_ffff_ffff_fffe,
    0x0000_0004_ffff_fffd,
]);

/// An element in the finite field modulo p = 2^{224}(2^{32} − 1) + 2^{192} + 2^{96} − 1.
// The internal representation is in little-endian order. Elements are always in
// Montgomery form; i.e., FieldElement(a) = aR mod p, with R = 2^256.
#[derive(Clone, Copy, Debug)]
pub struct FieldElement(pub(crate) [u64; LIMBS]);

impl ConditionallySelectable for FieldElement {
    fn conditional_select(a: &FieldElement, b: &FieldElement, choice: Choice) -> FieldElement {
        FieldElement([
            u64::conditional_select(&a.0[0], &b.0[0], choice),
            u64::conditional_select(&a.0[1], &b.0[1], choice),
            u64::conditional_select(&a.0[2], &b.0[2], choice),
            u64::conditional_select(&a.0[3], &b.0[3], choice),
        ])
    }
}

impl ConstantTimeEq for FieldElement {
    fn ct_eq(&self, other: &Self) -> Choice {
        self.0[0].ct_eq(&other.0[0])
            & self.0[1].ct_eq(&other.0[1])
            & self.0[2].ct_eq(&other.0[2])
            & self.0[3].ct_eq(&other.0[3])
    }
}

impl PartialEq for FieldElement {
    fn eq(&self, other: &Self) -> bool {
        self.ct_eq(other).into()
    }
}

impl Default for FieldElement {
    fn default() -> Self {
        FieldElement::zero()
    }
}

impl FieldElement {
    /// Returns the zero element.
    pub const fn zero() -> FieldElement {
        FieldElement([0, 0, 0, 0])
    }

    /// Returns the multiplicative identity.
    pub const fn one() -> FieldElement {
        R
    }

    /// Returns a uniformly-random element within the field.
    pub fn generate(mut rng: impl CryptoRng + RngCore) -> Self {
        // We reduce a random 512-bit value into a 256-bit field, which results in a
        // negligible bias from the uniform distribution.
        let mut buf = [0; 64];
        rng.fill_bytes(&mut buf);
        FieldElement::from_bytes_wide(buf)
    }

    fn from_bytes_wide(bytes: [u8; 64]) -> Self {
        FieldElement::montgomery_reduce(
            u64::from_be_bytes(bytes[0..8].try_into().unwrap()),
            u64::from_be_bytes(bytes[8..16].try_into().unwrap()),
            u64::from_be_bytes(bytes[16..24].try_into().unwrap()),
            u64::from_be_bytes(bytes[24..32].try_into().unwrap()),
            u64::from_be_bytes(bytes[32..40].try_into().unwrap()),
            u64::from_be_bytes(bytes[40..48].try_into().unwrap()),
            u64::from_be_bytes(bytes[48..56].try_into().unwrap()),
            u64::from_be_bytes(bytes[56..64].try_into().unwrap()),
        )
    }

    /// Attempts to parse the given byte array as an SEC1-encoded field element.
    ///
    /// Returns None if the byte array does not contain a big-endian integer in the range
    /// [0, p).
    pub fn from_bytes(bytes: &FieldBytes) -> CtOption<Self> {
        let mut w = [0u64; LIMBS];

        // Interpret the bytes as a big-endian integer w.
        w[3] = u64::from_be_bytes(bytes[0..8].try_into().unwrap());
        w[2] = u64::from_be_bytes(bytes[8..16].try_into().unwrap());
        w[1] = u64::from_be_bytes(bytes[16..24].try_into().unwrap());
        w[0] = u64::from_be_bytes(bytes[24..32].try_into().unwrap());

        // If w is in the range [0, p) then w - p will overflow, resulting in a borrow
        // value of 2^64 - 1.
        let (_, borrow) = sbb64(w[0], MODULUS.0[0], 0);
        let (_, borrow) = sbb64(w[1], MODULUS.0[1], borrow);
        let (_, borrow) = sbb64(w[2], MODULUS.0[2], borrow);
        let (_, borrow) = sbb64(w[3], MODULUS.0[3], borrow);
        let is_some = (borrow as u8) & 1;

        // Convert w to Montgomery form: w * R^2 * R^-1 mod p = wR mod p
        CtOption::new(FieldElement(w).mul(&R2), Choice::from(is_some))
    }

    /// Returns the SEC1 encoding of this field element.
    pub fn to_bytes(&self) -> FieldBytes {
        // Convert from Montgomery form to canonical form
        let tmp =
            FieldElement::montgomery_reduce(self.0[0], self.0[1], self.0[2], self.0[3], 0, 0, 0, 0);

        let mut ret = FieldBytes::default();
        ret[0..8].copy_from_slice(&tmp.0[3].to_be_bytes());
        ret[8..16].copy_from_slice(&tmp.0[2].to_be_bytes());
        ret[16..24].copy_from_slice(&tmp.0[1].to_be_bytes());
        ret[24..32].copy_from_slice(&tmp.0[0].to_be_bytes());
        ret
    }

    /// Determine if this `FieldElement` is zero.
    ///
    /// # Returns
    ///
    /// If zero, return `Choice(1)`.  Otherwise, return `Choice(0)`.
    pub fn is_zero(&self) -> Choice {
        self.ct_eq(&FieldElement::zero())
    }

    /// Determine if this `FieldElement` is odd in the SEC1 sense: `self mod 2 == 1`.
    ///
    /// # Returns
    ///
    /// If odd, return `Choice(1)`.  Otherwise, return `Choice(0)`.
    pub fn is_odd(&self) -> Choice {
        let bytes = self.to_bytes();
        (bytes[31] & 1).into()
    }

    /// Returns self + rhs mod p
    pub const fn add(&self, rhs: &Self) -> Self {
        // Bit 256 of p is set, so addition can result in five words.
        let (w0, carry) = adc64(self.0[0], rhs.0[0], 0);
        let (w1, carry) = adc64(self.0[1], rhs.0[1], carry);
        let (w2, carry) = adc64(self.0[2], rhs.0[2], carry);
        let (w3, w4) = adc64(self.0[3], rhs.0[3], carry);

        // Attempt to subtract the modulus, to ensure the result is in the field.
        Self::sub_inner(
            w0,
            w1,
            w2,
            w3,
            w4,
            MODULUS.0[0],
            MODULUS.0[1],
            MODULUS.0[2],
            MODULUS.0[3],
            0,
        )
    }

    /// Returns 2*self.
    pub const fn double(&self) -> Self {
        self.add(self)
    }

    /// Returns self - rhs mod p
    pub const fn subtract(&self, rhs: &Self) -> Self {
        Self::sub_inner(
            self.0[0], self.0[1], self.0[2], self.0[3], 0, rhs.0[0], rhs.0[1], rhs.0[2], rhs.0[3],
            0,
        )
    }

    #[inline]
    #[allow(clippy::too_many_arguments)]
    const fn sub_inner(
        l0: u64,
        l1: u64,
        l2: u64,
        l3: u64,
        l4: u64,
        r0: u64,
        r1: u64,
        r2: u64,
        r3: u64,
        r4: u64,
    ) -> Self {
        let (w0, borrow) = sbb64(l0, r0, 0);
        let (w1, borrow) = sbb64(l1, r1, borrow);
        let (w2, borrow) = sbb64(l2, r2, borrow);
        let (w3, borrow) = sbb64(l3, r3, borrow);
        let (_, borrow) = sbb64(l4, r4, borrow);

        // If underflow occurred on the final limb, borrow = 0xfff...fff, otherwise
        // borrow = 0x000...000. Thus, we use it as a mask to conditionally add the
        // modulus.
        let (w0, carry) = adc64(w0, MODULUS.0[0] & borrow, 0);
        let (w1, carry) = adc64(w1, MODULUS.0[1] & borrow, carry);
        let (w2, carry) = adc64(w2, MODULUS.0[2] & borrow, carry);
        let (w3, _) = adc64(w3, MODULUS.0[3] & borrow, carry);

        FieldElement([w0, w1, w2, w3])
    }

    /// Montgomery Reduction
    ///
    /// The general algorithm is:
    /// ```text
    /// A <- input (2n b-limbs)
    /// for i in 0..n {
    ///     k <- A[i] p' mod b
    ///     A <- A + k p b^i
    /// }
    /// A <- A / b^n
    /// if A >= p {
    ///     A <- A - p
    /// }
    /// ```
    ///
    /// For secp256r1, we have the following simplifications:
    ///
    /// - `p'` is 1, so our multiplicand is simply the first limb of the intermediate A.
    ///
    /// - The first limb of p is 2^64 - 1; multiplications by this limb can be simplified
    ///   to a shift and subtraction:
    ///   ```text
    ///       a_i * (2^64 - 1) = a_i * 2^64 - a_i = (a_i << 64) - a_i
    ///   ```
    ///   However, because `p' = 1`, the first limb of p is multiplied by limb i of the
    ///   intermediate A and then immediately added to that same limb, so we simply
    ///   initialize the carry to limb i of the intermediate.
    ///
    /// - The third limb of p is zero, so we can ignore any multiplications by it and just
    ///   add the carry.
    ///
    /// References:
    /// - Handbook of Applied Cryptography, Chapter 14
    ///   Algorithm 14.32
    ///   http://cacr.uwaterloo.ca/hac/about/chap14.pdf
    ///
    /// - Efficient and Secure Elliptic Curve Cryptography Implementation of Curve P-256
    ///   Algorithm 7) Montgomery Word-by-Word Reduction
    ///   https://csrc.nist.gov/csrc/media/events/workshop-on-elliptic-curve-cryptography-standards/documents/papers/session6-adalier-mehmet.pdf
    #[inline]
    #[allow(clippy::too_many_arguments)]
    const fn montgomery_reduce(
        r0: u64,
        r1: u64,
        r2: u64,
        r3: u64,
        r4: u64,
        r5: u64,
        r6: u64,
        r7: u64,
    ) -> Self {
        let (r1, carry) = mac64(r1, r0, MODULUS.0[1], r0);
        let (r2, carry) = adc64(r2, 0, carry);
        let (r3, carry) = mac64(r3, r0, MODULUS.0[3], carry);
        let (r4, carry2) = adc64(r4, 0, carry);

        let (r2, carry) = mac64(r2, r1, MODULUS.0[1], r1);
        let (r3, carry) = adc64(r3, 0, carry);
        let (r4, carry) = mac64(r4, r1, MODULUS.0[3], carry);
        let (r5, carry2) = adc64(r5, carry2, carry);

        let (r3, carry) = mac64(r3, r2, MODULUS.0[1], r2);
        let (r4, carry) = adc64(r4, 0, carry);
        let (r5, carry) = mac64(r5, r2, MODULUS.0[3], carry);
        let (r6, carry2) = adc64(r6, carry2, carry);

        let (r4, carry) = mac64(r4, r3, MODULUS.0[1], r3);
        let (r5, carry) = adc64(r5, 0, carry);
        let (r6, carry) = mac64(r6, r3, MODULUS.0[3], carry);
        let (r7, r8) = adc64(r7, carry2, carry);

        // Result may be within MODULUS of the correct value
        Self::sub_inner(
            r4,
            r5,
            r6,
            r7,
            r8,
            MODULUS.0[0],
            MODULUS.0[1],
            MODULUS.0[2],
            MODULUS.0[3],
            0,
        )
    }

    /// Returns self * rhs mod p
    pub const fn mul(&self, rhs: &Self) -> Self {
        // Schoolbook multiplication.

        let (w0, carry) = mac64(0, self.0[0], rhs.0[0], 0);
        let (w1, carry) = mac64(0, self.0[0], rhs.0[1], carry);
        let (w2, carry) = mac64(0, self.0[0], rhs.0[2], carry);
        let (w3, w4) = mac64(0, self.0[0], rhs.0[3], carry);

        let (w1, carry) = mac64(w1, self.0[1], rhs.0[0], 0);
        let (w2, carry) = mac64(w2, self.0[1], rhs.0[1], carry);
        let (w3, carry) = mac64(w3, self.0[1], rhs.0[2], carry);
        let (w4, w5) = mac64(w4, self.0[1], rhs.0[3], carry);

        let (w2, carry) = mac64(w2, self.0[2], rhs.0[0], 0);
        let (w3, carry) = mac64(w3, self.0[2], rhs.0[1], carry);
        let (w4, carry) = mac64(w4, self.0[2], rhs.0[2], carry);
        let (w5, w6) = mac64(w5, self.0[2], rhs.0[3], carry);

        let (w3, carry) = mac64(w3, self.0[3], rhs.0[0], 0);
        let (w4, carry) = mac64(w4, self.0[3], rhs.0[1], carry);
        let (w5, carry) = mac64(w5, self.0[3], rhs.0[2], carry);
        let (w6, w7) = mac64(w6, self.0[3], rhs.0[3], carry);

        FieldElement::montgomery_reduce(w0, w1, w2, w3, w4, w5, w6, w7)
    }

    /// Returns self * self mod p
    pub const fn square(&self) -> Self {
        // Schoolbook multiplication.
        self.mul(self)
    }

    /// Returns `self^by`, where `by` is a little-endian integer exponent.
    ///
    /// **This operation is variable time with respect to the exponent.** If the exponent
    /// is fixed, this operation is effectively constant time.
    pub fn pow_vartime(&self, by: &[u64; 4]) -> Self {
        let mut res = Self::one();
        for e in by.iter().rev() {
            for i in (0..64).rev() {
                res = res.square();

                if ((*e >> i) & 1) == 1 {
                    res = res * self;
                }
            }
        }
        res
    }

    /// Returns the multiplicative inverse of self, if self is non-zero.
    pub fn invert(&self) -> CtOption<Self> {
        // We need to find b such that b * a ≡ 1 mod p. As we are in a prime
        // field, we can apply Fermat's Little Theorem:
        //
        //    a^p         ≡ a mod p
        //    a^(p-1)     ≡ 1 mod p
        //    a^(p-2) * a ≡ 1 mod p
        //
        // Thus inversion can be implemented with a single exponentiation.
        let inverse = self.pow_vartime(&[
            0xffff_ffff_ffff_fffd,
            0x0000_0000_ffff_ffff,
            0x0000_0000_0000_0000,
            0xffff_ffff_0000_0001,
        ]);

        CtOption::new(inverse, !self.is_zero())
    }

    /// Returns the square root of self mod p, or `None` if no square root exists.
    pub fn sqrt(&self) -> CtOption<Self> {
        // We need to find alpha such that alpha^2 = beta mod p. For secp256r1,
        // p ≡ 3 mod 4. By Euler's Criterion, beta^(p-1)/2 ≡ 1 mod p. So:
        //
        //     alpha^2 = beta beta^((p - 1) / 2) mod p ≡ beta^((p + 1) / 2) mod p
        //     alpha = ± beta^((p + 1) / 4) mod p
        //
        // Thus sqrt can be implemented with a single exponentiation.

        let sqrt = self.pow_vartime(&[
            0x0000_0000_0000_0000,
            0x0000_0000_4000_0000,
            0x4000_0000_0000_0000,
            0x3fff_ffff_c000_0000,
        ]);

        CtOption::new(
            sqrt,
            (&sqrt * &sqrt).ct_eq(self), // Only return Some if it's the square root.
        )
    }
}

impl Add<&FieldElement> for &FieldElement {
    type Output = FieldElement;

    fn add(self, other: &FieldElement) -> FieldElement {
        FieldElement::add(self, other)
    }
}

impl Add<&FieldElement> for FieldElement {
    type Output = FieldElement;

    fn add(self, other: &FieldElement) -> FieldElement {
        FieldElement::add(&self, other)
    }
}

impl AddAssign<FieldElement> for FieldElement {
    fn add_assign(&mut self, rhs: FieldElement) {
        *self = FieldElement::add(self, &rhs);
    }
}

impl Sub<&FieldElement> for &FieldElement {
    type Output = FieldElement;

    fn sub(self, other: &FieldElement) -> FieldElement {
        FieldElement::subtract(self, other)
    }
}

impl Sub<&FieldElement> for FieldElement {
    type Output = FieldElement;

    fn sub(self, other: &FieldElement) -> FieldElement {
        FieldElement::subtract(&self, other)
    }
}

impl SubAssign<FieldElement> for FieldElement {
    fn sub_assign(&mut self, rhs: FieldElement) {
        *self = FieldElement::subtract(self, &rhs);
    }
}

impl Mul<&FieldElement> for &FieldElement {
    type Output = FieldElement;

    fn mul(self, other: &FieldElement) -> FieldElement {
        FieldElement::mul(self, other)
    }
}

impl Mul<&FieldElement> for FieldElement {
    type Output = FieldElement;

    fn mul(self, other: &FieldElement) -> FieldElement {
        FieldElement::mul(&self, other)
    }
}

impl MulAssign<FieldElement> for FieldElement {
    fn mul_assign(&mut self, rhs: FieldElement) {
        *self = FieldElement::mul(self, &rhs);
    }
}

impl Neg for FieldElement {
    type Output = FieldElement;

    fn neg(self) -> FieldElement {
        FieldElement::zero() - &self
    }
}

impl<'a> Neg for &'a FieldElement {
    type Output = FieldElement;

    fn neg(self) -> FieldElement {
        FieldElement::zero() - self
    }
}

#[cfg(feature = "zeroize")]
impl Zeroize for FieldElement {
    fn zeroize(&mut self) {
        self.0.zeroize();
    }
}

#[cfg(test)]
mod tests {
    use super::FieldElement;
    use crate::{test_vectors::field::DBL_TEST_VECTORS, FieldBytes};
    use proptest::{num::u64::ANY, prelude::*};

    #[test]
    fn zero_is_additive_identity() {
        let zero = FieldElement::zero();
        let one = FieldElement::one();
        assert_eq!(zero.add(&zero), zero);
        assert_eq!(one.add(&zero), one);
    }

    #[test]
    fn one_is_multiplicative_identity() {
        let one = FieldElement::one();
        assert_eq!(one.mul(&one), one);
    }

    #[test]
    fn from_bytes() {
        assert_eq!(
            FieldElement::from_bytes(&FieldBytes::default()).unwrap(),
            FieldElement::zero()
        );
        assert_eq!(
            FieldElement::from_bytes(
                &[
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 1
                ]
                .into()
            )
            .unwrap(),
            FieldElement::one()
        );
        assert!(bool::from(
            FieldElement::from_bytes(&[0xff; 32].into()).is_none()
        ));
    }

    #[test]
    fn to_bytes() {
        assert_eq!(FieldElement::zero().to_bytes(), FieldBytes::default());
        assert_eq!(
            FieldElement::one().to_bytes(),
            [
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 1
            ]
            .into()
        );
    }

    #[test]
    fn repeated_add() {
        let mut r = FieldElement::one();
        for i in 0..DBL_TEST_VECTORS.len() {
            assert_eq!(r.to_bytes(), DBL_TEST_VECTORS[i].into());
            r = r + &r;
        }
    }

    #[test]
    fn repeated_double() {
        let mut r = FieldElement::one();
        for i in 0..DBL_TEST_VECTORS.len() {
            assert_eq!(r.to_bytes(), DBL_TEST_VECTORS[i].into());
            r = r.double();
        }
    }

    #[test]
    fn repeated_mul() {
        let mut r = FieldElement::one();
        let two = r + &r;
        for i in 0..DBL_TEST_VECTORS.len() {
            assert_eq!(r.to_bytes(), DBL_TEST_VECTORS[i].into());
            r = r * &two;
        }
    }

    #[test]
    fn negation() {
        let two = FieldElement::one().double();
        let neg_two = -two;
        assert_eq!(two + &neg_two, FieldElement::zero());
        assert_eq!(-neg_two, two);
    }

    #[test]
    fn pow_vartime() {
        let one = FieldElement::one();
        let two = one + &one;
        let four = two.square();
        assert_eq!(two.pow_vartime(&[2, 0, 0, 0]), four);
    }

    #[test]
    fn invert() {
        assert!(bool::from(FieldElement::zero().invert().is_none()));

        let one = FieldElement::one();
        assert_eq!(one.invert().unwrap(), one);

        let two = one + &one;
        let inv_two = two.invert().unwrap();
        assert_eq!(two * &inv_two, one);
    }

    #[test]
    fn sqrt() {
        let one = FieldElement::one();
        let two = one + &one;
        let four = two.square();
        assert_eq!(four.sqrt().unwrap(), two);
    }

    proptest! {
        /// This checks behaviour well within the field ranges, because it doesn't set the
        /// highest limb.
        #[test]
        fn add_then_sub(
            a0 in ANY,
            a1 in ANY,
            a2 in ANY,
            b0 in ANY,
            b1 in ANY,
            b2 in ANY,
        ) {
            let a = FieldElement([a0, a1, a2, 0]);
            let b = FieldElement([b0, b1, b2, 0]);
            assert_eq!(a.add(&b).subtract(&a), b);
        }
    }
}
