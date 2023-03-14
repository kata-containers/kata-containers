// Copyright (c) 2022 Alibaba Cloud
//
// SPDX-License-Identifier: Apache-2.0
//

//! Payload format of simple signing

use std::collections::HashMap;

use anyhow::{bail, Result};
use oci_distribution::Reference;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::signature::policy::ref_match::PolicyReqMatchType;

// The spec of SigPayload is defined in https://github.com/containers/image/blob/main/docs/containers-signature.5.md.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SigPayload {
    pub critical: Critical,
    pub optional: Option<Optional>,
}

impl SigPayload {
    // Compare wether the docker reference in the JSON payload
    // is consistent with that of the container image.
    pub fn validate_signed_docker_reference(
        &self,
        image_ref: &Reference,
        match_policy: &PolicyReqMatchType,
    ) -> Result<()> {
        match_policy.matches_docker_reference(image_ref, &self.docker_reference())
    }

    // Compare wether the manifest digest in the JSON payload
    // is consistent with that of the container image.
    pub fn validate_signed_docker_manifest_digest(&self, ref_manifest_digest: &str) -> Result<()> {
        if self.manifest_digest() != ref_manifest_digest {
            bail!(
                "SigPayload's manifest digest does not match, the input is {}, but in SigPayload it is {}",
                &ref_manifest_digest,
                &self.manifest_digest()
            );
        }
        Ok(())
    }

    fn manifest_digest(&self) -> String {
        self.critical.image.docker_manifest_digest.clone()
    }

    fn docker_reference(&self) -> String {
        self.critical.identity.docker_reference.clone()
    }
}

#[cfg(feature = "signature-cosign")]
impl From<sigstore::simple_signing::SimpleSigning> for SigPayload {
    fn from(s: sigstore::simple_signing::SimpleSigning) -> Self {
        Self {
            critical: Critical {
                type_name: s.critical.type_name,
                image: Image {
                    docker_manifest_digest: s.critical.image.docker_manifest_digest,
                },
                identity: Identity {
                    docker_reference: s.critical.identity.docker_reference,
                },
            },
            optional: s.optional.map(|opt| Optional {
                creator: opt.creator,
                timestamp: opt.timestamp,
                extra: opt.extra,
            }),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
/// A JSON object which contains data critical to correctly evaluating the validity of a signature.
pub struct Critical {
    #[serde(rename = "type")]
    pub type_name: String,
    pub image: Image,
    pub identity: Identity,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "kebab-case")]
/// A JSON object which identifies the container image this signature applies to.
pub struct Image {
    /// A JSON string, in the github.com/opencontainers/go-digest.Digest string format.
    pub docker_manifest_digest: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "kebab-case")]
/// A JSON object which identifies the claimed identity of the image
/// (usually the purpose of the image, or the application, along with a version information),
/// as asserted by the author of the signature.
pub struct Identity {
    /// A JSON string, in the github.com/docker/distribution/reference string format,
    /// and using the same normalization semantics
    /// (where e.g. busybox:latest is equivalent to docker.io/library/busybox:latest).
    /// If the normalization semantics allows multiple string representations
    /// of the claimed identity with equivalent meaning,
    /// the critical.identity.docker-reference member SHOULD use the fully explicit form
    /// (including the full host name and namespaces).
    pub docker_reference: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Optional {
    pub creator: Option<String>,
    pub timestamp: Option<i64>,

    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use serde_json::json;

    use crate::signature::payload::simple_signing::{Critical, Identity, Image, Optional};

    use super::SigPayload;

    #[test]
    fn serialize_simple_signing_payload() {
        let json = json!({
            "critical": {
                "identity": {
                    "docker-reference": "quay.io/ali_os_security/alpine:latest"
                },
                  "image": {
                    "docker-manifest-digest": "sha256:69704ef328d05a9f806b6b8502915e6a0a4faa4d72018dc42343f511490daf8a"
                },
                  "type": "atomic container signature"
            },
            "optional": {
                "creator": "atomic 2.0.0",
                "timestamp": 1634533638
            }
        });

        let payload = SigPayload {
            critical: Critical {
                type_name: "atomic container signature".into(),
                image: Image {
                    docker_manifest_digest:
                        "sha256:69704ef328d05a9f806b6b8502915e6a0a4faa4d72018dc42343f511490daf8a"
                            .into(),
                },
                identity: Identity {
                    docker_reference: "quay.io/ali_os_security/alpine:latest".into(),
                },
            },
            optional: Some(Optional {
                creator: Some("atomic 2.0.0".into()),
                timestamp: Some(1634533638),
                extra: HashMap::new(),
            }),
        };

        let payload_serialize = serde_json::to_value(&payload).unwrap();
        assert_eq!(payload_serialize, json);
    }

    #[test]
    fn deserialize_simple_signing_payload() {
        let json = r#"{
            "critical": {
                "identity": {
                    "docker-reference": "quay.io/ali_os_security/alpine:latest"
                },
                  "image": {
                    "docker-manifest-digest": "sha256:69704ef328d05a9f806b6b8502915e6a0a4faa4d72018dc42343f511490daf8a"
                },
                  "type": "atomic container signature"
            },
            "optional": {
                "creator": "atomic 2.0.0",
                "timestamp": 1634533638
            }
        }"#;

        // Because the `PartialEq` trait is not derived, we can only do the
        // comparation one by one.
        let deserialized_payload: SigPayload = serde_json::from_str(json).unwrap();
        assert_eq!(
            deserialized_payload.critical.identity.docker_reference,
            "quay.io/ali_os_security/alpine:latest"
        );
        assert_eq!(
            deserialized_payload.critical.image.docker_manifest_digest,
            "sha256:69704ef328d05a9f806b6b8502915e6a0a4faa4d72018dc42343f511490daf8a"
        );
        assert_eq!(
            deserialized_payload.critical.type_name,
            "atomic container signature"
        );
        assert_eq!(
            deserialized_payload.optional.as_ref().unwrap().creator,
            Some("atomic 2.0.0".into())
        );
        assert_eq!(
            deserialized_payload.optional.as_ref().unwrap().timestamp,
            Some(1634533638)
        );
        assert_eq!(
            deserialized_payload.optional.as_ref().unwrap().extra,
            HashMap::new()
        );
    }
}
