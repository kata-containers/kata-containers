use std::convert::TryFrom;
use std::ops::Deref;
use std::time;

use crate::{
    HashAlgorithm,
    Result,
    SignatureType,
};
use crate::types::{
    ReasonForRevocation,
};
use crate::crypto::Signer;
use crate::packet::{
    Key,
    key,
    signature,
    Signature,
    UserAttribute,
    UserID,
};
use crate::packet::signature::subpacket::NotationDataFlags;
use crate::cert::prelude::*;

/// A builder for revocation certificates for OpenPGP certificates.
///
/// A revocation certificate for an OpenPGP certificate (as opposed
/// to, say, a subkey) has two degrees of freedom: the certificate,
/// and the key used to sign the revocation certificate.
///
/// Normally, the key used to sign the revocation certificate is the
/// certificate's primary key.  However, this is not required.  For
/// instance, if Alice has marked Robert's certificate (`R`) as a
/// [designated revoker] for her certificate (`A`), then `R` can
/// revoke `A` or parts of `A`.  In this case, the certificate is `A`,
/// and the key used to sign the revocation certificate comes from
/// `R`.
///
/// [designated revoker]: https://tools.ietf.org/html/rfc4880#section-5.2.3.15
///
/// # Examples
///
/// Revoke `cert`, which was compromised yesterday:
///
/// ```rust
/// use sequoia_openpgp as openpgp;
/// # use openpgp::Result;
/// use openpgp::cert::prelude::*;
/// use openpgp::policy::StandardPolicy;
/// use openpgp::types::ReasonForRevocation;
/// use openpgp::types::RevocationStatus;
/// use openpgp::types::SignatureType;
///
/// # fn main() -> Result<()> {
/// let p = &StandardPolicy::new();
///
/// # let (cert, _) = CertBuilder::new()
/// #     .generate()?;
/// # assert_eq!(RevocationStatus::NotAsFarAsWeKnow,
/// #            cert.revocation_status(p, None));
/// #
/// // Create and sign a revocation certificate.
/// let mut signer = cert.primary_key().key().clone()
///     .parts_into_secret()?.into_keypair()?;
/// # let yesterday = std::time::SystemTime::now();
/// let sig = CertRevocationBuilder::new()
///     // Don't use the current time, since the certificate was
///     // actually compromised yesterday.
///     .set_signature_creation_time(yesterday)?
///     .set_reason_for_revocation(ReasonForRevocation::KeyCompromised,
///                                b"It was the maid :/")?
///     .build(&mut signer, &cert, None)?;
///
/// // Merge it into the certificate.
/// let cert = cert.insert_packets(sig.clone())?;
///
/// // Now it's revoked.
/// assert_eq!(RevocationStatus::Revoked(vec![&sig]),
///            cert.revocation_status(p, None));
/// # Ok(())
/// # }
pub struct CertRevocationBuilder {
    builder: signature::SignatureBuilder,
}
assert_send_and_sync!(CertRevocationBuilder);

#[allow(clippy::new_without_default)]
impl CertRevocationBuilder {
    /// Returns a new `CertRevocationBuilder`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use sequoia_openpgp as openpgp;
    /// # use openpgp::Result;
    /// use openpgp::cert::prelude::*;
    ///
    /// # fn main() -> Result<()> {
    /// let builder = CertRevocationBuilder::new();
    /// # Ok(())
    /// # }
    pub fn new() -> Self {
        Self {
            builder:
                signature::SignatureBuilder::new(SignatureType::KeyRevocation)
        }
    }

    /// Sets the reason for revocation.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use sequoia_openpgp as openpgp;
    /// # use openpgp::Result;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::types::ReasonForRevocation;
    ///
    /// # fn main() -> Result<()> {
    /// let builder = CertRevocationBuilder::new()
    ///     .set_reason_for_revocation(ReasonForRevocation::KeyRetired,
    ///                                b"I'm retiring this key.  \
    ///                                  Please use my new OpenPGP certificate (FPR)");
    /// # Ok(())
    /// # }
    pub fn set_reason_for_revocation(self, code: ReasonForRevocation,
                                     reason: &[u8])
        -> Result<Self>
    {
        Ok(Self {
            builder: self.builder.set_reason_for_revocation(code, reason)?
        })
    }

    /// Sets the revocation certificate's creation time.
    ///
    /// The creation time is interpreted as the time at which the
    /// certificate should be considered revoked.  For a soft
    /// revocation, artifacts created prior to the revocation are
    /// still considered valid.
    ///
    /// You'll usually want to set this explicitly and not use the
    /// current time.
    ///
    /// First, the creation time should reflect the time of the event
    /// that triggered the revocation.  As such, if it is discovered
    /// that a certificate was compromised a week ago, then the
    /// revocation certificate should be backdated appropriately.
    ///
    /// Second, because access to secret key material can be lost, it
    /// can be useful to create a revocation certificate in advance.
    /// Of course, such a revocation certificate will inevitably be
    /// outdated.  To mitigate this problem, a number of revocation
    /// certificates can be created with different creation times.
    /// Then should a revocation certificate be needed, the most
    /// appropriate one can be used.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use std::time::{SystemTime, Duration};
    /// use sequoia_openpgp as openpgp;
    /// # use openpgp::Result;
    /// use openpgp::cert::prelude::*;
    ///
    /// # fn main() -> Result<()> {
    /// let now = SystemTime::now();
    /// let month = Duration::from_secs(((365.24 / 12.) * 24. * 60. * 60.) as u64);
    ///
    /// // Pre-generate revocation certificates, one for each month
    /// // for the next 48 months.
    /// for i in 0..48 {
    ///     let builder = CertRevocationBuilder::new()
    ///         .set_signature_creation_time(now + i * month);
    ///     // ...
    /// }
    /// # Ok(())
    /// # }
    pub fn set_signature_creation_time(self, creation_time: time::SystemTime)
        -> Result<Self>
    {
        Ok(Self {
            builder: self.builder.set_signature_creation_time(creation_time)?
        })
    }

