// Copyright (c) 2022 Alibaba Cloud
//
// SPDX-License-Identifier: Apache-2.0
//

//! Cosign verification

use std::{collections::HashMap, path::Path};

use anyhow::{anyhow, bail, Result};
use async_trait::async_trait;
use oci_distribution::secrets::RegistryAuth;
use serde::{Deserialize, Serialize};
use sigstore::cosign::verification_constraint::{PublicKeyVerifier, VerificationConstraintVec};
use sigstore::cosign::{verify_constraints, ClientBuilder, CosignCapabilities};
use sigstore::crypto::SignatureDigestAlgorithm;
use sigstore::errors::SigstoreVerifyConstraintsError;
use sigstore::registry::Auth;
use tokio::fs;

use super::SignScheme;
use crate::config::Paths;
use crate::signature::{
    image::Image, payload::simple_signing::SigPayload, policy::ref_match::PolicyReqMatchType,
};

/// The name of resource to request cosign verification key from kbs
pub const COSIGN_KEY_KBS: &str = "Cosign Key";

#[derive(Deserialize, Debug, Eq, PartialEq, Serialize, Default)]
pub struct CosignParameters {
    // KeyPath is a pathname to a local file containing the trusted key(s).
    // Exactly one of KeyPath and KeyData can be specified.
    //
    // This field is optional.
    #[serde(rename = "keyPath")]
    pub key_path: Option<String>,
    // KeyData contains the trusted key(s), base64-encoded.
    // Exactly one of KeyPath and KeyData can be specified.
    //
    // This field is optional.
    #[serde(rename = "keyData")]
    pub key_data: Option<String>,

    // SignedIdentity specifies what image identity the signature must be claiming about the image.
    // Defaults to "match-exact" if not specified.
    //
    // This field is optional.
    #[serde(default, rename = "signedIdentity")]
    pub signed_identity: Option<PolicyReqMatchType>,

    /// Dir for storage of cosign verification keys
    #[serde(skip)]
    pub cosign_key_dir: String,
}

#[async_trait]
impl SignScheme for CosignParameters {
    /// This initialization will:
    /// * Create [`COSIGN_KEY_DIR`] if not exist.
    async fn init(&mut self, config: &Paths) -> Result<()> {
        self.cosign_key_dir = config.cosign_key_dir.clone();
        if !Path::new(&self.cosign_key_dir).exists() {
            fs::create_dir_all(&self.cosign_key_dir)
                .await
                .map_err(|e| {
                    anyhow!("Create Simple Signing sigstore-config dir failed: {:?}", e)
                })?;
        }

        Ok(())
    }

    fn resource_manifest(&self) -> HashMap<&str, &str> {
        let mut manifest_from_kbs = HashMap::new();
        if let Some(key_path) = &self.key_path {
            if !Path::new(key_path).exists() {
                manifest_from_kbs.insert(COSIGN_KEY_KBS, &key_path[..]);
            }
        }
        manifest_from_kbs
    }

    /// Judge whether an image is allowed by this SignScheme.
    async fn allows_image(&self, image: &mut Image, auth: &RegistryAuth) -> Result<()> {
        // Check before we access the network
        self.check_reference_rule_types()?;

        // Verification, will access the network
        let payloads = self.verify_signature_and_get_payload(image, auth).await?;

        // check the reference rules (signed identity)
        for payload in payloads {
            if let Some(rule) = &self.signed_identity {
                payload.validate_signed_docker_reference(&image.reference, rule)?;
            }

            payload.validate_signed_docker_manifest_digest(&image.manifest_digest.to_string())?;
        }

        Ok(())
    }
}

impl CosignParameters {
    /// Check whether this Policy Request Match Type (i.e., signed identity
    /// check type) for the reference is MatchRepository or ExactRepository.
    /// Because cosign-created signatures only contain a repository,
    /// so only matchRepository and exactRepository can be used to accept them.
    /// Other types are all to be denied.
    /// If it is neither of them, return `Error`. Otherwise, return `Ok()`
    fn check_reference_rule_types(&self) -> Result<()> {
        match &self.signed_identity {
            Some(rule) => match rule {
                PolicyReqMatchType::MatchRepository
                | PolicyReqMatchType::ExactRepository { .. } => Ok(()),
                p => Err(anyhow!("Denied by {:?}", p)),
            },
            None => Ok(()),
        }
    }

