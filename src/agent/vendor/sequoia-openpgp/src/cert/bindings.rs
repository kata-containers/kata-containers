use std::time;

use crate::Error;
use crate::Result;
use crate::Cert;
use crate::types::{HashAlgorithm, SignatureType};
use crate::crypto::Signer;
use crate::packet::{UserID, UserAttribute, key, Key, signature, Signature};

impl<P: key::KeyParts> Key<P, key::SubordinateRole> {
    /// Creates a binding signature.
    ///
    /// The signature binds this subkey to `cert`. `signer` will be used
    /// to create a signature using `signature` as builder.
    /// The`hash_algo` defaults to SHA512, `creation_time` to the
    /// current time.
    ///
    /// Note that subkeys with signing capabilities need a [primary
    /// key binding signature].  If you are creating this binding
    /// signature from a previous binding signature, you can reuse the
    /// primary key binding signature if it is still valid and meets
    /// current algorithm requirements.  Otherwise, you can create one
    /// using [`SignatureBuilder::sign_primary_key_binding`].
    ///
    ///   [primary key binding signature]: https://tools.ietf.org/html/rfc4880#section-5.2.1
    ///   [`SignatureBuilder::sign_primary_key_binding`]: signature::SignatureBuilder::sign_primary_key_binding()
    ///
    /// This function adds a creation time subpacket, a issuer
    /// fingerprint subpacket, and a issuer subpacket to the
    /// signature.
    ///
    /// # Examples
    ///
    /// This example demonstrates how to bind this key to a Cert.  Note
    /// that in general, the `CertBuilder` is a better way to add
    /// subkeys to a Cert.
    ///
    /// ```
    /// # use sequoia_openpgp::{*, packet::prelude::*, types::*, cert::*};
    /// # fn main() -> Result<()> {
    /// use sequoia_openpgp::policy::StandardPolicy;
    /// let p = &StandardPolicy::new();
    ///
    /// // Generate a Cert, and create a keypair from the primary key.
    /// let (cert, _) = CertBuilder::new().generate()?;
    /// let mut keypair = cert.primary_key().key().clone()
    ///     .parts_into_secret()?.into_keypair()?;
    ///
    /// // Let's add an encryption subkey.
    /// let flags = KeyFlags::empty().set_storage_encryption();
    /// assert_eq!(cert.keys().with_policy(p, None).alive().revoked(false)
    ///                .key_flags(&flags).count(),
    ///            0);
    ///
    /// // Generate a subkey and a binding signature.
    /// let subkey: Key<_, key::SubordinateRole> =
    ///     Key4::generate_ecc(false, Curve::Cv25519)?
    ///     .into();
    /// let builder = signature::SignatureBuilder::new(SignatureType::SubkeyBinding)
    ///     .set_key_flags(flags.clone())?;
    /// let binding = subkey.bind(&mut keypair, &cert, builder)?;
    ///
    /// // Now merge the key and binding signature into the Cert.
    /// let cert = cert.insert_packets(vec![Packet::from(subkey),
    ///                                    binding.into()])?;
    ///
    /// // Check that we have an encryption subkey.
    /// assert_eq!(cert.keys().with_policy(p, None).alive().revoked(false)
    ///                .key_flags(flags).count(),
    ///            1);
    /// # Ok(()) }
    pub fn bind(&self, signer: &mut dyn Signer, cert: &Cert,
                signature: signature::SignatureBuilder)
        -> Result<Signature>
    {
        signature.sign_subkey_binding(
            signer, cert.primary_key().key(), self)
    }
}

impl UserID {
    /// Creates a binding signature.
    ///
    /// The signature binds this User ID to `cert`. `signer` will be used
    /// to create a signature using `signature` as builder.
    /// The`hash_algo` defaults to SHA512, `creation_time` to the
    /// current time.
    ///
    /// This function adds a creation time subpacket, a issuer
    /// fingerprint subpacket, and a issuer subpacket to the
    /// signature.
    ///
    /// # Examples
    ///
    /// This example demonstrates how to bind this User ID to a Cert.
    /// Note that in general, the `CertBuilder` is a better way to add
    /// User IDs to a Cert.
    ///
    /// ```
    /// # use sequoia_openpgp::{*, packet::prelude::*, types::*, cert::*};
    /// # fn main() -> Result<()> {
    /// // Generate a Cert, and create a keypair from the primary key.
    /// let (cert, _) = CertBuilder::new().generate()?;
    /// let mut keypair = cert.primary_key().key().clone()
    ///     .parts_into_secret()?.into_keypair()?;
    /// assert_eq!(cert.userids().len(), 0);
    ///
    /// // Generate a User ID and a binding signature.
    /// let userid = UserID::from("test@example.org");
    /// let builder =
    ///     signature::SignatureBuilder::new(SignatureType::PositiveCertification);
    /// let binding = userid.bind(&mut keypair, &cert, builder)?;
    ///
    /// // Now merge the User ID and binding signature into the Cert.
    /// let cert = cert.insert_packets(vec![Packet::from(userid),
    ///                                    binding.into()])?;
    ///
    /// // Check that we have a User ID.
    /// assert_eq!(cert.userids().len(), 1);
    /// # Ok(()) }
    pub fn bind(&self, signer: &mut dyn Signer, cert: &Cert,
                signature: signature::SignatureBuilder)
                -> Result<Signature>
    {
        signature.sign_userid_binding(
            signer, cert.primary_key().key(), self)
    }

