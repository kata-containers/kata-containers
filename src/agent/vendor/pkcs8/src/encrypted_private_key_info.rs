//! PKCS#8 `EncryptedPrivateKeyInfo`

use crate::{Error, Result};
use core::fmt;
use der::{asn1::OctetString, Decodable, Decoder, Encodable, Sequence};
use pkcs5::EncryptionScheme;

#[cfg(feature = "alloc")]
use crate::{EncryptedPrivateKeyDocument, PrivateKeyDocument};

#[cfg(feature = "pem")]
use {crate::LineEnding, alloc::string::String, der::Document, zeroize::Zeroizing};

/// PKCS#8 `EncryptedPrivateKeyInfo`.
///
/// ASN.1 structure containing a PKCS#5 [`EncryptionScheme`] identifier for a
/// password-based symmetric encryption scheme and encrypted private key data.
///
/// ## Schema
/// Structure described in [RFC 5208 Section 6]:
///
/// ```text
/// EncryptedPrivateKeyInfo ::= SEQUENCE {
///   encryptionAlgorithm  EncryptionAlgorithmIdentifier,
///   encryptedData        EncryptedData }
///
/// EncryptionAlgorithmIdentifier ::= AlgorithmIdentifier
///
/// EncryptedData ::= OCTET STRING
/// ```
///
/// [RFC 5208 Section 6]: https://tools.ietf.org/html/rfc5208#section-6
#[cfg_attr(docsrs, doc(cfg(feature = "pkcs5")))]
#[derive(Clone, Eq, PartialEq)]
pub struct EncryptedPrivateKeyInfo<'a> {
    /// Algorithm identifier describing a password-based symmetric encryption
    /// scheme used to encrypt the `encrypted_data` field.
    pub encryption_algorithm: EncryptionScheme<'a>,

    /// Private key data
    pub encrypted_data: &'a [u8],
}

impl<'a> EncryptedPrivateKeyInfo<'a> {
    /// Attempt to decrypt this encrypted private key using the provided
    /// password to derive an encryption key.
    #[cfg(feature = "encryption")]
    #[cfg_attr(docsrs, doc(cfg(feature = "encryption")))]
    pub fn decrypt(&self, password: impl AsRef<[u8]>) -> Result<PrivateKeyDocument> {
        Ok(self
            .encryption_algorithm
            .decrypt(password, self.encrypted_data)?
            .try_into()?)
    }

    /// Encode this [`EncryptedPrivateKeyInfo`] as ASN.1 DER.
    #[cfg(feature = "alloc")]
    #[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
    pub fn to_der(&self) -> Result<EncryptedPrivateKeyDocument> {
        self.try_into()
    }

    /// Encode this [`EncryptedPrivateKeyInfo`] as PEM-encoded ASN.1 DER with
    /// the given [`LineEnding`].
    #[cfg(feature = "pem")]
    #[cfg_attr(docsrs, doc(cfg(feature = "pem")))]
    pub fn to_pem(&self, line_ending: LineEnding) -> Result<Zeroizing<String>> {
        Ok(Zeroizing::new(
            EncryptedPrivateKeyDocument::try_from(self)?.to_pem(line_ending)?,
        ))
    }
}

impl<'a> Decodable<'a> for EncryptedPrivateKeyInfo<'a> {
    fn decode(decoder: &mut Decoder<'a>) -> der::Result<EncryptedPrivateKeyInfo<'a>> {
        decoder.sequence(|decoder| {
            Ok(Self {
                encryption_algorithm: decoder.decode()?,
                encrypted_data: decoder.octet_string()?.as_bytes(),
            })
        })
    }
}

impl<'a> Sequence<'a> for EncryptedPrivateKeyInfo<'a> {
    fn fields<F, T>(&self, f: F) -> der::Result<T>
    where
        F: FnOnce(&[&dyn Encodable]) -> der::Result<T>,
    {
        f(&[
            &self.encryption_algorithm,
            &OctetString::new(self.encrypted_data)?,
        ])
    }
}

impl<'a> TryFrom<&'a [u8]> for EncryptedPrivateKeyInfo<'a> {
    type Error = Error;

    fn try_from(bytes: &'a [u8]) -> Result<Self> {
        Ok(Self::from_der(bytes)?)
    }
}

impl<'a> fmt::Debug for EncryptedPrivateKeyInfo<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EncryptedPrivateKeyInfo")
            .field("encryption_algorithm", &self.encryption_algorithm)
            .finish() // TODO(tarcieri): use `finish_non_exhaustive` when stable
    }
}
