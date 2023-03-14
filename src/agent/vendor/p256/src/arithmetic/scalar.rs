//! Scalar field arithmetic modulo n = 115792089210356248762697446949407573529996955224135760342422259061068512044369

pub mod blinding;

use crate::{FieldBytes, NistP256};
use core::{
    convert::TryInto,
    ops::{Add, AddAssign, Mul, MulAssign, Neg, Sub, SubAssign},
};
use elliptic_curve::{
    ff::{Field, PrimeField},
    generic_array::arr,
    rand_core::RngCore,
    subtle::{Choice, ConditionallySelectable, ConstantTimeEq, CtOption},
    util::{adc64, mac64, sbb64},
};

#[cfg(feature = "digest")]
use ecdsa_core::{elliptic_curve::consts::U32, hazmat::FromDigest, signature::digest::Digest};

#[cfg(feature = "zeroize")]
use crate::SecretKey;
#[cfg(feature = "zeroize")]
use elliptic_curve::zeroize::Zeroize;

/// The number of 64-bit limbs used to represent a [`Scalar`].
const LIMBS: usize = 4;

type U256 = [u64; LIMBS];

/// Constant representing the modulus
/// n = FFFFFFFF 00000000 FFFFFFFF FFFFFFFF BCE6FAAD A7179E84 F3B9CAC2 FC632551

// One way to calculate the modulus is with `GP/PARI`:
// ```
// p = (2^224) * (2^32 - 1) + 2^192 + 2^96 - 1
// b = 41058363725152142129326129780047268409114441015993725554835256314039467401291
// E = ellinit([Mod(-3, p), Mod(b, p)])
// default(parisize, 120000000)
// n = ellsea(E)
// isprime(n)
// ```
pub const MODULUS: U256 = [
    0xf3b9_cac2_fc63_2551,
    0xbce6_faad_a717_9e84,
    0xffff_ffff_ffff_ffff,
    0xffff_ffff_0000_0000,
];

const MODULUS_SHR1: U256 = [
    0x79dc_e561_7e31_92a8,
    0xde73_7d56_d38b_cf42,
    0x7fff_ffff_ffff_ffff,
    0x7fff_ffff_8000_0000,
];

/// MU = floor(2^512 / n)
///    = 115792089264276142090721624801893421302707618245269942344307673200490803338238
///    = 0x100000000fffffffffffffffeffffffff43190552df1a6c21012ffd85eedf9bfe
pub const MU: [u64; 5] = [
    0x012f_fd85_eedf_9bfe,
    0x4319_0552_df1a_6c21,
    0xffff_fffe_ffff_ffff,
    0x0000_0000_ffff_ffff,
    0x0000_0000_0000_0001,
];

/// Non-zero scalar value.
pub type NonZeroScalar = elliptic_curve::NonZeroScalar<NistP256>;

/// NIST P-256 field element serialized as bits.
pub type ScalarBits = elliptic_curve::ScalarBits<NistP256>;

/// An element in the finite field modulo n.
// The internal representation is as little-endian ordered u64 words.
#[derive(Clone, Copy, Debug, Default)]
#[cfg_attr(docsrs, doc(cfg(feature = "arithmetic")))]
pub struct Scalar(pub(crate) U256);

impl Field for Scalar {
    fn random(mut rng: impl RngCore) -> Self {
        let mut bytes = FieldBytes::default();

        // Generate a uniformly random scalar using rejection sampling,
        // which produces a uniformly random distribution of scalars.
        //
        // This method is not constant time, but should be secure so long as
        // rejected RNG outputs are unrelated to future ones (which is a
        // necessary property of a `CryptoRng`).
        //
        // With an unbiased RNG, the probability of failing to complete after 4
        // iterations is vanishingly small.
        loop {
            rng.fill_bytes(&mut bytes);
            if let Some(scalar) = Scalar::from_repr(bytes) {
                return scalar;
            }
        }
    }

    fn zero() -> Self {
        Scalar::zero()
    }

    fn one() -> Self {
        Scalar::one()
    }

    fn is_zero(&self) -> bool {
        self.is_zero().into()
    }

    #[must_use]
    fn square(&self) -> Self {
        Scalar::square(self)
    }

    #[must_use]
    fn double(&self) -> Self {
        self.add(self)
    }

    fn invert(&self) -> CtOption<Self> {
        Scalar::invert(self)
    }

