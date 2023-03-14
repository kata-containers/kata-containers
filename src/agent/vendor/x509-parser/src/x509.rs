//! X.509 objects and types
//!
//! Based on RFC5280
//!

use crate::error::{X509Error, X509Result};
use crate::objects::*;
use crate::public_key::*;

use asn1_rs::{Any, BitString, DerSequence, FromBer, FromDer, Oid, ParseResult};
use data_encoding::HEXUPPER;
use der_parser::ber::MAX_OBJECT_SIZE;
use der_parser::der::*;
use der_parser::error::*;
use der_parser::num_bigint::BigUint;
use der_parser::*;
use nom::branch::alt;
use nom::bytes::complete::take;
use nom::combinator::{complete, map};
use nom::multi::{many0, many1};
use nom::{Err, Offset};
use oid_registry::*;
use rusticata_macros::newtype_enum;
use std::fmt;
use std::iter::FromIterator;

/// The version of the encoded certificate.
///
/// When extensions are used, as expected in this profile, version MUST be 3
/// (value is `2`).  If no extensions are present, but a UniqueIdentifier
/// is present, the version SHOULD be 2 (value is `1`); however, the
/// version MAY be 3.  If only basic fields are present, the version
/// SHOULD be 1 (the value is omitted from the certificate as the default
/// value); however, the version MAY be 2 or 3.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct X509Version(pub u32);

impl X509Version {
    pub(crate) fn from_der_required(i: &[u8]) -> X509Result<X509Version> {
        let (rem, hdr) =
            der_read_element_header(i).or(Err(Err::Error(X509Error::InvalidVersion)))?;
        match hdr.tag().0 {
            0 => {
                map(parse_der_u32, X509Version)(rem).or(Err(Err::Error(X509Error::InvalidVersion)))
            }
            _ => Ok((&rem[1..], X509Version::V1)),
        }
    }
}

// Parse [0] EXPLICIT Version DEFAULT v1
impl<'a> FromDer<'a, X509Error> for X509Version {
    fn from_der(i: &'a [u8]) -> X509Result<'a, Self> {
        let (rem, hdr) =
            der_read_element_header(i).or(Err(Err::Error(X509Error::InvalidVersion)))?;
        match hdr.tag().0 {
            0 => {
                map(parse_der_u32, X509Version)(rem).or(Err(Err::Error(X509Error::InvalidVersion)))
            }
            _ => Ok((i, X509Version::V1)),
        }
    }
}

newtype_enum! {
    impl display X509Version {
        V1 = 0,
        V2 = 1,
        V3 = 2,
    }
}

/// A generic attribute type and value
///
/// These objects are used as [`RelativeDistinguishedName`] components.
#[derive(Clone, Debug, PartialEq)]
pub struct AttributeTypeAndValue<'a> {
    attr_type: Oid<'a>,
    attr_value: Any<'a>, // ANY -- DEFINED BY AttributeType
}

impl<'a> AttributeTypeAndValue<'a> {
    /// Builds a new `AttributeTypeAndValue`
    #[inline]
    pub const fn new(attr_type: Oid<'a>, attr_value: Any<'a>) -> Self {
        AttributeTypeAndValue {
            attr_type,
            attr_value,
        }
    }

    /// Returns the attribute type
    #[inline]
    pub const fn attr_type(&self) -> &Oid {
        &self.attr_type
    }

    /// Returns the attribute value, as `ANY`
    #[inline]
    pub const fn attr_value(&self) -> &Any {
        &self.attr_value
    }

