//! Key-related functionality.
//!
//! # Data Types
//!
//! The main data type is the [`Key`] enum.  This enum abstracts away
//! the differences between the key formats (the deprecated [version
//! 3], the current [version 4], and the proposed [version 5]
//! formats).  Nevertheless, some functionality remains format
//! specific.  For instance, the `Key` enum doesn't provide a
//! mechanism to generate keys.  This functionality depends on the
//! format.
//!
//! This version of Sequoia only supports version 4 keys ([`Key4`]).
//! However, future versions may include limited support for version 3
//! keys to allow working with archived messages, and we intend to add
//! support for version 5 keys once the new version of the
//! specification has been finalized.
//!
//! OpenPGP specifies four different types of keys: [public keys],
//! [secret keys], [public subkeys], and [secret subkeys].  These are
//! all represented by the `Key` enum and the `Key4` struct using
//! marker types.  We use marker types rather than an enum, to better
//! exploit the type checking.  For instance, type-specific methods
//! like [`Key4::secret`] are only exposed for those types that
//! actually support them.  See the documentation for [`Key`] for an
//! explanation of how the markers work.
//!
//! The [`SecretKeyMaterial`] data type allows working with secret key
//! material directly.  This enum has two variants: [`Unencrypted`],
//! and [`Encrypted`].  It is not normally necessary to use this data
//! structure directly.  The primary functionality that is of interest
//! to most users is decrypting secret key material.  This is usually
//! more conveniently done using [`Key::decrypt_secret`].
//!
//! [`Key`]: super::Key
//! [version 3]: https://tools.ietf.org/html/rfc1991#section-6.6
//! [version 4]: https://tools.ietf.org/html/rfc4880#section-5.5.2
//! [version 5]: https://tools.ietf.org/html/draft-ietf-openpgp-rfc4880bis-09.html#name-public-key-packet-formats
//! [public keys]: https://tools.ietf.org/html/rfc4880#section-5.5.1.1
//! [secret keys]: https://tools.ietf.org/html/rfc4880#section-5.5.1.3
//! [public subkeys]: https://tools.ietf.org/html/rfc4880#section-5.5.1.2
//! [secret subkeys]: https://tools.ietf.org/html/rfc4880#section-5.5.1.4
//! [`Key::decrypt_secret`]: super::Key::decrypt_secret()
//!
//! # Key Creation
//!
//! Use [`Key4::generate_rsa`] or [`Key4::generate_ecc`] to create a
//! new key.
//!
//! Existing key material can be turned into an OpenPGP key using
//! [`Key4::import_public_cv25519`], [`Key4::import_public_ed25519`],
//! [`Key4::import_public_rsa`], [`Key4::import_secret_cv25519`],
//! [`Key4::import_secret_ed25519`], and [`Key4::import_secret_rsa`].
//!
//! Whether you create a new key or import existing key material, you
//! still need to create a binding signature, and, for signing keys, a
//! back signature for the key to be usable.
//!
//! [`Key4::generate_rsa`]: Key4::generate_rsa()
//! [`Key4::generate_ecc`]: Key4::generate_ecc()
//! [`Key4::import_public_cv25519`]: Key4::import_public_cv25519()
//! [`Key4::import_public_ed25519`]: Key4::import_public_ed25519()
//! [`Key4::import_public_rsa`]: Key4::import_public_rsa()
//! [`Key4::import_secret_cv25519`]: Key4::import_secret_cv25519()
//! [`Key4::import_secret_ed25519`]: Key4::import_secret_ed25519()
//! [`Key4::import_secret_rsa`]: Key4::import_secret_rsa()
//!
//! # In-Memory Protection of Secret Key Material
//!
//! Whether the secret key material is protected on disk or not,
//! Sequoia encrypts unencrypted secret key material ([`Unencrypted`])
//! while it is memory.  This helps protect against [heartbleed]-style
//! attacks where a buffer over-read allows an attacker to read from
//! the process's address space.  This protection is less important
//! for Rust programs, which are memory safe.  However, it is
//! essential when Sequoia is used via its FFI.
//!
//! See [`crypto::mem::Encrypted`] for details.
//!
//! [heartbleed]: https://en.wikipedia.org/wiki/Heartbleed
//! [`crypto::mem::Encrypted`]: super::super::crypto::mem::Encrypted

use std::fmt;
use std::cmp::Ordering;
use std::convert::TryInto;
use std::hash::Hasher;
use std::time;

#[cfg(test)]
use quickcheck::{Arbitrary, Gen};

use crate::Error;
use crate::cert::prelude::*;
use crate::crypto::{self, mem, mpi, hash::{Hash, Digest}};
use crate::packet;
use crate::packet::prelude::*;
use crate::PublicKeyAlgorithm;
use crate::seal;
use crate::SymmetricAlgorithm;
use crate::HashAlgorithm;
use crate::types::{Curve, Timestamp};
use crate::crypto::S2K;
use crate::Result;
use crate::crypto::Password;
use crate::KeyID;
use crate::Fingerprint;
use crate::KeyHandle;
use crate::policy::HashAlgoSecurity;

mod conversions;

/// A marker trait that captures whether a `Key` definitely contains
/// secret key material.
///
/// A [`Key`] can be treated as if it only has public key material
/// ([`key::PublicParts`]) or also has secret key material
/// ([`key::SecretParts`]).  For those cases where the type
/// information needs to be erased (e.g., interfaces like
/// [`Cert::keys`]), we provide the [`key::UnspecifiedParts`] marker.
///
/// Even if a `Key` does not have the `SecretKey` marker, it may still
/// have secret key material.  But, it will generally act as if it
/// didn't.  In particular, when serializing a `Key` without the
/// `SecretKey` marker, secret key material will be ignored.  See the
/// documentation for [`Key`] for a demonstration of this behavior.
///
/// [`Cert::keys`]: crate::cert::Cert::keys()
/// [`Key`]: super::Key
/// [`key::PublicParts`]: PublicParts
/// [`key::SecretParts`]: SecretParts
/// [`key::UnspecifiedParts`]: UnspecifiedParts
///
/// # Sealed trait
///
/// This trait is [sealed] and cannot be implemented for types outside this crate.
/// Therefore it can be extended in a non-breaking way.
/// If you want to implement the trait inside the crate
/// you also need to implement the `seal::Sealed` marker trait.
///
/// [sealed]: https://rust-lang.github.io/api-guidelines/future-proofing.html#sealed-traits-protect-against-downstream-implementations-c-sealed
pub trait KeyParts: fmt::Debug + seal::Sealed {
    /// Converts a key with unspecified parts into this kind of key.
    ///
    /// This function is helpful when you need to convert a concrete
    /// type into a generic type.  Using `From` works, but requires
    /// adding a type bound to the generic type, which is ugly and
    /// invasive.
    ///
    /// Converting a key with [`key::PublicParts`] or
    /// [`key::UnspecifiedParts`] will always succeed.  However,
    /// converting a key to one with [`key::SecretParts`] only
    /// succeeds if the key actually contains secret key material.
    ///
    /// [`key::PublicParts`]: PublicParts
    /// [`key::UnspecifiedParts`]: UnspecifiedParts
    /// [`key::SecretParts`]: SecretParts
    ///
    /// # Examples
    ///
    /// For a less construed example, refer to the [source code]:
    ///
    /// [source code]: https://gitlab.com/search?search=convert_key&project_id=4469613&search_code=true&repository_ref=master
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::Result;
    /// # use openpgp::cert::prelude::*;
    /// use openpgp::packet::prelude::*;
    ///
    /// fn f<P>(cert: &Cert, mut key: Key<P, key::UnspecifiedRole>)
    ///     -> Result<Key<P, key::UnspecifiedRole>>
    ///     where P: key::KeyParts
    /// {
    ///     // ...
    ///
    /// # let criterium = true;
    ///     if criterium {
    ///         // Cert::primary_key's return type is concrete
    ///         // (Key<key::PublicParts, key::PrimaryRole>).  We need to
    ///         // convert it to the generic type Key<P, key::UnspecifiedRole>.
    ///         // First, we "downcast" it to have unspecified parts and an
    ///         // unspecified role, then we use a method defined by the
    ///         // generic type to perform the conversion to the generic
    ///         // type P.
    ///         key = P::convert_key(
    ///             cert.primary_key().key().clone()
    ///                 .parts_into_unspecified()
    ///                 .role_into_unspecified())?;
    ///     }
    /// #   else { unreachable!() }
    ///
    ///     // ...
    ///
    ///     Ok(key)
    /// }
    /// # fn main() -> openpgp::Result<()> {
    /// # let (cert, _) =
    /// #     CertBuilder::general_purpose(None, Some("alice@example.org"))
    /// #     .generate()?;
    /// # f(&cert, cert.primary_key().key().clone().role_into_unspecified())?;
    /// # Ok(())
    /// # }
    /// ```
    fn convert_key<R: KeyRole>(key: Key<UnspecifiedParts, R>)
                               -> Result<Key<Self, R>>
        where Self: Sized;

    /// Converts a key reference with unspecified parts into this kind
    /// of key reference.
    ///
    /// This function is helpful when you need to convert a concrete
    /// type into a generic type.  Using `From` works, but requires
    /// adding a type bound to the generic type, which is ugly and
    /// invasive.
    ///
    /// Converting a key with [`key::PublicParts`] or
    /// [`key::UnspecifiedParts`] will always succeed.  However,
    /// converting a key to one with [`key::SecretParts`] only
    /// succeeds if the key actually contains secret key material.
    ///
    /// [`key::PublicParts`]: PublicParts
    /// [`key::UnspecifiedParts`]: UnspecifiedParts
    /// [`key::SecretParts`]: SecretParts
    fn convert_key_ref<R: KeyRole>(key: &Key<UnspecifiedParts, R>)
                                   -> Result<&Key<Self, R>>
        where Self: Sized;

    /// Converts a key bundle with unspecified parts into this kind of
    /// key bundle.
    ///
    /// This function is helpful when you need to convert a concrete
    /// type into a generic type.  Using `From` works, but requires
    /// adding a type bound to the generic type, which is ugly and
    /// invasive.
    ///
    /// Converting a key bundle with [`key::PublicParts`] or
    /// [`key::UnspecifiedParts`] will always succeed.  However,
    /// converting a key bundle to one with [`key::SecretParts`] only
    /// succeeds if the key bundle actually contains secret key
    /// material.
    ///
    /// [`key::PublicParts`]: PublicParts
    /// [`key::UnspecifiedParts`]: UnspecifiedParts
    /// [`key::SecretParts`]: SecretParts
    fn convert_bundle<R: KeyRole>(bundle: KeyBundle<UnspecifiedParts, R>)
                                  -> Result<KeyBundle<Self, R>>
        where Self: Sized;

    /// Converts a key bundle reference with unspecified parts into
    /// this kind of key bundle reference.
    ///
    /// This function is helpful when you need to convert a concrete
    /// type into a generic type.  Using `From` works, but requires
    /// adding a type bound to the generic type, which is ugly and
    /// invasive.
    ///
    /// Converting a key bundle with [`key::PublicParts`] or
    /// [`key::UnspecifiedParts`] will always succeed.  However,
    /// converting a key bundle to one with [`key::SecretParts`] only
    /// succeeds if the key bundle actually contains secret key
    /// material.
    ///
    /// [`key::PublicParts`]: PublicParts
    /// [`key::UnspecifiedParts`]: UnspecifiedParts
    /// [`key::SecretParts`]: SecretParts
    fn convert_bundle_ref<R: KeyRole>(bundle: &KeyBundle<UnspecifiedParts, R>)
                                      -> Result<&KeyBundle<Self, R>>
        where Self: Sized;

