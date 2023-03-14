use std::time;
use std::marker::PhantomData;

use crate::packet;
use crate::packet::{
    Key,
    key::Key4,
    key::KeyRole,
    key::SecretKey as KeySecretKey,
    key::SecretParts as KeySecretParts,
};
use crate::Result;
use crate::packet::Signature;
use crate::packet::signature::{
    self,
    SignatureBuilder,
    subpacket::SubpacketTag,
};
use crate::cert::prelude::*;
use crate::Error;
use crate::crypto::{Password, Signer};
use crate::types::{
    Features,
    HashAlgorithm,
    KeyFlags,
    SignatureType,
    SymmetricAlgorithm,
    RevocationKey,
};

mod key;
pub use key::{
    KeyBuilder,
    SubkeyBuilder,
};

/// Groups symmetric and asymmetric algorithms.
///
/// This is used to select a suite of ciphers.
///
/// # Examples
///
/// ```
/// use sequoia_openpgp as openpgp;
/// use openpgp::cert::prelude::*;
/// use openpgp::types::PublicKeyAlgorithm;
///
/// # fn main() -> openpgp::Result<()> {
/// let (ecc, _) =
///     CertBuilder::general_purpose(None, Some("alice@example.org"))
///         .set_cipher_suite(CipherSuite::Cv25519)
///         .generate()?;
/// assert_eq!(ecc.primary_key().pk_algo(), PublicKeyAlgorithm::EdDSA);
///
/// let (rsa, _) =
///     CertBuilder::general_purpose(None, Some("alice@example.org"))
///         .set_cipher_suite(CipherSuite::RSA4k)
///         .generate()?;
/// assert_eq!(rsa.primary_key().pk_algo(), PublicKeyAlgorithm::RSAEncryptSign);
/// # Ok(())
/// # }
/// ```
#[derive(Clone, Copy, PartialEq, PartialOrd, Eq, Ord, Debug)]
pub enum CipherSuite {
    /// EdDSA and ECDH over Curve25519 with SHA512 and AES256
    Cv25519,
    /// 3072 bit RSA with SHA512 and AES256
    RSA3k,
    /// EdDSA and ECDH over NIST P-256 with SHA256 and AES256
    P256,
    /// EdDSA and ECDH over NIST P-384 with SHA384 and AES256
    P384,
    /// EdDSA and ECDH over NIST P-521 with SHA512 and AES256
    P521,
    /// 2048 bit RSA with SHA512 and AES256
    RSA2k,
    /// 4096 bit RSA with SHA512 and AES256
    RSA4k,
}
assert_send_and_sync!(CipherSuite);

impl Default for CipherSuite {
    fn default() -> Self {
        CipherSuite::Cv25519
    }
}

impl CipherSuite {
    /// Returns whether the currently selected cryptographic backend
    /// supports the encryption and signing algorithms that the cipher
    /// suite selects.
    pub fn is_supported(&self) -> Result<()> {
        use crate::types::{Curve, PublicKeyAlgorithm};
        use CipherSuite::*;

        macro_rules! check_pk {
            ($pk: expr) => {
                if ! $pk.is_supported() {
                    return Err(Error::UnsupportedPublicKeyAlgorithm($pk)
                               .into());
                }
            }
        }

        macro_rules! check_curve {
            ($curve: expr) => {
                if ! $curve.is_supported() {
                    return Err(Error::UnsupportedEllipticCurve($curve)
                               .into());
                }
            }
        }

        match self {
            Cv25519 => {
                check_pk!(PublicKeyAlgorithm::EdDSA);
                check_curve!(Curve::Ed25519);
                check_pk!(PublicKeyAlgorithm::ECDH);
                check_curve!(Curve::Cv25519);
            },
            RSA2k | RSA3k | RSA4k => {
                check_pk!(PublicKeyAlgorithm::RSAEncryptSign);
            },
            P256 => {
                check_pk!(PublicKeyAlgorithm::ECDSA);
                check_curve!(Curve::NistP256);
                check_pk!(PublicKeyAlgorithm::ECDH);
            },
            P384 => {
                check_pk!(PublicKeyAlgorithm::ECDSA);
                check_curve!(Curve::NistP384);
                check_pk!(PublicKeyAlgorithm::ECDH);
            },
            P521 => {
                check_pk!(PublicKeyAlgorithm::ECDSA);
                check_curve!(Curve::NistP521);
                check_pk!(PublicKeyAlgorithm::ECDH);
            },
        }
        Ok(())
    }

    fn generate_key<K, R>(self, flags: K)
        -> Result<Key<KeySecretParts, R>>
        where R: KeyRole,
              K: AsRef<KeyFlags>,
    {
        use crate::types::Curve;

        match self {
            CipherSuite::RSA2k =>
                Key4::generate_rsa(2048),
            CipherSuite::RSA3k =>
                Key4::generate_rsa(3072),
            CipherSuite::RSA4k =>
                Key4::generate_rsa(4096),
            CipherSuite::Cv25519 | CipherSuite::P256 |
            CipherSuite::P384 | CipherSuite::P521 => {
                let flags = flags.as_ref();
                let sign = flags.for_certification() || flags.for_signing()
                    || flags.for_authentication();
                let encrypt = flags.for_transport_encryption()
                    || flags.for_storage_encryption();
                let curve = match self {
                    CipherSuite::Cv25519 if sign => Curve::Ed25519,
                    CipherSuite::Cv25519 if encrypt => Curve::Cv25519,
                    CipherSuite::Cv25519 => {
                        return Err(Error::InvalidOperation(
                            "No key flags set".into())
                            .into());
                    }
                    CipherSuite::P256 => Curve::NistP256,
                    CipherSuite::P384 => Curve::NistP384,
                    CipherSuite::P521 => Curve::NistP521,
                    _ => unreachable!(),
                };

                match (sign, encrypt) {
                    (true, false) => Key4::generate_ecc(true, curve),
                    (false, true) => Key4::generate_ecc(false, curve),
                    (true, true) =>
                        Err(Error::InvalidOperation(
                            "Can't use key for encryption and signing".into())
                            .into()),
                    (false, false) =>
                        Err(Error::InvalidOperation(
                            "No key flags set".into())
                            .into()),
                }
            },
        }.map(|key| key.into())
    }
}

#[derive(Clone, Debug)]
pub struct KeyBlueprint {
    flags: KeyFlags,
    validity: Option<time::Duration>,
    // If not None, uses the specified ciphersuite.  Otherwise, uses
    // CertBuilder::ciphersuite.
    ciphersuite: Option<CipherSuite>,
}
assert_send_and_sync!(KeyBlueprint);

