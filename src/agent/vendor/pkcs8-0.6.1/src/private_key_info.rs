//! PKCS#8 `PrivateKeyInfo`.
// TODO(tarcieri): merge this into `OneAsymmetricKey` in the next breaking release.

use crate::{AlgorithmIdentifier, Attributes, Error, Result, Version};
use core::{convert::TryFrom, fmt};
use der::{Decodable, Encodable, Message};

#[cfg(feature = "alloc")]
use crate::PrivateKeyDocument;

#[cfg(feature = "encryption")]
use {
    crate::EncryptedPrivateKeyDocument,
    rand_core::{CryptoRng, RngCore},
};

#[cfg(feature = "pem")]
use {crate::pem, zeroize::Zeroizing};

/// PKCS#8 `PrivateKeyInfo`.
///
/// ASN.1 structure containing an [`AlgorithmIdentifier`] and private key
/// data in an algorithm specific format.
///
/// Described in [RFC 5208 Section 5]:
///
/// ```text
/// PrivateKeyInfo ::= SEQUENCE {
///         version                   Version,
///         privateKeyAlgorithm       PrivateKeyAlgorithmIdentifier,
///         privateKey                PrivateKey,
///         attributes           [0]  IMPLICIT Attributes OPTIONAL }
///
/// Version ::= INTEGER
///
/// PrivateKeyAlgorithmIdentifier ::= AlgorithmIdentifier
///
/// PrivateKey ::= OCTET STRING
///
/// Attributes ::= SET OF Attribute
/// ```
///
/// Note: `PrivateKeyInfo` only allows version `v1` (`0x00`), use [`crate::OneAsymmetricKey`]
/// for PKCS#8 documents with version `v2` (`0x01`).
///
/// [RFC 5208 Section 5]: https://tools.ietf.org/html/rfc5208#section-5
#[derive(Clone)]
pub struct PrivateKeyInfo<'a> {
    /// X.509 [`AlgorithmIdentifier`] for the private key type
    pub algorithm: AlgorithmIdentifier<'a>,

    /// Private key data
    pub private_key: &'a [u8],
    // TODO(tarcieri): support for `Attributes`
}

impl<'a> PrivateKeyInfo<'a> {
    /// Encrypt this private key using a symmetric encryption key derived
    /// from the provided password.
    #[cfg(feature = "encryption")]
    #[cfg_attr(docsrs, doc(cfg(feature = "encryption")))]
    pub fn encrypt(
        &self,
        rng: impl CryptoRng + RngCore,
        password: impl AsRef<[u8]>,
    ) -> Result<EncryptedPrivateKeyDocument> {
        PrivateKeyDocument::from(self).encrypt(rng, password)
    }

    /// Encode this [`PrivateKeyInfo`] as ASN.1 DER.
    #[cfg(feature = "alloc")]
    #[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
    pub fn to_der(&self) -> PrivateKeyDocument {
        self.into()
    }

    /// Encode this [`PrivateKeyInfo`] as PEM-encoded ASN.1 DER.
    #[cfg(feature = "pem")]
    #[cfg_attr(docsrs, doc(cfg(feature = "pem")))]
    pub fn to_pem(&self) -> Zeroizing<alloc::string::String> {
        Zeroizing::new(pem::encode(
            self.to_der().as_ref(),
            pem::PRIVATE_KEY_BOUNDARY,
        ))
    }
}

impl<'a> TryFrom<&'a [u8]> for PrivateKeyInfo<'a> {
    type Error = Error;

    fn try_from(bytes: &'a [u8]) -> Result<Self> {
        Ok(Self::from_der(bytes)?)
    }
}

impl<'a> TryFrom<der::Any<'a>> for PrivateKeyInfo<'a> {
    type Error = der::Error;

    fn try_from(any: der::Any<'a>) -> der::Result<PrivateKeyInfo<'a>> {
        any.sequence(|decoder| {
            // Parse and validate `version` INTEGER.
            // For PrivateKeyInfo, only v1 is valid.
            if Version::V1 != Version::decode(decoder)? {
                return Err(der::ErrorKind::Value {
                    tag: der::Tag::Integer,
                }
                .into());
            }

            let algorithm = decoder.decode()?;
            let private_key = decoder.octet_string()?.into();
            let _attributes: Option<Attributes<'_>> = decoder.optional()?;

            Ok(Self {
                algorithm,
                private_key,
            })
        })
    }
}

impl<'a> Message<'a> for PrivateKeyInfo<'a> {
    fn fields<F, T>(&self, f: F) -> der::Result<T>
    where
        F: FnOnce(&[&dyn Encodable]) -> der::Result<T>,
    {
        f(&[
            &u8::from(Version::V1),
            &self.algorithm,
            &der::OctetString::new(self.private_key)?,
        ])
    }
}

impl<'a> fmt::Debug for PrivateKeyInfo<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PrivateKeyInfo")
            .field("algorithm", &self.algorithm)
            .finish() // TODO(tarcieri): use `finish_non_exhaustive` when stable
    }
}