    /// Converts a key amalgamation with unspecified parts into this
    /// kind of key amalgamation.
    ///
    /// This function is helpful when you need to convert a concrete
    /// type into a generic type.  Using `From` works, but requires
    /// adding a type bound to the generic type, which is ugly and
    /// invasive.
    ///
    /// Converting a key amalgamation with [`key::PublicParts`] or
    /// [`key::UnspecifiedParts`] will always succeed.  However,
    /// converting a key amalgamation to one with [`key::SecretParts`]
    /// only succeeds if the key amalgamation actually contains secret
    /// key material.
    ///
    /// [`key::PublicParts`]: PublicParts
    /// [`key::UnspecifiedParts`]: UnspecifiedParts
    /// [`key::SecretParts`]: SecretParts
    fn convert_key_amalgamation<R: KeyRole>(
        ka: ComponentAmalgamation<Key<UnspecifiedParts, R>>)
        -> Result<ComponentAmalgamation<Key<Self, R>>>
        where Self: Sized;

    /// Converts a key amalgamation reference with unspecified parts
    /// into this kind of key amalgamation reference.
    ///
    /// This function is helpful when you need to convert a concrete
    /// type into a generic type.  Using `From` works, but requires
    /// adding a type bound to the generic type, which is ugly and
    /// invasive.
    ///
    /// Converting a key amalgamation with [`key::PublicParts`] or
    /// [`key::UnspecifiedParts`] will always succeed.  However,
    /// converting a key amalgamation to one with [`key::SecretParts`]
    /// only succeeds if the key amalgamation actually contains secret
    /// key material.
    ///
    /// [`key::PublicParts`]: PublicParts
    /// [`key::UnspecifiedParts`]: UnspecifiedParts
    /// [`key::SecretParts`]: SecretParts
    fn convert_key_amalgamation_ref<'a, R: KeyRole>(
        ka: &'a ComponentAmalgamation<'a, Key<UnspecifiedParts, R>>)
        -> Result<&'a ComponentAmalgamation<'a, Key<Self, R>>>
        where Self: Sized;

    /// Indicates that secret key material should be considered when
    /// comparing or hashing this key.
    fn significant_secrets() -> bool;
}

/// A marker trait that captures a `Key`'s role.
///
/// A [`Key`] can either be a primary key ([`key::PrimaryRole`]) or a
/// subordinate key ([`key::SubordinateRole`]).  For those cases where
/// the type information needs to be erased (e.g., interfaces like
/// [`Cert::keys`]), we provide the [`key::UnspecifiedRole`] marker.
///
/// [`Key`]: super::Key
/// [`key::PrimaryRole`]: PrimaryRole
/// [`key::SubordinateRole`]: SubordinateRole
/// [`Cert::keys`]: crate::cert::Cert::keys()
/// [`key::UnspecifiedRole`]: UnspecifiedRole
///
/// # Sealed trait
///
/// This trait is [sealed] and cannot be implemented for types outside this crate.
/// Therefore it can be extended in a non-breaking way.
/// If you want to implement the trait inside the crate
/// you also need to implement the `seal::Sealed` marker trait.
///
/// [sealed]: https://rust-lang.github.io/api-guidelines/future-proofing.html#sealed-traits-protect-against-downstream-implementations-c-sealed
pub trait KeyRole: fmt::Debug + seal::Sealed {
    /// Converts a key with an unspecified role into this kind of key.
    ///
    /// This function is helpful when you need to convert a concrete
    /// type into a generic type.  Using `From` works, but requires
    /// adding a type bound to the generic type, which is ugly and
    /// invasive.
    ///
    /// # Examples
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::Result;
    /// # use openpgp::cert::prelude::*;
    /// use openpgp::packet::prelude::*;
    ///
    /// fn f<R>(cert: &Cert, mut key: Key<key::UnspecifiedParts, R>)
    ///     -> Result<Key<key::UnspecifiedParts, R>>
    ///     where R: key::KeyRole
    /// {
    ///     // ...
    ///
    /// # let criterium = true;
    ///     if criterium {
    ///         // Cert::primary_key's return type is concrete
    ///         // (Key<key::PublicParts, key::PrimaryRole>).  We need to
    ///         // convert it to the generic type Key<key::UnspecifiedParts, R>.
    ///         // First, we "downcast" it to have unspecified parts and an
    ///         // unspecified role, then we use a method defined by the
    ///         // generic type to perform the conversion to the generic
    ///         // type R.
    ///         key = R::convert_key(
    ///             cert.primary_key().key().clone()
    ///                 .parts_into_unspecified()
    ///                 .role_into_unspecified());
    ///     }
    /// #   else { unreachable!() }
    ///
    ///     // ...
    ///
    ///     Ok(key)
    /// }
    /// # fn main() -> openpgp::Result<()> {
    /// # let (cert, _) =
    /// #     CertBuilder::general_purpose(None, Some("alice@example.org"))
    /// #     .generate()?;
    /// # f(&cert, cert.primary_key().key().clone().parts_into_unspecified())?;
    /// # Ok(())
    /// # }
    /// ```
    fn convert_key<P: KeyParts>(key: Key<P, UnspecifiedRole>)
                                -> Key<P, Self>
        where Self: Sized;

    /// Converts a key reference with an unspecified role into this
    /// kind of key reference.
    ///
    /// This function is helpful when you need to convert a concrete
    /// type into a generic type.  Using `From` works, but requires
    /// adding a type bound to the generic type, which is ugly and
    /// invasive.
    fn convert_key_ref<P: KeyParts>(key: &Key<P, UnspecifiedRole>)
                                    -> &Key<P, Self>
        where Self: Sized;

    /// Converts a key bundle with an unspecified role into this kind
    /// of key bundle.
    ///
    /// This function is helpful when you need to convert a concrete
    /// type into a generic type.  Using `From` works, but requires
    /// adding a type bound to the generic type, which is ugly and
    /// invasive.
    fn convert_bundle<P: KeyParts>(bundle: KeyBundle<P, UnspecifiedRole>)
                                   -> KeyBundle<P, Self>
        where Self: Sized;

    /// Converts a key bundle reference with an unspecified role into
    /// this kind of key bundle reference.
    ///
    /// This function is helpful when you need to convert a concrete
    /// type into a generic type.  Using `From` works, but requires
    /// adding a type bound to the generic type, which is ugly and
    /// invasive.
    fn convert_bundle_ref<P: KeyParts>(bundle: &KeyBundle<P, UnspecifiedRole>)
                                       -> &KeyBundle<P, Self>
        where Self: Sized;
}

/// A marker that indicates that a `Key` should be treated like a
/// public key.
///
/// Note: this doesn't indicate whether the data structure contains
/// secret key material; it indicates whether any secret key material
/// should be ignored.  For instance, when exporting a key with the
/// `PublicParts` marker, secret key material will *not* be exported.
/// See the documentation for [`Key`] for a demonstration.
///
/// Refer to [`KeyParts`] for details.
///
/// [`Key`]: super::Key
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PublicParts;

assert_send_and_sync!(PublicParts);

impl seal::Sealed for PublicParts {}
impl KeyParts for PublicParts {
    fn convert_key<R: KeyRole>(key: Key<UnspecifiedParts, R>)
                               -> Result<Key<Self, R>> {
        Ok(key.into())
    }

    fn convert_key_ref<R: KeyRole>(key: &Key<UnspecifiedParts, R>)
                                   -> Result<&Key<Self, R>> {
        Ok(key.into())
    }

    fn convert_bundle<R: KeyRole>(bundle: KeyBundle<UnspecifiedParts, R>)
                                  -> Result<KeyBundle<Self, R>> {
        Ok(bundle.into())
    }

    fn convert_bundle_ref<R: KeyRole>(bundle: &KeyBundle<UnspecifiedParts, R>)
                                      -> Result<&KeyBundle<Self, R>> {
        Ok(bundle.into())
    }

    fn convert_key_amalgamation<R: KeyRole>(
        ka: ComponentAmalgamation<Key<UnspecifiedParts, R>>)
        -> Result<ComponentAmalgamation<Key<Self, R>>> {
        Ok(ka.into())
    }

    fn convert_key_amalgamation_ref<'a, R: KeyRole>(
        ka: &'a ComponentAmalgamation<'a, Key<UnspecifiedParts, R>>)
        -> Result<&'a ComponentAmalgamation<'a, Key<Self, R>>> {
        Ok(ka.into())
    }

    fn significant_secrets() -> bool {
        false
    }
}

/// A marker that indicates that a `Key` should be treated like a
/// secret key.
///
/// Unlike the [`key::PublicParts`] marker, this marker asserts that
/// the [`Key`] contains secret key material.  Because secret key
/// material is not protected by the self-signature, there is no
/// indication that the secret key material is actually valid.
///
/// Refer to [`KeyParts`] for details.
///
/// [`key::PublicParts`]: PublicParts
/// [`Key`]: super::Key
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SecretParts;

assert_send_and_sync!(SecretParts);

impl seal::Sealed for SecretParts {}
impl KeyParts for SecretParts {
    fn convert_key<R: KeyRole>(key: Key<UnspecifiedParts, R>)
                               -> Result<Key<Self, R>>{
        key.try_into()
    }

    fn convert_key_ref<R: KeyRole>(key: &Key<UnspecifiedParts, R>)
                                   -> Result<&Key<Self, R>> {
        key.try_into()
    }

    fn convert_bundle<R: KeyRole>(bundle: KeyBundle<UnspecifiedParts, R>)
                                  -> Result<KeyBundle<Self, R>> {
        bundle.try_into()
    }

    fn convert_bundle_ref<R: KeyRole>(bundle: &KeyBundle<UnspecifiedParts, R>)
                                      -> Result<&KeyBundle<Self, R>> {
        bundle.try_into()
    }

    fn convert_key_amalgamation<R: KeyRole>(
        ka: ComponentAmalgamation<Key<UnspecifiedParts, R>>)
        -> Result<ComponentAmalgamation<Key<Self, R>>> {
        ka.try_into()
    }

    fn convert_key_amalgamation_ref<'a, R: KeyRole>(
        ka: &'a ComponentAmalgamation<'a, Key<UnspecifiedParts, R>>)
        -> Result<&'a ComponentAmalgamation<'a, Key<Self, R>>> {
        ka.try_into()
    }

    fn significant_secrets() -> bool {
        true
    }
}

/// A marker that indicates that a `Key`'s parts are unspecified.
///
/// Neither public key-specific nor secret key-specific operations are
/// allowed on these types of keys.  For instance, it is not possible
/// to export a key with the `UnspecifiedParts` marker, because it is
/// unclear how to treat any secret key material.  To export such a
/// key, you need to first change the marker to [`key::PublicParts`]
/// or [`key::SecretParts`].
///
/// This marker is used when it is necessary to erase the type.  For
/// instance, we need to do this when mixing [`Key`]s with different
/// markers in the same collection.  See [`Cert::keys`] for an
/// example.
///
/// Refer to [`KeyParts`] for details.
///
/// [`key::PublicParts`]: PublicParts
/// [`key::SecretParts`]: SecretParts
/// [`Key`]: super::Key
/// [`Cert::keys`]: super::super::Cert::keys()
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct UnspecifiedParts;

assert_send_and_sync!(UnspecifiedParts);

impl seal::Sealed for UnspecifiedParts {}
impl KeyParts for UnspecifiedParts {
    fn convert_key<R: KeyRole>(key: Key<UnspecifiedParts, R>)
                               -> Result<Key<Self, R>> {
        Ok(key)
    }

    fn convert_key_ref<R: KeyRole>(key: &Key<UnspecifiedParts, R>)
                                   -> Result<&Key<Self, R>> {
        Ok(key)
    }

    fn convert_bundle<R: KeyRole>(bundle: KeyBundle<UnspecifiedParts, R>)
                                  -> Result<KeyBundle<Self, R>> {
        Ok(bundle)
    }

