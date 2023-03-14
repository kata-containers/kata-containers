// Copyright (c) 2022 Alibaba Cloud
// Copyright (c) 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

//! Test for signature verification.

use image_rs::image::ImageClient;
use serial_test::serial;
use strum_macros::{Display, EnumString};

mod common;

/// Name of different signing schemes.
#[derive(EnumString, Display, Debug, PartialEq)]
pub enum SigningName {
    #[strum(serialize = "Simple Signing")]
    SimpleSigning,
    #[strum(serialize = "None")]
    None,
    #[strum(serialize = "Cosign")]
    Cosign,
}

struct TestItem<'a, 'b> {
    image_ref: &'a str,
    allow: bool,
    signing_scheme: SigningName,
    description: &'b str,
}

#[cfg(feature = "cosign")]
const ALLOW_COSIGN: bool = true;

#[cfg(not(feature = "cosign"))]
const ALLOW_COSIGN: bool = false;

/// Four test cases.
const TESTS: [TestItem; 6] = [
    TestItem {
        image_ref: "quay.io/prometheus/busybox:latest",
        allow: true,
        signing_scheme: SigningName::None,
        description: "Allow pulling an unencrypted unsigned image from an unprotected registry.",
    },
    TestItem {
        image_ref: "quay.io/kata-containers/confidential-containers:signed",
        allow: true,
        signing_scheme: SigningName::SimpleSigning,
        description: "Allow pulling a unencrypted signed image from a protected registry.",
    },
    TestItem {
        image_ref: "quay.io/kata-containers/confidential-containers:unsigned",
        allow: false,
        signing_scheme: SigningName::None,
        description: "Deny pulling an unencrypted unsigned image from a protected registry.",
    },
    TestItem {
        image_ref: "quay.io/kata-containers/confidential-containers:other_signed",
        allow: false,
        signing_scheme: SigningName::SimpleSigning,
        description: "Deny pulling an unencrypted signed image with an unknown signature",
    },
    TestItem {
        image_ref: "quay.io/kata-containers/confidential-containers:cosign-signed",
        allow: ALLOW_COSIGN,
        signing_scheme: SigningName::Cosign,
        description: "Allow pulling an unencrypted signed image with cosign-signed signature",
    },
    TestItem {
        image_ref: "quay.io/kata-containers/confidential-containers:cosign-signed-key2",
        allow: false,
        signing_scheme: SigningName::Cosign,
        description: "Deny pulling an unencrypted signed image by cosign using a wrong public key",
    },
];

/// image-rs built without support for cosign image signing cannot use a policy that includes a type that
/// uses cosign (type: sigstoreSigned), even if the image being pulled is not signed using cosign.
/// https://github.com/confidential-containers/attestation-agent/blob/main/src/kbc_modules/sample_kbc/policy.json
#[cfg(feature = "getresource")]
#[tokio::test]
#[serial]
async fn signature_verification() {
    common::prepare_test().await;
    // Init AA
    let mut aa = common::start_attestation_agent()
        .await
        .expect("Failed to start attestation agent!");

    for test in &TESTS {
        // clean former test files
        common::clean_configs()
            .await
            .expect("Delete configs failed.");

        // Init tempdirs
        let work_dir = tempfile::tempdir().unwrap();
        std::env::set_var("CC_IMAGE_WORK_DIR", &work_dir.path());

        // a new client for every pulling, avoid effection
        // of cache of old client.
        let mut image_client = ImageClient::default();

        // enable signature verification
        image_client.config.security_validate = true;

        let bundle_dir = tempfile::tempdir().unwrap();

        let _res = image_client
            .pull_image(
                test.image_ref,
                bundle_dir.path(),
                &None,
                &Some(common::AA_PARAMETER),
            )
            .await;
        if cfg!(all(
            feature = "snapshot-overlayfs",
            feature = "signature-simple"
        )) {
            assert_eq!(
                _res.is_ok(),
                test.allow,
                "Test: {}, Signing scheme: {}, {:?}",
                test.description,
                test.signing_scheme.to_string(),
                _res
            );
        }
    }

    // kill AA when the test is finished
    aa.kill().await.expect("Failed to stop attestation agent!");
    common::clean().await;
}
