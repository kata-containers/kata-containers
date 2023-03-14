pub use picky_asn1_x509::attribute::Attribute;
pub use picky_asn1_x509::pkcs7::content_info;
pub use picky_asn1_x509::ShaVariant;

use crate::hash::{HashAlgorithm, UnsupportedHashAlgorithmError};
use crate::key::PrivateKey;
use crate::pem::Pem;
use crate::signature::{SignatureAlgorithm, SignatureError};
use crate::x509::certificate::{Cert, CertError, CertType, ValidityCheck};
use crate::x509::date::UtcDate;
use crate::x509::name::DirectoryName;
#[cfg(feature = "ctl")]
use crate::x509::pkcs7::ctl::{self, CTLEntryAttributeValues, CertificateTrustList};
use crate::x509::pkcs7::timestamp::{self, Timestamper};
use crate::x509::pkcs7::{self, Pkcs7};
use crate::x509::utils::{from_der, from_pem, from_pem_str, to_der, to_pem};
use picky_asn1::restricted_string::CharSetError;
use picky_asn1::tag::Tag;
use picky_asn1::wrapper::{Asn1SetOf, ExplicitContextTag0, ExplicitContextTag1, ObjectIdentifierAsn1};
use picky_asn1_der::Asn1DerError;
use picky_asn1_x509::algorithm_identifier::{AlgorithmIdentifier, UnsupportedAlgorithmError};
use picky_asn1_x509::cmsversion::CmsVersion;
use picky_asn1_x509::extension::ExtensionView;
use picky_asn1_x509::pkcs7::content_info::{
    ContentValue, EncapsulatedContentInfo, SpcAttributeAndOptionalValue, SpcAttributeAndOptionalValueValue,
    SpcIndirectDataContent, SpcLink, SpcPeImageData, SpcPeImageFlags, SpcSpOpusInfo, SpcString,
};
use picky_asn1_x509::pkcs7::crls::RevocationInfoChoices;
use picky_asn1_x509::pkcs7::signed_data::{
    CertificateChoices, CertificateSet, DigestAlgorithmIdentifiers, SignedData, SignersInfos,
};
use picky_asn1_x509::pkcs7::signer_info::{
    Attributes, CertificateSerialNumber, DigestAlgorithmIdentifier, IssuerAndSerialNumber,
    SignatureAlgorithmIdentifier, SignatureValue, SignerIdentifier, SignerInfo, UnsignedAttribute,
    UnsignedAttributeValue, UnsignedAttributes,
};
use picky_asn1_x509::pkcs7::Pkcs7Certificate;
use picky_asn1_x509::{oids, AttributeValues, Certificate, DigestInfo, Name};
use std::cell::RefCell;
use std::iter::Iterator;
use std::ops::DerefMut;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AuthenticodeError {
    #[error(transparent)]
    Asn1DerError(#[from] Asn1DerError),
    #[error("The Authenticode signature CA is not trusted")]
    CAIsNotTrusted,
    #[error("CA certificate was revoked")]
    CaCertificateRevoked,
    #[error("CA certificate was revoked(since: {not_after}, now: {now})")]
    CaCertificateExpired { not_after: UtcDate, now: UtcDate },
    #[error("CA certificate is not yet valid(not before:  {not_before}, now: {now})")]
    CaCertificateNotYetValid { not_before: UtcDate, now: UtcDate },
    #[error(transparent)]
    CertError(#[from] CertError),
    #[error("Digest algorithm mismatch: {description}")]
    DigestAlgorithmMismatch { description: String },
    #[error("PKCS9_MESSAGE_DIGEST does not match ContentInfo hash")]
    HashMismatch,
    #[error("Actual file hash is {actual:?}, but expected {expected:?}")]
    FileHashMismatch { actual: Vec<u8>, expected: Vec<u8> },
    #[error("Authenticode signatures support only one signer, digestAlgorithms must contain only one digestAlgorithmIdentifier, but {incorrect_count} entries present")]
    IncorrectDigestAlgorithmsCount { incorrect_count: usize },
    #[error(
        "Authenticode uses issuerAndSerialNumber to identify the signer, but got subjectKeyIdentifier identification"
    )]
    IncorrectSignerIdentifier,
    #[error("Incorrect version. Expected: {expected}, but got {got}")]
    IncorrectVersion { expected: u32, got: u32 },
    #[error("Authenticode must contain only one SignerInfo, but got {count}")]
    MultipleSignerInfo { count: usize },
    #[error("EncapsulatedContentInfo is missing")]
    NoEncapsulatedContentInfo,
    #[error("EncapsulatedContentInfo should contain SpcIndirectDataContent")]
    NoSpcIndirectDataContent,
    #[error("Certificates must contain at least Leaf and Intermediate certificates, but got no certificates")]
    NoCertificates,
    #[error("No Intermediate certificate")]
    NoIntermediateCertificate,
    #[error("The signing certificate must contain the extended key usage (EKU) value for code signing")]
    NoEKUCodeSigning,
    #[error("PKCS9_MESSAGE_DIGEST attribute is absent")]
    NoMessageDigest,
    #[error("Can't find certificate for issuer: {issuer}, and serial_number:  {serial_number:#?}")]
    NoCertificatesAssociatedWithIssuerAndSerialNumber { issuer: Name, serial_number: Vec<u8> },
    #[error("Timestamp has invalid certificate: {0:?}")]
    InvalidTimestampCert(CertError),
    #[error(transparent)]
    SignatureError(#[from] SignatureError),
    #[error("the program name has invalid charset")]
    ProgramNameCharSet(#[from] CharSetError),
    #[error(transparent)]
    UnsupportedHashAlgorithmError(UnsupportedHashAlgorithmError),
    #[error(transparent)]
    UnsupportedAlgorithmError(UnsupportedAlgorithmError),
    #[cfg(feature = "ctl")]
    #[error(transparent)]
    CtlError(#[from] ctl::CtlError),
    #[error(transparent)]
    TimestampError(timestamp::TimestampError),
}

type AuthenticodeResult<T> = Result<T, AuthenticodeError>;

#[derive(Clone, Debug, PartialEq)]
pub struct AuthenticodeSignature(pub Pkcs7);

impl AuthenticodeSignature {
    pub fn new(
        pkcs7: &Pkcs7,
        file_hash: Vec<u8>,
        hash_algo: ShaVariant,
        private_key: &PrivateKey,
        program_name: Option<String>,
    ) -> Result<Self, AuthenticodeError> {
        let digest_algorithm = AlgorithmIdentifier::new_sha(hash_algo);

        let data = SpcAttributeAndOptionalValue {
            ty: oids::spc_pe_image_dataobj().into(),
            value: SpcAttributeAndOptionalValueValue::SpcPeImageData(SpcPeImageData {
                flags: SpcPeImageFlags::default(),
                file: Default::default(),
            }),
        };

        let message_digest = DigestInfo {
            oid: digest_algorithm.clone(),
            digest: file_hash.to_vec().into(),
        };

        let program_name = program_name
            .map(SpcString::try_from)
            .transpose()?
            .map(ExplicitContextTag0);

        let mut raw_spc_indirect_data_content = picky_asn1_der::to_vec(&data)?;

        let mut raw_message_digest = picky_asn1_der::to_vec(&message_digest)?;

        raw_spc_indirect_data_content.append(&mut raw_message_digest);

        let message_digest_value = HashAlgorithm::try_from(hash_algo)
            .map_err(AuthenticodeError::UnsupportedHashAlgorithmError)?
            .digest(raw_spc_indirect_data_content.as_ref());

        let authenticated_attributes = vec![
            Attribute {
                ty: oids::content_type().into(),
                value: AttributeValues::ContentType(Asn1SetOf(vec![oids::spc_indirect_data_objid().into()])),
            },
            Attribute {
                ty: oids::spc_sp_opus_info_objid().into(),
                value: AttributeValues::SpcSpOpusInfo(Asn1SetOf(vec![SpcSpOpusInfo {
                    program_name,
                    more_info: Some(ExplicitContextTag1(SpcLink::default())),
                }])),
            },
            Attribute {
                ty: oids::message_digest().into(),
                value: AttributeValues::MessageDigest(Asn1SetOf(vec![message_digest_value.into()])),
            },
        ];

        let content = SpcIndirectDataContent { data, message_digest };

        let content_info = EncapsulatedContentInfo {
            content_type: oids::spc_indirect_data_objid().into(),
            content: Some(ContentValue::SpcIndirectDataContent(content).into()),
        };

        let certificates = pkcs7.decode_certificates();

        let signing_cert = certificates.get(0).ok_or(AuthenticodeError::NoCertificates)?;

        let issuer_and_serial_number = IssuerAndSerialNumber {
            issuer: signing_cert.issuer_name().into(),
            serial_number: CertificateSerialNumber(signing_cert.serial_number().clone()),
        };

        // The signing certificate must contain either the extended key usage (EKU) value for code signing,
        // or the entire certificate chain must contain no EKUs
        h_check_eku_code_signing(&certificates, signing_cert)?;

        // certificates contains the signer certificate and any intermediate certificates,
        // but typically does not contain the root certificate
        let certificates = if certificates.len() == 1 && certificates[0].ty() == CertType::Root {
            vec![Certificate::from(certificates[0].clone())]
        } else {
            certificates
                .into_iter()
                .filter_map(|cert| {
                    if cert.ty() != CertType::Root {
                        Some(Certificate::from(cert))
                    } else {
                        None
                    }
                })
                .collect::<Vec<Certificate>>()
        };

        let digest_encryption_algorithm = AlgorithmIdentifier::new_rsa_encryption_with_sha(hash_algo)
            .map_err(AuthenticodeError::UnsupportedAlgorithmError)?;

        let signature_algo = SignatureAlgorithm::from_algorithm_identifier(&digest_encryption_algorithm)?;

        let mut auth_raw_data = picky_asn1_der::to_vec(&authenticated_attributes)?;
        // According to the RFC:
        //
        // "[...] The Attributes value's tag is SET OF, and the DER encoding ofs
        // the SET OF tag, rather than of the IMPLICIT [0] tag [...]"
        auth_raw_data[0] = Tag::SET.inner();

        let encrypted_digest = SignatureValue(signature_algo.sign(auth_raw_data.as_ref(), private_key)?.into());

        let signer_info = SignerInfo {
            version: CmsVersion::V1,
            sid: SignerIdentifier::IssuerAndSerialNumber(issuer_and_serial_number),
            digest_algorithm: DigestAlgorithmIdentifier(digest_algorithm.clone()),
            signed_attrs: Attributes(authenticated_attributes.into()).into(),
            signature_algorithm: SignatureAlgorithmIdentifier(AlgorithmIdentifier::new_rsa_encryption()),
            signature: encrypted_digest,
            unsigned_attrs: UnsignedAttributes::default().into(),
        };

        let mut certs = Vec::new();
        for cert in certificates.into_iter() {
            let raw_certificates = picky_asn1_der::to_vec(&cert)?;
            certs.push(CertificateChoices::Certificate(picky_asn1_der::from_bytes(
                &raw_certificates,
            )?));
        }

        let signed_data = SignedData {
            version: CmsVersion::V1,
            digest_algorithms: DigestAlgorithmIdentifiers(vec![digest_algorithm].into()),
            content_info,
            certificates: CertificateSet(certs).into(),
            crls: Some(RevocationInfoChoices::default()),
            signers_infos: SignersInfos(vec![signer_info].into()),
        };

        Ok(AuthenticodeSignature(Pkcs7::from(Pkcs7Certificate {
            oid: oids::signed_data().into(),
            signed_data: signed_data.into(),
        })))
    }

    pub fn timestamp(
        &mut self,
        timestamper: &impl Timestamper,
        hash_algo: HashAlgorithm,
    ) -> Result<(), AuthenticodeError> {
        let signer_info = self
            .0
             .0
            .signed_data
            .signers_infos
            .0
             .0
            .first()
            .expect("Exactly one SignedInfo should be present");

        let encrypted_digest = signer_info.signature.0 .0.clone();

        let token = timestamper
            .timestamp(encrypted_digest, hash_algo)
            .map_err(AuthenticodeError::TimestampError)?;

        timestamper.modify_signed_data(token, &mut self.0 .0.signed_data);

        Ok(())
    }

    pub fn from_der<V: ?Sized + AsRef<[u8]>>(data: &V) -> AuthenticodeResult<Self> {
        Ok(from_der::<Pkcs7Certificate, V>(data, pkcs7::ELEMENT_NAME)
            .map(Pkcs7::from)
            .map(Self)?)
    }

    pub fn from_pem(pem: &Pem) -> AuthenticodeResult<Self> {
        Ok(
            from_pem::<Pkcs7Certificate>(pem, &[pkcs7::PKCS7_PEM_LABEL], pkcs7::ELEMENT_NAME)
                .map(Pkcs7::from)
                .map(Self)?,
        )
    }

    pub fn from_pem_str(pem_str: &str) -> AuthenticodeResult<Self> {
        Ok(
            from_pem_str::<Pkcs7Certificate>(pem_str, &[pkcs7::PKCS7_PEM_LABEL], pkcs7::ELEMENT_NAME)
                .map(Pkcs7::from)
                .map(Self)?,
        )
    }

    pub fn to_der(&self) -> AuthenticodeResult<Vec<u8>> {
        Ok(to_der(&self.0 .0, pkcs7::ELEMENT_NAME)?)
    }

    pub fn to_pem(&self) -> AuthenticodeResult<Pem<'static>> {
        Ok(to_pem(&self.0 .0, pkcs7::PKCS7_PEM_LABEL, pkcs7::ELEMENT_NAME)?)
    }

    pub fn signing_certificate<'a>(&'a self, certificates: &'a [Cert]) -> AuthenticodeResult<&'a Cert> {
        let signer_infos = &self.0 .0.signed_data.signers_infos.0;

        let signer_info = signer_infos.first().ok_or(AuthenticodeError::MultipleSignerInfo {
            count: signer_infos.len(),
        })?;

        let issuer_and_serial_number = match &signer_info.sid {
            SignerIdentifier::IssuerAndSerialNumber(issuer_and_serial_number) => issuer_and_serial_number,
            SignerIdentifier::SubjectKeyIdentifier(_) => return Err(AuthenticodeError::IncorrectSignerIdentifier),
        };

        certificates
            .iter()
            .find(|cert| {
                (Name::from(cert.issuer_name()) == issuer_and_serial_number.issuer)
                    && (cert.serial_number() == &issuer_and_serial_number.serial_number.0)
            })
            .ok_or_else(
                || AuthenticodeError::NoCertificatesAssociatedWithIssuerAndSerialNumber {
                    issuer: issuer_and_serial_number.issuer.clone(),
                    serial_number: issuer_and_serial_number.serial_number.0 .0.clone(),
                },
            )
    }

    pub fn authenticode_verifier(&self) -> AuthenticodeValidator {
        AuthenticodeValidator {
            authenticode_signature: self,
            inner: RefCell::new(AuthenticodeValidatorInner {
                strictness: Default::default(),
                now: None,
                excluded_cert_authorities: vec![],
                expected_file_hash: None,
                #[cfg(feature = "ctl")]
                ctl: None,
            }),
        }
    }

    pub fn file_hash(&self) -> Option<Vec<u8>> {
        let spc_indirect_data_content = match &self.0 .0.signed_data.content_info.content {
            Some(content_value) => match &content_value.0 {
                ContentValue::SpcIndirectDataContent(spc_indirect_data_content) => spc_indirect_data_content,
                _ => return None,
            },
            None => return None,
        };

        Some(spc_indirect_data_content.message_digest.digest.0.clone())
    }

    pub fn authenticated_attributes(&self) -> &[Attribute] {
        &self
            .0
            .signer_infos()
            .first()
            .expect("Exactly one SignerInfo should be present")
            .signed_attrs
            .0
             .0
    }

    pub fn unauthenticated_attributes(&self) -> &[UnsignedAttribute] {
        &self
            .0
             .0
            .signed_data
            .signers_infos
            .0
             .0
            .first()
            .expect("Exactly one SignerInfo should be present")
            .unsigned_attrs
            .0
             .0
    }
}

impl From<Pkcs7> for AuthenticodeSignature {
    fn from(pkcs7: Pkcs7) -> Self {
        AuthenticodeSignature(pkcs7)
    }
}

impl From<AuthenticodeSignature> for Pkcs7 {
    fn from(authenticode_signature: AuthenticodeSignature) -> Self {
        authenticode_signature.0
    }
}

#[derive(Debug, Clone)]
struct AuthenticodeStrictness {
    require_basic_authenticode_validation: bool,
    require_signing_certificate_check: bool,
    require_not_before_check: bool,
    require_not_after_check: bool,
    require_ca_verification_against_ctl: bool,
    require_chain_check: bool,
    exclude_specific_cert_authorities_from_ctl_check: bool,
}

impl Default for AuthenticodeStrictness {
    fn default() -> Self {
        AuthenticodeStrictness {
            require_basic_authenticode_validation: true,
            require_signing_certificate_check: true,
            require_not_before_check: true,
            require_not_after_check: true,
            require_chain_check: true,
            require_ca_verification_against_ctl: true,
            exclude_specific_cert_authorities_from_ctl_check: true,
        }
    }
}

#[derive(Clone, Debug)]
struct AuthenticodeValidatorInner<'a> {
    strictness: AuthenticodeStrictness,
    excluded_cert_authorities: Vec<DirectoryName>,
    now: Option<ValidityCheck<'a>>,
    expected_file_hash: Option<Vec<u8>>,
    #[cfg(feature = "ctl")]
    ctl: Option<&'a CertificateTrustList>,
}

pub struct AuthenticodeValidator<'a> {
    authenticode_signature: &'a AuthenticodeSignature,
    inner: RefCell<AuthenticodeValidatorInner<'a>>,
}

impl<'a> AuthenticodeValidator<'a> {
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
    pub fn require_not_before_check(&self) -> &Self {
        self.inner.borrow_mut().strictness.require_not_before_check = true;
        self
    }

    #[inline]
    pub fn ignore_not_before_check(&self) -> &Self {
        self.inner.borrow_mut().strictness.require_not_before_check = false;
        self
    }

    #[inline]
    pub fn require_not_after_check(&self) -> &Self {
        self.inner.borrow_mut().strictness.require_not_after_check = true;
        self
    }

    #[inline]
    pub fn ignore_not_after_check(&self) -> &Self {
        self.inner.borrow_mut().strictness.require_not_after_check = false;
        self
    }

    #[inline]
    pub fn require_signing_certificate_check(&self) -> &Self {
        self.inner.borrow_mut().strictness.require_signing_certificate_check = true;
        self
    }

    #[inline]
    pub fn ignore_signing_certificate_check(&self) -> &Self {
        self.inner.borrow_mut().strictness.require_signing_certificate_check = false;
        self
    }

    #[inline]
    pub fn require_basic_authenticode_validation(&self, expected_file_hash: Vec<u8>) -> &Self {
        self.inner.borrow_mut().strictness.require_basic_authenticode_validation = true;
        self.inner.borrow_mut().expected_file_hash = Some(expected_file_hash);
        self
    }

    #[inline]
    pub fn ignore_basic_authenticode_validation(&self) -> &Self {
        self.inner.borrow_mut().strictness.require_basic_authenticode_validation = false;
        self.inner.borrow_mut().expected_file_hash = None;
        self
    }

    #[cfg(feature = "ctl")]
    #[inline]
    pub fn require_ca_against_ctl_check(&self) -> &Self {
        self.inner.borrow_mut().strictness.require_ca_verification_against_ctl = true;
        self
    }

    #[cfg(feature = "ctl")]
    #[inline]
    pub fn ctl(&self, ctl: &'a CertificateTrustList) -> &Self {
        self.inner.borrow_mut().ctl = Some(ctl);
        self
    }

    #[cfg(feature = "ctl")]
    #[inline]
    pub fn ignore_ca_against_ctl_check(&self) -> &Self {
        self.inner.borrow_mut().strictness.require_ca_verification_against_ctl = false;
        self
    }

    pub fn require_chain_check(&self) -> &Self {
        self.inner.borrow_mut().strictness.require_chain_check = true;
        self
    }

    pub fn ignore_chain_check(&self) -> &Self {
        self.inner.borrow_mut().strictness.require_chain_check = false;
        self
    }

    #[inline]
    pub fn exclude_cert_authorities(&self, excluded_cert_authorities: &'a [DirectoryName]) -> &Self {
        self.inner
            .borrow_mut()
            .strictness
            .exclude_specific_cert_authorities_from_ctl_check = true;
        self.inner
            .borrow_mut()
            .excluded_cert_authorities
            .extend_from_slice(excluded_cert_authorities);

        self
    }

    #[cfg(feature = "ctl")]
    #[inline]
    pub fn ignore_excluded_cert_authorities(&self) -> &Self {
        self.inner
            .borrow_mut()
            .strictness
            .exclude_specific_cert_authorities_from_ctl_check = false;
        self.inner.borrow_mut().excluded_cert_authorities.clear();

        self
    }

