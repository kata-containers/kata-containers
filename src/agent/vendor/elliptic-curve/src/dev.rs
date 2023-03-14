//! Development-related functionality: helpers and types for writing tests
//! against concrete implementations of the traits in this crate.

use crate::{
    consts::U32,
    error::{Error, Result},
    ff::{Field, PrimeField},
    group,
    rand_core::RngCore,
    sec1::{FromEncodedPoint, ToEncodedPoint},
    subtle::{Choice, ConditionallySelectable, ConstantTimeEq, CtOption},
    util::sbb64,
    weierstrass,
    zeroize::Zeroize,
    AlgorithmParameters, Curve, Order, ProjectiveArithmetic, ScalarBits,
};
use core::{
    convert::{TryFrom, TryInto},
    iter::Sum,
    ops::{Add, AddAssign, Mul, MulAssign, Neg, Sub, SubAssign},
};
use hex_literal::hex;

#[cfg(feature = "jwk")]
use crate::JwkParameters;

/// Pseudo-coordinate for fixed-based scalar mult output
pub const PSEUDO_COORDINATE_FIXED_BASE_MUL: [u8; 32] =
    hex!("deadbeef00000000000000000000000000000000000000000000000000000001");

/// Mock elliptic curve type useful for writing tests which require a concrete
/// curve type.
///
/// Note: this type is roughly modeled off of NIST P-256, but does not provide
/// an actual cure arithmetic implementation.
#[derive(Clone, Debug, Default, Eq, PartialEq, PartialOrd, Ord)]
pub struct MockCurve;

impl Curve for MockCurve {
    type FieldSize = U32;
}

#[cfg(target_pointer_width = "32")]
impl Order for MockCurve {
    type Limbs = [u32; 8];

    const ORDER: Self::Limbs = [
        0xfc63_2551,
        0xf3b9_cac2,
        0xa717_9e84,
        0xbce6_faad,
        0xffff_ffff,
        0xffff_ffff,
        0x0000_0000,
        0xffff_ffff,
    ];
}

#[cfg(target_pointer_width = "64")]
impl Order for MockCurve {
    type Limbs = [u64; 4];

    const ORDER: Self::Limbs = [
        0xf3b9_cac2_fc63_2551,
        0xbce6_faad_a717_9e84,
        0xffff_ffff_ffff_ffff,
        0xffff_ffff_0000_0000,
    ];
}

impl weierstrass::Curve for MockCurve {}

impl ProjectiveArithmetic for MockCurve {
    type ProjectivePoint = ProjectivePoint;
}

impl AlgorithmParameters for MockCurve {
    /// OID for NIST P-256
    const OID: pkcs8::ObjectIdentifier = pkcs8::ObjectIdentifier::new("1.2.840.10045.3.1.7");
}

#[cfg(feature = "jwk")]
#[cfg_attr(docsrs, doc(cfg(feature = "jwk")))]
impl JwkParameters for MockCurve {
    const CRV: &'static str = "P-256";
}

/// SEC1 encoded point.
pub type EncodedPoint = crate::sec1::EncodedPoint<MockCurve>;

/// Field element bytes.
pub type FieldBytes = crate::FieldBytes<MockCurve>;

/// Non-zero scalar value.
pub type NonZeroScalar = crate::NonZeroScalar<MockCurve>;

/// Public key.
pub type PublicKey = crate::PublicKey<MockCurve>;

/// Secret key.
pub type SecretKey = crate::SecretKey<MockCurve>;

const LIMBS: usize = 4;

type U256 = [u64; LIMBS];

/// P-256 modulus
pub const MODULUS: U256 = [
    0xf3b9_cac2_fc63_2551,
    0xbce6_faad_a717_9e84,
    0xffff_ffff_ffff_ffff,
    0xffff_ffff_0000_0000,
];

/// Example scalar type
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Scalar([u64; LIMBS]);

impl Field for Scalar {
    fn random(_rng: impl RngCore) -> Self {
        unimplemented!();
    }

    fn zero() -> Self {
        Self(Default::default())
    }

    fn one() -> Self {
        Self([1, 0, 0, 0])
    }

    fn is_zero(&self) -> bool {
        self.ct_eq(&Self::zero()).into()
    }

    #[must_use]
    fn square(&self) -> Self {
        unimplemented!();
    }

    #[must_use]
    fn double(&self) -> Self {
        unimplemented!();
    }

    fn invert(&self) -> CtOption<Self> {
        unimplemented!();
    }

    fn sqrt(&self) -> CtOption<Self> {
        unimplemented!();
    }
}

#[cfg(target_pointer_width = "64")]
fn pack_bits(native: U256) -> ScalarBits<MockCurve> {
    native.into()
}

