use super::utils::{from_der, from_pem, from_pem_str, to_der, to_pem};
use crate::hash::HashAlgorithm;
use crate::key::{PrivateKey, PublicKey};
use crate::pem::{Pem, PemError};
use crate::signature::{SignatureAlgorithm, SignatureError};
use crate::x509::csr::{Csr, CsrError};
use crate::x509::date::UtcDate;
use crate::x509::key_id_gen_method::{KeyIdGenError, KeyIdGenMethod};
use crate::x509::name::{DirectoryName, GeneralNames};
use picky_asn1::bit_string::BitString;
use picky_asn1::wrapper::{ExplicitContextTag0, ExplicitContextTag3, IntegerAsn1};
use picky_asn1_der::{Asn1DerError, Asn1RawDer};
use picky_asn1_x509::{
    oids, AlgorithmIdentifier, AuthorityKeyIdentifier, BasicConstraints, Certificate, ExtendedKeyUsage, Extension,
    ExtensionView, Extensions, KeyIdentifier, KeyUsage, Name, SubjectPublicKeyInfo, TbsCertificate, Validity, Version,
};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use thiserror::Error;

const ELEMENT_NAME: &str = "x509 certificate";

#[derive(Debug, Error)]
pub enum CertError {
    /// couldn't generate certificate
    #[error("couldn't generate certificate: {source}")]
    CertGeneration { source: Box<CertError> },

    /// invalid certificate
    #[error("invalid certificate '{id}': {source}")]
    InvalidCertificate { id: String, source: Box<CertError> },

    /// ASN1 serialization error
    #[error("(ASN1) couldn't serialize {element}: {source}")]
    Asn1Serialization {
        element: &'static str,
        source: Asn1DerError,
    },

    /// ASN1 deserialization error
    #[error("(ASN1) couldn't deserialize {element}: {source}")]
    Asn1Deserialization {
        element: &'static str,
        source: Asn1DerError,
    },

    /// signature error
    #[error("signature error: {source}")]
    Signature { source: SignatureError },

    /// key id generation error
    #[error("key id generation error: {source}")]
    KeyIdGen { source: KeyIdGenError },

    /// CA chain error
    #[error("CA chain error: {source}")]
    InvalidChain { source: CaChainError },

    /// CSR error
    #[error("CSR error: {source}")]
    InvalidCsr { source: CsrError },

    /// extension not found
    #[error("extension not found: {name}")]
    ExtensionNotFound { name: &'static str },

    /// missing required builder argument
    #[error("missing required builder argument `{arg}`")]
    MissingBuilderArgument { arg: &'static str },

    /// certificate is not yet valid
    #[error("certificate is not yet valid (not before: {not_before}, now: {now})")]
    CertificateNotYetValid { not_before: UtcDate, now: UtcDate },

    /// certificate expired
    #[error("certificate expired (not after: {not_after}, now: {now})")]
    CertificateExpired { not_after: UtcDate, now: UtcDate },

    /// invalid PEM label error
    #[error("invalid PEM label: {label}")]
    InvalidPemLabel { label: String },

    /// invalid PEM provided
    #[error("invalid PEM provided: {source}")]
    Pem { source: PemError },
}

impl From<PemError> for CertError {
    fn from(e: PemError) -> Self {
        Self::Pem { source: e }
    }
}

#[derive(Debug, Error)]
pub enum CaChainError {
    /// chain depth does't satisfy basic constraints extension
    #[error(
        "chain depth doesn't satisfy basic constraints extension: certificate '{cert_id}' has pathlen of {pathlen}"
    )]
    TooDeep { cert_id: String, pathlen: u8 },

    /// chain is missing a root certificate
    #[error("chain is missing a root certificate")]
    NoRoot,

    /// issuer certificate is not a CA
    #[error("issuer certificate '{issuer_id}' is not a CA")]
    IssuerIsNotCA { issuer_id: String },

    /// authority key id doesn't match
    #[error("authority key id doesn't match (expected: {}, got: {})", base64::encode(&.expected), base64::encode(&.actual))]
    AuthorityKeyIdMismatch { expected: Vec<u8>, actual: Vec<u8> },