    fn convert_bundle_ref<R: KeyRole>(bundle: &KeyBundle<UnspecifiedParts, R>)
                                      -> Result<&KeyBundle<Self, R>> {
        Ok(bundle)
    }

    fn convert_key_amalgamation<R: KeyRole>(
        ka: ComponentAmalgamation<Key<UnspecifiedParts, R>>)
        -> Result<ComponentAmalgamation<Key<UnspecifiedParts, R>>> {
        Ok(ka)
    }

    fn convert_key_amalgamation_ref<'a, R: KeyRole>(
        ka: &'a ComponentAmalgamation<'a, Key<UnspecifiedParts, R>>)
        -> Result<&'a ComponentAmalgamation<'a, Key<Self, R>>> {
        Ok(ka)
    }

    fn significant_secrets() -> bool {
        true
    }
}

/// A marker that indicates the `Key` should be treated like a primary key.
///
/// Refer to [`KeyRole`] for details.
///
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PrimaryRole;

assert_send_and_sync!(PrimaryRole);

impl seal::Sealed for PrimaryRole {}
impl KeyRole for PrimaryRole {
    fn convert_key<P: KeyParts>(key: Key<P, UnspecifiedRole>)
                                -> Key<P, Self> {
        key.into()
    }

    fn convert_key_ref<P: KeyParts>(key: &Key<P, UnspecifiedRole>)
                                    -> &Key<P, Self> {
        key.into()
    }

    fn convert_bundle<P: KeyParts>(bundle: KeyBundle<P, UnspecifiedRole>)
                                   -> KeyBundle<P, Self> {
        bundle.into()
    }

    fn convert_bundle_ref<P: KeyParts>(bundle: &KeyBundle<P, UnspecifiedRole>)
                                       -> &KeyBundle<P, Self> {
        bundle.into()
    }
}


/// A marker that indicates the `Key` should treated like a subkey.
///
/// Refer to [`KeyRole`] for details.
///
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SubordinateRole;

assert_send_and_sync!(SubordinateRole);

impl seal::Sealed for SubordinateRole {}
impl KeyRole for SubordinateRole {
    fn convert_key<P: KeyParts>(key: Key<P, UnspecifiedRole>)
                                -> Key<P, Self> {
        key.into()
    }

    fn convert_key_ref<P: KeyParts>(key: &Key<P, UnspecifiedRole>)
                                    -> &Key<P, Self> {
        key.into()
    }

    fn convert_bundle<P: KeyParts>(bundle: KeyBundle<P, UnspecifiedRole>)
                                   -> KeyBundle<P, Self> {
        bundle.into()
    }

    fn convert_bundle_ref<P: KeyParts>(bundle: &KeyBundle<P, UnspecifiedRole>)
                                       -> &KeyBundle<P, Self> {
        bundle.into()
    }
}

/// A marker that indicates the `Key`'s role is unspecified.
///
/// Neither primary key-specific nor subkey-specific operations are
/// allowed.  To perform those operations, the marker first has to be
/// changed to either [`key::PrimaryRole`] or
/// [`key::SubordinateRole`], as appropriate.
///
/// Refer to [`KeyRole`] for details.
///
/// [`key::PrimaryRole`]: PrimaryRole
/// [`key::SubordinateRole`]: SubordinateRole
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct UnspecifiedRole;

assert_send_and_sync!(UnspecifiedRole);

impl seal::Sealed for UnspecifiedRole {}
impl KeyRole for UnspecifiedRole {
    fn convert_key<P: KeyParts>(key: Key<P, UnspecifiedRole>)
                                -> Key<P, Self> {
        key
    }

    fn convert_key_ref<P: KeyParts>(key: &Key<P, UnspecifiedRole>)
                                    -> &Key<P, Self> {
        key
    }

    fn convert_bundle<P: KeyParts>(bundle: KeyBundle<P, UnspecifiedRole>)
                                   -> KeyBundle<P, Self> {
        bundle
    }

    fn convert_bundle_ref<P: KeyParts>(bundle: &KeyBundle<P, UnspecifiedRole>)
                                       -> &KeyBundle<P, Self> {
        bundle
    }
}

/// A Public Key.
pub(crate) type PublicKey = Key<PublicParts, PrimaryRole>;
/// A Public Subkey.
pub(crate) type PublicSubkey = Key<PublicParts, SubordinateRole>;
/// A Secret Key.
pub(crate) type SecretKey = Key<SecretParts, PrimaryRole>;
/// A Secret Subkey.
pub(crate) type SecretSubkey = Key<SecretParts, SubordinateRole>;

/// A key with public parts, and an unspecified role
/// (`UnspecifiedRole`).
#[allow(dead_code)]
pub(crate) type UnspecifiedPublic = Key<PublicParts, UnspecifiedRole>;
/// A key with secret parts, and an unspecified role
/// (`UnspecifiedRole`).
pub(crate) type UnspecifiedSecret = Key<SecretParts, UnspecifiedRole>;

/// A primary key with unspecified parts (`UnspecifiedParts`).
#[allow(dead_code)]
pub(crate) type UnspecifiedPrimary = Key<UnspecifiedParts, PrimaryRole>;
/// A subkey key with unspecified parts (`UnspecifiedParts`).
#[allow(dead_code)]
pub(crate) type UnspecifiedSecondary = Key<UnspecifiedParts, SubordinateRole>;

/// A key whose parts and role are unspecified
/// (`UnspecifiedParts`, `UnspecifiedRole`).
#[allow(dead_code)]
pub(crate) type UnspecifiedKey = Key<UnspecifiedParts, UnspecifiedRole>;


/// Holds a public key, public subkey, private key or private subkey
/// packet.
///
/// Use [`Key4::generate_rsa`] or [`Key4::generate_ecc`] to create a
/// new key.
///
/// Existing key material can be turned into an OpenPGP key using
/// [`Key4::new`], [`Key4::with_secret`], [`Key4::import_public_cv25519`],
/// [`Key4::import_public_ed25519`], [`Key4::import_public_rsa`],
/// [`Key4::import_secret_cv25519`], [`Key4::import_secret_ed25519`],
/// and [`Key4::import_secret_rsa`].
///
/// Whether you create a new key or import existing key material, you
/// still need to create a binding signature, and, for signing keys, a
/// back signature before integrating the key into a certificate.
///
/// Normally, you won't directly use `Key4`, but [`Key`], which is a
/// relatively thin wrapper around `Key4`.
///
/// See [Section 5.5 of RFC 4880] and [the documentation for `Key`]
/// for more details.
///
/// [Section 5.5 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.5
/// [the documentation for `Key`]: super::Key
/// [`Key`]: super::Key
#[derive(Clone)]
pub struct Key4<P, R>
    where P: KeyParts, R: KeyRole
{
    /// CTB packet header fields.
    pub(crate) common: packet::Common,
    /// When the key was created.
    creation_time: Timestamp,
    /// Public key algorithm of this signature.
    pk_algo: PublicKeyAlgorithm,
    /// Public key MPIs.
    mpis: mpi::PublicKey,
    /// Optional secret part of the key.
    secret: Option<SecretKeyMaterial>,

    p: std::marker::PhantomData<P>,
    r: std::marker::PhantomData<R>,
}

assert_send_and_sync!(Key4<P, R> where P: KeyParts, R: KeyRole);

impl<P: KeyParts, R: KeyRole> PartialEq for Key4<P, R> {
    fn eq(&self, other: &Key4<P, R>) -> bool {
        self.creation_time == other.creation_time
            && self.pk_algo == other.pk_algo
            && self.mpis == other.mpis
            && (! P::significant_secrets() || self.secret == other.secret)
    }
}

impl<P: KeyParts, R: KeyRole> Eq for Key4<P, R> {}

impl<P: KeyParts, R: KeyRole> std::hash::Hash for Key4<P, R> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::hash::Hash::hash(&self.creation_time, state);
        std::hash::Hash::hash(&self.pk_algo, state);
        std::hash::Hash::hash(&self.mpis, state);
        if P::significant_secrets() {
            std::hash::Hash::hash(&self.secret, state);
        }
    }
}

impl<P, R> fmt::Debug for Key4<P, R>
    where P: key::KeyParts,
          R: key::KeyRole,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Key4")
            .field("fingerprint", &self.fingerprint())
            .field("creation_time", &self.creation_time)
            .field("pk_algo", &self.pk_algo)
            .field("mpis", &self.mpis)
            .field("secret", &self.secret)
            .finish()
    }
}

impl<P, R> fmt::Display for Key4<P, R>
    where P: key::KeyParts,
          R: key::KeyRole,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.fingerprint())
    }
}

impl<P, R> Key4<P, R>
    where P: key::KeyParts,
          R: key::KeyRole,
{
    /// The security requirements of the hash algorithm for
    /// self-signatures.
    ///
    /// A cryptographic hash algorithm usually has [three security
    /// properties]: pre-image resistance, second pre-image
    /// resistance, and collision resistance.  If an attacker can
    /// influence the signed data, then the hash algorithm needs to
    /// have both second pre-image resistance, and collision
    /// resistance.  If not, second pre-image resistance is
    /// sufficient.
    ///
    ///   [three security properties]: https://en.wikipedia.org/wiki/Cryptographic_hash_function#Properties
    ///
    /// In general, an attacker may be able to influence third-party
    /// signatures.  But direct key signatures, and binding signatures
    /// are only over data fully determined by signer.  And, an
    /// attacker's control over self signatures over User IDs is
    /// limited due to their structure.
    ///
    /// These observations can be used to extend the life of a hash
    /// algorithm after its collision resistance has been partially
    /// compromised, but not completely broken.  For more details,
    /// please refer to the documentation for [HashAlgoSecurity].
    ///
    ///   [HashAlgoSecurity]: crate::policy::HashAlgoSecurity
    pub fn hash_algo_security(&self) -> HashAlgoSecurity {
        HashAlgoSecurity::SecondPreImageResistance
    }

    /// Compares the public bits of two keys.
    ///
    /// This returns `Ordering::Equal` if the public MPIs, creation
    /// time, and algorithm of the two `Key4`s match.  This does not
    /// consider the packets' encodings, packets' tags or their secret
    /// key material.
    pub fn public_cmp<PB, RB>(&self, b: &Key4<PB, RB>) -> Ordering
        where PB: key::KeyParts,
              RB: key::KeyRole,
    {
        self.mpis.cmp(&b.mpis)
            .then_with(|| self.creation_time.cmp(&b.creation_time))
            .then_with(|| self.pk_algo.cmp(&b.pk_algo))
    }

    /// Tests whether two keys are equal modulo their secret key
    /// material.
    ///
    /// This returns true if the public MPIs, creation time and
    /// algorithm of the two `Key4`s match.  This does not consider
    /// the packets' encodings, packets' tags or their secret key
    /// material.
    pub fn public_eq<PB, RB>(&self, b: &Key4<PB, RB>) -> bool
        where PB: key::KeyParts,
              RB: key::KeyRole,
    {
        self.public_cmp(b) == Ordering::Equal
    }

    /// Hashes everything but any secret key material into state.
    ///
    /// This is an alternate implementation of [`Hash`], which never
    /// hashes the secret key material.
    ///
    ///   [`Hash`]: std::hash::Hash
    pub fn public_hash<H>(&self, state: &mut H)
        where H: Hasher
    {
        use std::hash::Hash;

        self.common.hash(state);
        self.creation_time.hash(state);
        self.pk_algo.hash(state);
        Hash::hash(&self.mpis(), state);
    }
}

