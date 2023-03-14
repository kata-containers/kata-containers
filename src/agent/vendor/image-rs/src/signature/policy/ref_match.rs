// Copyright (c) 2022 Alibaba Cloud
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{bail, Result};
use oci_distribution::Reference;
use serde::*;
use std::convert::TryFrom;

use crate::signature::{image, policy::ErrorInfo};

/// The `signedIdentity` field in simple signing. It is a JSON object, specifying what image
/// identity the signature claims about the image.
#[derive(Deserialize, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "type")]
pub enum PolicyReqMatchType {
    /// `matchExact` match type : the two references must match exactly.
    #[serde(rename = "matchExact")]
    MatchExact,

    /// `matchRepoDigestOrExact` match type: the two references must match exactly,
    /// except that digest references are also accepted
    /// if the repository name matches (regardless of tag/digest)
    /// and the signature applies to the referenced digest.
    #[serde(rename = "matchRepoDigestOrExact")]
    MatchRepoDigestOrExact,

    /// `matchRepository` match type: the two references must use the same repository, may differ in the tag.
    #[serde(rename = "matchRepository")]
    MatchRepository,

    /// `exactReference` match type: matches a specified reference exactly.
    #[serde(rename = "exactReference")]
    ExactReference {
        #[serde(rename = "dockerReference")]
        docker_reference: String,
    },

    /// `exactRepository` match type: matches a specified repository, with any tag.
    #[serde(rename = "exactRepository")]
    ExactRepository {
        #[serde(rename = "dockerRepository")]
        docker_repository: String,
    },

    /// `remapIdentity` match type:
    /// except that a namespace (at least a host:port, at most a single repository)
    /// is substituted before matching the two references.
    #[serde(rename = "remapIdentity")]
    RemapIdentity {
        prefix: String,
        #[serde(rename = "signedPrefix")]
        signed_prefix: String,
    },
}

// PolicyReferenceMatch specifies a set of image identities(image-reference) accepted in PolicyRequirement.
impl PolicyReqMatchType {
    /// Return a default match policy
    pub fn default_match_policy() -> PolicyReqMatchType {
        PolicyReqMatchType::MatchExact
    }