    /// issuer name doesn't match
    #[error("issuer name doesn't match (expected: {expected}, got: {actual})")]
    IssuerNameMismatch { expected: String, actual: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CertType {
    Root,
    Intermediate,
    Leaf,
    Unknown,
}

const CERT_PEM_LABELS: &[&str] = &["CERTIFICATE", "TRUSTED CERTIFICATE"];

/// CertificateOverview is used to validate signatures (using tbs_certificate der encoding) and encode back original certificate as is.
/// Refer PSDiagnostics PowerShell module authenticode test for details as to why this is useful.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
struct CertificateOverview {
    tbs_certificate: Asn1RawDer,
    signature_algorithm: Asn1RawDer,
    signature_value: Asn1RawDer,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Cert {
    details: Certificate,
    overview: CertificateOverview,
}

impl TryFrom<Certificate> for Cert {
    type Error = CertError;

    fn try_from(certificate: Certificate) -> Result<Self, Self::Error> {
        let der = picky_asn1_der::to_vec(&certificate).map_err(|source| CertError::Asn1Serialization {
            element: "certificate",
            source,
        })?;
        let overview = picky_asn1_der::from_bytes(&der).map_err(|source| CertError::Asn1Deserialization {
            element: "certificate",
            source,
        })?;
        Ok(Self {
            details: certificate,
            overview,
        })
    }
}

impl From<Cert> for Certificate {
    fn from(certificate: Cert) -> Self {
        certificate.details
    }
}

macro_rules! find_ext {
    ($oid:expr, $certificate:ident, $ext_name:literal) => {{
        let key_identifier_oid = $oid;
        ($certificate.tbs_certificate.extensions.0)
            .0
            .iter()
            .find(|ext| ext.extn_id() == &key_identifier_oid)
            .ok_or(CertError::ExtensionNotFound { name: $ext_name })
    }};
}

impl Cert {
    pub fn from_der<T: ?Sized + AsRef<[u8]>>(der: &T) -> Result<Self, CertError> {
        Ok(Self {
            details: from_der(der, ELEMENT_NAME)?,
            overview: from_der(der, ELEMENT_NAME)?,
        })
    }

    pub fn from_pem(pem: &Pem) -> Result<Self, CertError> {
        Ok(Self {
            details: from_pem(pem, CERT_PEM_LABELS, ELEMENT_NAME)?,
            overview: from_pem(pem, CERT_PEM_LABELS, ELEMENT_NAME)?,
        })
    }

    pub fn from_pem_str(pem_str: &str) -> Result<Self, CertError> {
        Ok(Self {
            details: from_pem_str(pem_str, CERT_PEM_LABELS, ELEMENT_NAME)?,
            overview: from_pem_str(pem_str, CERT_PEM_LABELS, ELEMENT_NAME)?,
        })
    }

    pub fn to_der(&self) -> Result<Vec<u8>, CertError> {
        to_der(&self.overview, ELEMENT_NAME)
    }

    pub fn to_pem(&self) -> Result<Pem<'static>, CertError> {
        to_pem(&self.overview, CERT_PEM_LABELS[0], ELEMENT_NAME)
    }

    pub fn ty(&self) -> CertType {
        if let Some(ca) = self.basic_constraints().map(|bc| bc.ca()).unwrap_or(None) {
            if ca {
                if self.subject_name() == self.issuer_name() {
                    CertType::Root
                } else {
                    CertType::Intermediate
                }
            } else {
                CertType::Leaf
            }
        } else {
            CertType::Unknown
        }
    }

    pub fn serial_number(&self) -> &IntegerAsn1 {
        &self.details.tbs_certificate.serial_number
    }

    pub fn signature_algorithm(&self) -> &AlgorithmIdentifier {
        &self.details.tbs_certificate.signature
    }

    pub fn valid_not_before(&self) -> UtcDate {
        self.details.tbs_certificate.validity.not_before.clone().into()
    }

    pub fn valid_not_after(&self) -> UtcDate {
        self.details.tbs_certificate.validity.not_after.clone().into()
    }

    pub fn subject_key_identifier(&self) -> Result<&[u8], CertError> {
        let certificate = &self.details;

        let ext = find_ext!(oids::subject_key_identifier(), certificate, "subject key identifier")?;
        match ext.extn_value() {
            ExtensionView::SubjectKeyIdentifier(ski) => Ok(&ski.0),
            _ => unreachable!("invalid extension (expected subject key identifier)"),
        }
    }

    pub fn authority_key_identifier(&self) -> Result<&AuthorityKeyIdentifier, CertError> {
        let certificate = &self.details;

        let ext = find_ext!(
            oids::authority_key_identifier(),
            certificate,
            "authority key identifier"
        )?;
        match ext.extn_value() {
            ExtensionView::AuthorityKeyIdentifier(aki) => Ok(aki),
            _ => unreachable!("invalid extension (expected authority key identifier)"),
        }
    }

    pub fn basic_constraints(&self) -> Result<&BasicConstraints, CertError> {
        let certificate = &self.details;
        let ext = find_ext!(oids::basic_constraints(), certificate, "basic constraints")?;
        match ext.extn_value() {
            ExtensionView::BasicConstraints(bc) => Ok(bc),
            _ => unreachable!("invalid extension (expected basic constraints)"),
        }
    }

    pub fn subject_name(&self) -> DirectoryName {
        self.details.tbs_certificate.subject.clone().into()
    }

    pub fn issuer_name(&self) -> DirectoryName {
        self.details.tbs_certificate.issuer.clone().into()
    }

    pub fn extensions(&self) -> &[Extension] {
        (self.details.tbs_certificate.extensions.0).0.as_slice()
    }

    pub fn public_key(&self) -> &PublicKey {
        (&self.details.tbs_certificate.subject_public_key_info).into()
    }

    pub fn into_public_key(self) -> PublicKey {
        self.details.tbs_certificate.subject_public_key_info.into()
    }

    pub fn is_parent_of(&self, other: &Cert) -> Result<(), CertError> {
        if let Ok(other_aki) = other.authority_key_identifier() {
            if let Some(other_aki) = other_aki.key_identifier() {
                let parent_ski = self
                    .subject_key_identifier()
                    .map_err(|e| CertError::InvalidCertificate {
                        source: Box::new(e),
                        id: self.subject_name().to_string(),
                    })?;

                if parent_ski != other_aki {
                    return Err(CaChainError::AuthorityKeyIdMismatch {
                        expected: other_aki.to_vec(),
                        actual: parent_ski.to_vec(),
                    })
                    .map_err(|e| CertError::InvalidChain { source: e })
                    .map_err(|e| CertError::InvalidCertificate {
                        source: Box::new(e),
                        id: other.subject_name().to_string(),
                    });
                }
            }
        }

        let other_issuer_name = other.issuer_name();
        let self_subject_name = self.subject_name();
        if other_issuer_name != self_subject_name {
            return Err(CaChainError::IssuerNameMismatch {
                expected: other_issuer_name.to_string(),
                actual: self_subject_name.to_string(),
            })
            .map_err(|e| CertError::InvalidChain { source: e })
            .map_err(|e| CertError::InvalidCertificate {
                source: Box::new(e),
                id: other.subject_name().to_string(),
            });
        }

        Ok(())
    }

    pub fn verifier<'a, 'b, Chain: Iterator<Item = &'b Cert>>(&'a self) -> CertValidator<'a, 'b, Chain> {
        CertValidator {
            cert: self,
            inner: RefCell::new(CertValidatorInner {
                strictness: Default::default(),
                now: None,
                chain: None,
            }),
        }
    }
}

// === certificate verifier === /

#[derive(Debug, Clone)]
pub(super) enum ValidityCheck<'a> {
    Interval { lower: &'a UtcDate, upper: &'a UtcDate },
    Exact(&'a UtcDate),
}

#[derive(Debug, Clone)]
struct CheckStrictness {
    require_not_before_check: bool,
    require_not_after_check: bool,
    require_chain_check: bool,
    chain_should_contains_root_certificate: bool,
}

impl Default for CheckStrictness {
    fn default() -> Self {
        Self {
            require_not_before_check: true,
            require_not_after_check: true,
            require_chain_check: true,
            chain_should_contains_root_certificate: true,
        }
    }
}

#[derive(Clone, Debug)]
struct CertValidatorInner<'a, 'b, Chain: Iterator<Item = &'b Cert>> {
    strictness: CheckStrictness,
    now: Option<ValidityCheck<'a>>,
    chain: Option<Chain>,
}

/// Utility to verify x509 `Cert`s
#[derive(Clone, Debug)]
pub struct CertValidator<'a, 'b, Chain: Iterator<Item = &'b Cert>> {
    cert: &'a Cert,
    inner: RefCell<CertValidatorInner<'a, 'b, Chain>>,
}