impl<P, R> Key4<P, R>
where
    P: key::KeyParts,
    R: key::KeyRole,
{
    /// Creates an OpenPGP public key from the specified key material.
    ///
    /// This is an internal version for parse.rs that avoids going
    /// through SystemTime.
    pub(crate) fn make<T>(creation_time: T,
                          pk_algo: PublicKeyAlgorithm,
                          mpis: mpi::PublicKey,
                          secret: Option<SecretKeyMaterial>)
                          -> Result<Self>
    where
        T: Into<Timestamp>,
    {
        Ok(Key4 {
            common: Default::default(),
            creation_time: creation_time.into(),
            pk_algo,
            mpis,
            secret,
            p: std::marker::PhantomData,
            r: std::marker::PhantomData,
        })
    }
}

impl<R> Key4<key::PublicParts, R>
    where R: key::KeyRole,
{
    /// Creates an OpenPGP public key from the specified key material.
    pub fn new<T>(creation_time: T, pk_algo: PublicKeyAlgorithm,
                  mpis: mpi::PublicKey)
                  -> Result<Self>
        where T: Into<time::SystemTime>
    {
        Ok(Key4 {
            common: Default::default(),
            creation_time: creation_time.into().try_into()?,
            pk_algo,
            mpis,
            secret: None,
            p: std::marker::PhantomData,
            r: std::marker::PhantomData,
        })
    }

    /// Creates an OpenPGP public key packet from existing X25519 key
    /// material.
    ///
    /// The ECDH key will use hash algorithm `hash` and symmetric
    /// algorithm `sym`.  If one or both are `None` secure defaults
    /// will be used.  The key will have its creation date set to
    /// `ctime` or the current time if `None` is given.
    pub fn import_public_cv25519<H, S, T>(public_key: &[u8],
                                          hash: H, sym: S, ctime: T)
        -> Result<Self> where H: Into<Option<HashAlgorithm>>,
                              S: Into<Option<SymmetricAlgorithm>>,
                              T: Into<Option<time::SystemTime>>
    {
        let mut point = Vec::from(public_key);
        point.insert(0, 0x40);

        use crate::crypto::ecdh;
        Self::new(
            ctime.into().unwrap_or_else(crate::now),
            PublicKeyAlgorithm::ECDH,
            mpi::PublicKey::ECDH {
                curve: Curve::Cv25519,
                hash: hash.into().unwrap_or_else(
                    || ecdh::default_ecdh_kdf_hash(&Curve::Cv25519)),
                sym: sym.into().unwrap_or_else(
                    || ecdh::default_ecdh_kek_cipher(&Curve::Cv25519)),
                q: mpi::MPI::new(&point),
            })
    }

    /// Creates an OpenPGP public key packet from existing Ed25519 key
    /// material.
    ///
    /// The ECDH key will use hash algorithm `hash` and symmetric
    /// algorithm `sym`.  If one or both are `None` secure defaults
    /// will be used.  The key will have its creation date set to
    /// `ctime` or the current time if `None` is given.
    pub fn import_public_ed25519<T>(public_key: &[u8], ctime: T) -> Result<Self>
        where  T: Into<Option<time::SystemTime>>
    {
        let mut point = Vec::from(public_key);
        point.insert(0, 0x40);

        Self::new(
            ctime.into().unwrap_or_else(crate::now),
            PublicKeyAlgorithm::EdDSA,
            mpi::PublicKey::EdDSA {
                curve: Curve::Ed25519,
                q: mpi::MPI::new(&point),
            })
    }

    /// Creates an OpenPGP public key packet from existing RSA key
    /// material.
    ///
    /// The RSA key will use the public exponent `e` and the modulo
    /// `n`. The key will have its creation date set to `ctime` or the
    /// current time if `None` is given.
    pub fn import_public_rsa<T>(e: &[u8], n: &[u8], ctime: T)
        -> Result<Self> where T: Into<Option<time::SystemTime>>
    {
        Self::new(
            ctime.into().unwrap_or_else(crate::now),
            PublicKeyAlgorithm::RSAEncryptSign,
            mpi::PublicKey::RSA {
                e: mpi::MPI::new(e),
                n: mpi::MPI::new(n),
            })
    }
}

impl<R> Key4<SecretParts, R>
    where R: key::KeyRole,
{
    /// Creates an OpenPGP key packet from the specified secret key
    /// material.
    pub fn with_secret<T>(creation_time: T, pk_algo: PublicKeyAlgorithm,
                          mpis: mpi::PublicKey,
                          secret: SecretKeyMaterial)
                          -> Result<Self>
        where T: Into<time::SystemTime>
    {
        Ok(Key4 {
            common: Default::default(),
            creation_time: creation_time.into().try_into()?,
            pk_algo,
            mpis,
            secret: Some(secret),
            p: std::marker::PhantomData,
            r: std::marker::PhantomData,
        })
    }
}

impl<P, R> Key4<P, R>
     where P: key::KeyParts,
           R: key::KeyRole,
{
    /// Gets the `Key`'s creation time.
    pub fn creation_time(&self) -> time::SystemTime {
        self.creation_time.into()
    }

    /// Gets the `Key`'s creation time without converting it to a
    /// system time.
    ///
    /// This conversion may truncate the time to signed 32-bit time_t.
    pub(crate) fn creation_time_raw(&self) -> Timestamp {
        self.creation_time
    }

    /// Sets the `Key`'s creation time.
    ///
    /// `timestamp` is converted to OpenPGP's internal format,
    /// [`Timestamp`]: a 32-bit quantity containing the number of
    /// seconds since the Unix epoch.
    ///
    /// `timestamp` is silently rounded to match the internal
    /// resolution.  An error is returned if `timestamp` is out of
    /// range.
    ///
    /// [`Timestamp`]: crate::types::Timestamp
    pub fn set_creation_time<T>(&mut self, timestamp: T)
                                -> Result<time::SystemTime>
        where T: Into<time::SystemTime>
    {
        Ok(std::mem::replace(&mut self.creation_time,
                             timestamp.into().try_into()?)
           .into())
    }

    /// Gets the public key algorithm.
    pub fn pk_algo(&self) -> PublicKeyAlgorithm {
        self.pk_algo
    }

    /// Sets the public key algorithm.
    ///
    /// Returns the old public key algorithm.
    pub fn set_pk_algo(&mut self, pk_algo: PublicKeyAlgorithm)
        -> PublicKeyAlgorithm
    {
        ::std::mem::replace(&mut self.pk_algo, pk_algo)
    }

    /// Returns a reference to the `Key`'s MPIs.
    pub fn mpis(&self) -> &mpi::PublicKey {
        &self.mpis
    }

    /// Returns a mutable reference to the `Key`'s MPIs.
    pub fn mpis_mut(&mut self) -> &mut mpi::PublicKey {
        &mut self.mpis
    }

    /// Sets the `Key`'s MPIs.
    ///
    /// This function returns the old MPIs, if any.
    pub fn set_mpis(&mut self, mpis: mpi::PublicKey) -> mpi::PublicKey {
        ::std::mem::replace(&mut self.mpis, mpis)
    }

    /// Returns whether the `Key` contains secret key material.
    pub fn has_secret(&self) -> bool {
        self.secret.is_some()
    }

    /// Returns whether the `Key` contains unencrypted secret key
    /// material.
    ///
    /// This returns false if the `Key` doesn't contain any secret key
    /// material.
    pub fn has_unencrypted_secret(&self) -> bool {
        matches!(self.secret, Some(SecretKeyMaterial::Unencrypted { .. }))
    }

    /// Returns `Key`'s secret key material, if any.
    pub fn optional_secret(&self) -> Option<&SecretKeyMaterial> {
        self.secret.as_ref()
    }

    /// Computes and returns the `Key`'s `Fingerprint` and returns it as
    /// a `KeyHandle`.
    ///
    /// See [Section 12.2 of RFC 4880].
    ///
    /// [Section 12.2 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-12.2
    pub fn key_handle(&self) -> KeyHandle {
        self.fingerprint().into()
    }

    /// Computes and returns the `Key`'s `Fingerprint`.
    ///
    /// See [Section 12.2 of RFC 4880].
    ///
    /// [Section 12.2 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-12.2
    pub fn fingerprint(&self) -> Fingerprint {
        let mut h = HashAlgorithm::SHA1.context().unwrap();

        self.hash(&mut h);

        let mut digest = vec![0u8; h.digest_size()];
        let _ = h.digest(&mut digest);
        Fingerprint::from_bytes(digest.as_slice())
    }

    /// Computes and returns the `Key`'s `Key ID`.
    ///
    /// See [Section 12.2 of RFC 4880].
    ///
    /// [Section 12.2 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-12.2
    pub fn keyid(&self) -> KeyID {
        self.fingerprint().into()
    }
}

macro_rules! impl_common_secret_functions {
    ($t: ident) => {
        /// Secret key material handling.
        impl<R> Key4<$t, R>
            where R: key::KeyRole,
        {
            /// Takes the `Key`'s `SecretKeyMaterial`, if any.
            pub fn take_secret(mut self)
                               -> (Key4<PublicParts, R>, Option<SecretKeyMaterial>)
            {
                let old = std::mem::replace(&mut self.secret, None);
                (self.parts_into_public(), old)
            }

            /// Adds the secret key material to the `Key`, returning
            /// the old secret key material, if any.
            pub fn add_secret(mut self, secret: SecretKeyMaterial)
                              -> (Key4<SecretParts, R>, Option<SecretKeyMaterial>)
            {
                let old = std::mem::replace(&mut self.secret, Some(secret));
                (self.parts_into_secret().expect("secret just set"), old)
            }
        }
    }
}
impl_common_secret_functions!(PublicParts);
impl_common_secret_functions!(UnspecifiedParts);

/// Secret key handling.
impl<R> Key4<SecretParts, R>
    where R: key::KeyRole,
{
    /// Gets the `Key`'s `SecretKeyMaterial`.
    pub fn secret(&self) -> &SecretKeyMaterial {
        self.secret.as_ref().expect("has secret")
    }

    /// Gets a mutable reference to the `Key`'s `SecretKeyMaterial`.
    pub fn secret_mut(&mut self) -> &mut SecretKeyMaterial {
        self.secret.as_mut().expect("has secret")
    }

    /// Takes the `Key`'s `SecretKeyMaterial`.
    pub fn take_secret(mut self)
                       -> (Key4<PublicParts, R>, SecretKeyMaterial)
    {
        let old = std::mem::replace(&mut self.secret, None);
        (self.parts_into_public(),
         old.expect("Key<SecretParts, _> has a secret key material"))
    }

    /// Adds `SecretKeyMaterial` to the `Key`.
    ///
    /// This function returns the old secret key material, if any.
    pub fn add_secret(mut self, secret: SecretKeyMaterial)
                      -> (Key4<SecretParts, R>, SecretKeyMaterial)
    {
        let old = std::mem::replace(&mut self.secret, Some(secret));
        (self.parts_into_secret().expect("secret just set"),
         old.expect("Key<SecretParts, _> has a secret key material"))
    }

    /// Decrypts the secret key material using `password`.
    ///
    /// In OpenPGP, secret key material can be [protected with a
    /// password].  The password is usually hardened using a [KDF].
    ///
    /// Refer to the documentation of [`Key::decrypt_secret`] for
    /// details.
    ///
    /// This function returns an error if the secret key material is
    /// not encrypted or the password is incorrect.
    ///
    /// [protected with a password]: https://tools.ietf.org/html/rfc4880#section-5.5.3
    /// [KDF]: https://tools.ietf.org/html/rfc4880#section-3.7
    /// [`Key::decrypt_secret`]: super::Key::decrypt_secret()
    pub fn decrypt_secret(mut self, password: &Password) -> Result<Self> {
        let pk_algo = self.pk_algo;
        self.secret_mut().decrypt_in_place(pk_algo, password)?;
        Ok(self)
    }

    /// Encrypts the secret key material using `password`.
    ///
    /// In OpenPGP, secret key material can be [protected with a
    /// password].  The password is usually hardened using a [KDF].
    ///
    /// Refer to the documentation of [`Key::encrypt_secret`] for
    /// details.
    ///
    /// This returns an error if the secret key material is already
    /// encrypted.
    ///
    /// [protected with a password]: https://tools.ietf.org/html/rfc4880#section-5.5.3
    /// [KDF]: https://tools.ietf.org/html/rfc4880#section-3.7
    /// [`Key::encrypt_secret`]: super::Key::encrypt_secret()
    pub fn encrypt_secret(mut self, password: &Password)
        -> Result<Key4<SecretParts, R>>
    {
        self.secret_mut().encrypt_in_place(password)?;
        Ok(self)
    }
}

