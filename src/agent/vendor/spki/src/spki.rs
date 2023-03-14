//! X.509 `SubjectPublicKeyInfo`

use crate::{AlgorithmIdentifier, Error, Result};
use der::{asn1::BitString, Decodable, Decoder, Encodable, Sequence};

#[cfg(feature = "fingerprint")]
use sha2::{digest, Digest, Sha256};

#[cfg(all(feature = "alloc", feature = "fingerprint"))]
use {
    alloc::string::String,
    base64ct::{Base64, Encoding},
};

/// X.509 `SubjectPublicKeyInfo` (SPKI) as defined in [RFC 5280 Section 4.1.2.7].
///
/// ASN.1 structure containing an [`AlgorithmIdentifier`] and public key
/// data in an algorithm specific format.
///
/// ```text
///    SubjectPublicKeyInfo  ::=  SEQUENCE  {
///         algorithm            AlgorithmIdentifier,
///         subjectPublicKey     BIT STRING  }
/// ```
///
/// [RFC 5280 Section 4.1.2.7]: https://tools.ietf.org/html/rfc5280#section-4.1.2.7
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct SubjectPublicKeyInfo<'a> {
    /// X.509 [`AlgorithmIdentifier`] for the public key type
    pub algorithm: AlgorithmIdentifier<'a>,

    /// Public key data
    pub subject_public_key: &'a [u8],
}

impl<'a> SubjectPublicKeyInfo<'a> {
    /// Calculate the SHA-256 fingerprint of this [`SubjectPublicKeyInfo`].
    #[cfg(feature = "fingerprint")]
    #[cfg_attr(docsrs, doc(cfg(feature = "fingerprint")))]
    pub fn fingerprint(&self) -> Result<digest::Output<Sha256>> {
        let mut buf = [0u8; 4096];
        Ok(Sha256::digest(self.encode_to_slice(&mut buf)?))
    }

    /// Calculate the SHA-256 fingerprint of this [`SubjectPublicKeyInfo`] and
    /// encode it as a Base64 string.
    #[cfg(all(feature = "fingerprint", feature = "alloc"))]
    #[cfg_attr(docsrs, doc(cfg(all(feature = "fingerprint", feature = "alloc"))))]
    pub fn fingerprint_base64(&self) -> Result<String> {
        Ok(Base64::encode_string(self.fingerprint()?.as_slice()))
    }
}

impl<'a> Decodable<'a> for SubjectPublicKeyInfo<'a> {
    fn decode(decoder: &mut Decoder<'a>) -> der::Result<Self> {
        decoder.sequence(|decoder| {
            let algorithm = decoder.decode()?;
            let subject_public_key = decoder
                .bit_string()?
                .as_bytes()
                .ok_or_else(|| der::Tag::BitString.value_error())?;

            Ok(Self {
                algorithm,
                subject_public_key,
            })
        })
    }
}

impl<'a> Sequence<'a> for SubjectPublicKeyInfo<'a> {
    fn fields<F, T>(&self, f: F) -> der::Result<T>
    where
        F: FnOnce(&[&dyn Encodable]) -> der::Result<T>,
    {
        f(&[
            &self.algorithm,
            &BitString::from_bytes(self.subject_public_key)?,
        ])
    }
}

impl<'a> TryFrom<&'a [u8]> for SubjectPublicKeyInfo<'a> {
    type Error = Error;

    fn try_from(bytes: &'a [u8]) -> Result<Self> {
        Ok(Self::from_der(bytes)?)
    }
}