    /// Adds a notation to the revocation certificate.
    ///
    /// Unlike the [`CertRevocationBuilder::set_notation`] method, this function
    /// does not first remove any existing notation with the specified name.
    ///
    /// See [`SignatureBuilder::add_notation`] for further documentation.
    ///
    /// [`SignatureBuilder::add_notation`]: crate::packet::signature::SignatureBuilder::add_notation()
    ///
    /// # Examples
    ///
    /// ```rust
    /// use sequoia_openpgp as openpgp;
    /// # use openpgp::Result;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::packet::signature::subpacket::NotationDataFlags;
    ///
    /// # fn main() -> Result<()> {
    /// let builder = CertRevocationBuilder::new().add_notation(
    ///     "revocation-policy@example.org",
    ///     "https://policy.example.org/cert-revocation-policy",
    ///     NotationDataFlags::empty().set_human_readable(),
    ///     false,
    /// );
    /// # Ok(())
    /// # }
    pub fn add_notation<N, V, F>(self, name: N, value: V, flags: F,
                                 critical: bool)
        -> Result<Self>
    where
        N: AsRef<str>,
        V: AsRef<[u8]>,
        F: Into<Option<NotationDataFlags>>,
    {
        Ok(Self {
            builder: self.builder.add_notation(name, value, flags, critical)?
        })
    }

    /// Sets a notation to the revocation certificate.
    ///
    /// Unlike the [`CertRevocationBuilder::add_notation`] method, this function
    /// first removes any existing notation with the specified name.
    ///
    /// See [`SignatureBuilder::set_notation`] for further documentation.
    ///
    /// [`SignatureBuilder::set_notation`]: crate::packet::signature::SignatureBuilder::set_notation()
    ///
    /// # Examples
    ///
    /// ```rust
    /// use sequoia_openpgp as openpgp;
    /// # use openpgp::Result;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::packet::signature::subpacket::NotationDataFlags;
    ///
    /// # fn main() -> Result<()> {
    /// let builder = CertRevocationBuilder::new().set_notation(
    ///     "revocation-policy@example.org",
    ///     "https://policy.example.org/cert-revocation-policy",
    ///     NotationDataFlags::empty().set_human_readable(),
    ///     false,
    /// );
    /// # Ok(())
    /// # }
    pub fn set_notation<N, V, F>(self, name: N, value: V, flags: F,
                                 critical: bool)
        -> Result<Self>
    where
        N: AsRef<str>,
        V: AsRef<[u8]>,
        F: Into<Option<NotationDataFlags>>,
    {
        Ok(Self {
            builder: self.builder.set_notation(name, value, flags, critical)?
        })
    }

    /// Returns a signed revocation certificate.
    ///
    /// A revocation certificate is generated for `cert` and signed
    /// using `signer` with the specified hash algorithm.  Normally,
    /// you should pass `None` to select the default hash algorithm.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use sequoia_openpgp as openpgp;
    /// # use openpgp::Result;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::policy::StandardPolicy;
    /// use openpgp::types::ReasonForRevocation;
    /// # use openpgp::types::RevocationStatus;
    /// # use openpgp::types::SignatureType;
    ///
    /// # fn main() -> Result<()> {
    /// let p = &StandardPolicy::new();
    ///
    /// # let (cert, _) = CertBuilder::new()
    /// #     .generate()?;
    /// #
    /// // Create and sign a revocation certificate.
    /// let mut signer = cert.primary_key().key().clone()
    ///     .parts_into_secret()?.into_keypair()?;
    /// let sig = CertRevocationBuilder::new()
    ///     .set_reason_for_revocation(ReasonForRevocation::KeyRetired,
    ///                                b"Left Foo Corp.")?
    ///     .build(&mut signer, &cert, None)?;
    ///
    /// # assert_eq!(sig.typ(), SignatureType::KeyRevocation);
    /// #
    /// # // Merge it into the certificate.
    /// # let cert = cert.insert_packets(sig.clone())?;
    /// #
    /// # // Now it's revoked.
    /// # assert_eq!(RevocationStatus::Revoked(vec![&sig]),
    /// #            cert.revocation_status(p, None));
    /// # Ok(())
    /// # }
    pub fn build<H>(self, signer: &mut dyn Signer, cert: &Cert, hash_algo: H)
        -> Result<Signature>
        where H: Into<Option<HashAlgorithm>>
    {
        self.builder
            .set_hash_algo(hash_algo.into().unwrap_or(HashAlgorithm::SHA512))
            .sign_direct_key(signer, cert.primary_key().key())
    }
}

impl Deref for CertRevocationBuilder {
    type Target = signature::SignatureBuilder;

    fn deref(&self) -> &Self::Target {
        &self.builder
    }
}

impl TryFrom<signature::SignatureBuilder> for CertRevocationBuilder {
    type Error = anyhow::Error;

    fn try_from(builder: signature::SignatureBuilder) -> Result<Self> {
        if builder.typ() != SignatureType::KeyRevocation {
            return Err(
                crate::Error::InvalidArgument(
                    format!("Expected signature type to be KeyRevocation but got {}",
                            builder.typ())).into());
        }
        Ok(Self {
            builder
        })
    }
}

/// A builder for revocation certificates for subkeys.
///
/// A revocation certificate for a subkey has three degrees of
/// freedom: the certificate, the key used to generate the revocation
/// certificate, and the subkey being revoked.
///
/// Normally, the key used to sign the revocation certificate is the
/// certificate's primary key, and the subkey is a subkey that is
/// bound to the certificate.  However, this is not required.  For
/// instance, if Alice has marked Robert's certificate (`R`) as a
/// [designated revoker] for her certificate (`A`), then `R` can
/// revoke `A` or parts of `A`.  In such a case, the certificate is
/// `A`, the key used to sign the revocation certificate comes from
/// `R`, and the subkey being revoked is bound to `A`.
///
/// But, the subkey doesn't technically need to be bound to the
/// certificate either.  For instance, it is technically possible for
/// `R` to create a revocation certificate for a subkey in the context
/// of `A`, even if that subkey is not bound to `A`.  Semantically,
/// such a revocation certificate is currently meaningless.
///
/// [designated revoker]: https://tools.ietf.org/html/rfc4880#section-5.2.3.15
///
/// # Examples
///
/// Revoke a subkey, which is now considered to be too weak:
///
/// ```rust
/// use sequoia_openpgp as openpgp;
/// # use openpgp::Result;
/// use openpgp::cert::prelude::*;
/// use openpgp::policy::StandardPolicy;
/// use openpgp::types::ReasonForRevocation;
/// use openpgp::types::RevocationStatus;
/// use openpgp::types::SignatureType;
///
/// # fn main() -> Result<()> {
/// let p = &StandardPolicy::new();
///
/// # let (cert, _) = CertBuilder::new()
/// #     .add_transport_encryption_subkey()
/// #     .generate()?;
/// # assert_eq!(RevocationStatus::NotAsFarAsWeKnow,
/// #            cert.revocation_status(p, None));
/// #
/// // Create and sign a revocation certificate.
/// let mut signer = cert.primary_key().key().clone()
///     .parts_into_secret()?.into_keypair()?;
/// let subkey = cert.keys().subkeys().nth(0).unwrap();
/// let sig = SubkeyRevocationBuilder::new()
///     .set_reason_for_revocation(ReasonForRevocation::KeyRetired,
///                                b"Revoking due to the recent crypto vulnerabilities.")?
///     .build(&mut signer, &cert, subkey.key(), None)?;
///
/// // Merge it into the certificate.
/// let cert = cert.insert_packets(sig.clone())?;
///
/// // Now it's revoked.
/// let subkey = cert.keys().subkeys().nth(0).unwrap();
/// if let RevocationStatus::Revoked(revocations) = subkey.revocation_status(p, None) {
///     assert_eq!(revocations.len(), 1);
///     assert_eq!(*revocations[0], sig);
/// } else {
///     panic!("Subkey is not revoked.");
/// }
///
/// // But the certificate isn't.
/// assert_eq!(RevocationStatus::NotAsFarAsWeKnow,
///            cert.revocation_status(p, None));
/// # Ok(()) }
/// ```
pub struct SubkeyRevocationBuilder {
    builder: signature::SignatureBuilder,
}
assert_send_and_sync!(SubkeyRevocationBuilder);