    // TODO(tarcieri); stub! See: https://github.com/RustCrypto/elliptic-curves/issues/170
    fn sqrt(&self) -> CtOption<Self> {
        todo!("see RustCrypto/elliptic-curves#170");
    }
}

impl PrimeField for Scalar {
    type Repr = FieldBytes;

    #[cfg(target_pointer_width = "32")]
    type ReprBits = [u32; 8];

    #[cfg(target_pointer_width = "64")]
    type ReprBits = [u64; 4];

    const NUM_BITS: u32 = 256;
    const CAPACITY: u32 = 255;
    const S: u32 = 4;

    /// Attempts to parse the given byte array as an SEC1-encoded scalar.
    ///
    /// Returns None if the byte array does not contain a big-endian integer in the range
    /// [0, p).
    fn from_repr(bytes: FieldBytes) -> Option<Self> {
        let mut w = [0u64; LIMBS];

        // Interpret the bytes as a big-endian integer w.
        w[3] = u64::from_be_bytes(bytes[0..8].try_into().unwrap());
        w[2] = u64::from_be_bytes(bytes[8..16].try_into().unwrap());
        w[1] = u64::from_be_bytes(bytes[16..24].try_into().unwrap());
        w[0] = u64::from_be_bytes(bytes[24..32].try_into().unwrap());

        // If w is in the range [0, n) then w - n will overflow, resulting in a borrow
        // value of 2^64 - 1.
        let (_, borrow) = sbb64(w[0], MODULUS[0], 0);
        let (_, borrow) = sbb64(w[1], MODULUS[1], borrow);
        let (_, borrow) = sbb64(w[2], MODULUS[2], borrow);
        let (_, borrow) = sbb64(w[3], MODULUS[3], borrow);
        let is_some = (borrow as u8) & 1;

        CtOption::new(Scalar(w), Choice::from(is_some)).into()
    }

    fn to_repr(&self) -> FieldBytes {
        self.to_bytes()
    }

    fn to_le_bits(&self) -> ScalarBits {
        self.into()
    }

    fn is_odd(&self) -> bool {
        self.0[0] as u8 == 1
    }

    #[cfg(target_pointer_width = "32")]
    fn char_le_bits() -> ScalarBits {
        [
            0xfc63_2551,
            0xf3b9_cac2,
            0xa717_9e84,
            0xbce6_faad,
            0xffff_ffff,
            0xffff_ffff,
            0x0000_0000,
            0xffff_ffff,
        ]
        .into()
    }

    #[cfg(target_pointer_width = "64")]
    fn char_le_bits() -> ScalarBits {
        MODULUS.into()
    }

    fn multiplicative_generator() -> Self {
        7u64.into()
    }

    fn root_of_unity() -> Self {
        Scalar::from_repr(arr![u8;
            0xff, 0xc9, 0x7f, 0x06, 0x2a, 0x77, 0x09, 0x92, 0xba, 0x80, 0x7a, 0xce, 0x84, 0x2a,
            0x3d, 0xfc, 0x15, 0x46, 0xca, 0xd0, 0x04, 0x37, 0x8d, 0xaf, 0x05, 0x92, 0xd7, 0xfb,
            0xb4, 0x1e, 0x66, 0x00,
        ])
        .unwrap()
    }
}

impl From<u64> for Scalar {
    fn from(k: u64) -> Self {
        Scalar([k, 0, 0, 0])
    }
}

impl PartialEq for Scalar {
    fn eq(&self, other: &Self) -> bool {
        self.ct_eq(other).into()
    }
}

impl Eq for Scalar {}

impl PartialOrd for Scalar {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// Returns sign of left - right
fn cmp_vartime(left: &U256, right: &U256) -> i32 {
    use core::cmp::Ordering::*;

    // since we're little-endian, need to reverse iterate
    for (l, r) in left.iter().rev().zip(right.iter().rev()) {
        match l.cmp(r) {
            Less => return -1,
            Greater => return 1,
            Equal => continue,
        }
    }
    0
}

fn shr1(u256: &mut U256) {
    let mut bit: u64 = 0;
    for digit in u256.iter_mut().rev() {
        let new_digit = (bit << 63) | (*digit >> 1);
        bit = *digit & 1;
        *digit = new_digit;
    }
}

impl Ord for Scalar {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        use core::cmp::Ordering::*;
        match cmp_vartime(&self.0, &other.0) {
            -1 => Less,
            0 => Equal,
            1 => Greater,
            _ => unreachable!(),
        }
    }
}

