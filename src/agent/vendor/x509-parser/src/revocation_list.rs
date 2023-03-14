use crate::error::{X509Error, X509Result};
use crate::extensions::*;
use crate::time::ASN1Time;
use crate::utils::format_serial;
use crate::x509::{
    parse_serial, parse_signature_value, AlgorithmIdentifier, ReasonCode, X509Name, X509Version,
};

use asn1_rs::{BitString, FromDer};
use der_parser::ber::Tag;
use der_parser::der::*;
use der_parser::num_bigint::BigUint;
use der_parser::oid::Oid;
use nom::combinator::{all_consuming, complete, map, opt};
use nom::multi::many0;
use nom::Offset;
use oid_registry::*;
use std::collections::HashMap;

/// An X.509 v2 Certificate Revocation List (CRL).
///
/// X.509 v2 CRLs are defined in [RFC5280](https://tools.ietf.org/html/rfc5280).
///
/// # Example
///
/// To parse a CRL and print information about revoked certificates:
///
/// ```rust
/// use x509_parser::prelude::FromDer;
/// use x509_parser::revocation_list::CertificateRevocationList;
///
/// # static DER: &'static [u8] = include_bytes!("../assets/example.crl");
/// #
/// # fn main() {
/// let res = CertificateRevocationList::from_der(DER);
/// match res {
///     Ok((_rem, crl)) => {
///         for revoked in crl.iter_revoked_certificates() {
///             println!("Revoked certificate serial: {}", revoked.raw_serial_as_string());
///             println!("  Reason: {}", revoked.reason_code().unwrap_or_default().1);
///         }
///     },
///     _ => panic!("CRL parsing failed: {:?}", res),
/// }
/// # }
/// ```
#[derive(Clone, Debug)]
pub struct CertificateRevocationList<'a> {
    pub tbs_cert_list: TbsCertList<'a>,
    pub signature_algorithm: AlgorithmIdentifier<'a>,
    pub signature_value: BitString<'a>,
}

impl<'a> CertificateRevocationList<'a> {
    /// Get the version of the encoded certificate
    pub fn version(&self) -> Option<X509Version> {
        self.tbs_cert_list.version
    }

    /// Get the certificate issuer.
    #[inline]
    pub fn issuer(&self) -> &X509Name {
        &self.tbs_cert_list.issuer
    }

    /// Get the date and time of the last (this) update.
    #[inline]
    pub fn last_update(&self) -> ASN1Time {
        self.tbs_cert_list.this_update
    }

    /// Get the date and time of the next update, if present.
    #[inline]
    pub fn next_update(&self) -> Option<ASN1Time> {
        self.tbs_cert_list.next_update
    }

    /// Return an iterator over the `RevokedCertificate` objects
    pub fn iter_revoked_certificates(&self) -> impl Iterator<Item = &RevokedCertificate<'a>> {
        self.tbs_cert_list.revoked_certificates.iter()
    }

    /// Get the CRL extensions.
    #[inline]
    pub fn extensions(&self) -> &[X509Extension] {
        &self.tbs_cert_list.extensions
    }

    /// Get the CRL number, if present
    ///
    /// Note that the returned value is a `BigUint`, because of the following RFC specification:
    /// <pre>
    /// Given the requirements above, CRL numbers can be expected to contain long integers.  CRL
    /// verifiers MUST be able to handle CRLNumber values up to 20 octets.  Conformant CRL issuers
    /// MUST NOT use CRLNumber values longer than 20 octets.
    /// </pre>
    pub fn crl_number(&self) -> Option<&BigUint> {
        self.extensions()
            .iter()
            .find(|&ext| ext.oid == OID_X509_EXT_BASIC_CONSTRAINTS)
            .and_then(|ext| match ext.parsed_extension {
                ParsedExtension::CRLNumber(ref num) => Some(num),
                _ => None,
            })
    }
}

