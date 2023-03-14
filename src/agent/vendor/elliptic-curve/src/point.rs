//! Elliptic curve points.

use crate::{Curve, FieldBytes, Scalar};

/// Elliptic curve with projective arithmetic implementation.
#[cfg_attr(docsrs, doc(cfg(feature = "arithmetic")))]
pub trait ProjectiveArithmetic: Curve
where
    Scalar<Self>: ff::PrimeField<Repr = FieldBytes<Self>>,
{
    /// Elliptic curve point in projective coordinates.
    type ProjectivePoint: group::Curve;
}

/// Affine point type for a given curve with a [`ProjectiveArithmetic`]
/// implementation.
#[cfg_attr(docsrs, doc(cfg(feature = "arithmetic")))]
pub type AffinePoint<C> =
    <<C as ProjectiveArithmetic>::ProjectivePoint as group::Curve>::AffineRepr;

/// Projective point type for a given curve with a [`ProjectiveArithmetic`]
/// implementation.
#[cfg_attr(docsrs, doc(cfg(feature = "arithmetic")))]
pub type ProjectivePoint<C> = <C as ProjectiveArithmetic>::ProjectivePoint;