    /// Verify the cosign-signed image. There will be three steps:
    /// * Get the pub key.
    /// * Download the cosign-signed image's manifest and its digest. Calculate its
    /// signature's image.
    /// * Download the signature image, gather the signatures and verify them
    /// using the pubkey.
    /// If succeeds, the payloads of the signature will be returned.
    async fn verify_signature_and_get_payload(
        &self,
        image: &mut Image,
        auth: &RegistryAuth,
    ) -> Result<Vec<SigPayload>> {
        // Get the pubkey
        let key = match (&self.key_data, &self.key_path) {
            (None, None) => bail!("Neither keyPath nor keyData is specified."),
            (None, Some(key_path)) => read_key_from(key_path).await?,
            (Some(key_data), None) => key_data.as_bytes().to_vec(),
            (Some(_), Some(_)) => bail!("Both keyPath and keyData are specified."),
        };

        let mut client = ClientBuilder::default().build()?;

        let auth = &Auth::from(auth);
        let image_ref = image.reference.whole();

        // Get the cosign signature "image"'s uri and the signed image's digest
        let (cosign_image, source_image_digest) = client.triangulate(&image_ref, auth).await?;

        // Get the signature layers in cosign signature "image"'s manifest
        let signature_layers = client
            .trusted_signature_layers(auth, &source_image_digest, &cosign_image)
            .await?;

        // By default, the hashing algorithm is SHA256
        let pub_key_verifier = PublicKeyVerifier::new(&key, SignatureDigestAlgorithm::Sha256)?;

        let verification_constraints: VerificationConstraintVec = vec![Box::new(pub_key_verifier)];

        let res = verify_constraints(&signature_layers, verification_constraints.iter());

        match res {
            Ok(()) => {
                // gather the payloads
                let payloads = signature_layers
                    .iter()
                    .map(|layer| SigPayload::from(layer.simple_signing.clone()))
                    .collect();
                Ok(payloads)
            }
            Err(SigstoreVerifyConstraintsError {
                unsatisfied_constraints,
            }) => Err(anyhow!("{:?}", unsatisfied_constraints)),
        }
    }
}