    fn h_verify_authenticode_basic(&self, certificates: &[Cert]) -> AuthenticodeResult<()> {
        // 1. SignedData version field must be set to 1.
        let version = self.authenticode_signature.0 .0.signed_data.version;
        if version != CmsVersion::V1 {
            return Err(AuthenticodeError::IncorrectVersion {
                expected: 1,
                got: version as u32,
            });
        }

        // 2. It must contain only one singer info.
        let signer_infos = self.authenticode_signature.0.signer_infos();
        if signer_infos.len() != 1 {
            return Err(AuthenticodeError::MultipleSignerInfo {
                count: signer_infos.len(),
            });
        }

        // 3. Authenticode signatures support only one signer, digestAlgorithms must contain only one digestAlgorithmIdentifier.
        let digest_algorithms = self.authenticode_signature.0.digest_algorithms();
        if digest_algorithms.len() != 1 {
            return Err(AuthenticodeError::IncorrectDigestAlgorithmsCount {
                incorrect_count: digest_algorithms.len(),
            });
        }

        // 4. Signature::digest_algorithm must match ContentInfo::digest_algorithm and SignerInfo::digest_algorithm.
        let content_info = self.authenticode_signature.0.encapsulated_content_info();
        let spc_indirect_data_content = match &content_info.content {
            Some(content_value) => match &content_value.0 {
                ContentValue::SpcIndirectDataContent(spc_indirect_data_content) => spc_indirect_data_content,
                _ => return Err(AuthenticodeError::NoSpcIndirectDataContent),
            },
            None => return Err(AuthenticodeError::NoEncapsulatedContentInfo),
        };

        let digest_algorithm = digest_algorithms
            .first()
            .expect("One digest algorithm should exist at this point");

        let message_digest = &spc_indirect_data_content.message_digest;
        if digest_algorithm != &message_digest.oid {
            return Err(AuthenticodeError::DigestAlgorithmMismatch {
                description: "Signature digest algorithm does not match EncapsulatedContentInfo digest algorithm"
                    .to_string(),
            });
        }

        let signer_info = signer_infos
            .first()
            .expect("One SignerInfo should exists at this point");
        if digest_algorithm != &signer_info.digest_algorithm.0 {
            return Err(AuthenticodeError::DigestAlgorithmMismatch {
                description: "Signature digest algorithm does not match SignerInfo digest algorithm".to_string(),
            });
        }

        // 5. Check file hash
        let actual_file_hash = &message_digest.digest.0;
        let expected_file_hash = self
            .inner
            .borrow()
            .expected_file_hash
            .clone()
            .expect("Expected file hash to be present for Authenticode basic validation");
        if actual_file_hash != &expected_file_hash {
            return Err(AuthenticodeError::FileHashMismatch {
                actual: actual_file_hash.clone(),
                expected: expected_file_hash,
            });
        }

        // 6.The x509 certificate specified by SignerInfo::serial_number and SignerInfo::issuer must exist within Signature::certificates
        let signing_certificate = self.authenticode_signature.signing_certificate(certificates)?;

        // 7. Given the x509 certificate, compare SignerInfo::encrypted_digest against hash of authenticated attributes and hash of ContentInfo
        let public_key = signing_certificate.public_key();

        let hash_algo = ShaVariant::try_from(Into::<ObjectIdentifierAsn1>::into(digest_algorithm.oid().clone()))
            .map_err(AuthenticodeError::UnsupportedAlgorithmError)?;

        let authenticated_attributes = self.authenticode_signature.authenticated_attributes();
        let mut raw_attributes = picky_asn1_der::to_vec(authenticated_attributes)?;
        // According to the RFC:
        //
        // "[...] The Attributes value's tag is SET OF, and the DER encoding ofs
        // the SET OF tag, rather than of the IMPLICIT [0] tag [...]"
        raw_attributes[0] = Tag::SET.inner();

        let signature_algorithm_identifier = AlgorithmIdentifier::new_rsa_encryption_with_sha(hash_algo)
            .map_err(AuthenticodeError::UnsupportedAlgorithmError)?;
        let signature_algorithm = SignatureAlgorithm::from_algorithm_identifier(&signature_algorithm_identifier)?;
        signature_algorithm
            .verify(public_key, &raw_attributes, &signer_info.signature.0 .0)
            .map_err(AuthenticodeError::SignatureError)?;

        // 8. PKCS9_MESSAGE_DIGEST attribute exists and that its value matches hash of ContentInfo.
        let message_digest_attr = authenticated_attributes
            .iter()
            .find(|attr| matches!(attr.value, AttributeValues::MessageDigest(_)))
            .ok_or(AuthenticodeError::NoMessageDigest)?;

        let hash_algo = HashAlgorithm::try_from(hash_algo).map_err(AuthenticodeError::UnsupportedHashAlgorithmError)?;

        let mut raw_spc_indirect_data_content = picky_asn1_der::to_vec(&spc_indirect_data_content.data)?;
        let mut raw_message_digest = picky_asn1_der::to_vec(&spc_indirect_data_content.message_digest)?;
        raw_spc_indirect_data_content.append(&mut raw_message_digest);

        let content_info_hash = hash_algo.digest(&raw_spc_indirect_data_content);

        if let AttributeValues::MessageDigest(message_digest_attr_val) = &message_digest_attr.value {
            if message_digest_attr_val
                .0
                .first()
                .expect("At least one element is always present in Asn1SetOf AttributeValues")
                != &content_info_hash
            {
                return Err(AuthenticodeError::HashMismatch);
            }
        }

        // 9. The signing certificate must contain either the extended key usage (EKU) value for code signing,
        // or the entire certificate chain must contain no EKUs
        h_check_eku_code_signing(certificates, signing_certificate)?;

        Ok(())
    }

    fn h_verify_signing_certificate(&self, certificates: &[Cert]) -> AuthenticodeResult<()> {
        let signing_certificate = self.authenticode_signature.signing_certificate(certificates)?;
        let cert_validator = signing_certificate.verifier();

        let inner = self.inner.borrow_mut();

        let cert_validator = match inner.now {
            Some(ValidityCheck::Exact(exact)) => cert_validator.exact_date(exact),
            Some(ValidityCheck::Interval { lower, upper }) => cert_validator.interval_date(lower, upper),
            None => &cert_validator,
        };

        let cert_validator = if inner.strictness.require_not_after_check {
            cert_validator.require_not_after_check()
        } else {
            cert_validator.ignore_not_after_check()
        };

        let cert_validator = if inner.strictness.require_not_before_check {
            cert_validator.require_not_before_check()
        } else {
            cert_validator.ignore_not_before_check()
        };

        let cert_validator = if inner.strictness.require_chain_check {
            let certificate_iter = SignatureCertificatesIterator::new(signing_certificate, certificates.iter());
            cert_validator
                // Authenticode has the signer certificate and any intermediate certificates,
                // but typically does not contain the root
                .chain_should_contains_root_certificate(false)
                .chain(certificate_iter.filter(|cert| cert.subject_name() != signing_certificate.subject_name()))
        } else {
            cert_validator.ignore_chain_check()
        };

        match cert_validator.verify() {
            Ok(()) => Ok(()),
            Err(err) => {
                // By default, timestamping an Authenticode signature extends the lifetime of the signature
                // indefinitely, as long as that signature was timestamped, both:
                // •	During the validity period of the signing certificate.
                // •	Before the certificate revocation date, if applicable.

                // If the publisher’s signing certificate contains the lifetime signer OID in addition to the PKIX code signing OID,
                // the signature becomes invalid when the publisher’s signing certificate expires, even
                // if the signature is timestamped

                if let CertError::InvalidCertificate { source, id } = &err {
                    if id == &signing_certificate.subject_name().to_string()
                        && matches!(source.as_ref(), CertError::CertificateExpired { .. })
                    {
                        let kp_lifetime_signing_is_present =
                            signing_certificate
                                .extensions()
                                .iter()
                                .any(|extension| match extension.extn_value() {
                                    ExtensionView::ExtendedKeyUsage(eku) => eku.contains(oids::kp_lifetime_signing()),
                                    _ => false,
                                });

                        let check_if_kp_time_stamping_present = |certificates: &[Cert]| -> bool {
                            certificates
                                .iter()
                                .flat_map(|cert| cert.extensions().iter())
                                .any(|extension| match extension.extn_value() {
                                    ExtensionView::ExtendedKeyUsage(eku) => eku.contains(oids::kp_time_stamping()),
                                    _ => false,
                                })
                        };

                        if check_if_kp_time_stamping_present(certificates) && !kp_lifetime_signing_is_present {
                            return Ok(());
                        }

                        let unsigned_attributes = self.authenticode_signature.unauthenticated_attributes();

                        if let Some(unsigned_attribute) = unsigned_attributes.first() {
                            match &unsigned_attribute.value {
                                UnsignedAttributeValue::MsCounterSign(mc_counter_sign) => {
                                    let pkcs7_certificate = mc_counter_sign
                                        .0
                                        .first()
                                        .expect("MsCounterSign should contain exactly one Pkcs7Certificate");

                                    let certificates = pkcs7_certificate
                                        .signed_data
                                        .certificates
                                        .0
                                         .0
                                        .iter()
                                        .filter_map(|cert| match cert {
                                            CertificateChoices::Certificate(certificate) => {
                                                Cert::from_der(&certificate.0).ok()
                                            }
                                            CertificateChoices::Other(_) => None,
                                        })
                                        .collect::<Vec<Cert>>();

                                    if check_if_kp_time_stamping_present(&certificates)
                                        && !kp_lifetime_signing_is_present
                                    {
                                        // check if certificates in the timestamp have the right chain
                                        if let Some(leaf) = certificates.get(0) {
                                            let timestamp_chain_validator = leaf.verifier();
                                            let timestamp_chain = certificates
                                                .iter()
                                                .filter(|cert| cert.subject_name() != leaf.subject_name());

                                            let timestamp_chain_validator =
                                                timestamp_chain_validator.require_chain_check().chain(timestamp_chain);

                                            let timestamp_chain_validator = timestamp_chain_validator
                                                .ignore_not_after_check()
                                                .ignore_not_before_check()
                                                .chain_should_contains_root_certificate(false);

                                            timestamp_chain_validator
                                                .verify()
                                                .map_err(AuthenticodeError::InvalidTimestampCert)?;

                                            #[cfg(feature = "ctl")]
                                            {
                                                if let Some(ctl) = self.inner.borrow().ctl {
                                                    let ca_name = h_get_ca_name(certificates.iter()).unwrap();
                                                    self.h_verify_ca_certificate_against_ctl(ctl, &ca_name)?;
                                                }
                                            }
                                        }

                                        return Ok(());
                                    }
                                }
                                UnsignedAttributeValue::CounterSign(_) => {}
                            };
                        }
                    }
                }

                Err(AuthenticodeError::CertError(err))
            }
        }
    }

    // https://github.com/robstradling/authroot_parser was used as a reference while implementing this function
    #[cfg(feature = "ctl")]
    fn h_verify_ca_certificate_against_ctl(
        &self,
        ctl: &CertificateTrustList,
        ca_name: &DirectoryName,
    ) -> AuthenticodeResult<()> {
        use chrono::{DateTime, Duration, NaiveDate, Utc};
        use picky_asn1::wrapper::OctetStringAsn1;
        use std::ops::Add;

        // In CTL time in a OctetString encoded as 64 bits Windows FILETIME LE
        let time_octet_string_to_utc_time = |time: &OctetStringAsn1| -> DateTime<Utc> {
            let since: DateTime<Utc> = DateTime::from_utc(NaiveDate::from_ymd(1601, 1, 1).and_hms(0, 0, 0), Utc);
            since.add(Duration::seconds(
                i64::from_le_bytes([
                    time.0[0], time.0[1], time.0[2], time.0[3], time.0[4], time.0[5], time.0[6], time.0[7],
                ]) / 10_000_000,
            ))
        };

        let raw_ca_name = picky_asn1_der::to_vec(&Name::from(ca_name.clone()))?;
        let ca_name_md5_digest = HashAlgorithm::MD5.digest(&raw_ca_name);

        let ctl_entries = ctl.ctl_entries()?;

        // find the CA certificate info by its md5 name digest
        let ca_ctl_entry_attributes = ctl_entries
            .iter()
            .find(|&ctl_entry| {
                ctl_entry.attributes.0.iter().any(|attr| match &attr.value {
                    CTLEntryAttributeValues::CertSubjectNameMd5HashPropId(ca_cert_md5_hash) => {
                        match &ca_cert_md5_hash.0.first() {
                            Some(ca_cert_md5_hash) => ca_cert_md5_hash.0 == ca_name_md5_digest,
                            None => false,
                        }
                    }
                    _ => false,
                })
            })
            .ok_or(AuthenticodeError::CAIsNotTrusted)?;

        // check if the CA certificate was revoked
        if let Some(CTLEntryAttributeValues::CertDisallowedFileTimePropId(when_ca_cert_was_revoked)) =
            ca_ctl_entry_attributes
                .attributes
                .0
                .iter()
                .find(|attr| matches!(attr.value, CTLEntryAttributeValues::CertDisallowedFileTimePropId(_)))
                .map(|attr| &attr.value)
        {
            let when_ca_cert_was_revoked = when_ca_cert_was_revoked
                .0
                .first()
                .expect("Asn1SetOf CertDisallowedFiletimePropId should contain exactly one value");

            if when_ca_cert_was_revoked.0.is_empty() || when_ca_cert_was_revoked.0.len() < 8 {
                return Err(AuthenticodeError::CaCertificateRevoked);
            }

            let not_after = time_octet_string_to_utc_time(when_ca_cert_was_revoked);
            let now = Utc::now();
            if not_after < now {
                return Err(AuthenticodeError::CaCertificateExpired {
                    not_after: not_after.into(),
                    now: now.into(),
                });
            }
        }

        // check if the CA certificate is not yet valid
        if let Some(CTLEntryAttributeValues::UnknownReservedPropId126(not_before)) = ca_ctl_entry_attributes
            .attributes
            .0
            .iter()
            .find(|attr| matches!(attr.value, CTLEntryAttributeValues::UnknownReservedPropId126(_)))
            .map(|attr| &attr.value)
        {
            let not_before = not_before
                .0
                .first()
                .expect("Asn1SetOf UnknownReservedPropId126 should contain exactly one value");

            let not_before = time_octet_string_to_utc_time(not_before);
            let now = Utc::now();

            if not_before > now {
                // UnknownReservedPropId127 appears to be set of EKUs for which the NotBefore-ing applies.
                // check if it contains code signing oid
                if let Some(CTLEntryAttributeValues::UnknownReservedPropId127(set_of_eku_oids)) =
                    ca_ctl_entry_attributes
                        .attributes
                        .0
                        .iter()
                        .find(|attr| matches!(attr.value, CTLEntryAttributeValues::UnknownReservedPropId126(_)))
                        .map(|attr| &attr.value)
                {
                    let set_of_eku_oids = set_of_eku_oids.0.first().expect(
                        "
                    Asn1SetOf UnknownReservedPropId127 should contain exactly one value",
                    );
                    let eku_code_signing_oid = oids::kp_code_signing();
                    if set_of_eku_oids
                        .0
                         .0
                        .iter()
                        .any(|kp_oid| kp_oid == &eku_code_signing_oid)
                    {
                        return Err(AuthenticodeError::CaCertificateNotYetValid {
                            not_before: not_before.into(),
                            now: now.into(),
                        });
                    }
                } else {
                    return Err(AuthenticodeError::CaCertificateNotYetValid {
                        not_before: not_before.into(),
                        now: now.into(),
                    });
                }
            }
        }

        Ok(())
    }