impl<'a, 'b, Chain: Iterator<Item = &'b Cert>> CertValidator<'a, 'b, Chain> {
    #[inline]
    pub fn exact_date(&self, exact: &'a UtcDate) -> &Self {
        self.inner.borrow_mut().now = Some(ValidityCheck::Exact(exact));
        self
    }

    #[inline]
    pub fn interval_date(&self, lower: &'a UtcDate, upper: &'a UtcDate) -> &Self {
        self.inner.borrow_mut().now = Some(ValidityCheck::Interval { lower, upper });
        self
    }

    #[inline]
    pub fn chain(&self, chain: Chain) -> &Self {
        self.inner.borrow_mut().chain = Some(chain);
        self
    }

    #[inline]
    pub fn require_not_before_check(&self) -> &Self {
        self.inner.borrow_mut().strictness.require_not_before_check = true;
        self
    }

    #[inline]
    pub fn require_not_after_check(&self) -> &Self {
        self.inner.borrow_mut().strictness.require_not_after_check = true;
        self
    }

    #[inline]
    pub fn require_chain_check(&self) -> &Self {
        self.inner.borrow_mut().strictness.require_chain_check = true;
        self
    }

    #[inline]
    pub fn ignore_not_before_check(&self) -> &Self {
        self.inner.borrow_mut().strictness.require_not_before_check = false;
        self
    }

    #[inline]
    pub fn ignore_not_after_check(&self) -> &Self {
        self.inner.borrow_mut().strictness.require_not_after_check = false;
        self
    }

    #[inline]
    pub fn ignore_chain_check(&self) -> &Self {
        self.inner.borrow_mut().strictness.require_chain_check = false;
        self
    }

    #[inline]
    #[allow(dead_code)]
    pub(super) fn chain_should_contains_root_certificate(&self, should_contains: bool) -> &Self {
        self.inner
            .borrow_mut()
            .strictness
            .chain_should_contains_root_certificate = should_contains;
        self
    }

    pub fn verify(&self) -> Result<(), CertError> {
        let mut inner = self.inner.borrow_mut();

        if (inner.strictness.require_not_after_check || inner.strictness.require_not_before_check)
            && inner.now.is_none()
        {
            return Err(CertError::MissingBuilderArgument { arg: "now" });
        }

        if let Some(now) = &inner.now {
            verify_cert_validity(self.cert, &inner.strictness, now.clone()).map_err(|e| {
                CertError::InvalidCertificate {
                    source: Box::new(e),
                    id: self.cert.subject_name().to_string(),
                }
            })?;
        }

        if !inner.strictness.require_chain_check {
            return Ok(());
        }

        let chain = if let Some(chain) = inner.chain.take() {
            chain
        } else {
            return Err(CertError::MissingBuilderArgument { arg: "chain" });
        };

        let mut current_cert = self.cert;

        for (number_certs, parent_cert) in chain.enumerate() {
            // check basic constraints
            match parent_cert
                .basic_constraints()
                .map(|bc| (bc.ca(), bc.pathlen()))
                .unwrap_or((None, None))
            {
                (Some(false), _) => {
                    return Err(CaChainError::IssuerIsNotCA {
                        issuer_id: parent_cert.subject_name().to_string(),
                    })
                    .map_err(|e| CertError::InvalidChain { source: e });
                }
                (_, Some(pathlen)) if usize::from(pathlen) < number_certs => {
                    return Err(CaChainError::TooDeep {
                        cert_id: parent_cert.subject_name().to_string(),
                        pathlen,
                    })
                    .map_err(|e| CertError::InvalidChain { source: e });
                }
                _ => {}
            }

            // verify parent validity
            if let Some(now) = &inner.now {
                verify_cert_validity(parent_cert, &inner.strictness, now.clone()).map_err(|e| {
                    CertError::InvalidCertificate {
                        source: Box::new(e),
                        id: parent_cert.subject_name().to_string(),
                    }
                })?;
            }

            // check parent_cert is the parent of current_cert
            parent_cert.is_parent_of(current_cert)?;

            // validate current cert signature using parent public key
            let hash_type = SignatureAlgorithm::from_algorithm_identifier(&current_cert.details.signature_algorithm)
                .map_err(|e| CertError::Signature { source: e })?;
            let public_key = &parent_cert.details.tbs_certificate.subject_public_key_info;
            hash_type
                .verify(
                    &public_key.clone().into(),
                    &current_cert.overview.tbs_certificate.0,
                    current_cert.details.signature_value.0.payload_view(),
                )
                .map_err(|e| CertError::Signature { source: e })
                .map_err(|e| CertError::InvalidCertificate {
                    source: Box::new(e),
                    id: current_cert.subject_name().to_string(),
                })?;

            current_cert = parent_cert;
        }

        // make sure `current_cert` (the last certificate of the chain) is a root CA
        if inner.strictness.chain_should_contains_root_certificate && current_cert.ty() != CertType::Root {
            return Err(CaChainError::NoRoot).map_err(|e| CertError::InvalidChain { source: e });
        }

        Ok(())
    }
}

fn verify_cert_validity(cert: &Cert, strictness: &CheckStrictness, now: ValidityCheck<'_>) -> Result<(), CertError> {
    let validity = &cert.details.tbs_certificate.validity;
    let not_before: UtcDate = validity.not_before.clone().into();
    let not_after: UtcDate = validity.not_after.clone().into();

    match now {
        ValidityCheck::Interval { lower, upper } => {
            if not_before.gt(upper) && strictness.require_not_before_check {
                return Err(CertError::CertificateNotYetValid {
                    not_before,
                    now: upper.clone(),
                });
            }

            if not_after.lt(lower) && strictness.require_not_after_check {
                return Err(CertError::CertificateExpired {
                    not_after,
                    now: lower.clone(),
                });
            }
        }
        ValidityCheck::Exact(now) => {
            if not_before.gt(now) && strictness.require_not_before_check {
                return Err(CertError::CertificateNotYetValid {
                    not_before,
                    now: now.clone(),
                });
            }

            if not_after.lt(now) && strictness.require_not_after_check {
                return Err(CertError::CertificateExpired {
                    not_after,
                    now: now.clone(),
                });
            }
        }
    }

    Ok(())
}

// === builder === //

#[derive(Clone, Debug)]
enum SubjectInfos {
    Csr(Csr),
    NameAndPublicKey { name: DirectoryName, public_key: PublicKey },
}

#[derive(Clone, Debug)]
struct IssuerInfos<'a> {
    name: DirectoryName,
    key: &'a PrivateKey,
    self_signed: bool,
}

// Statically checks the field actually exists and returns a &'static str of the field name
macro_rules! field_str {
    ($field:ident) => {{
        const _: fn() = || {
            let CertificateBuilderInner { $field: _, .. };
        };
        stringify!($field)
    }};
}

#[derive(Default, Clone, Debug)]
struct CertificateBuilderInner<'a> {
    valid_from: Option<UtcDate>,
    valid_to: Option<UtcDate>,
    subject_infos: Option<SubjectInfos>,
    issuer_infos: Option<IssuerInfos<'a>>,
    authority_key_identifier: Option<Vec<u8>>,
    ca: Option<bool>,
    pathlen: Option<u8>,
    signature_hash_type: Option<SignatureAlgorithm>,
    key_id_gen_method: Option<KeyIdGenMethod>,
    key_usage: Option<KeyUsage>,
    extended_key_usage: Option<ExtendedKeyUsage>,
    subject_alt_name: Option<GeneralNames>,
    issuer_alt_name: Option<GeneralNames>,
    serial_number: Option<Vec<u8>>,
    inherit_extensions_from_csr_attributes: bool,
}

#[derive(Default, Clone, Debug)]
pub struct CertificateBuilder<'a> {
    inner: RefCell<CertificateBuilderInner<'a>>,
}

