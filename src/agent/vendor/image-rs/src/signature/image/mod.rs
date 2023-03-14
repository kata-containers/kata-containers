// Copyright (c) 2022 Alibaba Cloud
//
// SPDX-License-Identifier: Apache-2.0
//

use std::convert::TryFrom;

use anyhow::*;
use oci_distribution::Reference;

pub mod digest;

use digest::Digest;
use strum::{Display, EnumString};

#[derive(EnumString, Display, Debug, PartialEq, Eq)]
pub enum TransportName {
    #[strum(to_string = "docker")]
    Docker,
    #[strum(to_string = "dir")]
    Dir,
}

// Image contains information about the image which may be used in signature verification.
pub struct Image {
    pub reference: Reference,
    // digest format: "digest-algorithm:digest-value"
    pub manifest_digest: Digest,
}

impl Image {
    pub fn default_with_reference(image_ref: Reference) -> Self {
        Image {
            reference: image_ref,
            manifest_digest: Digest::default(),
        }
    }

    pub fn transport_name(&self) -> String {
        // FIXME: Now only support "docker" transport (and it is hardcoded).
        // TODO: support "dir" transport.
        //
        // issue: https://github.com/confidential-containers/image-rs/issues/11
        TransportName::Docker.to_string()
    }

    pub fn set_manifest_digest(&mut self, digest: &str) -> Result<()> {
        self.manifest_digest = Digest::try_from(digest)?;
        Ok(())
    }
}

// Get repository full name:
// `registry-name/repository-name`
pub fn get_image_repository_full_name(image_ref: &Reference) -> String {
    if image_ref.registry().is_empty() {
        image_ref.repository().to_string()
    } else {
        format!("{}/{}", image_ref.registry(), image_ref.repository())
    }
}

// Returns a list of other policy configuration namespaces to search.
pub fn get_image_namespaces(image_ref: &Reference) -> Vec<String> {
    // Look for a match of the repository, and then of the possible parent
    // namespaces. Note that this only happens on the expanded host names
    // and repository names, i.e. "busybox" is looked up as "docker.io/library/busybox",
    // then in its parent "docker.io/library"; in none of "busybox",
    // un-namespaced "library" nor in "" supposedly implicitly representing "library/".
    //
    // image_full_name == host_name + "/" + repository_name, so the last
    // iteration matches the host name (for any namespace).
    let mut res = Vec::new();
    let mut name: String = get_image_repository_full_name(image_ref);

    loop {
        res.push(name.clone());
        match name.rsplit_once('/') {
            None => break,
            Some(n) => {
                name = n.0.to_string();
            }
        }
    }

    // Strip port number if any, before appending to res slice.
    // Currently, the most compatible behavior is to return
    // example.com:8443/ns, example.com:8443, *.com.
    // If a port number is not specified, the expected behavior would be
    // example.com/ns, example.com, *.com
    if let Some(n) = name.rsplit_once(':') {
        name = n.0.to_string();
    }

    // Append wildcarded domains to res slice
    loop {
        match name.split_once('.') {
            None => break,
            Some(n) => {
                name = n.1.to_string();
            }
        }
        res.push(format!("*.{}", name.clone()));
    }

    res
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_image_reference() {
        let test_cases: Vec<String> = vec![
            "",
            ":justtag",
            "docker.io//library///repo:tag",
            "docker.io/library/repo::tag",
            "docker.io/library/",
            "repo@@@sha256:ffffffffffffffffffffffffffffffffff",
            "*:tag",
            "***/&/repo:tag",
            "@sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
            "repo@sha256:ffffffffffffffffffffffffffffffffff",
            "validname@invaliddigest:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
            "Uppercase:tag",
            "test:5000/Uppercase/lowercase:tag",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "aa/asdf$$^/aa",
        ].iter()
        .map(|case| case.to_string())
        .collect();

        for case in test_cases.iter() {
            assert!(Reference::try_from(case.as_str()).is_err());
        }
    }

    #[test]
    fn test_get_image_id_and_ns() {
        #[derive(Debug)]
        struct TestData<'a> {
            image_reference: Reference,
            image_namespace: Vec<&'a str>,
        }

        let tests = &[
            TestData {
                image_reference: Reference::try_from(
                        "docker.io/opensuse/leap:15.3"
                    ).unwrap(),
                image_namespace: vec![
                    "docker.io/opensuse/leap",
                    "docker.io/opensuse",
                    "docker.io",
                    "*.io"
                    ],
            },
            TestData {
                image_reference: Reference::try_from(
                        "test:5000/library/busybox:latest"
                    ).unwrap(),
                image_namespace: vec![
                    "test:5000/library/busybox",
                    "test:5000/library",
                    "test:5000"
                    ],
            },
            TestData {
                image_reference: Reference::try_from(
                        "test:5000/library/busybox@sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"
                    ).unwrap(),
                image_namespace: vec![
                    "test:5000/library/busybox",
                    "test:5000/library",
                    "test:5000"
                    ],
            },
            TestData {
                image_reference: Reference::try_from(
                        "registry.access.redhat.com/busybox:latest"
                    ).unwrap(),
                image_namespace: vec![
                    "registry.access.redhat.com/busybox",
                    "registry.access.redhat.com",
                    "*.access.redhat.com",
                    "*.redhat.com",
                    "*.com"
                    ],
            },
        ];

        for test_case in tests.iter() {
            assert_eq!(
                test_case.image_reference.to_string(),
                test_case.image_reference.whole()
            );

            let mut image_namespace_strings = Vec::new();
            for name in test_case.image_namespace.iter() {
                image_namespace_strings.push(name.to_string());
            }

            assert_eq!(
                image_namespace_strings,
                get_image_namespaces(&test_case.image_reference)
            );
        }
    }
}
