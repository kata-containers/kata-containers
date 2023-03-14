//! X.509 `AlgorithmIdentifier`

use crate::{Error, Result};
use core::cmp::Ordering;
use der::{
    asn1::{Any, ObjectIdentifier},
    Decodable, Decoder, DerOrd, Encodable, Sequence, ValueOrd,
};

/// X.509 `AlgorithmIdentifier` as defined in [RFC 5280 Section 4.1.1.2].
///
/// ```text
/// AlgorithmIdentifier  ::=  SEQUENCE  {
///      algorithm               OBJECT IDENTIFIER,
///      parameters              ANY DEFINED BY algorithm OPTIONAL  }
/// ```
///
/// [RFC 5280 Section 4.1.1.2]: https://tools.ietf.org/html/rfc5280#section-4.1.1.2
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct AlgorithmIdentifier<'a> {
    /// Algorithm OID, i.e. the `algorithm` field in the `AlgorithmIdentifier`
    /// ASN.1 schema.
    pub oid: ObjectIdentifier,

    /// Algorithm `parameters`.
    pub parameters: Option<Any<'a>>,
}

impl<'a> AlgorithmIdentifier<'a> {
    /// Assert the `algorithm` OID is an expected value.
    pub fn assert_algorithm_oid(&self, expected_oid: ObjectIdentifier) -> Result<ObjectIdentifier> {
        if self.oid == expected_oid {
            Ok(expected_oid)
        } else {
            Err(Error::OidUnknown { oid: expected_oid })
        }
    }

    /// Assert `parameters` is an OID and has the expected value.
    pub fn assert_parameters_oid(
        &self,
        expected_oid: ObjectIdentifier,
    ) -> Result<ObjectIdentifier> {
        let actual_oid = self.parameters_oid()?;

        if actual_oid == expected_oid {
            Ok(actual_oid)
        } else {
            Err(Error::OidUnknown { oid: expected_oid })
        }
    }

    /// Assert the values of the `algorithm` and `parameters` OIDs.
    pub fn assert_oids(
        &self,
        algorithm: ObjectIdentifier,
        parameters: ObjectIdentifier,
    ) -> Result<()> {
        self.assert_algorithm_oid(algorithm)?;
        self.assert_parameters_oid(parameters)?;
        Ok(())
    }

    /// Get the `parameters` field as an [`Any`].
    ///
    /// Returns an error if `parameters` are `None`.
    pub fn parameters_any(&self) -> Result<Any<'a>> {
        self.parameters.ok_or(Error::AlgorithmParametersMissing)
    }

    /// Get the `parameters` field as an [`ObjectIdentifier`].
    ///
    /// Returns an error if it is absent or not an OID.
    pub fn parameters_oid(&self) -> Result<ObjectIdentifier> {
        Ok(ObjectIdentifier::try_from(self.parameters_any()?)?)
    }
}

impl<'a> Decodable<'a> for AlgorithmIdentifier<'a> {
    fn decode(decoder: &mut Decoder<'a>) -> der::Result<Self> {
        decoder.sequence(|decoder| {
            let oid = decoder.decode()?;
            let parameters = decoder.decode()?;
            Ok(Self { oid, parameters })
        })
    }
}

impl<'a> Sequence<'a> for AlgorithmIdentifier<'a> {
    fn fields<F, T>(&self, f: F) -> der::Result<T>
    where
        F: FnOnce(&[&dyn Encodable]) -> der::Result<T>,
    {
        f(&[&self.oid, &self.parameters])
    }
}

impl<'a> TryFrom<&'a [u8]> for AlgorithmIdentifier<'a> {
    type Error = Error;

    fn try_from(bytes: &'a [u8]) -> Result<Self> {
        Ok(Self::from_der(bytes)?)
    }
}

impl ValueOrd for AlgorithmIdentifier<'_> {
    fn value_cmp(&self, other: &Self) -> der::Result<Ordering> {
        match self.oid.der_cmp(&other.oid)? {
            Ordering::Equal => self.parameters.der_cmp(&other.parameters),
            other => Ok(other),
        }
    }
}
