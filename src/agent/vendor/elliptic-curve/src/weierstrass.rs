//! Elliptic curves in short Weierstrass form.

use crate::FieldBytes;
use subtle::{Choice, CtOption};

/// Marker trait for elliptic curves in short Weierstrass form.
pub trait Curve: super::Curve {}

/// Point compression settings.
pub trait PointCompression {
    /// Should point compression be applied by default?
    const COMPRESS_POINTS: bool;
}

/// Attempt to decompress an elliptic curve point from its x-coordinate and
/// a boolean flag indicating whether or not the y-coordinate is odd.
pub trait DecompressPoint<C: Curve>: Sized {
    /// Attempt to decompress an elliptic curve point.
    fn decompress(x: &FieldBytes<C>, y_is_odd: Choice) -> CtOption<Self>;
}
