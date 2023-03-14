//! X.509 Extensions objects and types

use crate::error::{X509Error, X509Result};
use crate::time::ASN1Time;
use crate::utils::format_serial;
use crate::x509::{ReasonCode, RelativeDistinguishedName};

use asn1_rs::FromDer;
use der_parser::ber::parse_ber_bool;
use der_parser::der::*;
use der_parser::error::{BerError, BerResult};
use der_parser::num_bigint::BigUint;
use der_parser::oid::Oid;
use nom::combinator::{all_consuming, complete, cut, map, map_res, opt};
use nom::multi::{many0, many1};
use nom::{Err, IResult, Parser};
use oid_registry::*;
use std::collections::HashMap;
use std::fmt::{self, LowerHex};

mod generalname;
mod keyusage;
mod nameconstraints;
mod policymappings;
mod sct;

pub use generalname::*;
pub use keyusage::*;
pub use nameconstraints::*;
pub use policymappings::*;
pub use sct::*;

/// X.509 version 3 extension
///
/// X.509 extensions allow adding attributes to objects like certificates or revocation lists.
///
/// Each extension in a certificate is designated as either critical or non-critical.  A
/// certificate using system MUST reject the certificate if it encounters a critical extension it
/// does not recognize; however, a non-critical extension MAY be ignored if it is not recognized.
///
/// Each extension includes an OID and an ASN.1 structure.  When an extension appears in a
/// certificate, the OID appears as the field extnID and the corresponding ASN.1 encoded structure
/// is the value of the octet string extnValue.  A certificate MUST NOT include more than one
/// instance of a particular extension.
///
/// When parsing an extension, the global extension structure (described above) is parsed,
/// and the object is returned if it succeeds.
/// During this step, it also attempts to parse the content of the extension, if known.
/// The returned object has a
/// [`X509Extension::parsed_extension()`] method. The returned
/// enum is either a known extension, or the special value `ParsedExtension::UnsupportedExtension`.
///
/// # Example
///
/// ```rust
/// use x509_parser::prelude::FromDer;
/// use x509_parser::extensions::{X509Extension, ParsedExtension};
///
/// static DER: &[u8] = &[
///    0x30, 0x1D, 0x06, 0x03, 0x55, 0x1D, 0x0E, 0x04, 0x16, 0x04, 0x14, 0xA3, 0x05, 0x2F, 0x18,
///    0x60, 0x50, 0xC2, 0x89, 0x0A, 0xDD, 0x2B, 0x21, 0x4F, 0xFF, 0x8E, 0x4E, 0xA8, 0x30, 0x31,
///    0x36 ];
///
/// # fn main() {
/// let res = X509Extension::from_der(DER);
/// match res {
///     Ok((_rem, ext)) => {
///         println!("Extension OID: {}", ext.oid);
///         println!("  Critical: {}", ext.critical);
///         let parsed_ext = ext.parsed_extension();
///         assert!(!parsed_ext.unsupported());
///         assert!(parsed_ext.error().is_none());
///         if let ParsedExtension::SubjectKeyIdentifier(key_id) = parsed_ext {
///             assert!(key_id.0.len() > 0);
///         } else {
///             panic!("Extension has wrong type");
///         }
///     },
///     _ => panic!("x509 extension parsing failed: {:?}", res),
/// }
/// # }
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct X509Extension<'a> {
    /// OID describing the extension content
    pub oid: Oid<'a>,
    /// Boolean value describing the 'critical' attribute of the extension
    ///
    /// An extension includes the boolean critical, with a default value of FALSE.
    pub critical: bool,
    /// Raw content of the extension
    pub value: &'a [u8],
    pub(crate) parsed_extension: ParsedExtension<'a>,
}

impl<'a> X509Extension<'a> {
    /// Creates a new extension with the provided values.
    #[inline]
    pub const fn new(
        oid: Oid<'a>,
        critical: bool,
        value: &'a [u8],
        parsed_extension: ParsedExtension<'a>,
    ) -> X509Extension<'a> {
        X509Extension {
            oid,
            critical,
            value,
            parsed_extension,
        }
    }

    /// Return the extension type or `UnsupportedExtension` if the extension is not implemented.
    #[inline]
    pub fn parsed_extension(&self) -> &ParsedExtension<'a> {
        &self.parsed_extension
    }
}

/// <pre>
/// Extension  ::=  SEQUENCE  {
///     extnID      OBJECT IDENTIFIER,
///     critical    BOOLEAN DEFAULT FALSE,
///     extnValue   OCTET STRING  }
/// </pre>
impl<'a> FromDer<'a, X509Error> for X509Extension<'a> {
    fn from_der(i: &'a [u8]) -> X509Result<Self> {
        X509ExtensionParser::new().parse(i)
    }
}

/// `X509Extension` parser builder
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct X509ExtensionParser {
    deep_parse_extensions: bool,
}

impl X509ExtensionParser {
    #[inline]
    pub const fn new() -> Self {
        X509ExtensionParser {
            deep_parse_extensions: true,
        }
    }

    #[inline]
    pub const fn with_deep_parse_extensions(self, deep_parse_extensions: bool) -> Self {
        X509ExtensionParser {
            deep_parse_extensions,
        }
    }
}