impl<P, R> From<Key4<P, R>> for super::Key<P, R>
    where P: key::KeyParts,
          R: key::KeyRole,
{
    fn from(p: Key4<P, R>) -> Self {
        super::Key::V4(p)
    }
}

/// Holds secret key material.
///
/// This type allows postponing the decryption of the secret key
/// material until it is actually needed.
///
/// If the secret key material is not encrypted with a password, then
/// we encrypt it in memory.  This helps protect against
/// [heartbleed]-style attacks where a buffer over-read allows an
/// attacker to read from the process's address space.  This
/// protection is less important for Rust programs, which are memory
/// safe.  However, it is essential when Sequoia is used via its FFI.
///
/// See [`crypto::mem::Encrypted`] for details.
///
/// [heartbleed]: https://en.wikipedia.org/wiki/Heartbleed
/// [`crypto::mem::Encrypted`]: super::super::crypto::mem::Encrypted
#[derive(PartialEq, Eq, Hash, Clone, Debug)]
pub enum SecretKeyMaterial {
    /// Unencrypted secret key. Can be used as-is.
    Unencrypted(Unencrypted),
    /// The secret key is encrypted with a password.
    Encrypted(Encrypted),
}

assert_send_and_sync!(SecretKeyMaterial);

impl From<mpi::SecretKeyMaterial> for SecretKeyMaterial {
    fn from(mpis: mpi::SecretKeyMaterial) -> Self {
        SecretKeyMaterial::Unencrypted(mpis.into())
    }
}

impl From<Unencrypted> for SecretKeyMaterial {
    fn from(key: Unencrypted) -> Self {
        SecretKeyMaterial::Unencrypted(key)
    }
}

impl From<Encrypted> for SecretKeyMaterial {
    fn from(key: Encrypted) -> Self {
        SecretKeyMaterial::Encrypted(key)
    }
}

impl SecretKeyMaterial {
    /// Decrypts the secret key material using `password`.
    ///
    /// The `SecretKeyMaterial` type does not know what kind of key it
    /// contains.  So, in order to know how many MPIs to parse, the
    /// public key algorithm needs to be provided explicitly.
    ///
    /// This returns an error if the secret key material is not
    /// encrypted or the password is incorrect.
    pub fn decrypt(mut self, pk_algo: PublicKeyAlgorithm,
                   password: &Password)
        -> Result<Self>
    {
        self.decrypt_in_place(pk_algo, password)?;
        Ok(self)
    }

    /// Decrypts the secret key material using `password`.
    ///
    /// The `SecretKeyMaterial` type does not know what kind of key it
    /// contains.  So, in order to know how many MPIs to parse, the
    /// public key algorithm needs to be provided explicitly.
    ///
    /// This returns an error if the secret key material is not
    /// encrypted or the password is incorrect.
    pub fn decrypt_in_place(&mut self, pk_algo: PublicKeyAlgorithm,
                            password: &Password)
        -> Result<()>
    {
        match self {
            SecretKeyMaterial::Encrypted(e) => {
                *self = e.decrypt(pk_algo, password)?.into();
                Ok(())
            }
            SecretKeyMaterial::Unencrypted(_) =>
                Err(Error::InvalidArgument(
                    "secret key is not encrypted".into()).into()),
        }
    }

    /// Encrypts the secret key material using `password`.
    ///
    /// This returns an error if the secret key material is encrypted.
    ///
    /// See [`Unencrypted::encrypt`] for details.
    pub fn encrypt(mut self, password: &Password) -> Result<Self> {
        self.encrypt_in_place(password)?;
        Ok(self)
    }

    /// Encrypts the secret key material using `password`.
    ///
    /// This returns an error if the secret key material is encrypted.
    ///
    /// See [`Unencrypted::encrypt`] for details.
    pub fn encrypt_in_place(&mut self, password: &Password) -> Result<()> {
        match self {
            SecretKeyMaterial::Unencrypted(ref u) => {
                *self = SecretKeyMaterial::Encrypted(
                    u.encrypt(password)?);
                Ok(())
            }
            SecretKeyMaterial::Encrypted(_) =>
                Err(Error::InvalidArgument(
                    "secret key is encrypted".into()).into()),
        }
    }

    /// Returns whether the secret key material is encrypted.
    pub fn is_encrypted(&self) -> bool {
        match self {
            SecretKeyMaterial::Encrypted(_) => true,
            SecretKeyMaterial::Unencrypted(_) => false,
        }
    }
}

/// Unencrypted secret key material.
///
/// This data structure is used by the [`SecretKeyMaterial`] enum.
///
/// Unlike an [`Encrypted`] key, this key an be used as-is.
///
/// The secret key is encrypted in memory and only decrypted on
/// demand.  This helps protect against [heartbleed]-style
/// attacks where a buffer over-read allows an attacker to read from
/// the process's address space.  This protection is less important
/// for Rust programs, which are memory safe.  However, it is
/// essential when Sequoia is used via its FFI.
///
/// See [`crypto::mem::Encrypted`] for details.
///
/// [heartbleed]: https://en.wikipedia.org/wiki/Heartbleed
/// [`crypto::mem::Encrypted`]: super::super::crypto::mem::Encrypted
// Note: PartialEq, Eq, and Hash on mem::Encrypted does the right
// thing.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Unencrypted {
    /// MPIs of the secret key.
    mpis: mem::Encrypted,
}

assert_send_and_sync!(Unencrypted);

impl From<mpi::SecretKeyMaterial> for Unencrypted {
    fn from(mpis: mpi::SecretKeyMaterial) -> Self {
        use crate::serialize::Marshal;
        // We need to store the type.
        let mut plaintext =
            vec![mpis.algo().unwrap_or(PublicKeyAlgorithm::Unknown(0)).into()];
        mpis.serialize(&mut plaintext)
            .expect("MPI serialization to vec failed");
        Unencrypted { mpis: mem::Encrypted::new(plaintext.into()), }
    }
}

impl Unencrypted {
    /// Maps the given function over the secret.
    pub fn map<F, T>(&self, mut fun: F) -> T
        where F: FnMut(&mpi::SecretKeyMaterial) -> T
    {
        self.mpis.map(|plaintext| {
            let algo: PublicKeyAlgorithm = plaintext[0].into();
            let mpis = mpi::SecretKeyMaterial::parse(algo, &plaintext[1..])
                .expect("Decrypted secret key is malformed");
            fun(&mpis)
        })
    }

    /// Encrypts the secret key material using `password`.
    ///
    /// This encrypts the secret key material using an [AES 256] key
    /// derived from the `password` using the default [`S2K`] scheme.
    ///
    /// [AES 256]: crate::types::SymmetricAlgorithm::AES256
    /// [`S2K`]: super::super::crypto::S2K
    pub fn encrypt(&self, password: &Password)
        -> Result<Encrypted>
    {
        use std::io::Write;
        use crate::crypto::symmetric::Encryptor;

        let s2k = S2K::default();
        let algo = SymmetricAlgorithm::AES256;
        let key = s2k.derive_key(password, algo.key_size()?)?;

        // Ciphertext is preceded by a random block.
        let mut trash = vec![0u8; algo.block_size()?];
        crypto::random(&mut trash);

        let checksum = Default::default();
        let mut esk = Vec::new();
        {
            let mut encryptor = Encryptor::new(algo, &key, &mut esk)?;
            encryptor.write_all(&trash)?;
            self.map(|mpis| mpis.serialize_with_checksum(&mut encryptor,
                                                         checksum))?;
        }

        Ok(Encrypted::new(s2k, algo, Some(checksum), esk.into_boxed_slice()))
    }
}

/// Secret key material encrypted with a password.
///
/// This data structure is used by the [`SecretKeyMaterial`] enum.
///
#[derive(Clone, Debug)]
pub struct Encrypted {
    /// Key derivation mechanism to use.
    s2k: S2K,
    /// Symmetric algorithm used to encrypt the secret key material.
    algo: SymmetricAlgorithm,
    /// Checksum method.
    checksum: Option<mpi::SecretKeyChecksum>,
    /// Encrypted MPIs prefixed with the IV.
    ///
    /// If we recognized the S2K object during parsing, we can
    /// successfully parse the data into S2K, IV, and ciphertext.
    /// However, if we do not recognize the S2K type, we do not know
    /// how large its parameters are, so we cannot cleanly parse it,
    /// and have to accept that the S2K's body bleeds into the rest of
    /// the data.
    ciphertext: std::result::Result<Box<[u8]>,  // IV + ciphertext.
                                    Box<[u8]>>, // S2K body + IV + ciphertext.
}

assert_send_and_sync!(Encrypted);

// Because the S2K and ciphertext cannot be cleanly separated at parse
// time, we need to carefully compare and hash encrypted key packets.

impl PartialEq for Encrypted {
    fn eq(&self, other: &Encrypted) -> bool {
        self.algo == other.algo
            && self.checksum == other.checksum
            // Treat S2K and ciphertext as opaque blob.
            && {
                // XXX: This would be nicer without the allocations.
                use crate::serialize::MarshalInto;
                let mut a = self.s2k.to_vec().unwrap();
                let mut b = other.s2k.to_vec().unwrap();
                a.extend_from_slice(self.raw_ciphertext());
                b.extend_from_slice(other.raw_ciphertext());
                a == b
            }
    }
}

impl Eq for Encrypted {}

impl std::hash::Hash for Encrypted {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.algo.hash(state);
        self.checksum.hash(state);
        // Treat S2K and ciphertext as opaque blob.
        // XXX: This would be nicer without the allocations.
        use crate::serialize::MarshalInto;
        let mut a = self.s2k.to_vec().unwrap();
        a.extend_from_slice(self.raw_ciphertext());
        a.hash(state);
    }
}

impl Encrypted {
    /// Creates a new encrypted key object.
    pub fn new(s2k: S2K, algo: SymmetricAlgorithm,
               checksum: Option<mpi::SecretKeyChecksum>, ciphertext: Box<[u8]>)
        -> Self
    {
        Self::new_raw(s2k, algo, checksum, Ok(ciphertext))
    }

    /// Creates a new encrypted key object.
    pub(crate) fn new_raw(s2k: S2K, algo: SymmetricAlgorithm,
                          checksum: Option<mpi::SecretKeyChecksum>,
                          ciphertext: std::result::Result<Box<[u8]>,
                                                          Box<[u8]>>)
        -> Self
    {
        Encrypted { s2k, algo, checksum, ciphertext }
    }

    /// Returns the key derivation mechanism.
    pub fn s2k(&self) -> &S2K {
        &self.s2k
    }

    /// Returns the symmetric algorithm used to encrypt the secret
    /// key material.
    pub fn algo(&self) -> SymmetricAlgorithm {
        self.algo
    }

    /// Returns the checksum method used to protect the encrypted
    /// secret key material, if any.
    pub fn checksum(&self) -> Option<mpi::SecretKeyChecksum> {
        self.checksum
    }