#[cfg(target_pointer_width = "32")]
fn pack_bits(native: U256) -> ScalarBits<MockCurve> {
    [
        (native[0] & 0xffff_ffff) as u32,
        (native[0] >> 32) as u32,
        (native[1] & 0xffff_ffff) as u32,
        (native[1] >> 32) as u32,
        (native[2] & 0xffff_ffff) as u32,
        (native[2] >> 32) as u32,
        (native[3] & 0xffff_ffff) as u32,
        (native[3] >> 32) as u32,
    ]
    .into()
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

    fn from_repr(bytes: FieldBytes) -> Option<Self> {
        let mut w = [0u64; LIMBS];

        // Interpret the bytes as a big-endian integer w.
        w[3] = u64::from_be_bytes(bytes[0..8].try_into().expect("bad field size"));
        w[2] = u64::from_be_bytes(bytes[8..16].try_into().expect("bad field size"));
        w[1] = u64::from_be_bytes(bytes[16..24].try_into().expect("bad field size"));
        w[0] = u64::from_be_bytes(bytes[24..32].try_into().expect("bad field size"));

        // If w is in the range [0, n) then w - n will overflow, resulting in a borrow
        // value of 2^64 - 1.
        let (_, borrow) = sbb64(w[0], MODULUS[0], 0);
        let (_, borrow) = sbb64(w[1], MODULUS[1], borrow);
        let (_, borrow) = sbb64(w[2], MODULUS[2], borrow);
        let (_, borrow) = sbb64(w[3], MODULUS[3], borrow);

        if (borrow as u8) & 1 == 1 {
            Some(Scalar(w))
        } else {
            None
        }
    }

    fn to_repr(&self) -> FieldBytes {
        let mut ret = FieldBytes::default();
        ret[0..8].copy_from_slice(&self.0[3].to_be_bytes());
        ret[8..16].copy_from_slice(&self.0[2].to_be_bytes());
        ret[16..24].copy_from_slice(&self.0[1].to_be_bytes());
        ret[24..32].copy_from_slice(&self.0[0].to_be_bytes());
        ret
    }

    fn to_le_bits(&self) -> ScalarBits<MockCurve> {
        pack_bits(self.0)
    }

    fn is_odd(&self) -> bool {
        unimplemented!();
    }

    fn char_le_bits() -> ScalarBits<MockCurve> {
        pack_bits(MODULUS)
    }

    fn multiplicative_generator() -> Self {
        unimplemented!();
    }

    fn root_of_unity() -> Self {
        unimplemented!();
    }
}

impl TryFrom<[u64; 4]> for Scalar {
    type Error = Error;