#[allow(clippy::new_without_default)]
impl SubkeyRevocationBuilder {
    /// Returns a new `SubkeyRevocationBuilder`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use sequoia_openpgp as openpgp;
    /// # use openpgp::Result;
    /// use openpgp::cert::prelude::*;
    ///
    /// # fn main() -> Result<()> {
    /// let builder = SubkeyRevocationBuilder::new();
    /// # Ok(())
    /// # }
    pub fn new() -> Self {
        Self {
            builder:
                signature::SignatureBuilder::new(SignatureType::SubkeyRevocation)
        }
    }

    /// Sets the reason for revocation.
    ///
    /// # Examples
    ///
    /// Revoke a possibly compromised subkey:
    ///
    /// ```rust
    /// use sequoia_openpgp as openpgp;
    /// # use openpgp::Result;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::types::ReasonForRevocation;
    ///
    /// # fn main() -> Result<()> {
    /// let builder = SubkeyRevocationBuilder::new()
    ///     .set_reason_for_revocation(ReasonForRevocation::KeyCompromised,
    ///                                b"I lost my smartcard.");
    /// # Ok(())
    /// # }
    pub fn set_reason_for_revocation(self, code: ReasonForRevocation,
                                     reason: &[u8])
        -> Result<Self>
    {
        Ok(Self {
            builder: self.builder.set_reason_for_revocation(code, reason)?
        })
    }

    /// Sets the revocation certificate's creation time.
    ///
    /// The creation time is interpreted as the time at which the
    /// subkey should be considered revoked.  For a soft revocation,
    /// artifacts created prior to the revocation are still considered
    /// valid.
    ///
    /// You'll usually want to set this explicitly and not use the
    /// current time.  In particular, if a subkey is compromised,
    /// you'll want to set this to the time when the compromise
    /// happened.
    ///
    /// # Examples
    ///
    /// Create a revocation certificate for a subkey that was
    /// compromised yesterday:
    ///
    /// ```rust
    /// use sequoia_openpgp as openpgp;
    /// # use openpgp::Result;
    /// use openpgp::cert::prelude::*;
    ///
    /// # fn main() -> Result<()> {
    /// # let yesterday = std::time::SystemTime::now();
    /// let builder = SubkeyRevocationBuilder::new()
    ///     .set_signature_creation_time(yesterday);
    /// # Ok(())
    /// # }
    pub fn set_signature_creation_time(self, creation_time: time::SystemTime)
        -> Result<Self>
    {
        Ok(Self {
            builder: self.builder.set_signature_creation_time(creation_time)?
        })
    }

    /// Adds a notation to the revocation certificate.
    ///
    /// Unlike the [`SubkeyRevocationBuilder::set_notation`] method, this function
    /// does not first remove any existing notation with the specified name.
    ///
    /// See [`SignatureBuilder::add_notation`] for further documentation.
    ///
    /// [`SignatureBuilder::add_notation`]: crate::packet::signature::SignatureBuilder::add_notation()
    ///
    /// # Examples
    ///
    /// ```rust
    /// use sequoia_openpgp as openpgp;
    /// # use openpgp::Result;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::packet::signature::subpacket::NotationDataFlags;
    ///
    /// # fn main() -> Result<()> {
    /// let builder = CertRevocationBuilder::new().add_notation(
    ///     "revocation-policy@example.org",
    ///     "https://policy.example.org/cert-revocation-policy",
    ///     NotationDataFlags::empty().set_human_readable(),
    ///     false,
    /// );
    /// # Ok(())
    /// # }
    pub fn add_notation<N, V, F>(self, name: N, value: V, flags: F,
                                 critical: bool)
        -> Result<Self>
    where
        N: AsRef<str>,
        V: AsRef<[u8]>,
        F: Into<Option<NotationDataFlags>>,
    {
        Ok(Self {
            builder: self.builder.add_notation(name, value, flags, critical)?
        })
    }

    /// Sets a notation to the revocation certificate.
    ///
    /// Unlike the [`SubkeyRevocationBuilder::add_notation`] method, this function
    /// first removes any existing notation with the specified name.
    ///
    /// See [`SignatureBuilder::set_notation`] for further documentation.
    ///
    /// [`SignatureBuilder::set_notation`]: crate::packet::signature::SignatureBuilder::set_notation()
    ///
    /// # Examples
    ///
    /// ```rust
    /// use sequoia_openpgp as openpgp;
    /// # use openpgp::Result;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::packet::signature::subpacket::NotationDataFlags;
    ///
    /// # fn main() -> Result<()> {
    /// let builder = CertRevocationBuilder::new().set_notation(
    ///     "revocation-policy@example.org",
    ///     "https://policy.example.org/cert-revocation-policy",
    ///     NotationDataFlags::empty().set_human_readable(),
    ///     false,
    /// );
    /// # Ok(())
    /// # }
    pub fn set_notation<N, V, F>(self, name: N, value: V, flags: F,
                                 critical: bool)
        -> Result<Self>
    where
        N: AsRef<str>,
        V: AsRef<[u8]>,
        F: Into<Option<NotationDataFlags>>,
    {
        Ok(Self {
            builder: self.builder.set_notation(name, value, flags, critical)?
        })
    }

