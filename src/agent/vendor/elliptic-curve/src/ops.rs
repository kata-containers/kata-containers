//! Traits for arithmetic operations on elliptic curve field elements.

pub use core::ops::{Add, AddAssign, Mul, Neg, Sub, SubAssign};

use subtle::CtOption;

/// Perform an inversion on a field element (i.e. base field element or scalar)
pub trait Invert {
    /// Field element type
    type Output;

    /// Invert a field element.
    fn invert(&self) -> CtOption<Self::Output>;
}

#[cfg(feature = "arithmetic")]
impl<F: ff::Field> Invert for F {
    type Output = F;

    fn invert(&self) -> CtOption<F> {
        ff::Field::invert(self)
    }
}