    /// Attempt to get the content as `str`.
    /// This can fail if the object does not contain a string type.
    ///
    /// Note: the [`TryFrom`](core::convert::TryFrom) trait is implemented for `&str`, so this is
    /// equivalent to `attr.try_into()`.
    ///
    /// Only NumericString, PrintableString, UTF8String and IA5String
    /// are considered here. Other string types can be read using `as_slice`.
    #[inline]
    pub fn as_str(&'a self) -> Result<&'a str, X509Error> {
        // TODO: replace this with helper function, when it is added to asn1-rs
        match self.attr_value.tag() {
            Tag::NumericString | Tag::PrintableString | Tag::Utf8String | Tag::Ia5String => {
                let s = core::str::from_utf8(self.attr_value.data)
                    .map_err(|_| X509Error::InvalidAttributes)?;
                Ok(s)
            }
            t => Err(X509Error::Der(Error::unexpected_tag(None, t))),
        }
    }

    /// Get the content as a slice.
    #[inline]
    pub fn as_slice(&'a self) -> &'a [u8] {
        self.attr_value.as_bytes()
    }
}

impl<'a, 'b> core::convert::TryFrom<&'a AttributeTypeAndValue<'b>> for &'a str {
    type Error = X509Error;

    fn try_from(value: &'a AttributeTypeAndValue<'b>) -> Result<Self, Self::Error> {
        value.attr_value.as_str().map_err(|e| e.into())
    }
}

impl<'a, 'b> From<&'a AttributeTypeAndValue<'b>> for &'a [u8] {
    fn from(value: &'a AttributeTypeAndValue<'b>) -> Self {
        value.as_slice()
    }
}

// AttributeTypeAndValue   ::= SEQUENCE {
//     type    AttributeType,
//     value   AttributeValue }
impl<'a> FromDer<'a, X509Error> for AttributeTypeAndValue<'a> {
    fn from_der(i: &'a [u8]) -> X509Result<'a, Self> {
        parse_der_sequence_defined_g(|i, _| {
            let (i, attr_type) = Oid::from_der(i).or(Err(X509Error::InvalidX509Name))?;
            let (i, attr_value) = parse_attribute_value(i).or(Err(X509Error::InvalidX509Name))?;
            let attr = AttributeTypeAndValue::new(attr_type, attr_value);
            Ok((i, attr))
        })(i)
    }
}

// AttributeValue          ::= ANY -- DEFINED BY AttributeType
#[inline]
fn parse_attribute_value(i: &[u8]) -> ParseResult<Any, Error> {
    alt((Any::from_der, parse_malformed_string))(i)
}

fn parse_malformed_string(i: &[u8]) -> ParseResult<Any, Error> {
    let (rem, hdr) = Header::from_der(i)?;
    let len = hdr.length().definite()?;
    if len > MAX_OBJECT_SIZE {
        return Err(nom::Err::Error(Error::InvalidLength));
    }
    match hdr.tag() {
        Tag::PrintableString => {
            // if we are in this function, the PrintableString could not be validated.
            // Accept it without validating charset, because some tools do not respect the charset
            // restrictions (for ex. they use '*' while explicingly disallowed)
            let (rem, data) = take(len as usize)(rem)?;
            // check valid encoding
            let _ = std::str::from_utf8(data).map_err(|_| Error::BerValueError)?;
            let obj = Any::new(hdr, data);
            Ok((rem, obj))
        }
        t => Err(nom::Err::Error(Error::unexpected_tag(
            Some(Tag::PrintableString),
            t,
        ))),
    }
}

/// A Relative Distinguished Name element.
///
/// These objects are used as [`X509Name`] components.
#[derive(Clone, Debug, PartialEq)]
pub struct RelativeDistinguishedName<'a> {
    set: Vec<AttributeTypeAndValue<'a>>,
}

impl<'a> RelativeDistinguishedName<'a> {
    /// Builds a new `RelativeDistinguishedName`
    #[inline]
    pub const fn new(set: Vec<AttributeTypeAndValue<'a>>) -> Self {
        RelativeDistinguishedName { set }
    }

    /// Return an iterator over the components of this object
    pub fn iter(&self) -> impl Iterator<Item = &AttributeTypeAndValue<'a>> {
        self.set.iter()
    }
}

impl<'a> FromIterator<AttributeTypeAndValue<'a>> for RelativeDistinguishedName<'a> {
    fn from_iter<T: IntoIterator<Item = AttributeTypeAndValue<'a>>>(iter: T) -> Self {
        let set = iter.into_iter().collect();
        RelativeDistinguishedName { set }
    }
}