async fn read_key_from(path: &str) -> Result<Vec<u8>> {
    // TODO: Do we need define a new URL scheme
    // named `kbs://` to indicate that the key
    // should be got from kbs? This would be
    // helpful for issue
    // <https://github.com/confidential-containers/image-rs/issues/9>
    Ok(fs::read(path).await?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::signature::{
        mechanism::SignScheme,
        policy::{policy_requirement::PolicyReqType, ref_match::PolicyReqMatchType},
    };

    use std::convert::TryFrom;

    use oci_distribution::Reference;
    use rstest::rstest;
    use serial_test::serial;

    // All the test images are the same image, but different
    // registry and repository
    const IMAGE_DIGEST: &str =
        "sha256:7bd0c945d7e4cc2ce5c21d449ba07eb89c8e6c28085edbcf6f5fa4bf90e7eedc";

    #[rstest]
    #[case(
        CosignParameters{
            key_path: Some("test_data/signature/cosign/cosign1.pub".into()),
            key_data: None,
            signed_identity: None,
            cosign_key_dir: crate::config::COSIGN_KEY_DIR.into(),
        },
        "registry.cn-hangzhou.aliyuncs.com/xynnn/cosign:latest",
    )]
    #[case(
        CosignParameters{
            key_path: Some("test_data/signature/cosign/cosign1.pub".into()),
            key_data: None,
            signed_identity: None,
            cosign_key_dir: crate::config::COSIGN_KEY_DIR.into(),
        },
        "registry-1.docker.io/xynnn007/cosign:latest",
    )]
    #[case(
        CosignParameters{
            key_path: Some("test_data/signature/cosign/cosign1.pub".into()),
            key_data: None,
            signed_identity: None,
            cosign_key_dir: crate::config::COSIGN_KEY_DIR.into(),
        },
        "quay.io/kata-containers/confidential-containers:cosign-signed",
    )]
    #[tokio::test]
    #[serial]
    async fn verify_signature_and_get_payload_test(
        #[case] parameter: CosignParameters,
        #[case] image_reference: &str,
    ) {
        let reference =
            Reference::try_from(image_reference).expect("deserialize OCI Reference failed.");
        let mut image = Image::default_with_reference(reference);
        image
            .set_manifest_digest(IMAGE_DIGEST)
            .expect("Set manifest digest failed.");
        assert!(
            parameter
                .verify_signature_and_get_payload(
                    &mut image,
                    &oci_distribution::secrets::RegistryAuth::Anonymous
                )
                .await
                .is_ok(),
            "failed test:\nparameter:{:?}\nimage reference:{}",
            parameter,
            image_reference
        );
    }

    #[rstest]
    #[case(PolicyReqMatchType::MatchExact, false)]
    #[case(PolicyReqMatchType::MatchRepoDigestOrExact, false)]
    #[case(PolicyReqMatchType::MatchRepository, true)]
    #[case(PolicyReqMatchType::ExactReference{docker_reference: "".into()}, false)]
    #[case(PolicyReqMatchType::ExactRepository{docker_repository: "".into()}, true)]
    #[case(PolicyReqMatchType::RemapIdentity{prefix:"".into(), signed_prefix:"".into()}, false)]
    fn check_reference_rule_types_test(
        #[case] policy_match: PolicyReqMatchType,
        #[case] pass: bool,
    ) {
        let parameter = CosignParameters {
            key_path: None,
            key_data: None,
            signed_identity: Some(policy_match),
            cosign_key_dir: crate::config::COSIGN_KEY_DIR.into(),
        };
        assert_eq!(parameter.check_reference_rule_types().is_ok(), pass);
    }

    #[rstest]
    #[case(
        r#"{
            "type": "sigstoreSigned",
            "keyPath": "test_data/signature/cosign/cosign2.pub"
        }"#, 
        "registry.cn-hangzhou.aliyuncs.com/xynnn/cosign:latest",
        false,
        // If verified failed, the pubkey given to verify will be printed.
        "[PublicKeyVerifier { key: CosignVerificationKey { verification_algorithm: ECDSA_P256_SHA256_ASN1, data: [4, 192, 146, 124, 21, 74, 44, 46, 129, 189, 211, 135, 35, 87, 145, 71, 172, 25, 92, 98, 102, 245, 109, 29, 191, 50, 55, 236, 233, 47, 136, 66, 124, 253, 181, 135, 68, 180, 68, 84, 60, 97, 97, 147, 39, 218, 80, 228, 49, 224, 66, 101, 2, 236, 78, 109, 162, 5, 171, 119, 141, 234, 112, 247, 247] } }]"
    )]
    #[case(
        r#"{
            "type": "sigstoreSigned",
            "keyPath": "test_data/signature/cosign/cosign1.pub",
            "signedIdentity": {
                "type": "exactRepository",
                "dockerRepository": "registry-1.docker.io/xynnn007/cosign-err"
            }
        }"#,
        // The repository of the given image's and the Payload's are different
        "registry-1.docker.io/xynnn007/cosign:latest",
        false,
        "Match reference failed.",
    )]
    #[case(
        r#"{
            "type": "sigstoreSigned",
            "keyPath": "test_data/signature/cosign/cosign2.pub"
        }"#,
        "quay.io/kata-containers/confidential-containers:cosign-signed",
        false,
        // If verified failed, the pubkey given to verify will be printed.
        "[PublicKeyVerifier { key: CosignVerificationKey { verification_algorithm: ECDSA_P256_SHA256_ASN1, data: [4, 192, 146, 124, 21, 74, 44, 46, 129, 189, 211, 135, 35, 87, 145, 71, 172, 25, 92, 98, 102, 245, 109, 29, 191, 50, 55, 236, 233, 47, 136, 66, 124, 253, 181, 135, 68, 180, 68, 84, 60, 97, 97, 147, 39, 218, 80, 228, 49, 224, 66, 101, 2, 236, 78, 109, 162, 5, 171, 119, 141, 234, 112, 247, 247] } }]",
    )]
    #[case(
        r#"{
            "type": "sigstoreSigned",
            "keyPath": "test_data/signature/cosign/cosign1.pub",
            "signedIdentity": {
                "type" : "matchExact"
            }
        }"#,
        "quay.io/kata-containers/confidential-containers:cosign-signed",
        false,
        // Only MatchRepository and ExactRepository are supported.
        "Denied by MatchExact",
    )]
    #[case(
        r#"{
            "type": "sigstoreSigned",
            "keyPath": "test_data/signature/cosign/cosign1.pub"
        }"#,
        "registry.cn-hangzhou.aliyuncs.com/xynnn/cosign:signed",
        true,
        ""
    )]
    #[case(
        r#"{
            "type": "sigstoreSigned",
            "keyPath": "test_data/signature/cosign/cosign1.pub"
        }"#,
        "registry-1.docker.io/xynnn007/cosign:latest",
        true,
        ""
    )]
    #[case(
        r#"{
            "type": "sigstoreSigned",
            "keyPath": "test_data/signature/cosign/cosign1.pub"
        }"#,
        "quay.io/kata-containers/confidential-containers:cosign-signed",
        true,
        ""
    )]
    #[tokio::test]
    #[serial]
    async fn verify_signature(
        #[case] policy: &str,
        #[case] image_reference: &str,
        #[case] allow: bool,
        #[case] failed_reason: &str,
    ) {
        let policy_requirement: PolicyReqType =
            serde_json::from_str(policy).expect("deserialize PolicyReqType failed.");
        let reference = oci_distribution::Reference::try_from(image_reference)
            .expect("deserialize OCI Reference failed.");

        let mut image = Image::default_with_reference(reference);
        image
            .set_manifest_digest(IMAGE_DIGEST)
            .expect("Set manifest digest failed.");

        if let PolicyReqType::Cosign(scheme) = policy_requirement {
            let res = scheme
                .allows_image(
                    &mut image,
                    &oci_distribution::secrets::RegistryAuth::Anonymous,
                )
                .await;
            assert_eq!(
                res.is_ok(),
                allow,
                "test failed: \nimage: {}\npolicy:{}",
                image_reference,
                policy
            );
            if !allow {
                let err_msg = res.unwrap_err().to_string();
                assert_eq!(
                    err_msg, failed_reason,
                    "test failed: failed reason unmatched.\nneed:{}\ngot:{}",
                    failed_reason, err_msg
                );
            }
        } else {
            panic!("Must be a sigstoreSigned policy!");
        }
    }
}
