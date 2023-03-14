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

use oci_distribution::client::ImageLayer;
use serde::Serialize;
use std::{collections::HashMap, fmt};
use tracing::{debug, info};
use x509_parser::{
    certificate::X509Certificate, der_parser::oid::Oid, extensions::GeneralName,
    parse_x509_certificate, pem::parse_x509_pem,
};

use super::bundle::Bundle;
use super::constants::{
    SIGSTORE_BUNDLE_ANNOTATION, SIGSTORE_CERT_ANNOTATION, SIGSTORE_GITHUB_WORKFLOW_NAME_OID,
    SIGSTORE_GITHUB_WORKFLOW_REF_OID, SIGSTORE_GITHUB_WORKFLOW_REPOSITORY_OID,
    SIGSTORE_GITHUB_WORKFLOW_SHA_OID, SIGSTORE_GITHUB_WORKFLOW_TRIGGER_OID, SIGSTORE_ISSUER_OID,
    SIGSTORE_OCI_MEDIA_TYPE, SIGSTORE_SIGNATURE_ANNOTATION,
};
use crate::crypto::certificate_pool::CertificatePool;
use crate::{
    crypto::{
        self, CosignVerificationKey, Signature, SIGSTORE_DEFAULT_SIGNATURE_VERIFICATION_ALGORITHM,
    },
    errors::{Result, SigstoreError},
    simple_signing::SimpleSigning,
};

/// Describe the details of a a certificate produced when signing artifacts
/// using the keyless mode.
#[derive(Clone, Debug, Serialize)]
pub struct CertificateSignature {
    /// The verification key embedded into the Certificate
    #[serde(skip_serializing)]
    pub verification_key: CosignVerificationKey,
    /// The unique ID associated to the identity
    pub subject: CertificateSubject,
    /// The issuer used by the signer to authenticate. (e.g. GitHub, GitHub Action, Microsoft, Google,...)
    pub issuer: Option<String>,
    /// The trigger of the GitHub workflow (e.g. `push`)
    pub github_workflow_trigger: Option<String>,
    /// The commit ID that triggered the GitHub workflow
    pub github_workflow_sha: Option<String>,
    /// The name of the GitHub workflow (e.g. `release artifact`)
    pub github_workflow_name: Option<String>,
    /// The repository that owns the GitHub workflow (e.g. `octocat/example-repo`)
    pub github_workflow_repository: Option<String>,
    /// The Git ref of the commit that triggered the GitHub workflow (e.g. `refs/tags/v0.9.9`)
    pub github_workflow_ref: Option<String>,
}

impl fmt::Display for CertificateSignature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let msg = format!(
            r#"CertificateSignature
- issuer: {:?}
- subject: {:?}
- GitHub Workflow trigger: {:?}
- GitHub Workflow SHA: {:?}
- GitHub Workflow name: {:?}
- GitHub Workflow repository: {:?}
- GitHub Workflow ref: {:?}
---"#,
            self.issuer,
            self.subject,
            self.github_workflow_trigger,
            self.github_workflow_sha,
            self.github_workflow_name,
            self.github_workflow_repository,
            self.github_workflow_ref,
        );

        write!(f, "{}", msg)
    }
}

/// Types of identities associated with the signer.
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", content = "value")]
pub enum CertificateSubject {
    /// An email address. This is what is used when the signer authenticated himself using something like his GitHub/Google account
    Email(String),
    /// A URL. This is used for example by the OIDC token issued by GitHub Actions
    Uri(String),
}

/// Object that contains all the data about a
/// [`SimpleSigning`](crate::simple_signing::SimpleSigning) object.
///
/// The struct provides some helper methods that can be used at verification
/// time.
///
/// Note well, the information needed to build a SignatureLayer are spread over
/// two places:
///   * The manifest of the signature object created by cosign
///   * One or more SIGSTORE_OCI_MEDIA_TYPE layers
///
/// End users of this library are not supposed to create this object directly.
/// `SignatureLayer` objects are instead obtained by using the
/// [`sigstore::cosign::Client::trusted_signature_layers`](crate::cosign::client::Client)
/// method.
#[derive(Clone, Debug, Serialize)]
pub struct SignatureLayer {
    /// The Simple Signing object associated with this layer
    pub simple_signing: SimpleSigning,
    /// The digest of the layer
    pub oci_digest: String,
    /// The certificate holding the identity of the signer, plus his
    /// verification key. This exists for signature done with keyless mode.
    pub certificate_signature: Option<CertificateSignature>,
    /// The bundle produced by Rekor.
    pub bundle: Option<Bundle>,
    #[serde(skip_serializing)]
    pub signature: String,
    #[serde(skip_serializing)]
    pub raw_data: Vec<u8>,
}