    /// Returns a certification for the User ID.
    ///
    /// The signature binds this User ID to `cert`. `signer` will be
    /// used to create a certification signature of type
    /// `signature_type`.  `signature_type` defaults to
    /// `SignatureType::GenericCertification`, `hash_algo` to SHA512,
    /// `creation_time` to the current time.
    ///
    /// This function adds a creation time subpacket, a issuer
    /// fingerprint subpacket, and a issuer subpacket to the
    /// signature.
    ///
    /// # Errors
    ///
    /// Returns `Error::InvalidArgument` if `signature_type` is not
    /// one of `SignatureType::{Generic, Persona, Casual,
    /// Positive}Certification`
    ///
    /// # Examples
    ///
    /// This example demonstrates how to certify a User ID.
    ///
    /// ```
    /// # use sequoia_openpgp::{*, packet::prelude::*, types::*, cert::*};
    /// # fn main() -> Result<()> {
    /// // Generate a Cert, and create a keypair from the primary key.
    /// let (alice, _) = CertBuilder::new()
    ///     .set_primary_key_flags(KeyFlags::empty().set_certification())
    ///     .add_userid("alice@example.org")
    ///     .generate()?;
    /// let mut keypair = alice.primary_key().key().clone()
    ///     .parts_into_secret()?.into_keypair()?;
    ///
    /// // Generate a Cert for Bob.
    /// let (bob, _) = CertBuilder::new()
    ///     .set_primary_key_flags(KeyFlags::empty().set_certification())
    ///     .add_userid("bob@example.org")
    ///     .generate()?;
    ///
    /// // Alice now certifies the binding between `bob@example.org` and `bob`.
    /// let certification =
    ///     bob.userids().nth(0).unwrap()
    ///     .certify(&mut keypair, &bob, SignatureType::PositiveCertification,
    ///              None, None)?;
    ///
    /// // `certification` can now be used, e.g. by merging it into `bob`.
    /// let bob = bob.insert_packets(certification)?;
    ///
    /// // Check that we have a certification on the User ID.
    /// assert_eq!(bob.userids().nth(0).unwrap()
    ///            .certifications().count(), 1);
    /// # Ok(()) }
    pub fn certify<S, H, T>(&self, signer: &mut dyn Signer, cert: &Cert,
                            signature_type: S,
                            hash_algo: H, creation_time: T)
        -> Result<Signature>
        where S: Into<Option<SignatureType>>,
              H: Into<Option<HashAlgorithm>>,
              T: Into<Option<time::SystemTime>>
    {
        let typ = signature_type.into();
        let typ = match typ {
            Some(SignatureType::GenericCertification)
                | Some(SignatureType::PersonaCertification)
                | Some(SignatureType::CasualCertification)
                | Some(SignatureType::PositiveCertification) => typ.unwrap(),
            Some(t) => return Err(Error::InvalidArgument(
                format!("Invalid signature type: {}", t)).into()),
            None => SignatureType::GenericCertification,
        };
        let mut sig = signature::SignatureBuilder::new(typ);
        if let Some(algo) = hash_algo.into() {
            sig = sig.set_hash_algo(algo);
        }
        if let Some(creation_time) = creation_time.into() {
            sig = sig.set_signature_creation_time(creation_time)?;
        }
        self.bind(signer, cert, sig)
    }
}

