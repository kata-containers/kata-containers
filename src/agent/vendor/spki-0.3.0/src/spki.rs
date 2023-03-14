//! X.509 `SubjectPublicKeyInfo`

use crate::AlgorithmIdentifier;
use core::convert::TryFrom;
use der::{Decodable, Encodable, Error, Message, Result};

/// X.509 `SubjectPublicKeyInfo` (SPKI).
///
/// ASN.1 structure containing an [`AlgorithmIdentifier`] and public key
/// data in an algorithm specific format.
///
/// Described in RFC 5280 Section 4.1.2.7:
/// <https://tools.ietf.org/html/rfc5280#section-4.1.2.7>
///
/// ```text
///    SubjectPublicKeyInfo  ::=  SEQUENCE  {
///         algorithm            AlgorithmIdentifier,
///         subjectPublicKey     BIT STRING  }
/// ```
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct SubjectPublicKeyInfo<'a> {
    /// X.509 [`AlgorithmIdentifier`] for the public key type
    pub algorithm: AlgorithmIdentifier<'a>,

    /// Public key data
    pub subject_public_key: &'a [u8],
}

impl<'a> TryFrom<&'a [u8]> for SubjectPublicKeyInfo<'a> {
    type Error = Error;

    fn try_from(bytes: &'a [u8]) -> Result<Self> {
        Self::from_der(bytes)
    }
}

impl<'a> TryFrom<der::Any<'a>> for SubjectPublicKeyInfo<'a> {
    type Error = Error;

    fn try_from(any: der::Any<'a>) -> Result<SubjectPublicKeyInfo<'a>> {
        any.sequence(|decoder| {
            Ok(Self {
                algorithm: decoder.decode()?,
                subject_public_key: decoder.bit_string()?.as_bytes(),
            })
        })
    }
}

impl<'a> Message<'a> for SubjectPublicKeyInfo<'a> {
    fn fields<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&[&dyn Encodable]) -> Result<T>,
    {
        f(&[
            &self.algorithm,
            &der::BitString::new(self.subject_public_key)?,
        ])
    }
}