/// Simplifies the generation of OpenPGP certificates.
///
/// A builder to generate complex certificate hierarchies with multiple
/// [`UserID`s], [`UserAttribute`s], and [`Key`s].
///
/// This builder does not aim to be as flexible as creating
/// certificates manually, but it should be sufficiently powerful to
/// cover most use cases.
///
/// [`UserID`s]: crate::packet::UserID
/// [`UserAttribute`s]: crate::packet::user_attribute::UserAttribute
/// [`Key`s]: crate::packet::Key
///
/// # Security considerations
///
/// ## Expiration
///
/// There are two ways to invalidate cryptographic key material:
/// revocation and freshness.  Both variants come with their own
/// challenges.  Revocations rely on a robust channel to update
/// certificates (and attackers may interfere with that).
///
/// On the other hand, freshness involves creating key material that
/// expires after a certain time, then periodically extending the
/// expiration time.  Again, consumers need a way to update
/// certificates, but should that fail (maybe because it was
/// interfered with), the consumer errs on the side of no longer
/// trusting that key material.
///
/// Because of the way metadata is added to OpenPGP certificates,
/// attackers who control the certificate lookup and update mechanism
/// may strip components like signatures from the certificate.  This
/// has implications for the robustness of relying on freshness.
///
/// If you first create a certificate that does not expire, and then
/// change your mind and set an expiration time, an attacker can
/// simply strip off that update, yielding the original certificate
/// that does not expire.
///
/// Hence, to ensure robust key expiration, you must set an expiration
/// with [`CertBuilder::set_validity_period`] when you create the
/// certificate.
///
/// By default, the `CertBuilder` creates certificates that do not
/// expire, because the expiration time is a policy decision and
/// depends on the use case.  For general purpose certificates,
/// [`CertBuilder::general_purpose`] sets the validity period to
/// roughly three years.
///
/// # Examples
///
/// Generate a general-purpose certificate with one User ID:
///
/// ```
/// use sequoia_openpgp as openpgp;
/// use openpgp::cert::prelude::*;
///
/// # fn main() -> openpgp::Result<()> {
/// let (cert, rev) =
///     CertBuilder::general_purpose(None, Some("alice@example.org"))
///         .generate()?;
/// # Ok(())
/// # }
/// ```
pub struct CertBuilder<'a> {
    creation_time: Option<std::time::SystemTime>,
    ciphersuite: CipherSuite,
    primary: KeyBlueprint,
    subkeys: Vec<(Option<SignatureBuilder>, KeyBlueprint)>,
    userids: Vec<(Option<SignatureBuilder>, packet::UserID)>,
    user_attributes: Vec<(Option<SignatureBuilder>, packet::UserAttribute)>,
    password: Option<Password>,
    revocation_keys: Option<Vec<RevocationKey>>,
    phantom: PhantomData<&'a ()>,
}
assert_send_and_sync!(CertBuilder<'_>);

#[allow(clippy::new_without_default)]
impl CertBuilder<'_> {
    /// Returns a new `CertBuilder`.
    ///
    /// The returned builder is configured to generate a minimal
    /// OpenPGP certificate, a certificate with just a
    /// certification-capable primary key.  You'll typically want to
    /// add at least one User ID (using
    /// [`CertBuilder::add_userid`]). and some subkeys (using
    /// [`CertBuilder::add_signing_subkey`],
    /// [`CertBuilder::add_transport_encryption_subkey`], etc.).
    ///
    /// By default, the generated certificate does not expire.  It is
    /// recommended to set a suitable validity period using
    /// [`CertBuilder::set_validity_period`].  See [this
    /// section](CertBuilder#expiration) of the type's documentation
    /// for security considerations of key expiration.
    ///
    /// # Examples
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::cert::prelude::*;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// let (cert, rev) =
    ///     CertBuilder::new()
    ///         .add_userid("Alice Lovelace <alice@lovelace.name>")
    ///         .add_signing_subkey()
    ///         .add_transport_encryption_subkey()
    ///         .add_storage_encryption_subkey()
    ///         .generate()?;
    /// # assert_eq!(cert.keys().count(), 1 + 3);
    /// # assert_eq!(cert.userids().count(), 1);
    /// # assert_eq!(cert.user_attributes().count(), 0);
    /// # Ok(())
    /// # }
    /// ```
    pub fn new() -> Self {
        CertBuilder {
            creation_time: None,
            ciphersuite: CipherSuite::default(),
            primary: KeyBlueprint{
                flags: KeyFlags::empty().set_certification(),
                validity: None,
                ciphersuite: None,
            },
            subkeys: vec![],
            userids: vec![],
            user_attributes: vec![],
            password: None,
            revocation_keys: None,
            phantom: PhantomData,
        }
    }

    /// Generates a general-purpose certificate.
    ///
    /// The returned builder is set to generate a certificate with a
    /// certification-capable primary key, a signing-capable subkey,
    /// and an encryption-capable subkey.  The encryption subkey is
    /// marked as being appropriate for both data in transit and data
    /// at rest.
    ///
    /// The certificate and all subkeys are valid for approximately
    /// three years.
    ///
    /// # Examples
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::cert::prelude::*;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// let (cert, rev) =
    ///     CertBuilder::general_purpose(None,
    ///                                  Some("Alice Lovelace <alice@example.org>"))
    ///         .generate()?;
    /// # assert_eq!(cert.keys().count(), 3);
    /// # assert_eq!(cert.userids().count(), 1);
    /// # Ok(())
    /// # }
    /// ```
    pub fn general_purpose<C, U>(ciphersuite: C, userid: Option<U>) -> Self
        where C: Into<Option<CipherSuite>>,
              U: Into<packet::UserID>
    {
        let mut builder = Self::new()
            .set_cipher_suite(ciphersuite.into().unwrap_or_default())
            .set_primary_key_flags(KeyFlags::empty().set_certification())
            .set_validity_period(
                time::Duration::new(3 * 52 * 7 * 24 * 60 * 60, 0))
            .add_signing_subkey()
            .add_subkey(KeyFlags::empty()
                        .set_transport_encryption()
                        .set_storage_encryption(), None, None);
        if let Some(u) = userid.map(Into::into) {
            builder = builder.add_userid(u);
        }

        builder
    }

    /// Sets the creation time.
    ///
    /// If `creation_time` is not `None`, this causes the
    /// `CertBuilder` to use that time when [`CertBuilder::generate`]
    /// is called.  If it is `None`, the default, then the current
    /// time minus 60 seconds is used as creation time.  Backdating
    /// the certificate by a minute has the advantage that the
    /// certificate can immediately be customized:
    ///
    /// In order to reliably override a binding signature, the
    /// overriding binding signature must be newer than the existing
    /// signature.  If, however, the existing signature is created
    /// `now`, any newer signature must have a future creation time,
    /// and is considered invalid by Sequoia.  To avoid this, we
    /// backdate certificate creation times (and hence binding
    /// signature creation times), so that there is "space" between
    /// the creation time and now for signature updates.
    ///
    /// Warning: this function takes a [`SystemTime`].  A `SystemTime`
    /// has a higher resolution, and a larger range than an OpenPGP
    /// [`Timestamp`].  Assuming the `creation_time` is in range, it
    /// will automatically be truncated to the nearest time that is
    /// representable by a `Timestamp`.  If it is not in range,
    /// [`generate`] will return an error.
    ///
    /// [`CertBuilder::generate`]: CertBuilder::generate()
    /// [`SystemTime`]: std::time::SystemTime
    /// [`Timestamp`]: crate::types::Timestamp
    /// [`generate`]: CertBuilder::generate()
    ///
    /// # Examples
    ///
    /// Generate a backdated certificate:
    ///
    /// ```
    /// use std::time::{SystemTime, Duration};
    /// use std::convert::TryFrom;
    ///
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::types::Timestamp;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// let t = SystemTime::now() - Duration::from_secs(365 * 24 * 60 * 60);
    /// // Roundtrip the time so that the assert below works.
    /// let t = SystemTime::from(Timestamp::try_from(t)?);
    ///
    /// let (cert, rev) =
    ///     CertBuilder::general_purpose(None,
    ///                                  Some("Alice Lovelace <alice@example.org>"))
    ///         .set_creation_time(t)
    ///         .generate()?;
    /// assert_eq!(cert.primary_key().self_signatures().nth(0).unwrap()
    ///            .signature_creation_time(),
    ///            Some(t));
    /// # Ok(())
    /// # }
    /// ```
    pub fn set_creation_time<T>(mut self, creation_time: T) -> Self
        where T: Into<Option<std::time::SystemTime>>,
    {
        self.creation_time = creation_time.into();
        self
    }

    /// Returns the configured creation time, if any.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::time::SystemTime;
    ///
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::cert::prelude::*;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// let mut builder = CertBuilder::new();
    /// assert!(builder.creation_time().is_none());
    ///
    /// let now = std::time::SystemTime::now();
    /// builder = builder.set_creation_time(Some(now));
    /// assert_eq!(builder.creation_time(), Some(now));
    ///
    /// builder = builder.set_creation_time(None);
    /// assert!(builder.creation_time().is_none());
    /// # Ok(())
    /// # }
    /// ```
    pub fn creation_time(&self) -> Option<std::time::SystemTime>
    {
        self.creation_time
    }

    /// Sets the default asymmetric algorithms.
    ///
    /// This method controls the set of algorithms that is used to
    /// generate the certificate's keys.
    ///
    /// # Examples
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::types::PublicKeyAlgorithm;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// let (ecc, _) =
    ///     CertBuilder::general_purpose(None, Some("alice@example.org"))
    ///         .set_cipher_suite(CipherSuite::Cv25519)
    ///         .generate()?;
    /// assert_eq!(ecc.primary_key().pk_algo(), PublicKeyAlgorithm::EdDSA);
    ///
    /// let (rsa, _) =
    ///     CertBuilder::general_purpose(None, Some("alice@example.org"))
    ///         .set_cipher_suite(CipherSuite::RSA2k)
    ///         .generate()?;
    /// assert_eq!(rsa.primary_key().pk_algo(), PublicKeyAlgorithm::RSAEncryptSign);
    /// # Ok(())
    /// # }
    /// ```
    pub fn set_cipher_suite(mut self, cs: CipherSuite) -> Self {
        self.ciphersuite = cs;
        self
    }

    /// Adds a User ID.
    ///
    /// Adds a User ID to the certificate.  The first User ID that is
    /// added, whether via this interface or another interface, e.g.,
    /// [`CertBuilder::general_purpose`], will have the [primary User
    /// ID flag] set.
    ///
    /// [`CertBuilder::general_purpose`]: CertBuilder::general_purpose()
    /// [primary User ID flag]: https://tools.ietf.org/html/rfc4880#section-5.2.3.19
    ///
    /// # Examples
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::packet::prelude::*;
    /// use openpgp::policy::StandardPolicy;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// let p = &StandardPolicy::new();
    ///
    /// let (cert, rev) =
    ///     CertBuilder::general_purpose(None,
    ///                                  Some("Alice Lovelace <alice@example.org>"))
    ///         .add_userid("Alice Lovelace <alice@lovelace.name>")
    ///         .generate()?;
    ///
    /// assert_eq!(cert.userids().count(), 2);
    /// let mut userids = cert.with_policy(p, None)?.userids().collect::<Vec<_>>();
    /// // Sort lexicographically.
    /// userids.sort_by(|a, b| a.value().cmp(b.value()));
    /// assert_eq!(userids[0].userid(),
    ///            &UserID::from("Alice Lovelace <alice@example.org>"));
    /// assert_eq!(userids[1].userid(),
    ///            &UserID::from("Alice Lovelace <alice@lovelace.name>"));
    ///
    ///
    /// assert_eq!(userids[0].binding_signature().primary_userid().unwrap_or(false), true);
    /// assert_eq!(userids[1].binding_signature().primary_userid().unwrap_or(false), false);
    /// # Ok(())
    /// # }
    /// ```
    pub fn add_userid<U>(mut self, uid: U) -> Self
        where U: Into<packet::UserID>
    {
        self.userids.push((None, uid.into()));
        self
    }

    /// Adds a User ID with a binding signature based on `builder`.
    ///
    /// Adds a User ID to the certificate, creating the binding
    /// signature using `builder`.  The `builder`s signature type must
    /// be a certification signature (i.e. either
    /// [`GenericCertification`], [`PersonaCertification`],
    /// [`CasualCertification`], or [`PositiveCertification`]).
    ///
    /// The key generation step uses `builder` as a template, but
    /// tweaks it so the signature is a valid binding signature.  If
    /// you need more control, consider using
    /// [`UserID::bind`](crate::packet::UserID::bind).
    ///
    /// The following modifications are performed on `builder`:
    ///
    ///   - An appropriate hash algorithm is selected.
    ///
    ///   - The creation time is set.
    ///
    ///   - Primary key metadata is added (key flags, key validity period).
    ///
    ///   - Certificate metadata is added (feature flags, algorithm
    ///     preferences).
    ///
    ///   - The [`CertBuilder`] marks exactly one User ID or User
    ///     Attribute as primary: The first one provided to
    ///     [`CertBuilder::add_userid_with`] or
    ///     [`CertBuilder::add_user_attribute_with`] (the UserID takes
    ///     precedence) that is marked as primary, or the first User
    ///     ID or User Attribute added to the [`CertBuilder`].
    ///
    ///   [`GenericCertification`]: crate::types::SignatureType::GenericCertification
    ///   [`PersonaCertification`]: crate::types::SignatureType::PersonaCertification
    ///   [`CasualCertification`]: crate::types::SignatureType::CasualCertification
    ///   [`PositiveCertification`]: crate::types::SignatureType::PositiveCertification
    ///   [primary User ID flag]: https://tools.ietf.org/html/rfc4880#section-5.2.3.19
    ///
    /// # Examples
    ///
    /// This example very casually binds a User ID to a certificate.
    ///
    /// ```
    /// # fn main() -> sequoia_openpgp::Result<()> {
    /// # use sequoia_openpgp as openpgp;
    /// # use openpgp::cert::prelude::*;
    /// # use openpgp::packet::{prelude::*, signature::subpacket::*};
    /// # use openpgp::policy::StandardPolicy;
    /// # use openpgp::types::*;
    /// # let policy = &StandardPolicy::new();
    /// #
    /// let (cert, revocation_cert) =
    ///     CertBuilder::general_purpose(
    ///         None, Some("Alice Lovelace <alice@example.org>"))
    ///     .add_userid_with(
    ///         "trinity",
    ///         SignatureBuilder::new(SignatureType::CasualCertification)
    ///             .set_notation("rabbit@example.org", b"follow me",
    ///                           NotationDataFlags::empty().set_human_readable(),
    ///                           false)?)?
    ///     .generate()?;
    ///
    /// assert_eq!(cert.userids().count(), 2);
    /// let mut userids = cert.with_policy(policy, None)?.userids().collect::<Vec<_>>();
    /// // Sort lexicographically.
    /// userids.sort_by(|a, b| a.value().cmp(b.value()));
    /// assert_eq!(userids[0].userid(),
    ///            &UserID::from("Alice Lovelace <alice@example.org>"));
    /// assert_eq!(userids[1].userid(),
    ///            &UserID::from("trinity"));
    ///
    /// assert!(userids[0].binding_signature().primary_userid().unwrap_or(false));
    /// assert!(! userids[1].binding_signature().primary_userid().unwrap_or(false));
    /// assert_eq!(userids[1].binding_signature().notation("rabbit@example.org")
    ///            .next().unwrap(), b"follow me");
    /// # Ok(()) }
    /// ```
    pub fn add_userid_with<U, B>(mut self, uid: U, builder: B)
                                 -> Result<Self>
    where U: Into<packet::UserID>,
          B: Into<SignatureBuilder>,
    {
        let builder = builder.into();
        match builder.typ() {
            SignatureType::GenericCertification
                | SignatureType::PersonaCertification
                | SignatureType::CasualCertification
                | SignatureType::PositiveCertification =>
            {
                self.userids.push((Some(builder), uid.into()));
                Ok(self)
            },
            t =>
                Err(Error::InvalidArgument(format!(
                    "Signature type is not a certification: {}", t)).into()),
        }
    }

    /// Adds a new User Attribute.
    ///
    /// Adds a User Attribute to the certificate.  If there are no
    /// User IDs, the first User attribute that is added, whether via
    /// this interface or another interface, will have the [primary
    /// User ID flag] set.
    ///
    /// [primary User ID flag]: https://tools.ietf.org/html/rfc4880#section-5.2.3.19
    ///
    /// # Examples
    ///
    /// When there are no User IDs, the first User Attribute has the
    /// primary User ID flag set:
    ///
    /// ```
    /// # use openpgp::packet::user_attribute::Subpacket;
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::packet::prelude::*;
    /// use openpgp::policy::StandardPolicy;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// let p = &StandardPolicy::new();
    /// #
    /// # // Create some user attribute. Doctests do not pass cfg(test),
    /// # // so UserAttribute::arbitrary is not available
    /// # let sp = Subpacket::Unknown(7, vec![7; 7].into_boxed_slice());
    /// # let user_attribute = UserAttribute::new(&[sp])?;
    ///
    /// let (cert, rev) =
    ///     CertBuilder::new()
    ///         .add_user_attribute(user_attribute)
    ///         .generate()?;
    ///
    /// assert_eq!(cert.userids().count(), 0);
    /// assert_eq!(cert.user_attributes().count(), 1);
    /// let mut uas = cert.with_policy(p, None)?.user_attributes().collect::<Vec<_>>();
    /// assert_eq!(uas[0].binding_signature().primary_userid().unwrap_or(false), true);
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// Where there are User IDs, then the primary User ID flag is not
    /// set:
    ///
    /// ```
    /// # use openpgp::packet::user_attribute::Subpacket;
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::packet::prelude::*;
    /// use openpgp::policy::StandardPolicy;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// let p = &StandardPolicy::new();
    /// #
    /// # // Create some user attribute. Doctests do not pass cfg(test),
    /// # // so UserAttribute::arbitrary is not available
    /// # let sp = Subpacket::Unknown(7, vec![7; 7].into_boxed_slice());
    /// # let user_attribute = UserAttribute::new(&[sp])?;
    ///
    /// let (cert, rev) =
    ///     CertBuilder::new()
    ///         .add_userid("alice@example.org")
    ///         .add_user_attribute(user_attribute)
    ///         .generate()?;
    ///
    /// assert_eq!(cert.userids().count(), 1);
    /// assert_eq!(cert.user_attributes().count(), 1);
    /// let mut uas = cert.with_policy(p, None)?.user_attributes().collect::<Vec<_>>();
    /// assert_eq!(uas[0].binding_signature().primary_userid().unwrap_or(false), false);
    /// # Ok(())
    /// # }
    /// ```
    pub fn add_user_attribute<U>(mut self, ua: U) -> Self
        where U: Into<packet::UserAttribute>
    {
        self.user_attributes.push((None, ua.into()));
        self
    }

    /// Adds a User Attribute with a binding signature based on `builder`.
    ///
    /// Adds a User Attribute to the certificate, creating the binding
    /// signature using `builder`.  The `builder`s signature type must
    /// be a certification signature (i.e. either
    /// [`GenericCertification`], [`PersonaCertification`],
    /// [`CasualCertification`], or [`PositiveCertification`]).
    ///
    /// The key generation step uses `builder` as a template, but
    /// tweaks it so the signature is a valid binding signature.  If
    /// you need more control, consider using
    /// [`UserAttribute::bind`](crate::packet::UserAttribute::bind).
    ///
    /// The following modifications are performed on `builder`:
    ///
    ///   - An appropriate hash algorithm is selected.
    ///
    ///   - The creation time is set.
    ///
    ///   - Primary key metadata is added (key flags, key validity period).
    ///
    ///   - Certificate metadata is added (feature flags, algorithm
    ///     preferences).
    ///
    ///   - The [`CertBuilder`] marks exactly one User ID or User
    ///     Attribute as primary: The first one provided to
    ///     [`CertBuilder::add_userid_with`] or
    ///     [`CertBuilder::add_user_attribute_with`] (the UserID takes
    ///     precedence) that is marked as primary, or the first User
    ///     ID or User Attribute added to the [`CertBuilder`].
    ///
    ///   [`GenericCertification`]: crate::types::SignatureType::GenericCertification
    ///   [`PersonaCertification`]: crate::types::SignatureType::PersonaCertification
    ///   [`CasualCertification`]: crate::types::SignatureType::CasualCertification
    ///   [`PositiveCertification`]: crate::types::SignatureType::PositiveCertification
    ///   [primary User ID flag]: https://tools.ietf.org/html/rfc4880#section-5.2.3.19
    ///
    /// # Examples
    ///
    /// This example very casually binds a user attribute to a
    /// certificate.
    ///
    /// ```
    /// # fn main() -> sequoia_openpgp::Result<()> {
    /// # use sequoia_openpgp as openpgp;
    /// # use openpgp::packet::user_attribute::Subpacket;
    /// # use openpgp::cert::prelude::*;
    /// # use openpgp::packet::{prelude::*, signature::subpacket::*};
    /// # use openpgp::policy::StandardPolicy;
    /// # use openpgp::types::*;
    /// #
    /// # let policy = &StandardPolicy::new();
    /// #
    /// # // Create some user attribute. Doctests do not pass cfg(test),
    /// # // so UserAttribute::arbitrary is not available
    /// # let user_attribute =
    /// #   UserAttribute::new(&[Subpacket::Unknown(7, vec![7; 7].into())])?;
    /// let (cert, revocation_cert) =
    ///     CertBuilder::general_purpose(
    ///         None, Some("Alice Lovelace <alice@example.org>"))
    ///     .add_user_attribute_with(
    ///         user_attribute,
    ///         SignatureBuilder::new(SignatureType::CasualCertification)
    ///             .set_notation("rabbit@example.org", b"follow me",
    ///                           NotationDataFlags::empty().set_human_readable(),
    ///                           false)?)?
    ///     .generate()?;
    ///
    /// let uas = cert.with_policy(policy, None)?.user_attributes().collect::<Vec<_>>();
    /// assert_eq!(uas.len(), 1);
    /// assert!(! uas[0].binding_signature().primary_userid().unwrap_or(false));
    /// assert_eq!(uas[0].binding_signature().notation("rabbit@example.org")
    ///            .next().unwrap(), b"follow me");
    /// # Ok(()) }
    /// ```
    pub fn add_user_attribute_with<U, B>(mut self, ua: U, builder: B)
                                         -> Result<Self>
    where U: Into<packet::UserAttribute>,
          B: Into<SignatureBuilder>,
    {
        let builder = builder.into();
        match builder.typ() {
            SignatureType::GenericCertification
                | SignatureType::PersonaCertification
                | SignatureType::CasualCertification
                | SignatureType::PositiveCertification =>
            {
                self.user_attributes.push((Some(builder), ua.into()));
                Ok(self)
            },
            t =>
                Err(Error::InvalidArgument(format!(
                    "Signature type is not a certification: {}", t)).into()),
        }
    }

    /// Adds a signing-capable subkey.
    ///
    /// The key uses the default cipher suite (see
    /// [`CertBuilder::set_cipher_suite`]), and is not set to expire.
    /// Use [`CertBuilder::add_subkey`] if you need to change these
    /// parameters.
    ///
    /// [`CertBuilder::set_cipher_suite`]: CertBuilder::set_cipher_suite()
    /// [`CertBuilder::add_subkey`]: CertBuilder::add_subkey()
    ///
    /// # Examples
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::policy::StandardPolicy;
    /// use openpgp::types::KeyFlags;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// let p = &StandardPolicy::new();
    ///
    /// let (cert, rev) =
    ///     CertBuilder::new()
    ///         .add_signing_subkey()
    ///         .generate()?;
    ///
    /// assert_eq!(cert.keys().count(), 2);
    /// let ka = cert.with_policy(p, None)?.keys().nth(1).unwrap();
    /// assert_eq!(ka.key_flags(),
    ///            Some(KeyFlags::empty().set_signing()));
    /// # Ok(())
    /// # }
    /// ```
    pub fn add_signing_subkey(self) -> Self {
        self.add_subkey(KeyFlags::empty().set_signing(), None, None)
    }

    /// Adds a subkey suitable for transport encryption.
    ///
    /// The key uses the default cipher suite (see
    /// [`CertBuilder::set_cipher_suite`]), and is not set to expire.
    /// Use [`CertBuilder::add_subkey`] if you need to change these
    /// parameters.
    ///
    /// [`CertBuilder::set_cipher_suite`]: CertBuilder::set_cipher_suite()
    /// [`CertBuilder::add_subkey`]: CertBuilder::add_subkey()
    ///
    ///
    /// # Examples
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::policy::StandardPolicy;
    /// use openpgp::types::KeyFlags;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// let p = &StandardPolicy::new();
    ///
    /// let (cert, rev) =
    ///     CertBuilder::new()
    ///         .add_transport_encryption_subkey()
    ///         .generate()?;
    ///
    /// assert_eq!(cert.keys().count(), 2);
    /// let ka = cert.with_policy(p, None)?.keys().nth(1).unwrap();
    /// assert_eq!(ka.key_flags(),
    ///            Some(KeyFlags::empty().set_transport_encryption()));
    /// # Ok(())
    /// # }
    /// ```
    pub fn add_transport_encryption_subkey(self) -> Self {
        self.add_subkey(KeyFlags::empty().set_transport_encryption(),
                        None, None)
    }

    /// Adds a subkey suitable for storage encryption.
    ///
    /// The key uses the default cipher suite (see
    /// [`CertBuilder::set_cipher_suite`]), and is not set to expire.
    /// Use [`CertBuilder::add_subkey`] if you need to change these
    /// parameters.
    ///
    /// [`CertBuilder::set_cipher_suite`]: CertBuilder::set_cipher_suite()
    /// [`CertBuilder::add_subkey`]: CertBuilder::add_subkey()
    ///
    ///
    /// # Examples
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::policy::StandardPolicy;
    /// use openpgp::types::KeyFlags;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// let p = &StandardPolicy::new();
    ///
    /// let (cert, rev) =
    ///     CertBuilder::new()
    ///         .add_storage_encryption_subkey()
    ///         .generate()?;
    ///
    /// assert_eq!(cert.keys().count(), 2);
    /// let ka = cert.with_policy(p, None)?.keys().nth(1).unwrap();
    /// assert_eq!(ka.key_flags(),
    ///            Some(KeyFlags::empty().set_storage_encryption()));
    /// # Ok(())
    /// # }
    /// ```
    pub fn add_storage_encryption_subkey(self) -> Self {
        self.add_subkey(KeyFlags::empty().set_storage_encryption(),
                        None, None)
    }

    /// Adds an certification-capable subkey.
    ///
    /// The key uses the default cipher suite (see
    /// [`CertBuilder::set_cipher_suite`]), and is not set to expire.
    /// Use [`CertBuilder::add_subkey`] if you need to change these
    /// parameters.
    ///
    /// [`CertBuilder::set_cipher_suite`]: CertBuilder::set_cipher_suite()
    /// [`CertBuilder::add_subkey`]: CertBuilder::add_subkey()
    ///
    ///
    /// # Examples
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::policy::StandardPolicy;
    /// use openpgp::types::KeyFlags;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// let p = &StandardPolicy::new();
    ///
    /// let (cert, rev) =
    ///     CertBuilder::new()
    ///         .add_certification_subkey()
    ///         .generate()?;
    ///
    /// assert_eq!(cert.keys().count(), 2);
    /// let ka = cert.with_policy(p, None)?.keys().nth(1).unwrap();
    /// assert_eq!(ka.key_flags(),
    ///            Some(KeyFlags::empty().set_certification()));
    /// # Ok(())
    /// # }
    /// ```
    pub fn add_certification_subkey(self) -> Self {
        self.add_subkey(KeyFlags::empty().set_certification(), None, None)
    }

    /// Adds an authentication-capable subkey.
    ///
    /// The key uses the default cipher suite (see
    /// [`CertBuilder::set_cipher_suite`]), and is not set to expire.
    /// Use [`CertBuilder::add_subkey`] if you need to change these
    /// parameters.
    ///
    /// [`CertBuilder::set_cipher_suite`]: CertBuilder::set_cipher_suite()
    /// [`CertBuilder::add_subkey`]: CertBuilder::add_subkey()
    ///
    ///
    /// # Examples
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::policy::StandardPolicy;
    /// use openpgp::types::KeyFlags;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// let p = &StandardPolicy::new();
    ///
    /// let (cert, rev) =
    ///     CertBuilder::new()
    ///         .add_authentication_subkey()
    ///         .generate()?;
    ///
    /// assert_eq!(cert.keys().count(), 2);
    /// let ka = cert.with_policy(p, None)?.keys().nth(1).unwrap();
    /// assert_eq!(ka.key_flags(),
    ///            Some(KeyFlags::empty().set_authentication()));
    /// # Ok(())
    /// # }
    /// ```
    pub fn add_authentication_subkey(self) -> Self {
        self.add_subkey(KeyFlags::empty().set_authentication(), None, None)
    }

    /// Adds a custom subkey.
    ///
    /// If `validity` is `None`, the subkey will be valid for the same
    /// period as the primary key.
    ///
    /// Likewise, if `cs` is `None`, the same cipher suite is used as
    /// for the primary key.
    ///
    /// # Examples
    ///
    /// Generates a certificate with an encryption subkey that is for
    /// protecting *both* data in transit and data at rest, and
    /// expires at a different time from the primary key:
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// # use openpgp::Result;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::policy::StandardPolicy;
    /// use openpgp::types::KeyFlags;
    ///
    /// # fn main() -> Result<()> {
    /// let p = &StandardPolicy::new();
    ///
    /// let now = std::time::SystemTime::now();
    /// let y = std::time::Duration::new(365 * 24 * 60 * 60, 0);
    ///
    /// // Make the certificate expire in 2 years, and the subkey
    /// // expire in a year.
    /// let (cert,_) = CertBuilder::new()
    ///     .set_creation_time(now)
    ///     .set_validity_period(2 * y)
    ///     .add_subkey(KeyFlags::empty()
    ///                     .set_storage_encryption()
    ///                     .set_transport_encryption(),
    ///                 y,
    ///                 None)
    ///     .generate()?;
    ///
    /// assert_eq!(cert.with_policy(p, now)?.keys().alive().count(), 2);
    /// assert_eq!(cert.with_policy(p, now + y)?.keys().alive().count(), 1);
    /// assert_eq!(cert.with_policy(p, now + 2 * y)?.keys().alive().count(), 0);
    ///
    /// let ka = cert.with_policy(p, None)?.keys().nth(1).unwrap();
    /// assert_eq!(ka.key_flags(),
    ///            Some(KeyFlags::empty()
    ///                     .set_storage_encryption()
    ///                     .set_transport_encryption()));
    /// # Ok(()) }
    /// ```
    pub fn add_subkey<T, C>(mut self, flags: KeyFlags, validity: T, cs: C)
        -> Self
        where T: Into<Option<time::Duration>>,
              C: Into<Option<CipherSuite>>,
    {
        self.subkeys.push((None, KeyBlueprint {
            flags,
            validity: validity.into(),
            ciphersuite: cs.into(),
        }));
        self
    }

    /// Adds a subkey with a binding signature based on `builder`.
    ///
    /// Adds a subkey to the certificate, creating the binding
    /// signature using `builder`.  The `builder`s signature type must
    /// be [`SubkeyBinding`].
    ///
    /// The key generation step uses `builder` as a template, but adds
    /// all subpackets that the signature needs to be a valid binding
    /// signature.  If you need more control, or want to adopt
    /// existing keys, consider using
    /// [`Key::bind`](crate::packet::Key::bind).
    ///
    /// The following modifications are performed on `builder`:
    ///
    ///   - An appropriate hash algorithm is selected.
    ///
    ///   - The creation time is set.
    ///
    ///   - Key metadata is added (key flags, key validity period).
    ///
    ///   [`SubkeyBinding`]: crate::types::SignatureType::SubkeyBinding
    ///
    /// If `validity` is `None`, the subkey will be valid for the same
    /// period as the primary key.
    ///
    /// # Examples
    ///
    /// This example binds a signing subkey to a certificate,
    /// restricting its use to authentication of software.
    ///
    /// ```
    /// # fn main() -> sequoia_openpgp::Result<()> {
    /// # use sequoia_openpgp as openpgp;
    /// # use openpgp::packet::user_attribute::Subpacket;
    /// # use openpgp::cert::prelude::*;
    /// # use openpgp::packet::{prelude::*, signature::subpacket::*};
    /// # use openpgp::policy::StandardPolicy;
    /// # use openpgp::types::*;
    /// let (cert, revocation_cert) =
    ///     CertBuilder::general_purpose(
    ///         None, Some("Alice Lovelace <alice@example.org>"))
    ///     .add_subkey_with(
    ///         KeyFlags::empty().set_signing(), None, None,
    ///         SignatureBuilder::new(SignatureType::SubkeyBinding)
    ///             // Add a critical notation!
    ///             .set_notation("code-signing@policy.example.org", b"",
    ///                           NotationDataFlags::empty(), true)?)?
    ///     .generate()?;
    ///
    /// // Under the standard policy, the additional signing subkey
    /// // is not bound.
    /// let p = StandardPolicy::new();
    /// assert_eq!(cert.with_policy(&p, None)?.keys().for_signing().count(), 1);
    ///
    /// // However, software implementing the notation see the additional
    /// // signing subkey.
    /// let mut p = StandardPolicy::new();
    /// p.good_critical_notations(&["code-signing@policy.example.org"]);
    /// assert_eq!(cert.with_policy(&p, None)?.keys().for_signing().count(), 2);
    /// # Ok(()) }
    /// ```
    pub fn add_subkey_with<T, C, B>(mut self, flags: KeyFlags, validity: T,
                                    cs: C, builder: B) -> Result<Self>
        where T: Into<Option<time::Duration>>,
              C: Into<Option<CipherSuite>>,
              B: Into<SignatureBuilder>,
    {
        let builder = builder.into();
        match builder.typ() {
            SignatureType::SubkeyBinding => {
                self.subkeys.push((Some(builder), KeyBlueprint {
                    flags,
                    validity: validity.into(),
                    ciphersuite: cs.into(),
                }));
                Ok(self)
            },
            t =>
                Err(Error::InvalidArgument(format!(
                    "Signature type is not a subkey binding: {}", t)).into()),
        }
    }

    /// Sets the primary key's key flags.
    ///
    /// By default, the primary key is set to only be certification
    /// capable.  This allows the caller to set additional flags.
    ///
    /// # Examples
    ///
    /// Makes the primary key signing-capable but not
    /// certification-capable.
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// # use openpgp::Result;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::policy::StandardPolicy;
    /// use openpgp::types::KeyFlags;
    ///
    /// # fn main() -> Result<()> {
    /// let p = &StandardPolicy::new();
    ///
    /// let (cert, rev) =
    ///     CertBuilder::general_purpose(None,
    ///                                  Some("Alice Lovelace <alice@example.org>"))
    ///         .set_primary_key_flags(KeyFlags::empty().set_signing())
    ///         .generate()?;
    ///
    /// // Observe that the primary key's certification capability is
    /// // set implicitly.
    /// assert_eq!(cert.with_policy(p, None)?.primary_key().key_flags(),
    ///            Some(KeyFlags::empty().set_signing()));
    /// # Ok(()) }
    /// ```
    pub fn set_primary_key_flags(mut self, flags: KeyFlags) -> Self {
        self.primary.flags = flags;
        self
    }

    /// Sets a password to encrypt the secret keys with.
    ///
    /// The password is used to encrypt all secret key material.
    ///
    /// # Examples
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// # use openpgp::Result;
    /// use openpgp::cert::prelude::*;
    ///
    /// # fn main() -> Result<()> {
    /// // Make the certificate expire in 10 minutes.
    /// let (cert, rev) =
    ///     CertBuilder::general_purpose(None,
    ///                                  Some("Alice Lovelace <alice@example.org>"))
    ///         .set_password(Some("1234".into()))
    ///         .generate()?;
    ///
    /// for ka in cert.keys() {
    ///     assert!(ka.has_secret());
    /// }
    /// # Ok(()) }
    /// ```
    pub fn set_password(mut self, password: Option<Password>) -> Self {
        self.password = password;
        self
    }

    /// Sets the certificate's validity period.
    ///
    /// The determines how long the certificate is valid.  That is,
    /// after the validity period, the certificate is considered to be
    /// expired.
    ///
    /// The validity period starts with the creation time (see
    /// [`CertBuilder::set_creation_time`]).
    ///
    /// A value of `None` means that the certificate never expires.
    ///
    /// See [this section](CertBuilder#expiration) of the type's
    /// documentation for security considerations of key expiration.
    ///
    /// # Examples
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// # use openpgp::Result;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::policy::StandardPolicy;
    /// use openpgp::types::RevocationKey;
    ///
    /// # fn main() -> Result<()> {
    /// let p = &StandardPolicy::new();
    ///
    /// let now = std::time::SystemTime::now();
    /// let s = std::time::Duration::new(1, 0);
    ///
    /// // Make the certificate expire in 10 minutes.
    /// let (cert,_) = CertBuilder::new()
    ///     .set_creation_time(now)
    ///     .set_validity_period(600 * s)
    ///     .generate()?;
    ///
    /// assert!(cert.with_policy(p, now)?.primary_key().alive().is_ok());
    /// assert!(cert.with_policy(p, now + 599 * s)?.primary_key().alive().is_ok());
    /// assert!(cert.with_policy(p, now + 600 * s)?.primary_key().alive().is_err());
    /// # Ok(()) }
    /// ```
    pub fn set_validity_period<T>(mut self, validity: T) -> Self
        where T: Into<Option<time::Duration>>
    {
        self.primary.validity = validity.into();
        self
    }

    /// Sets designated revokers.
    ///
    /// Adds designated revokers to the primary key.  This allows the
    /// designated revoker to issue revocation certificates on behalf
    /// of the primary key.
    ///
    /// # Examples
    ///
    /// Make Alice a designated revoker for Bob:
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// # use openpgp::Result;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::policy::StandardPolicy;
    /// use openpgp::types::RevocationKey;
    ///
    /// # fn main() -> Result<()> {
    /// let p = &StandardPolicy::new();
    ///
    /// let (alice, _) =
    ///     CertBuilder::general_purpose(None, Some("alice@example.org"))
    ///         .generate()?;
    /// let (bob, _) =
    ///     CertBuilder::general_purpose(None, Some("bob@example.org"))
    ///         .set_revocation_keys(vec![(&alice).into()])
    ///         .generate()?;
    ///
    /// // Make sure Alice is listed as a designated revoker for Bob.
    /// assert_eq!(bob.revocation_keys(p).collect::<Vec<&RevocationKey>>(),
    ///            vec![&(&alice).into()]);
    /// # Ok(()) }
    /// ```
    pub fn set_revocation_keys(mut self, revocation_keys: Vec<RevocationKey>)
        -> Self
    {
        self.revocation_keys = Some(revocation_keys);
        self
    }

    /// Generates a certificate.
    ///
    /// # Examples
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// # use openpgp::Result;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::policy::StandardPolicy;
    /// use openpgp::types::RevocationKey;
    ///
    /// # fn main() -> Result<()> {
    /// let p = &StandardPolicy::new();
    ///
    /// let (alice, _) =
    ///     CertBuilder::general_purpose(None, Some("alice@example.org"))
    ///         .generate()?;
    /// # Ok(()) }
    /// ```
    pub fn generate(self) -> Result<(Cert, Signature)> {
        use crate::Packet;
        use crate::types::ReasonForRevocation;
        use std::convert::TryFrom;

        let creation_time =
            self.creation_time.unwrap_or_else(|| {
                use crate::packet::signature::SIG_BACKDATE_BY;
                crate::now() -
                    time::Duration::new(SIG_BACKDATE_BY, 0)
            });

        // Generate & self-sign primary key.
        let (primary, sig, mut signer) = self.primary_key(creation_time)?;

        let mut cert = Cert::try_from(vec![
            Packet::SecretKey({
                let mut primary = primary.clone();
                if let Some(ref password) = self.password {
                    primary.secret_mut().encrypt_in_place(password)?;
                }
                primary
            }),
            sig.into(),
        ])?;

        // We want to mark exactly one User ID or Attribute as primary.
        // First, figure out whether one of the binding signature
        // templates have the primary flag set.
        let have_primary_user_thing = {
            let is_primary = |osig: &Option<SignatureBuilder>| -> bool {
                osig.as_ref().and_then(|s| s.primary_userid()).unwrap_or(false)
            };

            self.userids.iter().map(|(s, _)| s).any(is_primary)
                || self.user_attributes.iter().map(|(s, _)| s).any(is_primary)
        };
        let mut emitted_primary_user_thing = false;

        // Sign UserIDs.
        for (template, uid) in self.userids.into_iter() {
            let sig = template.unwrap_or_else(
                || SignatureBuilder::new(SignatureType::PositiveCertification));
            let sig = Self::signature_common(sig, creation_time)?;
            let mut sig = Self::add_primary_key_metadata(sig, &self.primary)?;

            // Make sure we mark exactly one User ID or Attribute as
            // primary.
            if emitted_primary_user_thing {
                sig = sig.modify_hashed_area(|mut a| {
                    a.remove_all(SubpacketTag::PrimaryUserID);
                    Ok(a)
                })?;
            } else if have_primary_user_thing {
                // Check if this is the first explicitly selected
                // user thing.
                emitted_primary_user_thing |=
                    sig.primary_userid().unwrap_or(false);
            } else {
                // Implicitly mark the first as primary.
                sig = sig.set_primary_userid(true)?;
                emitted_primary_user_thing = true;
            }

            let signature = uid.bind(&mut signer, &cert, sig)?;
            cert = cert.insert_packets(
                vec![Packet::from(uid), signature.into()])?;
        }

        // Sign UserAttributes.
        for (template, ua) in self.user_attributes.into_iter() {
            let sig = template.unwrap_or_else(
                || SignatureBuilder::new(SignatureType::PositiveCertification));
            let sig = Self::signature_common(sig, creation_time)?;
            let mut sig = Self::add_primary_key_metadata(sig, &self.primary)?;

            // Make sure we mark exactly one User ID or Attribute as
            // primary.
            if emitted_primary_user_thing {
                sig = sig.modify_hashed_area(|mut a| {
                    a.remove_all(SubpacketTag::PrimaryUserID);
                    Ok(a)
                })?;
            } else if have_primary_user_thing {
                // Check if this is the first explicitly selected
                // user thing.
                emitted_primary_user_thing |=
                    sig.primary_userid().unwrap_or(false);
            } else {
                // Implicitly mark the first as primary.
                sig = sig.set_primary_userid(true)?;
                emitted_primary_user_thing = true;
            }

            let signature = ua.bind(&mut signer, &cert, sig)?;
            cert = cert.insert_packets(
                vec![Packet::from(ua), signature.into()])?;
        }

        // Sign subkeys.
        for (template, blueprint) in self.subkeys {
            let flags = &blueprint.flags;
            let mut subkey = blueprint.ciphersuite
                .unwrap_or(self.ciphersuite)
                .generate_key(flags)?;
            subkey.set_creation_time(creation_time)?;

            let sig = template.unwrap_or_else(
                || SignatureBuilder::new(SignatureType::SubkeyBinding));
            let sig = Self::signature_common(sig, creation_time)?;
            let mut builder = sig
                .set_key_flags(flags.clone())?
                .set_key_validity_period(blueprint.validity.or(self.primary.validity))?;

            if flags.for_certification() || flags.for_signing() {
                // We need to create a primary key binding signature.
                let mut subkey_signer = subkey.clone().into_keypair().unwrap();
                let backsig =
                    signature::SignatureBuilder::new(SignatureType::PrimaryKeyBinding)
                    .set_signature_creation_time(creation_time)?
                    // GnuPG wants at least a 512-bit hash for P521 keys.
                    .set_hash_algo(HashAlgorithm::SHA512)
                    .sign_primary_key_binding(&mut subkey_signer, &primary,
                                              &subkey)?;
                builder = builder.set_embedded_signature(backsig)?;
            }

            let signature = subkey.bind(&mut signer, &cert, builder)?;

            if let Some(ref password) = self.password {
                subkey.secret_mut().encrypt_in_place(password)?;
            }
            cert = cert.insert_packets(vec![Packet::SecretSubkey(subkey),
                                           signature.into()])?;
        }

        let revocation = CertRevocationBuilder::new()
            .set_signature_creation_time(creation_time)?
            .set_reason_for_revocation(
                ReasonForRevocation::Unspecified, b"Unspecified")?
            .build(&mut signer, &cert, None)?;

        // keys generated by the builder are never invalid
        assert!(cert.bad.is_empty());
        assert!(cert.unknowns.is_empty());

        Ok((cert, revocation))
    }

    /// Creates the primary key and a direct key signature.
    fn primary_key(&self, creation_time: std::time::SystemTime)
        -> Result<(KeySecretKey, Signature, Box<dyn Signer>)>
    {
        let mut key = self.primary.ciphersuite
            .unwrap_or(self.ciphersuite)
            .generate_key(KeyFlags::empty().set_certification())?;
        key.set_creation_time(creation_time)?;
        let sig = SignatureBuilder::new(SignatureType::DirectKey);
        let sig = Self::signature_common(sig, creation_time)?;
        let mut sig = Self::add_primary_key_metadata(sig, &self.primary)?;

        if let Some(ref revocation_keys) = self.revocation_keys {
            sig = sig.set_revocation_key(revocation_keys.clone())?;
        }

        let mut signer = key.clone().into_keypair()
            .expect("key generated above has a secret");
        let sig = sig.sign_direct_key(&mut signer, key.parts_as_public())?;

        Ok((key, sig, Box::new(signer)))
    }

    /// Common settings for generated signatures.
    fn signature_common(builder: SignatureBuilder,
                        creation_time: time::SystemTime)
                        -> Result<SignatureBuilder>
    {
        builder
            // GnuPG wants at least a 512-bit hash for P521 keys.
            .set_hash_algo(HashAlgorithm::SHA512)
            .set_signature_creation_time(creation_time)
    }


    /// Adds primary key metadata to the signature.
    fn add_primary_key_metadata(builder: SignatureBuilder,
                                primary: &KeyBlueprint)
                                -> Result<SignatureBuilder>
    {
        builder
            .set_features(Features::sequoia())?
            .set_key_flags(primary.flags.clone())?
            .set_key_validity_period(primary.validity)?
            .set_preferred_hash_algorithms(vec![
                HashAlgorithm::SHA512,
                HashAlgorithm::SHA256,
            ])?
            .set_preferred_symmetric_algorithms(vec![
                SymmetricAlgorithm::AES256,
                SymmetricAlgorithm::AES128,
            ])
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;
    use crate::Fingerprint;
    use crate::packet::signature::subpacket::{SubpacketTag, SubpacketValue};
    use crate::types::PublicKeyAlgorithm;
    use crate::policy::StandardPolicy as P;

    #[test]
    fn all_opts() {
        let p = &P::new();

        let (cert, _) = CertBuilder::new()
            .set_cipher_suite(CipherSuite::Cv25519)
            .add_userid("test1@example.com")
            .add_userid("test2@example.com")
            .add_signing_subkey()
            .add_transport_encryption_subkey()
            .add_certification_subkey()
            .generate().unwrap();

        let mut userids = cert.userids().with_policy(p, None)
            .map(|u| String::from_utf8_lossy(u.userid().value()).into_owned())
            .collect::<Vec<String>>();
        userids.sort();

        assert_eq!(userids,
                   &[ "test1@example.com",
                      "test2@example.com",
                   ][..]);
        assert_eq!(cert.subkeys().count(), 3);
    }

    #[test]
    fn direct_key_sig() {
        let p = &P::new();

        let (cert, _) = CertBuilder::new()
            .set_cipher_suite(CipherSuite::Cv25519)
            .add_signing_subkey()
            .add_transport_encryption_subkey()
            .add_certification_subkey()
            .generate().unwrap();

        assert_eq!(cert.userids().count(), 0);
        assert_eq!(cert.subkeys().count(), 3);
        let sig =
            cert.primary_key().with_policy(p, None).unwrap().binding_signature();
        assert_eq!(sig.typ(), crate::types::SignatureType::DirectKey);
        assert!(sig.features().unwrap().supports_mdc());
    }

    #[test]
    fn setter() {
        let (cert1, _) = CertBuilder::new()
            .set_cipher_suite(CipherSuite::Cv25519)
            .set_cipher_suite(CipherSuite::RSA3k)
            .set_cipher_suite(CipherSuite::Cv25519)
            .generate().unwrap();
        assert_eq!(cert1.primary_key().pk_algo(), PublicKeyAlgorithm::EdDSA);

        let (cert2, _) = CertBuilder::new()
            .set_cipher_suite(CipherSuite::RSA3k)
            .add_userid("test2@example.com")
            .add_transport_encryption_subkey()
            .generate().unwrap();
        assert_eq!(cert2.primary_key().pk_algo(),
                   PublicKeyAlgorithm::RSAEncryptSign);
        assert_eq!(cert2.subkeys().next().unwrap().key().pk_algo(),
                   PublicKeyAlgorithm::RSAEncryptSign);
    }

    #[test]
    fn defaults() {
        let p = &P::new();
        let (cert1, _) = CertBuilder::new()
            .add_userid("test2@example.com")
            .generate().unwrap();
        assert_eq!(cert1.primary_key().pk_algo(),
                   PublicKeyAlgorithm::EdDSA);
        assert!(cert1.subkeys().next().is_none());
        assert!(cert1.with_policy(p, None).unwrap().primary_userid().unwrap()
                .binding_signature().features().unwrap().supports_mdc());
    }

    #[test]
    fn not_always_certify() {
        let p = &P::new();
        let (cert1, _) = CertBuilder::new()
            .set_cipher_suite(CipherSuite::Cv25519)
            .set_primary_key_flags(KeyFlags::empty())
            .add_transport_encryption_subkey()
            .generate().unwrap();
        assert!(! cert1.primary_key().with_policy(p, None).unwrap().for_certification());
        assert_eq!(cert1.keys().subkeys().count(), 1);
    }

    #[test]
    fn gen_wired_subkeys() {
        let (cert1, _) = CertBuilder::new()
            .set_cipher_suite(CipherSuite::Cv25519)
            .set_primary_key_flags(KeyFlags::empty())
            .add_subkey(KeyFlags::empty().set_certification(), None, None)
            .generate().unwrap();
        let sig_pkts = cert1.subkeys().next().unwrap().bundle().self_signatures[0].hashed_area();

        match sig_pkts.subpacket(SubpacketTag::KeyFlags).unwrap().value() {
            SubpacketValue::KeyFlags(ref ks) => assert!(ks.for_certification()),
            v => panic!("Unexpected subpacket: {:?}", v),
        }

        assert_eq!(cert1.subkeys().count(), 1);
    }

    #[test]
    fn generate_revocation_certificate() {
        let p = &P::new();
        use crate::types::RevocationStatus;
        let (cert, revocation) = CertBuilder::new()
            .set_cipher_suite(CipherSuite::Cv25519)
            .generate().unwrap();
        assert_eq!(cert.revocation_status(p, None),
                   RevocationStatus::NotAsFarAsWeKnow);

        let cert = cert.insert_packets(revocation.clone()).unwrap();
        assert_eq!(cert.revocation_status(p, None),
                   RevocationStatus::Revoked(vec![ &revocation ]));
    }

    #[test]
    fn builder_roundtrip() {
        use std::convert::TryFrom;

        let (cert,_) = CertBuilder::new()
            .set_cipher_suite(CipherSuite::Cv25519)
            .add_signing_subkey()
            .generate().unwrap();
        let pile = cert.clone().into_packet_pile().into_children().collect::<Vec<_>>();
        let exp = Cert::try_from(pile).unwrap();

        assert_eq!(cert, exp);
    }

    #[test]
    fn encrypted_secrets() {
        let (cert,_) = CertBuilder::new()
            .set_cipher_suite(CipherSuite::Cv25519)
            .set_password(Some(String::from("streng geheim").into()))
            .generate().unwrap();
        assert!(cert.primary_key().optional_secret().unwrap().is_encrypted());
    }

    #[test]
    fn all_ciphersuites() {
        use self::CipherSuite::*;

        for cs in vec![Cv25519, RSA3k, P256, P384, P521, RSA2k, RSA4k]
            .into_iter().filter(|cs| cs.is_supported().is_ok())
        {
            assert!(CertBuilder::new()
                .set_cipher_suite(cs)
                .generate().is_ok());
        }
    }

    #[test]
    fn validity_periods() {
        let p = &P::new();

        let now = crate::now();
        let s = std::time::Duration::new(1, 0);

        let (cert,_) = CertBuilder::new()
            .set_creation_time(now)
            .set_validity_period(600 * s)
            .add_subkey(KeyFlags::empty().set_signing(),
                        300 * s, None)
            .add_subkey(KeyFlags::empty().set_authentication(),
                        None, None)
            .generate().unwrap();

        let key = cert.primary_key().key();
        let sig = &cert.primary_key().bundle().self_signatures()[0];
        assert!(sig.key_alive(key, now).is_ok());
        assert!(sig.key_alive(key, now + 590 * s).is_ok());
        assert!(! sig.key_alive(key, now + 610 * s).is_ok());

        let ka = cert.keys().with_policy(p, now).alive().revoked(false)
            .for_signing().next().unwrap();
        assert!(ka.alive().is_ok());
        assert!(ka.clone().with_policy(p, now + 290 * s).unwrap().alive().is_ok());
        assert!(! ka.clone().with_policy(p, now + 310 * s).unwrap().alive().is_ok());

        let ka = cert.keys().with_policy(p, now).alive().revoked(false)
            .for_authentication().next().unwrap();
        assert!(ka.alive().is_ok());
        assert!(ka.clone().with_policy(p, now + 590 * s).unwrap().alive().is_ok());
        assert!(! ka.clone().with_policy(p, now + 610 * s).unwrap().alive().is_ok());
    }

    #[test]
    fn creation_time() {
        let p = &P::new();

        use std::time::UNIX_EPOCH;
        let (cert, rev) = CertBuilder::new()
            .set_creation_time(UNIX_EPOCH)
            .set_cipher_suite(CipherSuite::Cv25519)
            .add_userid("foo")
            .add_signing_subkey()
            .generate().unwrap();

        assert_eq!(cert.primary_key().creation_time(), UNIX_EPOCH);
        assert_eq!(cert.primary_key().with_policy(p, None).unwrap()
                   .binding_signature()
                   .signature_creation_time().unwrap(), UNIX_EPOCH);
        assert_eq!(cert.primary_key().with_policy(p, None).unwrap()
                   .direct_key_signature().unwrap()
                   .signature_creation_time().unwrap(), UNIX_EPOCH);
        assert_eq!(rev.signature_creation_time().unwrap(), UNIX_EPOCH);

        // (Sub)Keys.
        assert_eq!(cert.keys().with_policy(p, None).count(), 2);
        for ka in cert.keys().with_policy(p, None) {
            assert_eq!(ka.key().creation_time(), UNIX_EPOCH);
            assert_eq!(ka.binding_signature()
                       .signature_creation_time().unwrap(), UNIX_EPOCH);
        }

        // UserIDs.
        assert_eq!(cert.userids().count(), 1);
        for ui in cert.userids().with_policy(p, None) {
            assert_eq!(ui.binding_signature()
                       .signature_creation_time().unwrap(), UNIX_EPOCH);
        }
    }

    #[test]
    fn designated_revokers() -> Result<()> {
        use std::collections::HashSet;

        let p = &P::new();

        let fpr1 = "C03F A641 1B03 AE12 5764  6118 7223 B566 78E0 2528";
        let fpr2 = "50E6 D924 308D BF22 3CFB  510A C2B8 1905 6C65 2598";
        let revokers = vec![
            RevocationKey::new(PublicKeyAlgorithm::RSAEncryptSign,
                               Fingerprint::from_str(fpr1)?,
                               false),
            RevocationKey::new(PublicKeyAlgorithm::ECDSA,
                               Fingerprint::from_str(fpr2)?,
                               false)
        ];

        let (cert,_)
            = CertBuilder::general_purpose(None, Some("alice@example.org"))
            .set_revocation_keys(revokers.clone())
            .generate()?;
        let cert = cert.with_policy(p, None)?;

        assert_eq!(cert.revocation_keys(None).collect::<HashSet<_>>(),
                   revokers.iter().collect::<HashSet<_>>());

        // Do it again, with a key that has no User IDs.
        let (cert,_) = CertBuilder::new()
            .set_revocation_keys(revokers.clone())
            .generate()?;
        let cert = cert.with_policy(p, None)?;
        assert!(cert.primary_userid().is_err());

        assert_eq!(cert.revocation_keys(None).collect::<HashSet<_>>(),
                   revokers.iter().collect::<HashSet<_>>());

        // The designated revokers on all signatures should be
        // considered.
        let now = crate::types::Timestamp::now();
        let then = now.checked_add(crate::types::Duration::days(1)?).unwrap();
        let (cert,_) = CertBuilder::new()
            .set_revocation_keys(revokers.clone())
            .set_creation_time(now)
            .generate()?;

        // Add a newer direct key signature.
        use crate::crypto::hash::Hash;
        let mut hash = HashAlgorithm::SHA512.context()?;
        cert.primary_key().hash(&mut hash);
        let mut primary_signer =
            cert.primary_key().key().clone().parts_into_secret()?
            .into_keypair()?;
        let sig = signature::SignatureBuilder::new(SignatureType::DirectKey)
            .set_signature_creation_time(then)?
            .sign_hash(&mut primary_signer, hash)?;
        let cert = cert.insert_packets(sig)?;

        assert!(cert.with_policy(p, then)?.primary_userid().is_err());
        assert_eq!(cert.revocation_keys(p).collect::<HashSet<_>>(),
                   revokers.iter().collect::<HashSet<_>>());
        Ok(())
    }

    /// Checks that the builder emits exactly one user id or attribute
    /// marked as primary.
    #[test]
    fn primary_user_things() -> Result<()> {
        fn count_primary_user_things(c: Cert) -> usize {
            c.into_packets().map(|p| match p {
                Packet::Signature(s) if s.primary_userid().unwrap_or(false)
                    => 1,
                _ => 0,
            }).sum()
        }

        use crate::packet::{prelude::*, user_attribute::Subpacket};
        let ua_foo =
            UserAttribute::new(&[Subpacket::Unknown(7, vec![7; 7].into())])?;
        let ua_bar =
            UserAttribute::new(&[Subpacket::Unknown(11, vec![11; 11].into())])?;

        let p = &P::new();
        let positive = SignatureType::PositiveCertification;

        let (c, _) = CertBuilder::new().generate()?;
        assert_eq!(count_primary_user_things(c), 0);

        let (c, _) = CertBuilder::new()
            .add_userid("foo")
            .generate()?;
        assert_eq!(count_primary_user_things(c), 1);

        let (c, _) = CertBuilder::new()
            .add_userid("foo")
            .add_userid("bar")
            .generate()?;
        assert_eq!(count_primary_user_things(c), 1);

        let (c, _) = CertBuilder::new()
            .add_user_attribute(ua_foo.clone())
            .generate()?;
        assert_eq!(count_primary_user_things(c), 1);

        let (c, _) = CertBuilder::new()
            .add_user_attribute(ua_foo.clone())
            .add_user_attribute(ua_bar.clone())
            .generate()?;
        assert_eq!(count_primary_user_things(c), 1);

        let (c, _) = CertBuilder::new()
            .add_userid("foo")
            .add_user_attribute(ua_foo.clone())
            .generate()?;
        let vc = c.with_policy(p, None)?;
        assert_eq!(vc.primary_userid()?.binding_signature().primary_userid(),
                   Some(true));
        assert_eq!(vc.primary_user_attribute()?.binding_signature().primary_userid(),
                   None);
        assert_eq!(count_primary_user_things(c), 1);

        let (c, _) = CertBuilder::new()
            .add_user_attribute(ua_foo.clone())
            .add_userid("foo")
            .generate()?;
        let vc = c.with_policy(p, None)?;
        assert_eq!(vc.primary_userid()?.binding_signature().primary_userid(),
                   Some(true));
        assert_eq!(vc.primary_user_attribute()?.binding_signature().primary_userid(),
                   None);
        assert_eq!(count_primary_user_things(c), 1);

        let (c, _) = CertBuilder::new()
            .add_userid("foo")
            .add_userid_with(
                "buz",
                SignatureBuilder::new(positive).set_primary_userid(false)?)?
            .add_userid_with(
                "bar",
                SignatureBuilder::new(positive).set_primary_userid(true)?)?
            .add_userid_with(
                "baz",
                SignatureBuilder::new(positive).set_primary_userid(true)?)?
            .generate()?;
        let vc = c.with_policy(p, None)?;
        assert_eq!(vc.primary_userid()?.value(), b"bar");
        assert_eq!(count_primary_user_things(c), 1);

        Ok(())
    }
}