#[cfg(feature = "digest")]
#[cfg_attr(docsrs, doc(cfg(feature = "digest")))]
impl FromDigest<NistP256> for Scalar {
    /// Convert the output of a digest algorithm into a [`Scalar`] reduced
    /// modulo n.
    fn from_digest<D>(digest: D) -> Self
    where
        D: Digest<OutputSize = U32>,
    {
        Self::from_bytes_reduced(&digest.finalize())
    }
}

impl Scalar {
    /// Returns the zero scalar.
    pub const fn zero() -> Scalar {
        Scalar([0, 0, 0, 0])
    }

    /// Returns the multiplicative identity.
    pub const fn one() -> Scalar {
        Scalar([1, 0, 0, 0])
    }

    /// Parses the given byte array as a scalar.
    ///
    /// Subtracts the modulus when the byte array is larger than the modulus.
    pub fn from_bytes_reduced(bytes: &FieldBytes) -> Self {
        Self::sub_inner(
            u64::from_be_bytes(bytes[24..32].try_into().unwrap()),
            u64::from_be_bytes(bytes[16..24].try_into().unwrap()),
            u64::from_be_bytes(bytes[8..16].try_into().unwrap()),
            u64::from_be_bytes(bytes[0..8].try_into().unwrap()),
            0,
            MODULUS[0],
            MODULUS[1],
            MODULUS[2],
            MODULUS[3],
            0,
        )
    }

    /// Returns the SEC1 encoding of this scalar.
    pub fn to_bytes(&self) -> FieldBytes {
        let mut ret = FieldBytes::default();
        ret[0..8].copy_from_slice(&self.0[3].to_be_bytes());
        ret[8..16].copy_from_slice(&self.0[2].to_be_bytes());
        ret[16..24].copy_from_slice(&self.0[1].to_be_bytes());
        ret[24..32].copy_from_slice(&self.0[0].to_be_bytes());
        ret
    }

    /// Determine if this `Scalar` is zero.
    ///
    /// # Returns
    ///
    /// If zero, return `Choice(1)`.  Otherwise, return `Choice(0)`.
    pub fn is_zero(&self) -> Choice {
        self.ct_eq(&Scalar::zero())
    }

    /// Returns self + rhs mod n
    pub const fn add(&self, rhs: &Self) -> Self {
        // Bit 256 of n is set, so addition can result in five words.
        let (w0, carry) = adc64(self.0[0], rhs.0[0], 0);
        let (w1, carry) = adc64(self.0[1], rhs.0[1], carry);
        let (w2, carry) = adc64(self.0[2], rhs.0[2], carry);
        let (w3, w4) = adc64(self.0[3], rhs.0[3], carry);

        // Attempt to subtract the modulus, to ensure the result is in the field.
        Self::sub_inner(
            w0, w1, w2, w3, w4, MODULUS[0], MODULUS[1], MODULUS[2], MODULUS[3], 0,
        )
    }

    /// Returns 2*self.
    pub const fn double(&self) -> Self {
        self.add(self)
    }

    /// Returns self - rhs mod n
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
        let (w0, carry) = adc64(w0, MODULUS[0] & borrow, 0);
        let (w1, carry) = adc64(w1, MODULUS[1] & borrow, carry);
        let (w2, carry) = adc64(w2, MODULUS[2] & borrow, carry);
        let (w3, _) = adc64(w3, MODULUS[3] & borrow, carry);

