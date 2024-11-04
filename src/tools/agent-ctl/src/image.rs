// Copyright (c) 2024 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//
// Description: Image client to manage container images for testing container creation

use anyhow::{anyhow, Context, Result};
use image_rs::image::ImageClient;
use nix::mount::umount;
use safe_path::scoped_join;
use slog::{debug, warn};
use std::fs;
use std::path::PathBuf;

const IMAGE_WORK_DIR: &str = "/run/kata-containers/test_image/";
const CONTAINER_BASE_TEST: &str = "/run/kata-containers/testing/";

// Pulls the container image referenced in `image` using image-rs
// and returns the bundle path containing the rootfs (mounted by
// the underlying snapshotter, overlayfs in this case) & config.json
// Uses anonymous image registry authentication.
pub fn pull_image(image: &str, cid: &str) -> Result<String> {
    if image.is_empty() || cid.is_empty() {
        warn!(sl!(), "pull_image: invalid inputs");
        return Err(anyhow!(
            "Invalid image reference or container id to pull image"
        ));
    }

    debug!(sl!(), "pull_image: creating image client");
    let mut image_client = ImageClient::new(PathBuf::from(IMAGE_WORK_DIR));
    image_client.config.auth = false;
    image_client.config.security_validate = false;

    // setup the container test base path
    fs::create_dir_all(CONTAINER_BASE_TEST)?;

    // setup the container bundle path
    let bundle_dir = scoped_join(CONTAINER_BASE_TEST, cid)?;
    fs::create_dir_all(bundle_dir.clone())?;

    // pull the image
    let image_id = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(image_client.pull_image(image, &bundle_dir, &None, &None))
        .context("pull and unpack container image")?;

    debug!(
        sl!(),
        "pull_image: image pull for {:?} successfull", image_id
    );

    // return the bundle path created by unpacking the images
    Ok(bundle_dir.as_path().display().to_string())
}

pub fn remove_image_mount(cid: &str) -> Result<()> {
    let bundle_path = scoped_join(CONTAINER_BASE_TEST, cid)?;
    let rootfs_path = scoped_join(bundle_path, "rootfs")?;
    umount(&rootfs_path)?;
    Ok(())
}