impl<'a> FromDer<'a, X509Error> for RelativeDistinguishedName<'a> {
    fn from_der(i: &'a [u8]) -> X509Result<Self> {
        parse_der_set_defined_g(|i, _| {
            let (i, set) = many1(complete(AttributeTypeAndValue::from_der))(i)?;
            let rdn = RelativeDistinguishedName { set };
            Ok((i, rdn))
        })(i)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SubjectPublicKeyInfo<'a> {
    pub algorithm: AlgorithmIdentifier<'a>,
    pub subject_public_key: BitString<'a>,
    /// A raw unparsed PKIX, ASN.1 DER form (see RFC 5280, Section 4.1).
    ///
    /// Note: use the [`Self::parsed()`] function to parse this object.
    pub raw: &'a [u8],
}

impl<'a> SubjectPublicKeyInfo<'a> {
    /// Attempt to parse the public key, and return the parsed version or an error
    pub fn parsed(&self) -> Result<PublicKey, X509Error> {
        let b = &self.subject_public_key.data;
        if self.algorithm.algorithm == OID_PKCS1_RSAENCRYPTION {
            let (_, key) = RSAPublicKey::from_der(b).map_err(|_| X509Error::InvalidSPKI)?;
            Ok(PublicKey::RSA(key))
        } else if self.algorithm.algorithm == OID_KEY_TYPE_EC_PUBLIC_KEY {
            let key = ECPoint::from(b.as_ref());
            Ok(PublicKey::EC(key))
        } else if self.algorithm.algorithm == OID_KEY_TYPE_DSA {
            let s = parse_der_integer(b)
                .and_then(|(_, obj)| obj.as_slice().map_err(Err::Error))
                .or(Err(X509Error::InvalidSPKI))?;
            Ok(PublicKey::DSA(s))
        } else if self.algorithm.algorithm == OID_GOST_R3410_2001 {
            let (_, s) = <&[u8]>::from_der(b).or(Err(X509Error::InvalidSPKI))?;
            Ok(PublicKey::GostR3410(s))
        } else if self.algorithm.algorithm == OID_KEY_TYPE_GOST_R3410_2012_256
            || self.algorithm.algorithm == OID_KEY_TYPE_GOST_R3410_2012_512
        {
            let (_, s) = <&[u8]>::from_der(b).or(Err(X509Error::InvalidSPKI))?;
            Ok(PublicKey::GostR3410_2012(s))
        } else {
            Ok(PublicKey::Unknown(b))
        }
    }
}

impl<'a> FromDer<'a, X509Error> for SubjectPublicKeyInfo<'a> {
    /// Parse the SubjectPublicKeyInfo struct portion of a DER-encoded X.509 Certificate
    fn from_der(i: &'a [u8]) -> X509Result<Self> {
        let start_i = i;
        parse_der_sequence_defined_g(move |i, _| {
            let (i, algorithm) = AlgorithmIdentifier::from_der(i)?;
            let (i, subject_public_key) = BitString::from_der(i).or(Err(X509Error::InvalidSPKI))?;
            let len = start_i.offset(i);
            let raw = &start_i[..len];
            let spki = SubjectPublicKeyInfo {
                algorithm,
                subject_public_key,
                raw,
            };
            Ok((i, spki))
        })(i)
    }
}

/// Algorithm identifier
///
/// An algorithm identifier is defined by the following ASN.1 structure:
///
/// <pre>
/// AlgorithmIdentifier  ::=  SEQUENCE  {
///      algorithm               OBJECT IDENTIFIER,
///      parameters              ANY DEFINED BY algorithm OPTIONAL  }
/// </pre>
///
/// The algorithm identifier is used to identify a cryptographic
/// algorithm.  The OBJECT IDENTIFIER component identifies the algorithm
/// (such as DSA with SHA-1).  The contents of the optional parameters
/// field will vary according to the algorithm identified.
#[derive(Clone, Debug, PartialEq, DerSequence)]
#[error(X509Error)]
pub struct AlgorithmIdentifier<'a> {
    #[map_err(|_| X509Error::InvalidAlgorithmIdentifier)]
    pub algorithm: Oid<'a>,
    #[optional]
    pub parameters: Option<Any<'a>>,
}

impl<'a> AlgorithmIdentifier<'a> {
    /// Create a new `AlgorithmIdentifier`
    pub const fn new(algorithm: Oid<'a>, parameters: Option<Any<'a>>) -> Self {
        Self {
            algorithm,
            parameters,
        }
    }