    /// Returns a signed revocation certificate.
    ///
    /// A revocation certificate is generated for `cert` and `key` and
    /// signed using `signer` with the specified hash algorithm.
    /// Normally, you should pass `None` to select the default hash
    /// algorithm.
    ///
    /// # Examples
    ///
    /// Revoke a subkey, which is now considered to be too weak:
    ///
    /// ```rust
    /// use sequoia_openpgp as openpgp;
    /// # use openpgp::Result;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::policy::StandardPolicy;
    /// use openpgp::types::ReasonForRevocation;
    /// # use openpgp::types::RevocationStatus;
    /// # use openpgp::types::SignatureType;
    ///
    /// # fn main() -> Result<()> {
    /// let p = &StandardPolicy::new();
    ///
    /// # let (cert, _) = CertBuilder::new()
    /// #     .add_transport_encryption_subkey()
    /// #     .generate()?;
    /// #
    /// // Create and sign a revocation certificate.
    /// let mut signer = cert.primary_key().key().clone()
    ///     .parts_into_secret()?.into_keypair()?;
    /// let subkey = cert.keys().subkeys().nth(0).unwrap();
    /// let sig = SubkeyRevocationBuilder::new()
    ///     .set_reason_for_revocation(ReasonForRevocation::KeyRetired,
    ///                                b"Revoking due to the recent crypto vulnerabilities.")?
    ///     .build(&mut signer, &cert, subkey.key(), None)?;
    ///
    /// # assert_eq!(sig.typ(), SignatureType::SubkeyRevocation);
    /// #
    /// # // Merge it into the certificate.
    /// # let cert = cert.insert_packets(sig.clone())?;
    /// #
    /// # // Now it's revoked.
    /// # assert_eq!(RevocationStatus::Revoked(vec![&sig]),
    /// #            cert.keys().subkeys().nth(0).unwrap().revocation_status(p, None));
    /// # Ok(())
    /// # }
    pub fn build<H, P>(mut self, signer: &mut dyn Signer,
                       cert: &Cert, key: &Key<P, key::SubordinateRole>,
                       hash_algo: H)
        -> Result<Signature>
        where H: Into<Option<HashAlgorithm>>,
              P: key::KeyParts,
    {
        self.builder = self.builder
            .set_hash_algo(hash_algo.into().unwrap_or(HashAlgorithm::SHA512));

        key.bind(signer, cert, self.builder)
    }
}

impl Deref for SubkeyRevocationBuilder {
    type Target = signature::SignatureBuilder;

    fn deref(&self) -> &Self::Target {
        &self.builder
    }
}

impl TryFrom<signature::SignatureBuilder> for SubkeyRevocationBuilder {
    type Error = anyhow::Error;

    fn try_from(builder: signature::SignatureBuilder) -> Result<Self> {
        if builder.typ() != SignatureType::SubkeyRevocation {
            return Err(
                crate::Error::InvalidArgument(
                    format!("Expected signature type to be SubkeyRevocation but got {}",
                            builder.typ())).into());
        }
        Ok(Self {
            builder
        })
    }
}

/// A builder for revocation certificates for User ID.
///
/// A revocation certificate for a [User ID] has three degrees of
/// freedom: the certificate, the key used to generate the revocation
/// certificate, and the User ID being revoked.
///
/// Normally, the key used to sign the revocation certificate is the
/// certificate's primary key, and the User ID is a User ID that is
/// bound to the certificate.  However, this is not required.  For
/// instance, if Alice has marked Robert's certificate (`R`) as a
/// [designated revoker] for her certificate (`A`), then `R` can
/// revoke `A` or parts of `A`.  In such a case, the certificate is
/// `A`, the key used to sign the revocation certificate comes from
/// `R`, and the User ID being revoked is bound to `A`.
///
/// But, the User ID doesn't technically need to be bound to the
/// certificate either.  For instance, it is technically possible for
/// `R` to create a revocation certificate for a User ID in the
/// context of `A`, even if that User ID is not bound to `A`.
/// Semantically, such a revocation certificate is currently
/// meaningless.
///
/// [User ID]: crate::packet::UserID
/// [designated revoker]: https://tools.ietf.org/html/rfc4880#section-5.2.3.15
///
/// # Examples
///
/// Revoke a User ID that is no longer valid:
///
/// ```rust
/// use sequoia_openpgp as openpgp;
/// # use openpgp::Result;
/// use openpgp::cert::prelude::*;
/// use openpgp::policy::StandardPolicy;
/// use openpgp::types::ReasonForRevocation;
/// use openpgp::types::RevocationStatus;
/// use openpgp::types::SignatureType;
///
/// # fn main() -> Result<()> {
/// let p = &StandardPolicy::new();
///
/// # let (cert, _) = CertBuilder::new()
/// #     .add_userid("some@example.org")
/// #     .generate()?;
/// # assert_eq!(RevocationStatus::NotAsFarAsWeKnow,
/// #            cert.revocation_status(p, None));
/// #
/// // Create and sign a revocation certificate.
/// let mut signer = cert.primary_key().key().clone()
///     .parts_into_secret()?.into_keypair()?;
/// let ua = cert.userids().nth(0).unwrap();
/// let sig = UserIDRevocationBuilder::new()
///     .set_reason_for_revocation(ReasonForRevocation::UIDRetired,
///                                b"Left example.org.")?
///     .build(&mut signer, &cert, ua.userid(), None)?;
///
/// // Merge it into the certificate.
/// let cert = cert.insert_packets(sig.clone())?;
///
/// // Now it's revoked.
/// let ua = cert.userids().nth(0).unwrap();
/// if let RevocationStatus::Revoked(revocations) = ua.revocation_status(p, None) {
///     assert_eq!(revocations.len(), 1);
///     assert_eq!(*revocations[0], sig);
/// } else {
///     panic!("User ID is not revoked.");
/// }
///
/// // But the certificate isn't.
/// assert_eq!(RevocationStatus::NotAsFarAsWeKnow,
///            cert.revocation_status(p, None));
/// # Ok(()) }
/// ```
pub struct UserIDRevocationBuilder {
    builder: signature::SignatureBuilder,
}
assert_send_and_sync!(UserIDRevocationBuilder);

#[allow(clippy::new_without_default)]
impl UserIDRevocationBuilder {
    /// Returns a new `UserIDRevocationBuilder`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use sequoia_openpgp as openpgp;
    /// # use openpgp::Result;
    /// use openpgp::cert::prelude::*;
    ///
    /// # fn main() -> Result<()> {
    /// let builder = UserIDRevocationBuilder::new();
    /// # Ok(())
    /// # }
    pub fn new() -> Self {
        Self {
            builder:
                signature::SignatureBuilder::new(SignatureType::CertificationRevocation)
        }
    }