    fn try_from(limbs: [u64; 4]) -> Result<Self> {
        // TODO(tarcieri): reject values that overflow the order
        Ok(Scalar(limbs))
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

impl Add<Scalar> for Scalar {
    type Output = Scalar;

    fn add(self, _other: Scalar) -> Scalar {
        unimplemented!();
    }
}

impl Add<&Scalar> for Scalar {
    type Output = Scalar;

    fn add(self, _other: &Scalar) -> Scalar {
        unimplemented!();
    }
}

impl AddAssign<Scalar> for Scalar {
    fn add_assign(&mut self, _rhs: Scalar) {
        unimplemented!();
    }
}

impl AddAssign<&Scalar> for Scalar {
    fn add_assign(&mut self, _rhs: &Scalar) {
        unimplemented!();
    }
}

impl Sub<Scalar> for Scalar {
    type Output = Scalar;

    fn sub(self, _other: Scalar) -> Scalar {
        unimplemented!();
    }
}

impl Sub<&Scalar> for Scalar {
    type Output = Scalar;

    fn sub(self, _other: &Scalar) -> Scalar {
        unimplemented!();
    }
}

impl SubAssign<Scalar> for Scalar {
    fn sub_assign(&mut self, _rhs: Scalar) {
        unimplemented!();
    }
}

impl SubAssign<&Scalar> for Scalar {
    fn sub_assign(&mut self, _rhs: &Scalar) {
        unimplemented!();
    }
}

impl Mul<Scalar> for Scalar {
    type Output = Scalar;

    fn mul(self, _other: Scalar) -> Scalar {
        unimplemented!();
    }
}

impl Mul<&Scalar> for Scalar {
    type Output = Scalar;

    fn mul(self, _other: &Scalar) -> Scalar {
        unimplemented!();
    }
}

impl MulAssign<Scalar> for Scalar {
    fn mul_assign(&mut self, _rhs: Scalar) {
        unimplemented!();
    }
}

impl MulAssign<&Scalar> for Scalar {
    fn mul_assign(&mut self, _rhs: &Scalar) {
        unimplemented!();
    }
}

impl Neg for Scalar {
    type Output = Scalar;

    fn neg(self) -> Scalar {
        unimplemented!();
    }
}

impl From<u64> for Scalar {
    fn from(_: u64) -> Scalar {
        unimplemented!();
    }
}

impl From<Scalar> for FieldBytes {
    fn from(scalar: Scalar) -> Self {
        Self::from(&scalar)
    }
}

impl From<&Scalar> for FieldBytes {
    fn from(scalar: &Scalar) -> Self {
        let mut ret = FieldBytes::default();
        ret[0..8].copy_from_slice(&scalar.0[3].to_be_bytes());
        ret[8..16].copy_from_slice(&scalar.0[2].to_be_bytes());
        ret[16..24].copy_from_slice(&scalar.0[1].to_be_bytes());
        ret[24..32].copy_from_slice(&scalar.0[0].to_be_bytes());
        ret
    }
}

impl Zeroize for Scalar {
    fn zeroize(&mut self) {
        self.0.as_mut().zeroize()
    }
}

/// Example affine point type
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AffinePoint {
    /// Result of fixed-based scalar multiplication
    FixedBaseOutput(Scalar),

    /// Is this point the identity point?
    Identity,

    /// Is this point the generator point?
    Generator,

    /// Is this point a different point corresponding to a given [`EncodedPoint`]
    Other(EncodedPoint),
}

impl ConditionallySelectable for AffinePoint {
    fn conditional_select(_a: &Self, _b: &Self, _choice: Choice) -> Self {
        unimplemented!();
    }
}

impl Default for AffinePoint {
    fn default() -> Self {
        Self::Identity
    }
}

impl FromEncodedPoint<MockCurve> for AffinePoint {
    fn from_encoded_point(point: &EncodedPoint) -> Option<Self> {
        if point.is_identity() {
            Some(Self::Identity)
        } else {
            Some(Self::Other(*point))
        }
    }
}

impl ToEncodedPoint<MockCurve> for AffinePoint {
    fn to_encoded_point(&self, compress: bool) -> EncodedPoint {
        match self {
            Self::FixedBaseOutput(scalar) => EncodedPoint::from_affine_coordinates(
                &scalar.to_repr(),
                &PSEUDO_COORDINATE_FIXED_BASE_MUL.into(),
                false,
            ),
            Self::Other(point) => {
                if compress == point.is_compressed() {
                    *point
                } else {
                    unimplemented!();
                }
            }
            _ => unimplemented!(),
        }
    }
}

impl Mul<NonZeroScalar> for AffinePoint {
    type Output = AffinePoint;

    fn mul(self, _scalar: NonZeroScalar) -> Self {
        unimplemented!();
    }
}

/// Example projective point type
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectivePoint {
    /// Result of fixed-based scalar multiplication
    FixedBaseOutput(Scalar),

    /// Is this point the identity point?
    Identity,

    /// Is this point the generator point?
    Generator,

    /// Is this point a different point corresponding to a given [`AffinePoint`]
    Other(AffinePoint),
}

impl Default for ProjectivePoint {
    fn default() -> Self {
        Self::Identity
    }
}

impl From<AffinePoint> for ProjectivePoint {
    fn from(point: AffinePoint) -> ProjectivePoint {
        match point {
            AffinePoint::FixedBaseOutput(scalar) => ProjectivePoint::FixedBaseOutput(scalar),
            AffinePoint::Identity => ProjectivePoint::Identity,
            AffinePoint::Generator => ProjectivePoint::Generator,
            other => ProjectivePoint::Other(other),
        }
    }
}

impl FromEncodedPoint<MockCurve> for ProjectivePoint {
    fn from_encoded_point(_point: &EncodedPoint) -> Option<Self> {
        unimplemented!();
    }
}

impl ToEncodedPoint<MockCurve> for ProjectivePoint {
    fn to_encoded_point(&self, _compress: bool) -> EncodedPoint {
        unimplemented!();
    }
}

impl group::Group for ProjectivePoint {
    type Scalar = Scalar;

    fn random(_rng: impl RngCore) -> Self {
        unimplemented!();
    }

    fn identity() -> Self {
        Self::Identity
    }

    fn generator() -> Self {
        Self::Generator
    }

    fn is_identity(&self) -> Choice {
        Choice::from((self == &Self::Identity) as u8)
    }

    #[must_use]
    fn double(&self) -> Self {
        unimplemented!();
    }
}

impl group::Curve for ProjectivePoint {
    type AffineRepr = AffinePoint;

    fn to_affine(&self) -> AffinePoint {
        match self {
            Self::FixedBaseOutput(scalar) => AffinePoint::FixedBaseOutput(*scalar),
            Self::Other(affine) => *affine,
            _ => unimplemented!(),
        }
    }
}

impl Add<ProjectivePoint> for ProjectivePoint {
    type Output = ProjectivePoint;

    fn add(self, _other: ProjectivePoint) -> ProjectivePoint {
        unimplemented!();
    }
}

impl Add<&ProjectivePoint> for ProjectivePoint {
    type Output = ProjectivePoint;

    fn add(self, _other: &ProjectivePoint) -> ProjectivePoint {
        unimplemented!();
    }
}

impl AddAssign<ProjectivePoint> for ProjectivePoint {
    fn add_assign(&mut self, _rhs: ProjectivePoint) {
        unimplemented!();
    }
}

impl AddAssign<&ProjectivePoint> for ProjectivePoint {
    fn add_assign(&mut self, _rhs: &ProjectivePoint) {
        unimplemented!();
    }
}

impl Sub<ProjectivePoint> for ProjectivePoint {
    type Output = ProjectivePoint;

    fn sub(self, _other: ProjectivePoint) -> ProjectivePoint {
        unimplemented!();
    }
}

impl Sub<&ProjectivePoint> for ProjectivePoint {
    type Output = ProjectivePoint;

    fn sub(self, _other: &ProjectivePoint) -> ProjectivePoint {
        unimplemented!();
    }
}

impl SubAssign<ProjectivePoint> for ProjectivePoint {
    fn sub_assign(&mut self, _rhs: ProjectivePoint) {
        unimplemented!();
    }
}

impl SubAssign<&ProjectivePoint> for ProjectivePoint {
    fn sub_assign(&mut self, _rhs: &ProjectivePoint) {
        unimplemented!();
    }
}

impl Add<AffinePoint> for ProjectivePoint {
    type Output = ProjectivePoint;

    fn add(self, _other: AffinePoint) -> ProjectivePoint {
        unimplemented!();
    }
}

impl Add<&AffinePoint> for ProjectivePoint {
    type Output = ProjectivePoint;

    fn add(self, _other: &AffinePoint) -> ProjectivePoint {
        unimplemented!();
    }
}

impl AddAssign<AffinePoint> for ProjectivePoint {
    fn add_assign(&mut self, _rhs: AffinePoint) {
        unimplemented!();
    }
}

impl AddAssign<&AffinePoint> for ProjectivePoint {
    fn add_assign(&mut self, _rhs: &AffinePoint) {
        unimplemented!();
    }
}

impl Sum for ProjectivePoint {
    fn sum<I: Iterator<Item = Self>>(_iter: I) -> Self {
        unimplemented!();
    }
}

impl<'a> Sum<&'a ProjectivePoint> for ProjectivePoint {
    fn sum<I: Iterator<Item = &'a ProjectivePoint>>(_iter: I) -> Self {
        unimplemented!();
    }
}

impl Sub<AffinePoint> for ProjectivePoint {
    type Output = ProjectivePoint;