impl fmt::Display for SignatureLayer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let msg = format!(
            r#"---
# SignatureLayer
## digest
{}

## signature
{:?}

## bundle:
{:?}

## certificate signature
{}

## Simple Signing
{}
---"#,
            self.oci_digest,
            self.signature,
            self.bundle,
            self.certificate_signature
                .clone()
                .map(|cs| cs.to_string())
                .unwrap_or_else(|| "None".to_string()),
            self.simple_signing,
        );

        write!(f, "{}", msg)
    }
}

impl SignatureLayer {
    /// Create a SignatureLayer that can be considered trusted.
    ///
    /// Params:
    ///   * `descriptor`: the metatada of the layer, taken from the OCI manifest associated
    ///     with the Sigstore object
    ///   * `layer`: the data referenced by the descriptor
    ///   * `source_image_digest`: the digest of the object that we're trying
    ///      to verify. This is **not** the digest of the signature itself.
    ///   * `rekor_pub_key`: the public key of Rekor, used to verify `bundle`
    ///     entries
    ///   * `fulcio_pub_key`: the public key provided by Fulcio's certificate.
    ///     Used to verify the `certificate` entries
    ///   * `cert_email`: optional, the SAN to look for inside of trusted
    ///     certificates issued by Fulcio
    ///
    /// **Note well:** the certificate and bundle added to the final SignatureLayer
    /// object are to be considered **trusted** and **verified**, according to
    /// the parameters provided to this method.
    pub(crate) fn new(
        descriptor: &oci_distribution::manifest::OciDescriptor,
        layer: &oci_distribution::client::ImageLayer,
        source_image_digest: &str,
        rekor_pub_key: Option<&CosignVerificationKey>,
        fulcio_cert_pool: Option<&CertificatePool>,
    ) -> Result<SignatureLayer> {
        if descriptor.media_type != SIGSTORE_OCI_MEDIA_TYPE {
            return Err(SigstoreError::SigstoreMediaTypeNotFoundError);
        }

        if layer.media_type != SIGSTORE_OCI_MEDIA_TYPE {
            return Err(SigstoreError::SigstoreMediaTypeNotFoundError);
        }

        let layer_digest = layer.clone().sha256_digest();
        if descriptor.digest != layer_digest {
            return Err(SigstoreError::SigstoreLayerDigestMismatchError);
        }

        let simple_signing: SimpleSigning = serde_json::from_slice(&layer.data).map_err(|e| {
            SigstoreError::UnexpectedError(format!(
                "Cannot convert layer data into SimpleSigning object: {:?}",
                e
            ))
        })?;

        if !simple_signing.satisfies_manifest_digest(source_image_digest) {
            return Err(SigstoreError::UnexpectedError(
                "Simple signing image digest mismatch".to_string(),
            ));
        }

        let annotations = descriptor.annotations.clone().unwrap_or_default();

        let signature = Self::get_signature_from_annotations(&annotations)?;
        let bundle = Self::get_bundle_from_annotations(&annotations, rekor_pub_key)?;
        let certificate_signature = Self::get_certificate_signature_from_annotations(
            &annotations,
            fulcio_cert_pool,
            bundle.as_ref(),
        )?;

        Ok(SignatureLayer {
            oci_digest: descriptor.digest.clone(),
            raw_data: layer.data.clone(),
            simple_signing,
            signature,
            bundle,
            certificate_signature,
        })
    }

    fn get_signature_from_annotations(annotations: &HashMap<String, String>) -> Result<String> {
        let signature: String = annotations
            .get(SIGSTORE_SIGNATURE_ANNOTATION)
            .cloned()
            .ok_or(SigstoreError::SigstoreAnnotationNotFoundError)?;
        Ok(signature)
    }

    fn get_bundle_from_annotations(
        annotations: &HashMap<String, String>,
        rekor_pub_key: Option<&CosignVerificationKey>,
    ) -> Result<Option<Bundle>> {
        let bundle = match annotations.get(SIGSTORE_BUNDLE_ANNOTATION) {
            Some(value) => match rekor_pub_key {
                Some(key) => Some(Bundle::new_verified(value, key)?),
                None => {
                    info!(bundle = ?value, "Ignoring bundle, rekor public key not provided to verification client");
                    None
                }
            },
            None => None,
        };
        Ok(bundle)
    }

    fn get_certificate_signature_from_annotations(
        annotations: &HashMap<String, String>,
        fulcio_cert_pool: Option<&CertificatePool>,
        bundle: Option<&Bundle>,
    ) -> Result<Option<CertificateSignature>> {
        let cert_raw = match annotations.get(SIGSTORE_CERT_ANNOTATION) {
            Some(value) => value,
            None => return Ok(None),
        };

        let fulcio_cert_pool = match fulcio_cert_pool {
            Some(cp) => cp,
            None => {
                return Err(SigstoreError::SigstoreFulcioCertificatesNotProvidedError);
            }
        };

        let bundle = match bundle {
            Some(b) => b,
            None => {
                return Err(SigstoreError::SigstoreRekorBundleNotFoundError);
            }
        };

        let certificate_signature =
            CertificateSignature::from_certificate(cert_raw.as_bytes(), fulcio_cert_pool, bundle)?;
        Ok(Some(certificate_signature))
    }