impl<'a> CertificateBuilder<'a> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Required
    #[inline]
    pub fn validity(&self, valid_from: UtcDate, valid_to: UtcDate) -> &Self {
        let mut inner_mut = self.inner.borrow_mut();
        inner_mut.valid_from = Some(valid_from);
        inner_mut.valid_to = Some(valid_to);
        drop(inner_mut);
        self
    }

    /// Required (alternatives: `subject_from_csr`, `self_signed`)
    #[inline]
    pub fn subject(&self, subject_name: DirectoryName, public_key: PublicKey) -> &Self {
        self.inner.borrow_mut().subject_infos = Some(SubjectInfos::NameAndPublicKey {
            name: subject_name,
            public_key,
        });
        self
    }

    /// Required (alternatives: `subject`, `self_signed`)
    #[inline]
    pub fn subject_from_csr(&self, csr: Csr) -> &Self {
        self.inner.borrow_mut().subject_infos = Some(SubjectInfos::Csr(csr));
        self
    }

    /// Required (alternative: `self_signed`, `issuer_cert`)
    #[inline]
    pub fn issuer(&self, issuer_name: DirectoryName, issuer_key: &'a PrivateKey) -> &Self {
        self.inner.borrow_mut().issuer_infos = Some(IssuerInfos {
            name: issuer_name,
            key: issuer_key,
            self_signed: false,
        });
        self
    }

    /// Required (alternative: `issuer`, `issuer_cert`)
    #[inline]
    pub fn self_signed(&self, name: DirectoryName, key: &'a PrivateKey) -> &Self {
        self.inner.borrow_mut().issuer_infos = Some(IssuerInfos {
            name,
            key,
            self_signed: true,
        });
        self
    }

    /// Required (alternative: `issuer`, `self_signed`)
    #[inline]
    pub fn issuer_cert(&self, issuer_cert: &Cert, issuer_key: &'a PrivateKey) -> &Self {
        let builder = self.issuer(issuer_cert.subject_name(), issuer_key);

        if let Ok(issuer_ski) = issuer_cert.subject_key_identifier() {
            self.authority_key_identifier(issuer_ski.to_vec())
        } else {
            builder
        }
    }

    /// Optional (alternative: `issuer_cert`, `self_signed`)
    #[inline]
    pub fn authority_key_identifier(&self, aki: Vec<u8>) -> &Self {
        self.inner.borrow_mut().authority_key_identifier = Some(aki);
        self
    }

    /// Optional
    #[inline]
    pub fn ca(&self, ca: bool) -> &Self {
        self.inner.borrow_mut().ca = Some(ca);
        self
    }

    /// Optional
    #[inline]
    pub fn pathlen(&self, pathlen: u8) -> &Self {
        self.inner.borrow_mut().pathlen = Some(pathlen);
        self
    }

    /// Optional
    #[inline]
    pub fn signature_hash_type(&self, signature_hash_type: SignatureAlgorithm) -> &Self {
        self.inner.borrow_mut().signature_hash_type = Some(signature_hash_type);
        self
    }

    /// Optional
    #[inline]
    pub fn key_id_gen_method(&self, key_id_gen_method: KeyIdGenMethod) -> &Self {
        self.inner.borrow_mut().key_id_gen_method = Some(key_id_gen_method);
        self
    }

    /// Optional
    #[inline]
    pub fn key_usage(&self, key_usage: KeyUsage) -> &Self {
        self.inner.borrow_mut().key_usage = Some(key_usage);
        self
    }

    /// Optional
    #[inline]
    pub fn extended_key_usage(&self, extended_key_usage: ExtendedKeyUsage) -> &Self {
        self.inner.borrow_mut().extended_key_usage = Some(extended_key_usage);
        self
    }

    /// Optional
    #[inline]
    pub fn subject_alt_name(&self, subject_alt_name: GeneralNames) -> &Self {
        self.inner.borrow_mut().subject_alt_name = Some(subject_alt_name);
        self
    }

    /// Optional
    #[inline]
    pub fn issuer_alt_name(&self, issuer_alt_name: GeneralNames) -> &Self {
        self.inner.borrow_mut().issuer_alt_name = Some(issuer_alt_name);
        self
    }

    /// Optional
    ///
    /// Bypass picky serial number generator by providing your own.
    #[inline]
    pub fn serial_number(&self, unsigned_integer_bytes: Vec<u8>) -> &Self {
        self.inner.borrow_mut().serial_number = Some(unsigned_integer_bytes);
        self
    }

    /// Optional
    ///
    /// Inherit extensions from the "extension request" attribute of the provided CSR if applicable
    /// Extensions already present will be ignored.
    #[inline]
    pub fn inherit_extensions_from_csr_attributes(&self, inherit: bool) -> &Self {
        self.inner.borrow_mut().inherit_extensions_from_csr_attributes = inherit;
        self
    }

    pub fn build(&self) -> Result<Cert, CertError> {
        let mut inner = self.inner.borrow_mut();

        let valid_from = inner.valid_from.take().ok_or(CertError::MissingBuilderArgument {
            arg: field_str!(valid_from),
        })?;
        let valid_to = inner.valid_to.take().ok_or(CertError::MissingBuilderArgument {
            arg: field_str!(valid_to),
        })?;

        let signature_hash_type = inner
            .signature_hash_type
            .take()
            .unwrap_or(SignatureAlgorithm::RsaPkcs1v15(HashAlgorithm::SHA2_256));

        let key_id_gen_method = inner
            .key_id_gen_method
            .take()
            .unwrap_or(KeyIdGenMethod::SPKFullDER(HashAlgorithm::SHA2_256));

        let issuer_infos = inner.issuer_infos.take().ok_or(CertError::MissingBuilderArgument {
            arg: field_str!(issuer_infos),
        })?;
        let (issuer_name, issuer_key, aki, subject_infos) = {
            let (aki, subject_infos) = if issuer_infos.self_signed {
                let public_key = issuer_infos.key.to_public_key();
                let aki = key_id_gen_method
                    .generate_from(&public_key)
                    .map_err(|e| CertError::KeyIdGen { source: e })
                    .map_err(|e| CertError::CertGeneration { source: Box::new(e) })?;
                let subject_infos = SubjectInfos::NameAndPublicKey {
                    name: issuer_infos.name.clone(),
                    public_key,
                };
                (aki, subject_infos)
            } else {
                let aki = inner
                    .authority_key_identifier
                    .take()
                    .ok_or(CertError::MissingBuilderArgument {
                        arg: field_str!(authority_key_identifier),
                    })?;
                let subject_infos = inner.subject_infos.take().ok_or(CertError::MissingBuilderArgument {
                    arg: field_str!(subject_infos),
                })?;
                (aki, subject_infos)
            };

            (issuer_infos.name, issuer_infos.key, aki, subject_infos)
        };
        let (subject_name, subject_public_key, ext_req) = match subject_infos {
            SubjectInfos::Csr(csr) => {
                csr.verify().map_err(|e| CertError::InvalidCsr { source: e })?;

                let ext_req = ((csr.0.certification_request_info.attributes.0).0)
                    .into_iter()
                    .find_map(|attr| match attr.value {
                        picky_asn1_x509::AttributeValues::Extensions(set_of_extensions) => {
                            set_of_extensions.0.into_iter().next()
                        }
                        _ => None,
                    });

                let subject_name = csr.0.certification_request_info.subject.into();
                let subject_public_key = csr.0.certification_request_info.subject_public_key_info.into();

                (subject_name, subject_public_key, ext_req)
            }
            SubjectInfos::NameAndPublicKey { name, public_key } => (name, public_key, None),
        };

        let ca = inner.ca.take().unwrap_or(false);
        let pathlen = inner.pathlen.take();
        let key_usage_opt = inner.key_usage.take();
        let extended_key_usage_opt = inner.extended_key_usage.take();
        let subject_alt_name_opt = inner.subject_alt_name.take();
        let issuer_alt_name_opt = inner.issuer_alt_name.take();

        let serial_number = if let Some(unsigned_integer_bytes) = inner.serial_number.take() {
            IntegerAsn1::from_bytes_be_unsigned(unsigned_integer_bytes)
        } else {
            generate_serial_number()
        };

        let inherit_extensions_from_csr_attributes = inner.inherit_extensions_from_csr_attributes;

        drop(inner);

        let validity = Validity {
            not_before: valid_from.into(),
            not_after: valid_to.into(),
        };

        let extensions = {
            let mut extensions = Vec::new();

            // key usage + basic constraints
            if let Some(key_usage) = key_usage_opt {
                if key_usage.digital_signature() {
                    extensions.push(Extension::new_basic_constraints(ca, pathlen).into_critical());
                } else {
                    extensions.push(Extension::new_basic_constraints(ca, pathlen).into_non_critical());
                }
                extensions.push(Extension::new_key_usage(key_usage));
            } else {
                extensions.push(Extension::new_basic_constraints(ca, pathlen).into_non_critical());
            }

            // eku
            if let Some(extended_key_usage) = extended_key_usage_opt {
                extensions.push(Extension::new_extended_key_usage(extended_key_usage));
            }

            // san
            if let Some(san) = subject_alt_name_opt {
                extensions.push(Extension::new_subject_alt_name(san));
            }

            // ian
            if let Some(ian) = issuer_alt_name_opt {
                extensions.push(Extension::new_issuer_alt_name(ian));
            }

            // ski
            let ski = key_id_gen_method
                .generate_from(&subject_public_key)
                .map_err(|e| CertError::KeyIdGen { source: e })
                .map_err(|e| CertError::CertGeneration { source: Box::new(e) })?;
            extensions.push(Extension::new_subject_key_identifier(ski));

            // aki
            extensions.push(Extension::new_authority_key_identifier(
                KeyIdentifier::from(aki),
                None,
                None,
            ));

            // inherit extensions from csr "request extension" attribute if allowed to
            match ext_req {
                Some(requested_exts) if inherit_extensions_from_csr_attributes => {
                    for requested_ext in requested_exts.0 {
                        if !extensions.iter().any(|o| requested_ext.extn_id() == o.extn_id()) {
                            extensions.push(requested_ext);
                        }
                    }
                }
                _ => {}
            }

            Extensions(extensions)
        };

        let signature =
            AlgorithmIdentifier::try_from(signature_hash_type).map_err(|e| CertError::Signature { source: e })?;

        let tbs_certificate = TbsCertificate {
            version: ExplicitContextTag0(Version::V3),
            serial_number,
            signature,
            issuer: Name::from(issuer_name),
            validity,
            subject: Name::from(subject_name),
            subject_public_key_info: SubjectPublicKeyInfo::from(subject_public_key),
            extensions: ExplicitContextTag3(extensions),
        };

        let signature_algorithm = signature_hash_type
            .try_into()
            .map_err(|e| CertError::Signature { source: e })?;

        let tbs_der = picky_asn1_der::to_vec(&tbs_certificate)
            .map_err(|e| CertError::Asn1Serialization {
                source: e,
                element: "tbs certificate",
            })
            .map_err(|e| CertError::CertGeneration { source: Box::new(e) })?;

        let signature_value = BitString::with_bytes(
            signature_hash_type
                .sign(&tbs_der, issuer_key)
                .map_err(|e| CertError::Signature { source: e })
                .map_err(|e| CertError::CertGeneration { source: Box::new(e) })?,
        )
        .into();

        let signature_algorithm_der =
            picky_asn1_der::to_vec(&signature_algorithm).map_err(|source| CertError::Asn1Serialization {
                element: "signature_algorithm",
                source,
            })?;

        let signature_value_der =
            picky_asn1_der::to_vec(&signature_value).map_err(|source| CertError::Asn1Serialization {
                element: "signature_value",
                source,
            })?;

        Ok(Cert {
            details: Certificate {
                tbs_certificate,
                signature_algorithm,
                signature_value,
            },
            overview: CertificateOverview {
                tbs_certificate: Asn1RawDer(tbs_der),
                signature_algorithm: Asn1RawDer(signature_algorithm_der),
                signature_value: Asn1RawDer(signature_value_der),
            },
        })
    }
}

