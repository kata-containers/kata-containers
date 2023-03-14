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

//! Structs that can be used to verify [`crate::cosign::SignatureLayer`]
//! with special business logic.
//!
//! This module provides already the most common kind of verification constraints:
//! * [`PublicKeyVerifier`]: ensure a signature has been produced by a specific
//!   cosign key
//! * [`CertSubjectEmailVerifier`]: ensure a signature has been produced in keyless mode,
//!   plus the email address associated with the signer matches a specific one
//! * [`CertSubjectUrlVerifier`]: ensure a signature has been produced in keyless mode,
//!   plus the certificate SAN has a specific URI inside of it. This can be used to verify
//!   signatures produced by GitHub Actions.
//!
//! Developers can define ad-hoc validation logic by creating a Struct that implements
//! the [`VerificationConstraintVec`] trait.

use std::collections::HashMap;

use super::signature_layers::{CertificateSubject, SignatureLayer};
use crate::crypto::{CosignVerificationKey, SignatureDigestAlgorithm};
use crate::errors::Result;

/// A list of objects implementing the [`VerificationConstraint`] trait
pub type VerificationConstraintVec = Vec<Box<dyn VerificationConstraint>>;

/// A list of references to objects implementing the [`VerificationConstraint`] trait
pub type VerificationConstraintRefVec<'a> = Vec<&'a Box<dyn VerificationConstraint>>;

/// A trait that can be used to define verification constraints objects
/// that use a custom verification logic.
pub trait VerificationConstraint: std::fmt::Debug {
    /// Given the `signature_layer` object, return `true` if the verification
    /// check is satisfied.
    ///
    /// Developer can use the
    /// [`errors::SigstoreError::VerificationConstraintError`](crate::errors::SigstoreError::VerificationConstraintError)
    /// error when something goes wrong inside of the verification logic.
    ///
    /// ```
    /// use sigstore::{
    ///   cosign::verification_constraint::VerificationConstraint,
    ///   cosign::signature_layers::SignatureLayer,
    ///   errors::{SigstoreError, Result},
    /// };
    ///
    /// #[derive(Debug)]
    /// struct MyVerifier{}
    ///
    /// impl VerificationConstraint for MyVerifier {
    ///   fn verify(&self, _sl: &SignatureLayer) -> Result<bool> {
    ///     Err(SigstoreError::VerificationConstraintError(
    ///         "something went wrong!".to_string()))
    ///   }
    /// }
    fn verify(&self, signature_layer: &SignatureLayer) -> Result<bool>;
}

/// Verification Constraint for signatures produced with public/private keys
#[derive(Debug)]
pub struct PublicKeyVerifier {
    key: CosignVerificationKey,
}

impl PublicKeyVerifier {
    /// Create a new instance of `PublicKeyVerifier`.
    /// The `key_raw` variable holds a PEM encoded rapresentation of the
    /// public key to be used at verification time.
    pub fn new(
        key_raw: &[u8],
        signature_digest_algorithm: SignatureDigestAlgorithm,
    ) -> Result<Self> {
        let key = CosignVerificationKey::from_pem(key_raw, signature_digest_algorithm)?;
        Ok(PublicKeyVerifier { key })
    }
}

impl VerificationConstraint for PublicKeyVerifier {
    fn verify(&self, signature_layer: &SignatureLayer) -> Result<bool> {
        Ok(signature_layer.is_signed_by_key(&self.key))
    }
}

/// Verification Constraint for signatures produced in keyless mode.
///
/// Keyless signatures have a x509 certificate associated to them. This
/// verifier ensures the SAN portion of the certificate has an email
/// attribute that matches the one provided by the user.
///
/// It's also possible to specify the `Issuer`, this is the name of the
/// identity provider that was used by the user to authenticate.
///
/// For example, `cosign` produces the following signature when the user
/// relies on GitHub to authenticate himself:
///
/// ```hcl
/// {
///   "critical": {
///      // not relevant
///   },
///   "optional": {
///     "Bundle": {
///       // not relevant
///     },
///     "Issuer": "https://github.com/login/oauth",
///     "Subject": "alice@example.com"
///   }
/// }
/// ```
///
/// The following constraints would be able to enforce this signature to be
/// found:
///
/// ```rust
/// use sigstore::cosign::verification_constraint::CertSubjectEmailVerifier;
///
/// // This looks only for the email address of the trusted user
/// let vc_email = CertSubjectEmailVerifier{
///     email: String::from("alice@example.com"),
///     ..Default::default()
/// };
///
/// // This ensures the user authenticated via GitHub (see the issuer value),
/// // plus the email associated to his GitHub account must be the one specified.
/// let vc_email_and_issuer = CertSubjectEmailVerifier{
///     email: String::from("alice@example.com"),
///     issuer: Some(String::from("https://github.com/login/oauth")),
/// };
/// ```
///
/// When `issuer` is `None`, the value found inside of the signature's certificate
/// is not checked.
///
/// For example, given the following constraint:
/// ```rust
/// use sigstore::cosign::verification_constraint::CertSubjectEmailVerifier;
///
/// let constraint = CertSubjectEmailVerifier{
///     email: String::from("alice@example.com"),
///     ..Default::default()
/// };
/// ```
///
/// Both these signatures would be trusted:
/// ```hcl
/// [
///   {
///     "critical": {
///        // not relevant
///     },
///     "optional": {
///       "Bundle": {
///         // not relevant
///       },
///       "Issuer": "https://github.com/login/oauth",
///       "Subject": "alice@example.com"
///     }
///   },
///   {
///     "critical": {
///        // not relevant
///     },
///     "optional": {
///       "Bundle": {
///         // not relevant
///       },
///       "Issuer": "https://example.com/login/oauth",
///       "Subject": "alice@example.com"
///     }
///   }
/// ]
/// ```
#[derive(Default, Debug)]
pub struct CertSubjectEmailVerifier {
    pub email: String,
    pub issuer: Option<String>,
}