    /// Check whether matches reference
    pub fn matches_docker_reference(
        &self,
        origin: &Reference,
        signed_image_ref: &str,
    ) -> Result<()> {
        match self {
            PolicyReqMatchType::MatchExact => {
                if origin.digest().is_some() {
                    bail!("Can not reference the image with the digest in matchExact policy.",);
                }
                if origin.whole() != *signed_image_ref {
                    bail!(ErrorInfo::MatchReference.to_string());
                }
            }
            PolicyReqMatchType::MatchRepoDigestOrExact => {
                if origin.tag().is_some() && origin.whole() != *signed_image_ref {
                    bail!(ErrorInfo::MatchReference.to_string());
                }
                if origin.digest().is_some()
                    && image::get_image_repository_full_name(origin)
                        != image::get_image_repository_full_name(&Reference::try_from(
                            signed_image_ref,
                        )?)
                {
                    bail!(ErrorInfo::MatchReference.to_string());
                }
            }
            PolicyReqMatchType::MatchRepository => {
                if image::get_image_repository_full_name(origin)
                    != image::get_image_repository_full_name(&Reference::try_from(
                        signed_image_ref,
                    )?)
                {
                    bail!(ErrorInfo::MatchReference.to_string());
                }
            }
            PolicyReqMatchType::ExactReference { docker_reference } => {
                if signed_image_ref != docker_reference {
                    bail!(ErrorInfo::MatchReference.to_string());
                }
            }
            PolicyReqMatchType::ExactRepository { docker_repository } => {
                if image::get_image_repository_full_name(&Reference::try_from(signed_image_ref)?)
                    != *docker_repository
                {
                    bail!(ErrorInfo::MatchReference.to_string());
                }
            }
            PolicyReqMatchType::RemapIdentity {
                prefix,
                signed_prefix,
            } => {
                let mut origin_ref_string = origin.whole();

                if let Some(ref_with_no_prefix) = origin_ref_string.strip_prefix(prefix) {
                    origin_ref_string = format!("{signed_prefix}{ref_with_no_prefix}");
                }

                let new_origin_ref = Reference::try_from(origin_ref_string.as_str())?;

                if new_origin_ref.tag().is_some() && new_origin_ref.whole() != *signed_image_ref {
                    bail!(ErrorInfo::MatchReference.to_string());
                }
                if new_origin_ref.digest().is_some()
                    && image::get_image_repository_full_name(&new_origin_ref)
                        != image::get_image_repository_full_name(&Reference::try_from(
                            signed_image_ref,
                        )?)
                {
                    bail!(ErrorInfo::MatchReference.to_string());
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::convert::TryFrom;

    use crate::signature::policy::ref_match::PolicyReqMatchType;

    #[test]
    fn test_policy_matches_docker_reference() {
        struct TestData<'a> {
            match_policy: PolicyReqMatchType,
            origin_reference: oci_distribution::Reference,
            signed_reference: &'a str,
        }

        let tests_expect = &[
            TestData {
                match_policy: serde_json::from_str::<PolicyReqMatchType>(
                    r#"{
                        "type": "matchExact"
                    }"#
                ).unwrap(),
                origin_reference: oci_distribution::Reference::try_from("docker.io/example/busybox:latest").unwrap(),
                signed_reference: "docker.io/example/busybox:latest",
            },
            TestData {
                match_policy: serde_json::from_str::<PolicyReqMatchType>(
                    r#"{
                        "type": "matchRepoDigestOrExact"
                    }"#
                ).unwrap(),
                origin_reference: oci_distribution::Reference::try_from("docker.io/example/busybox:latest").unwrap(),
                signed_reference: "docker.io/example/busybox:latest",
            },
            TestData {
                match_policy: serde_json::from_str::<PolicyReqMatchType>(
                    r#"{
                        "type": "matchRepoDigestOrExact"
                    }"#
                ).unwrap(),
                origin_reference: oci_distribution::Reference::try_from(
                    "docker.io/example/busybox@sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"
                ).unwrap(),
                signed_reference: "docker.io/example/busybox:tag",
            },
            TestData {
                match_policy: serde_json::from_str::<PolicyReqMatchType>(
                    r#"{
                        "type": "matchRepository"
                    }"#
                ).unwrap(),
                origin_reference: oci_distribution::Reference::try_from("docker.io/example/busybox:latest").unwrap(),
                signed_reference: "docker.io/example/busybox:tag",
            },
            TestData {
                match_policy: serde_json::from_str::<PolicyReqMatchType>(
                    r#"{
                        "type": "exactReference",
                        "dockerReference": "docker.io/mylib/busybox:latest"
                    }"#
                ).unwrap(),
                origin_reference: oci_distribution::Reference::try_from("docker.io/example/busybox:latest").unwrap(),
                signed_reference: "docker.io/mylib/busybox:latest",
            },
            TestData {
                match_policy: serde_json::from_str::<PolicyReqMatchType>(
                    r#"{
                        "type": "exactRepository",
                        "dockerRepository": "docker.io/mylib/busybox"
                    }"#
                ).unwrap(),
                origin_reference: oci_distribution::Reference::try_from("docker.io/example/busybox:latest").unwrap(),
                signed_reference: "docker.io/mylib/busybox:tag",
            },
            TestData {
                match_policy: serde_json::from_str::<PolicyReqMatchType>(
                    r#"{
                        "type": "remapIdentity",
                        "prefix": "docker.io",
                        "signedPrefix": "quay.io"
                    }"#
                ).unwrap(),
                origin_reference: oci_distribution::Reference::try_from("docker.io/example/busybox:latest").unwrap(),
                signed_reference: "quay.io/example/busybox:latest",
            },
        ];

        let tests_unexpect = &[
            TestData {
                match_policy: serde_json::from_str::<PolicyReqMatchType>(
                    r#"{
                        "type": "matchExact"
                    }"#
                ).unwrap(),
                origin_reference: oci_distribution::Reference::try_from("docker.io/example/busybox:latest").unwrap(),
                signed_reference: "docker.io/example/busybox:tag",
            },
            TestData {
                match_policy: serde_json::from_str::<PolicyReqMatchType>(
                    r#"{
                        "type": "matchExact"
                    }"#
                ).unwrap(),
                origin_reference: oci_distribution::Reference::try_from(
                    "docker.io/example/busybox@sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"
                ).unwrap(),
                signed_reference: "docker.io/example/busybox@sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
            },
            TestData {
                match_policy: serde_json::from_str::<PolicyReqMatchType>(
                    r#"{
                        "type": "matchRepoDigestOrExact"
                    }"#
                ).unwrap(),
                origin_reference: oci_distribution::Reference::try_from("docker.io/example/busybox:latest").unwrap(),
                signed_reference: "docker.io/example/busybox:tag",
            },
            TestData {
                match_policy: serde_json::from_str::<PolicyReqMatchType>(
                    r#"{
                        "type": "exactReference",
                        "dockerReference": "docker.io/mylib/busybox:latest"
                    }"#
                ).unwrap(),
                origin_reference: oci_distribution::Reference::try_from("docker.io/example/busybox:latest").unwrap(),
                signed_reference: "docker.io/example/busybox:latest",
            },
        ];

        for test_case in tests_expect.iter() {
            assert!(test_case
                .match_policy
                .matches_docker_reference(&test_case.origin_reference, test_case.signed_reference)
                .is_ok());
        }

        for test_case in tests_unexpect.iter() {
            assert!(test_case
                .match_policy
                .matches_docker_reference(&test_case.origin_reference, test_case.signed_reference)
                .is_err());
        }
    }
}
