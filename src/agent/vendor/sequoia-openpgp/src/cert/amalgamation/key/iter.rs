use std::fmt;
use std::convert::TryInto;
use std::time::SystemTime;
use std::borrow::Borrow;
use std::slice;

use crate::{
    KeyHandle,
    types::RevocationStatus,
    packet::key,
    packet::key::SecretKeyMaterial,
    types::KeyFlags,
    cert::prelude::*,
    policy::Policy,
};

/// An iterator over `Key`s.
///
/// An iterator over [`KeyAmalgamation`]s.
///
/// A `KeyAmalgamationIter` is like a [`ComponentAmalgamationIter`],
/// but specialized for keys.  Refer to the [module documentation] for
/// an explanation of why a different type is necessary.
///
/// Using the [`KeyAmalgamationIter::with_policy`], it is possible to
/// change the iterator to only return [`KeyAmalgamation`]s for valid
/// `Key`s.  In this case, `KeyAmalgamationIter::with_policy`
/// transforms the `KeyAmalgamationIter` into a
/// [`ValidKeyAmalgamationIter`], which returns
/// [`ValidKeyAmalgamation`]s.  `ValidKeyAmalgamation` offers
/// additional filters.
///
/// `KeyAmalgamationIter` supports other filters.  For instance
/// [`KeyAmalgamationIter::secret`] filters on whether secret key
/// material is present, and
/// [`KeyAmalgamationIter::unencrypted_secret`] filters on whether
/// secret key material is present and unencrypted.  Of course, since
/// `KeyAmalgamationIter` implements `Iterator`, it is possible to use
/// [`Iterator::filter`] to implement custom filters.
///
/// `KeyAmalgamationIter` follows the builder pattern.  There is no
/// need to explicitly finalize it: it already implements the
/// `Iterator` trait.
///
/// A `KeyAmalgamationIter` is returned by [`Cert::keys`].
///
/// [`ComponentAmalgamationIter`]: super::super::ComponentAmalgamationIter
/// [module documentation]: super
/// [`KeyAmalgamationIter::with_policy`]: super::ValidateAmalgamation
/// [`KeyAmalgamationIter::secret`]: KeyAmalgamationIter::secret()
/// [`KeyAmalgamationIter::unencrypted_secret`]: KeyAmalgamationIter::unencrypted_secret()
/// [`Iterator::filter`]: std::iter::Iterator::filter()
/// [`Cert::keys`]: super::super::Cert::keys()
pub struct KeyAmalgamationIter<'a, P, R>
    where P: key::KeyParts,
          R: key::KeyRole,
{
    // This is an option to make it easier to create an empty KeyAmalgamationIter.
    cert: Option<&'a Cert>,
    primary: bool,
    subkey_iter: slice::Iter<'a, KeyBundle<key::PublicParts,
                                           key::SubordinateRole>>,

    // If not None, filters by whether a key has a secret.
    secret: Option<bool>,

    // If not None, filters by whether a key has an unencrypted
    // secret.
    unencrypted_secret: Option<bool>,

    // Only return keys in this set.
    key_handles: Option<Vec<KeyHandle>>,

    // If not None, filters by whether we support the key's asymmetric
    // algorithm.
    supported: Option<bool>,

    _p: std::marker::PhantomData<P>,
    _r: std::marker::PhantomData<R>,
}
assert_send_and_sync!(KeyAmalgamationIter<'_, P, R>
     where P: key::KeyParts,
           R: key::KeyRole,
);

impl<'a, P, R> fmt::Debug for KeyAmalgamationIter<'a, P, R>
    where P: key::KeyParts,
          R: key::KeyRole,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("KeyAmalgamationIter")
            .field("secret", &self.secret)
            .field("unencrypted_secret", &self.unencrypted_secret)
            .field("key_handles", &self.key_handles)
            .field("supported", &self.supported)
            .finish()
    }
}

macro_rules! impl_iterator {
    ($parts:path, $role:path, $item:ty) => {
        impl<'a> Iterator for KeyAmalgamationIter<'a, $parts, $role>
        {
            type Item = $item;

            fn next(&mut self) -> Option<Self::Item> {
                // We unwrap the result of the conversion.  But, this
                // is safe by construction: next_common only returns
                // keys that can be correctly converted.
                self.next_common().map(|k| k.try_into().expect("filtered"))
            }
        }
    }
}