    /// Sets the reason for revocation.
    ///
    /// Note: of the assigned reasons for revocation, only
    /// [`ReasonForRevocation::UIDRetired`] is appropriate for User
    /// IDs.  This parameter is not fixed, however, to allow the use
    /// of the [private name space].
    ///
    /// [`ReasonForRevocation::UIDRetired`]: crate::types::ReasonForRevocation::UIDRetired
    /// [private name space]: crate::types::ReasonForRevocation::Private
    ///
    ///
    /// # Examples
    ///
    /// Revoke a User ID that is no longer valid:
    ///
    /// ```rust
    /// use sequoia_openpgp as openpgp;
    /// # use openpgp::Result;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::types::ReasonForRevocation;
    ///
    /// # fn main() -> Result<()> {
    /// let builder = UserIDRevocationBuilder::new()
    ///     .set_reason_for_revocation(ReasonForRevocation::UIDRetired,
    ///                                b"Left example.org.");
    /// # Ok(())
    /// # }
    pub fn set_reason_for_revocation(self, code: ReasonForRevocation,
                                     reason: &[u8])
        -> Result<Self>
    {
        Ok(Self {
            builder: self.builder.set_reason_for_revocation(code, reason)?
        })
    }

    /// Sets the revocation certificate's creation time.
    ///
    /// The creation time is interpreted as the time at which the User
    /// ID should be considered revoked.
    ///
    /// You'll usually want to set this explicitly and not use the
    /// current time.  In particular, if a User ID is retired, you'll
    /// want to set this to the time when the User ID was actually
    /// retired.
    ///
    /// # Examples
    ///
    /// Create a revocation certificate for a User ID that was
    /// retired yesterday:
    ///
    /// ```rust
    /// use sequoia_openpgp as openpgp;
    /// # use openpgp::Result;
    /// use openpgp::cert::prelude::*;
    ///
    /// # fn main() -> Result<()> {
    /// # let yesterday = std::time::SystemTime::now();
    /// let builder = UserIDRevocationBuilder::new()
    ///     .set_signature_creation_time(yesterday);
    /// # Ok(())
    /// # }
    pub fn set_signature_creation_time(self, creation_time: time::SystemTime)
        -> Result<Self>
    {
        Ok(Self {
            builder: self.builder.set_signature_creation_time(creation_time)?
        })
    }

    /// Adds a notation to the revocation certificate.
    ///
    /// Unlike the [`UserIDRevocationBuilder::set_notation`] method, this function
    /// does not first remove any existing notation with the specified name.
    ///
    /// See [`SignatureBuilder::add_notation`] for further documentation.
    ///
    /// [`SignatureBuilder::add_notation`]: crate::packet::signature::SignatureBuilder::add_notation()
    ///
    /// # Examples
    ///
    /// ```rust
    /// use sequoia_openpgp as openpgp;
    /// # use openpgp::Result;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::packet::signature::subpacket::NotationDataFlags;
    ///
    /// # fn main() -> Result<()> {
    /// let builder = CertRevocationBuilder::new().add_notation(
    ///     "revocation-policy@example.org",
    ///     "https://policy.example.org/cert-revocation-policy",
    ///     NotationDataFlags::empty().set_human_readable(),
    ///     false,
    /// );
    /// # Ok(())
    /// # }
    pub fn add_notation<N, V, F>(self, name: N, value: V, flags: F,
                                 critical: bool)
        -> Result<Self>
    where
        N: AsRef<str>,
        V: AsRef<[u8]>,
        F: Into<Option<NotationDataFlags>>,
    {
        Ok(Self {
            builder: self.builder.add_notation(name, value, flags, critical)?
        })
    }

    /// Sets a notation to the revocation certificate.
    ///
    /// Unlike the [`UserIDRevocationBuilder::add_notation`] method, this function
    /// first removes any existing notation with the specified name.
    ///
    /// See [`SignatureBuilder::set_notation`] for further documentation.
    ///
    /// [`SignatureBuilder::set_notation`]: crate::packet::signature::SignatureBuilder::set_notation()
    ///
    /// # Examples
    ///
    /// ```rust
    /// use sequoia_openpgp as openpgp;
    /// # use openpgp::Result;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::packet::signature::subpacket::NotationDataFlags;
    ///
    /// # fn main() -> Result<()> {
    /// let builder = CertRevocationBuilder::new().set_notation(
    ///     "revocation-policy@example.org",
    ///     "https://policy.example.org/cert-revocation-policy",
    ///     NotationDataFlags::empty().set_human_readable(),
    ///     false,
    /// );
    /// # Ok(())
    /// # }
    pub fn set_notation<N, V, F>(self, name: N, value: V, flags: F,
                                 critical: bool)
        -> Result<Self>
    where
        N: AsRef<str>,
        V: AsRef<[u8]>,
        F: Into<Option<NotationDataFlags>>,
    {
        Ok(Self {
            builder: self.builder.set_notation(name, value, flags, critical)?
        })
    }

    /// Returns a signed revocation certificate.
    ///
    /// A revocation certificate is generated for `cert` and `userid`
    /// and signed using `signer` with the specified hash algorithm.
    /// Normally, you should pass `None` to select the default hash
    /// algorithm.
    ///
    /// # Examples
    ///
    /// Revoke a User ID, because the user has left the organization:
    ///
    /// ```rust
    /// use sequoia_openpgp as openpgp;
    /// # use openpgp::Result;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::policy::StandardPolicy;
    /// use openpgp::types::ReasonForRevocation;
    /// # use openpgp::types::RevocationStatus;
    /// # use openpgp::types::SignatureType;
    ///
    /// # fn main() -> Result<()> {
    /// let p = &StandardPolicy::new();
    ///
    /// # let (cert, _) = CertBuilder::new()
    /// #     .add_userid("some@example.org")
    /// #     .generate()?;
    /// #
    /// // Create and sign a revocation certificate.
    /// let mut signer = cert.primary_key().key().clone()
    ///     .parts_into_secret()?.into_keypair()?;
    /// let ua = cert.userids().nth(0).unwrap();
    /// let sig = UserIDRevocationBuilder::new()
    ///     .set_reason_for_revocation(ReasonForRevocation::UIDRetired,
    ///                                b"Left example.org.")?
    ///     .build(&mut signer, &cert, ua.userid(), None)?;
    ///
    /// # assert_eq!(sig.typ(), SignatureType::CertificationRevocation);
    /// #
    /// # // Merge it into the certificate.
    /// # let cert = cert.insert_packets(sig.clone())?;
    /// #
    /// # // Now it's revoked.
    /// # assert_eq!(RevocationStatus::Revoked(vec![&sig]),
    /// #            cert.userids().nth(0).unwrap().revocation_status(p, None));
    /// # Ok(())
    /// # }
    pub fn build<H>(mut self, signer: &mut dyn Signer,
                    cert: &Cert, userid: &UserID,
                    hash_algo: H)
        -> Result<Signature>
        where H: Into<Option<HashAlgorithm>>
    {
        self.builder = self.builder
            .set_hash_algo(hash_algo.into().unwrap_or(HashAlgorithm::SHA512));

        userid.bind(signer, cert, self.builder)
    }
}