fn generate_serial_number() -> IntegerAsn1 {
    let x = rand::random::<u32>();
    let b1 = ((x >> 24) & 0xff) as u8;
    let b2 = ((x >> 16) & 0xff) as u8;
    let b3 = ((x >> 8) & 0xff) as u8;
    let b4 = (x & 0xff) as u8;
    // serial number MUST be a positive integer
    IntegerAsn1::from_bytes_be_unsigned(vec![b1, b2, b3, b4])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pem::{parse_pem, Pem};
    use crate::x509::csr::Attribute;

    #[test]
    fn read_pem_and_parse_certificate() {
        let pem = parse_pem(crate::test_files::INTERMEDIATE_CA.as_bytes()).unwrap();
        let cert = Cert::from_der(pem.data()).unwrap();

        assert_eq!(cert.serial_number(), &vec![1]);
        assert_eq!(
            Into::<String>::into(cert.signature_algorithm().oid()).as_str(),
            oids::SHA1_WITH_RSA_ENCRYPTION
        );
        assert_eq!(cert.valid_not_before(), UtcDate::new(2011, 2, 12, 14, 44, 6).unwrap());
        assert_eq!(cert.valid_not_after(), UtcDate::new(2021, 2, 12, 14, 44, 6).unwrap());

        assert_eq!(cert.issuer_name().to_string(), "C=NL,O=PolarSSL,CN=PolarSSL Test CA");
    }

    #[test]
    fn get_identifier() {
        let pem = crate::test_files::RSA_2048_PK_1
            .parse::<Pem>()
            .expect("couldn't parse pem");
        let private_key = PrivateKey::from_pkcs8(pem.data()).expect("couldn't extract private key from pkcs8");

        // validity
        let valid_from = UtcDate::ymd(2019, 10, 10).unwrap();
        let valid_to = UtcDate::ymd(2019, 10, 11).unwrap();

        let root = CertificateBuilder::new()
            .validity(valid_from, valid_to)
            .self_signed(DirectoryName::new_common_name("test"), &private_key)
            .ca(true)
            .build()
            .expect("couldn't generate root ca");

        root.subject_key_identifier()
            .expect("couldn't get subject key identifier");
        root.authority_key_identifier()
            .expect("couldn't get authority key identifier");

        assert_eq!(root.ty(), CertType::Root);
    }

    #[test]
    fn key_id_and_cert() {
        let kid = "c4a7b1a47b2c71fadbe14b9075ffc41560858910";
        let pem = crate::test_files::ROOT_CA.parse::<Pem>().expect("couldn't parse PEM");
        let cert = Cert::from_der(pem.data()).expect("couldn't deserialize certificate");
        assert_eq!(cert.ty(), CertType::Root);
        let key_id = cert
            .subject_key_identifier()
            .expect("couldn't get subject key identifier");
        pretty_assertions::assert_eq!(hex::encode(&key_id), kid);
    }

    fn parse_key(pem_str: &str) -> PrivateKey {
        let pem = pem_str.parse::<Pem>().unwrap();
        PrivateKey::from_pkcs8(pem.data()).unwrap()
    }

    #[test]
    fn valid_ca_chain() {
        let root_key = parse_key(crate::test_files::RSA_2048_PK_1);
        let intermediate_key = parse_key(crate::test_files::RSA_2048_PK_2);
        let leaf_key = parse_key(crate::test_files::RSA_2048_PK_3);

        let root = CertificateBuilder::new()
            .validity(UtcDate::ymd(2065, 6, 15).unwrap(), UtcDate::ymd(2070, 6, 15).unwrap())
            .self_signed(DirectoryName::new_common_name("TheFuture.usodakedo Root CA"), &root_key)
            .ca(true)
            .signature_hash_type(SignatureAlgorithm::RsaPkcs1v15(HashAlgorithm::SHA2_512))
            .key_id_gen_method(KeyIdGenMethod::SPKFullDER(HashAlgorithm::SHA2_384))
            .build()
            .expect("couldn't build root ca");
        assert_eq!(root.ty(), CertType::Root);

        let intermediate = CertificateBuilder::new()
            .validity(UtcDate::ymd(2068, 1, 1).unwrap(), UtcDate::ymd(2071, 1, 1).unwrap())
            .subject(
                DirectoryName::new_common_name("TheFuture.usodakedo Authority"),
                intermediate_key.to_public_key(),
            )
            .issuer_cert(&root, &root_key)
            .signature_hash_type(SignatureAlgorithm::RsaPkcs1v15(HashAlgorithm::SHA2_224))
            .key_id_gen_method(KeyIdGenMethod::SPKValueHashedLeftmost160(HashAlgorithm::SHA1))
            .ca(true)
            .pathlen(0)
            .build()
            .expect("couldn't build intermediate ca");
        assert_eq!(intermediate.ty(), CertType::Intermediate);

        let csr = Csr::generate(
            DirectoryName::new_common_name("ChillingInTheFuture.usobakkari"),
            &leaf_key,
            SignatureAlgorithm::RsaPkcs1v15(HashAlgorithm::SHA1),
        )
        .unwrap();

        let signed_leaf = CertificateBuilder::new()
            .validity(UtcDate::ymd(2069, 1, 1).unwrap(), UtcDate::ymd(2072, 1, 1).unwrap())
            .subject_from_csr(csr)
            .issuer_cert(&intermediate, &intermediate_key)
            .signature_hash_type(SignatureAlgorithm::RsaPkcs1v15(HashAlgorithm::SHA2_384))
            .key_id_gen_method(KeyIdGenMethod::SPKFullDER(HashAlgorithm::SHA2_512))
            .pathlen(0) // not meaningful in non-CA certificates
            .build()
            .expect("couldn't build signed leaf");
        assert_eq!(signed_leaf.ty(), CertType::Leaf);

        let chain = [intermediate, root];

        // check with exact date
        signed_leaf
            .verifier()
            .chain(chain.iter())
            .exact_date(&UtcDate::ymd(2069, 10, 1).unwrap())
            .verify()
            .expect("couldn't verify chain");

        // check with interval date
        signed_leaf
            .verifier()
            .chain(chain.iter())
            .interval_date(
                &UtcDate::new(2068, 12, 31, 23, 59, 59).unwrap(),
                &UtcDate::ymd(2069, 1, 1).unwrap(),
            )
            .verify()
            .expect("couldn't verify chain with interval date");

        // check with ignore not before
        signed_leaf
            .verifier()
            .chain(chain.iter())
            .exact_date(&UtcDate::new(2068, 12, 31, 23, 59, 59).unwrap())
            .ignore_not_before_check()
            .verify()
            .expect("couldn't verify chain with interval date");

        // check with no date validity check
        signed_leaf
            .verifier()
            .chain(chain.iter())
            .ignore_not_after_check()
            .ignore_not_before_check()
            .verify()
            .expect("couldn't verify chain with no date validity check");

        let expired_err = signed_leaf
            .verifier()
            .chain(chain.iter())
            .exact_date(&UtcDate::ymd(2080, 10, 1).unwrap())
            .verify()
            .unwrap_err();
        assert_eq!(
            expired_err.to_string(),
            "invalid certificate \'CN=ChillingInTheFuture.usobakkari\': \
            certificate expired (not after: 2072-01-01 00:00:00, now: 2080-10-01 00:00:00)"
        );

        let intermediate_expired_err = signed_leaf
            .verifier()
            .chain(chain.iter())
            .exact_date(&UtcDate::ymd(2071, 6, 1).unwrap())
            .verify()
            .unwrap_err();
        assert_eq!(
            intermediate_expired_err.to_string(),
            "invalid certificate \'CN=TheFuture.usodakedo Authority\': \
            certificate expired (not after: 2071-01-01 00:00:00, now: 2071-06-01 00:00:00)"
        );

        let root_expired_err = signed_leaf
            .verifier()
            .chain(chain.iter())
            .exact_date(&UtcDate::ymd(2070, 6, 16).unwrap())
            .verify()
            .unwrap_err();
        assert_eq!(
            root_expired_err.to_string(),
            "invalid certificate \'CN=TheFuture.usodakedo Root CA\': \
            certificate expired (not after: 2070-06-15 00:00:00, now: 2070-06-16 00:00:00)"
        );

        let still_in_2019_err = signed_leaf
            .verifier()
            .chain(chain.iter())
            .exact_date(&UtcDate::ymd(2019, 11, 14).unwrap())
            .verify()
            .unwrap_err();
        assert_eq!(
            still_in_2019_err.to_string(),
            "invalid certificate \'CN=ChillingInTheFuture.usobakkari\': \
            certificate is not yet valid (not before: 2069-01-01 00:00:00, now: 2019-11-14 00:00:00)"
        );

        let not_yet_valid_with_interval_err = signed_leaf
            .verifier()
            .chain(chain.iter())
            .interval_date(
                &UtcDate::ymd(2068, 12, 30).unwrap(),
                &UtcDate::ymd(2068, 12, 31).unwrap(),
            )
            .verify()
            .unwrap_err();
        assert_eq!(
            not_yet_valid_with_interval_err.to_string(),
            "invalid certificate \'CN=ChillingInTheFuture.usobakkari\': \
            certificate is not yet valid (not before: 2069-01-01 00:00:00, now: 2068-12-31 00:00:00)"
        );

        let date_is_missing_err = signed_leaf.verifier().chain(chain.iter()).verify().unwrap_err();
        assert_eq!(
            date_is_missing_err.to_string(),
            "missing required builder argument `now`"
        );
    }

    #[test]
    fn malicious_ca_chain() {
        let root_key = parse_key(crate::test_files::RSA_2048_PK_1);
        let intermediate_key = parse_key(crate::test_files::RSA_2048_PK_2);
        let leaf_key = parse_key(crate::test_files::RSA_2048_PK_3);
        let malicious_root_key = parse_key(crate::test_files::RSA_2048_PK_4);

        let root = CertificateBuilder::new()
            .validity(UtcDate::ymd(2065, 6, 15).unwrap(), UtcDate::ymd(2070, 6, 15).unwrap())
            .self_signed(DirectoryName::new_common_name("VerySafe Root CA"), &root_key)
            .ca(true)
            .pathlen(1)
            .signature_hash_type(SignatureAlgorithm::RsaPkcs1v15(HashAlgorithm::SHA1))
            .key_id_gen_method(KeyIdGenMethod::SPKFullDER(HashAlgorithm::SHA2_224))
            .build()
            .expect("couldn't build root ca");

        let intermediate = CertificateBuilder::new()
            .validity(UtcDate::ymd(2068, 1, 1).unwrap(), UtcDate::ymd(2071, 1, 1).unwrap())
            .subject(
                DirectoryName::new_common_name("V.E.R.Y Legitimate VerySafe Authority"),
                intermediate_key.to_public_key(),
            )
            .issuer_cert(&root, &malicious_root_key)
            .signature_hash_type(SignatureAlgorithm::RsaPkcs1v15(HashAlgorithm::SHA2_512))
            .key_id_gen_method(KeyIdGenMethod::SPKValueHashedLeftmost160(HashAlgorithm::SHA2_384))
            .ca(true)
            .pathlen(0)
            .build()
            .expect("couldn't build intermediate ca");

        let csr = Csr::generate(
            DirectoryName::new_common_name("I Trust This V.E.R.Y Legitimate Intermediate Certificate"),
            &leaf_key,
            SignatureAlgorithm::RsaPkcs1v15(HashAlgorithm::SHA1),
        )
        .unwrap();

        let signed_leaf = CertificateBuilder::new()
            .validity(UtcDate::ymd(2069, 1, 1).unwrap(), UtcDate::ymd(2072, 1, 1).unwrap())
            .subject_from_csr(csr)
            .issuer_cert(&intermediate, &intermediate_key)
            .signature_hash_type(SignatureAlgorithm::RsaPkcs1v15(HashAlgorithm::SHA2_224))
            .key_id_gen_method(KeyIdGenMethod::SPKFullDER(HashAlgorithm::SHA2_384))
            .build()
            .expect("couldn't build signed leaf");

        let chain = [intermediate, root];

        let root_missing_err = signed_leaf
            .verifier()
            .chain(chain[..1].iter())
            .exact_date(&UtcDate::ymd(2069, 10, 1).unwrap())
            .verify()
            .unwrap_err();
        assert_eq!(
            root_missing_err.to_string(),
            "CA chain error: chain is missing a root certificate"
        );

        let invalid_sig_err = signed_leaf
            .verifier()
            .chain(chain.iter())
            .exact_date(&UtcDate::ymd(2069, 10, 1).unwrap())
            .verify()
            .unwrap_err();
        assert_eq!(
            invalid_sig_err.to_string(),
            "invalid certificate \'CN=V.E.R.Y Legitimate VerySafe Authority\': signature error: invalid signature"
        );
    }

    #[test]
    fn invalid_basic_constraints_chain() {
        let root_key = parse_key(crate::test_files::RSA_2048_PK_1);
        let intermediate_key = parse_key(crate::test_files::RSA_2048_PK_2);
        let leaf_key = parse_key(crate::test_files::RSA_2048_PK_3);

        let root = CertificateBuilder::new()
            .validity(UtcDate::ymd(2065, 6, 15).unwrap(), UtcDate::ymd(2070, 6, 15).unwrap())
            .self_signed(DirectoryName::new_common_name("VerySafe Root CA"), &root_key)
            .ca(true)
            .pathlen(0)
            .build()
            .expect("couldn't build root ca");

        let intermediate = CertificateBuilder::new()
            .validity(UtcDate::ymd(2068, 1, 1).unwrap(), UtcDate::ymd(2071, 1, 1).unwrap())
            .subject(
                DirectoryName::new_common_name("V.E.R.Y Legitimate VerySafe Authority"),
                intermediate_key.to_public_key(),
            )
            .issuer_cert(&root, &root_key)
            .ca(true)
            .pathlen(0)
            .build()
            .expect("couldn't build intermediate ca");

        let csr = Csr::generate(
            DirectoryName::new_common_name("I Trust This V.E.R.Y Legitimate Intermediate Certificate"),
            &leaf_key,
            SignatureAlgorithm::RsaPkcs1v15(HashAlgorithm::SHA1),
        )
        .unwrap();

        let signed_leaf = CertificateBuilder::new()
            .validity(UtcDate::ymd(2069, 1, 1).unwrap(), UtcDate::ymd(2072, 1, 1).unwrap())
            .subject_from_csr(csr.clone())
            .issuer_cert(&intermediate, &intermediate_key)
            .build()
            .expect("couldn't build signed leaf");

        let chain = [intermediate.clone(), root.clone()];

        let invalid_pathlen_err = signed_leaf
            .verifier()
            .chain(chain.iter())
            .exact_date(&UtcDate::ymd(2069, 10, 1).unwrap())
            .verify()
            .unwrap_err();
        assert_eq!(
            invalid_pathlen_err.to_string(),
            "CA chain error: chain depth doesn\'t satisfy basic constraints extension: \
            certificate \'CN=VerySafe Root CA\' has pathlen of 0"
        );

        let invalid_issuer_signed_leaf = CertificateBuilder::new()
            .validity(UtcDate::ymd(2069, 1, 1).unwrap(), UtcDate::ymd(2072, 1, 1).unwrap())
            .subject_from_csr(csr)
            .issuer_cert(&signed_leaf, &leaf_key)
            .build()
            .expect("couldn't build invalid issuer signed leaf");

        let chain = [signed_leaf, intermediate.clone(), root.clone()];

        let invalid_issuer_err = invalid_issuer_signed_leaf
            .verifier()
            .chain(chain.iter())
            .exact_date(&UtcDate::ymd(2069, 10, 1).unwrap())
            .verify()
            .unwrap_err();
        assert_eq!(
            invalid_issuer_err.to_string(),
            "CA chain error: issuer certificate \'CN=I Trust This V.E.R.Y Legitimate Intermediate Certificate\' is not a CA"
        );
    }

    #[test]
    fn bypass_serial_number_generator() {
        let root_key = parse_key(crate::test_files::RSA_2048_PK_1);

        let unsigned_integer_bytes = [21, 84, 58, 122];

        let cert = CertificateBuilder::new()
            .validity(UtcDate::ymd(2065, 6, 15).unwrap(), UtcDate::ymd(2070, 6, 15).unwrap())
            .self_signed(DirectoryName::new_common_name("TheFuture.usodakedo Root CA"), &root_key)
            .ca(true)
            .signature_hash_type(SignatureAlgorithm::RsaPkcs1v15(HashAlgorithm::SHA2_512))
            .key_id_gen_method(KeyIdGenMethod::SPKFullDER(HashAlgorithm::SHA2_384))
            .serial_number(unsigned_integer_bytes.to_vec())
            .build()
            .expect("couldn't build root ca");

        assert_eq!(cert.serial_number().as_unsigned_bytes_be(), unsigned_integer_bytes);
    }

    #[test]
    fn validity_encoding() {
        use picky_asn1_x509::validity::Time;

        let root_key = parse_key(crate::test_files::RSA_2048_PK_1);

        let cert = CertificateBuilder::new()
            .validity(UtcDate::ymd(2045, 6, 15).unwrap(), UtcDate::ymd(2055, 6, 15).unwrap())
            .self_signed(DirectoryName::new_common_name("Am I valid"), &root_key)
            .ca(true)
            .signature_hash_type(SignatureAlgorithm::RsaPkcs1v15(HashAlgorithm::SHA1))
            .key_id_gen_method(KeyIdGenMethod::SPKFullDER(HashAlgorithm::SHA2_224))
            .build()
            .expect("couldn't build root ca");

        let validity = &cert.details.tbs_certificate.validity;

        assert!(matches!(validity.not_before, Time::Utc(_)));
        assert!(matches!(validity.not_after, Time::Generalized(_)));
    }

    #[test]
    fn inherit_requested_extensions_by_csr() {
        use crate::x509::name::GeneralName;

        let root_key = parse_key(crate::test_files::RSA_2048_PK_1);
        let leaf_key = parse_key(crate::test_files::RSA_2048_PK_3);

        let root = CertificateBuilder::new()
            .validity(UtcDate::ymd(2065, 6, 15).unwrap(), UtcDate::ymd(2070, 6, 15).unwrap())
            .self_signed(DirectoryName::new_common_name("VerySafe Root CA"), &root_key)
            .ca(true)
            .pathlen(0)
            .build()
            .expect("couldn't build root ca");

        let extensions =
            vec![Extension::new_subject_alt_name(GeneralName::new_dns_name("localhost").unwrap()).into_non_critical()];
        let attr = Attribute::new_extension_request(extensions);
        let csr = Csr::generate_with_attributes(
            DirectoryName::new_common_name("I want more extensions"),
            &leaf_key,
            SignatureAlgorithm::RsaPkcs1v15(HashAlgorithm::SHA2_256),
            vec![attr],
        )
        .unwrap();

        let signed_leaf = CertificateBuilder::new()
            .validity(UtcDate::ymd(2069, 1, 1).unwrap(), UtcDate::ymd(2072, 1, 1).unwrap())
            .subject_from_csr(csr)
            .issuer_cert(&root, &root_key)
            .inherit_extensions_from_csr_attributes(true)
            .build()
            .expect("couldn't build signed leaf");

        let subject_alt_name = signed_leaf
            .extensions()
            .iter()
            .find_map(|ext| match ext.extn_value() {
                ExtensionView::SubjectAltName(gn) => match gn.0.first().unwrap() {
                    picky_asn1_x509::GeneralName::DnsName(name) => Some(name.to_string()),
                    _ => None,
                },
                _ => None,
            })
            .unwrap();

        assert_eq!(subject_alt_name, "localhost");
    }

    /// We noticed a few Authenticode certificates where encoded using a constructed (explicit)
    /// context tag instead of a primitive (implicit) context tag for the subject alternative name
    /// extension (notably PSDiagnostics PowerShell module).
    ///
    /// Relevant documentation from RFC5280 Appendix A.2:
    /// ```not_rust
    /// DEFINITIONS IMPLICIT TAGS ::=
    ///
    /// […]
    ///
    /// GeneralName ::= CHOICE {
    ///      otherName                 [0]  AnotherName,
    ///      rfc822Name                [1]  IA5String,
    ///      dNSName                   [2]  IA5String,
    ///      x400Address               [3]  ORAddress,
    ///      directoryName             [4]  Name,
    ///      ediPartyName              [5]  EDIPartyName,
    ///      uniformResourceIdentifier [6]  IA5String,
    ///      iPAddress                 [7]  OCTET STRING,
    ///      registeredID              [8]  OBJECT IDENTIFIER }
    /// ```
    /// [Link](https://datatracker.ietf.org/doc/html/rfc5280#appendix-A.2)
    ///
    /// `DEFINITIONS IMPLICIT TAGS ::=` is used to specify that except stated otherwise, tags are
    /// implicits (also said primitives).
    ///
    /// Picky is encoding this using an implicit context tag as specified by the RFC, and this is a
    /// problem when validating some Windows certificates because picky 6.3.0 (and prior) is fully
    /// parsing the certificate and encode back into der when validating the signature causing
    /// signature validation to fail.
    ///
    /// To improve validation robustness, it was decided starting picky 6.4.0 to use the originally
    /// parsed DER representation internally instead of re-encoding on demand.
    ///
    /// The aforementioned PSDiagnostics module certificate chain is used as test case to validate
    /// this behavior.
    #[cfg(feature = "chrono_conversion")]
    #[test]
    fn psdiag_constructed_context_tag_in_subject_alt_name_ext() {
        let leaf = Cert::from_pem_str(crate::test_files::PSDIAG_LEAF).unwrap();
        let inter = Cert::from_pem_str(crate::test_files::PSDIAG_INTER).unwrap();
        let root = Cert::from_pem_str(crate::test_files::PSDIAG_ROOT).unwrap();

        let chain = [inter, root];
        let unexpired_date = UtcDate::new(2021, 11, 21, 1, 0, 0).unwrap();

        leaf.verifier()
            .exact_date(&unexpired_date)
            .chain(chain.iter())
            .verify()
            .unwrap();
    }
}
