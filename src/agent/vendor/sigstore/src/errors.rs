//
// Copyright 2021 The Sigstore Authors.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! The errors that can be raised by sigstore-rs

use thiserror::Error;

use crate::cosign::verification_constraint::VerificationConstraintRefVec;

#[derive(Error, Debug)]
#[error("Several Signature Layers failed verification")]
pub struct SigstoreVerifyConstraintsError<'a> {
    pub unsatisfied_constraints: VerificationConstraintRefVec<'a>,
}

pub type Result<T> = std::result::Result<T, SigstoreError>;

#[derive(Error, Debug)]
pub enum SigstoreError {
    #[error("failed to parse URL: {0}")]
    UrlParseError(#[from] url::ParseError),

    #[error("failed to construct redirect URL")]
    RedirectUrlRequestLineError,

    #[error("failed to construct oauth code pair")]
    CodePairError,

    #[error("invalid key format: {error}")]
    InvalidKeyFormat { error: String },

    #[error(transparent)]
    PEMParseError(#[from] x509_parser::nom::Err<x509_parser::error::PEMError>),

    #[error(transparent)]
    FromPEMError(#[from] pem::PemError),

    #[error(transparent)]
    X509ParseError(#[from] x509_parser::nom::Err<x509_parser::error::X509Error>),
    #[error(transparent)]
    X509Error(#[from] x509_parser::error::X509Error),

    #[error(transparent)]
    CertError(#[from] picky::x509::certificate::CertError),

    #[error(transparent)]
    Base64DecodeError(#[from] base64::DecodeError),

    #[error("Public key with unsupported algorithm: {0}")]
    PublicKeyUnsupportedAlgorithmError(String),

    #[error("Public key verification error")]
    PublicKeyVerificationError,

    #[error("Certificate validity check failed: cannot be used before {0}")]
    CertificateValidityError(String),

    #[error("Certificate has not been issued for {0}")]
    CertificateInvalidEmail(String),

    #[error("Certificate expired before signatures were entered in log: {integrated_time} is before {not_before}")]
    CertificateExpiredBeforeSignaturesSubmittedToRekor {
        integrated_time: String,
        not_before: String,
    },

    #[error("Certificate was issued after signatures were entered in log: {integrated_time} is after {not_after}")]
    CertificateIssuedAfterSignaturesSubmittedToRekor {
        integrated_time: String,
        not_after: String,
    },

    #[error("Bundled certificate does not have digital signature key usage")]
    CertificateWithoutDigitalSignatureKeyUsage,

    #[error("Bundled certificate does not have code signing extended key usage")]
    CertificateWithoutCodeSigningKeyUsage,

    #[error("Certificate without Subject Alternative Name")]
    CertificateWithoutSubjectAlternativeName,

    #[error("Certificate with incomplete Subject Alternative Name")]
    CertificateWithIncompleteSubjectAlternativeName,

    #[error("Certificate pool error: {0}")]
    CertificatePoolError(String),

    #[error("Cannot fetch manifest of {image}: {error}")]
    RegistryFetchManifestError { image: String, error: String },

    #[error("Cannot pull manifest of {image}: {error}")]
    RegistryPullManifestError { image: String, error: String },

    #[error("Cannot pull {image}: {error}")]
    RegistryPullError { image: String, error: String },

    #[error("OCI reference not valid: {reference}")]
    OciReferenceNotValidError { reference: String },

    #[error("Layer doesn't have Sigstore media type")]
    SigstoreMediaTypeNotFoundError,

    #[error("Layer digest mismatch")]
    SigstoreLayerDigestMismatchError,

    #[error("Missing signature annotation")]
    SigstoreAnnotationNotFoundError,

    #[error("Rekor bundle missing")]
    SigstoreRekorBundleNotFoundError,

    #[error("Fulcio certificates not provided")]
    SigstoreFulcioCertificatesNotProvidedError,

    #[error("No Signature Layer passed verification")]
    SigstoreNoVerifiedLayer,

    #[error(transparent)]
    TufError(#[from] tough::error::Error),

    #[error("TUF target {0} not found inside of repository")]
    TufTargetNotFoundError(String),

    #[error(transparent)]
    IOError(#[from] std::io::Error),

    #[error("{0}")]
    UnexpectedError(String),

    #[error("{0}")]
    VerificationConstraintError(String),

    #[error("Verification of OIDC claims received from OpenIdProvider failed")]
    ClaimsVerificationError,

    #[error("Failed to access token endpoint")]
    ClaimsAccessPointError,

    #[error("Failed to get id_token")]
    NoIDToken,
}