impl Deref for UserIDRevocationBuilder {
    type Target = signature::SignatureBuilder;

    fn deref(&self) -> &Self::Target {
        &self.builder
    }
}

impl TryFrom<signature::SignatureBuilder> for UserIDRevocationBuilder {
    type Error = anyhow::Error;

    fn try_from(builder: signature::SignatureBuilder) -> Result<Self> {
        if builder.typ() != SignatureType::CertificationRevocation {
            return Err(
                crate::Error::InvalidArgument(
                    format!("Expected signature type to be CertificationRevocation but got {}",
                            builder.typ())).into());
        }
        Ok(Self {
            builder
        })
    }
}

/// A builder for revocation certificates for User Attributes.
///
/// A revocation certificate for a [User Attribute] has three degrees of
/// freedom: the certificate, the key used to generate the revocation
/// certificate, and the User Attribute being revoked.
///
/// Normally, the key used to sign the revocation certificate is the
/// certificate's primary key, and the User Attribute is a User
/// Attribute that is bound to the certificate.  However, this is not
/// required.  For instance, if Alice has marked Robert's certificate
/// (`R`) as a [designated revoker] for her certificate (`A`), then
/// `R` can revoke `A` or parts of `A`.  In such a case, the
/// certificate is `A`, the key used to sign the revocation
/// certificate comes from `R`, and the User Attribute being revoked
/// is bound to `A`.
///
/// But, the User Attribute doesn't technically need to be bound to
/// the certificate either.  For instance, it is technically possible
/// for `R` to create a revocation certificate for a User Attribute in
/// the context of `A`, even if that User Attribute is not bound to
/// `A`.  Semantically, such a revocation certificate is currently
/// meaningless.
///
/// [User Attribute]: crate::packet::user_attribute
/// [designated revoker]: https://tools.ietf.org/html/rfc4880#section-5.2.3.15
///
/// # Examples
///
/// Revoke a User Attribute that is no longer valid:
///
/// ```rust
/// # use openpgp::packet::user_attribute::Subpacket;
/// use sequoia_openpgp as openpgp;
/// # use openpgp::Result;
/// use openpgp::cert::prelude::*;
/// # use openpgp::packet::UserAttribute;
/// use openpgp::policy::StandardPolicy;
/// use openpgp::types::ReasonForRevocation;
/// use openpgp::types::RevocationStatus;
/// use openpgp::types::SignatureType;
///
/// # fn main() -> Result<()> {
/// let p = &StandardPolicy::new();
///
/// # // Create some user attribute. Doctests do not pass cfg(test),
/// # // so UserAttribute::arbitrary is not available
/// # let sp = Subpacket::Unknown(7, vec![7; 7].into_boxed_slice());
/// # let user_attribute = UserAttribute::new(&[sp])?;
/// #
/// # let (cert, _) = CertBuilder::new()
/// #     .add_user_attribute(user_attribute)
/// #     .generate()?;
/// # assert_eq!(RevocationStatus::NotAsFarAsWeKnow,
/// #            cert.revocation_status(p, None));
/// #
/// // Create and sign a revocation certificate.
/// let mut signer = cert.primary_key().key().clone()
///     .parts_into_secret()?.into_keypair()?;
/// let ua = cert.user_attributes().nth(0).unwrap();
/// let sig = UserAttributeRevocationBuilder::new()
///     .set_reason_for_revocation(ReasonForRevocation::UIDRetired,
///                                b"Lost the beard.")?
///     .build(&mut signer, &cert, ua.user_attribute(), None)?;
///
/// // Merge it into the certificate.
/// let cert = cert.insert_packets(sig.clone())?;
///
/// // Now it's revoked.
/// let ua = cert.user_attributes().nth(0).unwrap();
/// if let RevocationStatus::Revoked(revocations) = ua.revocation_status(p, None) {
///     assert_eq!(revocations.len(), 1);
///     assert_eq!(*revocations[0], sig);
/// } else {
///     panic!("User Attribute is not revoked.");
/// }
///
/// // But the certificate isn't.
/// assert_eq!(RevocationStatus::NotAsFarAsWeKnow,
///            cert.revocation_status(p, None));
/// # Ok(()) }
/// ```
pub struct UserAttributeRevocationBuilder {
    builder: signature::SignatureBuilder,
}
assert_send_and_sync!(UserAttributeRevocationBuilder);

#[allow(clippy::new_without_default)]
impl UserAttributeRevocationBuilder {
    /// Returns a new `UserAttributeRevocationBuilder`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use sequoia_openpgp as openpgp;
    /// # use openpgp::Result;
    /// use openpgp::cert::prelude::*;
    ///
    /// # fn main() -> Result<()> {
    /// let builder = UserAttributeRevocationBuilder::new();
    /// # Ok(())
    /// # }
    pub fn new() -> Self {
        Self {
            builder:
                signature::SignatureBuilder::new(SignatureType::CertificationRevocation)
        }
    }

    /// Sets the reason for revocation.
    ///
    /// Note: of the assigned reasons for revocation, only
    /// [`ReasonForRevocation::UIDRetired`] is appropriate for User
    /// Attributes.  This parameter is not fixed, however, to allow
    /// the use of the [private name space].
    ///
    /// [`ReasonForRevocation::UIDRetired`]: crate::types::ReasonForRevocation::UIDRetired
    /// [private name space]: crate::types::ReasonForRevocation::Private
    ///
    /// # Examples
    ///
    /// Revoke a User Attribute that is no longer valid:
    ///
    /// ```rust
    /// use sequoia_openpgp as openpgp;
    /// # use openpgp::Result;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::types::ReasonForRevocation;
    ///
    /// # fn main() -> Result<()> {
    /// let builder = UserAttributeRevocationBuilder::new()
    ///     .set_reason_for_revocation(ReasonForRevocation::UIDRetired,
    ///                                b"Lost the beard.");
    /// # Ok(())
    /// # }
    pub fn set_reason_for_revocation(self, code: ReasonForRevocation,
                                     reason: &[u8])
        -> Result<Self>
    {
        Ok(Self {
            builder: self.builder.set_reason_for_revocation(code, reason)?
        })
    }