    /// Returns the encrypted secret key material.
    ///
    /// If the [`S2K`] mechanism is not supported by Sequoia, this
    /// function will fail.  Note that the information is not lost,
    /// but stored in the packet.  If the packet is serialized again,
    /// it is written out.
    ///
    ///   [`S2K`]: super::super::crypto::S2K
    pub fn ciphertext(&self) -> Result<&[u8]> {
        self.ciphertext
            .as_ref()
            .map(|ciphertext| &ciphertext[..])
            .map_err(|_| Error::MalformedPacket(
                format!("Unknown S2K: {:?}", self.s2k)).into())
    }

    /// Returns the encrypted secret key material, possibly including
    /// the body of the S2K object.
    pub(crate) fn raw_ciphertext(&self) -> &[u8] {
        match self.ciphertext.as_ref() {
            Ok(ciphertext) => &ciphertext[..],
            Err(s2k_ciphertext) => &s2k_ciphertext[..],
        }
    }

    /// Decrypts the secret key material using `password`.
    ///
    /// The `Encrypted` key does not know what kind of key it is, so
    /// the public key algorithm is needed to parse the correct number
    /// of MPIs.
    pub fn decrypt(&self, pk_algo: PublicKeyAlgorithm, password: &Password)
        -> Result<Unencrypted>
    {
        use std::io::{Cursor, Read};
        use crate::crypto::symmetric::Decryptor;

        let key = self.s2k.derive_key(password, self.algo.key_size()?)?;
        let cur = Cursor::new(self.ciphertext()?);
        let mut dec = Decryptor::new(self.algo, &key, cur)?;

        // Consume the first block.
        let mut trash = vec![0u8; self.algo.block_size()?];
        dec.read_exact(&mut trash)?;

        mpi::SecretKeyMaterial::parse_with_checksum(
            pk_algo, &mut dec, self.checksum.unwrap_or_default())
            .map(|m| m.into())
    }
}

#[cfg(test)]
impl<P, R> Arbitrary for super::Key<P, R>
    where P: KeyParts, P: Clone,
          R: KeyRole, R: Clone,
          Key4<P, R>: Arbitrary,
{
    fn arbitrary(g: &mut Gen) -> Self {
        Key4::arbitrary(g).into()
    }
}

#[cfg(test)]
impl Arbitrary for Key4<PublicParts, PrimaryRole> {
    fn arbitrary(g: &mut Gen) -> Self {
        Key4::<PublicParts, UnspecifiedRole>::arbitrary(g).into()
    }
}

#[cfg(test)]
impl Arbitrary for Key4<PublicParts, SubordinateRole> {
    fn arbitrary(g: &mut Gen) -> Self {
        Key4::<PublicParts, UnspecifiedRole>::arbitrary(g).into()
    }
}

#[cfg(test)]
impl Arbitrary for Key4<PublicParts, UnspecifiedRole> {
    fn arbitrary(g: &mut Gen) -> Self {
        let mpis = mpi::PublicKey::arbitrary(g);
        Key4 {
            common: Arbitrary::arbitrary(g),
            creation_time: Arbitrary::arbitrary(g),
            pk_algo: mpis.algo()
                .expect("mpi::PublicKey::arbitrary only uses known algos"),
            mpis,
            secret: None,
            p: std::marker::PhantomData,
            r: std::marker::PhantomData,
        }
    }
}

#[cfg(test)]
impl Arbitrary for Key4<SecretParts, PrimaryRole> {
    fn arbitrary(g: &mut Gen) -> Self {
        Key4::<SecretParts, UnspecifiedRole>::arbitrary(g).into()
    }
}

#[cfg(test)]
impl Arbitrary for Key4<SecretParts, SubordinateRole> {
    fn arbitrary(g: &mut Gen) -> Self {
        Key4::<SecretParts, UnspecifiedRole>::arbitrary(g).into()
    }
}

#[cfg(test)]
impl Arbitrary for Key4<SecretParts, UnspecifiedRole> {
    fn arbitrary(g: &mut Gen) -> Self {
        use PublicKeyAlgorithm::*;
        use mpi::MPI;

        let key = Key4::arbitrary(g);
        let mut secret: SecretKeyMaterial = match key.pk_algo() {
            RSAEncryptSign => mpi::SecretKeyMaterial::RSA {
                d: MPI::arbitrary(g).into(),
                p: MPI::arbitrary(g).into(),
                q: MPI::arbitrary(g).into(),
                u: MPI::arbitrary(g).into(),
            },

            DSA => mpi::SecretKeyMaterial::DSA {
                x: MPI::arbitrary(g).into(),
            },

            ElGamalEncrypt => mpi::SecretKeyMaterial::ElGamal {
                x: MPI::arbitrary(g).into(),
            },

            EdDSA => mpi::SecretKeyMaterial::EdDSA {
                scalar: MPI::arbitrary(g).into(),
            },

            ECDSA => mpi::SecretKeyMaterial::ECDSA {
                scalar: MPI::arbitrary(g).into(),
            },

            ECDH => mpi::SecretKeyMaterial::ECDH {
                scalar: MPI::arbitrary(g).into(),
            },

            _ => unreachable!("only valid algos, normalizes to these values"),
        }.into();

        if <bool>::arbitrary(g) {
            secret.encrypt_in_place(&Password::from(Vec::arbitrary(g)))
                .unwrap();
        }

        Key4::<PublicParts, UnspecifiedRole>::add_secret(key, secret).0
    }
}

#[cfg(test)]
mod tests {
    use crate::packet::Key;
    use crate::Cert;
    use crate::packet::pkesk::PKESK3;
    use crate::packet::key;
    use crate::packet::key::SecretKeyMaterial;
    use crate::packet::Packet;
    use super::*;
    use crate::PacketPile;
    use crate::serialize::Serialize;
    use crate::parse::Parse;

    #[test]
    fn encrypted_rsa_key() {
        let cert = Cert::from_bytes(
            crate::tests::key("testy-new-encrypted-with-123.pgp")).unwrap();
        let mut pair = cert.primary_key().key().clone();
        let pk_algo = pair.pk_algo();
        let secret = pair.secret.as_mut().unwrap();

        assert!(secret.is_encrypted());
        secret.decrypt_in_place(pk_algo, &"123".into()).unwrap();
        assert!(!secret.is_encrypted());

        match secret {
            SecretKeyMaterial::Unencrypted(ref u) => u.map(|mpis| match mpis {
                mpi::SecretKeyMaterial::RSA { .. } => (),
                _ => panic!(),
            }),
            _ => panic!(),
        }
    }

    #[test]
    fn key_encrypt_decrypt() -> Result<()> {
        let mut g = quickcheck::Gen::new(256);
        let p: Password = Vec::<u8>::arbitrary(&mut g).into();

        let check = |key: Key4<SecretParts, UnspecifiedRole>| -> Result<()> {
            let key: Key<_, _> = key.into();
            let encrypted = key.clone().encrypt_secret(&p)?;
            let decrypted = encrypted.decrypt_secret(&p)?;
            assert_eq!(key, decrypted);
            Ok(())
        };

        use crate::types::Curve::*;
        for curve in vec![NistP256, NistP384, NistP521, Ed25519] {
            if ! curve.is_supported() {
                eprintln!("Skipping unsupported {}", curve);
                continue;
            }

            let key: Key4<_, key::UnspecifiedRole>
                = Key4::generate_ecc(true, curve.clone())?;
            check(key)?;
        }

        for bits in vec![2048, 3072] {
            if ! PublicKeyAlgorithm::RSAEncryptSign.is_supported() {
                eprintln!("Skipping unsupported RSA");
                continue;
            }

            let key: Key4<_, key::UnspecifiedRole>
                = Key4::generate_rsa(bits)?;
            check(key)?;
        }

        Ok(())
    }

    #[test]
    fn eq() {
        use crate::types::Curve::*;

        for curve in vec![NistP256, NistP384, NistP521] {
            if ! curve.is_supported() {
                eprintln!("Skipping unsupported {}", curve);
                continue;
            }

            let sign_key : Key4<_, key::UnspecifiedRole>
                = Key4::generate_ecc(true, curve.clone()).unwrap();
            let enc_key : Key4<_, key::UnspecifiedRole>
                = Key4::generate_ecc(false, curve).unwrap();
            let sign_clone = sign_key.clone();
            let enc_clone = enc_key.clone();

            assert_eq!(sign_key, sign_clone);
            assert_eq!(enc_key, enc_clone);
        }

        for bits in vec![1024, 2048, 3072, 4096] {
            if ! PublicKeyAlgorithm::RSAEncryptSign.is_supported() {
                eprintln!("Skipping unsupported RSA");
                continue;
            }

            let key : Key4<_, key::UnspecifiedRole>
                = Key4::generate_rsa(bits).unwrap();
            let clone = key.clone();
            assert_eq!(key, clone);
        }
    }

    #[test]
    fn roundtrip() {
        use crate::types::Curve::*;

        let keys = vec![NistP256, NistP384, NistP521].into_iter().flat_map(|cv|
        {
            if ! cv.is_supported() {
                eprintln!("Skipping unsupported {}", cv);
                return Vec::new();
            }

            let sign_key : Key4<key::SecretParts, key::PrimaryRole>
                = Key4::generate_ecc(true, cv.clone()).unwrap();
            let enc_key = Key4::generate_ecc(false, cv).unwrap();

            vec![sign_key, enc_key]
        }).chain(vec![1024, 2048, 3072, 4096].into_iter().filter_map(|b| {
            Key4::generate_rsa(b).ok()
        }));

        for key in keys {
            let mut b = Vec::new();
            Packet::SecretKey(key.clone().into()).serialize(&mut b).unwrap();

            let pp = PacketPile::from_bytes(&b).unwrap();
            if let Some(Packet::SecretKey(Key::V4(ref parsed_key))) =
                pp.path_ref(&[0])
            {
                assert_eq!(key.creation_time, parsed_key.creation_time);
                assert_eq!(key.pk_algo, parsed_key.pk_algo);
                assert_eq!(key.mpis, parsed_key.mpis);
                assert_eq!(key.secret, parsed_key.secret);

                assert_eq!(&key, parsed_key);
            } else {
                panic!("bad packet: {:?}", pp.path_ref(&[0]));
            }

            let mut b = Vec::new();
            let pk4 : Key4<PublicParts, PrimaryRole> = key.clone().into();
            Packet::PublicKey(pk4.into()).serialize(&mut b).unwrap();

            let pp = PacketPile::from_bytes(&b).unwrap();
            if let Some(Packet::PublicKey(Key::V4(ref parsed_key))) =
                pp.path_ref(&[0])
            {
                assert!(! parsed_key.has_secret());

                let key = key.take_secret().0;
                assert_eq!(&key, parsed_key);
            } else {
                panic!("bad packet: {:?}", pp.path_ref(&[0]));
            }
        }
    }

    #[test]
    fn encryption_roundtrip() {
        use crate::crypto::SessionKey;
        use crate::types::Curve::*;

        let keys = vec![NistP256, NistP384, NistP521].into_iter()
            .filter_map(|cv| {
                Key4::generate_ecc(false, cv).ok()
            }).chain(vec![1024, 2048, 3072, 4096].into_iter().filter_map(|b| {
                Key4::generate_rsa(b).ok()
            }));

        for key in keys.into_iter() {
            let key: Key<key::SecretParts, key::UnspecifiedRole> = key.into();
            let mut keypair = key.clone().into_keypair().unwrap();
            let cipher = SymmetricAlgorithm::AES256;
            let sk = SessionKey::new(cipher.key_size().unwrap());

            let pkesk = PKESK3::for_recipient(cipher, &sk, &key).unwrap();
            let (cipher_, sk_) = pkesk.decrypt(&mut keypair, None).unwrap();

            assert_eq!(cipher, cipher_);
            assert_eq!(sk, sk_);

            let (cipher_, sk_) =
                pkesk.decrypt(&mut keypair, Some(cipher)).unwrap();

            assert_eq!(cipher, cipher_);
            assert_eq!(sk, sk_);
        }
    }