    fn sub(self, _other: AffinePoint) -> ProjectivePoint {
        unimplemented!();
    }
}

impl Sub<&AffinePoint> for ProjectivePoint {
    type Output = ProjectivePoint;

    fn sub(self, _other: &AffinePoint) -> ProjectivePoint {
        unimplemented!();
    }
}

impl SubAssign<AffinePoint> for ProjectivePoint {
    fn sub_assign(&mut self, _rhs: AffinePoint) {
        unimplemented!();
    }
}

impl SubAssign<&AffinePoint> for ProjectivePoint {
    fn sub_assign(&mut self, _rhs: &AffinePoint) {
        unimplemented!();
    }
}

impl Mul<Scalar> for ProjectivePoint {
    type Output = ProjectivePoint;

    fn mul(self, scalar: Scalar) -> ProjectivePoint {
        match self {
            Self::Generator => Self::FixedBaseOutput(scalar),
            _ => unimplemented!(),
        }
    }
}

impl Mul<&Scalar> for ProjectivePoint {
    type Output = ProjectivePoint;

    fn mul(self, scalar: &Scalar) -> ProjectivePoint {
        self * *scalar
    }
}

impl MulAssign<Scalar> for ProjectivePoint {
    fn mul_assign(&mut self, _rhs: Scalar) {
        unimplemented!();
    }
}

impl MulAssign<&Scalar> for ProjectivePoint {
    fn mul_assign(&mut self, _rhs: &Scalar) {
        unimplemented!();
    }
}

impl Neg for ProjectivePoint {
    type Output = ProjectivePoint;

    fn neg(self) -> ProjectivePoint {
        unimplemented!();
    }
}