        Scalar([w0, w1, w2, w3])
    }

    /// Barrett Reduction
    ///
    /// The general algorithm is:
    /// ```text
    /// p = n = order of group
    /// b = 2^64 = 64bit machine word
    /// k = 4
    /// a \in [0, 2^512]
    /// mu := floor(b^{2k} / p)
    /// q1 := floor(a / b^{k - 1})
    /// q2 := q1 * mu
    /// q3 := <- floor(a / b^{k - 1})
    /// r1 := a mod b^{k + 1}
    /// r2 := q3 * m mod b^{k + 1}
    /// r := r1 - r2
    ///
    /// if r < 0: r := r + b^{k + 1}
    /// while r >= p: do r := r - p (at most twice)
    /// ```
    ///
    /// References:
    /// - Handbook of Applied Cryptography, Chapter 14
    ///   Algorithm 14.42
    ///   http://cacr.uwaterloo.ca/hac/about/chap14.pdf
    ///
    /// - Efficient and Secure Elliptic Curve Cryptography Implementation of Curve P-256
    ///   Algorithm 6) Barrett Reduction modulo p
    ///   https://csrc.nist.gov/csrc/media/events/workshop-on-elliptic-curve-cryptography-standards/documents/papers/session6-adalier-mehmet.pdf
    #[inline]
    #[allow(clippy::too_many_arguments)]
    const fn barrett_reduce(
        a0: u64,
        a1: u64,
        a2: u64,
        a3: u64,
        a4: u64,
        a5: u64,
        a6: u64,
        a7: u64,
    ) -> Self {
        let q1: [u64; 5] = [a3, a4, a5, a6, a7];

        const fn q1_times_mu_shift_five(q1: &[u64; 5]) -> [u64; 5] {
            // Schoolbook multiplication.

            let (_w0, carry) = mac64(0, q1[0], MU[0], 0);
            let (w1, carry) = mac64(0, q1[0], MU[1], carry);
            let (w2, carry) = mac64(0, q1[0], MU[2], carry);
            let (w3, carry) = mac64(0, q1[0], MU[3], carry);
            let (w4, w5) = mac64(0, q1[0], MU[4], carry);

            let (_w1, carry) = mac64(w1, q1[1], MU[0], 0);
            let (w2, carry) = mac64(w2, q1[1], MU[1], carry);
            let (w3, carry) = mac64(w3, q1[1], MU[2], carry);
            let (w4, carry) = mac64(w4, q1[1], MU[3], carry);
            let (w5, w6) = mac64(w5, q1[1], MU[4], carry);

            let (_w2, carry) = mac64(w2, q1[2], MU[0], 0);
            let (w3, carry) = mac64(w3, q1[2], MU[1], carry);
            let (w4, carry) = mac64(w4, q1[2], MU[2], carry);
            let (w5, carry) = mac64(w5, q1[2], MU[3], carry);
            let (w6, w7) = mac64(w6, q1[2], MU[4], carry);

            let (_w3, carry) = mac64(w3, q1[3], MU[0], 0);
            let (w4, carry) = mac64(w4, q1[3], MU[1], carry);
            let (w5, carry) = mac64(w5, q1[3], MU[2], carry);
            let (w6, carry) = mac64(w6, q1[3], MU[3], carry);
            let (w7, w8) = mac64(w7, q1[3], MU[4], carry);

            let (_w4, carry) = mac64(w4, q1[4], MU[0], 0);
            let (w5, carry) = mac64(w5, q1[4], MU[1], carry);
            let (w6, carry) = mac64(w6, q1[4], MU[2], carry);
            let (w7, carry) = mac64(w7, q1[4], MU[3], carry);
            let (w8, w9) = mac64(w8, q1[4], MU[4], carry);

            // let q2 = [_w0, _w1, _w2, _w3, _w4, w5, w6, w7, w8, w9];
            [w5, w6, w7, w8, w9]
        }

        let q3 = q1_times_mu_shift_five(&q1);

        let r1: [u64; 5] = [a0, a1, a2, a3, a4];

        const fn q3_times_n_keep_five(q3: &[u64; 5]) -> [u64; 5] {
            // Schoolbook multiplication.

            let (w0, carry) = mac64(0, q3[0], MODULUS[0], 0);
            let (w1, carry) = mac64(0, q3[0], MODULUS[1], carry);
            let (w2, carry) = mac64(0, q3[0], MODULUS[2], carry);
            let (w3, carry) = mac64(0, q3[0], MODULUS[3], carry);
            let (w4, _) = mac64(0, q3[0], 0, carry);

            let (w1, carry) = mac64(w1, q3[1], MODULUS[0], 0);
            let (w2, carry) = mac64(w2, q3[1], MODULUS[1], carry);
            let (w3, carry) = mac64(w3, q3[1], MODULUS[2], carry);
            let (w4, _) = mac64(w4, q3[1], MODULUS[3], carry);

            let (w2, carry) = mac64(w2, q3[2], MODULUS[0], 0);
            let (w3, carry) = mac64(w3, q3[2], MODULUS[1], carry);
            let (w4, _) = mac64(w4, q3[2], MODULUS[2], carry);

            let (w3, carry) = mac64(w3, q3[3], MODULUS[0], 0);
            let (w4, _) = mac64(w4, q3[3], MODULUS[1], carry);

            let (w4, _) = mac64(w4, q3[4], MODULUS[0], 0);

            [w0, w1, w2, w3, w4]
        }

        let r2: [u64; 5] = q3_times_n_keep_five(&q3);

        #[inline]
        #[allow(clippy::too_many_arguments)]
        const fn sub_inner_five(l: [u64; 5], r: [u64; 5]) -> [u64; 5] {
            let (w0, borrow) = sbb64(l[0], r[0], 0);
            let (w1, borrow) = sbb64(l[1], r[1], borrow);
            let (w2, borrow) = sbb64(l[2], r[2], borrow);
            let (w3, borrow) = sbb64(l[3], r[3], borrow);
            let (w4, _borrow) = sbb64(l[4], r[4], borrow);

            // If underflow occurred on the final limb - don't care (= add b^{k+1}).
            [w0, w1, w2, w3, w4]
        }

        let r: [u64; 5] = sub_inner_five(r1, r2);

        #[inline]
        #[allow(clippy::too_many_arguments)]
        const fn subtract_n_if_necessary(r0: u64, r1: u64, r2: u64, r3: u64, r4: u64) -> [u64; 5] {
            let (w0, borrow) = sbb64(r0, MODULUS[0], 0);
            let (w1, borrow) = sbb64(r1, MODULUS[1], borrow);
            let (w2, borrow) = sbb64(r2, MODULUS[2], borrow);
            let (w3, borrow) = sbb64(r3, MODULUS[3], borrow);
            let (w4, borrow) = sbb64(r4, 0, borrow);

            // If underflow occurred on the final limb, borrow = 0xfff...fff, otherwise
            // borrow = 0x000...000. Thus, we use it as a mask to conditionally add the
            // modulus.
            let (w0, carry) = adc64(w0, MODULUS[0] & borrow, 0);
            let (w1, carry) = adc64(w1, MODULUS[1] & borrow, carry);
            let (w2, carry) = adc64(w2, MODULUS[2] & borrow, carry);
            let (w3, carry) = adc64(w3, MODULUS[3] & borrow, carry);
            let (w4, _carry) = adc64(w4, 0, carry);

            [w0, w1, w2, w3, w4]
        }

        // Result is in range (0, 3*n - 1),
        // and 90% of the time, no subtraction will be needed.
        let r = subtract_n_if_necessary(r[0], r[1], r[2], r[3], r[4]);
        let r = subtract_n_if_necessary(r[0], r[1], r[2], r[3], r[4]);
        Scalar([r[0], r[1], r[2], r[3]])
    }

    /// Returns self * rhs mod n
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

        Scalar::barrett_reduce(w0, w1, w2, w3, w4, w5, w6, w7)
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
                    res *= self;
                }
            }
        }
        res
    }

    /// Returns the multiplicative inverse of self, if self is non-zero
    pub fn invert(&self) -> CtOption<Self> {
        // We need to find b such that b * a ≡ 1 mod p. As we are in a prime
        // field, we can apply Fermat's Little Theorem:
        //
        //    a^p         ≡ a mod p
        //    a^(p-1)     ≡ 1 mod p
        //    a^(p-2) * a ≡ 1 mod p
        //
        // Thus inversion can be implemented with a single exponentiation.
        //
        // This is `n - 2`, so the top right two digits are `4f` instead of `51`.
        let inverse = self.pow_vartime(&[
            0xf3b9_cac2_fc63_254f,
            0xbce6_faad_a717_9e84,
            0xffff_ffff_ffff_ffff,
            0xffff_ffff_0000_0000,
        ]);

        CtOption::new(inverse, !self.is_zero())
    }

    /// Is integer representing equivalence class odd
    pub fn is_odd(&self) -> Choice {
        ((self.0[0] & 1) as u8).into()
    }

    /// Is integer representing equivalence class even
    pub fn is_even(&self) -> Choice {
        !self.is_odd()
    }

    fn shr1(&mut self) {
        shr1(&mut self.0);
    }

    /// Faster inversion using Stein's algorithm
    pub fn invert_vartime(&self) -> CtOption<Self> {
        // https://link.springer.com/article/10.1007/s13389-016-0135-4

        let mut u = *self;
        // currently an invalid scalar
        let mut v = Scalar(MODULUS);

        #[allow(non_snake_case)]
        let mut A = Self::one();
        #[allow(non_snake_case)]
        let mut C = Self::zero();

        while !bool::from(u.is_zero()) {
            // u-loop
            while bool::from(u.is_even()) {
                u.shr1();
                if bool::from(A.is_even()) {
                    A.shr1();
                } else {
                    A.shr1();
                    A += Scalar(MODULUS_SHR1);
                    A += Self::one();
                }
            }

            // v-loop
            while bool::from(v.is_even()) {
                v.shr1();
                if bool::from(C.is_even()) {
                    C.shr1();
                } else {
                    C.shr1();
                    C += Scalar(MODULUS_SHR1);
                    C += Self::one();
                }
            }

            // sub-step
            if u >= v {
                u -= &v;
                A -= &C;
            } else {
                v -= &u;
                C -= &A;
            }
        }

        CtOption::new(C, !self.is_zero())
    }
}

