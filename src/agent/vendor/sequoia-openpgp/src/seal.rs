//! [Sealing Traits]
//!
//!   [Sealing Traits]: https://rust-lang.github.io/api-guidelines/future-proofing.html#sealed-traits-protect-against-downstream-implementations-c-sealed
//!
//! Prevent the implementation of traits outside of the crate
//! to allow extension of the traits at a later time.
//!
//! Mark a trait as sealed by deriving it from seal::Sealed.
//!
//! Only Implementations of seal::Sealed will be able to implement the trait.
//! Since seal::Sealed is only visible inside the crate
//! sealed traits can only be implemented in the crate.

/// This trait is used to [seal] other traits so they cannot
/// be implemented for types outside this crate.
/// Therefore they can be extended in a non-breaking way.
///
///   [seal]: https://rust-lang.github.io/api-guidelines/future-proofing.html#sealed-traits-protect-against-downstream-implementations-c-sealed
///
/// # Examples
///
/// For example the [`cert::Preferences`] trait is sealed.
/// Therefore attempts to implement it will not compile:
///
///   [`cert::Preferences`]: crate::cert::Preferences
///
/// ```compile_fail
/// # extern crate sequoia_openpgp as openpgp;
/// use openpgp::cert::prelude::*;
/// use openpgp::cert::Preferences;
/// use openpgp::types::*;
///
/// pub struct InvalidComponentAmalgamation {}
/// impl<'a> Preferences<'a> for InvalidComponentAmalgamation { //~ ERROR `_x @` is not allowed in a tuple
/// fn preferred_symmetric_algorithms(&self)
///     -> Option<&'a [SymmetricAlgorithm]> { None }
/// fn preferred_hash_algorithms(&self) -> Option<&'a [HashAlgorithm]> { None }
/// fn preferred_compression_algorithms(&self)
///     -> Option<&'a [CompressionAlgorithm]> { None }
/// fn preferred_aead_algorithms(&self) -> Option<&'a [AEADAlgorithm]> { None }
/// fn key_server_preferences(&self) -> Option<KeyServerPreferences> { None }
/// fn preferred_key_server(&self) -> Option<&'a [u8]> { None }
/// fn features(&self) -> Option<Features> { None }
/// }
/// ```
pub trait Sealed {}