impl VerificationConstraint for CertSubjectEmailVerifier {
    fn verify(&self, signature_layer: &SignatureLayer) -> Result<bool> {
        let verified = match &signature_layer.certificate_signature {
            Some(signature) => {
                let email_matches = match &signature.subject {
                    CertificateSubject::Email(e) => e == &self.email,
                    _ => false,
                };

                let issuer_matches = match self.issuer {
                    Some(_) => self.issuer == signature.issuer,
                    None => true,
                };

                email_matches && issuer_matches
            }
            _ => false,
        };
        Ok(verified)
    }
}

/// Verification Constraint for signatures produced in keyless mode.
///
/// Keyless signatures have a x509 certificate associated to them. This
/// verifier ensures the SAN portion of the certificate has a URI
/// attribute that matches the one provided by the user.
///
/// The constraints needs also the `Issuer` to be provided, this is the name
/// of the identity provider that was used by the user to authenticate.
///
/// This verifier can be used to check keyless signatures produced in
/// non-interactive mode inside of GitHub Actions.
///
/// For example, `cosign` produces the following signature when the
/// OIDC token is extracted from the GITHUB_TOKEN:
///
/// ```hcl
/// {
///   "critical": {
///     // not relevant
///   },
///   "optional": {
///     "Bundle": {
///     // not relevant
///     },
///     "Issuer": "https://token.actions.githubusercontent.com",
///     "Subject": "https://github.com/flavio/policy-secure-pod-images/.github/workflows/release.yml@refs/heads/main"
///   }
/// }
/// ```
///
/// The following constraint would be able to enforce this signature to be
/// found:
///
/// ```rust
/// use sigstore::cosign::verification_constraint::CertSubjectUrlVerifier;
///
/// let vc = CertSubjectUrlVerifier{
///     url: String::from("https://github.com/flavio/policy-secure-pod-images/.github/workflows/release.yml@refs/heads/main"),
///     issuer: String::from("https://token.actions.githubusercontent.com"),
/// };
/// ```
#[derive(Default, Debug)]
pub struct CertSubjectUrlVerifier {
    pub url: String,
    pub issuer: String,
}

impl VerificationConstraint for CertSubjectUrlVerifier {
    fn verify(&self, signature_layer: &SignatureLayer) -> Result<bool> {
        let verified = match &signature_layer.certificate_signature {
            Some(signature) => {
                let url_matches = match &signature.subject {
                    CertificateSubject::Uri(u) => u == &self.url,
                    _ => false,
                };
                let issuer_matches = Some(self.issuer.clone()) == signature.issuer;

                url_matches && issuer_matches
            }
            _ => false,
        };
        Ok(verified)
    }
}

/// Verification Constraint for the annotations added by `cosign sign`
///
/// The `SimpleSigning` object produced at signature time can be enriched by
/// signer with so called "anntoations".
///
/// This constraint ensures that all the annotations specified by the user are
/// found inside of the SignatureLayer.
///
/// It's perfectly find for the SignatureLayer to have additional annotations.
/// These will be simply be ignored by the verifier.
#[derive(Default, Debug)]
pub struct AnnotationVerifier {
    pub annotations: HashMap<String, String>,
}