impl Add<Scalar> for Scalar {
    type Output = Scalar;

    fn add(self, other: Scalar) -> Scalar {
        Scalar::add(&self, &other)
    }
}

impl Add<&Scalar> for &Scalar {
    type Output = Scalar;

    fn add(self, other: &Scalar) -> Scalar {
        Scalar::add(self, other)
    }
}

impl Add<&Scalar> for Scalar {
    type Output = Scalar;

    fn add(self, other: &Scalar) -> Scalar {
        Scalar::add(&self, other)
    }
}

impl AddAssign<Scalar> for Scalar {
    fn add_assign(&mut self, rhs: Scalar) {
        *self = Scalar::add(self, &rhs);
    }
}

impl AddAssign<&Scalar> for Scalar {
    fn add_assign(&mut self, rhs: &Scalar) {
        *self = Scalar::add(self, rhs);
    }
}

impl Sub<Scalar> for Scalar {
    type Output = Scalar;

    fn sub(self, other: Scalar) -> Scalar {
        Scalar::subtract(&self, &other)
    }
}

impl Sub<&Scalar> for &Scalar {
    type Output = Scalar;

    fn sub(self, other: &Scalar) -> Scalar {
        Scalar::subtract(self, other)
    }
}

impl Sub<&Scalar> for Scalar {
    type Output = Scalar;