impl UserAttribute {
    /// Creates a binding signature.
    ///
    /// The signature binds this user attribute to `cert`. `signer`
    /// will be used to create a signature using `signature` as
    /// builder.  The`hash_algo` defaults to SHA512, `creation_time`
    /// to the current time.
    ///
    /// This function adds a creation time subpacket, a issuer
    /// fingerprint subpacket, and a issuer subpacket to the
    /// signature.
    ///
    /// # Examples
    ///
    /// This example demonstrates how to bind this user attribute to a
    /// Cert.  Note that in general, the `CertBuilder` is a better way
    /// to add User IDs to a Cert.
    ///
    /// ```
    /// # use sequoia_openpgp::{*, packet::prelude::*, types::*, cert::*,
    /// #                       packet::user_attribute::*};
    /// # fn main() -> Result<()> {
    /// // Generate a Cert, and create a keypair from the primary key.
    /// let (cert, _) = CertBuilder::new()
    ///     .generate()?;
    /// let mut keypair = cert.primary_key().key().clone()
    ///     .parts_into_secret()?.into_keypair()?;
    /// assert_eq!(cert.userids().len(), 0);
    ///
    /// // Generate a user attribute and a binding signature.
    /// let user_attr = UserAttribute::new(&[
    ///     Subpacket::Image(
    ///         Image::Private(100, vec![0, 1, 2].into_boxed_slice())),
    /// ])?;
    /// let builder =
    ///     signature::SignatureBuilder::new(SignatureType::PositiveCertification);
    /// let binding = user_attr.bind(&mut keypair, &cert, builder)?;
    ///
    /// // Now merge the user attribute and binding signature into the Cert.
    /// let cert = cert.insert_packets(vec![Packet::from(user_attr),
    ///                                    binding.into()])?;
    ///
    /// // Check that we have a user attribute.
    /// assert_eq!(cert.user_attributes().count(), 1);
    /// # Ok(()) }
    pub fn bind(&self, signer: &mut dyn Signer, cert: &Cert,
                signature: signature::SignatureBuilder)
        -> Result<Signature>
    {
        signature.sign_user_attribute_binding(
            signer, cert.primary_key().key(), self)
    }

    /// Returns a certification for the user attribute.
    ///
    /// The signature binds this user attribute to `cert`. `signer` will be
    /// used to create a certification signature of type
    /// `signature_type`.  `signature_type` defaults to
    /// `SignatureType::GenericCertification`, `hash_algo` to SHA512,
    /// `creation_time` to the current time.
    ///
    /// This function adds a creation time subpacket, a issuer
    /// fingerprint subpacket, and a issuer subpacket to the
    /// signature.
    ///
    /// # Errors
    ///
    /// Returns `Error::InvalidArgument` if `signature_type` is not
    /// one of `SignatureType::{Generic, Persona, Casual,
    /// Positive}Certification`
    ///
    /// # Examples
    ///
    /// This example demonstrates how to certify a User ID.
    ///
    /// ```
    /// # use sequoia_openpgp::{*, packet::prelude::*, types::*, cert::*,
    /// #                       packet::user_attribute::*};
    /// # fn main() -> Result<()> {
    /// // Generate a Cert, and create a keypair from the primary key.
    /// let (alice, _) = CertBuilder::new()
    ///     .add_userid("alice@example.org")
    ///     .generate()?;
    /// let mut keypair = alice.primary_key().key().clone()
    ///     .parts_into_secret()?.into_keypair()?;
    ///
    /// // Generate a Cert for Bob.
    /// let user_attr = UserAttribute::new(&[
    ///     Subpacket::Image(
    ///         Image::Private(100, vec![0, 1, 2].into_boxed_slice())),
    /// ])?;
    /// let (bob, _) = CertBuilder::new()
    ///     .set_primary_key_flags(KeyFlags::empty().set_certification())
    ///     .add_user_attribute(user_attr)
    ///     .generate()?;
    ///
    /// // Alice now certifies the binding between `bob@example.org` and `bob`.
    /// let certification =
    ///     bob.user_attributes().nth(0).unwrap()
    ///     .certify(&mut keypair, &bob, SignatureType::PositiveCertification,
    ///              None, None)?;
    ///
    /// // `certification` can now be used, e.g. by merging it into `bob`.
    /// let bob = bob.insert_packets(certification)?;
    ///
    /// // Check that we have a certification on the User ID.
    /// assert_eq!(bob.user_attributes().nth(0).unwrap()
    ///            .certifications().count(),
    ///            1);
    /// # Ok(()) }
    pub fn certify<S, H, T>(&self, signer: &mut dyn Signer, cert: &Cert,
                            signature_type: S,
                            hash_algo: H, creation_time: T)
        -> Result<Signature>
        where S: Into<Option<SignatureType>>,
              H: Into<Option<HashAlgorithm>>,
              T: Into<Option<time::SystemTime>>
    {
        let typ = signature_type.into();
        let typ = match typ {
            Some(SignatureType::GenericCertification)
                | Some(SignatureType::PersonaCertification)
                | Some(SignatureType::CasualCertification)
                | Some(SignatureType::PositiveCertification) => typ.unwrap(),
            Some(t) => return Err(Error::InvalidArgument(
                format!("Invalid signature type: {}", t)).into()),
            None => SignatureType::GenericCertification,
        };
        let mut sig = signature::SignatureBuilder::new(typ);
        if let Some(algo) = hash_algo.into() {
            sig = sig.set_hash_algo(algo);
        }
        if let Some(creation_time) = creation_time.into() {
            sig = sig.set_signature_creation_time(creation_time)?;
        }
        self.bind(signer, cert, sig)
    }
}