    #[test]
    fn secret_encryption_roundtrip() {
        use crate::types::Curve::*;

        let keys = vec![NistP256, NistP384, NistP521].into_iter()
            .filter_map(|cv| -> Option<Key4<key::SecretParts, key::PrimaryRole>> {
                Key4::generate_ecc(false, cv).ok()
            }).chain(vec![1024, 2048, 3072, 4096].into_iter().filter_map(|b| {
                Key4::generate_rsa(b).ok()
            }));

        for key in keys {
            assert!(! key.secret().is_encrypted());

            let password = Password::from("foobarbaz");
            let mut encrypted_key = key.clone();

            encrypted_key.secret_mut().encrypt_in_place(&password).unwrap();
            assert!(encrypted_key.secret().is_encrypted());

            encrypted_key.secret_mut()
                .decrypt_in_place(key.pk_algo, &password).unwrap();
            assert!(! key.secret().is_encrypted());
            assert_eq!(key, encrypted_key);
            assert_eq!(key.secret(), encrypted_key.secret());
        }
    }

    #[test]
    fn import_cv25519() {
        use crate::crypto::{ecdh, mem, SessionKey};
        use self::mpi::{MPI, Ciphertext};

        // X25519 key
        let ctime =
            time::UNIX_EPOCH + time::Duration::new(0x5c487129, 0);
        let public = b"\xed\x59\x0a\x15\x08\x95\xe9\x92\xd2\x2c\x14\x01\xb3\xe9\x3b\x7f\xff\xe6\x6f\x22\x65\xec\x69\xd9\xb8\xda\x24\x2c\x64\x84\x44\x11";
        let key : Key<_, key::UnspecifiedRole>
            = Key4::import_public_cv25519(&public[..],
                                          HashAlgorithm::SHA256,
                                          SymmetricAlgorithm::AES128,
                                          ctime).unwrap().into();

        // PKESK
        let eph_pubkey = MPI::new(&b"\x40\xda\x1c\x69\xc4\xe3\xb6\x9c\x6e\xd4\xc6\x69\x6c\x89\xc7\x09\xe9\xf8\x6a\xf1\xe3\x8d\xb6\xaa\xb5\xf7\x29\xae\xa6\xe7\xdd\xfe\x38"[..]);
        let ciphertext = Ciphertext::ECDH{
            e: eph_pubkey.clone(),
            key: Vec::from(&b"\x45\x8b\xd8\x4d\x88\xb3\xd2\x16\xb6\xc2\x3b\x99\x33\xd1\x23\x4b\x10\x15\x8e\x04\x16\xc5\x7c\x94\x88\xf6\x63\xf2\x68\x37\x08\x66\xfd\x5a\x7b\x40\x58\x21\x6b\x2c\xc0\xf4\xdc\x91\xd3\x48\xed\xc1"[..]).into_boxed_slice()
        };
        let shared_sec: mem::Protected = b"\x44\x0C\x99\x27\xF7\xD6\x1E\xAD\xD1\x1E\x9E\xC8\x22\x2C\x5D\x43\xCE\xB0\xE5\x45\x94\xEC\xAF\x67\xD9\x35\x1D\xA1\xA3\xA8\x10\x0B"[..].into();

        // Session key
        let dek = b"\x09\x0D\xDC\x40\xC5\x71\x51\x88\xAC\xBD\x45\x56\xD4\x2A\xDF\x77\xCD\xF4\x82\xA2\x1B\x8F\x2E\x48\x3B\xCA\xBF\xD3\xE8\x6D\x0A\x7C\xDF\x10\xe6";
        let sk = SessionKey::from(Vec::from(&dek[..]));

        // Expected
        let got_enc = ecdh::encrypt_wrap(&key.parts_into_public(),
                                           &sk, eph_pubkey, &shared_sec)
            .unwrap();

        assert_eq!(ciphertext, got_enc);
    }

    #[test]
    fn import_cv25519_sec() {
        use crate::crypto::ecdh;
        use self::mpi::{MPI, Ciphertext};

        // X25519 key
        let ctime =
            time::UNIX_EPOCH + time::Duration::new(0x5c487129, 0);
        let public = b"\xed\x59\x0a\x15\x08\x95\xe9\x92\xd2\x2c\x14\x01\xb3\xe9\x3b\x7f\xff\xe6\x6f\x22\x65\xec\x69\xd9\xb8\xda\x24\x2c\x64\x84\x44\x11";
        let secret = b"\xa0\x27\x13\x99\xc9\xe3\x2e\xd2\x47\xf6\xd6\x63\x9d\xe6\xec\xcb\x57\x0b\x92\xbb\x17\xfe\xb8\xf1\xc4\x1f\x06\x7c\x55\xfc\xdd\x58";
        let key: Key<_, UnspecifiedRole>
            = Key4::import_secret_cv25519(&secret[..],
                                          HashAlgorithm::SHA256,
                                          SymmetricAlgorithm::AES128,
                                          ctime).unwrap().into();
        match key.mpis {
            self::mpi::PublicKey::ECDH{ ref q,.. } =>
                assert_eq!(&q.value()[1..], &public[..]),
            _ => unreachable!(),
        }

        // PKESK
        let eph_pubkey: &[u8; 33] = b"\x40\xda\x1c\x69\xc4\xe3\xb6\x9c\x6e\xd4\xc6\x69\x6c\x89\xc7\x09\xe9\xf8\x6a\xf1\xe3\x8d\xb6\xaa\xb5\xf7\x29\xae\xa6\xe7\xdd\xfe\x38";
        let ciphertext = Ciphertext::ECDH{
            e: MPI::new(&eph_pubkey[..]),
            key: Vec::from(&b"\x45\x8b\xd8\x4d\x88\xb3\xd2\x16\xb6\xc2\x3b\x99\x33\xd1\x23\x4b\x10\x15\x8e\x04\x16\xc5\x7c\x94\x88\xf6\x63\xf2\x68\x37\x08\x66\xfd\x5a\x7b\x40\x58\x21\x6b\x2c\xc0\xf4\xdc\x91\xd3\x48\xed\xc1"[..]).into_boxed_slice()
        };

        // Session key
        let dek = b"\x09\x0D\xDC\x40\xC5\x71\x51\x88\xAC\xBD\x45\x56\xD4\x2A\xDF\x77\xCD\xF4\x82\xA2\x1B\x8F\x2E\x48\x3B\xCA\xBF\xD3\xE8\x6D\x0A\x7C\xDF\x10\xe6";

        let key = key.parts_into_public();
        let got_dek = match key.optional_secret() {
            Some(SecretKeyMaterial::Unencrypted(ref u)) => u.map(|mpis| {
                ecdh::decrypt(&key, mpis, &ciphertext)
                    .unwrap()
            }),
            _ => unreachable!(),
        };

        assert_eq!(&dek[..], &got_dek[..]);
    }

    #[test]
    fn import_rsa() {
        use crate::crypto::SessionKey;
        use self::mpi::{MPI, Ciphertext};

        // RSA key
        let ctime =
            time::UNIX_EPOCH + time::Duration::new(1548950502, 0);
        let d = b"\x14\xC4\x3A\x0C\x3A\x79\xA4\xF7\x63\x0D\x89\x93\x63\x8B\x56\x9C\x29\x2E\xCD\xCF\xBF\xB0\xEC\x66\x52\xC3\x70\x1B\x19\x21\x73\xDE\x8B\xAC\x0E\xF2\xE1\x28\x42\x66\x56\x55\x00\x3B\xFD\x50\xC4\x7C\xBC\x9D\xEB\x7D\xF4\x81\xFC\xC3\xBF\xF7\xFF\xD0\x41\x3E\x50\x3B\x5F\x5D\x5F\x56\x67\x5E\x00\xCE\xA4\x53\xB8\x59\xA0\x40\xC8\x96\x6D\x12\x09\x27\xBE\x1D\xF1\xC2\x68\xFC\xF0\x14\xD6\x52\x77\x07\xC8\x12\x36\x9C\x9A\x5C\xAF\x43\xCC\x95\x20\xBB\x0A\x44\x94\xDD\xB4\x4F\x45\x4E\x3A\x1A\x30\x0D\x66\x40\xAC\x68\xE8\xB0\xFD\xCD\x6C\x6B\x6C\xB5\xF7\xE4\x36\x95\xC2\x96\x98\xFD\xCA\x39\x6C\x1A\x2E\x55\xAD\xB6\xE0\xF8\x2C\xFF\xBC\xD3\x32\x15\x52\x39\xB3\x92\x35\xDB\x8B\x68\xAF\x2D\x4A\x6E\x64\xB8\x28\x63\xC4\x24\x94\x2D\xA9\xDB\x93\x56\xE3\xBC\xD0\xB6\x38\x84\x04\xA4\xC6\x18\x48\xFE\xB2\xF8\xE1\x60\x37\x52\x96\x41\xA5\x79\xF6\x3D\xB7\x2A\x71\x5B\x7A\x75\xBF\x7F\xA2\x5A\xC8\xA1\x38\xF2\x5A\xBD\x14\xFC\xAF\xB4\x54\x83\xA4\xBD\x49\xA2\x8B\x91\xB0\xE0\x4A\x1B\x21\x54\x07\x19\x70\x64\x7C\x3E\x9F\x8D\x8B\xE4\x70\xD1\xE7\xBE\x4E\x5C\xCE\xF1";
        let p = b"\xC8\x32\xD1\x17\x41\x4D\x8F\x37\x09\x18\x32\x4C\x4C\xF4\xA2\x15\x27\x43\x3D\xBB\xB5\xF6\x1F\xCF\xD2\xE4\x43\x61\x07\x0E\x9E\x35\x1F\x0A\x5D\xFB\x3A\x45\x74\x61\x73\x73\x7B\x5F\x1F\x87\xFB\x54\x8D\xA8\x85\x3E\xB0\xB7\xC7\xF5\xC9\x13\x99\x8D\x40\xE6\xA6\xD0\x71\x3A\xE3\x2D\x4A\xC3\xA3\xFF\xF7\x72\x82\x14\x52\xA4\xBA\x63\x0E\x17\xCA\xCA\x18\xC4\x3A\x40\x79\xF1\x86\xB3\x10\x4B\x9F\xB2\xAE\x2E\x13\x38\x8D\x2C\xF9\x88\x4C\x25\x53\xEF\xF9\xD1\x8B\x1A\x7C\xE7\xF6\x4B\x73\x51\x31\xFA\x44\x1D\x36\x65\x71\xDA\xFC\x6F";
        let q = b"\xCC\x30\xE9\xCC\xCB\x31\x28\xB5\x90\xFF\x06\x62\x42\x5B\x24\x0E\x00\xFE\xE2\x37\xC4\xAC\xBB\x3B\x8F\xF2\x0E\x3F\x78\xCF\x6B\x7C\xE8\x75\x57\x7C\x15\x9D\x1A\x66\xF2\x0A\xE5\xD3\x0B\xE7\x40\xF7\xE7\x00\xB6\x86\xB5\xD9\x20\x67\xE0\x4A\xC0\x90\xA4\x13\x4D\xC9\xB0\x12\xC5\xCD\x4C\xEB\xA1\x91\x2D\x43\x58\x6E\xB6\x75\xA0\x93\xF0\x5B\xC5\x31\xCA\xB7\xC6\x22\x0C\xD3\xEC\x84\xC5\x91\xA1\x5F\x2C\x8E\x07\x5D\xA1\x98\x67\xC5\x7A\x58\x16\x71\x3D\xED\x91\x03\x0D\xD4\x25\x07\x89\x9B\x33\x98\xA3\x70\xD9\xE7\xC8\x17\xA3\xD9";
        let key: key::SecretKey
            = Key4::import_secret_rsa(&d[..], &p[..], &q[..], ctime)
            .unwrap().into();

        // PKESK
        let c = b"\x8A\x1A\xD4\x82\x91\x6B\xBF\xA1\x65\xD3\x82\x8C\x97\xAB\xD0\x91\xE4\xB4\xC4\x9D\x08\xD8\x8B\xB7\xE6\x13\x3F\x6F\x52\x14\xED\xC4\x77\xB7\x31\x00\xC1\x43\xF9\x62\x53\xBF\x21\x21\x52\x74\x35\xD8\xC7\xA2\x11\x89\xA5\xD5\x21\x98\x6D\x3C\x9F\xF0\xED\xDB\xD7\x0F\xAC\x3C\x15\x25\x34\x52\xC7\x7C\x82\x07\x5A\x99\xC1\xC6\xF6\xF2\x6D\x46\xC8\x56\x59\xE7\xC6\x34\x0C\xCA\x37\x70\xB4\x97\xDA\x18\x14\xC4\x03\x0A\xCB\xE5\x0C\x41\x43\x61\xBA\x32\xB6\x9A\xF3\xDF\x0C\xB0\xCE\xBD\xFE\x72\x6C\xCC\xC1\xE8\xF0\x05\x97\x61\xEA\x30\x10\xB9\x43\xC4\x9A\x41\xED\x72\x27\xA4\xD5\xE7\x08\x41\x6C\x57\x80\xF3\x64\xF0\x45\x70\x27\x36\xBD\x64\x59\x74\xCF\xCD\x39\xE6\xEB\x7C\x62\xC8\x38\x23\xF8\x4C\xB7\x30\x9F\xF1\x40\x4A\xE9\x72\x66\x99\xF7\x2A\x47\x1C\xE7\x12\x20\x58\xBA\x87\x00\xB8\xFC\x54\xBC\xA5\x1D\x7D\x8B\x50\xA4\x4B\xB3\xD7\x44\xC7\x68\x5E\x2D\xBB\xE9\x6E\xC4\xD0\x31\xB0\xD0\xB6\x02\xD1\x74\x6B\xC9\x3D\x19\x32\x3B\xF1\x0E\x74\xF6\x12\x13\xE6\x40\x8F\xA6\x97\xAD\x83\xB0\x84\xD6\xD9\xE5\x25\x8E\x57\x0B\x7A\x7B\xD0\x5C\x29\x96\xED\x29\xED";
        let ciphertext = Ciphertext::RSA{
            c: MPI::new(&c[..]),
        };
        let pkesk = PKESK3::new(key.keyid(), PublicKeyAlgorithm::RSAEncryptSign,
                                ciphertext).unwrap();

        // Session key
        let dek = b"\xA5\x58\x3A\x04\x35\x8B\xC7\x3F\x4A\xEF\x0C\x5A\xEB\xED\x59\xCA\xFD\x96\xB5\x32\x23\x26\x0C\x91\x78\xD1\x31\x12\xF0\x41\x42\x9D";
        let sk = SessionKey::from(Vec::from(&dek[..]));

        // Expected
        let mut decryptor = key.into_keypair().unwrap();
        let got_sk = pkesk.decrypt(&mut decryptor, None).unwrap();
        assert_eq!(got_sk.1, sk);
    }

