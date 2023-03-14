//! Scalar bytes.

use crate::{Curve, Error, FieldBytes, Order, Result};
use core::{
    convert::{TryFrom, TryInto},
    mem,
};
use generic_array::{typenum::Unsigned, GenericArray};
use subtle::{Choice, ConditionallySelectable, ConstantTimeEq, CtOption};

#[cfg(feature = "arithmetic")]
use crate::{ff::PrimeField, ProjectiveArithmetic, Scalar};

// TODO(tarcieri): unify these into a target-width gated `sbb`
#[cfg(target_pointer_width = "32")]
use crate::util::sbb32;
#[cfg(target_pointer_width = "64")]
use crate::util::sbb64;

/// Scalar bytes: wrapper for [`FieldBytes`] which guarantees that the the
/// inner byte value is within range of the curve's [`Order`].
///
/// Does not require an arithmetic implementation.
#[derive(Clone, Debug, Eq)]
pub struct ScalarBytes<C: Curve + Order> {
    /// Inner byte value; guaranteed to be in range of the curve's order.
    inner: FieldBytes<C>,
}

impl<C> ScalarBytes<C>
where
    C: Curve + Order,
{
    /// Create new [`ScalarBytes`], checking that the given input is within
    /// range of the curve's [`Order`].
    #[cfg(target_pointer_width = "32")]
    pub fn new(bytes: FieldBytes<C>) -> CtOption<Self> {
        assert_eq!(
            mem::size_of::<C::Limbs>(),
            mem::size_of::<FieldBytes<C>>(),
            "malformed curve order"
        );

        let mut borrow = 0;

        for (i, chunk) in bytes.as_ref().chunks(4).rev().enumerate() {
            let limb = u32::from_be_bytes(chunk.try_into().unwrap());
            borrow = sbb32(limb, C::ORDER.as_ref()[i], borrow).1;
        }

        let is_some = Choice::from((borrow as u8) & 1);
        CtOption::new(Self { inner: bytes }, is_some)
    }

    /// Create new [`ScalarBytes`], checking that the given input is within
    /// range of the curve's [`Order`].
    #[cfg(target_pointer_width = "64")]
    pub fn new(bytes: FieldBytes<C>) -> CtOption<Self> {
        assert_eq!(
            mem::size_of::<C::Limbs>(),
            mem::size_of::<FieldBytes<C>>(),
            "malformed curve order"
        );

        let mut borrow = 0;

        for (i, chunk) in bytes.as_ref().chunks(8).rev().enumerate() {
            let limb = u64::from_be_bytes(chunk.try_into().expect("invalid chunk size"));
            borrow = sbb64(limb, C::ORDER.as_ref()[i], borrow).1;
        }

        let is_some = Choice::from((borrow as u8) & 1);
        CtOption::new(Self { inner: bytes }, is_some)
    }

    /// Convert from a [`Scalar`] type for this curve.
    #[cfg(feature = "arithmetic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "arithmetic")))]
    pub fn from_scalar(scalar: &Scalar<C>) -> Self
    where
        C: ProjectiveArithmetic,
        Scalar<C>: PrimeField<Repr = FieldBytes<C>>,
    {
        Self {
            inner: scalar.to_repr(),
        }
    }

    /// Convert to a [`Scalar`] type for this curve.
    #[cfg(feature = "arithmetic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "arithmetic")))]
    pub fn to_scalar(&self) -> Scalar<C>
    where
        C: ProjectiveArithmetic,
        Scalar<C>: PrimeField<Repr = FieldBytes<C>>,
    {
        self.clone().into_scalar()
    }

    /// Convert into a [`Scalar`] type for this curve.
    #[cfg(feature = "arithmetic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "arithmetic")))]
    pub fn into_scalar(self) -> Scalar<C>
    where
        C: ProjectiveArithmetic,
        Scalar<C>: PrimeField<Repr = FieldBytes<C>>,
    {
        Scalar::<C>::from_repr(self.inner).expect("ScalarBytes order invariant violated")
    }

    /// Borrow the inner [`FieldBytes`]
    pub fn as_bytes(&self) -> &FieldBytes<C> {
        &self.inner
    }

    /// Convert into [`FieldBytes`]
    pub fn into_bytes(self) -> FieldBytes<C> {
        self.inner
    }

    /// Create [`ScalarBytes`] representing a value of zero.
    pub fn zero() -> Self {
        Self {
            inner: Default::default(),
        }
    }

    /// Is this [`ScalarBytes`] value all zeroes?
    pub fn is_zero(&self) -> Choice {
        self.ct_eq(&Self::zero())
    }
}

