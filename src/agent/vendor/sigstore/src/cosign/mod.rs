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

//! Strucs providing cosign verification capabilities
//!
//! The focus of this crate is to provide the verification capabilities of cosign,
//! not the signing one.
//!
//! Sigstore verification can be done using [`sigstore::cosign::Client`](crate::cosign::client::Client).
//! Instances of this struct can be created via the [`sigstore::cosign::ClientBuilder`](crate::cosign::client_builder::ClientBuilder).
//!
//! ## What is currently supported
//!
//! The crate implements the following verification mechanisms:
//!
//!   * Verify using a given key
//!   * Verify bundle produced by transparency log (Rekor)
//!   * Verify signature produced in keyless mode, using Fulcio Web-PKI
//!
//! Signature annotations and certificate email can be provided at verification time.
//!
//! ## Unit testing inside of our own libraries
//!
//! In case you want to mock sigstore interactions inside of your own code, you
//! can implement the [`CosignCapabilities`] trait inside of your test suite.

use async_trait::async_trait;
use tracing::warn;

use crate::errors::{Result, SigstoreVerifyConstraintsError};
use crate::registry::Auth;

mod bundle;
pub(crate) mod constants;
pub mod signature_layers;
pub use signature_layers::SignatureLayer;

pub mod client;
pub use self::client::Client;

pub mod client_builder;
pub use self::client_builder::ClientBuilder;

pub mod verification_constraint;
use self::verification_constraint::{VerificationConstraint, VerificationConstraintRefVec};

#[async_trait]
/// Cosign Abilities that have to be implemented by a
/// Cosign client
pub trait CosignCapabilities {
    /// Calculate the cosign image reference.
    /// This is the location cosign stores signatures.
    async fn triangulate(&mut self, image: &str, auth: &Auth) -> Result<(String, String)>;

    /// Returns the list of [`SignatureLayer`](crate::cosign::signature_layers::SignatureLayer)
    /// objects that are associated with the given signature object.
    ///
    /// When Fulcio's integration has been enabled, the returned `SignatureLayer`
    /// objects have been verified using the certificates bundled inside of the
    /// signature image. All these certificates have been issues by Fulcio's CA.
    ///
    /// When Rekor's integration is enabled, the [`SignatureLayer`] objects have
    /// been successfully verified using the Bundle object found inside of the
    /// signature image. All the Bundled objects have been verified using Rekor's
    /// signature.
    ///
    /// These returned objects can then be verified against
    /// [`VerificationConstraints`](crate::cosign::verification_constraint::VerificationConstraint)
    /// using the [`verify_constraints`] function.
    async fn trusted_signature_layers(
        &mut self,
        auth: &Auth,
        source_image_digest: &str,
        cosign_image: &str,
    ) -> Result<Vec<SignatureLayer>>;
}