    pub fn verify(&self) -> AuthenticodeResult<()> {
        let certificates = self.authenticode_signature.0.decode_certificates();

        if self.inner.borrow().strictness.require_basic_authenticode_validation {
            self.h_verify_authenticode_basic(&certificates)?;
        }

        if self.inner.borrow().strictness.require_signing_certificate_check {
            self.h_verify_signing_certificate(&certificates)?;
        }

        #[cfg(feature = "ctl")]
        if self.inner.borrow().strictness.require_ca_verification_against_ctl {
            let signing_certificate = self.authenticode_signature.signing_certificate(&certificates)?;
            let certificates_iter = SignatureCertificatesIterator::new(signing_certificate, certificates.iter());

            let ca_name = h_get_ca_name(certificates_iter).unwrap();

            if let Some(ctl) = self.inner.borrow().ctl {
                match self.h_verify_ca_certificate_against_ctl(ctl, &ca_name) {
                    Ok(()) => {}
                    Err(err) => {
                        if !self
                            .inner
                            .borrow()
                            .strictness
                            .exclude_specific_cert_authorities_from_ctl_check
                            || !self.inner.borrow().excluded_cert_authorities.contains(&ca_name)
                        {
                            return Err(err);
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

fn h_check_eku_code_signing(certificates: &[Cert], signing_certificate: &Cert) -> AuthenticodeResult<()> {
    let certificates_iter = SignatureCertificatesIterator::new(signing_certificate, certificates.iter());

    if certificates_iter
        .flat_map(|cert| cert.extensions().iter())
        .any(|extension| matches!(extension.extn_value(), ExtensionView::ExtendedKeyUsage(_)))
        && !signing_certificate
            .extensions()
            .iter()
            .any(|extension| match extension.extn_value() {
                ExtensionView::ExtendedKeyUsage(eku) => eku.contains(oids::kp_code_signing()),
                _ => false,
            })
    {
        return Err(AuthenticodeError::NoEKUCodeSigning);
    }

    Ok(())
}

#[cfg(feature = "ctl")]
fn h_get_ca_name<'i, I: Iterator<Item = &'i Cert>>(mut certificates: I) -> Option<DirectoryName> {
    let first_certificate = certificates.next().map(|cert| cert.issuer_name());

    let certificates_iter =
        certificates.filter(|cert| cert.ty() == CertType::Intermediate || cert.ty() == CertType::Root);
    if let Some(certificate) = certificates_iter.last() {
        if certificate.ty() == CertType::Root {
            Some(certificate.subject_name())
        } else {
            Some(certificate.issuer_name())
        }
    } else {
        first_certificate
    }
}

struct SignatureCertificatesIterator<'i> {
    iter: Box<dyn Iterator<Item = &'i Cert> + 'i>,
}

impl<'i> SignatureCertificatesIterator<'i> {
    fn new<I: Iterator<Item = &'i Cert> + 'i + Clone>(singing_certificate: &'i Cert, certificates: I) -> Self {
        let mut prev_issuer_name = singing_certificate.subject_name();

        // Authenticode signature contains timestamp certificate beside the signature certificates.
        // That makes a mess if we try to validate the signature certificates, so let's filter out certificates
        // to not include timestamp related certificates :)
        let iter = Box::new(certificates.filter(move |cert| {
            let should_be_validated = cert.subject_name() == prev_issuer_name;
            if should_be_validated {
                prev_issuer_name = cert.issuer_name();
            }
            should_be_validated
        }));

        Self { iter }
    }
}

impl<'i> Iterator for SignatureCertificatesIterator<'i> {
    type Item = &'i Cert;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

#[derive(Error, Debug)]
pub enum AuthenticodeSignatureBuilderError {
    #[error("Digest algorithm is required, but missing")]
    MissingDigestAlgorithm,
    #[error("Signing key is required, but missing")]
    MissingSigningKey,
    #[error("Issuer and serial number are required, but missing")]
    MissingIssuerAndSerialNumber,
    #[error("Certificates are required, but missing")]
    MissingCertificatesRequired,
    #[error("Content info is required, but missing")]
    MissingContentInfo,
    #[error(transparent)]
    Asn1DerError(#[from] Asn1DerError),
    #[error(transparent)]
    SignatureError(#[from] SignatureError),
    #[error(transparent)]
    AuthenticodeError(#[from] AuthenticodeError),
}

#[derive(Default, Clone, Debug)]
struct AuthenticodeSignatureBuilderInner<'a> {
    certs: Option<Vec<Cert>>,
    digest_algorithm: Option<HashAlgorithm>,
    content_info: Option<EncapsulatedContentInfo>,
    signing_key: Option<&'a PrivateKey>,
    issuer_and_serial_number: Option<IssuerAndSerialNumber>,
    authenticated_attributes: Option<Vec<Attribute>>,
    unsigned_attributes: Option<Vec<UnsignedAttribute>>,
}

#[derive(Default, Clone, Debug)]
pub struct AuthenticodeSignatureBuilder<'a> {
    inner: RefCell<AuthenticodeSignatureBuilderInner<'a>>,
}

impl<'a> AuthenticodeSignatureBuilder<'a> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Required
    #[inline]
    pub fn certs(&self, certs: Vec<Cert>) -> &Self {
        self.inner.borrow_mut().certs = Some(certs);
        self
    }

    /// Required
    #[inline]
    pub fn digest_algorithm(&self, digest_algorithm: HashAlgorithm) -> &Self {
        self.inner.borrow_mut().digest_algorithm = Some(digest_algorithm);
        self
    }

    /// Required
    #[inline]
    pub fn content_info(&self, content_info: EncapsulatedContentInfo) -> &Self {
        self.inner.borrow_mut().content_info = Some(content_info);
        self
    }

    /// Required
    #[inline]
    pub fn signing_key(&self, signing_key: &'a PrivateKey) -> &Self {
        self.inner.borrow_mut().signing_key = Some(signing_key);
        self
    }

    /// Optional
    #[inline]
    pub fn authenticated_attributes(&self, authenticate_attributes: Vec<Attribute>) -> &Self {
        self.inner.borrow_mut().authenticated_attributes = Some(authenticate_attributes);
        self
    }

    /// Optional
    #[inline]
    pub fn unsigned_attributes(&self, unsigned_attributes: Vec<UnsignedAttribute>) -> &Self {
        self.inner.borrow_mut().unsigned_attributes = Some(unsigned_attributes);
        self
    }

    /// Required
    #[inline]
    pub fn issuer_and_serial_number(&self, issuer: DirectoryName, serial_number: Vec<u8>) -> &Self {
        self.inner.borrow_mut().issuer_and_serial_number = Some(IssuerAndSerialNumber {
            issuer: issuer.into(),
            serial_number: CertificateSerialNumber(serial_number.into()),
        });
        self
    }

    pub fn build(&self) -> Result<AuthenticodeSignature, AuthenticodeSignatureBuilderError> {
        let mut inner = self.inner.borrow_mut();
        let AuthenticodeSignatureBuilderInner {
            certs,
            digest_algorithm,
            content_info,
            signing_key,
            issuer_and_serial_number,
            authenticated_attributes,
            unsigned_attributes,
            ..
        } = inner.deref_mut();

        let digest_algorithm = ShaVariant::try_from(
            digest_algorithm
                .take()
                .ok_or(AuthenticodeSignatureBuilderError::MissingDigestAlgorithm)?,
        )
        .map_err(|err| {
            AuthenticodeSignatureBuilderError::AuthenticodeError(AuthenticodeError::UnsupportedHashAlgorithmError(err))
        })?;

        let content_info = content_info
            .take()
            .ok_or(AuthenticodeSignatureBuilderError::MissingContentInfo)?;

        let signing_key = signing_key
            .take()
            .ok_or(AuthenticodeSignatureBuilderError::MissingSigningKey)?;

        let issuer_and_serial_number = issuer_and_serial_number
            .take()
            .ok_or(AuthenticodeSignatureBuilderError::MissingIssuerAndSerialNumber)?;

        let certificates = certs
            .take()
            .ok_or(AuthenticodeSignatureBuilderError::MissingCertificatesRequired)?;

        let signing_certificate = certificates
            .iter()
            .find(|cert| {
                Name::from(cert.issuer_name()) == issuer_and_serial_number.issuer
                    && cert.serial_number() == &issuer_and_serial_number.serial_number.0
            })
            .ok_or_else(
                || AuthenticodeError::NoCertificatesAssociatedWithIssuerAndSerialNumber {
                    issuer: issuer_and_serial_number.issuer.clone(),
                    serial_number: issuer_and_serial_number.serial_number.0 .0.clone(),
                },
            )
            .map_err(AuthenticodeSignatureBuilderError::AuthenticodeError)?;

        // The signing certificate must contain either the extended key usage (EKU) value for code signing,
        // or the entire certificate chain must contain no EKUs
        h_check_eku_code_signing(&certificates, signing_certificate)
            .map_err(AuthenticodeSignatureBuilderError::AuthenticodeError)?;

        // certificates contains the signer certificate and any intermediate certificates,
        // but typically does not contain the root certificate
        let certificates = certificates.into_iter().filter_map(|cert| {
            if cert.ty() != CertType::Root {
                Some(Certificate::from(cert))
            } else {
                None
            }
        });

        let digest_encryption_algorithm = AlgorithmIdentifier::new_rsa_encryption_with_sha(digest_algorithm)
            .map_err(AuthenticodeError::UnsupportedAlgorithmError)?;

        let signature_algo = SignatureAlgorithm::from_algorithm_identifier(&digest_encryption_algorithm)?;

        let to_sign_data = if let Some(ref authenticated_attributes) = authenticated_attributes {
            let mut auth_raw_data = picky_asn1_der::to_vec(&authenticated_attributes)?;
            // According to the RFC:
            //
            // "[...] The Attributes value's tag is SET OF, and the DER encoding ofs
            // the SET OF tag, rather than of the IMPLICIT [0] tag [...]"
            auth_raw_data[0] = Tag::SET.inner();
            auth_raw_data
        } else {
            // If there is no authenticated attributes, then we should sign content_info
            picky_asn1_der::to_vec(&content_info)?
        };

        let encrypted_digest = SignatureValue(
            signature_algo
                .sign(&to_sign_data, signing_key)
                .map_err(|err| {
                    AuthenticodeSignatureBuilderError::AuthenticodeError(AuthenticodeError::SignatureError(err))
                })?
                .into(),
        );

        let digest_algorithm = AlgorithmIdentifier::new_sha(digest_algorithm);

        let signer_info = SignerInfo {
            version: CmsVersion::V1,
            sid: SignerIdentifier::IssuerAndSerialNumber(issuer_and_serial_number),
            digest_algorithm: DigestAlgorithmIdentifier(digest_algorithm.clone()),
            signed_attrs: Attributes(authenticated_attributes.take().unwrap_or_default().into()).into(),
            signature_algorithm: SignatureAlgorithmIdentifier(AlgorithmIdentifier::new_rsa_encryption()),
            signature: encrypted_digest,
            unsigned_attrs: UnsignedAttributes(unsigned_attributes.take().unwrap_or_default()).into(),
        };

        let mut certs = Vec::new();
        for cert in certificates {
            let raw_certificates = picky_asn1_der::to_vec(&cert)?;
            certs.push(CertificateChoices::Certificate(picky_asn1_der::from_bytes(
                &raw_certificates,
            )?));
        }

        let signed_data = SignedData {
            version: CmsVersion::V1,
            digest_algorithms: DigestAlgorithmIdentifiers(vec![digest_algorithm].into()),
            content_info,
            certificates: CertificateSet(certs).into(),
            crls: Some(RevocationInfoChoices::default()),
            signers_infos: SignersInfos(vec![signer_info].into()),
        };

        Ok(AuthenticodeSignature(Pkcs7::from(Pkcs7Certificate {
            oid: oids::signed_data().into(),
            signed_data: signed_data.into(),
        })))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::pem::parse_pem;
    use crate::x509::certificate::{CertType, CertificateBuilder};
    use crate::x509::{Csr, KeyIdGenMethod};
    use picky_asn1_x509::Extension;

    const RSA_PRIVATE_KEY: &str = "-----BEGIN RSA PRIVATE KEY-----\n\
                                   MIIEpAIBAAKCAQEA0vg4PmmJdy1W/ayyuP3ovRBbggAZ98dEY5uzEU23ENaN3jsx\n\
                                   R9zEAAmQ9OZbbJXN33l+PMKY7+5izgI/RlGSNF2s0mdyWEhoRMTxuwpJoFgBkYEE\n\
                                   Jwr40xoLCbw9TpBooJgdYg/n/Fu4NGM7YJdcfKjf3/le7kNTZYPBx09wBkHvkuTD\n\
                                   gpdDDnb5R6sTouD0bCPjC/gZCoRAAlzfuAmAAHVb+i8fkTV32OzokLYcneuLZU/+\n\
                                   FBpR2w9UprfDLWFbPxuOBf+mIGp4WWVN82+3SdEkp/5BRQ//MhGhj7NhEYp+KyWJ\n\
                                   Hm+1iGvrxsgQH+4MQTJGdp838sl+w77QGFZU9QIDAQABAoIBAEBWNkDCSouvpgHC\n\
                                   ctZ7iEhv/pgMk96+RBrkVp2GR7e41pbZElRJ/PPN9wjYXzUkEh5+nILHDYDOAA+3\n\
                                   G7jEE4QotRWNOo+1tSaTsOxLXNyrOf83ix6k9/DY1ljnsQKOg3nGKd/H3gVVqz0+\n\
                                   rdLtFeVmUq+pCsw6d+pTXfr8PLuLPfe8r9fu/BGU2wtINAEuQ4x3/S/JPTm6XnsV\n\
                                   NUW62K/lB7RjXlEqnKMwxcVCu/m0C1HdlwTlHyzktIydjL9Bk1GjGQVt0zC/rfvA\n\
                                   zrlsTPg4UTL6zs4D9B5PPaZMJeBieXaQ0JdqKdJkRm+mPCOEGf+BLz1zHVAVZaSZ\n\
                                   PK8E7NkCgYEA+e7WUOlr4nR6fqvte0ZTewIeG/j2m5gSVjjy8Olh2D3v8q4hZj5s\n\
                                   2jaFJJ7RUGXZdiySodlEpLR2nrrUURC6fGukvbFCW2j/0SotBl53Wa2zJdrU3AZc\n\
                                   b9j7MOyJbJDKJYqdivYJXp7ra4vCs0xAMXfuQD1AWaKlCQxbeyrKWxcCgYEA2BdA\n\
                                   fB7IL0ec3WOsLyGGhcCrGDWIiOlXzk5Fuus+NOEp70/bYCqGpu+tNAxtbY4e+zHp\n\
                                   5gXApKU6PSQ/I/eG/o0WGQCZazfhIGpwORrWHAVxDlxJ+/hlZd6DmTjaIJw1k2gr\n\
                                   D849l1WIEr2Ps8Bv3Y7XeLpnUAQFv1ekfMKxZ9MCgYEA2vtNYfUypmZh0Uy4NZNn\n\
                                   n1Y6pU2cXLWAE3WwPi5toTabXuj8sIWvf/3W6EASqzuhri3dh9tCjoDjka2mSyS6\n\
                                   EDuMSvvdZRP5V/15F6R7M+LCHT+/0svr/7+ATtxgh/PQedYatN9fVD0vjboVrFz5\n\
                                   vZ4T7Mr978tWiDgAi0jxpZ8CgYA/AOiIR+FOB68wzXLSew/hx38bG+CnKoGzYRbr\n\
                                   nNMST+QOJlZr/3orCg6R8l2lZ56Y1sC/lEXKu3HzibHvJqhxZ2ld+NLCdBRrgx0d\n\
                                   STnMCbog2b+oe4/015+++NiAUYs9Y03K2fMTQJjf/ez8F8uF6bPhO1gL+GBEnaUT\n\
                                   yyA2iQKBgQD1KfqZeJtPCwmfdPokblKgorstuMKjMegD/6ztjIFw4c9XkvUAvlD5\n\
                                   MvS4rPuhVYrvouZHJ50bcwccByJ8aCOJxLdH7+bjojMSAgV2kGq+FNh7F1wRcwx8\n\
                                   8Z+DBbeVaCpYSQa5bCr5jG6nIX5v/KbS3HCmAkUzwqGoEsk53yFmKw==\n\
                                   -----END RSA PRIVATE KEY-----";

    const SELF_SIGNED_PKCS7: &str = "-----BEGIN PKCS7-----\
                                                                MIIKvwYJKoZIhvcNAQcCoIIKsDCCCqwCAQExADALBgkqhkiG9w0BBwGgggqSMIID\n\
                                                                QjCCAioCAQMwDQYJKoZIhvcNAQELBQAwYTELMAkGA1UEBhMCSW4xCzAJBgNVBAgM\n\
                                                                AkluMQswCQYDVQQHDAJJbjELMAkGA1UECgwCSW4xCzAJBgNVBAsMAkluMQswCQYD\n\
                                                                VQQDDAJJbjERMA8GCSqGSIb3DQEJARYCSW4wHhcNMjEwNzA4MTE1NjQ5WhcNMjIw\n\
                                                                NzA4MTE1NjQ5WjBtMQswCQYDVQQGEwJMZjENMAsGA1UECAwETGVhZjENMAsGA1UE\n\
                                                                BwwETGVhZjENMAsGA1UECgwETGVhZjENMAsGA1UECwwETGVhZjENMAsGA1UEAwwE\n\
                                                                TGVhZjETMBEGCSqGSIb3DQEJARYETGVhZjCCASIwDQYJKoZIhvcNAQEBBQADggEP\n\
                                                                ADCCAQoCggEBANdMZ/MHsDoW4K1G3TB8z0TDbKQeBTYv7//rlPv81OMLhkMJgcQb\n\
                                                                XkhHkSoyynw/wUAvWH3U8ZN4vYc4jWuw/j4pTjsf2lf1MQFoVZJehaqYoTfsYaIi\n\
                                                                89AwHxDlkQitumuWs24VrPCe2PO+fNM04V/4FgI1RniOsAFTlSfNyG2cIZCYbAVk\n\
                                                                pBCmci5LbAlm2zC6zZBoGpp1GqjFnwR7JJQcgOKHr9inJmjFM0D0ZIiadDKHSuKm\n\
                                                                U9c6vdAQHWrIXHaxvZspLg2YUWH9dDZVe+ddSuGEM7772N9/FdaMe+7r/r5uim4E\n\
                                                                imhdjn+PPa3Qhr6eD3nkNd+s3wtVekyYQIUCAwEAATANBgkqhkiG9w0BAQsFAAOC\n\
                                                                AQEAT0Bzl3U+fxhAEAGgU1gp0og7J9VM1diprUOl3C1RHUZtovlTBctltqDdSc7o\n\
                                                                YY4r3ubo25mkvJ0PH8d3pGJDOvE9SnmgP4BRschCu2LOjXgbV3pBk6ejgvPPTcMo\n\
                                                                rwiNJxf5exX35Ju1AzcpI71twP9Ty8YBOg3aAhqwu8MdcXbXbPESg5X8wpb30qVi\n\
                                                                RH7PzyAJlQynqCWMxTECXgtwLISHp/Ae2x3MUT2CKBZC65Z17UdYHN7uR0zavKwb\n\
                                                                3A2jzIPySFJL/KSy9WZLwmQdMUU3tcFRHDpaoMmJpPNBBbcXuhFoP9MLWTmm9+ma\n\
                                                                yaK7vOyltAK3MVuCpmccl7SNjDCCA5wwggKEoAMCAQICAQIwDQYJKoZIhvcNAQEL\n\
                                                                BQAwbTELMAkGA1UEBhMCUnQxDTALBgNVBAgMBFJvb3QxDTALBgNVBAcMBFJvb3Qx\n\
                                                                DTALBgNVBAoMBFJvb3QxDTALBgNVBAsMBFJvb3QxDTALBgNVBAMMBFJvb3QxEzAR\n\
                                                                BgkqhkiG9w0BCQEWBFJvb3QwHhcNMjEwNzA4MDkwMzMxWhcNMjIwNzA4MDkwMzMx\n\
                                                                WjBhMQswCQYDVQQGEwJJbjELMAkGA1UECAwCSW4xCzAJBgNVBAcMAkluMQswCQYD\n\
                                                                VQQKDAJJbjELMAkGA1UECwwCSW4xCzAJBgNVBAMMAkluMREwDwYJKoZIhvcNAQkB\n\
                                                                FgJJbjCCASIwDQYJKoZIhvcNAQEBBQADggEPADCCAQoCggEBAN4llgeaEesdTv+L\n\
                                                                mWhrMmluN1LnveWuQsRBV7dySd1d/3gRWwFMxXfaBjh1y/mDhGe1Kb8zz0buSOyn\n\
                                                                WMhT3xppsumF9y6aOGupSUij+nC+VFkcbZzWxJKBRJGJWcmPMNm+eEumY0ZrS21e\n\
                                                                EvVmKPlZSCZUkJgx3ogEsKaUQrHymx9+AUvjGGsIbmOB07cEcVjxz3eexOr7cMVw\n\
                                                                XXdnujdsgLYiR5rkxTP4pKkB4CdPEfy+q6cwO5KtO5pkgMcIhHCC/P+9pfwS5CVF\n\
                                                                5mUb0xj+yZGQ85fezRKy7mGSMhRvNvIhmnoVWyuvkoYdFUWzEDZqj4YJpJMHT2RN\n\
                                                                hTIdx0cCAwEAAaNTMFEwHQYDVR0OBBYEFHmBQGjcx/fnNFj4UxGyeCHFb/aaMB8G\n\
                                                                A1UdIwQYMBaAFEcLWYRko86McWZbVwnLHJXW/B1SMA8GA1UdEwEB/wQFMAMBAf8w\n\
                                                                DQYJKoZIhvcNAQELBQADggEBAI3oKESkfFQ/0B3xLFYvXMCuWv224wxGdw0TWi64\n\
                                                                8OwmCrNgYEXmkQPz4SZ0oQJjazllAflF+5Kc49zSdrCOPPz6bhw9O4Pcn875jCYl\n\
                                                                CD23+OexKGyfXFgc7/bzKTjN2tXA/Slo9ol1xvvY9HnhpL2UFf0jkecz41rP+TRl\n\
                                                                sxG7LwEF24P3xgZLlaySCp2S9WcBtIf7p1Z+6ekLl4KwihD/Q4uhibhFQqqOPuj8\n\
                                                                Fc4Jy8eyZ+0vEoVTQMrFahUrKjbfuxtYZ+8y5S4QbL6O5Ox7mYmTrloDUd0UIzMF\n\
                                                                iprHKnwHFjVAYDJkq6t5xt1o2kJ0EVSZkyUaD4U/zoSNHBIwggOoMIICkKADAgEC\n\
                                                                AgEBMA0GCSqGSIb3DQEBCwUAMG0xCzAJBgNVBAYTAlJ0MQ0wCwYDVQQIDARSb290\n\
                                                                MQ0wCwYDVQQHDARSb290MQ0wCwYDVQQKDARSb290MQ0wCwYDVQQLDARSb290MQ0w\n\
                                                                CwYDVQQDDARSb290MRMwEQYJKoZIhvcNAQkBFgRSb290MB4XDTIxMDcwODA5MDI1\n\
                                                                M1oXDTIyMDcwODA5MDI1M1owbTELMAkGA1UEBhMCUnQxDTALBgNVBAgMBFJvb3Qx\n\
                                                                DTALBgNVBAcMBFJvb3QxDTALBgNVBAoMBFJvb3QxDTALBgNVBAsMBFJvb3QxDTAL\n\
                                                                BgNVBAMMBFJvb3QxEzARBgkqhkiG9w0BCQEWBFJvb3QwggEiMA0GCSqGSIb3DQEB\n\
                                                                AQUAA4IBDwAwggEKAoIBAQC8dV0AD30BbZdBr9laj5sKb+PIzW2P/gir7VXXCz+q\n\
                                                                UHoZvK6ZqDW1K+jn4iTEx+HUGH9JYhB3syYOrMpi7CjXjz2x0lVJKvx5qSieGrQr\n\
                                                                yJaWePwhDWfVUjHVfbcFfdJLAH3pZjfNKHmm68n37Acc/mFZXTG3xN0yfQgbPwbO\n\
                                                                NGcfUze1u2kcpVjHJ1yOk9wwdO252HhJJx1Hd5wKWgeTkBQ73/vtZCQuLN3MZ+d4\n\
                                                                ModaTtCj/dA88p4PMyw2POiCpFrgxPxVrjfjPb6V7HmNP/1xzFEFkJvTfWlTmlCX\n\
                                                                rYG8BL3jHqfVw5gM1o1f1nClOpX/fmjqHvzT1AZ17IGVAgMBAAGjUzBRMB0GA1Ud\n\
                                                                DgQWBBRHC1mEZKPOjHFmW1cJyxyV1vwdUjAfBgNVHSMEGDAWgBRHC1mEZKPOjHFm\n\
                                                                W1cJyxyV1vwdUjAPBgNVHRMBAf8EBTADAQH/MA0GCSqGSIb3DQEBCwUAA4IBAQCL\n\
                                                                KgLbBc2R5oB0al9ce68zIRpdyoNVR2TTTqwzD5K0m4HhI93dG+1e4Mbtr970Q8DM\n\
                                                                KQvjvT0nf/jJjjoUKVG0dszTNPg5qlHPL+OgAj44dHzf7jBgEGJAez3Pk4zC4zvi\n\
                                                                0BusfObVryc0j3oZ2JFIRaBdon4MPI2HcTMLzPFMcprzMnDx7aQbDlkQLksL1Z2E\n\
                                                                5VvopUG5rTMMWItwWAVHwT/J9x0MPYs+LFc3Yeg7l3hsV03gC1jsh6pd0MR3p5vr\n\
                                                                WrOnUvpo7YFFGlKamwRpxIlYAgSEQFnD3LOjx+NGdGP1H0PQd9DA4xCwtPKkoCSw\n\
                                                                bOrBNDoLzSPaN6jy3JNeoQAxAA==\n\
                                                                -----END PKCS7-----";

    const SELF_SIGNED_PKCS7_RSA_PRIVATE_KEY: &str = "-----BEGIN PRIVATE KEY-----\n\
                                                                                MIIEvQIBADANBgkqhkiG9w0BAQEFAASCBKcwggSjAgEAAoIBAQDXTGfzB7A6FuCt\n\
                                                                                Rt0wfM9Ew2ykHgU2L+//65T7/NTjC4ZDCYHEG15IR5EqMsp8P8FAL1h91PGTeL2H\n\
                                                                                OI1rsP4+KU47H9pX9TEBaFWSXoWqmKE37GGiIvPQMB8Q5ZEIrbprlrNuFazwntjz\n\
                                                                                vnzTNOFf+BYCNUZ4jrABU5UnzchtnCGQmGwFZKQQpnIuS2wJZtswus2QaBqadRqo\n\
                                                                                xZ8EeySUHIDih6/YpyZoxTNA9GSImnQyh0riplPXOr3QEB1qyFx2sb2bKS4NmFFh\n\
                                                                                /XQ2VXvnXUrhhDO++9jffxXWjHvu6/6+bopuBIpoXY5/jz2t0Ia+ng955DXfrN8L\n\
                                                                                VXpMmECFAgMBAAECggEAWa5fAnHqa1gKQMNq8X6by9XnlDlZDGhNfXoBNjHr76Nm\n\
                                                                                SthT8H9B97Ov+Tbs93KLKhROtSOVeUtrDz90US6JyRTlnGU5Szg8MIzoUC8FWLl5\n\
                                                                                NlVFmgcbLlZNKnmlv0q2g4hjt3BZ+GUClA1963B0jMhHSqYsc51kHTlWwRzL5zPF\n\
                                                                                ekddpixpkE8fogE5+OTRtTlU6grWM0cMN/pNZK03A9H7APzUDZMxr51ues+cC2MH\n\
                                                                                OLiw2mjRgXCjK+IXutcCJToicp1JRWAuA0WjMTzjfMXUfm25bwcCMmyznMJiDTU2\n\
                                                                                JJ1OVYzTgQbsyftFs8E+05L5n2E/bAAH3/9OOFMxoQKBgQDvy6NLjoCk9OR1EQf5\n\
                                                                                0aCvUH3ZtGk/JBtM61F81Z6he0bSOFwKKGH8cv9f4TLqpIZWRoaD+SCWBTs+9iCp\n\
                                                                                kQHdfTe7/ElmUloxQRQVA7UXeoPA/JgX021YQFjLASNnBHSP5hMDnMGbDox9uuSn\n\
                                                                                1MEPzP5gljnECHko/rDh+Ab2SQKBgQDl2PrxDjBS0z5TymKgg5zsRFzanuJDOYjW\n\
                                                                                VGju4zmduqof9Mo2loRR33/xUu0jxrKhdHUx5pJRU/rlDaG7/lKNE4ZoF6RMh//C\n\
                                                                                traUCFLB55TMUEaAWPiBTVvbZRg4W5iIYc+HRz1uBnk+5c5rlShPVZwlXO3fKaom\n\
                                                                                3K8dXWqIXQKBgQDDpZNrDy6Y6BIKDcZDJq0CvRqhaJhCYxQ/MvP+dVCDElDbLg6y\n\
                                                                                XvZrgewob1YaqffNJqeTv8y9ejE3kptdnik2bHbv0syURna+Hwnih27WZChhafYx\n\
                                                                                4lghnAaWQyx+Xd04lxBGbzxrZXhtEPKEmIqYeLnHVmp1LjCkqQDqrXIIuQKBgHVe\n\
                                                                                ubYCotaInJk5DegdjTJxLmFNJQljBec8r2Ddk3xh56Ht5Jy/e847LSBUUlgkjO85\n\
                                                                                gub6cNkq40G4FlDja9Aymj3pZLLX99i8aLtrDKeL1EYI8Bd2V1/f2vpLw3R0AY4T\n\
                                                                                NGBGFq5qi9t8ik4RmsX4V4YU0DtXEVZK9vktzMrZAoGAHy698y0i0L4AIGlpIuW7\n\
                                                                                YZcLE0RSUfCdPA8HQad98GSKP8aipONwLbVT9KmTEmUA02g0Nb2caFirW3OYKZ8l\n\
                                                                                qOuqrRK+/evcuQixBSTPbAdNWyhbwYgSLUtR6q8erOmfdjjt5MD9SoS/luDV89NF\n\
                                                                                ocqmQTrEqWzH7mmVUFXY5GA=\n\
                                                                                -----END PRIVATE KEY-----";

    const FILE_HASH: [u8; 32] = [
        0xa7, 0x38, 0xda, 0x44, 0x46, 0xa4, 0xe7, 0x8a, 0xb6, 0x47, 0xdb, 0x7e, 0x53, 0x42, 0x7e, 0xb0, 0x79, 0x61,
        0xc9, 0x94, 0x31, 0x7f, 0x4c, 0x59, 0xd7, 0xed, 0xbe, 0xa5, 0xcc, 0x78, 0x6d, 0x80,
    ];

    #[test]
    fn decoding_into_authenticode_signature() {
        let pem = parse_pem(crate::test_files::PKCS7.as_bytes()).unwrap();
        let pkcs7 = Pkcs7::from_pem(&pem).unwrap();
        let hash_type = ShaVariant::SHA2_256;
        let private_key = PrivateKey::from_pem_str(RSA_PRIVATE_KEY).unwrap();
        let program_name = "decoding_into_authenticode_signature".to_string();

        let authenticode_signature =
            AuthenticodeSignature::new(&pkcs7, FILE_HASH.to_vec(), hash_type, &private_key, Some(program_name))
                .unwrap();

        let pkcs7certificate = authenticode_signature.0 .0;

        let Pkcs7Certificate { signed_data, .. } = pkcs7certificate;

        let content_info = &signed_data.content_info;

        assert_eq!(
            Into::<String>::into(&content_info.content_type.0).as_str(),
            oids::SPC_INDIRECT_DATA_OBJID
        );

        let spc_indirect_data_content = content_info.content.as_ref().unwrap();
        let message_digest = match &spc_indirect_data_content.0 {
            ContentValue::SpcIndirectDataContent(SpcIndirectDataContent {
                data: _,
                message_digest,
            }) => message_digest.clone(),
            _ => panic!("Expected ContentValue with SpcIndirectDataContent, but got something else"),
        };

        let hash_algo = AlgorithmIdentifier::new_sha(hash_type);
        assert_eq!(message_digest.oid, hash_algo);

        pretty_assertions::assert_eq!(message_digest.digest.0, FILE_HASH);

        assert_eq!(signed_data.signers_infos.0 .0.len(), 1);

        let singer_info = signed_data.signers_infos.0 .0.first().unwrap();
        assert_eq!(singer_info.digest_algorithm.0, hash_algo);

        let authenticated_attributes = &singer_info.signed_attrs.0 .0;

        if !authenticated_attributes
            .iter()
            .any(|attr| matches!(attr.value, AttributeValues::ContentType(_)))
        {
            panic!("ContentType attribute is missing");
        }

        if !authenticated_attributes
            .iter()
            .any(|attr| matches!(attr.value, AttributeValues::MessageDigest(_)))
        {
            panic!("MessageDigest attribute is missing");
        }

        if !authenticated_attributes
            .iter()
            .any(|attr| matches!(attr.value, AttributeValues::SpcSpOpusInfo(_)))
        {
            panic!("SpcSpOpusInfo attribute is missing");
        }

        assert_eq!(
            singer_info.signature_algorithm,
            SignatureAlgorithmIdentifier(AlgorithmIdentifier::new_rsa_encryption())
        );

        let signature_algo =
            SignatureAlgorithm::from_algorithm_identifier(&AlgorithmIdentifier::new_sha256_with_rsa_encryption())
                .unwrap();

        let certificate = signed_data
            .certificates
            .0
            .clone()
            .0
            .into_iter()
            .filter_map(|cert| match cert {
                CertificateChoices::Certificate(certificate) => Cert::from_der(&certificate.0).ok(),
                CertificateChoices::Other(_) => None,
            })
            .find(|certificate| matches!(certificate.ty(), CertType::Intermediate))
            .map(Certificate::from)
            .unwrap();

        let code_signing_ext_key_usage = Extension::new_extended_key_usage(vec![oids::kp_code_signing()]);
        assert!(!certificate
            .tbs_certificate
            .extensions
            .0
             .0
            .iter()
            .any(|extension| extension == &code_signing_ext_key_usage));

        let public_key = certificate.tbs_certificate.subject_public_key_info;
        let encrypted_digest = singer_info.signature.0 .0.as_ref();

        let mut auth_raw_data = picky_asn1_der::to_vec(&authenticated_attributes).unwrap();
        auth_raw_data[0] = Tag::SET.inner();

        signature_algo
            .verify(&public_key.into(), auth_raw_data.as_ref(), encrypted_digest)
            .unwrap();
    }

    #[test]
    fn into_authenticate_signature_from_pkcs7_with_x509_root_chain() {
        let pem = "-----BEGIN PKCS7-----\
                     MIIKDgYJKoZIhvcNAQcCoIIJ/zCCCfsCAQExADALBgkqhkiG9w0BBwGgggnhMIIE\
                     NjCCAh4CAWUwDQYJKoZIhvcNAQELBQAwYTELMAkGA1UEBhMCSUkxCzAJBgNVBAgM\
                     AklJMQswCQYDVQQHDAJJSTELMAkGA1UECgwCSUkxCzAJBgNVBAsMAklJMQswCQYD\
                     VQQDDAJJSTERMA8GCSqGSIb3DQEJARYCSUkwHhcNMjEwNDI5MTAxNTM3WhcNMjQw\
                     MTI0MTAxNTM3WjBhMQswCQYDVQQGEwJTUzELMAkGA1UECAwCU1MxCzAJBgNVBAcM\
                     AlNTMQswCQYDVQQKDAJTUzELMAkGA1UECwwCU1MxCzAJBgNVBAMMAlNTMREwDwYJ\
                     KoZIhvcNAQkBFgJTUzCCASIwDQYJKoZIhvcNAQEBBQADggEPADCCAQoCggEBALFT\
                     ERznf389kwmFdy3RFeSQKRiU5Sr8k5ChXp+74u2kKVcbQixS2KQti3KopB8Xkly4\
                     31TBDRHRqR5H/x+KY/Pjp/iTFX6AXom4mxAglPxGeKdNuWesBdIf6hZcIJ2rZv94\
                     G67m4ggCS0oDB3qYykw02wO6QeEZHA0AiLRusR4SQZZNZc3Z6JUSijTJZE+TKxL8\
                     eoVNI5P4/+aSY4wdLPK+qEfzsumxVSbqQWO7aWWd6yYYCsGhd/k9pVJMmqXH+rkL\
                     lVcxtYkAnH1TOuvsAn+FBMwr/lXuenT3DiFTDJtm/Mu4ZM2lD60o6aDCSdYSOh/w\
                     amTGCX3WWMmpqfBFRy8CAwEAATANBgkqhkiG9w0BAQsFAAOCAgEAy51uXBnzdU1d\
                     U8K3kh1naldLG6/UF8jJhiN7vmltPuYSvPeIzNk/g4UXUACHnUzg7JWRobf0VBHw\
                     /I8xGXjnRRo2s9w2ZyF4CyDC7YVZWK/bvUe/7qMteZPBXC1Fgz03UC0Y/Y9jqCUJ\
                     ev0bl+u9jRkoE5aMiJhQOzn+CDGxKXvefDpDtvt8nuqNnY9vP7fo3wTLhWF4RUFA\
                     p8iNQu4Pw1XaHhJ467c5kFZLBz+E75myIRJfRYYmBw6nWLSDNueI/Jw+N6jxTKw6\
                     +PtqGx91YTgUK61HHTe8qY7HYCt8ZNmJWvzYpBjUCMEx0BS3sQ7KLc8piD7C5aH7\
                     YzS1PLA2hk4nAk1+uDlQrbfZl+p3ED8NTIvbL9GPBqTQAjOwVkspuidfabgsg/yk\
                     0Nh+3AFMkAy3MoSHmf0AugWyd1F37xx8SePY7NSznWbd7z6UP0WpS4k3BaYWxll1\
                     Q4jXPVghHuQBgmsamx6uXI950DszVvvzubmoVFGsOhdq6BLoZ4dx2mh2teLyPOH0\
                     77nEEOREhilxLMunGUZsZ5rcZuLgMKwOMxY7Sk3x4ETLG9R5Fhe+w70xZfWkKEt0\
                     o7cjRnNM3njJs+TKSZYXcv/9AKhWNUyqhrgUtbsWjTBnXaRyBtDZR3iQx9t0QQZ/\
                     cX0bsED8y9zkFxTIYcSbJuYtcO2ldm8wggWjMIIDi6ADAgECAhRf5s94qhLBW634\
                     aeLH/M67kmZ+8TANBgkqhkiG9w0BAQsFADBhMQswCQYDVQQGEwJTUzELMAkGA1UE\
                     CAwCUlIxCzAJBgNVBAcMAlJSMQswCQYDVQQKDAJSUjELMAkGA1UECwwCUlIxCzAJ\
                     BgNVBAMMAlJSMREwDwYJKoZIhvcNAQkBFgJSUjAeFw0yMTA0MjkxMDA0MzZaFw0y\
                     NjA0MjkxMDA0MzZaMGExCzAJBgNVBAYTAlNTMQswCQYDVQQIDAJSUjELMAkGA1UE\
                     BwwCUlIxCzAJBgNVBAoMAlJSMQswCQYDVQQLDAJSUjELMAkGA1UEAwwCUlIxETAP\
                     BgkqhkiG9w0BCQEWAlJSMIICIjANBgkqhkiG9w0BAQEFAAOCAg8AMIICCgKCAgEA\
                     2AXxt6RZbLKqkw+9Y0FbT+hMS/MpPMEnCINTHK3gzhjP3hhOKfVhHekfWoZ07gZz\
                     IcJMfcXTLFbsFDuZssj63YXiKkk5AXstgd+8F2nW4xNLdXiAD6/vQQYzX/KJO1+T\
                     5y1vAuAvO4xybe0HHMsIcLlUv45BaEBOFizUwMsDnE+GEVfsfFNhxxvLz0daGrSF\
                     c7C0DgG4qNC9ONrOThZhBeDud8g6LLYSTHnIblEfbsVPUlNI+mk8vFNoQYAoz74M\
                     dIfJZQ3+yoqqDAlVAERo9bD8ejnB296OVIpzjHr8v4Y1hxB1UIE7P9LIYhl2FOI1\
                     F57MyGM+qUD86s4ycxq1emrjurhG8xzUttUAg4RRogtJdJrGu9AX2RbnWV6yLfZl\
                     bE6NG2LBuxihZ40vMYFNt3CgZ/7MUc7mJcPsg+5uu86jMWWJ3/0kBIhGIEVsyDsy\
                     RyAE1trB5Zs1yvuEVx8UDs6nrXns+q7lexliomVGPQGf4eoJaNGXqR4xB8oKqCCJ\
                     pMdNARAYOEAjtYvkoZVSgQb0HoScZmlXywwlRVMiDToSsE7pheiv5WEyiBSoMMnI\
                     OMFyIu9YP5DBjVweggqJDvg6n+iRqsTckRbY/wxUIcMpczkhTPAI0zCHLbtkCA7S\
                     caZjpPP7Q1I2XtquR08vflsGrwcVl9OSOVqJ4AN5xiMCAwEAAaNTMFEwHQYDVR0O\
                     BBYEFFbRR8T+Mn5/zWJ4dRa4o7oqzWK3MB8GA1UdIwQYMBaAFFbRR8T+Mn5/zWJ4\
                     dRa4o7oqzWK3MA8GA1UdEwEB/wQFMAMBAf8wDQYJKoZIhvcNAQELBQADggIBAL+5\
                     oOOyd1M14xbdyxKTJyQQbR/3Fu+ycLoKqHI8xDtqpxQz39r6TjZVzRPGmkaO2LFr\
                     jS6vatIG+X2A8qsKS6Oab0BMj/0uKpibS8sm/4wJCUVj5fRHRAfTHOeis6efYgB4\
                     t/Er3WKbpfnySPKtxr5vO6KpYk+cRz6aIs5zD3I/5LXdyn4BD4O4RH8w5m55xs0G\
                     pZWH1K1SZmv6bmWmnKM5x5kECbsJQDA9oNCV2Vqg3y52dxmvuworzMlu2gnpdQQT\
                     6Ibh65SYtfYBQTn2bPQB+YPfWqGoWSDUq7CHybUNCgKqYw7+4X/cJB0IkUMZt+Pw\
                     kS3YiYP8hRM4lQAs6ITiB1GSpPuL9cCRcOjHvjilLiZJGfskGy8ucqlj24LEvtb6\
                     DWu45SyjuQ08r6ORxkVg/cz1ztx0BrIVMQMxpIYUi1xPHPpz60j4Y1v1O2XvRJMu\
                     Xg6ulyYWYaw+V+VopcQWBvAe1gYUk0CVzneBEjauzT1qX8K/Fu6f5ltQEJ5XGuYY\
                     pHEn99xhnRUSThoBvOwQj8JjD8uiCJvvOVugF1wEh4RIcCKj7r4u91c41ndg7FU0\
                     kxVRpfjDmxxzQib3Q/z4ZAoqW7+Hjq6gqim2ngrB9Co9pv4ckJ5APDx6x9WF8gpc\
                     Ydn+09ezBdJ4Zgn5U7GdrkNAgOzXtBwbiKxlGcWWoQAxAA==\
                     -----END PKCS7-----";

        let pkcs7 = Pkcs7::from_pem_str(pem).unwrap();
        let private_key = PrivateKey::from_pem_str(RSA_PRIVATE_KEY).unwrap();

        AuthenticodeSignature::new(
            &pkcs7,
            FILE_HASH.to_vec(),
            ShaVariant::SHA2_256,
            &private_key,
            Some("into_authenticate_signature_from_pkcs7_with_x509_root_chain".to_string()),
        )
        .unwrap();
    }

    #[test]
    fn self_signed_authenticode_signature_basic_validation() {
        let pkcs7 = Pkcs7::from_pem_str(SELF_SIGNED_PKCS7).unwrap();
        let private_key = PrivateKey::from_pem_str(SELF_SIGNED_PKCS7_RSA_PRIVATE_KEY).unwrap();

        let authenticode_signature = AuthenticodeSignature::new(
            &pkcs7,
            FILE_HASH.to_vec(),
            ShaVariant::SHA2_256,
            &private_key,
            Some("self_signed_authenticode_signature_basic_validation".to_string()),
        )
        .unwrap();
        let file_hash = authenticode_signature.file_hash().expect("File hash should be present");

        let validator = authenticode_signature.authenticode_verifier();
        validator
            .require_basic_authenticode_validation(file_hash)
            .ignore_signing_certificate_check()
            .ignore_chain_check()
            .ignore_not_after_check()
            .ignore_not_before_check()
            .verify()
            .unwrap();
    }

    #[test]
    fn self_signed_authenticate_signature_with_basic_and_signing_certificate_validation() {
        let pkcs7 = Pkcs7::from_pem_str(SELF_SIGNED_PKCS7).unwrap();
        let private_key = PrivateKey::from_pem_str(SELF_SIGNED_PKCS7_RSA_PRIVATE_KEY).unwrap();

        let authenticode_signature = AuthenticodeSignature::new(
            &pkcs7,
            FILE_HASH.to_vec(),
            ShaVariant::SHA2_256,
            &private_key,
            Some("self_signed_authenticate_signature_with_basic_and_signing_certificate_validation".to_string()),
        )
        .unwrap();

        let file_hash = authenticode_signature.file_hash().expect("File hash should be present");
        let validator = authenticode_signature.authenticode_verifier();
        validator
            .require_basic_authenticode_validation(file_hash)
            .require_signing_certificate_check()
            .exact_date(&UtcDate::new(2021, 8, 7, 0, 0, 0).unwrap())
            .ignore_chain_check()
            .verify()
            .unwrap();
    }

    #[cfg(feature = "ctl_http_fetch")]
    #[test]
    fn self_signed_authenticode_signature_validation_against_ctl() {
        use ctl::http_fetch::CtlHttpFetch;

        let pkcs7 = Pkcs7::from_pem_str(SELF_SIGNED_PKCS7).unwrap();
        let private_key = PrivateKey::from_pem_str(SELF_SIGNED_PKCS7_RSA_PRIVATE_KEY).unwrap();

        let authenticode_signature = AuthenticodeSignature::new(
            &pkcs7,
            FILE_HASH.to_vec(),
            ShaVariant::SHA2_256,
            &private_key,
            Some("self_signed_authenticode_signature_validation_against_ctl".to_string()),
        )
        .unwrap();

        let ctl = CertificateTrustList::fetch().unwrap();

        let validator = authenticode_signature.authenticode_verifier();
        let validator = validator
            .ctl(&ctl)
            .require_ca_against_ctl_check()
            .ignore_signing_certificate_check()
            .ignore_chain_check()
            .ignore_not_after_check()
            .ignore_not_before_check()
            .ignore_basic_authenticode_validation();
        let err = validator.verify().unwrap_err();
        assert_eq!(err.to_string(), "The Authenticode signature CA is not trusted");
    }

    #[cfg(feature = "ctl_http_fetch")]
    #[test]
    fn self_signed_authenticode_signature_validation_against_ctl_with_excluded_ca_certificate() {
        use ctl::http_fetch::CtlHttpFetch;

        let pkcs7 = Pkcs7::from_pem_str(SELF_SIGNED_PKCS7).unwrap();
        let private_key = PrivateKey::from_pem_str(SELF_SIGNED_PKCS7_RSA_PRIVATE_KEY).unwrap();

        let authenticode_signature = AuthenticodeSignature::new(
            &pkcs7,
            FILE_HASH.to_vec(),
            ShaVariant::SHA2_256,
            &private_key,
            Some("self_signed_authenticode_signature_validation_against_ctl_with_excluded_ca_certificate".to_string()),
        )
        .unwrap();

        let validator = authenticode_signature.authenticode_verifier();
        let ca_name = authenticode_signature
            .0
            .decode_certificates()
            .iter()
            .find(|cert| cert.ty() == CertType::Intermediate)
            .unwrap()
            .issuer_name();

        let ctl = CertificateTrustList::fetch().unwrap();

        validator
            .ctl(&ctl)
            .require_ca_against_ctl_check()
            .exclude_cert_authorities(&[ca_name])
            .ignore_signing_certificate_check()
            .ignore_chain_check()
            .ignore_not_after_check()
            .ignore_not_before_check()
            .ignore_basic_authenticode_validation()
            .verify()
            .unwrap();
    }

    #[cfg(feature = "ctl_http_fetch")]
    #[test]
    fn self_signed_authenticode_signature_validation_against_ctl_with_excluded_not_existing_ca_certificate() {
        use crate::x509::name::NameAttr;
        use ctl::http_fetch::CtlHttpFetch;

        let pkcs7 = Pkcs7::from_pem_str(SELF_SIGNED_PKCS7).unwrap();
        let private_key = PrivateKey::from_pem_str(SELF_SIGNED_PKCS7_RSA_PRIVATE_KEY).unwrap();

        let authenticode_signature = AuthenticodeSignature::new(
            &pkcs7,
            FILE_HASH.to_vec(),
            ShaVariant::SHA2_256,
            &private_key,
            Some(
                "self_signed_authenticode_signature_validation_against_ctl_with_excluded_not_existing_ca_certificate"
                    .to_string(),
            ),
        )
        .unwrap();

        let validator = authenticode_signature.authenticode_verifier();
        let mut ca_name = DirectoryName::new_common_name("A non-existent CA");
        ca_name.add_attr(NameAttr::LocalityName, "The Place that nobody knows");
        ca_name.add_attr(NameAttr::OrganizationName, "A Bad known organization");
        ca_name.add_attr(NameAttr::StateOrProvinceName, "The first state of Mars");
        let ctl = CertificateTrustList::fetch().unwrap();

        let err = validator
            .ctl(&ctl)
            .require_ca_against_ctl_check()
            .exclude_cert_authorities(&[ca_name])
            .ignore_signing_certificate_check()
            .ignore_chain_check()
            .ignore_not_after_check()
            .ignore_not_before_check()
            .ignore_basic_authenticode_validation()
            .verify()
            .unwrap_err();

        assert_eq!(err.to_string(), "The Authenticode signature CA is not trusted");
    }

    #[cfg(feature = "ctl_http_fetch")]
    #[test]
    fn full_validation_self_signed_authenticode_signature() {
        use ctl::http_fetch::CtlHttpFetch;

        let pkcs7 = Pkcs7::from_pem_str(SELF_SIGNED_PKCS7).unwrap();
        let private_key = PrivateKey::from_pem_str(SELF_SIGNED_PKCS7_RSA_PRIVATE_KEY).unwrap();

        let authenticode_signature = AuthenticodeSignature::new(
            &pkcs7,
            FILE_HASH.to_vec(),
            ShaVariant::SHA2_256,
            &private_key,
            Some("self_signed_authenticode_signature_validation_against_ctl_with_excluded_ca_certificate".to_string()),
        )
        .unwrap();
        let file_hash = authenticode_signature.file_hash().expect("File hash should be present");

        let validator = authenticode_signature.authenticode_verifier();

        let ca_name = authenticode_signature
            .0
            .decode_certificates()
            .iter()
            .find(|cert| cert.ty() == CertType::Intermediate)
            .unwrap()
            .issuer_name();

        let ctl = CertificateTrustList::fetch().unwrap();

        validator
            .require_basic_authenticode_validation(file_hash)
            .require_signing_certificate_check()
            .require_chain_check()
            .interval_date(
                &UtcDate::new(2021, 7, 8, 11, 56, 49).unwrap(),
                &UtcDate::new(2022, 7, 8, 11, 56, 49).unwrap(),
            )
            .require_not_before_check()
            .require_not_after_check()
            .ctl(&ctl)
            .require_ca_against_ctl_check()
            .exclude_cert_authorities(&[ca_name])
            .verify()
            .unwrap();
    }

    #[cfg(feature = "ctl_http_fetch")]
    #[test]
    fn full_validation_authenticode_signature_with_well_known_ca() {
        use ctl::http_fetch::CtlHttpFetch;

        let pkcs7 = "-----BEGIN PKCS7-----\
                        MIIjkgYJKoZIhvcNAQcCoIIjgzCCI38CAQExDzANBglghkgBZQMEAgEFADB5Bgor\
                        BgEEAYI3AgEEoGswaTA0BgorBgEEAYI3AgEeMCYCAwEAAAQQH8w7YFlLCE63JNLG\
                        KX7zUQIBAAIBAAIBAAIBAAIBADAxMA0GCWCGSAFlAwQCAQUABCBOlmcSb72K+wH5\
                        7rgEoyM/xepQH0ZFeACdfeWgW6yh06CCDYEwggX/MIID56ADAgECAhMzAAABh3IX\
                        chVZQMcJAAAAAAGHMA0GCSqGSIb3DQEBCwUAMH4xCzAJBgNVBAYTAlVTMRMwEQYD\
                        VQQIEwpXYXNoaW5ndG9uMRAwDgYDVQQHEwdSZWRtb25kMR4wHAYDVQQKExVNaWNy\
                        b3NvZnQgQ29ycG9yYXRpb24xKDAmBgNVBAMTH01pY3Jvc29mdCBDb2RlIFNpZ25p\
                        bmcgUENBIDIwMTEwHhcNMjAwMzA0MTgzOTQ3WhcNMjEwMzAzMTgzOTQ3WjB0MQsw\
                        CQYDVQQGEwJVUzETMBEGA1UECBMKV2FzaGluZ3RvbjEQMA4GA1UEBxMHUmVkbW9u\
                        ZDEeMBwGA1UEChMVTWljcm9zb2Z0IENvcnBvcmF0aW9uMR4wHAYDVQQDExVNaWNy\
                        b3NvZnQgQ29ycG9yYXRpb24wggEiMA0GCSqGSIb3DQEBAQUAA4IBDwAwggEKAoIB\
                        AQDOt8kLc7P3T7MKIhouYHewMFmnq8Ayu7FOhZCQabVwBp2VS4WyB2Qe4TQBT8aB\
                        znANDEPjHKNdPT8Xz5cNali6XHefS8i/WXtF0vSsP8NEv6mBHuA2p1fw2wB/F0dH\
                        sJ3GfZ5c0sPJjklsiYqPw59xJ54kM91IOgiO2OUzjNAljPibjCWfH7UzQ1TPHc4d\
                        weils8GEIrbBRb7IWwiObL12jWT4Yh71NQgvJ9Fn6+UhD9x2uk3dLj84vwt1NuFQ\
                        itKJxIV0fVsRNR3abQVOLqpDugbr0SzNL6o8xzOHL5OXiGGwg6ekiXA1/2XXY7yV\
                        Fc39tledDtZjSjNbex1zzwSXAgMBAAGjggF+MIIBejAfBgNVHSUEGDAWBgorBgEE\
                        AYI3TAgBBggrBgEFBQcDAzAdBgNVHQ4EFgQUhov4ZyO96axkJdMjpzu2zVXOJcsw\
                        UAYDVR0RBEkwR6RFMEMxKTAnBgNVBAsTIE1pY3Jvc29mdCBPcGVyYXRpb25zIFB1\
                        ZXJ0byBSaWNvMRYwFAYDVQQFEw0yMzAwMTIrNDU4Mzg1MB8GA1UdIwQYMBaAFEhu\
                        ZOVQBdOCqhc3NyK1bajKdQKVMFQGA1UdHwRNMEswSaBHoEWGQ2h0dHA6Ly93d3cu\
                        bWljcm9zb2Z0LmNvbS9wa2lvcHMvY3JsL01pY0NvZFNpZ1BDQTIwMTFfMjAxMS0w\
                        Ny0wOC5jcmwwYQYIKwYBBQUHAQEEVTBTMFEGCCsGAQUFBzAChkVodHRwOi8vd3d3\
                        Lm1pY3Jvc29mdC5jb20vcGtpb3BzL2NlcnRzL01pY0NvZFNpZ1BDQTIwMTFfMjAx\
                        MS0wNy0wOC5jcnQwDAYDVR0TAQH/BAIwADANBgkqhkiG9w0BAQsFAAOCAgEAixmy\
                        S6E6vprWD9KFNIB9G5zyMuIjZAOuUJ1EK/Vlg6Fb3ZHXjjUwATKIcXbFuFC6Wr4K\
                        NrU4DY/sBVqmab5AC/je3bpUpjtxpEyqUqtPc30wEg/rO9vmKmqKoLPT37svc2NV\
                        BmGNl+85qO4fV/w7Cx7J0Bbqk19KcRNdjt6eKoTnTPHBHlVHQIHZpMxacbFOAkJr\
                        qAVkYZdz7ikNXTxV+GRb36tC4ByMNxE2DF7vFdvaiZP0CVZ5ByJ2gAhXMdK9+usx\
                        zVk913qKde1OAuWdv+rndqkAIm8fUlRnr4saSCg7cIbUwCCf116wUJ7EuJDg0vHe\
                        yhnCeHnBbyH3RZkHEi2ofmfgnFISJZDdMAeVZGVOh20Jp50XBzqokpPzeZ6zc1/g\
                        yILNyiVgE+RPkjnUQshd1f1PMgn3tns2Cz7bJiVUaqEO3n9qRFgy5JuLae6UweGf\
                        AeOo3dgLZxikKzYs3hDMaEtJq8IP71cX7QXe6lnMmXU/Hdfz2p897Zd+kU+vZvKI\
                        3cwLfuVQgK2RZ2z+Kc3K3dRPz2rXycK5XCuRZmvGab/WbrZiC7wJQapgBodltMI5\
                        GMdFrBg9IeF7/rP4EqVQXeKtevTlZXjpuNhhjuR+2DMt/dWufjXpiW91bo3aH6Ea\
                        jOALXmoxgltCp1K7hrS6gmsvj94cLRf50QQ4U8Qwggd6MIIFYqADAgECAgphDpDS\
                        AAAAAAADMA0GCSqGSIb3DQEBCwUAMIGIMQswCQYDVQQGEwJVUzETMBEGA1UECBMK\
                        V2FzaGluZ3RvbjEQMA4GA1UEBxMHUmVkbW9uZDEeMBwGA1UEChMVTWljcm9zb2Z0\
                        IENvcnBvcmF0aW9uMTIwMAYDVQQDEylNaWNyb3NvZnQgUm9vdCBDZXJ0aWZpY2F0\
                        ZSBBdXRob3JpdHkgMjAxMTAeFw0xMTA3MDgyMDU5MDlaFw0yNjA3MDgyMTA5MDla\
                        MH4xCzAJBgNVBAYTAlVTMRMwEQYDVQQIEwpXYXNoaW5ndG9uMRAwDgYDVQQHEwdS\
                        ZWRtb25kMR4wHAYDVQQKExVNaWNyb3NvZnQgQ29ycG9yYXRpb24xKDAmBgNVBAMT\
                        H01pY3Jvc29mdCBDb2RlIFNpZ25pbmcgUENBIDIwMTEwggIiMA0GCSqGSIb3DQEB\
                        AQUAA4ICDwAwggIKAoICAQCr8PpyEBwurdhuqoIQTTS68rZYIZ9CGypr6VpQqrgG\
                        OBoESbp/wwwe3TdrxhLYC/A4wpkGsMg51QEUMULTiQ15ZId+lGAkbK+eSZzpaF7S\
                        35tTsgosw6/ZqSuuegmv15ZZymAaBelmdugyUiYSL+erCFDPs0S3XdjELgN1q2jz\
                        y23zOlyhFvRGuuA4ZKxuZDV4pqBjDy3TQJP4494HDdVceaVJKecNvqATd76UPe/7\
                        4ytaEB9NViiienLgEjq3SV7Y7e1DkYPZe7J7hhvZPrGMXeiJT4Qa8qEvWeSQOy2u\
                        M1jFtz7+MtOzAz2xsq+SOH7SnYAs9U5WkSE1JcM5bmR/U7qcD60ZI4TL9LoDho33\
                        X/DQUr+MlIe8wCF0JV8YKLbMJyg4JZg5SjbPfLGSrhwjp6lm7GEfauEoSZ1fiOIl\
                        XdMhSz5SxLVXPyQD8NF6Wy/VI+NwXQ9RRnez+ADhvKwCgl/bwBWzvRvUVUvnOaEP\
                        6SNJvBi4RHxF5MHDcnrgcuck379GmcXvwhxX24ON7E1JMKerjt/sW5+v/N2wZuLB\
                        l4F77dbtS+dJKacTKKanfWeA5opieF+yL4TXV5xcv3coKPHtbcMojyyPQDdPweGF\
                        RInECUzF1KVDL3SV9274eCBYLBNdYJWaPk8zhNqwiBfenk70lrC8RqBsmNLg1oiM\
                        CwIDAQABo4IB7TCCAekwEAYJKwYBBAGCNxUBBAMCAQAwHQYDVR0OBBYEFEhuZOVQ\
                        BdOCqhc3NyK1bajKdQKVMBkGCSsGAQQBgjcUAgQMHgoAUwB1AGIAQwBBMAsGA1Ud\
                        DwQEAwIBhjAPBgNVHRMBAf8EBTADAQH/MB8GA1UdIwQYMBaAFHItOgIxkEO5FAVO\
                        4eqnxzHRI4k0MFoGA1UdHwRTMFEwT6BNoEuGSWh0dHA6Ly9jcmwubWljcm9zb2Z0\
                        LmNvbS9wa2kvY3JsL3Byb2R1Y3RzL01pY1Jvb0NlckF1dDIwMTFfMjAxMV8wM18y\
                        Mi5jcmwwXgYIKwYBBQUHAQEEUjBQME4GCCsGAQUFBzAChkJodHRwOi8vd3d3Lm1p\
                        Y3Jvc29mdC5jb20vcGtpL2NlcnRzL01pY1Jvb0NlckF1dDIwMTFfMjAxMV8wM18y\
                        Mi5jcnQwgZ8GA1UdIASBlzCBlDCBkQYJKwYBBAGCNy4DMIGDMD8GCCsGAQUFBwIB\
                        FjNodHRwOi8vd3d3Lm1pY3Jvc29mdC5jb20vcGtpb3BzL2RvY3MvcHJpbWFyeWNw\
                        cy5odG0wQAYIKwYBBQUHAgIwNB4yIB0ATABlAGcAYQBsAF8AcABvAGwAaQBjAHkA\
                        XwBzAHQAYQB0AGUAbQBlAG4AdAAuIB0wDQYJKoZIhvcNAQELBQADggIBAGfyhqWY\
                        4FR5Gi7T2HRnIpsLlhHhY5KZQpZ90nkMkMFlXy4sPvjDctFtg/6+P+gKyju/R6mj\
                        82nbY78iNaWXXWWEkH2LRlBV2AySfNIaSxzzPEKLUtCw/WvjPgcuKZvmPRul1LUd\
                        d5Q54ulkyUQ9eHoj8xN9ppB0g430yyYCRirCihC7pKkFDJvtaPpoLpWgKj8qa1hJ\
                        Yx8JaW5amJbkg/TAj/NGK978O9C9Ne9uJa7lryft0N3zDq+ZKJeYTQ49C/IIidYf\
                        wzIY4vDFLc5bnrRJOQrGCsLGra7lstnbFYhRRVg4MnEnGn+x9Cf43iw6IGmYslmJ\
                        aG5vp7d0w0AFBqYBKig+gj8TTWYLwLNN9eGPfxxvFX1Fp3blQCplo8NdUmKGwx1j\
                        NpeG39rz+PIWoZon4c2ll9DuXWNB41sHnIc+BncG0QaxdR8UvmFhtfDcxhsEvt9B\
                        xw4o7t5lL+yX9qFcltgA1qFGvVnzl6UJS0gQmYAf0AApxbGbpT9Fdx41xtKiop96\
                        eiL6SJUfq/tHI4D1nvi/a7dLl+LrdXga7Oo3mXkYS//WsyNodeav+vyL6wuA6mk7\
                        r/ww7QRMjt/fdW1jkT3RnVZOT7+AVyKheBEyIXrvQQqxP/uozKRdwaGIm1dxVk5I\
                        RcBCyZt2WwqASGv9eZ/BvW1taslScxMNelDNMYIVZzCCFWMCAQEwgZUwfjELMAkG\
                        A1UEBhMCVVMxEzARBgNVBAgTCldhc2hpbmd0b24xEDAOBgNVBAcTB1JlZG1vbmQx\
                        HjAcBgNVBAoTFU1pY3Jvc29mdCBDb3Jwb3JhdGlvbjEoMCYGA1UEAxMfTWljcm9z\
                        b2Z0IENvZGUgU2lnbmluZyBQQ0EgMjAxMQITMwAAAYdyF3IVWUDHCQAAAAABhzAN\
                        BglghkgBZQMEAgEFAKCBrjAZBgkqhkiG9w0BCQMxDAYKKwYBBAGCNwIBBDAcBgor\
                        BgEEAYI3AgELMQ4wDAYKKwYBBAGCNwIBFTAvBgkqhkiG9w0BCQQxIgQgSI3mmyEc\
                        XjWLEpbhWFEEl6gPBJhjiWhxF4WcneiXnlYwQgYKKwYBBAGCNwIBDDE0MDKgFIAS\
                        AE0AaQBjAHIAbwBzAG8AZgB0oRqAGGh0dHA6Ly93d3cubWljcm9zb2Z0LmNvbTAN\
                        BgkqhkiG9w0BAQEFAASCAQCyr15gPEMGURRpVeQjtCEpn9waDuDlkW11PiBt2A/j\
                        PdbhN4JupkncXgZtKt29s1usM8p+bSTkao5bpeIEV5UEMxgbsaxUCipxNki+z7LW\
                        KmFzviTsUU1/CqSJ2EKZdhQENUtpmgOr0D/CHTbbAVSpiVcfQuZI8hWulziFVqRE\
                        4xGCR/sKOfQ1DT2DiOwlbf6tmceD04QaDlioZ8SVXTEvlP36a5rv8tmyw9lkkBgV\
                        B824Xh0H8CrqajF+x9zR9CjBox4Y/bf3Oe1Pir6k5IT7ZEkSQ9XRJfaNNm42i/9h\
                        IUPesYs9gr0zXJdxlri7Y2PPkphB9JQ+k+wa20nxBBIDoYIS8TCCEu0GCisGAQQB\
                        gjcDAwExghLdMIIS2QYJKoZIhvcNAQcCoIISyjCCEsYCAQMxDzANBglghkgBZQME\
                        AgEFADCCAVUGCyqGSIb3DQEJEAEEoIIBRASCAUAwggE8AgEBBgorBgEEAYRZCgMB\
                        MDEwDQYJYIZIAWUDBAIBBQAEIPeHx1THLsARquah0ml1x5Wutabkis4dsFKSE3WJ\
                        HwZlAgZfYPphXw0YEzIwMjAwOTIyMjIxOTUzLjI1NVowBIACAfSggdSkgdEwgc4x\
                        CzAJBgNVBAYTAlVTMRMwEQYDVQQIEwpXYXNoaW5ndG9uMRAwDgYDVQQHEwdSZWRt\
                        b25kMR4wHAYDVQQKExVNaWNyb3NvZnQgQ29ycG9yYXRpb24xKTAnBgNVBAsTIE1p\
                        Y3Jvc29mdCBPcGVyYXRpb25zIFB1ZXJ0byBSaWNvMSYwJAYDVQQLEx1UaGFsZXMg\
                        VFNTIEVTTjowQTU2LUUzMjktNEQ0RDElMCMGA1UEAxMcTWljcm9zb2Z0IFRpbWUt\
                        U3RhbXAgU2VydmljZaCCDkQwggT1MIID3aADAgECAhMzAAABJy9uo++RqBmoAAAA\
                        AAEnMA0GCSqGSIb3DQEBCwUAMHwxCzAJBgNVBAYTAlVTMRMwEQYDVQQIEwpXYXNo\
                        aW5ndG9uMRAwDgYDVQQHEwdSZWRtb25kMR4wHAYDVQQKExVNaWNyb3NvZnQgQ29y\
                        cG9yYXRpb24xJjAkBgNVBAMTHU1pY3Jvc29mdCBUaW1lLVN0YW1wIFBDQSAyMDEw\
                        MB4XDTE5MTIxOTAxMTQ1OVoXDTIxMDMxNzAxMTQ1OVowgc4xCzAJBgNVBAYTAlVT\
                        MRMwEQYDVQQIEwpXYXNoaW5ndG9uMRAwDgYDVQQHEwdSZWRtb25kMR4wHAYDVQQK\
                        ExVNaWNyb3NvZnQgQ29ycG9yYXRpb24xKTAnBgNVBAsTIE1pY3Jvc29mdCBPcGVy\
                        YXRpb25zIFB1ZXJ0byBSaWNvMSYwJAYDVQQLEx1UaGFsZXMgVFNTIEVTTjowQTU2\
                        LUUzMjktNEQ0RDElMCMGA1UEAxMcTWljcm9zb2Z0IFRpbWUtU3RhbXAgU2Vydmlj\
                        ZTCCASIwDQYJKoZIhvcNAQEBBQADggEPADCCAQoCggEBAPgB3nERnk6fS40vvWeD\
                        3HCgM9Ep4xTIQiPnJXE9E+HkZVtTsPemoOyhfNAyF95E/rUvXOVTUcJFL7Xb16jT\
                        KPXONsCWY8DCixSDIiid6xa30TiEWVcIZRwiDlcx29D467OTav5rA1G6TwAEY5rQ\
                        jhUHLrOoJgfJfakZq6IHjd+slI0/qlys7QIGakFk2OB6mh/ln/nS8G4kNRK6Do4g\
                        xDtnBSFLNfhsSZlRSMDJwFvrZ2FCkaoexd7rKlUNOAAScY411IEqQeI1PwfRm3aW\
                        bS8IvAfJPC2Ah2LrtP8sKn5faaU8epexje7vZfcZif/cbxgUKStJzqbdvTBNc93n\
                        /Z8CAwEAAaOCARswggEXMB0GA1UdDgQWBBTl9JZVgF85MSRbYlOJXbhY022V8jAf\
                        BgNVHSMEGDAWgBTVYzpcijGQ80N7fEYbxTNoWoVtVTBWBgNVHR8ETzBNMEugSaBH\
                        hkVodHRwOi8vY3JsLm1pY3Jvc29mdC5jb20vcGtpL2NybC9wcm9kdWN0cy9NaWNU\
                        aW1TdGFQQ0FfMjAxMC0wNy0wMS5jcmwwWgYIKwYBBQUHAQEETjBMMEoGCCsGAQUF\
                        BzAChj5odHRwOi8vd3d3Lm1pY3Jvc29mdC5jb20vcGtpL2NlcnRzL01pY1RpbVN0\
                        YVBDQV8yMDEwLTA3LTAxLmNydDAMBgNVHRMBAf8EAjAAMBMGA1UdJQQMMAoGCCsG\
                        AQUFBwMIMA0GCSqGSIb3DQEBCwUAA4IBAQAKyo180VXHBqVnjZwQy7NlzXbo2+W5\
                        qfHxR7ANV5RBkRkdGamkwUcDNL+DpHObFPJHa0oTeYKE0Zbl1MvvfS8RtGGdhGYG\
                        CJf+BPd/gBCs4+dkZdjvOzNyuVuDPGlqQ5f7HS7iuQ/cCyGHcHYJ0nXVewF2Lk+J\
                        lrWykHpTlLwPXmCpNR+gieItPi/UMF2RYTGwojW+yIVwNyMYnjFGUxEX5/DtJjRZ\
                        mg7PBHMrENN2DgO6wBelp4ptyH2KK2EsWT+8jFCuoKv+eJby0QD55LN5f8SrUPRn\
                        K86fh7aVOfCglQofo5ABZIGiDIrg4JsV4k6p0oBSIFOAcqRAhiH+1spCMIIGcTCC\
                        BFmgAwIBAgIKYQmBKgAAAAAAAjANBgkqhkiG9w0BAQsFADCBiDELMAkGA1UEBhMC\
                        VVMxEzARBgNVBAgTCldhc2hpbmd0b24xEDAOBgNVBAcTB1JlZG1vbmQxHjAcBgNV\
                        BAoTFU1pY3Jvc29mdCBDb3Jwb3JhdGlvbjEyMDAGA1UEAxMpTWljcm9zb2Z0IFJv\
                        b3QgQ2VydGlmaWNhdGUgQXV0aG9yaXR5IDIwMTAwHhcNMTAwNzAxMjEzNjU1WhcN\
                        MjUwNzAxMjE0NjU1WjB8MQswCQYDVQQGEwJVUzETMBEGA1UECBMKV2FzaGluZ3Rv\
                        bjEQMA4GA1UEBxMHUmVkbW9uZDEeMBwGA1UEChMVTWljcm9zb2Z0IENvcnBvcmF0\
                        aW9uMSYwJAYDVQQDEx1NaWNyb3NvZnQgVGltZS1TdGFtcCBQQ0EgMjAxMDCCASIw\
                        DQYJKoZIhvcNAQEBBQADggEPADCCAQoCggEBAKkdDbx3EYo6IOz8E5f1+n9plGt0\
                        VBDVpQoAgoX77XxoSyxfxcPlYcJ2tz5mK1vwFVMnBDEfQRsalR3OCROOfGEwWbEw\
                        RA/xYIiEVEMM1024OAizQt2TrNZzMFcmgqNFDdDq9UeBzb8kYDJYYEbyWEeGMoQe\
                        dGFnkV+BVLHPk0ySwcSmXdFhE24oxhr5hoC732H8RsEnHSRnEnIaIYqvS2SJUGKx\
                        Xf13Hz3wV3WsvYpCTUBR0Q+cBj5nf/VmwAOWRH7v0Ev9buWayrGo8noqCjHw2k4G\
                        kbaICDXoeByw6ZnNPOcvRLqn9NxkvaQBwSAJk3jN/LzAyURdXhacAQVPIk0CAwEA\
                        AaOCAeYwggHiMBAGCSsGAQQBgjcVAQQDAgEAMB0GA1UdDgQWBBTVYzpcijGQ80N7\
                        fEYbxTNoWoVtVTAZBgkrBgEEAYI3FAIEDB4KAFMAdQBiAEMAQTALBgNVHQ8EBAMC\
                        AYYwDwYDVR0TAQH/BAUwAwEB/zAfBgNVHSMEGDAWgBTV9lbLj+iiXGJo0T2UkFvX\
                        zpoYxDBWBgNVHR8ETzBNMEugSaBHhkVodHRwOi8vY3JsLm1pY3Jvc29mdC5jb20v\
                        cGtpL2NybC9wcm9kdWN0cy9NaWNSb29DZXJBdXRfMjAxMC0wNi0yMy5jcmwwWgYI\
                        KwYBBQUHAQEETjBMMEoGCCsGAQUFBzAChj5odHRwOi8vd3d3Lm1pY3Jvc29mdC5j\
                        b20vcGtpL2NlcnRzL01pY1Jvb0NlckF1dF8yMDEwLTA2LTIzLmNydDCBoAYDVR0g\
                        AQH/BIGVMIGSMIGPBgkrBgEEAYI3LgMwgYEwPQYIKwYBBQUHAgEWMWh0dHA6Ly93\
                        d3cubWljcm9zb2Z0LmNvbS9QS0kvZG9jcy9DUFMvZGVmYXVsdC5odG0wQAYIKwYB\
                        BQUHAgIwNB4yIB0ATABlAGcAYQBsAF8AUABvAGwAaQBjAHkAXwBTAHQAYQB0AGUA\
                        bQBlAG4AdAAuIB0wDQYJKoZIhvcNAQELBQADggIBAAfmiFEN4sbgmD+BcQM9naOh\
                        IW+z66bM9TG+zwXiqf76V20ZMLPCxWbJat/15/B4vceoniXj+bzta1RXCCtRgkQS\
                        +7lTjMz0YBKKdsxAQEGb3FwX/1z5Xhc1mCRWS3TvQhDIr79/xn/yN31aPxzymXlK\
                        kVIArzgPF/UveYFl2am1a+THzvbKegBvSzBEJCI8z+0DpZaPWSm8tv0E4XCfMkon\
                        /VWvL/625Y4zu2JfmttXQOnxzplmkIz/amJ/3cVKC5Em4jnsGUpxY517IW3DnKOi\
                        PPp/fZZqkHimbdLhnPkd/DjYlPTGpQqWhqS9nhquBEKDuLWAmyI4ILUl5WTs9/S/\
                        fmNZJQ96LjlXdqJxqgaKD4kWumGnEcua2A5HmoDF0M2n0O99g/DhO3EJ3110mCII\
                        YdqwUB5vvfHhAN/nMQekkzr3ZUd46PioSKv33nJ+YWtvd6mBy6cJrDm77MbL2IK0\
                        cs0d9LiFAR6A+xuJKlQ5slvayA1VmXqHczsI5pgt6o3gMy4SKfXAL1QnIffIrE7a\
                        KLixqduWsqdCosnPGUFN4Ib5KpqjEWYw07t0MkvfY3v1mYovG8chr1m1rtxEPJdQ\
                        cdeh0sVV42neV8HR3jDA/czmTfsNv11P6Z0eGTgvvM9YBS7vDaBQNdrvCScc1bN+\
                        NR4Iuto229Nfj950iEkSoYIC0jCCAjsCAQEwgfyhgdSkgdEwgc4xCzAJBgNVBAYT\
                        AlVTMRMwEQYDVQQIEwpXYXNoaW5ndG9uMRAwDgYDVQQHEwdSZWRtb25kMR4wHAYD\
                        VQQKExVNaWNyb3NvZnQgQ29ycG9yYXRpb24xKTAnBgNVBAsTIE1pY3Jvc29mdCBP\
                        cGVyYXRpb25zIFB1ZXJ0byBSaWNvMSYwJAYDVQQLEx1UaGFsZXMgVFNTIEVTTjow\
                        QTU2LUUzMjktNEQ0RDElMCMGA1UEAxMcTWljcm9zb2Z0IFRpbWUtU3RhbXAgU2Vy\
                        dmljZaIjCgEBMAcGBSsOAwIaAxUAs5W4TmyDHMRM7iz6mgGojqvXHzOggYMwgYCk\
                        fjB8MQswCQYDVQQGEwJVUzETMBEGA1UECBMKV2FzaGluZ3RvbjEQMA4GA1UEBxMH\
                        UmVkbW9uZDEeMBwGA1UEChMVTWljcm9zb2Z0IENvcnBvcmF0aW9uMSYwJAYDVQQD\
                        Ex1NaWNyb3NvZnQgVGltZS1TdGFtcCBQQ0EgMjAxMDANBgkqhkiG9w0BAQUFAAIF\
                        AOMUsu8wIhgPMjAyMDA5MjIyMTI5MTlaGA8yMDIwMDkyMzIxMjkxOVowdzA9Bgor\
                        BgEEAYRZCgQBMS8wLTAKAgUA4xSy7wIBADAKAgEAAgIVPgIB/zAHAgEAAgIRtjAK\
                        AgUA4xYEbwIBADA2BgorBgEEAYRZCgQCMSgwJjAMBgorBgEEAYRZCgMCoAowCAIB\
                        AAIDB6EgoQowCAIBAAIDAYagMA0GCSqGSIb3DQEBBQUAA4GBAEMD4esQRMLwQdhk\
                        Co1zgvmclcwl3lYYpk1oMh1ndsU3+97Rt6FV3adS4Hezc/K94oQKjcxtMVzLzQhG\
                        agM6XlqB31VD8n2nxVuaWD1yp2jm/0IvfL9nFMHJRhgANMiBdHqvqNrd86c/Kryq\
                        sI0Ch0sOx9wg3BozzqQhmdNjf9c6MYIDDTCCAwkCAQEwgZMwfDELMAkGA1UEBhMC\
                        VVMxEzARBgNVBAgTCldhc2hpbmd0b24xEDAOBgNVBAcTB1JlZG1vbmQxHjAcBgNV\
                        BAoTFU1pY3Jvc29mdCBDb3Jwb3JhdGlvbjEmMCQGA1UEAxMdTWljcm9zb2Z0IFRp\
                        bWUtU3RhbXAgUENBIDIwMTACEzMAAAEnL26j75GoGagAAAAAAScwDQYJYIZIAWUD\
                        BAIBBQCgggFKMBoGCSqGSIb3DQEJAzENBgsqhkiG9w0BCRABBDAvBgkqhkiG9w0B\
                        CQQxIgQgcyC5Zi6T5dXlcj+V9kHGOarq/wFRtxNkp+J8JwTtAV0wgfoGCyqGSIb3\
                        DQEJEAIvMYHqMIHnMIHkMIG9BCAbkuhLEoYdahb/BUyVszO2VDi6kB3MSaof/+8u\
                        7SM+IjCBmDCBgKR+MHwxCzAJBgNVBAYTAlVTMRMwEQYDVQQIEwpXYXNoaW5ndG9u\
                        MRAwDgYDVQQHEwdSZWRtb25kMR4wHAYDVQQKExVNaWNyb3NvZnQgQ29ycG9yYXRp\
                        b24xJjAkBgNVBAMTHU1pY3Jvc29mdCBUaW1lLVN0YW1wIFBDQSAyMDEwAhMzAAAB\
                        Jy9uo++RqBmoAAAAAAEnMCIEIK4r6N3NISekswMCG1kSBJCCCePrlLDQWbMKz0wt\
                        Lj6CMA0GCSqGSIb3DQEBCwUABIIBAASNHnbCvOgFNv5zwj0UKuGscSrC0R2GxT2p\
                        H6E/QlYix36uklxd1YSqolAA30q/2BQg23N75wfA8chIgOMnaRslF9uk/oKxKHAK\
                        WezF5wx3Qoc08MJmgBQ+f/vkMUr05JIoSjgCVhlnQbO7S+aqV9ZFPDcO6IzlrmiA\
                        okZONeswosfnv1puWHRUhFJx6v3L1y+YKrRfhytDIIw1biSQ/VTO8Wnf06H0miJC\
                        1VLKNa5p8Uwx4tsWz6RvIhztN/wvOo5yUoXR55DLKUMAp283TM4A3n6exf7iEb5N\
                        4jvlHkA6au1Uan+buR92YRqCvyUjqSzSJZo7w3NwLUM6GdFUIY0=\
                        -----END PKCS7-----";

        let authenticode_signature = AuthenticodeSignature::from_pem_str(pkcs7).unwrap();
        let file_hash = authenticode_signature.file_hash().expect("File hash should be present");
        let ctl = CertificateTrustList::fetch().unwrap();

        authenticode_signature
            .authenticode_verifier()
            .require_basic_authenticode_validation(file_hash)
            .require_signing_certificate_check()
            .require_chain_check()
            .exact_date(&UtcDate::new(2021, 3, 3, 18, 39, 47).unwrap())
            .require_not_after_check()
            .require_not_before_check()
            .ctl(&ctl)
            .require_ca_against_ctl_check()
            .verify()
            .unwrap();
    }

    #[cfg(feature = "ctl_http_fetch")]
    #[test]
    fn full_validation_self_signed_authenticode_signature_with_only_leaf_certificate() {
        use ctl::http_fetch::CtlHttpFetch;

        let pkcs7 = "-----BEGIN PKCS7-----\
                            MIIDpQYJKoZIhvcNAQcCoIIDljCCA5ICAQExADALBgkqhkiG9w0BBwGgggN4MIID\
                            dDCCAlygAwIBAgIUcSw2pEU1K7Rx7HKPuFsl13pPmqswDQYJKoZIhvcNAQELBQAw\
                            YTELMAkGA1UEBhMCTGYxCzAJBgNVBAgMAkxmMQswCQYDVQQHDAJMZjELMAkGA1UE\
                            CgwCTGYxCzAJBgNVBAsMAkxmMQswCQYDVQQDDAJMZjERMA8GCSqGSIb3DQEJARYC\
                            TGYwHhcNMjEwNzA5MDc0ODAzWhcNMjIwNzA5MDc0ODAzWjBhMQswCQYDVQQGEwJM\
                            ZjELMAkGA1UECAwCTGYxCzAJBgNVBAcMAkxmMQswCQYDVQQKDAJMZjELMAkGA1UE\
                            CwwCTGYxCzAJBgNVBAMMAkxmMREwDwYJKoZIhvcNAQkBFgJMZjCCASIwDQYJKoZI\
                            hvcNAQEBBQADggEPADCCAQoCggEBAL3QP2wc675k7OgE/hvPuXebUThBha9LWKyw\
                            3n9tdVEWDbFR7jmmE5W4AHOoqp6ha6YspGY+muP0cprfuIIts4/A7BS/3y7B+a1M\
                            BjAa1mOqlR4WPluKSGQIjbaiyexv2ApazXeurcf2Zg+enFhZBi0WnPDKOp8e8jJv\
                            DUBOBSplrPCVBTmZGaYclzxlBlkuMAlHgEuektG3keaPTCPwuCBEwaClBOH6PV1Z\
                            EUBHUv075vISgr0XOdRblZDd5p+71+XMany2+SxKyNLUnDlUgjFoZOWvlTpi/MX8\
                            VRV47AzsTRUTqPzPNZNpeyzUW9/LAtZ8zRE3z+4h2bLQ8uJTeoECAwEAAaMkMCIw\
                            CwYDVR0PBAQDAgG2MBMGA1UdJQQMMAoGCCsGAQUFBwMDMA0GCSqGSIb3DQEBCwUA\
                            A4IBAQChcBmq/tk4rQOaRprgAORssL9JiZ1Gn4QcKHgeZBhif1I6vebtmjVIneMc\
                            a9wWMavWohlUbTHF4GrMbUIyos31XJz0V03Uhdrssm4BZIZuSzv+P3/Vl3w4KNaA\
                            QKSKxKN9dBf4Vok/k4K2tXWuccyI4GzmMAjgvhhSGYuICsHDr2ra8hYZWE8TLGDD\
                            d3V21Ep/DwQt2dZacmkv1ElUbTWBkucY+jF3Icatri/LccADIbtFta80ImTP+9rR\
                            uH/s5ZIoC65hhTv5KmYMqrlmnbuh/UcRe7+z7bV4ccZFkaxh/CbWvmcvvWOQGcUk\
                            25zo8SQ4e71ceN4HQ8x8anHoAT23oQAxAA==\
                            -----END PKCS7-----";

        let private_key = "-----BEGIN RSA PRIVATE KEY-----\
                                MIIEowIBAAKCAQEAvdA/bBzrvmTs6AT+G8+5d5tROEGFr0tYrLDef211URYNsVHu\
                                OaYTlbgAc6iqnqFrpiykZj6a4/Rymt+4gi2zj8DsFL/fLsH5rUwGMBrWY6qVHhY+\
                                W4pIZAiNtqLJ7G/YClrNd66tx/ZmD56cWFkGLRac8Mo6nx7yMm8NQE4FKmWs8JUF\
                                OZkZphyXPGUGWS4wCUeAS56S0beR5o9MI/C4IETBoKUE4fo9XVkRQEdS/Tvm8hKC\
                                vRc51FuVkN3mn7vX5cxqfLb5LErI0tScOVSCMWhk5a+VOmL8xfxVFXjsDOxNFROo\
                                /M81k2l7LNRb38sC1nzNETfP7iHZstDy4lN6gQIDAQABAoIBAFHFSd1AZEqkXe7i\
                                X7oJdePR9F5g07+dnPjgRSnuNLEW6BUwr4kEQ8GnAALTcZVfAuoWp0goxj9Xyptv\
                                r6PdHlLakJmrwvD4vZ/rdWr51Mwg65aHjJuQ6fi2Op6oaIbD8/UaAxQBG3pear9l\
                                3AKvb1qzOC7/X9u20C3r63B9a/pEDxVHLiZ64EEX0io2P81byOzs0LEdD7/6BaFK\
                                md8IWUuzm2KMmV2fNqPMi0B0r1ipPXGewuLhonpcfcLGh/tWflKgeiTYPjQC+tcS\
                                7/qzSXckrfF3PzeAJQs+7FUqvsmvwtt35mOBiFo6GBEBtItkIQ7CVgtt6RupXDRZ\
                                q3qWhcECgYEA4Bm6KsfGnyq5a0Jzyi9420+IBIoP8lLjlu1YFrg6aGuybgc089CH\
                                OCPFzYV5faPHUGbBHdUaHXeCslBnFk8xwm5lhXtlykQQyOHTjIKm9mKGRqMDCeAz\
                                ecL4GCk0bdTmSAT4Wl+1WVeZ2Wo+EZejENIvlFKvVUIMcrMQ8afzmZkCgYEA2NUX\
                                6xrBuWtKxOExkW+0DxcuTLS6n4sB9KaSwG+6Hd7V7FtDQZ9CG8Qv3gL2h5o1odNI\
                                uJxTWPzE8DTrLYv9c5XAmbcwBfcrAO3lYx5suxc7E+UkWtW8GM+Rgw73wYpXbAe/\
                                rFb5dGVallDt4qZOS+KFobqSMc94LL5bZ0wliSkCgYAT5dTk1YYqPcXm4yiazCpD\
                                9sTR+lw+HOP+U6adpc/x05YtNNCb0WkgL/TxMae+4xrgZa9B8dj2wtTE9mSg03lM\
                                lTbIalN4aSDAZWS+Nh+TAt5/SRwM9W48onYa1xXDpsKnpGFUzOiyPRf4+Pj34Onm\
                                pXL6DXlp7YpjaMjZXBtCCQKBgFVj7exveBUeNK6+BHhC5kT/GwOoNMp5wsZnBunz\
                                1fbHd7WB50WjgzROGY+z2QRj7XUSMNRK8+Paf3AdVvRz6dcoBVZDtwzSXsQZ67kS\
                                FT3Ek0ZtedivzUh0Ddjv/w/f/DeWAZzMD6cP9xG1Q0l7tt/ZkEi1obct/iSYvoQ6\
                                j5mpAoGBAJFJkvjT4AYdgNvM150EyQmunujN7/pftL3gemexNYMMaDJC+xGBWr0o\
                                xWsUrwNtiE7tqvOYYzPEdQ+cYt43aoWSe8w/WuveFVd/Cc0yX22PdyvZbJMBFsn/\
                                y+mdkJ6H9jzIzXBDWTwRR8qCy2CPYMtPmnuE9x8UMv7m3uHoXnda\
                                -----END RSA PRIVATE KEY-----";

        let pkcs7 = Pkcs7::from_pem_str(pkcs7).unwrap();
        let private_key = PrivateKey::from_pem_str(private_key).unwrap();

        let authenticode_signature = AuthenticodeSignature::new(
            &pkcs7,
            FILE_HASH.to_vec(),
            ShaVariant::SHA2_256,
            &private_key,
            Some("validate_self_signed_authenticode_signature_with_only_leaf_certificate".to_string()),
        )
        .unwrap();

        let file_hash = authenticode_signature.file_hash().expect("File hash should be present");
        let certificates = authenticode_signature.0.decode_certificates();
        let ca_name = authenticode_signature
            .signing_certificate(&certificates)
            .unwrap()
            .issuer_name();
        let ctl = CertificateTrustList::fetch().unwrap();

        let validator = authenticode_signature.authenticode_verifier();
        validator
            .require_basic_authenticode_validation(file_hash)
            .require_signing_certificate_check()
            .require_chain_check()
            .interval_date(
                &UtcDate::new(2021, 7, 9, 7, 48, 3).unwrap(),
                &UtcDate::new(2022, 7, 9, 7, 48, 3).unwrap(),
            )
            .require_not_before_check()
            .ctl(&ctl)
            .require_ca_against_ctl_check()
            .exclude_cert_authorities(&[ca_name])
            .verify()
            .unwrap();
    }

    #[cfg(feature = "ctl_http_fetch")]
    #[test]
    fn full_validation_self_signed_authenticode_signature_with_root_and_leaf_certificate() {
        use ctl::http_fetch::CtlHttpFetch;

        let pkcs7 = "-----BEGIN PKCS7-----\
                          MIIG6wYJKoZIhvcNAQcCoIIG3DCCBtgCAQExADALBgkqhkiG9w0BBwGggga+MIID\
                          VTCCAj0CFCjdwv1V0L2iCEmXLgUMcYo5o9d2MA0GCSqGSIb3DQEBCwUAMG0xCzAJ\
                          BgNVBAYTAlJ0MQ0wCwYDVQQIDARSb290MQ0wCwYDVQQHDARSb290MQ0wCwYDVQQK\
                          DARSb290MQ0wCwYDVQQLDARSb290MQ0wCwYDVQQDDARSb290MRMwEQYJKoZIhvcN\
                          AQkBFgRSb290MB4XDTIxMDcwOTA5NDIyNloXDTMxMDcwNzA5NDIyNlowYTELMAkG\
                          A1UEBhMCTGYxCzAJBgNVBAgMAkxmMQswCQYDVQQHDAJMZjELMAkGA1UECgwCTGYx\
                          CzAJBgNVBAsMAkxmMQswCQYDVQQDDAJMZjERMA8GCSqGSIb3DQEJARYCTGYwggEi\
                          MA0GCSqGSIb3DQEBAQUAA4IBDwAwggEKAoIBAQDjyUE74hwG4xhqw8g13ARxMY2t\
                          2e9js4kSQGBWkRbs+qLPPl1XYe8Wj54WsFNi4sqH8V0jJaLAXs2heLf+JLWW2/y3\
                          dcA6tgyNO3rCD8CXIuHhisf5qspHJny0wVOt+7ZaGJmFwA27X+E0eDf74lj+WnUe\
                          Bk6uIckvvoPj81N8mFkfF4r5FRjVA4Pvj8+PJvqJlE952386PSIMdwQpLTAX/REq\
                          617SDfBdsVt0L7bG05ayJ8JOMrOIZZccMV4FsTzmo8QtiyGmNJnvyfFdxKsZj7//\
                          lsLO3j7PKyQLGhepf2SfJf5/GxUThbu4H3Q2adfCH86n9gWUNqBVmB+b/fzvAgMB\
                          AAEwDQYJKoZIhvcNAQELBQADggEBAAVmpeUsWP1OpBRXROmCwGsKuoLCE00aaJ2P\
                          8dlbg0De6NVqRlHY6UNgEri75AAC/JiNNmrDVrJefASCsPQkrQRz3/bcVqJ2F8dz\
                          cm7SktKC1jExPxUa9WbrzYIkNP4JIC9gvhBi5zw4IbocHJkVm8F6xL5XPxubsx0v\
                          uw5yvkoimij7gNyp92aBda3VeAfBpLg6ZEo24O42x3vCnxvkMSUCAipXX8cVmjOP\
                          8R75NFhtkyMnxkW3wSJjv3uFmBRI7ngRvuFxBaXImrGL/ekBf1lFnTvPdrXG2NWs\
                          x9QKqzkKS51AaJFeLc9E0aLcBlYOlpvBBZjIQAgkpzMjUDaSXhswggNhMIICSQIU\
                          SYIrOYeD84KqPtPl2sHKzaRitRUwDQYJKoZIhvcNAQELBQAwbTELMAkGA1UEBhMC\
                          UnQxDTALBgNVBAgMBFJvb3QxDTALBgNVBAcMBFJvb3QxDTALBgNVBAoMBFJvb3Qx\
                          DTALBgNVBAsMBFJvb3QxDTALBgNVBAMMBFJvb3QxEzARBgkqhkiG9w0BCQEWBFJv\
                          b3QwHhcNMjEwNzA5MDk0MTU2WhcNNDgxMTI0MDk0MTU2WjBtMQswCQYDVQQGEwJS\
                          dDENMAsGA1UECAwEUm9vdDENMAsGA1UEBwwEUm9vdDENMAsGA1UECgwEUm9vdDEN\
                          MAsGA1UECwwEUm9vdDENMAsGA1UEAwwEUm9vdDETMBEGCSqGSIb3DQEJARYEUm9v\
                          dDCCASIwDQYJKoZIhvcNAQEBBQADggEPADCCAQoCggEBANzVvKN47S4eTbOBLZ3F\
                          a5OlnjGmUxcITrCwyI4xjH6zHr0bES+70UuSA0oIpSF7NFHlBEO0sDuZHYH+5PpO\
                          XvR+LCYWNsDtm6AbycKY0c2Wer02UcWa7nT7HgcLPvaujh4bztBq/lEXoo/TSLmQ\
                          nq/WOl/683bgvq1EL9E3eFl4oP1h27h32Zm8HYOZa3trkiD05HuPynzljSDvx2KD\
                          YZRtItJPdGr99NN8cfrL1vLh+7Q6tBRrrykxBtozy8QoHMjPjSxHhK9FFHxHK/pZ\
                          tVGLe4tvXPwXw652xt36QC8N6sdDccWtpNYj/6LSPPhKeLzcuXUQa2jv9dKMpRjd\
                          u6ECAwEAATANBgkqhkiG9w0BAQsFAAOCAQEAH9lovsbJSYRUOPg7CzaRbtfQi1pw\
                          JOuUVNPK4FPElyrtwSl3WYveRcfeIk8UE8mbG31X4j6GJr05l/nk+9QJTVCzteOV\
                          fdods3zP6R5kwinD6ceNX0bb8rjiaoisoBma1sja0ZtKL/bne+8ftaLCnV6swdtW\
                          BkTdh/lYbNLmFEhHrhbRDxMbIpZUdaXPbL572hgy3SN/31KRVTQu34X8aXvzbyoL\
                          hKaaOIlUSmd8PLCDSyqMdgtO+REqGk9zfzsLfTetJc3aB+V396zG1fSU2/wQ9V60\
                          j7JgC7NMr5Tf1k4B9kepkd/dMWc5Ht/hZKmURAUvsIhkuux5ybfMaFlVn6EAMQA=\
                          -----END PKCS7-----";

        let private_key = "-----BEGIN PRIVATE KEY-----\
                                MIIEvQIBADANBgkqhkiG9w0BAQEFAASCBKcwggSjAgEAAoIBAQDjyUE74hwG4xhq\
                                w8g13ARxMY2t2e9js4kSQGBWkRbs+qLPPl1XYe8Wj54WsFNi4sqH8V0jJaLAXs2h\
                                eLf+JLWW2/y3dcA6tgyNO3rCD8CXIuHhisf5qspHJny0wVOt+7ZaGJmFwA27X+E0\
                                eDf74lj+WnUeBk6uIckvvoPj81N8mFkfF4r5FRjVA4Pvj8+PJvqJlE952386PSIM\
                                dwQpLTAX/REq617SDfBdsVt0L7bG05ayJ8JOMrOIZZccMV4FsTzmo8QtiyGmNJnv\
                                yfFdxKsZj7//lsLO3j7PKyQLGhepf2SfJf5/GxUThbu4H3Q2adfCH86n9gWUNqBV\
                                mB+b/fzvAgMBAAECggEAdCWlrqwvkE9xntbvmo7ycOlMjc4nY5YjGXxb4ygeIX33\
                                UGdDXxAfwkg+2uDT1ANCNCkdTZOeNirg/Sm538vGEANiDAXtm8JCCi2+/X7cu/Pc\
                                a43BRAwTEk6MnfpJ+df0dmI+vdVc6yMLiR6XpUcYC7ICL+oVanLty/t/8taaxldO\
                                BDcGw800d4oghU6Y7FNc+owq5qBiWCIAIrJmAWxbAAKFRrynMwOzZK8gb6+LC8Ta\
                                m1o5Sp/Ns7zSQpem0xZLWzjeVC+1WBRNhAjdcU2Kd8bf1l9T/yvYe0+S7nboEy40\
                                p1G/R53Gmasd2JubRcxjPTjqMV57SPHbwXiL5G1NAQKBgQD6fuszVHnqk6YD3yY1\
                                R/IWm+wW+DMSdnSl0Wup7+/pgiX8ztmtooQbmytgFVghidcD4cqIZ1b+dS20x4lS\
                                xH1FGkZm251Pj1o96bAfIPiDkji95CVdGtnNlrl60DCRFSPMC6othwF8fSJq12gm\
                                KR1Ia1VuhvMzENFtyBhO/JaBgQKBgQDoypcv8jmwCdXdpwW6N2xYJo1zV8BVmDqg\
                                83c18YwOTH/l035QkBxg5y8P12w+Bo6+7E3mUu2R2rdSMKqy9WoA7wM9LsQI5WPb\
                                rDTaOfSY7bsrCWxuJjHXuGymKGml4XoEqGzVuzohMtahLCXvXJkq1yFYVfUFL52y\
                                rTZqD3rWbwKBgQCQnugR6Yq9yOLHR3Vau5/kN781f7SUyzkLZv4ezb0YdqCR9aat\
                                Xa+h9JM1VP1d16QAxMJWwDr0jBiIT89TrseYNtRAnDiVb3EtX5bkUffIlooV7/s2\
                                ZsMqtAOACWSQzsCtFGr6///2rJRLVPP3XDNg1T8sodMVP3d6R1TpfWEzgQKBgHR5\
                                wVHNGc4Z7bcctcHprz0f9RBsLKDnLRaRGumTtScGYcwFmSMIKBrYMXT0rYUPVOb0\
                                ZznB7npW+/iUvyQRpPtYm79GIfHtjJxCOqOh8d9+u3KaIXWviKrN7RbqC4pjGeEw\
                                wFvkdP5daIR2CXkNVNnZkCaZw6HXpEjdX+eLXUPjAoGAZKWs+nC6UXKFum9uAOmt\
                                XthnLdKXQ0z4xodnu97c47Lu2yEB3JItBErLzMiiOOxJrX8zb0OkhbZinz58U6/K\
                                oeKJRUzTxZKls3pWue14bXDvMHuPbkvDWDbFIfe4CpMNY1oJRwETxwLUBN9cmIMT\
                                WdxXlTuB2WQxCuahLyJwWpw=\
                                -----END PRIVATE KEY-----";

        let pkcs7 = Pkcs7::from_pem_str(pkcs7).unwrap();
        let private_key = PrivateKey::from_pem_str(private_key).unwrap();

        let authenticode_signature =
            AuthenticodeSignature::new(&pkcs7, FILE_HASH.to_vec(), ShaVariant::SHA2_256, &private_key, None).unwrap();
        let file_hash = authenticode_signature.file_hash().expect("File hash should be present");

        let certificates = authenticode_signature.0.decode_certificates();
        let ca_name = authenticode_signature
            .signing_certificate(&certificates)
            .unwrap()
            .issuer_name();
        let ctl = CertificateTrustList::fetch().unwrap();

        let validator = authenticode_signature.authenticode_verifier();
        validator
            .require_basic_authenticode_validation(file_hash)
            .require_signing_certificate_check()
            .require_chain_check()
            .exact_date(&UtcDate::new(2021, 9, 7, 12, 50, 40).unwrap())
            .require_not_before_check()
            .require_not_after_check()
            .ctl(&ctl)
            .require_ca_against_ctl_check()
            .exclude_cert_authorities(&[ca_name])
            .verify()
            .unwrap();
    }

    #[test]
    fn validate_authenticode_singature_create_by_authenticode_builder() {
        let parse_key = |pem_str: &str| -> PrivateKey {
            let pem = pem_str.parse::<Pem>().unwrap();
            PrivateKey::from_pkcs8(pem.data()).unwrap()
        };

        // certificates generation code copied from `valid_ca_chain` unit test in certificate.rs :)
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

        let digest_algorithm = AlgorithmIdentifier::new_sha(ShaVariant::SHA2_256);

        let data = SpcAttributeAndOptionalValue {
            ty: oids::spc_pe_image_dataobj().into(),
            value: SpcAttributeAndOptionalValueValue::SpcPeImageData(SpcPeImageData {
                flags: SpcPeImageFlags::default(),
                file: Default::default(),
            }),
        };

        let message_digest = DigestInfo {
            oid: digest_algorithm,
            digest: FILE_HASH.to_vec().into(),
        };

        let mut raw_spc_indirect_data_content = picky_asn1_der::to_vec(&data).unwrap();

        let mut raw_message_digest = picky_asn1_der::to_vec(&message_digest).unwrap();

        raw_spc_indirect_data_content.append(&mut raw_message_digest);

        let message_digest_value = HashAlgorithm::try_from(ShaVariant::SHA2_256)
            .unwrap()
            .digest(raw_spc_indirect_data_content.as_ref());

        let authenticated_attributes = vec![
            Attribute {
                ty: oids::content_type().into(),
                value: AttributeValues::ContentType(Asn1SetOf(vec![oids::spc_indirect_data_objid().into()])),
            },
            Attribute {
                ty: oids::spc_sp_opus_info_objid().into(),
                value: AttributeValues::SpcSpOpusInfo(Asn1SetOf(vec![SpcSpOpusInfo {
                    program_name: None,
                    more_info: Some(ExplicitContextTag1(SpcLink::default())),
                }])),
            },
            Attribute {
                ty: oids::message_digest().into(),
                value: AttributeValues::MessageDigest(Asn1SetOf(vec![message_digest_value.into()])),
            },
        ];

        let content = SpcIndirectDataContent { data, message_digest };

        let content_info = EncapsulatedContentInfo {
            content_type: oids::spc_indirect_data_objid().into(),
            content: Some(ContentValue::SpcIndirectDataContent(content).into()),
        };

        let authenticode_signature = AuthenticodeSignatureBuilder::new()
            .digest_algorithm(HashAlgorithm::SHA2_256)
            .content_info(content_info)
            .issuer_and_serial_number(signed_leaf.issuer_name(), signed_leaf.serial_number().0.clone())
            .authenticated_attributes(authenticated_attributes)
            .signing_key(&leaf_key)
            .certs(vec![signed_leaf, intermediate, root])
            .build()
            .unwrap();

        let validator = authenticode_signature.authenticode_verifier();
        validator
            .require_basic_authenticode_validation(FILE_HASH.to_vec())
            .require_signing_certificate_check()
            .require_chain_check()
            .ignore_not_before_check()
            .ignore_not_after_check()
            .verify()
            .unwrap();
    }

    #[test]
    fn test_timestamped_signature_with_simple_chain_verification() {
        // this signature signed using Root -> End certificates chain
        let signature = "MIINFgYJKoZIhvcNAQcCoIINBzCCDQMCAQExDzANBglghkgBZQMEAgEFADB5BgorBgEEAY\
                            I3AgEEoGswaTA0BgorBgEEAYI3AgEPMCYDAgWAoCCiHoAcADwAPAA8AE8AYgBzAG8AbABl\
                            AHQAZQA+AD4APjAxMA0GCWCGSAFlAwQCAQUABCBrzsxooHhDUS5MX6mQrGeiWzWMUhlXO8\
                            MlVr3wCPo2MaCCCEwwggQUMIIC/KADAgECAhQZvvjV83AsB614bVI6VFZ7XHJahTANBgkq\
                            hkiG9w0BAQsFADCBiDELMAkGA1UEBhMCVUExEjAQBgNVBAgMCVJvb3RTdGF0ZTENMAsGA1\
                            UEBwwEbGFuZDESMBAGA1UECgwJUGFzaGFDb3JwMQ8wDQYDVQQLDAZjb29wZXIxEjAQBgNV\
                            BAMMCXBhc2hhLmNvbTEdMBsGCSqGSIb3DQEJARYOcGFzaGFAY29ycC5jb20wHhcNMjExMT\
                            E1MTg0NDUyWhcNMzExMTEzMTg0NDUyWjCBmDELMAkGA1UEBhMCQ08xGDAWBgNVBAgMD0Jy\
                            aXRpc2hDb2x1bWJpYTENMAsGA1UEBwwEY2l0eTESMBAGA1UECgwJVGVjb21Db3JwMRIwEA\
                            YDVQQLDAlVbml2ZXJzYWwxHTAbBgNVBAMMFHRlY29tLmNvcnBAZ21haWwuY29tMRkwFwYJ\
                            KoZIhvcNAQkBFgp0ZWNvbS5jb3JwMIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQ\
                            EAtLUsSVVvIIf6h4FJHaiw2bCbBuEl0onXPNs6cUoDRBtwDCN+8I3BTk+8IkDBwk360r+9\
                            WRv14dwP54CjirCKqS90xMsC3rpjwHBFEUWfYVS/W2F5cFs5hTH2G/tp158PkGyTzUxluc\
                            LN3xDlV9XB90dzDK3cBJPIz6XmwYCH8ies18WBMwrdWyKYBpavZ0yP4EsjFl0LZgQKoItn\
                            el3GIEvqTJKolAolcunRxxcyiQetBrregVZL12ArKHMXmzW1eqIqJ4BLDVPWWs7OMjEkmv\
                            af/Qg2KIT8f+BVj/6zz9BdQlFMaS/kog91rcqmh5fd+HfXqHDFbm43C1BJiZfXkQIDAQAB\
                            o2QwYjALBgNVHQ8EBAMCAbYwEwYDVR0lBAwwCgYIKwYBBQUHAwMwHQYDVR0OBBYEFAxq+j\
                            nnXAe9nEUPyN7H19UVktTKMB8GA1UdIwQYMBaAFHSMPvti9N00ILGqdSg14i2i8RgPMA0G\
                            CSqGSIb3DQEBCwUAA4IBAQCBm+Q31pBby3insoF/1Le4uvxp3bQniw6PhhNmpVmBcFcVk1\
                            fS0tuZDBWUrWzMnKEGlKTOpR4WSdAgaYqxG8FHS4FotSwzFnJoEadhu9RUYzJ6JPLUKcNe\
                            0zTn0jZ0L6RBUm7J8xSLYCVIkkIn3h67CORkuUDUsiByAqETzVJ0byl1BLEyqeMg/f7HRM\
                            PnnhGTUTV4ay5XJobY53hf/5WlaEFFasqjUVRyL816ZCYM6QPVtzYsCD0HwvJ8qNuXS/Zu\
                            Xf1pD8a4P/g3HPmzC4cA49urEomr6/Ggf+6KmTDE6KCVk9p7MotpxEWJhl8Ijbmm9csrlU\
                            wDu1cO6M3a0/LyMIIEMDCCAhigAwIBAgIFAP0rOYYwDQYJKoZIhvcNAQELBQAwGDEWMBQG\
                            A1UEAwwNU2FzaGEgUm9vdCBDQTAeFw0yMTExMTUxOTE4MTRaFw0yNjExMTQxOTE4MTRaMB\
                            oxGDAWBgNVBAMMD1Nhc2hhIEF1dGhvcml0eTCCASIwDQYJKoZIhvcNAQEBBQADggEPADCC\
                            AQoCggEBANZRqREbdNdmS+InSj455zfF1FGZxveNNV6pPs9Ht3WW5LZI8B56zoy66o24Su\
                            znDFDGZcdxUnLnlM2j/gMNYdPJvsVH6qLR6Nd1VSHyQPZVPd9QBuFaMbr/Gs1Yj6ofYazd\
                            FwF3AwI4Nw2HeIdtm1oxsz/peZCFYThZTZzioEtNnfbnF2s9E+aZXPS0jl+x7kzbBpVzAy\
                            0rvMPymDtaIxYgoI+pGZbqblLTYec/LmXQaBgWictbRAny9cOm6BLMB2f7CWYmJNOJNOH6\
                            kpY0jC7BghcLc5id/2EvNgeb8p87cElR7oIjhXyFU1uV6x7VtGWMWGIfQOSqObeo8JEzqu\
                            8CAwEAAaN/MH0wEgYDVR0TAQH/BAgwBgEB/wIBADAPBgNVHQ8BAf8EBQMDB4YAMCkGA1Ud\
                            DgQiBCCnik1mKZTTk8AlP3iFEAEx0WxqlL9hYQ0UKHI/cGhbXzArBgNVHSMEJDAigCDoFj\
                            JS7DjrCOEXyzdeN8t1verL9hCvsHIP10zAmRQ9nTANBgkqhkiG9w0BAQsFAAOCAgEADZlO\
                            HfkroGZtaDClxC3zUUkdvB24ohFBmYlZtOZATycrbEb6s5/DepL/LHrADiLyDbnd6VxLCL\
                            zqkNB0eiU/37R4PljrODUS10I9byGXclljltusY1A07fWIDUwk1hkh2U/6RntGPIY4vPlX\
                            6ve/F/IDKVT9OynNTarZmYiFOnWSutZZokO8bNAv1U38EglPTBcKd22330l6xEc4vcWRkB\
                            ieiZPiqPRLfpDwk+4akVQbgwi7lkJSZQUB8NvwZj70hER6AWLDXZF+IKRlr5GDl02Q29hE\
                            hfzHJnoJJb7UI7lI6GhZXAIOampmm3vGkMbhSV5kAx8HcaKG390UY9CNMWdynQo9S/XwCH\
                            jcwoEJkleItOOJj9fOszlGyA6JpjERsLYtoGksUdGxOOr3Ozr6SV6gXkaqa9c5RXsBzJQg\
                            oWDK1emBM3+hogwaLplW2jfLsRH+XfXg4QFmCtMHynsahBlgR8J+2Z6yLdEy/pMBeL987x\
                            4wHOyqWMYXH537dpam/3LqzOg1508TkDt1Pr36YbNynPU3BrmN+lB+dN/KFWovAjiaWYSF\
                            K3fT3YTaZrdvy0lE2iG9uyi88ZpvV03wQ1n4fIiE2F4CiU5slrduYRw/8f/qbGvNpoETHd\
                            1dVNQAK55kfz1I1roCGZu4JgmFzz/e9BomqUwLcKxmmbxwj36hADGCBB4wggQaAgEBMIGh\
                            MIGIMQswCQYDVQQGEwJVQTESMBAGA1UECAwJUm9vdFN0YXRlMQ0wCwYDVQQHDARsYW5kMR\
                            IwEAYDVQQKDAlQYXNoYUNvcnAxDzANBgNVBAsMBmNvb3BlcjESMBAGA1UEAwwJcGFzaGEu\
                            Y29tMR0wGwYJKoZIhvcNAQkBFg5wYXNoYUBjb3JwLmNvbQIUGb741fNwLAeteG1SOlRWe1\
                            xyWoUwDQYJYIZIAWUDBAIBBQCggYAwGQYJKoZIhvcNAQkDMQwGCisGAQQBgjcCAQQwMgYK\
                            KwYBBAGCNwIBDDEkMCKhIKIegBwAPAA8ADwATwBiAHMAbwBsAGUAdABlAD4APgA+MC8GCS\
                            qGSIb3DQEJBDEiBCC9eETpakSiPnInPgy2r3fGQFP3ZDSmGN4SW8pZfUEHmTANBgkqhkiG\
                            9w0BAQEFAASCAQBmzok+hjGVQP0DPoSKdW4nXtwyzD42wdnGpnN3gXIxIClGw3WbbFQcRe\
                            J868Ispv3q14CDHOJicSpHnZeIZdrN5G/Mbyrtzhd8DfiP6x1GNnVETAUO33IwMG18qxbN\
                            O+kuD16D59V5WmcRwqTFXmUoLvzJxhlxaE65ZA4UCGbHvxffWuW8PzGz8B7t/wh02Qzdj6\
                            s047TAC0841yWm5xrcU8wJxgDCVjYRB9nCfVcavhHyTAqsiGxXP8zWq84wW//OuJ70e6j6\
                            w8GcSJJRHSByVMBJd0alvqpoZCH3Pk3ZUaOW0M72M0Ru4xObGuhcF4n9LP3BP2ejOo2q6K\
                            2dwmyeoYIByjCCAcYGCSqGSIb3DQEJBjGCAbcwggGzAgEBMCEwGDEWMBQGA1UEAwwNU2Fz\
                            aGEgUm9vdCBDQQIFAP0rOYYwDQYJYIZIAWUDBAIBBQCgaTAYBgkqhkiG9w0BCQMxCwYJKo\
                            ZIhvcNAQcBMBwGCSqGSIb3DQEJBTEPFw0yMTExMTUxOTI0MTFaMC8GCSqGSIb3DQEJBDEi\
                            BCAk+UwrzxzsljUCVqZrEYmhZYYAVU5rPdSMR1dP0jt7djANBgkqhkiG9w0BAQEFAASCAQ\
                            AfCNejQbDkZeEPgmyD9yDzL5/urHpTp/gKIt1SRUealVmvNLg5LeknZJ7BQfd2G53zEA9c\
                            fJ8TvyY6GP1YNROtkrQ0sWucJFNwFPMAlDtosJf+2L7BPvem+OI5/5QpL+wz+Ck/jDEF8m\
                            xHQm8b1Foo7RDc9iJnRaZTK1ES2MJKjxgEAKKVQois5UVTxO2WuG+HZ0x69Ye30u3vTlvq\
                            2/zdX1wrovKdyKa8bF+34x+bnObgiiiNs0vqWXELgiCu2pHtcEHnHv70H62f3jBUKc5gIZ\
                            PHvJPJq54bXy2JxFBRUZFo8r8af75X3+atMp9eSErcSUK2UTygICS2lz/ZRaIZ";

        let der_signature = base64::decode(signature).unwrap();
        let authenticode_signature = AuthenticodeSignature::from_der(&der_signature).unwrap();

        let time = UtcDate::now();
        let validator = authenticode_signature.authenticode_verifier();
        let validator = validator
            .ignore_basic_authenticode_validation()
            .ignore_chain_check()
            .ignore_ca_against_ctl_check()
            .ignore_excluded_cert_authorities()
            .require_signing_certificate_check()
            .require_not_after_check()
            .require_not_before_check()
            .require_chain_check()
            .exact_date(&time);

        validator.verify().unwrap();
    }

    #[test]
    fn test_timestamped_signature_with_full_chain_verification() {
        // this signature signed using Root -> Intermediate -> End certificates chain
        let signature = "MIIQ6wYJKoZIhvcNAQcCoIIQ3DCCENgCAQExDzANBglghkgBZQMEAgEFADB5BgorBgEEAY\
                              I3AgEEoGswaTA0BgorBgEEAYI3AgEPMCYDAgWAoCCiHoAcADwAPAA8AE8AYgBzAG8AbABl\
                              AHQAZQA+AD4APjAxMA0GCWCGSAFlAwQCAQUABCBrzsxooHhDUS5MX6mQrGeiWzWMUhlXO8\
                              MlVr3wCPo2MaCCDDcwggQfMIIDB6ADAgECAgEDMA0GCSqGSIb3DQEBCwUAMIGFMQswCQYD\
                              VQQGEwJJVDENMAsGA1UECAwEa3J1czENMAsGA1UEBwwEbGFrZTESMBAGA1UECgwJS3J1c0\
                              l0YWx5MQ8wDQYDVQQLDAZsYXBzaGExEzARBgNVBAMMCmxhcHNoYS5jb20xHjAcBgkqhkiG\
                              9w0BCQEWD2xhcHNoYUBrcnVzLmNvbTAeFw0yMTExMTYwOTU4NTlaFw0yMjExMTYwOTU4NT\
                              laMIGJMQswCQYDVQQGEwJFTjEPMA0GA1UECAwGTG9uZG9uMQ0wCwYDVQQHDARjaXR5MQ8w\
                              DQYDVQQKDAZCcmlja0IxDTALBgNVBAsMBHBvcnQxGTAXBgNVBAMMEGJyaWNrLmJAcG9ydC\
                              5jb20xHzAdBgkqhkiG9w0BCQEWEGJyaWNrLmJAcG9ydC5jb20wggEiMA0GCSqGSIb3DQEB\
                              AQUAA4IBDwAwggEKAoIBAQDPTE55lW0MDHc1sfeckJXGk15sku6Wa6NFeD7QnJX2HB0TSl\
                              2Zx8AJ9G6G2rbfVUFv4aQlHB7rKki+CHLKuYYa4qoqUGfI9+oj4XtjEgky/ksgZgR+CR/J\
                              kZpauYXVgUXxMLosoYwFhtPNvLrITYC9vjpub39tDN/CQSQUbm7uR3i3KNCG1VrpmjMccu\
                              j3G7KU0/n9Y9QWma2kDF/79hbWjIpP2KCJLR61JDNxRbQS64wOfAojtOJiluo6ZLI1rPOn\
                              jeugUKjAlL+0dJUt40Wh2PHI6ysg4s6Iw7IIK/zW7n+xI2WsF0bixKRaxTHOEnOewN0jix\
                              B/yR9CsHUwU+kbAgMBAAGjgZMwgZAwCwYDVR0PBAQDAgG2MBMGA1UdJQQMMAoGCCsGAQUF\
                              BwMDMCwGCWCGSAGG+EIBDQQfFh1PcGVuU1NMIEdlbmVyYXRlZCBDZXJ0aWZpY2F0ZTAdBg\
                              NVHQ4EFgQUoljjRcajmZI7KTF21LizYPPCq2owHwYDVR0jBBgwFoAUfqsVmxAp5WyQpfDX\
                              Ipg8KLMitQIwDQYJKoZIhvcNAQELBQADggEBAA64XnyVCad0+eC9Z7H0UISm4onEEwbTgf\
                              O6H1pimhLKz14WJ1bXYqLRK3GvlBsU3XWS/1HLSeYhW0bahOMnb5d+ASOullFs+5Kmmu8f\
                              g3HbbTrUkUMS3+fxzR8tkIYpQ/JXK+nA1r9SXVDP5DJUQ245V+QB5hdQITrAumppwxdmWW\
                              NbpM4SjFPvUn4BjRKiuvYzWgEzKyCbEIidqrVOUEwpApVbZpltJI7HyP1LwW0ohZ3KThkp\
                              iKrmSOKFaa4iZ1P4/MuEG5J8FOzYG8JPkcpZtGPwaaqPkdGaxQ4U0gGctl6pct16XGFAu0\
                              4cOw//uXVzHD2STWRzZaolGf8P4XQwggPcMIICxKADAgECAgECMA0GCSqGSIb3DQEBCwUA\
                              MIGHMQswCQYDVQQGEwJVQTESMBAGA1UECAwJUm9vdFN0YXRlMQ0wCwYDVQQHDARsYW5kMR\
                              IwEAYDVQQKDAlQYXNoYUNvcnAxDjAMBgNVBAsMBWNvb3JwMRIwEAYDVQQDDAlwYXNoYS5j\
                              b20xHTAbBgkqhkiG9w0BCQEWDnBhc2hhQGxhbmQuY29tMB4XDTIxMTExNjA5NTU0NFoXDT\
                              IyMTExNjA5NTU0NFowgYUxCzAJBgNVBAYTAklUMQ0wCwYDVQQIDARrcnVzMQ0wCwYDVQQH\
                              DARsYWtlMRIwEAYDVQQKDAlLcnVzSXRhbHkxDzANBgNVBAsMBmxhcHNoYTETMBEGA1UEAw\
                              wKbGFwc2hhLmNvbTEeMBwGCSqGSIb3DQEJARYPbGFwc2hhQGtydXMuY29tMIIBIjANBgkq\
                              hkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEAvC8zMDWPNgpGGpyrVcOXCdYLwGHndS0LDdhfn1\
                              3HaZ+5F2kQ1aXKDLEXbOR4EN5Z563QMqyBYuWWfX0IIuIjxSHT3ablyHpEjw7/AvWN28Vh\
                              5bbkXaPWaxQ0/QaMXtQg2Mmt/32mWtniwhFhuLSKIE845fX+UVlSWqH7NfdbrNwJZUwNf/\
                              21Le7VuNaK2ju9ozZ7FtunWjUQs+IZAsxXxwh18aIk24MY2zYHKR4ZPRpqvNXuNgSCjkgF\
                              swT94KPhQqYB9Fqm52xpz43Acwb0rq91V1+LPULEiiVaXeXOyTX1935wwWBwKoJdpMjm5P\
                              wnEnuEB1LejDNSoCnvNZ5vfQIDAQABo1MwUTAdBgNVHQ4EFgQUfqsVmxAp5WyQpfDXIpg8\
                              KLMitQIwHwYDVR0jBBgwFoAUQQZbLPOL+TzMrd5xBN+QW1lSncYwDwYDVR0TAQH/BAUwAw\
                              EB/zANBgkqhkiG9w0BAQsFAAOCAQEAQl7N6B9BOHbUuvHnohDuAcCXxkiyLgGywKyDDTLf\
                              h+5eDrMJwuRjMNYw+X4GLc2675Gu1dPcsGr2XRwRE60wSjPnoZ4+4USZFV5H77frjuxddD\
                              1qG0DXvnBilmFUOOzOLYc+Z2gWMba6Yh081LfCVpcIT8ll+yCFLpd49YXFiCyEUD0pSX6C\
                              iFqsND+QVEy3bdhmO9uNp1pWBO3NyKhDrE17IMWPJF7tmCJ2tDE6uuHiJcWiTo5XWC3qge\
                              UksluqYtfuk2Ys/FMwb58WkNqcxC1YZyxSFSrFtWSdJ783boduS7mlhx98M64n1eJ8OHsg\
                              xeNa//yh0gewSxmxqFwewTCCBDAwggIYoAMCAQICBQD9KzmGMA0GCSqGSIb3DQEBCwUAMB\
                              gxFjAUBgNVBAMMDVNhc2hhIFJvb3QgQ0EwHhcNMjExMTE1MTkxODE0WhcNMjYxMTE0MTkx\
                              ODE0WjAaMRgwFgYDVQQDDA9TYXNoYSBBdXRob3JpdHkwggEiMA0GCSqGSIb3DQEBAQUAA4\
                              IBDwAwggEKAoIBAQDWUakRG3TXZkviJ0o+Oec3xdRRmcb3jTVeqT7PR7d1luS2SPAees6M\
                              uuqNuErs5wxQxmXHcVJy55TNo/4DDWHTyb7FR+qi0ejXdVUh8kD2VT3fUAbhWjG6/xrNWI\
                              +qH2Gs3RcBdwMCODcNh3iHbZtaMbM/6XmQhWE4WU2c4qBLTZ325xdrPRPmmVz0tI5fse5M\
                              2waVcwMtK7zD8pg7WiMWIKCPqRmW6m5S02HnPy5l0GgYFonLW0QJ8vXDpugSzAdn+wlmJi\
                              TTiTTh+pKWNIwuwYIXC3OYnf9hLzYHm/KfO3BJUe6CI4V8hVNblese1bRljFhiH0Dkqjm3\
                              qPCRM6rvAgMBAAGjfzB9MBIGA1UdEwEB/wQIMAYBAf8CAQAwDwYDVR0PAQH/BAUDAweGAD\
                              ApBgNVHQ4EIgQgp4pNZimU05PAJT94hRABMdFsapS/YWENFChyP3BoW18wKwYDVR0jBCQw\
                              IoAg6BYyUuw46wjhF8s3XjfLdb3qy/YQr7ByD9dMwJkUPZ0wDQYJKoZIhvcNAQELBQADgg\
                              IBAA2ZTh35K6BmbWgwpcQt81FJHbwduKIRQZmJWbTmQE8nK2xG+rOfw3qS/yx6wA4i8g25\
                              3elcSwi86pDQdHolP9+0eD5Y6zg1EtdCPW8hl3JZY5bbrGNQNO31iA1MJNYZIdlP+kZ7Rj\
                              yGOLz5V+r3vxfyAylU/TspzU2q2ZmIhTp1krrWWaJDvGzQL9VN/BIJT0wXCndtt99JesRH\
                              OL3FkZAYnomT4qj0S36Q8JPuGpFUG4MIu5ZCUmUFAfDb8GY+9IREegFiw12RfiCkZa+Rg5\
                              dNkNvYRIX8xyZ6CSW+1CO5SOhoWVwCDmpqZpt7xpDG4UleZAMfB3Giht/dFGPQjTFncp0K\
                              PUv18Ah43MKBCZJXiLTjiY/XzrM5RsgOiaYxEbC2LaBpLFHRsTjq9zs6+kleoF5GqmvXOU\
                              V7AcyUIKFgytXpgTN/oaIMGi6ZVto3y7ER/l314OEBZgrTB8p7GoQZYEfCftmesi3RMv6T\
                              AXi/fO8eMBzsqljGFx+d+3aWpv9y6szoNedPE5A7dT69+mGzcpz1Nwa5jfpQfnTfyhVqLw\
                              I4mlmEhSt3092E2ma3b8tJRNohvbsovPGab1dN8ENZ+HyIhNheAolObJa3bmEcP/H/6mxr\
                              zaaBEx3dXVTUACueZH89SNa6AhmbuCYJhc8/3vQaJqlMC3CsZpm8cI9+oQAxggQIMIIEBA\
                              IBATCBizCBhTELMAkGA1UEBhMCSVQxDTALBgNVBAgMBGtydXMxDTALBgNVBAcMBGxha2Ux\
                              EjAQBgNVBAoMCUtydXNJdGFseTEPMA0GA1UECwwGbGFwc2hhMRMwEQYDVQQDDApsYXBzaG\
                              EuY29tMR4wHAYJKoZIhvcNAQkBFg9sYXBzaGFAa3J1cy5jb20CAQMwDQYJYIZIAWUDBAIB\
                              BQCggYAwGQYJKoZIhvcNAQkDMQwGCisGAQQBgjcCAQQwMgYKKwYBBAGCNwIBDDEkMCKhIK\
                              IegBwAPAA8ADwATwBiAHMAbwBsAGUAdABlAD4APgA+MC8GCSqGSIb3DQEJBDEiBCC9eETp\
                              akSiPnInPgy2r3fGQFP3ZDSmGN4SW8pZfUEHmTANBgkqhkiG9w0BAQEFAASCAQC40luaPt\
                              aTsPAtQ3wF8LCf9OPWiXWiDmuMVopDgDCoTw3OI8GTWhQ2uEE19GUd++1DdBY96++lWckA\
                              NVd7xz1HUdWzXRV9YyFW1Pg7VFB/qcjeWYUM8mgWH5wEyxEexhGygcQDZ1vCUkQvfodM85\
                              1jU9DEhEQeZkhKniisTDkmZL8Y4Zuee2aQr+/EutRcO9LTUzy4XvyJDmcrXW7vCrhDnBJq\
                              ptrbU+bh2umnXo2dZKTDh1/Y2bu5hKbtpc6laSh6ecHNmxxjguO0ve4NpW3GIhH+4kHjbr\
                              iJxpBkPcHrV4lbOHZJ4Dmd6qGzp+bq8Lc42eGnM4BItBSPZl2xba0uoYIByjCCAcYGCSqG\
                              SIb3DQEJBjGCAbcwggGzAgEBMCEwGDEWMBQGA1UEAwwNU2FzaGEgUm9vdCBDQQIFAP0rOY\
                              YwDQYJYIZIAWUDBAIBBQCgaTAYBgkqhkiG9w0BCQMxCwYJKoZIhvcNAQcBMBwGCSqGSIb3\
                              DQEJBTEPFw0yMTExMTYxMDEzMjZaMC8GCSqGSIb3DQEJBDEiBCDHUYaaNCyO4nNkrkLJ5G\
                              HIdEhgP9x/cOu0Os7BVaPWpjANBgkqhkiG9w0BAQEFAASCAQCU9oBkFiA3J5WvylHzNVGI\
                              NKmllhjrVd4b1Edhd5eIHn4k2SPR2wTEhHOM7Tg5wDvJyEJE5yMqL4LAOl88slk0hrNEQ0\
                              4/sPkZh40JTXwh/J8faKG+zsA0oL2JhhTBRy4BEqVyQ8f3qV/kq1ufxgLr4Cvsp71golwW\
                              DcZnkWMel/+lUY4DcSvByChKcedu+Mc2qMo+nrO569GoTWA2MC8Z2yNK1OQqaBkiinaadA\
                              CmE1KKJ1vU1qzC03SsoPtxHKFKo5cJ88tiehDxCKDNjf1c2JJy9iGEaQZN9tLCPHtz/W6p\
                              Q355Y6GW1xdqI879CvAQX/Dy9+g42WddYjeJE0iD";

        let der_signature = base64::decode(signature).unwrap();
        let authenticode_signature = AuthenticodeSignature::from_der(&der_signature).unwrap();

        let time = UtcDate::now();
        let validator = authenticode_signature.authenticode_verifier();
        let validator = validator
            .ignore_basic_authenticode_validation()
            .ignore_chain_check()
            .ignore_ca_against_ctl_check()
            .ignore_excluded_cert_authorities()
            .require_signing_certificate_check()
            .require_not_after_check()
            .require_not_before_check()
            .require_chain_check()
            .exact_date(&time);

        validator.verify().unwrap();
    }
}
