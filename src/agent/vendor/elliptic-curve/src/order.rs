//! Low-level elliptic curve parameters.

use crate::Curve;
use core::fmt::Debug;

/// Order of an elliptic curve group.
///
/// This trait is available even when the `arithmetic` feature of the crate
/// is disabled and does not require any additional crate dependencies.
///
/// This trait is useful for supporting a baseline level of functionality
/// across curve implementations, even ones which do not provide a field
/// arithmetic backend.
// TODO(tarcieri): merge this with the `Curve` type in the next release?
pub trait Order: Curve {
    /// Type representing the "limbs" of the curves group's order on
    /// 32-bit platforms.
    #[cfg(target_pointer_width = "32")]
    type Limbs: AsRef<[u32]> + Copy + Debug;

    /// Type representing the "limbs" of the curves group's order on
    /// 64-bit platforms.
    #[cfg(target_pointer_width = "64")]
    type Limbs: AsRef<[u64]> + Copy + Debug;

    /// Order constant.
    ///
    /// Subdivided into either 32-bit or 64-bit "limbs" (depending on the
    /// target CPU's word size), specified from least to most significant.
    const ORDER: Self::Limbs;
}
