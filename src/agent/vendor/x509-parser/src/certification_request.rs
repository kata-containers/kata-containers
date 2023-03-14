use crate::cri_attributes::*;
use crate::error::{X509Error, X509Result};
use crate::extensions::*;
use crate::x509::{
    parse_signature_value, AlgorithmIdentifier, SubjectPublicKeyInfo, X509Name, X509Version,
};

use asn1_rs::{BitString, FromDer};
use der_parser::der::*;
use der_parser::oid::Oid;
use der_parser::*;
use nom::Offset;
#[cfg(feature = "verify")]
use oid_registry::*;
use std::collections::HashMap;

/// Certification Signing Request (CSR)
#[derive(Debug, PartialEq)]
pub struct X509CertificationRequest<'a> {
    pub certification_request_info: X509CertificationRequestInfo<'a>,
    pub signature_algorithm: AlgorithmIdentifier<'a>,
    pub signature_value: BitString<'a>,
}

impl<'a> X509CertificationRequest<'a> {
    pub fn requested_extensions(&self) -> Option<impl Iterator<Item = &ParsedExtension>> {
        self.certification_request_info
            .iter_attributes()
            .find_map(|attr| {
                if let ParsedCriAttribute::ExtensionRequest(requested) = &attr.parsed_attribute {
                    Some(requested.extensions.iter().map(|ext| &ext.parsed_extension))
                } else {
                    None
                }
            })
    }

    /// Verify the cryptographic signature of this certification request
    ///
    /// Uses the public key contained in the CSR, which must be the one of the entity
    /// requesting the certification for this verification to succeed.
    #[cfg(feature = "verify")]
    pub fn verify_signature(&self) -> Result<(), X509Error> {
        use ring::signature;
        let spki = &self.certification_request_info.subject_pki;
        let signature_alg = &self.signature_algorithm.algorithm;
        // identify verification algorithm
        let verification_alg: &dyn signature::VerificationAlgorithm =
            if *signature_alg == OID_PKCS1_SHA1WITHRSA {
                &signature::RSA_PKCS1_1024_8192_SHA1_FOR_LEGACY_USE_ONLY
            } else if *signature_alg == OID_PKCS1_SHA256WITHRSA {
                &signature::RSA_PKCS1_2048_8192_SHA256
            } else if *signature_alg == OID_PKCS1_SHA384WITHRSA {
                &signature::RSA_PKCS1_2048_8192_SHA384
            } else if *signature_alg == OID_PKCS1_SHA512WITHRSA {
                &signature::RSA_PKCS1_2048_8192_SHA512
            } else if *signature_alg == OID_SIG_ECDSA_WITH_SHA256 {
                &signature::ECDSA_P256_SHA256_ASN1
            } else if *signature_alg == OID_SIG_ECDSA_WITH_SHA384 {
                &signature::ECDSA_P384_SHA384_ASN1
            } else if *signature_alg == OID_SIG_ED25519 {
                &signature::ED25519
            } else {
                return Err(X509Error::SignatureUnsupportedAlgorithm);
            };
        // get public key
        let key =
            signature::UnparsedPublicKey::new(verification_alg, &spki.subject_public_key.data);
        // verify signature
        let sig = &self.signature_value.data;
        key.verify(self.certification_request_info.raw, sig)
            .or(Err(X509Error::SignatureVerificationError))
    }
}

/// <pre>
/// CertificationRequest ::= SEQUENCE {
///     certificationRequestInfo CertificationRequestInfo,
///     signatureAlgorithm AlgorithmIdentifier{{ SignatureAlgorithms }},
///     signature          BIT STRING
/// }
/// </pre>
impl<'a> FromDer<'a, X509Error> for X509CertificationRequest<'a> {
    fn from_der(i: &'a [u8]) -> X509Result<'a, Self> {
        parse_der_sequence_defined_g(|i, _| {
            let (i, certification_request_info) = X509CertificationRequestInfo::from_der(i)?;
            let (i, signature_algorithm) = AlgorithmIdentifier::from_der(i)?;
            let (i, signature_value) = parse_signature_value(i)?;
            let cert = X509CertificationRequest {
                certification_request_info,
                signature_algorithm,
                signature_value,
            };
            Ok((i, cert))
        })(i)
    }
}

/// Certification Request Info structure
///
/// Certification request information is defined by the following ASN.1 structure:
///
/// <pre>
/// CertificationRequestInfo ::= SEQUENCE {
///      version       INTEGER { v1(0) } (v1,...),
///      subject       Name,
///      subjectPKInfo SubjectPublicKeyInfo{{ PKInfoAlgorithms }},
///      attributes    [0] Attributes{{ CRIAttributes }}
/// }
/// </pre>
///
/// version is the version number; subject is the distinguished name of the certificate
/// subject; subject_pki contains information about the public key being certified, and
/// attributes is a collection of attributes providing additional information about the
/// subject of the certificate.
#[derive(Debug, PartialEq)]
pub struct X509CertificationRequestInfo<'a> {
    pub version: X509Version,
    pub subject: X509Name<'a>,
    pub subject_pki: SubjectPublicKeyInfo<'a>,
    attributes: Vec<X509CriAttribute<'a>>,
    pub raw: &'a [u8],
}

impl<'a> X509CertificationRequestInfo<'a> {
    /// Get the CRL entry extensions.
    #[inline]
    pub fn attributes(&self) -> &[X509CriAttribute] {
        &self.attributes
    }

    /// Returns an iterator over the CRL entry extensions
    #[inline]
    pub fn iter_attributes(&self) -> impl Iterator<Item = &X509CriAttribute> {
        self.attributes.iter()
    }

    /// Searches for a CRL entry extension with the given `Oid`.
    ///
    /// Note: if there are several extensions with the same `Oid`, the first one is returned.
    pub fn find_attribute(&self, oid: &Oid) -> Option<&X509CriAttribute> {
        self.attributes.iter().find(|&ext| ext.oid == *oid)
    }

    /// Builds and returns a map of CRL entry extensions.
    ///
    /// If an extension is present twice, this will fail and return `DuplicateExtensions`.
    pub fn attributes_map(&self) -> Result<HashMap<Oid, &X509CriAttribute>, X509Error> {
        self.attributes
            .iter()
            .try_fold(HashMap::new(), |mut m, ext| {
                if m.contains_key(&ext.oid) {
                    return Err(X509Error::DuplicateAttributes);
                }
                m.insert(ext.oid.clone(), ext);
                Ok(m)
            })
    }
}

/// <pre>
/// CertificationRequestInfo ::= SEQUENCE {
///      version       INTEGER { v1(0) } (v1,...),
///      subject       Name,
///      subjectPKInfo SubjectPublicKeyInfo{{ PKInfoAlgorithms }},
///      attributes    [0] Attributes{{ CRIAttributes }}
/// }
/// </pre>
impl<'a> FromDer<'a, X509Error> for X509CertificationRequestInfo<'a> {
    fn from_der(i: &'a [u8]) -> X509Result<Self> {
        let start_i = i;
        parse_der_sequence_defined_g(move |i, _| {
            let (i, version) = X509Version::from_der_required(i)?;
            let (i, subject) = X509Name::from_der(i)?;
            let (i, subject_pki) = SubjectPublicKeyInfo::from_der(i)?;
            let (i, attributes) = parse_cri_attributes(i)?;
            let len = start_i.offset(i);
            let tbs = X509CertificationRequestInfo {
                version,
                subject,
                subject_pki,
                attributes,
                raw: &start_i[..len],
            };
            Ok((i, tbs))
        })(i)
    }
}
