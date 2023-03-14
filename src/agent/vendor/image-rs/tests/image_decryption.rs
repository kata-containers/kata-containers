// Copyright (c) 2022 Intel Corporation
// Copyright (c) 2022 Alibaba Cloud
//
// SPDX-License-Identifier: Apache-2.0
//

//! Test for decryption of image layers.

use image_rs::image::ImageClient;
use serial_test::serial;

mod common;

/// The image to be decrypted using offline-fs-kbc
const ENCRYPTED_IMAGE_REFERENCE_OFFLINE_FS_KBS: &str = "docker.io/xynnn007/busybox:encrypted";
const UNENCRYPTED_IMAGE_REFERENCE_OFFLINE_FS_KBS: &str = "docker.io/arronwang/busybox_zstd";

/// Ocicrypt-rs config
const OCICRYPT_CONFIG: &str = "test_data/ocicrypt_keyprovider.conf";

#[cfg(feature = "getresource")]
#[tokio::test]
#[serial]
async fn test_decrypt_layers() {
    common::prepare_test().await;
    // Init AA
    let mut aa = common::start_attestation_agent()
        .await
        .expect("Failed to start attestation agent!");

    // Set env for ocicrypt-rs. The env is needed by ocicrypt-rs
    // to communicate with AA
    let manifest_dir = std::env!("CARGO_MANIFEST_DIR");
    let keyprovider_config = format!("{}/{}", manifest_dir, OCICRYPT_CONFIG);
    std::env::set_var("OCICRYPT_KEYPROVIDER_CONFIG", keyprovider_config);

    let work_dir = tempfile::tempdir().unwrap();
    std::env::set_var("CC_IMAGE_WORK_DIR", &work_dir.path());
    let bundle_dir = tempfile::tempdir().unwrap();

    // clean former test files, which is needed to prevent
    // lint from warning dead code.
    common::clean_configs()
        .await
        .expect("Delete configs failed.");
    let mut image_client = ImageClient::default();
    let image_name = if cfg!(all(feature = "encryption", feature = "keywrap-grpc")) {
        ENCRYPTED_IMAGE_REFERENCE_OFFLINE_FS_KBS
    } else {
        UNENCRYPTED_IMAGE_REFERENCE_OFFLINE_FS_KBS
    };
    if cfg!(feature = "snapshot-overlayfs") {
        if let Err(e) = image_client
            .pull_image(
                image_name,
                bundle_dir.path(),
                &None,
                &Some(common::AA_PARAMETER),
            )
            .await
        {
            panic!("test_decrypt_layers() failed to download image, {}", e);
        }
    } else {
        image_client
            .pull_image(
                image_name,
                bundle_dir.path(),
                &None,
                &Some(common::AA_PARAMETER),
            )
            .await
            .unwrap_err();
    }

    // kill AA when the test is finished
    aa.kill().await.expect("Failed to stop attestation agent!");
    common::clean().await;
}