/// Given a list of trusted `SignatureLayer`, find all the constraints that
/// aren't satisfied by the layers.
///
/// If there's any unsatisfied constraints it means that the image failed
/// verification.
/// If there's no unsatisfied constraints it means that the image passed
/// verification.
///
/// Returns a `Result` with either `Ok()` for passed verification or
/// [`SigstoreVerifyConstraintsError`](crate::errors::SigstoreVerifyConstraintsError),
/// which contains a vector of references to unsatisfied constraints.
///
/// See the documentation of the [`cosign::verification_constraint`](crate::cosign::verification_constraint) module for more
/// details about how to define verification constraints.
pub fn verify_constraints<'a, 'b, I>(
    signature_layers: &'a [SignatureLayer],
    constraints: I,
) -> std::result::Result<(), SigstoreVerifyConstraintsError<'b>>
where
    I: Iterator<Item = &'b Box<dyn VerificationConstraint>>,
{
    let unsatisfied_constraints: VerificationConstraintRefVec = constraints.filter(|c| {
        let mut is_c_unsatisfied = true;
        signature_layers.iter().any( | sl | {
            // iterate through all layers and find if at least one layer
            // satisfies constraint. If so, we stop iterating
            match c.verify(sl) {
                Ok(is_sl_verified) => {
                    is_c_unsatisfied = !is_sl_verified;
                    is_sl_verified // if true, stop searching
                }
                Err(e) => {
                    warn!(error = ?e, constraint = ?c, "Skipping layer because constraint verification returned an error");
                    // handle errors as verification failures
                    is_c_unsatisfied = true;
                    false // keep searching to see if other layer satisfies
                }
            }
        });
        is_c_unsatisfied // if true, constraint gets filtered into result
    }).collect();

    if unsatisfied_constraints.is_empty() {
        Ok(())
    } else {
        Err(SigstoreVerifyConstraintsError {
            unsatisfied_constraints,
        })
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use std::collections::HashMap;

    use super::*;
    use crate::cosign::signature_layers::tests::build_correct_signature_layer_with_certificate;
    use crate::cosign::signature_layers::CertificateSubject;
    use crate::cosign::verification_constraint::{
        AnnotationVerifier, CertSubjectEmailVerifier, VerificationConstraintVec,
    };
    use crate::crypto::certificate_pool::CertificatePool;
    use crate::crypto::{CosignVerificationKey, SignatureDigestAlgorithm};
    use crate::simple_signing::Optional;

    pub(crate) const REKOR_PUB_KEY: &str = r#"-----BEGIN PUBLIC KEY-----
MFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAE2G2Y+2tabdTV5BcGiBIx0a9fAFwr
kBbmLSGtks4L3qX6yYY0zufBnhC8Ur/iy55GhWP/9A/bY2LhC30M9+RYtw==
-----END PUBLIC KEY-----"#;

    const FULCIO_CRT_1_PEM: &str = r#"-----BEGIN CERTIFICATE-----
MIIB+DCCAX6gAwIBAgITNVkDZoCiofPDsy7dfm6geLbuhzAKBggqhkjOPQQDAzAq
MRUwEwYDVQQKEwxzaWdzdG9yZS5kZXYxETAPBgNVBAMTCHNpZ3N0b3JlMB4XDTIx
MDMwNzAzMjAyOVoXDTMxMDIyMzAzMjAyOVowKjEVMBMGA1UEChMMc2lnc3RvcmUu
ZGV2MREwDwYDVQQDEwhzaWdzdG9yZTB2MBAGByqGSM49AgEGBSuBBAAiA2IABLSy
A7Ii5k+pNO8ZEWY0ylemWDowOkNa3kL+GZE5Z5GWehL9/A9bRNA3RbrsZ5i0Jcas
taRL7Sp5fp/jD5dxqc/UdTVnlvS16an+2Yfswe/QuLolRUCrcOE2+2iA5+tzd6Nm
MGQwDgYDVR0PAQH/BAQDAgEGMBIGA1UdEwEB/wQIMAYBAf8CAQEwHQYDVR0OBBYE
FMjFHQBBmiQpMlEk6w2uSu1KBtPsMB8GA1UdIwQYMBaAFMjFHQBBmiQpMlEk6w2u
Su1KBtPsMAoGCCqGSM49BAMDA2gAMGUCMH8liWJfMui6vXXBhjDgY4MwslmN/TJx
Ve/83WrFomwmNf056y1X48F9c4m3a3ozXAIxAKjRay5/aj/jsKKGIkmQatjI8uup
Hr/+CxFvaJWmpYqNkLDGRU+9orzh5hI2RrcuaQ==
-----END CERTIFICATE-----"#;

    const FULCIO_CRT_2_PEM: &str = r#"-----BEGIN CERTIFICATE-----
MIIB9zCCAXygAwIBAgIUALZNAPFdxHPwjeDloDwyYChAO/4wCgYIKoZIzj0EAwMw
KjEVMBMGA1UEChMMc2lnc3RvcmUuZGV2MREwDwYDVQQDEwhzaWdzdG9yZTAeFw0y
MTEwMDcxMzU2NTlaFw0zMTEwMDUxMzU2NThaMCoxFTATBgNVBAoTDHNpZ3N0b3Jl
LmRldjERMA8GA1UEAxMIc2lnc3RvcmUwdjAQBgcqhkjOPQIBBgUrgQQAIgNiAAT7
XeFT4rb3PQGwS4IajtLk3/OlnpgangaBclYpsYBr5i+4ynB07ceb3LP0OIOZdxex
X69c5iVuyJRQ+Hz05yi+UF3uBWAlHpiS5sh0+H2GHE7SXrk1EC5m1Tr19L9gg92j
YzBhMA4GA1UdDwEB/wQEAwIBBjAPBgNVHRMBAf8EBTADAQH/MB0GA1UdDgQWBBRY
wB5fkUWlZql6zJChkyLQKsXF+jAfBgNVHSMEGDAWgBRYwB5fkUWlZql6zJChkyLQ
KsXF+jAKBggqhkjOPQQDAwNpADBmAjEAj1nHeXZp+13NWBNa+EDsDP8G1WWg1tCM
WP/WHPqpaVo0jhsweNFZgSs0eE7wYI4qAjEA2WB9ot98sIkoF3vZYdd3/VtWB5b9
TNMea7Ix/stJ5TfcLLeABLE4BNJOsQ4vnBHJ
-----END CERTIFICATE-----"#;

    pub(crate) fn get_fulcio_cert_pool() -> CertificatePool {
        let certificates = vec![
            crate::registry::Certificate {
                encoding: crate::registry::CertificateEncoding::Pem,
                data: FULCIO_CRT_1_PEM.as_bytes().to_vec(),
            },
            crate::registry::Certificate {
                encoding: crate::registry::CertificateEncoding::Pem,
                data: FULCIO_CRT_2_PEM.as_bytes().to_vec(),
            },
        ];
        CertificatePool::from_certificates(&certificates).unwrap()
    }

    pub(crate) fn get_rekor_public_key() -> CosignVerificationKey {
        CosignVerificationKey::from_pem(
            REKOR_PUB_KEY.as_bytes(),
            SignatureDigestAlgorithm::default(),
        )
        .expect("Cannot create test REKOR_PUB_KEY")
    }

    #[test]
    fn verify_constraints_all_satisfied() {
        let email = "alice@example.com".to_string();
        let issuer = "an issuer".to_string();

        let mut annotations: HashMap<String, String> = HashMap::new();
        annotations.insert("key1".into(), "value1".into());
        annotations.insert("key2".into(), "value2".into());

        let mut layers: Vec<SignatureLayer> = Vec::new();
        for _ in 0..5 {
            let mut sl = build_correct_signature_layer_with_certificate();
            let mut cert_signature = sl.certificate_signature.unwrap();
            let cert_subj = CertificateSubject::Email(email.clone());
            cert_signature.issuer = Some(issuer.clone());
            cert_signature.subject = cert_subj;
            sl.certificate_signature = Some(cert_signature);

            let mut extra: HashMap<String, serde_json::Value> = annotations
                .iter()
                .map(|(k, v)| (k.clone(), json!(v)))
                .collect();
            extra.insert("something extra".into(), json!("value extra"));

            let mut simple_signing = sl.simple_signing;
            let optional = Optional {
                creator: Some("test".into()),
                timestamp: None,
                extra,
            };
            simple_signing.optional = Some(optional);
            sl.simple_signing = simple_signing;

            layers.push(sl);
        }

        let mut constraints: VerificationConstraintVec = Vec::new();
        let vc = CertSubjectEmailVerifier {
            email: email.clone(),
            issuer: Some(issuer),
        };
        constraints.push(Box::new(vc));

        let vc = CertSubjectEmailVerifier {
            email,
            issuer: None,
        };
        constraints.push(Box::new(vc));

        let vc = AnnotationVerifier { annotations };
        constraints.push(Box::new(vc));

        verify_constraints(&layers, constraints.iter()).expect("should not return an error");
    }

    #[test]
    fn verify_constraints_none_satisfied() {
        let email = "alice@example.com".to_string();
        let issuer = "an issuer".to_string();
        let wrong_email = "bob@example.com".to_string();

        let mut layers: Vec<SignatureLayer> = Vec::new();
        for _ in 0..5 {
            let mut sl = build_correct_signature_layer_with_certificate();
            let mut cert_signature = sl.certificate_signature.unwrap();
            let cert_subj = CertificateSubject::Email(email.clone());
            cert_signature.issuer = Some(issuer.clone());
            cert_signature.subject = cert_subj;
            sl.certificate_signature = Some(cert_signature);

            let mut extra: HashMap<String, serde_json::Value> = HashMap::new();
            extra.insert("something extra".into(), json!("value extra"));

            let mut simple_signing = sl.simple_signing;
            let optional = Optional {
                creator: Some("test".into()),
                timestamp: None,
                extra,
            };
            simple_signing.optional = Some(optional);
            sl.simple_signing = simple_signing;

            layers.push(sl);
        }

        let mut constraints: VerificationConstraintVec = Vec::new();
        let vc = CertSubjectEmailVerifier {
            email: wrong_email.clone(),
            issuer: Some(issuer), // correct issuer
        };
        constraints.push(Box::new(vc));

        let vc = CertSubjectEmailVerifier {
            email: wrong_email,
            issuer: None, // missing issuer, more relaxed
        };
        constraints.push(Box::new(vc));

        let err =
            verify_constraints(&layers, constraints.iter()).expect_err("we should have an err");
        assert_eq!(err.unsatisfied_constraints.len(), 2);
    }

    #[test]
    fn verify_constraints_some_unsatisfied() {
        let email = "alice@example.com".to_string();
        let issuer = "an issuer".to_string();
        let email_incorrect = "bob@example.com".to_string();

        let mut layers: Vec<SignatureLayer> = Vec::new();
        for _ in 0..5 {
            let mut sl = build_correct_signature_layer_with_certificate();
            let mut cert_signature = sl.certificate_signature.unwrap();
            let cert_subj = CertificateSubject::Email(email.clone());
            cert_signature.issuer = Some(issuer.clone());
            cert_signature.subject = cert_subj;
            sl.certificate_signature = Some(cert_signature);

            let mut extra: HashMap<String, serde_json::Value> = HashMap::new();
            extra.insert("something extra".into(), json!("value extra"));

            let mut simple_signing = sl.simple_signing;
            let optional = Optional {
                creator: Some("test".into()),
                timestamp: None,
                extra,
            };
            simple_signing.optional = Some(optional);
            sl.simple_signing = simple_signing;

            layers.push(sl);
        }

        let mut constraints: VerificationConstraintVec = Vec::new();
        let satisfied_constraint = CertSubjectEmailVerifier {
            email,
            issuer: Some(issuer),
        };
        constraints.push(Box::new(satisfied_constraint));

        let unsatisfied_constraint = CertSubjectEmailVerifier {
            email: email_incorrect,
            issuer: None,
        };
        constraints.push(Box::new(unsatisfied_constraint));

        let err =
            verify_constraints(&layers, constraints.iter()).expect_err("we should have an err");
        assert_eq!(err.unsatisfied_constraints.len(), 1);
    }
}