    /// Get the algorithm OID
    pub const fn oid(&'a self) -> &'a Oid {
        &self.algorithm
    }

    /// Get a reference to the algorithm parameters, if present
    pub const fn parameters(&'a self) -> Option<&'a Any> {
        self.parameters.as_ref()
    }
}

/// X.509 Name (as used in `Issuer` and `Subject` fields)
///
/// The Name describes a hierarchical name composed of attributes, such
/// as country name, and corresponding values, such as US.  The type of
/// the component AttributeValue is determined by the AttributeType; in
/// general it will be a DirectoryString.
#[derive(Clone, Debug, PartialEq)]
pub struct X509Name<'a> {
    pub(crate) rdn_seq: Vec<RelativeDistinguishedName<'a>>,
    pub(crate) raw: &'a [u8],
}

impl<'a> fmt::Display for X509Name<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match x509name_to_string(&self.rdn_seq, oid_registry()) {
            Ok(o) => write!(f, "{}", o),
            Err(_) => write!(f, "<X509Error: Invalid X.509 name>"),
        }
    }
}

impl<'a> X509Name<'a> {
    /// Builds a new `X509Name` from the provided elements.
    #[inline]
    pub const fn new(rdn_seq: Vec<RelativeDistinguishedName<'a>>, raw: &'a [u8]) -> Self {
        X509Name { rdn_seq, raw }
    }

    /// Attempt to format the current name, using the given registry to convert OIDs to strings.
    ///
    /// Note: a default registry is provided with this crate, and is returned by the
    /// [`oid_registry()`] method.
    pub fn to_string_with_registry(&self, oid_registry: &OidRegistry) -> Result<String, X509Error> {
        x509name_to_string(&self.rdn_seq, oid_registry)
    }

    // Not using the AsRef trait, as that would not give back the full 'a lifetime
    pub fn as_raw(&self) -> &'a [u8] {
        self.raw
    }

    /// Return an iterator over the `RelativeDistinguishedName` components of the name
    pub fn iter(&self) -> impl Iterator<Item = &RelativeDistinguishedName<'a>> {
        self.rdn_seq.iter()
    }

    /// Return an iterator over the `RelativeDistinguishedName` components of the name
    pub fn iter_rdn(&self) -> impl Iterator<Item = &RelativeDistinguishedName<'a>> {
        self.rdn_seq.iter()
    }

    /// Return an iterator over the attribute types and values of the name
    pub fn iter_attributes(&self) -> impl Iterator<Item = &AttributeTypeAndValue<'a>> {
        self.rdn_seq.iter().flat_map(|rdn| rdn.set.iter())
    }

    /// Return an iterator over the components identified by the given OID
    ///
    /// The type of the component AttributeValue is determined by the AttributeType; in
    /// general it will be a DirectoryString.
    ///
    /// Attributes with same OID may be present multiple times, so the returned object is
    /// an iterator.
    /// Expected number of objects in this iterator are
    ///   - 0: not found
    ///   - 1: present once (common case)
    ///   - 2 or more: attribute is present multiple times
    pub fn iter_by_oid(&self, oid: &Oid<'a>) -> impl Iterator<Item = &AttributeTypeAndValue<'a>> {
        // this is necessary, otherwise rustc complains
        // that caller creates a temporary value for reference (for ex.
        // `self.iter_by_oid(&OID_X509_LOCALITY_NAME)`
        // )
        let oid = oid.clone();
        self.iter_attributes()
            .filter(move |obj| obj.attr_type == oid)
    }

    /// Return an iterator over the `CommonName` attributes of the X.509 Name.
    ///
    /// Returned iterator can be empty if there are no `CommonName` attributes.
    /// If you expect only one `CommonName` to be present, then using `next()` will
    /// get an `Option<&AttributeTypeAndValue>`.
    ///
    /// A common operation is to extract the `CommonName` as a string.
    ///
    /// ```
    /// use x509_parser::x509::X509Name;
    ///
    /// fn get_first_cn_as_str<'a>(name: &'a X509Name<'_>) -> Option<&'a str> {
    ///     name.iter_common_name()
    ///         .next()
    ///         .and_then(|cn| cn.as_str().ok())
    /// }
    /// ```
    ///
    /// Note that there are multiple reasons for failure or incorrect behavior, for ex. if
    /// the attribute is present multiple times, or is not a UTF-8 encoded string (it can be
    /// UTF-16, or even an OCTETSTRING according to the standard).
    pub fn iter_common_name(&self) -> impl Iterator<Item = &AttributeTypeAndValue<'a>> {
        self.iter_by_oid(&OID_X509_COMMON_NAME)
    }

    /// Return an iterator over the `Country` attributes of the X.509 Name.
    pub fn iter_country(&self) -> impl Iterator<Item = &AttributeTypeAndValue<'a>> {
        self.iter_by_oid(&OID_X509_COUNTRY_NAME)
    }

    /// Return an iterator over the `Organization` attributes of the X.509 Name.
    pub fn iter_organization(&self) -> impl Iterator<Item = &AttributeTypeAndValue<'a>> {
        self.iter_by_oid(&OID_X509_ORGANIZATION_NAME)
    }

    /// Return an iterator over the `OrganizationalUnit` attributes of the X.509 Name.
    pub fn iter_organizational_unit(&self) -> impl Iterator<Item = &AttributeTypeAndValue<'a>> {
        self.iter_by_oid(&OID_X509_ORGANIZATIONAL_UNIT)
    }

    /// Return an iterator over the `StateOrProvinceName` attributes of the X.509 Name.
    pub fn iter_state_or_province(&self) -> impl Iterator<Item = &AttributeTypeAndValue<'a>> {
        self.iter_by_oid(&OID_X509_STATE_OR_PROVINCE_NAME)
    }

    /// Return an iterator over the `Locality` attributes of the X.509 Name.
    pub fn iter_locality(&self) -> impl Iterator<Item = &AttributeTypeAndValue<'a>> {
        self.iter_by_oid(&OID_X509_LOCALITY_NAME)
    }

    /// Return an iterator over the `EmailAddress` attributes of the X.509 Name.
    pub fn iter_email(&self) -> impl Iterator<Item = &AttributeTypeAndValue<'a>> {
        self.iter_by_oid(&OID_PKCS9_EMAIL_ADDRESS)
    }
}