impl<C> AsRef<FieldBytes<C>> for ScalarBytes<C>
where
    C: Curve + Order,
{
    fn as_ref(&self) -> &FieldBytes<C> {
        &self.inner
    }
}

impl<C> AsRef<[u8]> for ScalarBytes<C>
where
    C: Curve + Order,
{
    fn as_ref(&self) -> &[u8] {
        self.inner.as_slice()
    }
}

impl<C> ConditionallySelectable for ScalarBytes<C>
where
    Self: Copy,
    C: Curve + Order,
{
    fn conditional_select(a: &Self, b: &Self, choice: Choice) -> Self {
        let mut inner = FieldBytes::<C>::default();

        for (i, (byte_a, byte_b)) in a.inner.iter().zip(b.inner.iter()).enumerate() {
            inner[i] = u8::conditional_select(byte_a, byte_b, choice)
        }

        Self { inner }
    }
}

impl<C> ConstantTimeEq for ScalarBytes<C>
where
    C: Curve + Order,
{
    fn ct_eq(&self, other: &Self) -> Choice {
        self.inner
            .iter()
            .zip(other.inner.iter())
            .fold(Choice::from(0u8), |acc, (a, b)| acc & a.ct_eq(b))
    }
}

impl<C> Copy for ScalarBytes<C>
where
    C: Curve + Order,
    FieldBytes<C>: Copy,
{
}

impl<C> Default for ScalarBytes<C>
where
    C: Curve + Order,
{
    fn default() -> Self {
        Self::zero()
    }
}

impl<C> From<ScalarBytes<C>> for FieldBytes<C>
where
    C: Curve + Order,
{
    fn from(scalar_bytes: ScalarBytes<C>) -> FieldBytes<C> {
        scalar_bytes.inner
    }
}

impl<C> PartialEq for ScalarBytes<C>
where
    C: Curve + Order,
{
    fn eq(&self, other: &Self) -> bool {
        self.ct_eq(other).into()
    }
}

impl<C> TryFrom<&[u8]> for ScalarBytes<C>
where
    C: Curve + Order,
{
    type Error = Error;

    fn try_from(bytes: &[u8]) -> Result<Self> {
        if bytes.len() == C::FieldSize::to_usize() {
            Option::from(ScalarBytes::new(GenericArray::clone_from_slice(bytes))).ok_or(Error)
        } else {
            Err(Error)
        }
    }
}

#[cfg(all(test, feature = "dev"))]
mod tests {
    use crate::dev::MockCurve;
    use core::convert::TryFrom;
    use hex_literal::hex;

    type ScalarBytes = super::ScalarBytes<MockCurve>;

    const SCALAR_REPR_ZERO: [u8; 32] = [0u8; 32];

    const SCALAR_REPR_IN_RANGE: [u8; 32] =
        hex!("FFFFFFFF 00000000 FFFFFFFF FFFFFFFF BCE6FAAD A7179E84 F3B9CAC2 FC632550");

    const SCALAR_REPR_ORDER: [u8; 32] =
        hex!("FFFFFFFF 00000000 FFFFFFFF FFFFFFFF BCE6FAAD A7179E84 F3B9CAC2 FC632551");

    const SCALAR_REPR_MAX: [u8; 32] =
        hex!("FFFFFFFF FFFFFFFF FFFFFFFF FFFFFFFF FFFFFFFF FFFFFFFF FFFFFFFF FFFFFFFF");

    #[test]
    fn scalar_in_range() {
        assert!(ScalarBytes::try_from(SCALAR_REPR_ZERO.as_ref()).is_ok());
        assert!(ScalarBytes::try_from(SCALAR_REPR_IN_RANGE.as_ref()).is_ok());
    }

    #[test]
    fn scalar_with_overflow() {
        assert!(ScalarBytes::try_from(SCALAR_REPR_ORDER.as_ref()).is_err());
        assert!(ScalarBytes::try_from(SCALAR_REPR_MAX.as_ref()).is_err());
    }
}