/// <pre>
/// CertificateList  ::=  SEQUENCE  {
///      tbsCertList          TBSCertList,
///      signatureAlgorithm   AlgorithmIdentifier,
///      signatureValue       BIT STRING  }
/// </pre>
impl<'a> FromDer<'a, X509Error> for CertificateRevocationList<'a> {
    fn from_der(i: &'a [u8]) -> X509Result<Self> {
        parse_der_sequence_defined_g(|i, _| {
            let (i, tbs_cert_list) = TbsCertList::from_der(i)?;
            let (i, signature_algorithm) = AlgorithmIdentifier::from_der(i)?;
            let (i, signature_value) = parse_signature_value(i)?;
            let crl = CertificateRevocationList {
                tbs_cert_list,
                signature_algorithm,
                signature_value,
            };
            Ok((i, crl))
        })(i)
    }
}

/// The sequence TBSCertList contains information about the certificates that have
/// been revoked by the CA that issued the CRL.
///
/// RFC5280 definition:
///
/// <pre>
/// TBSCertList  ::=  SEQUENCE  {
///         version                 Version OPTIONAL,
///                                      -- if present, MUST be v2
///         signature               AlgorithmIdentifier,
///         issuer                  Name,
///         thisUpdate              Time,
///         nextUpdate              Time OPTIONAL,
///         revokedCertificates     SEQUENCE OF SEQUENCE  {
///             userCertificate         CertificateSerialNumber,
///             revocationDate          Time,
///             crlEntryExtensions      Extensions OPTIONAL
///                                      -- if present, version MUST be v2
///                                   } OPTIONAL,
///         crlExtensions           [0]  EXPLICIT Extensions OPTIONAL
///                                      -- if present, version MUST be v2
///                             }
/// </pre>
#[derive(Clone, Debug, PartialEq)]
pub struct TbsCertList<'a> {
    pub version: Option<X509Version>,
    pub signature: AlgorithmIdentifier<'a>,
    pub issuer: X509Name<'a>,
    pub this_update: ASN1Time,
    pub next_update: Option<ASN1Time>,
    pub revoked_certificates: Vec<RevokedCertificate<'a>>,
    extensions: Vec<X509Extension<'a>>,
    pub(crate) raw: &'a [u8],
}

impl<'a> TbsCertList<'a> {
    /// Returns the certificate extensions
    #[inline]
    pub fn extensions(&self) -> &[X509Extension] {
        &self.extensions
    }

    /// Returns an iterator over the certificate extensions
    #[inline]
    pub fn iter_extensions(&self) -> impl Iterator<Item = &X509Extension> {
        self.extensions.iter()
    }

    /// Searches for an extension with the given `Oid`.
    ///
    /// Note: if there are several extensions with the same `Oid`, the first one is returned.
    pub fn find_extension(&self, oid: &Oid) -> Option<&X509Extension> {
        self.extensions.iter().find(|&ext| ext.oid == *oid)
    }

    /// Builds and returns a map of extensions.
    ///
    /// If an extension is present twice, this will fail and return `DuplicateExtensions`.
    pub fn extensions_map(&self) -> Result<HashMap<Oid, &X509Extension>, X509Error> {
        self.extensions
            .iter()
            .try_fold(HashMap::new(), |mut m, ext| {
                if m.contains_key(&ext.oid) {
                    return Err(X509Error::DuplicateExtensions);
                }
                m.insert(ext.oid.clone(), ext);
                Ok(m)
            })
    }
}

impl<'a> AsRef<[u8]> for TbsCertList<'a> {
    fn as_ref(&self) -> &[u8] {
        self.raw
    }
}

impl<'a> FromDer<'a, X509Error> for TbsCertList<'a> {
    fn from_der(i: &'a [u8]) -> X509Result<Self> {
        let start_i = i;
        parse_der_sequence_defined_g(move |i, _| {
            let (i, version) =
                opt(map(parse_der_u32, X509Version))(i).or(Err(X509Error::InvalidVersion))?;
            let (i, signature) = AlgorithmIdentifier::from_der(i)?;
            let (i, issuer) = X509Name::from_der(i)?;
            let (i, this_update) = ASN1Time::from_der(i)?;
            let (i, next_update) = ASN1Time::from_der_opt(i)?;
            let (i, revoked_certificates) = opt(complete(parse_revoked_certificates))(i)?;
            let (i, extensions) = parse_extensions(i, Tag(0))?;
            let len = start_i.offset(i);
            let tbs = TbsCertList {
                version,
                signature,
                issuer,
                this_update,
                next_update,
                revoked_certificates: revoked_certificates.unwrap_or_default(),
                extensions,
                raw: &start_i[..len],
            };
            Ok((i, tbs))
        })(i)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RevokedCertificate<'a> {
    /// The Serial number of the revoked certificate
    pub user_certificate: BigUint,
    /// The date on which the revocation occurred is specified.
    pub revocation_date: ASN1Time,
    /// Additional information about revocation
    extensions: Vec<X509Extension<'a>>,
    pub(crate) raw_serial: &'a [u8],
}