    /// Given a Cosign public key, check whether this Signature Layer has been
    /// signed by it
    pub(crate) fn is_signed_by_key(&self, verification_key: &CosignVerificationKey) -> bool {
        match verification_key.verify_signature(
            Signature::Base64Encoded(self.signature.as_bytes()),
            &self.raw_data,
        ) {
            Ok(_) => true,
            Err(e) => {
                debug!(signature=self.signature.as_str(), reason=?e, "Cannot verify signature with the given key");
                false
            }
        }
    }
}

/// Creates a list of [`SignatureLayer`] objects by inspecting
/// the given OCI manifest and its associated layers.
///
/// **Note well:** when Rekor and Fulcio data has been provided, the
/// returned `SignatureLayer` is guaranteed to be
/// verified using the given Rekor and Fulcio keys.
pub(crate) fn build_signature_layers(
    manifest: &oci_distribution::manifest::OciImageManifest,
    source_image_digest: &str,
    layers: &[oci_distribution::client::ImageLayer],
    rekor_pub_key: Option<&CosignVerificationKey>,
    fulcio_cert_pool: Option<&CertificatePool>,
) -> Result<Vec<SignatureLayer>> {
    let mut signature_layers: Vec<SignatureLayer> = Vec::new();

    for manifest_layer in &manifest.layers {
        let matching_layer: Option<&oci_distribution::client::ImageLayer> =
            layers.iter().find(|l| {
                let tmp: ImageLayer = (*l).clone();
                tmp.sha256_digest() == manifest_layer.digest
            });
        if let Some(layer) = matching_layer {
            match SignatureLayer::new(
                manifest_layer,
                layer,
                source_image_digest,
                rekor_pub_key,
                fulcio_cert_pool,
            ) {
                Ok(sl) => signature_layers.push(sl),
                Err(e) => {
                    info!(error = ?e, "Skipping OCI layer because of error");
                }
            }
        }
    }

    if signature_layers.is_empty() {
        Err(SigstoreError::SigstoreNoVerifiedLayer)
    } else {
        Ok(signature_layers)
    }
}

impl CertificateSignature {
    /// Ensure the given certificate can be trusted, then extracts
    /// its details and return them as a `CertificateSignature` object
    pub(crate) fn from_certificate(
        cert_raw: &[u8],
        fulcio_cert_pool: &CertificatePool,
        trusted_bundle: &Bundle,
    ) -> Result<Self> {
        let (_, pem) = parse_x509_pem(cert_raw)?;
        let (_, cert) = parse_x509_certificate(&pem.contents)?;
        let integrated_time = trusted_bundle.payload.integrated_time;

        // ensure the certificate has been issued by Fulcio
        fulcio_cert_pool.verify(cert_raw)?;

        crypto::certificate::is_trusted(&cert, integrated_time)?;

        let subject = CertificateSubject::from_certificate(&cert)?;
        let verification_key = CosignVerificationKey::from_der(
            cert.public_key().raw,
            SIGSTORE_DEFAULT_SIGNATURE_VERIFICATION_ALGORITHM,
        )?;

        let issuer = get_cert_extension_by_oid(&cert, SIGSTORE_ISSUER_OID, "Issuer")?;

        let github_workflow_trigger = get_cert_extension_by_oid(
            &cert,
            SIGSTORE_GITHUB_WORKFLOW_TRIGGER_OID,
            "GitHub Workflow trigger",
        )?;

        let github_workflow_sha = get_cert_extension_by_oid(
            &cert,
            SIGSTORE_GITHUB_WORKFLOW_SHA_OID,
            "GitHub Workflow sha",
        )?;

        let github_workflow_name = get_cert_extension_by_oid(
            &cert,
            SIGSTORE_GITHUB_WORKFLOW_NAME_OID,
            "GitHub Workflow name",
        )?;

        let github_workflow_repository = get_cert_extension_by_oid(
            &cert,
            SIGSTORE_GITHUB_WORKFLOW_REPOSITORY_OID,
            "GitHub Workflow repository",
        )?;

        let github_workflow_ref = get_cert_extension_by_oid(
            &cert,
            SIGSTORE_GITHUB_WORKFLOW_REF_OID,
            "GitHub Workflow ref",
        )?;

        Ok(CertificateSignature {
            verification_key,
            issuer,
            github_workflow_trigger,
            github_workflow_sha,
            github_workflow_name,
            github_workflow_repository,
            github_workflow_ref,
            subject,
        })
    }
}

fn get_cert_extension_by_oid(
    cert: &X509Certificate,
    ext_oid: Oid,
    ext_oid_name: &str,
) -> Result<Option<String>> {
    let extension = cert.tbs_certificate.get_extension_unique(&ext_oid)?;
    extension
        .map(|ext| {
            String::from_utf8(ext.value.to_vec()).map_err(|_| {
                SigstoreError::UnexpectedError(format!(
                    "Certificate's extension Sigstore {} is not UTF8 compatible",
                    ext_oid_name,
                ))
            })
        })
        .transpose()
}