    /// Sets the revocation certificate's creation time.
    ///
    /// The creation time is interpreted as the time at which the User
    /// Attribute should be considered revoked.
    ///
    /// You'll usually want to set this explicitly and not use the
    /// current time.  In particular, if a User Attribute is retired,
    /// you'll want to set this to the time when the User Attribute
    /// was actually retired.
    ///
    /// # Examples
    ///
    /// Create a revocation certificate for a User Attribute that was
    /// retired yesterday:
    ///
    /// ```rust
    /// use sequoia_openpgp as openpgp;
    /// # use openpgp::Result;
    /// use openpgp::cert::prelude::*;
    ///
    /// # fn main() -> Result<()> {
    /// # let yesterday = std::time::SystemTime::now();
    /// let builder = UserAttributeRevocationBuilder::new()
    ///     .set_signature_creation_time(yesterday);
    /// # Ok(())
    /// # }
    pub fn set_signature_creation_time(self, creation_time: time::SystemTime)
        -> Result<Self>
    {
        Ok(Self {
            builder: self.builder.set_signature_creation_time(creation_time)?
        })
    }

    /// Adds a notation to the revocation certificate.
    ///
    /// Unlike the [`UserAttributeRevocationBuilder::set_notation`] method, this function
    /// does not first remove any existing notation with the specified name.
    ///
    /// See [`SignatureBuilder::add_notation`] for further documentation.
    ///
    /// [`SignatureBuilder::add_notation`]: crate::packet::signature::SignatureBuilder::add_notation()
    ///
    /// # Examples
    ///
    /// ```rust
    /// use sequoia_openpgp as openpgp;
    /// # use openpgp::Result;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::packet::signature::subpacket::NotationDataFlags;
    ///
    /// # fn main() -> Result<()> {
    /// let builder = CertRevocationBuilder::new().add_notation(
    ///     "revocation-policy@example.org",
    ///     "https://policy.example.org/cert-revocation-policy",
    ///     NotationDataFlags::empty().set_human_readable(),
    ///     false,
    /// );
    /// # Ok(())
    /// # }
    pub fn add_notation<N, V, F>(self, name: N, value: V, flags: F,
                                 critical: bool)
        -> Result<Self>
    where
        N: AsRef<str>,
        V: AsRef<[u8]>,
        F: Into<Option<NotationDataFlags>>,
    {
        Ok(Self {
            builder: self.builder.add_notation(name, value, flags, critical)?
        })
    }

    /// Sets a notation to the revocation certificate.
    ///
    /// Unlike the [`UserAttributeRevocationBuilder::add_notation`] method, this function
    /// first removes any existing notation with the specified name.
    ///
    /// See [`SignatureBuilder::set_notation`] for further documentation.
    ///
    /// [`SignatureBuilder::set_notation`]: crate::packet::signature::SignatureBuilder::set_notation()
    ///
    /// # Examples
    ///
    /// ```rust
    /// use sequoia_openpgp as openpgp;
    /// # use openpgp::Result;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::packet::signature::subpacket::NotationDataFlags;
    ///
    /// # fn main() -> Result<()> {
    /// let builder = CertRevocationBuilder::new().set_notation(
    ///     "revocation-policy@example.org",
    ///     "https://policy.example.org/cert-revocation-policy",
    ///     NotationDataFlags::empty().set_human_readable(),
    ///     false,
    /// );
    /// # Ok(())
    /// # }
    pub fn set_notation<N, V, F>(self, name: N, value: V, flags: F,
                                 critical: bool)
        -> Result<Self>
    where
        N: AsRef<str>,
        V: AsRef<[u8]>,
        F: Into<Option<NotationDataFlags>>,
    {
        Ok(Self {
            builder: self.builder.set_notation(name, value, flags, critical)?
        })
    }

    /// Returns a signed revocation certificate.
    ///
    /// A revocation certificate is generated for `cert` and `ua` and
    /// signed using `signer` with the specified hash algorithm.
    /// Normally, you should pass `None` to select the default hash
    /// algorithm.
    ///
    /// # Examples
    ///
    /// Revoke a User Attribute, because the identity is no longer
    /// valid:
    ///
    /// ```rust
    /// # use openpgp::packet::user_attribute::Subpacket;
    /// use sequoia_openpgp as openpgp;
    /// # use openpgp::Result;
    /// use openpgp::cert::prelude::*;
    /// # use openpgp::packet::UserAttribute;
    /// use openpgp::policy::StandardPolicy;
    /// use openpgp::types::ReasonForRevocation;
    /// # use openpgp::types::RevocationStatus;
    /// # use openpgp::types::SignatureType;
    ///
    /// # fn main() -> Result<()> {
    /// let p = &StandardPolicy::new();
    ///
    /// # // Create some user attribute. Doctests do not pass cfg(test),
    /// # // so UserAttribute::arbitrary is not available
    /// # let sp = Subpacket::Unknown(7, vec![7; 7].into_boxed_slice());
    /// # let user_attribute = UserAttribute::new(&[sp])?;
    /// #
    /// # let (cert, _) = CertBuilder::new()
    /// #     .add_user_attribute(user_attribute)
    /// #     .generate()?;
    /// // Create and sign a revocation certificate.
    /// let mut signer = cert.primary_key().key().clone()
    ///     .parts_into_secret()?.into_keypair()?;
    /// let ua = cert.user_attributes().nth(0).unwrap();
    /// let sig = UserAttributeRevocationBuilder::new()
    ///     .set_reason_for_revocation(ReasonForRevocation::UIDRetired,
    ///                                b"Lost the beard.")?
    ///     .build(&mut signer, &cert, ua.user_attribute(), None)?;
    ///
    /// # assert_eq!(sig.typ(), SignatureType::CertificationRevocation);
    /// #
    /// # // Merge it into the certificate.
    /// # let cert = cert.insert_packets(sig.clone())?;
    /// #
    /// # // Now it's revoked.
    /// # assert_eq!(RevocationStatus::Revoked(vec![&sig]),
    /// #            cert.user_attributes().nth(0).unwrap().revocation_status(p, None));
    /// # Ok(())
    /// # }
    pub fn build<H>(mut self, signer: &mut dyn Signer,
                    cert: &Cert, ua: &UserAttribute,
                    hash_algo: H)
        -> Result<Signature>
        where H: Into<Option<HashAlgorithm>>
    {
        self.builder = self.builder
            .set_hash_algo(hash_algo.into().unwrap_or(HashAlgorithm::SHA512));

        ua.bind(signer, cert, self.builder)
    }
}

impl Deref for UserAttributeRevocationBuilder {
    type Target = signature::SignatureBuilder;

    fn deref(&self) -> &Self::Target {
        &self.builder
    }
}

impl TryFrom<signature::SignatureBuilder> for UserAttributeRevocationBuilder {
    type Error = anyhow::Error;