impl_iterator!(key::PublicParts, key::PrimaryRole,
               PrimaryKeyAmalgamation<'a, key::PublicParts>);
impl_iterator!(key::SecretParts, key::PrimaryRole,
               PrimaryKeyAmalgamation<'a, key::SecretParts>);
impl_iterator!(key::UnspecifiedParts, key::PrimaryRole,
               PrimaryKeyAmalgamation<'a, key::UnspecifiedParts>);

impl_iterator!(key::PublicParts, key::SubordinateRole,
               SubordinateKeyAmalgamation<'a, key::PublicParts>);
impl_iterator!(key::SecretParts, key::SubordinateRole,
               SubordinateKeyAmalgamation<'a, key::SecretParts>);
impl_iterator!(key::UnspecifiedParts, key::SubordinateRole,
               SubordinateKeyAmalgamation<'a, key::UnspecifiedParts>);

impl_iterator!(key::PublicParts, key::UnspecifiedRole,
               ErasedKeyAmalgamation<'a, key::PublicParts>);
impl_iterator!(key::SecretParts, key::UnspecifiedRole,
               ErasedKeyAmalgamation<'a, key::SecretParts>);
impl_iterator!(key::UnspecifiedParts, key::UnspecifiedRole,
               ErasedKeyAmalgamation<'a, key::UnspecifiedParts>);

impl<'a, P, R> KeyAmalgamationIter<'a, P, R>
    where P: key::KeyParts,
          R: key::KeyRole,
{
    fn next_common(&mut self) -> Option<ErasedKeyAmalgamation<'a, key::PublicParts>>
    {
        tracer!(false, "KeyAmalgamationIter::next", 0);
        t!("KeyAmalgamationIter: {:?}", self);

        let cert = self.cert?;

        loop {
            let ka : ErasedKeyAmalgamation<key::PublicParts>
                = if ! self.primary {
                    self.primary = true;
                    PrimaryKeyAmalgamation::new(cert).into()
                } else {
                    SubordinateKeyAmalgamation::new(
                        cert, self.subkey_iter.next()?).into()
                };

            t!("Considering key: {:?}", ka.key());

            if let Some(key_handles) = self.key_handles.as_ref() {
                if !key_handles
                    .iter()
                    .any(|h| h.aliases(ka.key().key_handle()))
                {
                    t!("{} is not one of the keys that we are looking for ({:?})",
                       ka.key().fingerprint(), self.key_handles);
                    continue;
                }
            }

            if let Some(want_supported) = self.supported {
                if ka.key().pk_algo().is_supported() {
                    // It is supported.
                    if ! want_supported {
                        t!("PK algo is supported... skipping.");
                        continue;
                    }
                } else if want_supported {
                    t!("PK algo is not supported... skipping.");
                    continue;
                }
            }

            if let Some(want_secret) = self.secret {
                if ka.key().has_secret() {
                    // We have a secret.
                    if ! want_secret {
                        t!("Have a secret... skipping.");
                        continue;
                    }
                } else if want_secret {
                    t!("No secret... skipping.");
                    continue;
                }
            }

            if let Some(want_unencrypted_secret) = self.unencrypted_secret {
                if let Some(secret) = ka.key().optional_secret() {
                    if let SecretKeyMaterial::Unencrypted { .. } = secret {
                        if ! want_unencrypted_secret {
                            t!("Unencrypted secret... skipping.");
                            continue;
                        }
                    } else if want_unencrypted_secret {
                        t!("Encrypted secret... skipping.");
                        continue;
                    }
                } else {
                    // No secret.
                    t!("No secret... skipping.");
                    continue;
                }
            }

            return Some(ka);
        }
    }
}

impl<'a, P, R> KeyAmalgamationIter<'a, P, R>
    where P: key::KeyParts,
          R: key::KeyRole,
{
    /// Returns a new `KeyAmalgamationIter` instance.
    pub(crate) fn new(cert: &'a Cert) -> Self where Self: 'a {
        KeyAmalgamationIter {
            cert: Some(cert),
            primary: false,
            subkey_iter: cert.subkeys.iter(),

            // The filters.
            secret: None,
            unencrypted_secret: None,
            key_handles: None,
            supported: None,

            _p: std::marker::PhantomData,
            _r: std::marker::PhantomData,
        }
    }

    /// Changes the iterator to only return keys with secret key
    /// material.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use sequoia_openpgp as openpgp;
    /// # use openpgp::Result;
    /// # use openpgp::cert::prelude::*;
    /// # fn main() -> Result<()> {
    /// #     let (cert, _) =
    /// #         CertBuilder::general_purpose(None, Some("alice@example.org"))
    /// #         .generate()?;
    /// for ka in cert.keys().secret() {
    ///     // Use it.
    /// }
    /// #     Ok(())
    /// # }
    /// ```
    pub fn secret(self) -> KeyAmalgamationIter<'a, key::SecretParts, R> {
        KeyAmalgamationIter {
            cert: self.cert,
            primary: self.primary,
            subkey_iter: self.subkey_iter,

            // The filters.
            secret: Some(true),
            unencrypted_secret: self.unencrypted_secret,
            key_handles: self.key_handles,
            supported: self.supported,

            _p: std::marker::PhantomData,
            _r: std::marker::PhantomData,
        }
    }

    /// Changes the iterator to only return keys with unencrypted
    /// secret key material.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use sequoia_openpgp as openpgp;
    /// # use openpgp::Result;
    /// # use openpgp::cert::prelude::*;
    /// # fn main() -> Result<()> {
    /// #     let (cert, _) =
    /// #         CertBuilder::general_purpose(None, Some("alice@example.org"))
    /// #         .generate()?;
    /// for ka in cert.keys().unencrypted_secret() {
    ///     // Use it.
    /// }
    /// #     Ok(())
    /// # }
    /// ```
    pub fn unencrypted_secret(self) -> KeyAmalgamationIter<'a, key::SecretParts, R> {
        KeyAmalgamationIter {
            cert: self.cert,
            primary: self.primary,
            subkey_iter: self.subkey_iter,

            // The filters.
            secret: self.secret,
            unencrypted_secret: Some(true),
            key_handles: self.key_handles,
            supported: self.supported,

            _p: std::marker::PhantomData,
            _r: std::marker::PhantomData,
        }
    }

    /// Changes the iterator to only return a key if it matches one of
    /// the specified `KeyHandle`s.
    ///
    /// This function is cumulative.  If you call this function (or
    /// [`key_handles`]) multiple times, then the iterator returns a key
    /// if it matches *any* of the specified [`KeyHandle`s].
    ///
    /// This function uses [`KeyHandle::aliases`] to compare key
    /// handles.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use sequoia_openpgp as openpgp;
    /// # use openpgp::Result;
    /// # use openpgp::cert::prelude::*;
    /// # fn main() -> Result<()> {
    /// #     let (cert, _) =
    /// #         CertBuilder::general_purpose(None, Some("alice@example.org"))
    /// #         .generate()?;
    /// # let key_handle = cert.primary_key().key_handle();
    /// # let mut i = 0;
    /// for ka in cert.keys().key_handle(key_handle) {
    ///     // Use it.
    /// #   i += 1;
    /// }
    /// # assert_eq!(i, 1);
    /// #     Ok(())
    /// # }
    /// ```
    ///
    /// [`KeyHandle`s]: super::super::super::KeyHandle
    /// [`key_handles`]: KeyAmalgamationIter::key_handles()
    /// [`KeyHandle::aliases`]: super::super::super::KeyHandle::aliases()
    pub fn key_handle<H>(mut self, h: H) -> Self
        where H: Into<KeyHandle>
    {
        if self.key_handles.is_none() {
            self.key_handles = Some(Vec::new());
        }
        self.key_handles.as_mut().unwrap().push(h.into());
        self
    }

    /// Changes the iterator to only return a key if it matches one of
    /// the specified `KeyHandle`s.
    ///
    /// This function is cumulative.  If you call this function (or
    /// [`key_handle`]) multiple times, then the iterator returns a key
    /// if it matches *any* of the specified [`KeyHandle`s].
    ///
    /// This function uses [`KeyHandle::aliases`] to compare key
    /// handles.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use sequoia_openpgp as openpgp;
    /// # use openpgp::Result;
    /// # use openpgp::cert::prelude::*;
    /// # fn main() -> Result<()> {
    /// #     let (cert, _) =
    /// #         CertBuilder::general_purpose(None, Some("alice@example.org"))
    /// #         .generate()?;
    /// # let key_handles = &[cert.primary_key().key_handle()][..];
    /// # let mut i = 0;
    /// for ka in cert.keys().key_handles(key_handles.iter()) {
    ///     // Use it.
    /// #   i += 1;
    /// }
    /// # assert_eq!(i, 1);
    /// #     Ok(())
    /// # }
    /// ```
    ///
    /// [`KeyHandle`s]: super::super::super::KeyHandle
    /// [`key_handle`]: KeyAmalgamationIter::key_handle()
    /// [`KeyHandle::aliases`]: super::super::super::KeyHandle::aliases()
    pub fn key_handles<'b>(mut self, h: impl Iterator<Item=&'b KeyHandle>)
        -> Self
        where 'a: 'b
    {
        if self.key_handles.is_none() {
            self.key_handles = Some(Vec::new());
        }
        self.key_handles.as_mut().unwrap().extend(h.cloned());
        self
    }

    /// Changes the iterator to only return a key if it is supported
    /// by Sequoia's cryptographic backend.
    ///
    /// Which public key encryption algorithms Sequoia supports
    /// depends on the cryptographic backend selected at compile time.
    /// This filter makes sure that only supported keys are returned.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # fn main() -> sequoia_openpgp::Result<()> {
    /// # use sequoia_openpgp as openpgp;
    /// # use openpgp::cert::prelude::*;
    /// #     let (cert, _) =
    /// #         CertBuilder::general_purpose(None, Some("alice@example.org"))
    /// #         .generate()?;
    /// # let mut i = 0;
    /// for ka in cert.keys().supported() {
    ///     // Use it.
    /// #   i += 1;
    /// }
    /// # assert_eq!(i, 3);
    /// # Ok(()) }
    /// ```
    pub fn supported(mut self) -> Self {
        self.supported = Some(true);
        self
    }

    /// Changes the iterator to only return subkeys.
    ///
    /// This function also changes the return type.  Instead of the
    /// iterator returning a [`ErasedKeyAmalgamation`], it returns a
    /// [`SubordinateKeyAmalgamation`].
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use sequoia_openpgp as openpgp;
    /// # use openpgp::Result;
    /// # use openpgp::cert::prelude::*;
    /// #
    /// # fn main() -> Result<()> {
    /// #      let (cert, _) = CertBuilder::new()
    /// #          .add_signing_subkey()
    /// #          .add_certification_subkey()
    /// #          .add_transport_encryption_subkey()
    /// #          .add_storage_encryption_subkey()
    /// #          .add_authentication_subkey()
    /// #          .generate()?;
    /// # let mut i = 0;
    /// for ka in cert.keys().subkeys() {
    ///     // Use it.
    ///     assert!(! ka.primary());
    /// #   i += 1;
    /// }
    /// # assert_eq!(i, 5);
    /// #     Ok(())
    /// # }
    /// ```
    ///
    pub fn subkeys(self) -> KeyAmalgamationIter<'a, P, key::SubordinateRole> {
        KeyAmalgamationIter {
            cert: self.cert,
            primary: true,
            subkey_iter: self.subkey_iter,

            // The filters.
            secret: self.secret,
            unencrypted_secret: self.unencrypted_secret,
            key_handles: self.key_handles,
            supported: self.supported,

            _p: std::marker::PhantomData,
            _r: std::marker::PhantomData,
        }
    }

    /// Changes the iterator to only return valid `Key`s.
    ///
    /// If `time` is None, then the current time is used.
    ///
    /// This also makes a number of additional filters like [`alive`]
    /// and [`revoked`] available.
    ///
    /// Refer to the [`ValidateAmalgamation`] trait for a definition
    /// of a valid component.
    ///
    /// # Examples
    ///
    /// ```
    /// # use sequoia_openpgp as openpgp;
    /// # use openpgp::cert::prelude::*;
    /// use openpgp::policy::StandardPolicy;
    /// #
    /// # fn main() -> openpgp::Result<()> {
    /// let p = &StandardPolicy::new();
    ///
    /// #     let (cert, _) =
    /// #         CertBuilder::general_purpose(None, Some("alice@example.org"))
    /// #         .generate()?;
    /// #     let fpr = cert.fingerprint();
    /// // Iterate over all valid User Attributes.
    /// for ka in cert.keys().with_policy(p, None) {
    ///     // ka is a `ValidKeyAmalgamation`, specifically, an
    ///     // `ValidErasedKeyAmalgamation`.
    /// }
    /// #     Ok(())
    /// # }
    /// ```
    ///
    /// [`ValidateAmalgamation`]: super::ValidateAmalgamation
    /// [`alive`]: ValidKeyAmalgamationIter::alive()
    /// [`revoked`]: ValidKeyAmalgamationIter::revoked()
    pub fn with_policy<T>(self, policy: &'a dyn Policy, time: T)
        -> ValidKeyAmalgamationIter<'a, P, R>
        where T: Into<Option<SystemTime>>
    {
        ValidKeyAmalgamationIter {
            cert: self.cert,
            primary: self.primary,
            subkey_iter: self.subkey_iter,

            policy,
            time: time.into().unwrap_or_else(crate::now),

            // The filters.
            secret: self.secret,
            unencrypted_secret: self.unencrypted_secret,
            key_handles: self.key_handles,
            supported: self.supported,
            flags: None,
            alive: None,
            revoked: None,

            _p: self._p,
            _r: self._r,
        }
    }
}

/// An iterator over valid `Key`s.
///
/// An iterator over [`ValidKeyAmalgamation`]s.
///
/// A `ValidKeyAmalgamationIter` is a [`KeyAmalgamationIter`]
/// that includes a [`Policy`] and a reference time, which it firstly
/// uses to only return valid `Key`s.  (For a definition of valid
/// keys, see the documentation for [`ValidateAmalgamation`].)
///
/// A `ValidKeyAmalgamationIter` also provides additional
/// filters based on information available in the `Key`s' binding
/// signatures.  For instance, [`ValidKeyAmalgamationIter::revoked`]
/// filters the returned `Key`s by whether or not they are revoked.
/// And, [`ValidKeyAmalgamationIter::alive`] changes the iterator to
/// only return `Key`s that are live.
///
/// `ValidKeyAmalgamationIter` follows the builder pattern.  But,
/// there is no need to explicitly finalize it: it already implements
/// the `Iterator` trait.
///
/// A `ValidKeyAmalgamationIter` is returned by
/// [`KeyAmalgamationIter::with_policy`] and [`ValidCert::keys`].
///
/// # Examples
///
/// Find a key that we can use to sign a document:
///
/// ```
/// # use sequoia_openpgp as openpgp;
/// # use openpgp::cert::prelude::*;
/// use openpgp::policy::StandardPolicy;
/// use openpgp::types::RevocationStatus;
///
/// # fn main() -> openpgp::Result<()> {
/// let p = &StandardPolicy::new();
///
/// #     let (cert, _) =
/// #         CertBuilder::general_purpose(None, Some("alice@example.org"))
/// #         .generate()?;
/// # let mut i = 0;
/// // The certificate *and* keys need to be valid.
/// let cert = cert.with_policy(p, None)?;
///
/// if let RevocationStatus::Revoked(_) = cert.revocation_status() {
///     // Certificate is revoked.
/// } else if let Err(_err) = cert.alive() {
///     // The certificate is not alive.
/// } else {
///     // Iterate over all valid keys.
///     //
///     // Note: using the combinator interface (instead of checking
///     // the individual keys) makes it harder to report exactly why no
///     // key was usable.
///     for ka in cert.keys()
///         // Not revoked.
///         .revoked(false)
///         // Alive.
///         .alive()
///         // Be signing capable.
///         .for_signing()
///         // And have unencrypted secret material.
///         .unencrypted_secret()
///     {
///         // We can use it.
/// #       i += 1;
///     }
/// }
/// # assert_eq!(i, 1);
/// #     Ok(())
/// # }
/// ```
///
/// [`Policy`]: crate::policy::Policy
/// [`ValidateAmalgamation`]: super::ValidateAmalgamation
/// [`ValidKeyAmalgamationIter::revoked`]: ValidKeyAmalgamationIter::revoked()
/// [`ValidKeyAmalgamationIter::alive`]: ValidKeyAmalgamationIter::alive()
/// [`KeyAmalgamationIter::with_policy`]: KeyAmalgamationIter::with_policy()
/// [`ValidCert::keys`]: super::super::ValidCert::keys()
pub struct ValidKeyAmalgamationIter<'a, P, R>
    where P: key::KeyParts,
          R: key::KeyRole,
{
    // This is an option to make it easier to create an empty ValidKeyAmalgamationIter.
    cert: Option<&'a Cert>,
    primary: bool,
    subkey_iter: slice::Iter<'a, KeyBundle<key::PublicParts,
                                           key::SubordinateRole>>,

    // The policy.
    policy: &'a dyn Policy,

    // The time.
    time: SystemTime,

    // If not None, filters by whether a key has a secret.
    secret: Option<bool>,

    // If not None, filters by whether a key has an unencrypted
    // secret.
    unencrypted_secret: Option<bool>,

    // Only return keys in this set.
    key_handles: Option<Vec<KeyHandle>>,

    // If not None, filters by whether we support the key's asymmetric
    // algorithm.
    supported: Option<bool>,

    // If not None, only returns keys with the specified flags.
    flags: Option<KeyFlags>,

    // If not None, filters by whether a key is alive at time `t`.
    alive: Option<()>,

    // If not None, filters by whether the key is revoked or not at
    // time `t`.
    revoked: Option<bool>,

    _p: std::marker::PhantomData<P>,
    _r: std::marker::PhantomData<R>,
}
assert_send_and_sync!(ValidKeyAmalgamationIter<'_, P, R>
     where P: key::KeyParts,
           R: key::KeyRole,
);

impl<'a, P, R> fmt::Debug for ValidKeyAmalgamationIter<'a, P, R>
    where P: key::KeyParts,
          R: key::KeyRole,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("ValidKeyAmalgamationIter")
            .field("policy", &self.policy)
            .field("time", &self.time)
            .field("secret", &self.secret)
            .field("unencrypted_secret", &self.unencrypted_secret)
            .field("key_handles", &self.key_handles)
            .field("supported", &self.supported)
            .field("flags", &self.flags)
            .field("alive", &self.alive)
            .field("revoked", &self.revoked)
            .finish()
    }
}

macro_rules! impl_iterator {
    ($parts:path, $role:path, $item:ty) => {
        impl<'a> Iterator for ValidKeyAmalgamationIter<'a, $parts, $role>
        {
            type Item = $item;

            fn next(&mut self) -> Option<Self::Item> {
                // We unwrap the result of the conversion.  But, this
                // is safe by construction: next_common only returns
                // keys that can be correctly converted.
                self.next_common().map(|k| k.try_into().expect("filtered"))
            }
        }
    }
}

impl_iterator!(key::PublicParts, key::PrimaryRole,
               ValidPrimaryKeyAmalgamation<'a, key::PublicParts>);
impl_iterator!(key::SecretParts, key::PrimaryRole,
               ValidPrimaryKeyAmalgamation<'a, key::SecretParts>);
impl_iterator!(key::UnspecifiedParts, key::PrimaryRole,
               ValidPrimaryKeyAmalgamation<'a, key::UnspecifiedParts>);

impl_iterator!(key::PublicParts, key::SubordinateRole,
               ValidSubordinateKeyAmalgamation<'a, key::PublicParts>);
impl_iterator!(key::SecretParts, key::SubordinateRole,
               ValidSubordinateKeyAmalgamation<'a, key::SecretParts>);
impl_iterator!(key::UnspecifiedParts, key::SubordinateRole,
               ValidSubordinateKeyAmalgamation<'a, key::UnspecifiedParts>);

impl_iterator!(key::PublicParts, key::UnspecifiedRole,
               ValidErasedKeyAmalgamation<'a, key::PublicParts>);
impl_iterator!(key::SecretParts, key::UnspecifiedRole,
               ValidErasedKeyAmalgamation<'a, key::SecretParts>);
impl_iterator!(key::UnspecifiedParts, key::UnspecifiedRole,
               ValidErasedKeyAmalgamation<'a, key::UnspecifiedParts>);

impl<'a, P, R> ValidKeyAmalgamationIter<'a, P, R>
    where P: key::KeyParts,
          R: key::KeyRole,
{
    fn next_common(&mut self)
        -> Option<ValidErasedKeyAmalgamation<'a, key::PublicParts>>
    {
        tracer!(false, "ValidKeyAmalgamationIter::next", 0);
        t!("ValidKeyAmalgamationIter: {:?}", self);

        let cert = self.cert?;

        if let Some(flags) = self.flags.as_ref() {
            if flags.is_empty() {
                // Nothing to do.
                t!("short circuiting: flags is empty");
                return None;
            }
        }

        loop {
            let ka = if ! self.primary {
                self.primary = true;
                let ka : ErasedKeyAmalgamation<'a, key::PublicParts>
                    = PrimaryKeyAmalgamation::new(cert).into();
                match ka.with_policy(self.policy, self.time) {
                    Ok(ka) => ka,
                    Err(err) => {
                        // The primary key is bad.  Abort.
                        t!("Getting primary key: {:?}", err);
                        return None;
                    }
                }
            } else {
                let ka : ErasedKeyAmalgamation<'a, key::PublicParts>
                    = SubordinateKeyAmalgamation::new(
                        cert, self.subkey_iter.next()?).into();
                match ka.with_policy(self.policy, self.time) {
                    Ok(ka) => ka,
                    Err(err) => {
                        // The subkey is bad, abort.
                        t!("Getting subkey: {:?}", err);
                        continue;
                    }
                }
            };

            let key = ka.key();
            t!("Considering key: {:?}", key);

            if let Some(key_handles) = self.key_handles.as_ref() {
                if !key_handles
                    .iter()
                    .any(|h| h.aliases(key.key_handle()))
                {
                    t!("{} is not one of the keys that we are looking for ({:?})",
                       key.key_handle(), self.key_handles);
                    continue;
                }
            }

            if let Some(want_supported) = self.supported {
                if ka.key().pk_algo().is_supported() {
                    // It is supported.
                    if ! want_supported {
                        t!("PK algo is supported... skipping.");
                        continue;
                    }
                } else if want_supported {
                    t!("PK algo is not supported... skipping.");
                    continue;
                }
            }

            if let Some(flags) = self.flags.as_ref() {
                if !ka.has_any_key_flag(flags) {
                    t!("Have flags: {:?}, want flags: {:?}... skipping.",
                      flags, flags);
                    continue;
                }
            }

            if let Some(()) = self.alive {
                if let Err(err) = ka.alive() {
                    t!("Key not alive: {:?}", err);
                    continue;
                }
            }

            if let Some(want_revoked) = self.revoked {
                if let RevocationStatus::Revoked(_) = ka.revocation_status() {
                    // The key is definitely revoked.
                    if ! want_revoked {
                        t!("Key revoked... skipping.");
                        continue;
                    }
                } else {
                    // The key is probably not revoked.
                    if want_revoked {
                        t!("Key not revoked... skipping.");
                        continue;
                    }
                }
            }

            if let Some(want_secret) = self.secret {
                if key.has_secret() {
                    // We have a secret.
                    if ! want_secret {
                        t!("Have a secret... skipping.");
                        continue;
                    }
                } else if want_secret {
                    t!("No secret... skipping.");
                    continue;
                }
            }

            if let Some(want_unencrypted_secret) = self.unencrypted_secret {
                if let Some(secret) = key.optional_secret() {
                    if let SecretKeyMaterial::Unencrypted { .. } = secret {
                        if ! want_unencrypted_secret {
                            t!("Unencrypted secret... skipping.");
                            continue;
                        }
                    } else if want_unencrypted_secret {
                        t!("Encrypted secret... skipping.");
                        continue;
                    }
                } else {
                    // No secret.
                    t!("No secret... skipping.");
                    continue;
                }
            }

            return Some(ka);
        }
    }
}

impl<'a, P, R> ValidKeyAmalgamationIter<'a, P, R>
    where P: key::KeyParts,
          R: key::KeyRole,
{
    /// Returns keys that have the at least one of the flags specified
    /// in `flags`.
    ///
    /// If you call this function (or one of `for_certification`,
    /// `for_signing`, etc.) multiple times, the *union* of
    /// the values is used.
    ///
    /// Note: [Section 12.1 of RFC 4880] says that the primary key is
    /// certification capable independent of the `Key Flags`
    /// subpacket:
    ///
    /// > In a V4 key, the primary key MUST be a key capable of
    /// > certification.
    ///
    /// This function only reflects what is stored in the `Key Flags`
    /// packet; it does not implicitly set this flag.  In practice,
    /// there are keys whose primary key's `Key Flags` do not have the
    /// certification capable flag set.  Some versions of netpgp, for
    /// instance, create keys like this.  Sequoia's higher-level
    /// functionality correctly handles these keys by always
    /// considering the primary key to be certification capable.
    /// Users of this interface should too.
    ///
    /// The key flags are looked up as described in
    /// [`ValidKeyAmalgamation::key_flags`].
    ///
    /// # Examples
    ///
    /// ```
    /// # use sequoia_openpgp as openpgp;
    /// # use openpgp::cert::prelude::*;
    /// use openpgp::policy::StandardPolicy;
    /// use openpgp::types::KeyFlags;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// let p = &StandardPolicy::new();
    ///
    /// #   let (cert, _) = CertBuilder::new()
    /// #       .add_signing_subkey()
    /// #       .add_certification_subkey()
    /// #       .add_transport_encryption_subkey()
    /// #       .add_storage_encryption_subkey()
    /// #       .add_authentication_subkey()
    /// #       .generate()?;
    /// #   let mut i = 0;
    /// for ka in cert.keys()
    ///     .with_policy(p, None)
    ///     .key_flags(KeyFlags::empty()
    ///         .set_transport_encryption()
    ///         .set_storage_encryption())
    /// {
    ///     // Valid encryption-capable keys.
    /// #   i += 1;
    /// }
    /// # assert_eq!(i, 2);
    /// # Ok(()) }
    /// ```
    ///
    ///   [Section 12.1 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.2.3.21
    ///   [`ValidKeyAmalgamation::key_flags`]: ValidKeyAmalgamation::key_flags()
    pub fn key_flags<F>(mut self, flags: F) -> Self
        where F: Borrow<KeyFlags>
    {
        let flags = flags.borrow();
        if let Some(flags_old) = self.flags {
            self.flags = Some(flags | &flags_old);
        } else {
            self.flags = Some(flags.clone());
        }
        self
    }

    /// Returns certification-capable keys.
    ///
    /// If you call this function (or one of `key_flags`,
    /// `for_signing`, etc.) multiple times, the *union* of
    /// the values is used.
    ///
    /// Note: [Section 12.1 of RFC 4880] says that the primary key is
    /// certification capable independent of the `Key Flags`
    /// subpacket:
    ///
    /// > In a V4 key, the primary key MUST be a key capable of
    /// > certification.
    ///
    /// This function only reflects what is stored in the `Key Flags`
    /// packet; it does not implicitly set this flag.  In practice,
    /// there are keys whose primary key's `Key Flags` do not have the
    /// certification capable flag set.  Some versions of netpgp, for
    /// instance, create keys like this.  Sequoia's higher-level
    /// functionality correctly handles these keys by always
    /// considering the primary key to be certification capable.
    /// Users of this interface should too.
    ///
    /// The key flags are looked up as described in
    /// [`ValidKeyAmalgamation::key_flags`].
    ///
    /// # Examples
    ///
    /// ```
    /// # use sequoia_openpgp as openpgp;
    /// # use openpgp::cert::prelude::*;
    /// use openpgp::policy::StandardPolicy;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// let p = &StandardPolicy::new();
    ///
    /// #   let (cert, _) = CertBuilder::new()
    /// #       .add_signing_subkey()
    /// #       .add_certification_subkey()
    /// #       .add_transport_encryption_subkey()
    /// #       .add_storage_encryption_subkey()
    /// #       .add_authentication_subkey()
    /// #       .generate()?;
    /// #   let mut i = 0;
    /// for ka in cert.keys()
    ///     .with_policy(p, None)
    ///     .for_certification()
    /// {
    ///     // Valid certification-capable keys.
    /// #   i += 1;
    /// }
    /// # assert_eq!(i, 2);
    /// # Ok(()) }
    /// ```
    ///
    ///   [`ValidKeyAmalgamation::for_certification`]: ValidKeyAmalgamation::for_certification()
    ///   [Section 12.1 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.2.3.21
    ///   [`ValidKeyAmalgamation::key_flags`]: ValidKeyAmalgamation::key_flags()
    pub fn for_certification(self) -> Self {
        self.key_flags(KeyFlags::empty().set_certification())
    }

    /// Returns signing-capable keys.
    ///
    /// If you call this function (or one of `key_flags`,
    /// `for_certification`, etc.) multiple times, the *union* of
    /// the values is used.
    ///
    /// Refer to [`ValidKeyAmalgamation::for_signing`] for additional
    /// details and caveats.
    ///
    /// # Examples
    ///
    /// ```
    /// # use sequoia_openpgp as openpgp;
    /// # use openpgp::cert::prelude::*;
    /// use openpgp::policy::StandardPolicy;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// let p = &StandardPolicy::new();
    ///
    /// #   let (cert, _) = CertBuilder::new()
    /// #       .add_signing_subkey()
    /// #       .add_certification_subkey()
    /// #       .add_transport_encryption_subkey()
    /// #       .add_storage_encryption_subkey()
    /// #       .add_authentication_subkey()
    /// #       .generate()?;
    /// #   let mut i = 0;
    /// for ka in cert.keys()
    ///     .with_policy(p, None)
    ///     .for_signing()
    /// {
    ///     // Valid signing-capable keys.
    /// #   i += 1;
    /// }
    /// # assert_eq!(i, 1);
    /// # Ok(()) }
    /// ```
    ///
    ///   [`ValidKeyAmalgamation::for_signing`]: ValidKeyAmalgamation::for_signing()
    pub fn for_signing(self) -> Self {
        self.key_flags(KeyFlags::empty().set_signing())
    }

    /// Returns authentication-capable keys.
    ///
    /// If you call this function (or one of `key_flags`,
    /// `for_certification`, etc.) multiple times, the
    /// *union* of the values is used.
    ///
    /// Refer to [`ValidKeyAmalgamation::for_authentication`] for
    /// additional details and caveats.
    ///
    /// # Examples
    ///
    /// ```
    /// # use sequoia_openpgp as openpgp;
    /// # use openpgp::cert::prelude::*;
    /// use openpgp::policy::StandardPolicy;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// let p = &StandardPolicy::new();
    ///
    /// #   let (cert, _) = CertBuilder::new()
    /// #       .add_authentication_subkey()
    /// #       .add_certification_subkey()
    /// #       .add_transport_encryption_subkey()
    /// #       .add_storage_encryption_subkey()
    /// #       .add_authentication_subkey()
    /// #       .generate()?;
    /// #   let mut i = 0;
    /// for ka in cert.keys()
    ///     .with_policy(p, None)
    ///     .for_authentication()
    /// {
    ///     // Valid authentication-capable keys.
    /// #   i += 1;
    /// }
    /// # assert_eq!(i, 2);
    /// # Ok(()) }
    /// ```
    ///
    ///   [`ValidKeyAmalgamation::for_authentication`]: ValidKeyAmalgamation::for_authentication()
    pub fn for_authentication(self) -> Self {
        self.key_flags(KeyFlags::empty().set_authentication())
    }

    /// Returns encryption-capable keys for data at rest.
    ///
    /// If you call this function (or one of `key_flags`,
    /// `for_certification`, etc.) multiple times, the
    /// *union* of the values is used.
    ///
    /// Refer to [`ValidKeyAmalgamation::for_storage_encryption`] for
    /// additional details and caveats.
    ///
    /// # Examples
    ///
    /// ```
    /// # use sequoia_openpgp as openpgp;
    /// # use openpgp::cert::prelude::*;
    /// use openpgp::policy::StandardPolicy;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// let p = &StandardPolicy::new();
    ///
    /// #   let (cert, _) = CertBuilder::new()
    /// #       .add_authentication_subkey()
    /// #       .add_certification_subkey()
    /// #       .add_transport_encryption_subkey()
    /// #       .add_storage_encryption_subkey()
    /// #       .add_authentication_subkey()
    /// #       .generate()?;
    /// #   let mut i = 0;
    /// for ka in cert.keys()
    ///     .with_policy(p, None)
    ///     .for_storage_encryption()
    /// {
    ///     // Valid encryption-capable keys for data at rest.
    /// #   i += 1;
    /// }
    /// # assert_eq!(i, 1);
    /// # Ok(()) }
    /// ```
    ///
    ///   [`ValidKeyAmalgamation::for_storage_encryption`]: ValidKeyAmalgamation::for_storage_encryption()
    pub fn for_storage_encryption(self) -> Self {
        self.key_flags(KeyFlags::empty().set_storage_encryption())
    }

    /// Returns encryption-capable keys for data in transit.
    ///
    /// If you call this function (or one of `key_flags`,
    /// `for_certification`, etc.) multiple times, the
    /// *union* of the values is used.
    ///
    /// Refer to [`ValidKeyAmalgamation::for_transport_encryption`] for
    /// additional details and caveats.
    ///
    /// # Examples
    ///
    /// ```
    /// # use sequoia_openpgp as openpgp;
    /// # use openpgp::cert::prelude::*;
    /// use openpgp::policy::StandardPolicy;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// let p = &StandardPolicy::new();
    ///
    /// #   let (cert, _) = CertBuilder::new()
    /// #       .add_authentication_subkey()
    /// #       .add_certification_subkey()
    /// #       .add_transport_encryption_subkey()
    /// #       .add_transport_encryption_subkey()
    /// #       .add_authentication_subkey()
    /// #       .generate()?;
    /// #   let mut i = 0;
    /// for ka in cert.keys()
    ///     .with_policy(p, None)
    ///     .for_transport_encryption()
    /// {
    ///     // Valid encryption-capable keys for data in transit.
    /// #   i += 1;
    /// }
    /// # assert_eq!(i, 2);
    /// # Ok(()) }
    /// ```
    ///
    ///   [`ValidKeyAmalgamation::for_transport_encryption`]: ValidKeyAmalgamation::for_transport_encryption()
    pub fn for_transport_encryption(self) -> Self {
        self.key_flags(KeyFlags::empty().set_transport_encryption())
    }

    /// Returns keys that are alive.
    ///
    /// A `ValidKeyAmalgamation` is guaranteed to have a live *binding
    /// signature*.  This is independent of whether the *key* is live,
    /// or the *certificate* is live, i.e., if you care about those
    /// things, you need to check them too.
    ///
    /// For a definition of liveness, see the [`key_alive`] method.
    ///
    /// # Examples
    ///
    /// ```
    /// # use sequoia_openpgp as openpgp;
    /// # use openpgp::cert::prelude::*;
    /// use openpgp::policy::StandardPolicy;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// let p = &StandardPolicy::new();
    ///
    /// #   let (cert, _) = CertBuilder::new()
    /// #       .add_authentication_subkey()
    /// #       .add_certification_subkey()
    /// #       .add_transport_encryption_subkey()
    /// #       .add_transport_encryption_subkey()
    /// #       .add_authentication_subkey()
    /// #       .generate()?;
    /// for ka in cert.keys()
    ///     .with_policy(p, None)
    ///     .alive()
    /// {
    ///     // ka is alive.
    /// }
    /// # Ok(()) }
    /// ```
    ///
    /// [`key_alive`]: crate::packet::signature::subpacket::SubpacketAreas::key_alive()
    pub fn alive(mut self) -> Self
    {
        self.alive = Some(());
        self
    }

    /// Returns keys based on their revocation status.
    ///
    /// A value of `None` disables this filter.
    ///
    /// If you call this function multiple times on the same
    /// `ValidKeyAmalgamationIter`, only the last value is used.
    ///
    /// This filter checks the key's revocation status; it does
    /// not check the certificate's revocation status.
    ///
    /// This filter only checks whether the key has no valid-self
    /// revocations at the specified time.  It does not check
    /// third-party revocations.
    ///
    /// # Examples
    ///
    /// ```
    /// # use sequoia_openpgp as openpgp;
    /// # use openpgp::cert::prelude::*;
    /// use openpgp::policy::StandardPolicy;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// let p = &StandardPolicy::new();
    ///
    /// #   let (cert, _) = CertBuilder::new()
    /// #       .add_authentication_subkey()
    /// #       .add_certification_subkey()
    /// #       .add_transport_encryption_subkey()
    /// #       .add_transport_encryption_subkey()
    /// #       .add_authentication_subkey()
    /// #       .generate()?;
    /// for ka in cert.keys()
    ///     .with_policy(p, None)
    ///     .revoked(false)
    /// {
    ///     // ka has no self-revocations; recall: this filter doesn't check
    ///     // third-party revocations.
    /// }
    /// # Ok(()) }
    /// ```
    ///
    /// This filter checks whether a key's revocation status is
    /// `RevocationStatus::Revoked` or not.
    /// `ValidKeyAmalgamationIter::revoked(false)` is equivalent to:
    ///
    /// ```rust
    /// # use sequoia_openpgp as openpgp;
    /// # use openpgp::Result;
    /// use openpgp::types::RevocationStatus;
    /// # use openpgp::cert::prelude::*;
    /// use openpgp::policy::StandardPolicy;
    ///
    /// # fn main() -> Result<()> {
    /// #     let (cert, _) =
    /// #         CertBuilder::general_purpose(None, Some("alice@example.org"))
    /// #         .generate()?;
    /// let p = &StandardPolicy::new();
    ///
    /// # let timestamp = None;
    /// let non_revoked_keys = cert
    ///     .keys()
    ///     .with_policy(p, timestamp)
    ///     .filter(|ka| {
    ///         match ka.revocation_status() {
    ///             RevocationStatus::Revoked(_) =>
    ///                 // It's definitely revoked, skip it.
    ///                 false,
    ///             RevocationStatus::CouldBe(_) =>
    ///                 // There is a designated revoker that we
    ///                 // could check, but don't (or can't).  To
    ///                 // avoid a denial of service attack arising from
    ///                 // fake revocations, we assume that the key has
    ///                 // not been revoked and return it.
    ///                 true,
    ///             RevocationStatus::NotAsFarAsWeKnow =>
    ///                 // We have no evidence to suggest that the key
    ///                 // is revoked.
    ///                 true,
    ///         }
    ///     })
    ///     .map(|ka| ka.key())
    ///     .collect::<Vec<_>>();
    /// #     Ok(())
    /// # }
    /// ```
    ///
    /// As the example shows, this filter is significantly less
    /// flexible than using `KeyAmalgamation::revocation_status`.
    /// However, this filter implements a typical policy, and does not
    /// preclude using something like `Iter::filter` to implement
    /// alternative policies.
    pub fn revoked<T>(mut self, revoked: T) -> Self
        where T: Into<Option<bool>>
    {
        self.revoked = revoked.into();
        self
    }

    /// Changes the iterator to only return keys with secret key
    /// material.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use sequoia_openpgp as openpgp;
    /// # use openpgp::Result;
    /// # use openpgp::cert::prelude::*;
    /// use openpgp::policy::StandardPolicy;
    ///
    /// # fn main() -> Result<()> {
    /// let p = &StandardPolicy::new();
    ///
    /// #     let (cert, _) =
    /// #         CertBuilder::general_purpose(None, Some("alice@example.org"))
    /// #         .generate()?;
    /// for ka in cert.keys().with_policy(p, None).secret() {
    ///     // Use it.
    /// }
    /// #     Ok(())
    /// # }
    /// ```
    pub fn secret(self) -> ValidKeyAmalgamationIter<'a, key::SecretParts, R> {
        ValidKeyAmalgamationIter {
            cert: self.cert,
            primary: self.primary,
            subkey_iter: self.subkey_iter,

            time: self.time,
            policy: self.policy,

            // The filters.
            secret: Some(true),
            unencrypted_secret: self.unencrypted_secret,
            key_handles: self.key_handles,
            supported: self.supported,
            flags: self.flags,
            alive: self.alive,
            revoked: self.revoked,

            _p: std::marker::PhantomData,
            _r: std::marker::PhantomData,
        }
    }

    /// Changes the iterator to only return keys with unencrypted
    /// secret key material.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use sequoia_openpgp as openpgp;
    /// # use openpgp::Result;
    /// # use openpgp::cert::prelude::*;
    /// use openpgp::policy::StandardPolicy;
    ///
    /// # fn main() -> Result<()> {
    /// let p = &StandardPolicy::new();
    ///
    /// #     let (cert, _) =
    /// #         CertBuilder::general_purpose(None, Some("alice@example.org"))
    /// #         .generate()?;
    /// for ka in cert.keys().with_policy(p, None).unencrypted_secret() {
    ///     // Use it.
    /// }
    /// #     Ok(())
    /// # }
    /// ```
    pub fn unencrypted_secret(self) -> ValidKeyAmalgamationIter<'a, key::SecretParts, R> {
        ValidKeyAmalgamationIter {
            cert: self.cert,
            primary: self.primary,
            subkey_iter: self.subkey_iter,

            time: self.time,
            policy: self.policy,

            // The filters.
            secret: self.secret,
            unencrypted_secret: Some(true),
            key_handles: self.key_handles,
            supported: self.supported,
            flags: self.flags,
            alive: self.alive,
            revoked: self.revoked,

            _p: std::marker::PhantomData,
            _r: std::marker::PhantomData,
        }
    }

    /// Changes the iterator to only return a key if it matches one of
    /// the specified `KeyHandle`s.
    ///
    /// This function is cumulative.  If you call this function (or
    /// [`key_handles`]) multiple times, then the iterator returns a
    /// key if it matches *any* of the specified [`KeyHandle`s].
    ///
    /// This function uses [`KeyHandle::aliases`] to compare key
    /// handles.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use sequoia_openpgp as openpgp;
    /// # use openpgp::Result;
    /// # use openpgp::cert::prelude::*;
    /// use openpgp::policy::StandardPolicy;
    ///
    /// # fn main() -> Result<()> {
    /// let p = &StandardPolicy::new();
    ///
    /// #     let (cert, _) =
    /// #         CertBuilder::general_purpose(None, Some("alice@example.org"))
    /// #         .generate()?;
    /// # let key_handle = cert.primary_key().key_handle();
    /// # let mut i = 0;
    /// for ka in cert.keys().with_policy(p, None).key_handle(key_handle) {
    ///     // Use it.
    /// #   i += 1;
    /// }
    /// # assert_eq!(i, 1);
    /// #     Ok(())
    /// # }
    /// ```
    ///
    /// [`KeyHandle`s]: super::super::super::KeyHandle
    /// [`key_handles`]: ValidKeyAmalgamationIter::key_handles()
    /// [`KeyHandle::aliases`]: super::super::super::KeyHandle::aliases()
    pub fn key_handle<H>(mut self, h: H) -> Self
        where H: Into<KeyHandle>
    {
        if self.key_handles.is_none() {
            self.key_handles = Some(Vec::new());
        }
        self.key_handles.as_mut().unwrap().push(h.into());
        self
    }

    /// Changes the iterator to only return a key if it matches one of
    /// the specified `KeyHandle`s.
    ///
    /// This function is cumulative.  If you call this function (or
    /// [`key_handle`]) multiple times, then the iterator returns a key
    /// if it matches *any* of the specified [`KeyHandle`s].
    ///
    /// This function uses [`KeyHandle::aliases`] to compare key
    /// handles.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use sequoia_openpgp as openpgp;
    /// # use openpgp::Result;
    /// # use openpgp::cert::prelude::*;
    /// use openpgp::policy::StandardPolicy;
    ///
    /// # fn main() -> Result<()> {
    /// let p = &StandardPolicy::new();
    ///
    /// #     let (cert, _) =
    /// #         CertBuilder::general_purpose(None, Some("alice@example.org"))
    /// #         .generate()?;
    /// # let key_handles = &[cert.primary_key().key_handle()][..];
    /// # let mut i = 0;
    /// for ka in cert.keys().with_policy(p, None).key_handles(key_handles.iter()) {
    ///     // Use it.
    /// #   i += 1;
    /// }
    /// # assert_eq!(i, 1);
    /// #     Ok(())
    /// # }
    /// ```
    ///
    /// [`KeyHandle`s]: super::super::super::KeyHandle
    /// [`key_handle`]: ValidKeyAmalgamationIter::key_handle()
    /// [`KeyHandle::aliases`]: super::super::super::KeyHandle::aliases()
    pub fn key_handles<'b>(mut self, h: impl Iterator<Item=&'b KeyHandle>)
        -> Self
        where 'a: 'b
    {
        if self.key_handles.is_none() {
            self.key_handles = Some(Vec::new());
        }
        self.key_handles.as_mut().unwrap().extend(h.cloned());
        self
    }

    /// Changes the iterator to only return a key if it is supported
    /// by Sequoia's cryptographic backend.
    ///
    /// Which public key encryption algorithms Sequoia supports
    /// depends on the cryptographic backend selected at compile time.
    /// This filter makes sure that only supported keys are returned.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # fn main() -> sequoia_openpgp::Result<()> {
    /// # use sequoia_openpgp as openpgp;
    /// # use openpgp::cert::prelude::*;
    /// #     let (cert, _) =
    /// #         CertBuilder::general_purpose(None, Some("alice@example.org"))
    /// #         .generate()?;
    /// # let mut i = 0;
    /// use openpgp::policy::StandardPolicy;
    ///
    /// let p = &StandardPolicy::new();
    ///
    /// for ka in cert.keys().with_policy(p, None).supported() {
    ///     // Use it.
    /// #   i += 1;
    /// }
    /// # assert_eq!(i, 3);
    /// # Ok(()) }
    /// ```
    pub fn supported(mut self) -> Self {
        self.supported = Some(true);
        self
    }

    /// Changes the iterator to skip the primary key.
    ///
    /// This also changes the iterator's return type.  Instead of
    /// returning a [`ValidErasedKeyAmalgamation`], it returns a
    /// [`ValidSubordinateKeyAmalgamation`].
    ///
    /// # Examples
    ///
    /// ```
    /// # use sequoia_openpgp as openpgp;
    /// # use openpgp::cert::prelude::*;
    /// use openpgp::policy::StandardPolicy;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// let p = &StandardPolicy::new();
    ///
    /// #   let (cert, _) = CertBuilder::new()
    /// #       .add_signing_subkey()
    /// #       .add_certification_subkey()
    /// #       .add_transport_encryption_subkey()
    /// #       .add_storage_encryption_subkey()
    /// #       .add_authentication_subkey()
    /// #       .generate()?;
    /// #   let mut i = 0;
    /// for ka in cert.keys().with_policy(p, None).subkeys() {
    ///     assert!(! ka.primary());
    /// #   i += 1;
    /// }
    /// # assert_eq!(cert.keys().count(), 6);
    /// # assert_eq!(i, 5);
    /// # Ok(()) }
    /// ```
    ///
    pub fn subkeys(self) -> ValidKeyAmalgamationIter<'a, P, key::SubordinateRole> {
        ValidKeyAmalgamationIter {
            cert: self.cert,
            primary: true,
            subkey_iter: self.subkey_iter,

            time: self.time,
            policy: self.policy,

            // The filters.
            secret: self.secret,
            unencrypted_secret: self.unencrypted_secret,
            key_handles: self.key_handles,
            supported: self.supported,
            flags: self.flags,
            alive: self.alive,
            revoked: self.revoked,

            _p: std::marker::PhantomData,
            _r: std::marker::PhantomData,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        parse::Parse,
        cert::builder::CertBuilder,
    };
    use crate::policy::StandardPolicy as P;

    #[test]
    fn key_iter_test() {
        let key = Cert::from_bytes(crate::tests::key("neal.pgp")).unwrap();
        assert_eq!(1 + key.subkeys().count(),
                   key.keys().count());
    }

    #[test]
    fn select_no_keys() {
        let p = &P::new();
        let (cert, _) = CertBuilder::new()
            .generate().unwrap();
        let flags = KeyFlags::empty().set_transport_encryption();

        assert_eq!(cert.keys().with_policy(p, None).key_flags(flags).count(), 0);
    }

    #[test]
    fn select_valid_and_right_flags() {
        let p = &P::new();
        let (cert, _) = CertBuilder::new()
            .add_transport_encryption_subkey()
            .generate().unwrap();
        let flags = KeyFlags::empty().set_transport_encryption();

        assert_eq!(cert.keys().with_policy(p, None).key_flags(flags).count(), 1);
    }

    #[test]
    fn select_valid_and_wrong_flags() {
        let p = &P::new();
        let (cert, _) = CertBuilder::new()
            .add_transport_encryption_subkey()
            .add_signing_subkey()
            .generate().unwrap();
        let flags = KeyFlags::empty().set_transport_encryption();

        assert_eq!(cert.keys().with_policy(p, None).key_flags(flags).count(), 1);
    }

    #[test]
    fn select_invalid_and_right_flags() {
        let p = &P::new();
        let (cert, _) = CertBuilder::new()
            .add_transport_encryption_subkey()
            .generate().unwrap();
        let flags = KeyFlags::empty().set_transport_encryption();

        let now = crate::now()
            - std::time::Duration::new(52 * 7 * 24 * 60 * 60, 0);
        assert_eq!(cert.keys().with_policy(p, now).key_flags(flags).alive().count(),
                   0);
    }

    #[test]
    fn select_primary() {
        let p = &P::new();
        let (cert, _) = CertBuilder::new()
            .add_certification_subkey()
            .generate().unwrap();
        let flags = KeyFlags::empty().set_certification();

        assert_eq!(cert.keys().with_policy(p, None).key_flags(flags).count(),
                   2);
    }

    #[test]
    fn selectors() {
        let p = &P::new();
        let (cert, _) = CertBuilder::new()
            .add_signing_subkey()
            .add_certification_subkey()
            .add_transport_encryption_subkey()
            .add_storage_encryption_subkey()
            .add_authentication_subkey()
            .generate().unwrap();
        assert_eq!(cert.keys().with_policy(p, None).alive().revoked(false)
                       .for_certification().count(),
                   2);
        assert_eq!(cert.keys().with_policy(p, None).alive().revoked(false)
                       .for_transport_encryption().count(),
                   1);
        assert_eq!(cert.keys().with_policy(p, None).alive().revoked(false)
                       .for_storage_encryption().count(),
                   1);

        assert_eq!(cert.keys().with_policy(p, None).alive().revoked(false)
                       .for_signing().count(),
                   1);
        assert_eq!(cert.keys().with_policy(p, None).alive().revoked(false)
                       .key_flags(KeyFlags::empty().set_authentication())
                       .count(),
                   1);
    }

    #[test]
    fn select_key_handle() {
        let p = &P::new();

        let (cert, _) = CertBuilder::new()
            .add_signing_subkey()
            .add_certification_subkey()
            .add_transport_encryption_subkey()
            .add_storage_encryption_subkey()
            .add_authentication_subkey()
            .generate().unwrap();

        let keys = cert.keys().count();
        assert_eq!(keys, 6);

        let keyids = cert.keys().map(|ka| ka.key().keyid()).collect::<Vec<_>>();

        fn check(got: &[KeyHandle], expected: &[KeyHandle]) {
            if expected.len() != got.len() {
                panic!("Got {}, expected {} handles",
                       got.len(), expected.len());
            }

            for (g, e) in got.iter().zip(expected.iter()) {
                if !e.aliases(g) {
                    panic!("     Got: {:?}\nExpected: {:?}",
                           got, expected);
                }
            }
        }

        for i in 1..keys {
            for keyids in keyids[..].windows(i) {
                let keyids : Vec<KeyHandle>
                    = keyids.iter().map(Into::into).collect();
                assert_eq!(keyids.len(), i);

                check(
                    &cert.keys().key_handles(keyids.iter())
                        .map(|ka| ka.key().key_handle())
                        .collect::<Vec<KeyHandle>>(),
                    &keyids);
                check(
                    &cert.keys().with_policy(p, None).key_handles(keyids.iter())
                        .map(|ka| ka.key().key_handle())
                        .collect::<Vec<KeyHandle>>(),
                    &keyids);
                check(
                    &cert.keys().key_handles(keyids.iter()).with_policy(p, None)
                        .map(|ka| ka.key().key_handle())
                        .collect::<Vec<KeyHandle>>(),
                    &keyids);
            }
        }
    }

    #[test]
    fn select_supported() -> crate::Result<()> {
        use crate::types::PublicKeyAlgorithm;
        if ! PublicKeyAlgorithm::DSA.is_supported()
            || PublicKeyAlgorithm::ElGamalEncrypt.is_supported()
        {
            return Ok(()); // Skip on this backend.
        }

        let cert =
            Cert::from_bytes(crate::tests::key("dsa2048-elgamal3072.pgp"))?;
        assert_eq!(cert.keys().count(), 2);
        assert_eq!(cert.keys().supported().count(), 1);
        let p = &crate::policy::NullPolicy::new();
        assert_eq!(cert.keys().with_policy(p, None).count(), 2);
        assert_eq!(cert.keys().with_policy(p, None).supported().count(), 1);
        Ok(())
    }
}