impl VerificationConstraint for AnnotationVerifier {
    fn verify(&self, signature_layer: &SignatureLayer) -> Result<bool> {
        let verified = signature_layer
            .simple_signing
            .satisfies_annotations(&self.annotations);
        Ok(verified)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cosign::signature_layers::tests::{
        build_correct_signature_layer_with_certificate,
        build_correct_signature_layer_without_bundle,
    };
    use crate::cosign::signature_layers::CertificateSubject;

    #[test]
    fn pub_key_verifier() {
        let (sl, key) = build_correct_signature_layer_without_bundle();

        let vc = PublicKeyVerifier { key };
        assert!(vc.verify(&sl).unwrap());

        let sl = build_correct_signature_layer_with_certificate();
        assert!(!vc.verify(&sl).unwrap());
    }

    #[test]
    fn cert_email_verifier_only_email() {
        let email = "alice@example.com".to_string();
        let mut sl = build_correct_signature_layer_with_certificate();
        let mut cert_signature = sl.certificate_signature.unwrap();
        let cert_subj = CertificateSubject::Email(email.clone());
        cert_signature.issuer = None;
        cert_signature.subject = cert_subj;
        sl.certificate_signature = Some(cert_signature);

        let vc = CertSubjectEmailVerifier {
            email,
            issuer: None,
        };
        assert!(vc.verify(&sl).unwrap());

        let vc = CertSubjectEmailVerifier {
            email: "different@email.com".to_string(),
            issuer: None,
        };
        assert!(!vc.verify(&sl).unwrap());
    }

    #[test]
    fn cert_email_verifier_email_and_issuer() {
        let email = "alice@example.com".to_string();
        let mut sl = build_correct_signature_layer_with_certificate();
        let mut cert_signature = sl.certificate_signature.unwrap();

        // The cerificate subject doesn't have an issuer
        let cert_subj = CertificateSubject::Email(email.clone());
        cert_signature.issuer = None;
        cert_signature.subject = cert_subj;
        sl.certificate_signature = Some(cert_signature.clone());

        // fail because the issuer we want doesn't exist
        let vc = CertSubjectEmailVerifier {
            email: email.clone(),
            issuer: Some("an issuer".to_string()),
        };
        assert!(!vc.verify(&sl).unwrap());

        // The cerificate subject has an issuer
        let issuer = "the issuer".to_string();
        let cert_subj = CertificateSubject::Email(email.clone());
        cert_signature.issuer = Some(issuer.clone());
        cert_signature.subject = cert_subj;
        sl.certificate_signature = Some(cert_signature);

        let vc = CertSubjectEmailVerifier {
            email: email.clone(),
            issuer: Some(issuer.clone()),
        };
        assert!(vc.verify(&sl).unwrap());

        let vc = CertSubjectEmailVerifier {
            email,
            issuer: Some("another issuer".to_string()),
        };
        assert!(!vc.verify(&sl).unwrap());

        // another verifier should fail
        let vc = CertSubjectUrlVerifier {
            url: "https://sigstore.dev/test".to_string(),
            issuer,
        };
        assert!(!vc.verify(&sl).unwrap());
    }

    #[test]
    fn cert_email_verifier_no_signature() {
        let (sl, _) = build_correct_signature_layer_without_bundle();

        let vc = CertSubjectEmailVerifier {
            email: "alice@example.com".to_string(),
            issuer: None,
        };
        assert!(!vc.verify(&sl).unwrap());
    }

    #[test]
    fn cert_subject_url_verifier() {
        let url = "https://sigstore.dev/test".to_string();
        let issuer = "the issuer".to_string();

        let mut sl = build_correct_signature_layer_with_certificate();
        let mut cert_signature = sl.certificate_signature.unwrap();
        let cert_subj = CertificateSubject::Uri(url.clone());
        cert_signature.issuer = Some(issuer.clone());
        cert_signature.subject = cert_subj;
        sl.certificate_signature = Some(cert_signature);

        let vc = CertSubjectUrlVerifier {
            url: url.clone(),
            issuer: issuer.clone(),
        };
        assert!(vc.verify(&sl).unwrap());

        let vc = CertSubjectUrlVerifier {
            url: "a different url".to_string(),
            issuer: issuer.clone(),
        };
        assert!(!vc.verify(&sl).unwrap());

        let vc = CertSubjectUrlVerifier {
            url,
            issuer: "a different issuer".to_string(),
        };
        assert!(!vc.verify(&sl).unwrap());

        // A Cert email verifier should also report a non match
        let vc = CertSubjectEmailVerifier {
            email: "alice@example.com".to_string(),
            issuer: Some(issuer),
        };
        assert!(!vc.verify(&sl).unwrap());
    }

    #[test]
    fn cert_subject_verifier_no_signature() {
        let (sl, _) = build_correct_signature_layer_without_bundle();

        let vc = CertSubjectUrlVerifier {
            url: "https://sigstore.dev/test".to_string(),
            issuer: "an issuer".to_string(),
        };
        assert!(!vc.verify(&sl).unwrap());
    }
}