    #[test]
    fn import_ed25519() {
        use crate::types::SignatureType;
        use crate::packet::signature::Signature4;
        use crate::packet::signature::subpacket::{
            Subpacket, SubpacketValue, SubpacketArea};

        // Ed25519 key
        let ctime =
            time::UNIX_EPOCH + time::Duration::new(1548249630, 0);
        let q = b"\x57\x15\x45\x1B\x68\xA5\x13\xA2\x20\x0F\x71\x9D\xE3\x05\x3B\xED\xA2\x21\xDE\x61\x5A\xF5\x67\x45\xBB\x97\x99\x43\x53\x59\x7C\x3F";
        let key: key::PublicKey
            = Key4::import_public_ed25519(q, ctime).unwrap().into();

        let mut hashed = SubpacketArea::default();
        let mut unhashed = SubpacketArea::default();
        let fpr = "D81A 5DC0 DEBF EE5F 9AC8  20EB 6769 5DB9 920D 4FAC"
            .parse().unwrap();
        let kid = "6769 5DB9 920D 4FAC".parse().unwrap();
        let ctime = 1549460479.into();
        let r = b"\x5A\xF9\xC7\x42\x70\x24\x73\xFF\x7F\x27\xF9\x20\x9D\x20\x0F\xE3\x8F\x71\x3C\x5F\x97\xFD\x60\x80\x39\x29\xC2\x14\xFD\xC2\x4D\x70";
        let s = b"\x6E\x68\x74\x11\x72\xF4\x9C\xE1\x99\x99\x1F\x67\xFC\x3A\x68\x33\xF9\x3F\x3A\xB9\x1A\xA5\x72\x4E\x78\xD4\x81\xCB\x7B\xA5\xE5\x0A";

        hashed.add(Subpacket::new(SubpacketValue::IssuerFingerprint(fpr), false).unwrap()).unwrap();
        hashed.add(Subpacket::new(SubpacketValue::SignatureCreationTime(ctime), false).unwrap()).unwrap();
        unhashed.add(Subpacket::new(SubpacketValue::Issuer(kid), false).unwrap()).unwrap();

        eprintln!("fpr: {}", key.fingerprint());
        let sig = Signature4::new(SignatureType::Binary, PublicKeyAlgorithm::EdDSA,
                                  HashAlgorithm::SHA256, hashed, unhashed,
                                  [0xa7,0x19],
                                  mpi::Signature::EdDSA{
                                      r: mpi::MPI::new(r), s: mpi::MPI::new(s)
                                  });
        let mut sig: Signature = sig.into();
        sig.verify_message(&key, b"Hello, World\n").unwrap();
    }

    #[test]
    fn fingerprint_test() {
        let pile =
            PacketPile::from_bytes(crate::tests::key("public-key.gpg")).unwrap();

        // The blob contains a public key and a three subkeys.
        let mut pki = 0;
        let mut ski = 0;

        let pks = [ "8F17777118A33DDA9BA48E62AACB3243630052D9" ];
        let sks = [ "C03FA6411B03AE12576461187223B56678E02528",
                    "50E6D924308DBF223CFB510AC2B819056C652598",
                    "2DC50AB55BE2F3B04C2D2CF8A3506AFB820ABD08"];

        for p in pile.descendants() {
            if let &Packet::PublicKey(ref p) = p {
                let fp = p.fingerprint().to_hex();
                // eprintln!("PK: {:?}", fp);

                assert!(pki < pks.len());
                assert_eq!(fp, pks[pki]);
                pki += 1;
            }

            if let &Packet::PublicSubkey(ref p) = p {
                let fp = p.fingerprint().to_hex();
                // eprintln!("SK: {:?}", fp);

                assert!(ski < sks.len());
                assert_eq!(fp, sks[ski]);
                ski += 1;
            }
        }
        assert!(pki == pks.len() && ski == sks.len());
    }

    #[test]
    fn issue_617() -> Result<()> {
        use crate::serialize::MarshalInto;
        let p = Packet::from_bytes(&b"-----BEGIN PGP ARMORED FILE-----

xcClBAAAAMUWBSuBBAAjAPDbS+Z6Ti+PouOV6c5Ypr3jn1w1Ih5GqikN5E29PGz+
CQMIoYc7R4YRiLr/ZJB/MW5M0kuuWyUirUKRkYCotB5omVE8fGtqW5wGCGf79Tzb
rKVmPl25CJdEabIfAOl0WwciipDx1tqNOOYEci/JWSbTEymEyCH9oQPObt2sdDxh
wLcBgsd/CVl3kuqiXFHNYDvWVBmUHeltS/J22Kfy/n1qD3CCBFooHGdc13KwtMLk
UPb5LTTqCk2ihQ7e+5u7EmueLUp1431HJiYa+olaPZ7caRNfQfggtHcfQOJdnWRJ
FN2nTDgLHX0cEOiMboZrS4S9xtjyVRLcRZcCIyeQF0Q889rq0lmxHG38XUeIj/3y
SJJNnZxmJtHNo+SZQ/gXhO9TzeeA6yQm2myQlRkXBtdQEz6mtznphWeWMkWApZpa
FwPoSAbbsLkNS/iNN2MDGAVYvezYn2QZ
=0cxs
-----END PGP ARMORED FILE-----"[..])?;
        let i: usize = 360;
        let mut buf = p.to_vec().unwrap();
        // Avoid first two bytes so that we don't change the
        // type and reduce the chance of changing the length.
        let bit = i.saturating_add(2 * 8) % (buf.len() * 8);
        buf[bit / 8] ^= 1 << (bit % 8);
        match Packet::from_bytes(&buf) {
            Ok(q) => {
                eprintln!("{:?}", p);
                eprintln!("{:?}", q);
                assert!(p != q);
            },
            Err(_) => unreachable!(),
        };
        Ok(())
    }

    #[test]
    fn encrypt_huge_plaintext() -> Result<()> {
        let sk = crate::crypto::SessionKey::new(256);

        if PublicKeyAlgorithm::RSAEncryptSign.is_supported() {
            let rsa2k: Key<SecretParts, UnspecifiedRole> =
                Key4::generate_rsa(2048)?.into();
            assert!(matches!(
                rsa2k.encrypt(&sk).unwrap_err().downcast().unwrap(),
                crate::Error::InvalidArgument(_)
            ));
        }

        if PublicKeyAlgorithm::ECDH.is_supported()
            && Curve::Cv25519.is_supported()
        {
            let cv25519: Key<SecretParts, UnspecifiedRole> =
                Key4::generate_ecc(false, Curve::Cv25519)?.into();
            assert!(matches!(
                cv25519.encrypt(&sk).unwrap_err().downcast().unwrap(),
                crate::Error::InvalidArgument(_)
            ));
        }

        Ok(())
    }

    fn mutate_eq_discriminates_key<P, R>(key: Key<P, R>, i: usize) -> bool
        where P: KeyParts,
              R: KeyRole,
              Key<P, R>: Into<Packet>,
    {
        use crate::serialize::MarshalInto;
        let p: Packet = key.into();
        let mut buf = p.to_vec().unwrap();
        // Avoid first two bytes so that we don't change the
        // type and reduce the chance of changing the length.
        let bit = i.saturating_add(2 * 8) % (buf.len() * 8);
        buf[bit / 8] ^= 1 << (bit % 8);
        let ok = match Packet::from_bytes(&buf) {
            Ok(q) => p != q,
            Err(_) => true, // Packet failed to parse.
        };
        if ! ok {
            eprintln!("mutate_eq_discriminates_key for ({:?}, {})", p, i);
        }
        ok
    }

    // Given a packet and a position, induces a bit flip in the
    // serialized form, then checks that PartialEq detects that.
    // Recall that for packets, PartialEq is defined using the
    // serialized form.
    quickcheck! {
        fn mutate_eq_discriminates_pp(key: Key<PublicParts, PrimaryRole>,
                                      i: usize) -> bool {
            mutate_eq_discriminates_key(key, i)
        }
    }
    quickcheck! {
        fn mutate_eq_discriminates_ps(key: Key<PublicParts, SubordinateRole>,
                                      i: usize) -> bool {
            mutate_eq_discriminates_key(key, i)
        }
    }
    quickcheck! {
        fn mutate_eq_discriminates_sp(key: Key<SecretParts, PrimaryRole>,
                                      i: usize) -> bool {
            mutate_eq_discriminates_key(key, i)
        }
    }
    quickcheck! {
        fn mutate_eq_discriminates_ss(key: Key<SecretParts, SubordinateRole>,
                                      i: usize) -> bool {
            mutate_eq_discriminates_key(key, i)
        }
    }
}