impl<'a> FromIterator<RelativeDistinguishedName<'a>> for X509Name<'a> {
    fn from_iter<T: IntoIterator<Item = RelativeDistinguishedName<'a>>>(iter: T) -> Self {
        let rdn_seq = iter.into_iter().collect();
        X509Name { rdn_seq, raw: &[] }
    }
}

impl<'a> From<X509Name<'a>> for Vec<RelativeDistinguishedName<'a>> {
    fn from(name: X509Name<'a>) -> Self {
        name.rdn_seq
    }
}

impl<'a> FromDer<'a, X509Error> for X509Name<'a> {
    /// Parse the X.501 type Name, used for ex in issuer and subject of a X.509 certificate
    fn from_der(i: &'a [u8]) -> X509Result<Self> {
        let start_i = i;
        parse_der_sequence_defined_g(move |i, _| {
            let (i, rdn_seq) = many0(complete(RelativeDistinguishedName::from_der))(i)?;
            let len = start_i.offset(i);
            let name = X509Name {
                rdn_seq,
                raw: &start_i[..len],
            };
            Ok((i, name))
        })(i)
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct ReasonCode(pub u8);

newtype_enum! {
impl display ReasonCode {
    Unspecified = 0,
    KeyCompromise = 1,
    CACompromise = 2,
    AffiliationChanged = 3,
    Superseded = 4,
    CessationOfOperation = 5,
    CertificateHold = 6,
    // value 7 is not used
    RemoveFromCRL = 8,
    PrivilegeWithdrawn = 9,
    AACompromise = 10,
}
}

impl Default for ReasonCode {
    fn default() -> Self {
        ReasonCode::Unspecified
    }
}

// Attempt to convert attribute to string. If type is not a string, return value is the hex
// encoding of the attribute value
fn attribute_value_to_string(attr: &Any, _attr_type: &Oid) -> Result<String, X509Error> {
    // TODO: replace this with helper function, when it is added to asn1-rs
    match attr.tag() {
        Tag::NumericString
        | Tag::BmpString
        | Tag::VisibleString
        | Tag::PrintableString
        | Tag::GeneralString
        | Tag::ObjectDescriptor
        | Tag::GraphicString
        | Tag::T61String
        | Tag::VideotexString
        | Tag::Utf8String
        | Tag::Ia5String => {
            let s = core::str::from_utf8(attr.data).map_err(|_| X509Error::InvalidAttributes)?;
            Ok(s.to_owned())
        }
        _ => {
            // type is not a string, get slice and convert it to base64
            Ok(HEXUPPER.encode(attr.as_bytes()))
        }
    }
}

/// Convert a DER representation of a X.509 name to a human-readable string
///
/// RDNs are separated with ","
/// Multiple RDNs are separated with "+"
///
/// Attributes that cannot be represented by a string are hex-encoded
fn x509name_to_string(
    rdn_seq: &[RelativeDistinguishedName],
    oid_registry: &OidRegistry,
) -> Result<String, X509Error> {
    rdn_seq.iter().fold(Ok(String::new()), |acc, rdn| {
        acc.and_then(|mut _vec| {
            rdn.set
                .iter()
                .fold(Ok(String::new()), |acc2, attr| {
                    acc2.and_then(|mut _vec2| {
                        let val_str = attribute_value_to_string(&attr.attr_value, &attr.attr_type)?;
                        // look ABBREV, and if not found, use shortname
                        let abbrev = match oid2abbrev(&attr.attr_type, oid_registry) {
                            Ok(s) => String::from(s),
                            _ => format!("{:?}", attr.attr_type),
                        };
                        let rdn = format!("{}={}", abbrev, val_str);
                        match _vec2.len() {
                            0 => Ok(rdn),
                            _ => Ok(_vec2 + " + " + &rdn),
                        }
                    })
                })
                .map(|v| match _vec.len() {
                    0 => v,
                    _ => _vec + ", " + &v,
                })
        })
    })
}

pub(crate) fn parse_signature_value(i: &[u8]) -> X509Result<BitString> {
    BitString::from_der(i).or(Err(Err::Error(X509Error::InvalidSignatureValue)))
}

pub(crate) fn parse_serial(i: &[u8]) -> X509Result<(&[u8], BigUint)> {
    let (rem, any) = Any::from_ber(i).map_err(|_| X509Error::InvalidSerial)?;
    // RFC 5280 4.1.2.2: "The serial number MUST be a positive integer"
    // however, many CAs do not respect this and send integers with MSB set,
    // so we do not use `as_biguint()`
    any.tag()
        .assert_eq(Tag::Integer)
        .map_err(|_| X509Error::InvalidSerial)?;
    let slice = any.data;
    let big = BigUint::from_bytes_be(slice);
    Ok((rem, (slice, big)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_x509_name() {
        let name = X509Name {
            rdn_seq: vec![
                RelativeDistinguishedName {
                    set: vec![AttributeTypeAndValue {
                        attr_type: oid! {2.5.4.6}, // countryName
                        attr_value: Any::from_tag_and_data(Tag::PrintableString, b"FR"),
                    }],
                },
                RelativeDistinguishedName {
                    set: vec![AttributeTypeAndValue {
                        attr_type: oid! {2.5.4.8}, // stateOrProvinceName
                        attr_value: Any::from_tag_and_data(Tag::PrintableString, b"Some-State"),
                    }],
                },
                RelativeDistinguishedName {
                    set: vec![AttributeTypeAndValue {
                        attr_type: oid! {2.5.4.10}, // organizationName
                        attr_value: Any::from_tag_and_data(
                            Tag::PrintableString,
                            b"Internet Widgits Pty Ltd",
                        ),
                    }],
                },
                RelativeDistinguishedName {
                    set: vec![
                        AttributeTypeAndValue {
                            attr_type: oid! {2.5.4.3}, // CN
                            attr_value: Any::from_tag_and_data(Tag::PrintableString, b"Test1"),
                        },
                        AttributeTypeAndValue {
                            attr_type: oid! {2.5.4.3}, // CN
                            attr_value: Any::from_tag_and_data(Tag::PrintableString, b"Test2"),
                        },
                    ],
                },
            ],
            raw: &[], // incorrect, but enough for testing
        };
        assert_eq!(
            name.to_string(),
            "C=FR, ST=Some-State, O=Internet Widgits Pty Ltd, CN=Test1 + CN=Test2"
        );
    }
}