    fn sub(self, other: &Scalar) -> Scalar {
        Scalar::subtract(&self, other)
    }
}

impl SubAssign<Scalar> for Scalar {
    fn sub_assign(&mut self, rhs: Scalar) {
        *self = Scalar::subtract(self, &rhs);
    }
}

impl SubAssign<&Scalar> for Scalar {
    fn sub_assign(&mut self, rhs: &Scalar) {
        *self = Scalar::subtract(self, rhs);
    }
}

impl Mul<Scalar> for Scalar {
    type Output = Scalar;

    fn mul(self, other: Scalar) -> Scalar {
        Scalar::mul(&self, &other)
    }
}

impl Mul<&Scalar> for &Scalar {
    type Output = Scalar;

    fn mul(self, other: &Scalar) -> Scalar {
        Scalar::mul(self, other)
    }
}

impl Mul<&Scalar> for Scalar {
    type Output = Scalar;

    fn mul(self, other: &Scalar) -> Scalar {
        Scalar::mul(&self, other)
    }
}

impl MulAssign<Scalar> for Scalar {
    fn mul_assign(&mut self, rhs: Scalar) {
        *self = Scalar::mul(self, &rhs);
    }
}

impl MulAssign<&Scalar> for Scalar {
    fn mul_assign(&mut self, rhs: &Scalar) {
        *self = Scalar::mul(self, rhs);
    }
}

impl Neg for Scalar {
    type Output = Scalar;

    fn neg(self) -> Scalar {
        Scalar::zero() - self
    }
}

impl<'a> Neg for &'a Scalar {
    type Output = Scalar;

    fn neg(self) -> Scalar {
        Scalar::zero() - self
    }
}

impl ConditionallySelectable for Scalar {
    fn conditional_select(a: &Self, b: &Self, choice: Choice) -> Self {
        Scalar([
            u64::conditional_select(&a.0[0], &b.0[0], choice),
            u64::conditional_select(&a.0[1], &b.0[1], choice),
            u64::conditional_select(&a.0[2], &b.0[2], choice),
            u64::conditional_select(&a.0[3], &b.0[3], choice),
        ])
    }
}