impl CertificateSubject {
    pub fn from_certificate(certificate: &X509Certificate) -> Result<CertificateSubject> {
        let subject_alternative_name = certificate
            .tbs_certificate
            .subject_alternative_name()?
            .ok_or(SigstoreError::CertificateWithoutSubjectAlternativeName)?;

        for general_name in &subject_alternative_name.value.general_names {
            if let GeneralName::RFC822Name(name) = general_name {
                return Ok(CertificateSubject::Email(name.to_string()));
            }

            if let GeneralName::URI(uri) = general_name {
                return Ok(CertificateSubject::Uri(uri.to_string()));
            }
        }

        Err(SigstoreError::CertificateWithIncompleteSubjectAlternativeName)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use openssl::x509::X509;
    use serde_json::json;
    use std::collections::HashMap;
    use std::convert::TryFrom;

    use crate::cosign::tests::{get_fulcio_cert_pool, get_rekor_public_key};
    use crate::crypto::SignatureDigestAlgorithm;

    pub(crate) fn build_correct_signature_layer_without_bundle(
    ) -> (SignatureLayer, CosignVerificationKey) {
        let public_key = r#"-----BEGIN PUBLIC KEY-----
MFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAENptdY/l3nB0yqkXLBWkZWQwo6+cu
OSWS1X9vPavpiQOoTTGC0xX57OojUadxF1cdQmrsiReWg2Wn4FneJfa8xw==
-----END PUBLIC KEY-----"#;

        let signature = String::from("MEUCIQD6q/COgzOyW0YH1Dk+CCYSt4uAhm3FDHUwvPI55zwnlwIgE0ZK58ZOWpZw8YVmBapJhBqCfdPekIknimuO0xH8Jh8=");
        let verification_key = CosignVerificationKey::from_pem(
            public_key.as_bytes(),
            SignatureDigestAlgorithm::default(),
        )
        .expect("Cannot create CosignVerificationKey");
        let ss_value = json!({
            "critical": {
                "identity": {
                    "docker-reference":"registry-testing.svc.lan/busybox"
                },
                "image":{
                    "docker-manifest-digest":"sha256:f3cfc9d0dbf931d3db4685ec659b7ac68e2a578219da4aae65427886e649b06b"
                },
                "type":"cosign container image signature"
            },
            "optional":null
        });

        (
            SignatureLayer {
                simple_signing: serde_json::from_value(ss_value.clone()).unwrap(),
                oci_digest: String::from("digest"),
                signature,
                bundle: None,
                certificate_signature: None,
                raw_data: serde_json::to_vec(&ss_value).unwrap(),
            },
            verification_key,
        )
    }

    pub(crate) fn build_bundle() -> Bundle {
        let bundle_value = json!({
          "SignedEntryTimestamp": "MEUCIDBGJijj2FqU25yRWzlEWHqE64XKwUvychBs1bSM1PaKAiEAwcR2u81c42TLBk3lWJqhtB7SnM7Lh0OYEl6Bfa7ZA4s=",
          "Payload": {
            "body": "eyJhcGlWZXJzaW9uIjoiMC4wLjEiLCJraW5kIjoicmVrb3JkIiwic3BlYyI6eyJkYXRhIjp7Imhhc2giOnsiYWxnb3JpdGhtIjoic2hhMjU2IiwidmFsdWUiOiJlNzgwMWRlOTM1NTEyZTIyYjIzN2M3YjU3ZTQyY2E0ZDIwZTIxMzRiZGYxYjk4Zjk3NmM4ZjU1ZDljZmU0MDY3In19LCJzaWduYXR1cmUiOnsiY29udGVudCI6Ik1FVUNJR3FXU2N6N3M5YVAyc0dYTkZLZXFpdnczQjZrUFJzNTZBSVRJSG52ZDVpZ0FpRUExa3piYVYyWTV5UEU4MUVOOTJOVUZPbDMxTExKU3Z3c2pGUTA3bTJYcWFBPSIsImZvcm1hdCI6Ing1MDkiLCJwdWJsaWNLZXkiOnsiY29udGVudCI6IkxTMHRMUzFDUlVkSlRpQkRSVkpVU1VaSlEwRlVSUzB0TFMwdENrMUpTVU5rZWtORFFXWjVaMEYzU1VKQlowbFVRU3RRYzJGTGFtRkZXbkZ1TjBsWk9UUmlNV1V2YWtwdWFYcEJTMEpuWjNGb2EycFBVRkZSUkVGNlFYRUtUVkpWZDBWM1dVUldVVkZMUlhkNGVtRlhaSHBrUnpsNVdsTTFhMXBZV1hoRlZFRlFRbWRPVmtKQlRWUkRTRTV3V2pOT01HSXpTbXhOUWpSWVJGUkplQXBOVkVGNVRVUkJNMDFxVlhoT2JHOVlSRlJKZUUxVVFYbE5SRUV6VGtSVmVFNVdiM2RCUkVKYVRVSk5SMEo1Y1VkVFRUUTVRV2RGUjBORGNVZFRUVFE1Q2tGM1JVaEJNRWxCUWtsT1pYZFJRbE14WmpSQmJVNUpSVTVrVEN0VkwwaEtiM1JOVTAwM1drNXVhMVJ1V1dWbWVIZFdPVlJGY25CMmJrRmFNQ3RFZWt3S2VXWkJRVlpoWlVwMFMycEdkbUpQVkdJNFJqRjVhRXBHVlRCWVdTdFNhV3BuWjBWd1RVbEpRa3BVUVU5Q1owNVdTRkU0UWtGbU9FVkNRVTFEUWpSQmR3cEZkMWxFVmxJd2JFSkJkM2REWjFsSlMzZFpRa0pSVlVoQmQwMTNSRUZaUkZaU01GUkJVVWd2UWtGSmQwRkVRV1JDWjA1V1NGRTBSVVpuVVZWTlpqRlNDazFOYzNGT1JrSnlWMko0T0cxU1RtUjRUMnRGUlZsemQwaDNXVVJXVWpCcVFrSm5kMFp2UVZWNVRWVmtRVVZIWVVwRGEzbFZVMVJ5UkdFMVN6ZFZiMGNLTUN0M2QyZFpNRWREUTNOSFFWRlZSa0ozUlVKQ1NVZEJUVWcwZDJaQldVbExkMWxDUWxGVlNFMUJTMGRqUjJnd1pFaEJOa3g1T1hkamJXd3lXVmhTYkFwWk1rVjBXVEk1ZFdSSFZuVmtRekF5VFVST2JWcFVaR3hPZVRCM1RVUkJkMHhVU1hsTmFtTjBXVzFaTTA1VE1XMU9SMWt4V2xSbmQxcEVTVFZPVkZGMUNtTXpVblpqYlVadVdsTTFibUl5T1c1aVIxWm9ZMGRzZWt4dFRuWmlVemxxV1ZSTk1sbFVSbXhQVkZsNVRrUkthVTlYV21wWmFrVXdUbWs1YWxsVE5Xb0tZMjVSZDBsQldVUldVakJTUVZGSUwwSkNXWGRHU1VWVFdtMTRhR1J0YkhaUlIwNW9Zek5TYkdKSGVIQk1iVEZzVFVGdlIwTkRjVWRUVFRRNVFrRk5SQXBCTW10QlRVZFpRMDFSUXpOWk1uVnNVRlJ6VUcxT1V6UmplbUZMWldwbE1FSnVUMUZJZWpWbE5rNUNXREJDY1hnNVdHTmhLM1F5YTA5cE1UZHpiM0JqQ2k5MkwzaElNWGhNZFZCdlEwMVJSRXRPUkRSWGFraG1TM0ZZV0U5bFZYWmFPVUU1TmtSeGNrVjNSMkZ4UjAxMGJrbDFUalJLZWxwWllWVk1Xbko0T1djS2IxaHhjVzh2UXpsUmJrOUlWSFJ2UFFvdExTMHRMVVZPUkNCRFJWSlVTVVpKUTBGVVJTMHRMUzB0Q2c9PSJ9fX19",
            "integratedTime": 1634714717,
            "logIndex": 783607,
            "logID": "c0d23d6ad406973f9559f3ba2d1ca01f84147d8ffc5b8445c224f98b9591801d"
          }
        });
        let bundle: Bundle = serde_json::from_value(bundle_value).expect("Cannot parse bundle");
        bundle
    }

    pub(crate) fn build_correct_signature_layer_with_certificate() -> SignatureLayer {
        let ss_value = json!({
            "critical": {
              "identity": {
                "docker-reference": "registry-testing.svc.lan/kubewarden/disallow-service-nodeport"
              },
              "image": {
                "docker-manifest-digest": "sha256:5f481572d088dc4023afb35fced9530ced3d9b03bf7299c6f492163cb9f0452e"
              },
              "type": "cosign container image signature"
            },
            "optional": null
        });

        let bundle = build_bundle();

        let cert_raw = r#"-----BEGIN CERTIFICATE-----
MIICdzCCAfygAwIBAgITA+PsaKjaEZqn7IY94b1e/jJnizAKBggqhkjOPQQDAzAq
MRUwEwYDVQQKEwxzaWdzdG9yZS5kZXYxETAPBgNVBAMTCHNpZ3N0b3JlMB4XDTIx
MTAyMDA3MjUxNloXDTIxMTAyMDA3NDUxNVowADBZMBMGByqGSM49AgEGCCqGSM49
AwEHA0IABINewQBS1f4AmNIENdL+U/HJotMSM7ZNnkTnYefxwV9TErpvnAZ0+DzL
yfAAVaeJtKjFvbOTb8F1yhJFU0XY+RijggEpMIIBJTAOBgNVHQ8BAf8EBAMCB4Aw
EwYDVR0lBAwwCgYIKwYBBQUHAwMwDAYDVR0TAQH/BAIwADAdBgNVHQ4EFgQUMf1R
MMsqNFBrWbx8mRNdxOkEEYswHwYDVR0jBBgwFoAUyMUdAEGaJCkyUSTrDa5K7UoG
0+wwgY0GCCsGAQUFBwEBBIGAMH4wfAYIKwYBBQUHMAKGcGh0dHA6Ly9wcml2YXRl
Y2EtY29udGVudC02MDNmZTdlNy0wMDAwLTIyMjctYmY3NS1mNGY1ZTgwZDI5NTQu
c3RvcmFnZS5nb29nbGVhcGlzLmNvbS9jYTM2YTFlOTYyNDJiOWZjYjE0Ni9jYS5j
cnQwIAYDVR0RAQH/BBYwFIESZmxhdmlvQGNhc3RlbGxpLm1lMAoGCCqGSM49BAMD
A2kAMGYCMQC3Y2ulPTsPmNS4czaKeje0BnOQHz5e6NBX0Bqx9Xca+t2kOi17sopc
/v/xH1xLuPoCMQDKND4WjHfKqXXOeUvZ9A96DqrEwGaqGMtnIuN4JzZYaULZrx9g
oXqqo/C9QnOHTto=
-----END CERTIFICATE-----"#;

        let fulcio_cert_pool = get_fulcio_cert_pool();
        let certificate_signature =
            CertificateSignature::from_certificate(cert_raw.as_bytes(), &fulcio_cert_pool, &bundle)
                .expect("Cannot create certificate signature");

        SignatureLayer {
            simple_signing: serde_json::from_value(ss_value.clone()).unwrap(),
            oci_digest: String::from("sha256:5f481572d088dc4023afb35fced9530ced3d9b03bf7299c6f492163cb9f0452e"),
            signature: String::from("MEUCIGqWScz7s9aP2sGXNFKeqivw3B6kPRs56AITIHnvd5igAiEA1kzbaV2Y5yPE81EN92NUFOl31LLJSvwsjFQ07m2XqaA="),
            bundle: Some(bundle),
            certificate_signature: Some(certificate_signature),
            raw_data: serde_json::to_vec(&ss_value).unwrap(),
        }
    }

    #[test]
    fn is_signed_by_key_fails_when_signature_is_not_valid() {
        let (signature_layer, _) = build_correct_signature_layer_without_bundle();
        let verification_key = CosignVerificationKey::from_pem(
            r#"-----BEGIN PUBLIC KEY-----
MFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAETJP9cqpUQsn2ggmJniWGjHdlsHzD
JsB89BPhZYch0U0hKANx5TY+ncrm0s8bfJxxHoenAEFhwhuXeb4PqIrtoQ==
-----END PUBLIC KEY-----"#
                .as_bytes(),
            SignatureDigestAlgorithm::default(),
        )
        .expect("Cannot create CosignVerificationKey");

        let actual = signature_layer.is_signed_by_key(&verification_key);
        assert!(!actual, "expected false, got true");
    }

    #[test]
    fn new_signature_layer_fails_because_bad_descriptor() {
        let descriptor = oci_distribution::manifest::OciDescriptor {
            media_type: "not what you would expected".into(),
            ..Default::default()
        };
        let layer = oci_distribution::client::ImageLayer {
            media_type: super::SIGSTORE_OCI_MEDIA_TYPE.to_string(),
            data: Vec::new(),
            annotations: None,
        };

        let rekor_pub_key = get_rekor_public_key();

        let fulcio_cert_pool = get_fulcio_cert_pool();

        let error = SignatureLayer::new(
            &descriptor,
            &layer,
            "source_image_digest is not relevant now",
            Some(&rekor_pub_key),
            Some(&fulcio_cert_pool),
        )
        .expect_err("Didn't get an error");

        let found = match error {
            SigstoreError::SigstoreMediaTypeNotFoundError => true,
            _ => false,
        };
        assert!(found, "Got a different error type: {}", error);
    }

    #[test]
    fn new_signature_layer_fails_because_bad_layer() {
        let descriptor = oci_distribution::manifest::OciDescriptor {
            media_type: super::SIGSTORE_OCI_MEDIA_TYPE.to_string(),
            ..Default::default()
        };
        let layer = oci_distribution::client::ImageLayer {
            media_type: "not what you would expect".into(),
            data: Vec::new(),
            annotations: None,
        };

        let rekor_pub_key = get_rekor_public_key();

        let fulcio_cert_pool = get_fulcio_cert_pool();

        let error = SignatureLayer::new(
            &descriptor,
            &layer,
            "source_image_digest is not relevant now",
            Some(&rekor_pub_key),
            Some(&fulcio_cert_pool),
        )
        .expect_err("Didn't get an error");

        let found = match error {
            SigstoreError::SigstoreMediaTypeNotFoundError => true,
            _ => false,
        };
        assert!(found, "Got a different error type: {}", error);
    }

    #[test]
    fn new_signature_layer_fails_because_checksum_mismatch() {
        let descriptor = oci_distribution::manifest::OciDescriptor {
            media_type: super::SIGSTORE_OCI_MEDIA_TYPE.to_string(),
            digest: "some digest".into(),
            ..Default::default()
        };
        let layer = oci_distribution::client::ImageLayer {
            media_type: super::SIGSTORE_OCI_MEDIA_TYPE.to_string(),
            data: "some other contents".into(),
            annotations: None,
        };

        let rekor_pub_key = get_rekor_public_key();

        let fulcio_cert_pool = get_fulcio_cert_pool();

        let error = SignatureLayer::new(
            &descriptor,
            &layer,
            "source_image_digest is not relevant now",
            Some(&rekor_pub_key),
            Some(&fulcio_cert_pool),
        )
        .expect_err("Didn't get an error");

        let found = match error {
            SigstoreError::SigstoreLayerDigestMismatchError => true,
            _ => false,
        };
        assert!(found, "Got a different error type: {}", error);
    }

    #[test]
    fn get_signature_from_annotations_success() {
        let mut annotations: HashMap<String, String> = HashMap::new();
        annotations.insert(SIGSTORE_SIGNATURE_ANNOTATION.into(), "foo".into());

        let actual = SignatureLayer::get_signature_from_annotations(&annotations);
        assert!(actual.is_ok());
    }

    #[test]
    fn get_signature_from_annotations_failure() {
        let annotations: HashMap<String, String> = HashMap::new();

        let actual = SignatureLayer::get_signature_from_annotations(&annotations);
        assert!(actual.is_err());
    }

    #[test]
    fn get_bundle_from_annotations_works() {
        // we are **not** going to test neither the creation from a valid bundle
        // nor the fauilure because the bundle cannot be verified. These cases
        // are already covered by Bundle's test suite
        //
        // We care only about the only case not tested: to not
        // fail when no bundle is specified.
        let annotations: HashMap<String, String> = HashMap::new();
        let rekor_pub_key = get_rekor_public_key();

        let actual =
            SignatureLayer::get_bundle_from_annotations(&annotations, Some(&rekor_pub_key));
        assert!(actual.is_ok());
        assert!(actual.unwrap().is_none());
    }

    #[test]
    fn get_certificate_signature_from_annotations_returns_none() {
        let annotations: HashMap<String, String> = HashMap::new();
        let fulcio_cert_pool = get_fulcio_cert_pool();

        let actual = SignatureLayer::get_certificate_signature_from_annotations(
            &annotations,
            Some(&fulcio_cert_pool),
            None,
        );

        assert!(actual.unwrap().is_none());
    }

    #[test]
    fn get_certificate_signature_from_annotations_fails_when_no_bundle_is_given() {
        let mut annotations: HashMap<String, String> = HashMap::new();

        // add a fake cert, contents are not relevant
        annotations.insert(SIGSTORE_CERT_ANNOTATION.to_string(), "a cert".to_string());

        let fulcio_cert_pool = get_fulcio_cert_pool();

        let error = SignatureLayer::get_certificate_signature_from_annotations(
            &annotations,
            Some(&fulcio_cert_pool),
            None,
        )
        .expect_err("Didn't get an error");

        assert!(matches!(
            error,
            SigstoreError::SigstoreRekorBundleNotFoundError
        ));
    }

    #[test]
    fn get_certificate_signature_from_annotations_fails_when_no_fulcio_pub_key_is_given() {
        let mut annotations: HashMap<String, String> = HashMap::new();

        // add a fake cert, contents are not relevant
        annotations.insert(SIGSTORE_CERT_ANNOTATION.to_string(), "a cert".to_string());

        let bundle = build_bundle();

        let error = SignatureLayer::get_certificate_signature_from_annotations(
            &annotations,
            None,
            Some(&bundle),
        )
        .expect_err("Didn't get an error");

        assert!(matches!(
            error,
            SigstoreError::SigstoreFulcioCertificatesNotProvidedError
        ));
    }

    #[test]
    fn is_signed_by_key() {
        // a SignatureLayer created with traditional signing
        let (sl, key) = build_correct_signature_layer_without_bundle();
        assert!(sl.is_signed_by_key(&key));

        // a SignatureLayer created with keyless signing -> there's no pub key
        let sl = build_correct_signature_layer_with_certificate();

        // fail because the signature layer wasn't signed with the given key
        let verification_key = CosignVerificationKey::from_pem(
            r#"-----BEGIN PUBLIC KEY-----
MFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAETJP9cqpUQsn2ggmJniWGjHdlsHzD
JsB89BPhZYch0U0hKANx5TY+ncrm0s8bfJxxHoenAEFhwhuXeb4PqIrtoQ==
-----END PUBLIC KEY-----"#
                .as_bytes(),
            SignatureDigestAlgorithm::default(),
        )
        .expect("Cannot create CosignVerificationKey");
        assert!(!sl.is_signed_by_key(&verification_key));
    }

    // Testing CertificateSignature
    use crate::cosign::bundle::Payload;
    use crate::crypto::tests::{generate_certificate, CertGenerationOptions};
    use chrono::{Duration, Utc};

    impl TryFrom<X509> for crate::registry::Certificate {
        type Error = anyhow::Error;

        fn try_from(value: X509) -> std::result::Result<Self, Self::Error> {
            let data = value.to_pem()?;
            let encoding = crate::registry::CertificateEncoding::Pem;
            Ok(Self { data, encoding })
        }
    }

    #[test]
    fn certificate_signature_from_certificate_using_email() -> anyhow::Result<()> {
        let expected_email = "test@sigstore.dev".to_string();
        let ca_data = generate_certificate(None, CertGenerationOptions::default())?;

        let issued_cert = generate_certificate(
            Some(&ca_data),
            CertGenerationOptions {
                subject_email: Some(expected_email.clone()),
                ..Default::default()
            },
        )?;

        let issued_cert_pem = issued_cert.cert.to_pem()?;

        let certs = vec![crate::registry::Certificate::try_from(ca_data.cert).unwrap()];
        let cert_pool = CertificatePool::from_certificates(&certs).unwrap();

        let integrated_time = Utc::now().checked_sub_signed(Duration::minutes(1)).unwrap();
        let bundle = Bundle {
            signed_entry_timestamp: "not relevant".to_string(),
            payload: Payload {
                body: "not relevant".to_string(),
                integrated_time: integrated_time.timestamp(),
                log_index: 0,
                log_id: "not relevant".to_string(),
            },
        };

        let certificate_signature =
            CertificateSignature::from_certificate(&issued_cert_pem, &cert_pool, &bundle)
                .expect("Didn't expect an error");

        let expected_issuer = match certificate_signature.subject.clone() {
            CertificateSubject::Email(mail) => mail == expected_email,
            _ => false,
        };
        assert!(
            expected_issuer,
            "Didn't get the expected subject: {:?}",
            certificate_signature.subject
        );

        Ok(())
    }

    #[test]
    fn certificate_signature_from_certificate_using_uri() -> anyhow::Result<()> {
        let expected_url = "https://sigstore.dev/test".to_string();
        let ca_data = generate_certificate(None, CertGenerationOptions::default())?;

        let issued_cert = generate_certificate(
            Some(&ca_data),
            CertGenerationOptions {
                subject_email: None,
                subject_url: Some(expected_url.clone()),
                ..Default::default()
            },
        )?;

        let issued_cert_pem = issued_cert.cert.to_pem()?;

        let certs = vec![crate::registry::Certificate::try_from(ca_data.cert).unwrap()];
        let cert_pool = CertificatePool::from_certificates(&certs).unwrap();

        let integrated_time = Utc::now().checked_sub_signed(Duration::minutes(1)).unwrap();
        let bundle = Bundle {
            signed_entry_timestamp: "not relevant".to_string(),
            payload: Payload {
                body: "not relevant".to_string(),
                integrated_time: integrated_time.timestamp(),
                log_index: 0,
                log_id: "not relevant".to_string(),
            },
        };

        let certificate_signature =
            CertificateSignature::from_certificate(&issued_cert_pem, &cert_pool, &bundle)
                .expect("Didn't expect an error");

        let expected_issuer = match certificate_signature.subject.clone() {
            CertificateSubject::Uri(url) => url == expected_url,
            _ => false,
        };
        assert!(
            expected_issuer,
            "Didn't get the expected subject: {:?}",
            certificate_signature.subject
        );

        Ok(())
    }

    #[test]
    fn certificate_signature_from_certificate_without_email_and_uri() -> anyhow::Result<()> {
        let ca_data = generate_certificate(None, CertGenerationOptions::default())?;

        let issued_cert = generate_certificate(
            Some(&ca_data),
            CertGenerationOptions {
                subject_email: None,
                subject_url: None,
                ..Default::default()
            },
        )?;

        let issued_cert_pem = issued_cert.cert.to_pem()?;

        let certs = vec![crate::registry::Certificate::try_from(ca_data.cert).unwrap()];
        let cert_pool = CertificatePool::from_certificates(&certs).unwrap();

        let integrated_time = Utc::now().checked_sub_signed(Duration::minutes(1)).unwrap();
        let bundle = Bundle {
            signed_entry_timestamp: "not relevant".to_string(),
            payload: Payload {
                body: "not relevant".to_string(),
                integrated_time: integrated_time.timestamp(),
                log_index: 0,
                log_id: "not relevant".to_string(),
            },
        };

        let error = CertificateSignature::from_certificate(&issued_cert_pem, &cert_pool, &bundle)
            .expect_err("Didn't get an error");
        assert!(matches!(
            error,
            SigstoreError::CertificateWithoutSubjectAlternativeName
        ));

        Ok(())
    }
}
