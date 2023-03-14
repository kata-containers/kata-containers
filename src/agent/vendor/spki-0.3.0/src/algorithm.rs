//! X.509 `AlgorithmIdentifier`

use core::convert::{TryFrom, TryInto};
use der::{Any, Decodable, Encodable, Error, ErrorKind, Message, ObjectIdentifier, Result};

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
    /// Get the `parameters` field as an [`Any`].
    ///
    /// Returns an error if `parameters` are `None`.
    pub fn parameters_any(&self) -> Result<Any<'a>> {
        self.parameters.ok_or_else(|| ErrorKind::Truncated.into())
    }

    /// Get the `parameters` field as an [`ObjectIdentifier`].
    ///
    /// Returns an error if it is absent or not an OID.
    pub fn parameters_oid(&self) -> Result<ObjectIdentifier> {
        self.parameters_any().and_then(TryInto::try_into)
    }
}

impl<'a> TryFrom<&'a [u8]> for AlgorithmIdentifier<'a> {
    type Error = Error;

    fn try_from(bytes: &'a [u8]) -> Result<Self> {
        Self::from_der(bytes)
    }
}

impl<'a> TryFrom<Any<'a>> for AlgorithmIdentifier<'a> {
    type Error = Error;

    fn try_from(any: Any<'a>) -> Result<AlgorithmIdentifier<'a>> {
        any.sequence(|decoder| {
            let oid = decoder.decode()?;
            let parameters = decoder.decode()?;
            Ok(Self { oid, parameters })
        })
    }
}

impl<'a> Message<'a> for AlgorithmIdentifier<'a> {
    fn fields<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&[&dyn Encodable]) -> Result<T>,
    {
        f(&[&self.oid, &self.parameters])
    }
}