impl ConstantTimeEq for Scalar {
    fn ct_eq(&self, other: &Self) -> Choice {
        self.0[0].ct_eq(&other.0[0])
            & self.0[1].ct_eq(&other.0[1])
            & self.0[2].ct_eq(&other.0[2])
            & self.0[3].ct_eq(&other.0[3])
    }
}

#[cfg(target_pointer_width = "32")]
impl From<&Scalar> for ScalarBits {
    fn from(scalar: &Scalar) -> ScalarBits {
        let mut output = [0u32; 8];

        for (input, output) in scalar.0.iter().zip(output.chunks_mut(2)) {
            output[0] = (input & 0xFFFFFFFF) as u32;
            output[1] = (input >> 32) as u32;
        }

        output.into()
    }
}

#[cfg(target_pointer_width = "64")]
impl From<&Scalar> for ScalarBits {
    fn from(scalar: &Scalar) -> ScalarBits {
        scalar.0.into()
    }
}

impl From<Scalar> for FieldBytes {
    fn from(scalar: Scalar) -> Self {
        scalar.to_bytes()
    }
}

impl From<&Scalar> for FieldBytes {
    fn from(scalar: &Scalar) -> Self {
        scalar.to_bytes()
    }
}

#[cfg(feature = "zeroize")]
impl From<&SecretKey> for Scalar {
    fn from(secret_key: &SecretKey) -> Scalar {
        **secret_key.secret_scalar()
    }
}

#[cfg(feature = "zeroize")]
impl Zeroize for Scalar {
    fn zeroize(&mut self) {
        self.0.as_mut().zeroize()
    }
}

#[cfg(test)]
mod tests {
    use super::Scalar;
    use crate::FieldBytes;
    use elliptic_curve::ff::PrimeField;

    #[cfg(feature = "zeroize")]
    use crate::SecretKey;

    #[test]
    fn from_to_bytes_roundtrip() {
        let k: u64 = 42;
        let mut bytes = FieldBytes::default();
        bytes[24..].copy_from_slice(k.to_be_bytes().as_ref());

        let scalar = Scalar::from_repr(bytes).unwrap();
        assert_eq!(bytes, scalar.to_bytes());
    }

    #[test]
    // Basic tests that multiplication works.
    fn multiply() {
        let one = Scalar::one();
        let two = one + &one;
        let three = two + &one;
        let six = three + &three;
        assert_eq!(six, two * &three);

        let minus_two = -two;
        let minus_three = -three;
        assert_eq!(two, -minus_two);

        assert_eq!(minus_three * &minus_two, minus_two * &minus_three);
        assert_eq!(six, minus_two * &minus_three);
    }

    #[test]
    // Basic tests that scalar inversion works.
    fn invert() {
        let one = Scalar::one();
        let three = one + &one + &one;
        let inv_three = three.invert().unwrap();
        // println!("1/3 = {:x?}", &inv_three);
        assert_eq!(three * &inv_three, one);

        let minus_three = -three;
        // println!("-3 = {:x?}", &minus_three);
        let inv_minus_three = minus_three.invert().unwrap();
        assert_eq!(inv_minus_three, -inv_three);
        // println!("-1/3 = {:x?}", &inv_minus_three);
        assert_eq!(three * &inv_minus_three, -one);
    }

    // Tests that a Scalar can be safely converted to a SecretKey and back
    #[test]
    #[cfg(feature = "zeroize")]
    fn from_ec_secret() {
        let scalar = Scalar::one();
        let secret = SecretKey::from_bytes(scalar.to_bytes()).unwrap();
        let rederived_scalar = Scalar::from(&secret);
        assert_eq!(scalar.0, rederived_scalar.0);
    }

    #[test]
    #[cfg(target_pointer_width = "32")]
    fn scalar_into_scalarbits() {
        use super::ScalarBits;

        let minus_one = ScalarBits::from([
            0xfc63_2550,
            0xf3b9_cac2,
            0xa717_9e84,
            0xbce6_faad,
            0xffff_ffff,
            0xffff_ffff,
            0x0000_0000,
            0xffff_ffff,
        ]);

        let scalar_bits = ScalarBits::from(&-Scalar::from(1));
        assert_eq!(minus_one, scalar_bits);
    }
}