    fn try_from(builder: signature::SignatureBuilder) -> Result<Self> {
        if builder.typ() != SignatureType::CertificationRevocation {
            return Err(
                crate::Error::InvalidArgument(
                    format!("Expected signature type to be CertificationRevocation but got {}",
                            builder.typ())).into());
        }
        Ok(Self {
            builder
        })
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn try_into_cert_revocation_builder_success() -> crate::Result<()> {
        use std::convert::TryInto;
        use crate as openpgp;
        use openpgp::cert::prelude::*;
        use openpgp::packet::signature::SignatureBuilder;
        use openpgp::cert::CertRevocationBuilder;
        use openpgp::types::SignatureType;

        let (cert, _) = CertBuilder::new()
            .generate()?;

        // Create and sign a revocation certificate.
        let mut signer = cert.primary_key().key().clone()
            .parts_into_secret()?.into_keypair()?;
        let builder = SignatureBuilder::new(SignatureType::KeyRevocation);
        let revocation_builder: CertRevocationBuilder = builder.try_into()?;
        let sig = revocation_builder.build(&mut signer, &cert, None)?;
        assert_eq!(sig.typ(), SignatureType::KeyRevocation);
        Ok(())
    }

    #[test]
    fn try_into_cert_revocation_builder_failure() -> crate::Result<()> {
        use std::convert::TryInto;
        use crate as openpgp;
        use openpgp::packet::signature::SignatureBuilder;
        use openpgp::cert::CertRevocationBuilder;
        use openpgp::types::SignatureType;

        let builder = SignatureBuilder::new(SignatureType::Binary);
        let result: openpgp::Result<CertRevocationBuilder> = builder.try_into();
        assert!(result.is_err());
        Ok(())
    }

    #[test]
    fn try_into_subkey_revocation_builder_success() -> crate::Result<()> {
        use std::convert::TryInto;
        use crate as openpgp;
        use openpgp::cert::prelude::*;
        use openpgp::packet::signature::SignatureBuilder;
        use openpgp::cert::SubkeyRevocationBuilder;
        use openpgp::types::SignatureType;

        let (cert, _) = CertBuilder::new()
            .add_transport_encryption_subkey()
            .generate()?;

        // Create and sign a revocation certificate.
        let mut signer = cert.primary_key().key().clone()
            .parts_into_secret()?.into_keypair()?;
        let subkey = cert.keys().subkeys().nth(0).unwrap();
        let builder = SignatureBuilder::new(SignatureType::SubkeyRevocation);
        let revocation_builder: SubkeyRevocationBuilder = builder.try_into()?;
        let sig = revocation_builder.build(&mut signer, &cert, subkey.key(), None)?;
        assert_eq!(sig.typ(), SignatureType::SubkeyRevocation);
        Ok(())
    }

    #[test]
    fn try_into_subkey_revocation_builder_failure() -> crate::Result<()> {
        use std::convert::TryInto;
        use crate as openpgp;
        use openpgp::packet::signature::SignatureBuilder;
        use openpgp::cert::SubkeyRevocationBuilder;
        use openpgp::types::SignatureType;

        let builder = SignatureBuilder::new(SignatureType::Binary);
        let result: openpgp::Result<SubkeyRevocationBuilder> = builder.try_into();
        assert!(result.is_err());
        Ok(())
    }

    #[test]
    fn try_into_userid_revocation_builder_success() -> crate::Result<()> {
        use std::convert::TryInto;
        use crate as openpgp;
        use openpgp::cert::prelude::*;
        use openpgp::packet::signature::SignatureBuilder;
        use openpgp::cert::UserIDRevocationBuilder;
        use openpgp::types::SignatureType;

        let (cert, _) = CertBuilder::new()
            .add_userid("test@example.com")
            .generate()?;

        // Create and sign a revocation certificate.
        let mut signer = cert.primary_key().key().clone()
            .parts_into_secret()?.into_keypair()?;
        let user_id = cert.userids().next().unwrap();
        let builder = SignatureBuilder::new(SignatureType::CertificationRevocation);
        let revocation_builder: UserIDRevocationBuilder = builder.try_into()?;
        let sig = revocation_builder.build(&mut signer, &cert, &user_id, None)?;
        assert_eq!(sig.typ(), SignatureType::CertificationRevocation);
        Ok(())
    }

    #[test]
    fn try_into_userid_revocation_builder_failure() -> crate::Result<()> {
        use std::convert::TryInto;
        use crate as openpgp;
        use openpgp::packet::signature::SignatureBuilder;
        use openpgp::cert::UserIDRevocationBuilder;
        use openpgp::types::SignatureType;

        let builder = SignatureBuilder::new(SignatureType::Binary);
        let result: openpgp::Result<UserIDRevocationBuilder> = builder.try_into();
        assert!(result.is_err());
        Ok(())
    }

    #[test]
    fn try_into_userattribute_revocation_builder_success() -> crate::Result<()> {
        use std::convert::TryInto;
        use crate as openpgp;
        use openpgp::cert::prelude::*;
        use openpgp::packet::prelude::*;
        use openpgp::packet::signature::SignatureBuilder;
        use openpgp::packet::user_attribute::Subpacket;
        use openpgp::cert::UserAttributeRevocationBuilder;
        use openpgp::types::SignatureType;

        let sp = Subpacket::Unknown(7, vec![7; 7].into_boxed_slice());
        let user_attribute = UserAttribute::new(&[sp])?;

        let (cert, _) = CertBuilder::new()
            .add_user_attribute(user_attribute)
            .generate()?;

        // Create and sign a revocation certificate.
        let mut signer = cert.primary_key().key().clone()
            .parts_into_secret()?.into_keypair()?;
        let user_attribute = cert.user_attributes().next().unwrap();
        let builder = SignatureBuilder::new(SignatureType::CertificationRevocation);
        let revocation_builder: UserAttributeRevocationBuilder = builder.try_into()?;
        let sig = revocation_builder.build(&mut signer, &cert, &user_attribute, None)?;
        assert_eq!(sig.typ(), SignatureType::CertificationRevocation);
        Ok(())
    }

    #[test]
    fn try_into_userattribute_revocation_builder_failure() -> crate::Result<()> {
        use std::convert::TryInto;
        use crate as openpgp;
        use openpgp::packet::signature::SignatureBuilder;
        use openpgp::cert::UserAttributeRevocationBuilder;
        use openpgp::types::SignatureType;

        let builder = SignatureBuilder::new(SignatureType::Binary);
        let result: openpgp::Result<UserAttributeRevocationBuilder> = builder.try_into();
        assert!(result.is_err());
        Ok(())
    }
}