impl<'a> Parser<&'a [u8], X509Extension<'a>, X509Error> for X509ExtensionParser {
    fn parse(&mut self, input: &'a [u8]) -> IResult<&'a [u8], X509Extension<'a>, X509Error> {
        parse_der_sequence_defined_g(|i, _| {
            let (i, oid) = Oid::from_der(i)?;
            let (i, critical) = der_read_critical(i)?;
            let (i, value) = <&[u8]>::from_der(i)?;
            let (i, parsed_extension) = if self.deep_parse_extensions {
                parser::parse_extension(i, value, &oid)?
            } else {
                (&[] as &[_], ParsedExtension::Unparsed)
            };
            let ext = X509Extension {
                oid,
                critical,
                value,
                parsed_extension,
            };
            Ok((i, ext))
        })(input)
        .map_err(|_| X509Error::InvalidExtensions.into())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ParsedExtension<'a> {
    /// Crate parser does not support this extension (yet)
    UnsupportedExtension {
        oid: Oid<'a>,
    },
    ParseError {
        error: Err<BerError>,
    },
    /// Section 4.2.1.1 of rfc 5280
    AuthorityKeyIdentifier(AuthorityKeyIdentifier<'a>),
    /// Section 4.2.1.2 of rfc 5280
    SubjectKeyIdentifier(KeyIdentifier<'a>),
    /// Section 4.2.1.3 of rfc 5280
    KeyUsage(KeyUsage),
    /// Section 4.2.1.4 of rfc 5280
    CertificatePolicies(CertificatePolicies<'a>),
    /// Section 4.2.1.5 of rfc 5280
    PolicyMappings(PolicyMappings<'a>),
    /// Section 4.2.1.6 of rfc 5280
    SubjectAlternativeName(SubjectAlternativeName<'a>),
    /// Section 4.2.1.7 of rfc 5280
    IssuerAlternativeName(IssuerAlternativeName<'a>),
    /// Section 4.2.1.9 of rfc 5280
    BasicConstraints(BasicConstraints),
    /// Section 4.2.1.10 of rfc 5280
    NameConstraints(NameConstraints<'a>),
    /// Section 4.2.1.11 of rfc 5280
    PolicyConstraints(PolicyConstraints),
    /// Section 4.2.1.12 of rfc 5280
    ExtendedKeyUsage(ExtendedKeyUsage<'a>),
    /// Section 4.2.1.13 of rfc 5280
    CRLDistributionPoints(CRLDistributionPoints<'a>),
    /// Section 4.2.1.14 of rfc 5280
    InhibitAnyPolicy(InhibitAnyPolicy),
    /// Section 4.2.2.1 of rfc 5280
    AuthorityInfoAccess(AuthorityInfoAccess<'a>),
    /// Netscape certificate type (subject is SSL client, an SSL server, or a CA)
    NSCertType(NSCertType),
    /// Netscape certificate comment
    NsCertComment(&'a str),
    /// Section 5.3.1 of rfc 5280
    CRLNumber(BigUint),
    /// Section 5.3.1 of rfc 5280
    ReasonCode(ReasonCode),
    /// Section 5.3.3 of rfc 5280
    InvalidityDate(ASN1Time),
    /// rfc 6962
    SCT(Vec<SignedCertificateTimestamp<'a>>),
    /// Unparsed extension (was not requested in parsing options)
    Unparsed,
}

impl<'a> ParsedExtension<'a> {
    /// Return `true` if the extension is unsupported
    pub fn unsupported(&self) -> bool {
        matches!(self, &ParsedExtension::UnsupportedExtension { .. })
    }

    /// Return a reference on the parsing error if the extension parsing failed
    pub fn error(&self) -> Option<&Err<BerError>> {
        match self {
            ParsedExtension::ParseError { error } => Some(error),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AuthorityKeyIdentifier<'a> {
    pub key_identifier: Option<KeyIdentifier<'a>>,
    pub authority_cert_issuer: Option<Vec<GeneralName<'a>>>,
    pub authority_cert_serial: Option<&'a [u8]>,
}

impl<'a> FromDer<'a, X509Error> for AuthorityKeyIdentifier<'a> {
    fn from_der(i: &'a [u8]) -> X509Result<'a, Self> {
        parser::parse_authoritykeyidentifier(i).map_err(Err::convert)
    }
}

pub type CertificatePolicies<'a> = Vec<PolicyInformation<'a>>;

// impl<'a> FromDer<'a> for CertificatePolicies<'a> {
//     fn from_der(i: &'a [u8]) -> X509Result<'a, Self> {
//         parser::parse_certificatepolicies(i).map_err(Err::convert)
//     }
// }

#[derive(Clone, Debug, PartialEq)]
pub struct PolicyInformation<'a> {
    pub policy_id: Oid<'a>,
    pub policy_qualifiers: Option<Vec<PolicyQualifierInfo<'a>>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PolicyQualifierInfo<'a> {
    pub policy_qualifier_id: Oid<'a>,
    pub qualifier: &'a [u8],
}

/// Identifies whether the subject of the certificate is a CA, and the max validation depth.
#[derive(Clone, Debug, PartialEq)]
pub struct BasicConstraints {
    pub ca: bool,
    pub path_len_constraint: Option<u32>,
}

impl<'a> FromDer<'a, X509Error> for BasicConstraints {
    fn from_der(i: &'a [u8]) -> X509Result<'a, Self> {
        parser::parse_basicconstraints(i).map_err(Err::convert)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct KeyIdentifier<'a>(pub &'a [u8]);

impl<'a> FromDer<'a, X509Error> for KeyIdentifier<'a> {
    fn from_der(i: &'a [u8]) -> X509Result<'a, Self> {
        parser::parse_keyidentifier(i).map_err(Err::convert)
    }
}

impl<'a> LowerHex for KeyIdentifier<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = format_serial(self.0);
        f.write_str(&s)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NSCertType(u8);

// The value is a bit-string, where the individual bit positions are defined as:
//
//     bit-0 SSL client - this cert is certified for SSL client authentication use
//     bit-1 SSL server - this cert is certified for SSL server authentication use
//     bit-2 S/MIME - this cert is certified for use by clients (New in PR3)
//     bit-3 Object Signing - this cert is certified for signing objects such as Java applets and plugins(New in PR3)
//     bit-4 Reserved - this bit is reserved for future use
//     bit-5 SSL CA - this cert is certified for issuing certs for SSL use
//     bit-6 S/MIME CA - this cert is certified for issuing certs for S/MIME use (New in PR3)
//     bit-7 Object Signing CA - this cert is certified for issuing certs for Object Signing (New in PR3)
impl NSCertType {
    pub fn ssl_client(&self) -> bool {
        self.0 & 0x1 == 1
    }
    pub fn ssl_server(&self) -> bool {
        (self.0 >> 1) & 1 == 1
    }
    pub fn smime(&self) -> bool {
        (self.0 >> 2) & 1 == 1
    }
    pub fn object_signing(&self) -> bool {
        (self.0 >> 3) & 1 == 1
    }
    pub fn ssl_ca(&self) -> bool {
        (self.0 >> 5) & 1 == 1
    }
    pub fn smime_ca(&self) -> bool {
        (self.0 >> 6) & 1 == 1
    }
    pub fn object_signing_ca(&self) -> bool {
        (self.0 >> 7) & 1 == 1
    }
}

const NS_CERT_TYPE_FLAGS: &[&str] = &[
    "SSL CLient",
    "SSL Server",
    "S/MIME",
    "Object Signing",
    "Reserved",
    "SSL CA",
    "S/MIME CA",
    "Object Signing CA",
];

impl fmt::Display for NSCertType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut s = String::new();
        let mut acc = self.0;
        for flag_text in NS_CERT_TYPE_FLAGS {
            if acc & 1 != 0 {
                s = s + flag_text + ", ";
            }
            acc >>= 1;
        }
        s.pop();
        s.pop();
        f.write_str(&s)
    }
}

impl<'a> FromDer<'a, X509Error> for NSCertType {
    fn from_der(i: &'a [u8]) -> X509Result<'a, Self> {
        parser::parse_nscerttype(i).map_err(Err::convert)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AuthorityInfoAccess<'a> {
    pub accessdescs: Vec<AccessDescription<'a>>,
}

impl<'a> AuthorityInfoAccess<'a> {
    /// Returns an iterator over the Access Descriptors
    pub fn iter(&self) -> impl Iterator<Item = &AccessDescription<'a>> {
        self.accessdescs.iter()
    }

    /// Returns a `HashMap` mapping `Oid` to the list of references to `GeneralNames`
    ///
    /// If several names match the same `Oid`, they are merged in the same entry.
    pub fn as_hashmap(&self) -> HashMap<Oid<'a>, Vec<&GeneralName<'a>>> {
        // create the hashmap and merge entries with same OID
        let mut m: HashMap<Oid, Vec<&GeneralName>> = HashMap::new();
        for desc in &self.accessdescs {
            let AccessDescription {
                access_method: oid,
                access_location: gn,
            } = desc;
            if let Some(general_names) = m.get_mut(oid) {
                general_names.push(gn);
            } else {
                m.insert(oid.clone(), vec![gn]);
            }
        }
        m
    }

    /// Returns a `HashMap` mapping `Oid` to the list of `GeneralNames` (consuming the input)
    ///
    /// If several names match the same `Oid`, they are merged in the same entry.
    pub fn into_hashmap(self) -> HashMap<Oid<'a>, Vec<GeneralName<'a>>> {
        let mut aia_list = self.accessdescs;
        // create the hashmap and merge entries with same OID
        let mut m: HashMap<Oid, Vec<GeneralName>> = HashMap::new();
        for desc in aia_list.drain(..) {
            let AccessDescription {
                access_method: oid,
                access_location: gn,
            } = desc;
            if let Some(general_names) = m.get_mut(&oid) {
                general_names.push(gn);
            } else {
                m.insert(oid, vec![gn]);
            }
        }
        m
    }
}

impl<'a> FromDer<'a, X509Error> for AuthorityInfoAccess<'a> {
    fn from_der(i: &'a [u8]) -> X509Result<'a, Self> {
        parser::parse_authorityinfoaccess(i).map_err(Err::convert)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AccessDescription<'a> {
    pub access_method: Oid<'a>,
    pub access_location: GeneralName<'a>,
}

impl<'a> AccessDescription<'a> {
    pub const fn new(access_method: Oid<'a>, access_location: GeneralName<'a>) -> Self {
        AccessDescription {
            access_method,
            access_location,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct InhibitAnyPolicy {
    pub skip_certs: u32,
}

impl<'a> FromDer<'a, X509Error> for InhibitAnyPolicy {
    fn from_der(i: &'a [u8]) -> X509Result<'a, Self> {
        map(parse_der_u32, |skip_certs| InhibitAnyPolicy { skip_certs })(i).map_err(Err::convert)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PolicyConstraints {
    pub require_explicit_policy: Option<u32>,
    pub inhibit_policy_mapping: Option<u32>,
}

impl<'a> FromDer<'a, X509Error> for PolicyConstraints {
    fn from_der(i: &'a [u8]) -> X509Result<'a, Self> {
        parser::parse_policyconstraints(i).map_err(Err::convert)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SubjectAlternativeName<'a> {
    pub general_names: Vec<GeneralName<'a>>,
}

impl<'a> FromDer<'a, X509Error> for SubjectAlternativeName<'a> {
    fn from_der(i: &'a [u8]) -> X509Result<'a, Self> {
        parse_der_sequence_defined_g(|input, _| {
            let (i, general_names) =
                all_consuming(many0(complete(cut(GeneralName::from_der))))(input)?;
            Ok((i, SubjectAlternativeName { general_names }))
        })(i)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct IssuerAlternativeName<'a> {
    pub general_names: Vec<GeneralName<'a>>,
}

impl<'a> FromDer<'a, X509Error> for IssuerAlternativeName<'a> {
    fn from_der(i: &'a [u8]) -> X509Result<'a, Self> {
        parse_der_sequence_defined_g(|input, _| {
            let (i, general_names) =
                all_consuming(many0(complete(cut(GeneralName::from_der))))(input)?;
            Ok((i, IssuerAlternativeName { general_names }))
        })(i)
    }
}

pub type CRLDistributionPoints<'a> = Vec<CRLDistributionPoint<'a>>;

// impl<'a> FromDer<'a> for CRLDistributionPoints<'a> {
//     fn from_der(i: &'a [u8]) -> X509Result<'a, Self> {
//         parser::parse_crldistributionpoints(i).map_err(Err::convert)
//     }
// }

#[derive(Clone, Debug, PartialEq)]
pub struct CRLDistributionPoint<'a> {
    pub distribution_point: Option<DistributionPointName<'a>>,
    pub reasons: Option<ReasonFlags>,
    pub crl_issuer: Option<Vec<GeneralName<'a>>>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum DistributionPointName<'a> {
    FullName(Vec<GeneralName<'a>>),
    NameRelativeToCRLIssuer(RelativeDistinguishedName<'a>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReasonFlags {
    pub flags: u16,
}

impl ReasonFlags {
    pub fn key_compromise(&self) -> bool {
        (self.flags >> 1) & 1 == 1
    }
    pub fn ca_compromise(&self) -> bool {
        (self.flags >> 2) & 1 == 1
    }
    pub fn affilation_changed(&self) -> bool {
        (self.flags >> 3) & 1 == 1
    }
    pub fn superseded(&self) -> bool {
        (self.flags >> 4) & 1 == 1
    }
    pub fn cessation_of_operation(&self) -> bool {
        (self.flags >> 5) & 1 == 1
    }
    pub fn certificate_hold(&self) -> bool {
        (self.flags >> 6) & 1 == 1
    }
    pub fn privelege_withdrawn(&self) -> bool {
        (self.flags >> 7) & 1 == 1
    }
    pub fn aa_compromise(&self) -> bool {
        (self.flags >> 8) & 1 == 1
    }
}

const REASON_FLAGS: &[&str] = &[
    "Unused",
    "Key Compromise",
    "CA Compromise",
    "Affiliation Changed",
    "Superseded",
    "Cessation Of Operation",
    "Certificate Hold",
    "Privilege Withdrawn",
    "AA Compromise",
];

impl fmt::Display for ReasonFlags {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut s = String::new();
        let mut acc = self.flags;
        for flag_text in REASON_FLAGS {
            if acc & 1 != 0 {
                s = s + flag_text + ", ";
            }
            acc >>= 1;
        }
        s.pop();
        s.pop();
        f.write_str(&s)
    }
}

pub(crate) mod parser {
    use crate::extensions::*;
    use crate::time::ASN1Time;
    use asn1_rs::{GeneralizedTime, ParseResult};
    use der_parser::error::BerError;
    use der_parser::{oid::Oid, *};
    use lazy_static::lazy_static;
    use nom::combinator::{cut, map};
    use nom::{Err, IResult};

    type ExtParser = fn(&[u8]) -> IResult<&[u8], ParsedExtension, BerError>;

    lazy_static! {
        static ref EXTENSION_PARSERS: HashMap<Oid<'static>, ExtParser> = {
            macro_rules! add {
                ($m:ident, $oid:ident, $p:ident) => {
                    $m.insert($oid, $p as ExtParser);
                };
            }

            let mut m = HashMap::new();
            add!(
                m,
                OID_X509_EXT_SUBJECT_KEY_IDENTIFIER,
                parse_keyidentifier_ext
            );
            add!(m, OID_X509_EXT_KEY_USAGE, parse_keyusage_ext);
            add!(
                m,
                OID_X509_EXT_SUBJECT_ALT_NAME,
                parse_subjectalternativename_ext
            );
            add!(
                m,
                OID_X509_EXT_ISSUER_ALT_NAME,
                parse_issueralternativename_ext
            );
            add!(
                m,
                OID_X509_EXT_BASIC_CONSTRAINTS,
                parse_basicconstraints_ext
            );
            add!(m, OID_X509_EXT_NAME_CONSTRAINTS, parse_nameconstraints_ext);
            add!(
                m,
                OID_X509_EXT_CERTIFICATE_POLICIES,
                parse_certificatepolicies_ext
            );
            add!(m, OID_X509_EXT_POLICY_MAPPINGS, parse_policymappings_ext);
            add!(
                m,
                OID_X509_EXT_POLICY_CONSTRAINTS,
                parse_policyconstraints_ext
            );
            add!(
                m,
                OID_X509_EXT_EXTENDED_KEY_USAGE,
                parse_extendedkeyusage_ext
            );
            add!(
                m,
                OID_X509_EXT_CRL_DISTRIBUTION_POINTS,
                parse_crldistributionpoints_ext
            );
            add!(
                m,
                OID_X509_EXT_INHIBITANT_ANY_POLICY,
                parse_inhibitanypolicy_ext
            );
            add!(
                m,
                OID_PKIX_AUTHORITY_INFO_ACCESS,
                parse_authorityinfoaccess_ext
            );
            add!(
                m,
                OID_X509_EXT_AUTHORITY_KEY_IDENTIFIER,
                parse_authoritykeyidentifier_ext
            );
            add!(m, OID_CT_LIST_SCT, parse_sct_ext);
            add!(m, OID_X509_EXT_CERT_TYPE, parse_nscerttype_ext);
            add!(m, OID_X509_EXT_CERT_COMMENT, parse_nscomment_ext);
            add!(m, OID_X509_EXT_CRL_NUMBER, parse_crl_number);
            add!(m, OID_X509_EXT_REASON_CODE, parse_reason_code);
            add!(m, OID_X509_EXT_INVALIDITY_DATE, parse_invalidity_date);
            m
        };
    }

    // look into the parser map if the extension is known, and parse it
    // otherwise, leave it as UnsupportedExtension
    fn parse_extension0<'a>(
        orig_i: &'a [u8],
        i: &'a [u8],
        oid: &Oid,
    ) -> IResult<&'a [u8], ParsedExtension<'a>, BerError> {
        if let Some(parser) = EXTENSION_PARSERS.get(oid) {
            match parser(i) {
                Ok((_, ext)) => Ok((orig_i, ext)),
                Err(error) => Ok((orig_i, ParsedExtension::ParseError { error })),
            }
        } else {
            Ok((
                orig_i,
                ParsedExtension::UnsupportedExtension {
                    oid: oid.to_owned(),
                },
            ))
        }
    }

    pub(crate) fn parse_extension<'a>(
        orig_i: &'a [u8],
        i: &'a [u8],
        oid: &Oid,
    ) -> IResult<&'a [u8], ParsedExtension<'a>, BerError> {
        parse_extension0(orig_i, i, oid)
    }

    /// Parse a "Basic Constraints" extension
    ///
    /// <pre>
    ///   id-ce-basicConstraints OBJECT IDENTIFIER ::=  { id-ce 19 }
    ///   BasicConstraints ::= SEQUENCE {
    ///        cA                      BOOLEAN DEFAULT FALSE,
    ///        pathLenConstraint       INTEGER (0..MAX) OPTIONAL }
    /// </pre>
    ///
    /// Note the maximum length of the `pathLenConstraint` field is limited to the size of a 32-bits
    /// unsigned integer, and parsing will fail if value if larger.
    pub(super) fn parse_basicconstraints(i: &[u8]) -> IResult<&[u8], BasicConstraints, BerError> {
        let (rem, obj) = parse_der_sequence(i)?;
        if let Ok(seq) = obj.as_sequence() {
            let (ca, path_len_constraint) = match seq.len() {
                0 => (false, None),
                1 => {
                    if let Ok(b) = seq[0].as_bool() {
                        (b, None)
                    } else if let Ok(u) = seq[0].as_u32() {
                        (false, Some(u))
                    } else {
                        return Err(nom::Err::Error(BerError::InvalidTag));
                    }
                }
                2 => {
                    let ca = seq[0]
                        .as_bool()
                        .or(Err(nom::Err::Error(BerError::InvalidLength)))?;
                    let pl = seq[1]
                        .as_u32()
                        .or(Err(nom::Err::Error(BerError::InvalidLength)))?;
                    (ca, Some(pl))
                }
                _ => return Err(nom::Err::Error(BerError::InvalidLength)),
            };
            Ok((
                rem,
                BasicConstraints {
                    ca,
                    path_len_constraint,
                },
            ))
        } else {
            Err(nom::Err::Error(BerError::InvalidLength))
        }
    }

    fn parse_basicconstraints_ext(i: &[u8]) -> IResult<&[u8], ParsedExtension, BerError> {
        map(parse_basicconstraints, ParsedExtension::BasicConstraints)(i)
    }

    fn parse_nameconstraints_ext<'a>(i: &'a [u8]) -> IResult<&'a [u8], ParsedExtension, BerError> {
        map(parse_nameconstraints, ParsedExtension::NameConstraints)(i)
    }

    pub(super) fn parse_subjectalternativename_ext<'a>(
        i: &'a [u8],
    ) -> IResult<&'a [u8], ParsedExtension, BerError> {
        parse_der_sequence_defined_g(|input, _| {
            let (i, general_names) = all_consuming(many0(complete(cut(parse_generalname))))(input)?;
            Ok((
                i,
                ParsedExtension::SubjectAlternativeName(SubjectAlternativeName { general_names }),
            ))
        })(i)
    }

    pub(super) fn parse_issueralternativename_ext<'a>(
        i: &'a [u8],
    ) -> IResult<&'a [u8], ParsedExtension, BerError> {
        parse_der_sequence_defined_g(|input, _| {
            let (i, general_names) = all_consuming(many0(complete(cut(parse_generalname))))(input)?;
            Ok((
                i,
                ParsedExtension::IssuerAlternativeName(IssuerAlternativeName { general_names }),
            ))
        })(i)
    }

    pub(super) fn parse_policyconstraints(i: &[u8]) -> IResult<&[u8], PolicyConstraints, BerError> {
        parse_der_sequence_defined_g(|input, _| {
            let (i, require_explicit_policy) = opt(complete(map_res(
                parse_der_tagged_implicit(0, parse_der_content(Tag::Integer)),
                |x| x.as_u32(),
            )))(input)?;
            let (i, inhibit_policy_mapping) = all_consuming(opt(complete(map_res(
                parse_der_tagged_implicit(1, parse_der_content(Tag::Integer)),
                |x| x.as_u32(),
            ))))(i)?;
            let policy_constraint = PolicyConstraints {
                require_explicit_policy,
                inhibit_policy_mapping,
            };
            Ok((i, policy_constraint))
        })(i)
    }

    fn parse_policyconstraints_ext(i: &[u8]) -> IResult<&[u8], ParsedExtension, BerError> {
        map(parse_policyconstraints, ParsedExtension::PolicyConstraints)(i)
    }

    fn parse_policymappings_ext(i: &[u8]) -> IResult<&[u8], ParsedExtension, BerError> {
        map(parse_policymappings, ParsedExtension::PolicyMappings)(i)
    }

    fn parse_inhibitanypolicy_ext(i: &[u8]) -> IResult<&[u8], ParsedExtension, BerError> {
        let (ret, skip_certs) = parse_der_u32(i)?;
        Ok((
            ret,
            ParsedExtension::InhibitAnyPolicy(InhibitAnyPolicy { skip_certs }),
        ))
    }

    fn parse_extendedkeyusage_ext(i: &[u8]) -> IResult<&[u8], ParsedExtension, BerError> {
        map(parse_extendedkeyusage, ParsedExtension::ExtendedKeyUsage)(i)
    }

    // DistributionPointName ::= CHOICE {
    //     fullName                [0]     GeneralNames,
    //     nameRelativeToCRLIssuer [1]     RelativeDistinguishedName }
    fn parse_distributionpointname(i: &[u8]) -> IResult<&[u8], DistributionPointName, BerError> {
        let (rem, header) = der_read_element_header(i)?;
        match header.tag().0 {
            0 => {
                let (rem, names) = many1(complete(parse_generalname))(rem)?;
                Ok((rem, DistributionPointName::FullName(names)))
            }
            1 => {
                let (rem, rdn) = RelativeDistinguishedName::from_der(rem)
                    .map_err(|_| BerError::BerValueError)?;
                Ok((rem, DistributionPointName::NameRelativeToCRLIssuer(rdn)))
            }
            _ => Err(Err::Error(BerError::InvalidTag)),
        }
    }

    // ReasonFlags ::= BIT STRING {
    // unused                  (0),
    // keyCompromise           (1),
    // cACompromise            (2),
    // affiliationChanged      (3),
    // superseded              (4),
    // cessationOfOperation    (5),
    // certificateHold         (6),
    // privilegeWithdrawn      (7),
    // aACompromise            (8) }
    fn parse_tagged1_reasons(i: &[u8]) -> BerResult<ReasonFlags> {
        let (rem, obj) = parse_der_tagged_implicit(1, parse_der_content(Tag::BitString))(i)?;
        if let DerObjectContent::BitString(_, b) = obj.content {
            let flags = b
                .data
                .iter()
                .rev()
                .fold(0, |acc, x| acc << 8 | (x.reverse_bits() as u16));
            Ok((rem, ReasonFlags { flags }))
        } else {
            Err(nom::Err::Failure(BerError::InvalidTag))
        }
    }

    fn parse_crlissuer_content(i: &[u8]) -> BerResult<Vec<GeneralName>> {
        many1(complete(parse_generalname))(i)
    }

    // DistributionPoint ::= SEQUENCE {
    //     distributionPoint       [0]     DistributionPointName OPTIONAL,
    //     reasons                 [1]     ReasonFlags OPTIONAL,
    //     cRLIssuer               [2]     GeneralNames OPTIONAL }
    pub(super) fn parse_crldistributionpoint(
        i: &[u8],
    ) -> IResult<&[u8], CRLDistributionPoint, BerError> {
        parse_der_sequence_defined_g(|content, _| {
            let (rem, distribution_point) =
                opt(complete(parse_der_tagged_explicit_g(0, |b, _| {
                    parse_distributionpointname(b)
                })))(content)?;
            let (rem, reasons) = opt(complete(parse_tagged1_reasons))(rem)?;
            let (rem, crl_issuer) = opt(complete(parse_der_tagged_implicit_g(2, |i, _, _| {
                parse_crlissuer_content(i)
            })))(rem)?;
            let crl_dp = CRLDistributionPoint {
                distribution_point,
                reasons,
                crl_issuer,
            };
            Ok((rem, crl_dp))
        })(i)
    }

    pub(super) fn parse_crldistributionpoints(
        i: &[u8],
    ) -> IResult<&[u8], CRLDistributionPoints, BerError> {
        parse_der_sequence_of_v(parse_crldistributionpoint)(i)
    }

    fn parse_crldistributionpoints_ext(i: &[u8]) -> IResult<&[u8], ParsedExtension, BerError> {
        map(
            parse_crldistributionpoints,
            ParsedExtension::CRLDistributionPoints,
        )(i)
    }

    // AuthorityInfoAccessSyntax  ::=
    //         SEQUENCE SIZE (1..MAX) OF AccessDescription
    //
    // AccessDescription  ::=  SEQUENCE {
    //         accessMethod          OBJECT IDENTIFIER,
    //         accessLocation        GeneralName  }
    pub(super) fn parse_authorityinfoaccess(
        i: &[u8],
    ) -> IResult<&[u8], AuthorityInfoAccess, BerError> {
        fn parse_aia(i: &[u8]) -> IResult<&[u8], AccessDescription, BerError> {
            parse_der_sequence_defined_g(|content, _| {
                // Read first element, an oid.
                let (gn, oid) = Oid::from_der(content)?;
                // Parse second element
                let (rest, gn) = parse_generalname(gn)?;
                Ok((rest, AccessDescription::new(oid, gn)))
            })(i)
        }
        let (ret, accessdescs) = parse_der_sequence_of_v(parse_aia)(i)?;
        Ok((ret, AuthorityInfoAccess { accessdescs }))
    }

    fn parse_authorityinfoaccess_ext(i: &[u8]) -> IResult<&[u8], ParsedExtension, BerError> {
        map(
            parse_authorityinfoaccess,
            ParsedExtension::AuthorityInfoAccess,
        )(i)
    }

    fn parse_aki_content<'a>(
        i: &'a [u8],
        _hdr: Header<'_>,
    ) -> IResult<&'a [u8], AuthorityKeyIdentifier<'a>, BerError> {
        let (i, key_identifier) = opt(complete(parse_der_tagged_implicit_g(0, |d, _, _| {
            Ok((&[], KeyIdentifier(d)))
        })))(i)?;
        let (i, authority_cert_issuer) =
            opt(complete(parse_der_tagged_implicit_g(1, |d, _, _| {
                many0(complete(parse_generalname))(d)
            })))(i)?;
        let (i, authority_cert_serial) = opt(complete(parse_der_tagged_implicit(
            2,
            parse_der_content(Tag::Integer),
        )))(i)?;
        let authority_cert_serial = authority_cert_serial.and_then(|o| o.as_slice().ok());
        let aki = AuthorityKeyIdentifier {
            key_identifier,
            authority_cert_issuer,
            authority_cert_serial,
        };
        Ok((i, aki))
    }

    // RFC 5280 section 4.2.1.1: Authority Key Identifier
    pub(super) fn parse_authoritykeyidentifier(
        i: &[u8],
    ) -> IResult<&[u8], AuthorityKeyIdentifier, BerError> {
        let (rem, aki) = parse_der_sequence_defined_g(parse_aki_content)(i)?;
        Ok((rem, aki))
    }

    fn parse_authoritykeyidentifier_ext(i: &[u8]) -> IResult<&[u8], ParsedExtension, BerError> {
        map(
            parse_authoritykeyidentifier,
            ParsedExtension::AuthorityKeyIdentifier,
        )(i)
    }

    pub(super) fn parse_keyidentifier<'a>(
        i: &'a [u8],
    ) -> IResult<&'a [u8], KeyIdentifier, BerError> {
        let (rest, id) = <&[u8]>::from_der(i)?;
        let ki = KeyIdentifier(id);
        Ok((rest, ki))
    }

    fn parse_keyidentifier_ext<'a>(i: &'a [u8]) -> IResult<&'a [u8], ParsedExtension, BerError> {
        map(parse_keyidentifier, ParsedExtension::SubjectKeyIdentifier)(i)
    }

    fn parse_keyusage_ext(i: &[u8]) -> IResult<&[u8], ParsedExtension, BerError> {
        map(parse_keyusage, ParsedExtension::KeyUsage)(i)
    }

    pub(super) fn parse_nscerttype(i: &[u8]) -> IResult<&[u8], NSCertType, BerError> {
        let (rest, obj) = parse_der_bitstring(i)?;
        let bitstring = obj
            .content
            .as_bitstring()
            .or(Err(Err::Error(BerError::BerTypeError)))?;
        // bitstring should be 1 byte long
        if bitstring.data.len() != 1 {
            return Err(Err::Error(BerError::BerValueError));
        }
        let flags = bitstring.data[0].reverse_bits();
        Ok((rest, NSCertType(flags)))
    }

    fn parse_nscerttype_ext(i: &[u8]) -> IResult<&[u8], ParsedExtension, BerError> {
        map(parse_nscerttype, ParsedExtension::NSCertType)(i)
    }

    fn parse_nscomment_ext(i: &[u8]) -> IResult<&[u8], ParsedExtension, BerError> {
        match parse_der_ia5string(i) {
            Ok((i, obj)) => {
                let s = obj.as_str()?;
                Ok((i, ParsedExtension::NsCertComment(s)))
            }
            Err(e) => {
                // Some implementations encode the comment directly, without
                // wrapping it in an IA5String
                if let Ok(s) = std::str::from_utf8(i) {
                    Ok((&[], ParsedExtension::NsCertComment(s)))
                } else {
                    Err(e)
                }
            }
        }
    }

    // CertificatePolicies ::= SEQUENCE SIZE (1..MAX) OF PolicyInformation
    //
    // PolicyInformation ::= SEQUENCE {
    //      policyIdentifier   CertPolicyId,
    //      policyQualifiers   SEQUENCE SIZE (1..MAX) OF
    //              PolicyQualifierInfo OPTIONAL }
    //
    // CertPolicyId ::= OBJECT IDENTIFIER
    //
    // PolicyQualifierInfo ::= SEQUENCE {
    //      policyQualifierId  PolicyQualifierId,
    //      qualifier          ANY DEFINED BY policyQualifierId }
    //
    // -- Implementations that recognize additional policy qualifiers MUST
    // -- augment the following definition for PolicyQualifierId
    //
    // PolicyQualifierId ::= OBJECT IDENTIFIER ( id-qt-cps | id-qt-unotice )
    pub(super) fn parse_certificatepolicies(
        i: &[u8],
    ) -> IResult<&[u8], Vec<PolicyInformation>, BerError> {
        fn parse_policy_qualifier_info(i: &[u8]) -> IResult<&[u8], PolicyQualifierInfo, BerError> {
            parse_der_sequence_defined_g(|content, _| {
                let (rem, policy_qualifier_id) = Oid::from_der(content)?;
                let info = PolicyQualifierInfo {
                    policy_qualifier_id,
                    qualifier: rem,
                };
                Ok((&[], info))
            })(i)
        }
        fn parse_policy_information(i: &[u8]) -> IResult<&[u8], PolicyInformation, BerError> {
            parse_der_sequence_defined_g(|content, _| {
                let (rem, policy_id) = Oid::from_der(content)?;
                let (rem, policy_qualifiers) =
                    opt(complete(parse_der_sequence_defined_g(|content, _| {
                        many1(complete(parse_policy_qualifier_info))(content)
                    })))(rem)?;
                let info = PolicyInformation {
                    policy_id,
                    policy_qualifiers,
                };
                Ok((rem, info))
            })(i)
        }
        parse_der_sequence_of_v(parse_policy_information)(i)
    }

    fn parse_certificatepolicies_ext(i: &[u8]) -> IResult<&[u8], ParsedExtension, BerError> {
        map(
            parse_certificatepolicies,
            ParsedExtension::CertificatePolicies,
        )(i)
    }

    // CRLReason ::= ENUMERATED { ...
    fn parse_reason_code<'a>(i: &'a [u8]) -> IResult<&'a [u8], ParsedExtension, BerError> {
        let (rest, obj) = parse_der_enum(i)?;
        let code = obj
            .content
            .as_u32()
            .or(Err(Err::Error(BerError::BerValueError)))?;
        if code > 10 {
            return Err(Err::Error(BerError::BerValueError));
        }
        let ret = ParsedExtension::ReasonCode(ReasonCode(code as u8));
        Ok((rest, ret))
    }

    // invalidityDate ::=  GeneralizedTime
    fn parse_invalidity_date<'a>(i: &'a [u8]) -> ParseResult<'a, ParsedExtension> {
        let (rest, t) = GeneralizedTime::from_der(i)?;
        let dt = t.utc_datetime()?;
        Ok((rest, ParsedExtension::InvalidityDate(ASN1Time::new(dt))))
    }

    // CRLNumber ::= INTEGER (0..MAX)
    // Note from RFC 3280: "CRL verifiers MUST be able to handle CRLNumber values up to 20 octets."
    fn parse_crl_number(i: &[u8]) -> IResult<&[u8], ParsedExtension, BerError> {
        let (rest, num) = map_res(parse_der_integer, |obj| obj.as_biguint())(i)?;
        Ok((rest, ParsedExtension::CRLNumber(num)))
    }

    fn parse_sct_ext(i: &[u8]) -> IResult<&[u8], ParsedExtension, BerError> {
        map(
            parse_ct_signed_certificate_timestamp_list,
            ParsedExtension::SCT,
        )(i)
    }
}

/// Extensions  ::=  SEQUENCE SIZE (1..MAX) OF Extension
pub(crate) fn parse_extension_sequence(i: &[u8]) -> X509Result<Vec<X509Extension>> {
    parse_der_sequence_defined_g(|a, _| all_consuming(many0(complete(X509Extension::from_der)))(a))(
        i,
    )
}

pub(crate) fn parse_extensions(i: &[u8], explicit_tag: Tag) -> X509Result<Vec<X509Extension>> {
    if i.is_empty() {
        return Ok((i, Vec::new()));
    }

    match der_read_element_header(i) {
        Ok((rem, hdr)) => {
            if hdr.tag() != explicit_tag {
                return Err(Err::Error(X509Error::InvalidExtensions));
            }
            all_consuming(parse_extension_sequence)(rem)
        }
        Err(_) => Err(X509Error::InvalidExtensions.into()),
    }
}

/// Extensions  ::=  SEQUENCE SIZE (1..MAX) OF Extension
pub(crate) fn parse_extension_envelope_sequence(i: &[u8]) -> X509Result<Vec<X509Extension>> {
    let parser = X509ExtensionParser::new().with_deep_parse_extensions(false);

    parse_der_sequence_defined_g(move |a, _| all_consuming(many0(complete(parser)))(a))(i)
}

pub(crate) fn parse_extensions_envelope(
    i: &[u8],
    explicit_tag: Tag,
) -> X509Result<Vec<X509Extension>> {
    if i.is_empty() {
        return Ok((i, Vec::new()));
    }

    match der_read_element_header(i) {
        Ok((rem, hdr)) => {
            if hdr.tag() != explicit_tag {
                return Err(Err::Error(X509Error::InvalidExtensions));
            }
            all_consuming(parse_extension_envelope_sequence)(rem)
        }
        Err(_) => Err(X509Error::InvalidExtensions.into()),
    }
}

fn der_read_critical(i: &[u8]) -> BerResult<bool> {
    // Some certificates do not respect the DER BOOLEAN constraint (true must be encoded as 0xff)
    // so we attempt to parse as BER
    let (rem, obj) = opt(parse_ber_bool)(i)?;
    let value = obj
        .map(|o| o.as_bool().unwrap_or_default()) // unwrap cannot fail, we just read a bool
        .unwrap_or(false) // default critical value
        ;
    Ok((rem, value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keyusage_flags() {
        let ku = KeyUsage { flags: 98 };
        assert!(!ku.digital_signature());
        assert!(ku.non_repudiation());
        assert!(!ku.key_encipherment());
        assert!(!ku.data_encipherment());
        assert!(!ku.key_agreement());
        assert!(ku.key_cert_sign());
        assert!(ku.crl_sign());
        assert!(!ku.encipher_only());
        assert!(!ku.decipher_only());
    }

    #[test]
    fn test_extensions1() {
        use der_parser::oid;
        let crt = crate::parse_x509_certificate(include_bytes!("../../assets/extension1.der"))
            .unwrap()
            .1;
        let tbs = &crt.tbs_certificate;
        let bc = crt
            .basic_constraints()
            .expect("could not get basic constraints")
            .expect("no basic constraints found");
        assert_eq!(
            bc.value,
            &BasicConstraints {
                ca: true,
                path_len_constraint: Some(1)
            }
        );
        {
            let ku = tbs
                .key_usage()
                .expect("could not get key usage")
                .expect("no key usage found")
                .value;
            assert!(ku.digital_signature());
            assert!(!ku.non_repudiation());
            assert!(ku.key_encipherment());
            assert!(ku.data_encipherment());
            assert!(ku.key_agreement());
            assert!(!ku.key_cert_sign());
            assert!(!ku.crl_sign());
            assert!(ku.encipher_only());
            assert!(ku.decipher_only());
        }
        {
            let eku = tbs
                .extended_key_usage()
                .expect("could not get extended key usage")
                .expect("no extended key usage found")
                .value;
            assert!(!eku.any);
            assert!(eku.server_auth);
            assert!(!eku.client_auth);
            assert!(eku.code_signing);
            assert!(!eku.email_protection);
            assert!(eku.time_stamping);
            assert!(!eku.ocsp_signing);
            assert_eq!(eku.other, vec![oid!(1.2.3 .4 .0 .42)]);
        }
        assert_eq!(
            tbs.policy_constraints()
                .expect("could not get policy constraints")
                .expect("no policy constraints found")
                .value,
            &PolicyConstraints {
                require_explicit_policy: None,
                inhibit_policy_mapping: Some(10)
            }
        );
        let val = tbs
            .inhibit_anypolicy()
            .expect("could not get inhibit_anypolicy")
            .expect("no inhibit_anypolicy found")
            .value;
        assert_eq!(val, &InhibitAnyPolicy { skip_certs: 2 });
        {
            let alt_names = &tbs
                .subject_alternative_name()
                .expect("could not get subject alt names")
                .expect("no subject alt names found")
                .value
                .general_names;
            assert_eq!(alt_names[0], GeneralName::RFC822Name("foo@example.com"));
            assert_eq!(alt_names[1], GeneralName::URI("http://my.url.here/"));
            assert_eq!(
                alt_names[2],
                GeneralName::IPAddress([192, 168, 7, 1].as_ref())
            );
            assert_eq!(
                format!(
                    "{}",
                    match alt_names[3] {
                        GeneralName::DirectoryName(ref dn) => dn,
                        _ => unreachable!(),
                    }
                ),
                "C=UK, O=My Organization, OU=My Unit, CN=My Name"
            );
            assert_eq!(alt_names[4], GeneralName::DNSName("localhost"));
            assert_eq!(alt_names[5], GeneralName::RegisteredID(oid!(1.2.90 .0)));
            assert_eq!(
                alt_names[6],
                GeneralName::OtherName(oid!(1.2.3 .4), b"\xA0\x17\x0C\x15some other identifier")
            );
        }

        {
            let name_constraints = &tbs
                .name_constraints()
                .expect("could not get name constraints")
                .expect("no name constraints found")
                .value;
            assert_eq!(name_constraints.permitted_subtrees, None);
            assert_eq!(
                name_constraints.excluded_subtrees,
                Some(vec![
                    GeneralSubtree {
                        base: GeneralName::IPAddress([192, 168, 0, 0, 255, 255, 0, 0].as_ref())
                    },
                    GeneralSubtree {
                        base: GeneralName::RFC822Name("foo.com")
                    },
                ])
            );
        }
    }

    #[test]
    fn test_extensions2() {
        use der_parser::oid;
        let crt = crate::parse_x509_certificate(include_bytes!("../../assets/extension2.der"))
            .unwrap()
            .1;
        let tbs = crt.tbs_certificate;
        assert_eq!(
            tbs.policy_constraints()
                .expect("could not get policy constraints")
                .expect("no policy constraints found")
                .value,
            &PolicyConstraints {
                require_explicit_policy: Some(5000),
                inhibit_policy_mapping: None
            }
        );
        {
            let pm = tbs
                .policy_mappings()
                .expect("could not get policy_mappings")
                .expect("no policy_mappings found")
                .value
                .clone()
                .into_hashmap();
            let mut pm_ref = HashMap::new();
            pm_ref.insert(oid!(2.34.23), vec![oid!(2.2)]);
            pm_ref.insert(oid!(1.1), vec![oid!(0.0.4)]);
            pm_ref.insert(oid!(2.2), vec![oid!(2.2.1), oid!(2.2.3)]);
            assert_eq!(pm, pm_ref);
        }
    }

    #[test]
    fn test_extensions_crl_distribution_points() {
        // Extension not present
        {
            let crt = crate::parse_x509_certificate(include_bytes!(
                "../../assets/crl-ext/crl-no-crl.der"
            ))
            .unwrap()
            .1;
            assert!(crt
                .tbs_certificate
                .extensions_map()
                .unwrap()
                .get(&OID_X509_EXT_CRL_DISTRIBUTION_POINTS)
                .is_none());
        }
        // CRLDistributionPoints has 1 entry with 1 URI
        {
            let crt = crate::parse_x509_certificate(include_bytes!(
                "../../assets/crl-ext/crl-simple.der"
            ))
            .unwrap()
            .1;
            let crl = crt
                .tbs_certificate
                .extensions_map()
                .unwrap()
                .get(&OID_X509_EXT_CRL_DISTRIBUTION_POINTS)
                .unwrap()
                .parsed_extension();
            assert!(matches!(crl, ParsedExtension::CRLDistributionPoints(_)));
            if let ParsedExtension::CRLDistributionPoints(crl) = crl {
                assert_eq!(crl.len(), 1);
                assert!(crl[0].reasons.is_none());
                assert!(crl[0].crl_issuer.is_none());
                let distribution_point = crl[0].distribution_point.as_ref().unwrap();
                assert!(matches!(
                    distribution_point,
                    DistributionPointName::FullName(_)
                ));
                if let DistributionPointName::FullName(names) = distribution_point {
                    assert_eq!(names.len(), 1);
                    assert!(matches!(names[0], GeneralName::URI(_)));
                    if let GeneralName::URI(uri) = names[0] {
                        assert_eq!(uri, "http://example.com/myca.crl")
                    }
                }
            }
        }
        // CRLDistributionPoints has 2 entries
        {
            let crt = crate::parse_x509_certificate(include_bytes!(
                "../../assets/crl-ext/crl-complex.der"
            ))
            .unwrap()
            .1;
            let crl = crt
                .tbs_certificate
                .extensions_map()
                .unwrap()
                .get(&OID_X509_EXT_CRL_DISTRIBUTION_POINTS)
                .unwrap()
                .parsed_extension();
            assert!(matches!(crl, ParsedExtension::CRLDistributionPoints(_)));
            if let ParsedExtension::CRLDistributionPoints(crl) = crl {
                assert_eq!(crl.len(), 2);
                // First CRL Distribution point
                let reasons = crl[0].reasons.as_ref().unwrap();
                assert!(reasons.key_compromise());
                assert!(reasons.ca_compromise());
                assert!(!reasons.affilation_changed());
                assert!(!reasons.superseded());
                assert!(!reasons.cessation_of_operation());
                assert!(!reasons.certificate_hold());
                assert!(!reasons.privelege_withdrawn());
                assert!(reasons.aa_compromise());
                assert_eq!(
                    format!("{}", reasons),
                    "Key Compromise, CA Compromise, AA Compromise"
                );
                let issuers = crl[0].crl_issuer.as_ref().unwrap();
                assert_eq!(issuers.len(), 1);
                assert!(matches!(issuers[0], GeneralName::DirectoryName(_)));
                if let GeneralName::DirectoryName(name) = &issuers[0] {
                    assert_eq!(name.to_string(), "C=US, O=Organisation, CN=Some Name");
                }
                let distribution_point = crl[0].distribution_point.as_ref().unwrap();
                assert!(matches!(
                    distribution_point,
                    DistributionPointName::FullName(_)
                ));
                if let DistributionPointName::FullName(names) = distribution_point {
                    assert_eq!(names.len(), 1);
                    assert!(matches!(names[0], GeneralName::URI(_)));
                    if let GeneralName::URI(uri) = names[0] {
                        assert_eq!(uri, "http://example.com/myca.crl")
                    }
                }
                // Second CRL Distribution point
                let reasons = crl[1].reasons.as_ref().unwrap();
                assert!(reasons.key_compromise());
                assert!(reasons.ca_compromise());
                assert!(!reasons.affilation_changed());
                assert!(!reasons.superseded());
                assert!(!reasons.cessation_of_operation());
                assert!(!reasons.certificate_hold());
                assert!(!reasons.privelege_withdrawn());
                assert!(!reasons.aa_compromise());
                assert_eq!(format!("{}", reasons), "Key Compromise, CA Compromise");
                assert!(crl[1].crl_issuer.is_none());
                let distribution_point = crl[1].distribution_point.as_ref().unwrap();
                assert!(matches!(
                    distribution_point,
                    DistributionPointName::FullName(_)
                ));
                if let DistributionPointName::FullName(names) = distribution_point {
                    assert_eq!(names.len(), 1);
                    assert!(matches!(names[0], GeneralName::URI(_)));
                    if let GeneralName::URI(uri) = names[0] {
                        assert_eq!(uri, "http://example.com/myca2.crl")
                    }
                }
            }
        }
    }

    // Test cases for:
    // - parsing SubjectAlternativeName
    // - parsing NameConstraints
}