impl<'a> RevokedCertificate<'a> {
    /// Return the serial number of the revoked certificate
    pub fn serial(&self) -> &BigUint {
        &self.user_certificate
    }

    /// Get the CRL entry extensions.
    #[inline]
    pub fn extensions(&self) -> &[X509Extension] {
        &self.extensions
    }

    /// Returns an iterator over the CRL entry extensions
    #[inline]
    pub fn iter_extensions(&self) -> impl Iterator<Item = &X509Extension> {
        self.extensions.iter()
    }

    /// Searches for a CRL entry extension with the given `Oid`.
    ///
    /// Note: if there are several extensions with the same `Oid`, the first one is returned.
    pub fn find_extension(&self, oid: &Oid) -> Option<&X509Extension> {
        self.extensions.iter().find(|&ext| ext.oid == *oid)
    }

    /// Builds and returns a map of CRL entry extensions.
    ///
    /// If an extension is present twice, this will fail and return `DuplicateExtensions`.
    pub fn extensions_map(&self) -> Result<HashMap<Oid, &X509Extension>, X509Error> {
        self.extensions
            .iter()
            .try_fold(HashMap::new(), |mut m, ext| {
                if m.contains_key(&ext.oid) {
                    return Err(X509Error::DuplicateExtensions);
                }
                m.insert(ext.oid.clone(), ext);
                Ok(m)
            })
    }

    /// Get the raw bytes of the certificate serial number
    pub fn raw_serial(&self) -> &[u8] {
        self.raw_serial
    }

    /// Get a formatted string of the certificate serial number, separated by ':'
    pub fn raw_serial_as_string(&self) -> String {
        format_serial(self.raw_serial)
    }

    /// Get the code identifying the reason for the revocation, if present
    pub fn reason_code(&self) -> Option<(bool, ReasonCode)> {
        self.find_extension(&OID_X509_EXT_REASON_CODE)
            .and_then(|ext| match ext.parsed_extension {
                ParsedExtension::ReasonCode(code) => Some((ext.critical, code)),
                _ => None,
            })
    }

    /// Get the invalidity date, if present
    ///
    /// The invalidity date is the date on which it is known or suspected that the private
    ///  key was compromised or that the certificate otherwise became invalid.
    pub fn invalidity_date(&self) -> Option<(bool, ASN1Time)> {
        self.find_extension(&OID_X509_EXT_INVALIDITY_DATE)
            .and_then(|ext| match ext.parsed_extension {
                ParsedExtension::InvalidityDate(date) => Some((ext.critical, date)),
                _ => None,
            })
    }
}

// revokedCertificates     SEQUENCE OF SEQUENCE  {
//     userCertificate         CertificateSerialNumber,
//     revocationDate          Time,
//     crlEntryExtensions      Extensions OPTIONAL
//                                   -- if present, MUST be v2
//                          }  OPTIONAL,
impl<'a> FromDer<'a, X509Error> for RevokedCertificate<'a> {
    fn from_der(i: &'a [u8]) -> X509Result<Self> {
        parse_der_sequence_defined_g(|i, _| {
            let (i, (raw_serial, user_certificate)) = parse_serial(i)?;
            let (i, revocation_date) = ASN1Time::from_der(i)?;
            let (i, extensions) = opt(complete(parse_extension_sequence))(i)?;
            let revoked = RevokedCertificate {
                user_certificate,
                revocation_date,
                extensions: extensions.unwrap_or_default(),
                raw_serial,
            };
            Ok((i, revoked))
        })(i)
    }
}

fn parse_revoked_certificates(i: &[u8]) -> X509Result<Vec<RevokedCertificate>> {
    parse_der_sequence_defined_g(|a, _| {
        all_consuming(many0(complete(RevokedCertificate::from_der)))(a)
    })(i)
}
