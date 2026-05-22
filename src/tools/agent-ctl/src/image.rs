// Copyright (c) 2024 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//
// Description: Image client to manage container images for testing container creation

use anyhow::{anyhow, Context, Result};
use nix::mount::umount;
use safe_path::scoped_join;
use slog::{debug, warn};
use std::fs;
use std::process::Command;

const IMAGE_WORK_DIR: &str = "/run/kata-containers/test_image/";
const CONTAINER_BASE_TEST: &str = "/run/kata-containers/testing/";

// Pulls the container image referenced in `image` using skopeo and umoci
// and returns the bundle path containing the rootfs & config.json
// Uses anonymous image registry authentication.
pub fn pull_image(image: &str, cid: &str) -> Result<String> {
    if image.is_empty() || cid.is_empty() {
        warn!(sl!(), "pull_image: invalid inputs");
        return Err(anyhow!(
            "Invalid image reference or container id to pull image"
        ));
    }

    debug!(sl!(), "pull_image: setting up directories");

    // Setup the container test base path
    fs::create_dir_all(CONTAINER_BASE_TEST)?;

    // Setup the image work directory
    fs::create_dir_all(IMAGE_WORK_DIR)?;

    // Setup the container bundle path
    let bundle_dir = scoped_join(CONTAINER_BASE_TEST, cid)?;
    fs::create_dir_all(&bundle_dir)?;

    // OCI image directory for skopeo - use oci-archive format with explicit tag
    let oci_dir = scoped_join(IMAGE_WORK_DIR, format!("oci-{}", cid))?;

    // Step 1: Use skopeo to copy the image to a local OCI directory with explicit tag
    debug!(sl!(), "pull_image: copying image with skopeo");
    let skopeo_output = Command::new("skopeo")
        .arg("copy")
        .arg(format!("docker://{}", image))
        .arg(format!("oci:{}:latest", oci_dir.display()))
        .output()
        .context("Failed to execute skopeo")?;

    if !skopeo_output.status.success() {
        let stderr = String::from_utf8_lossy(&skopeo_output.stderr);
        return Err(anyhow!(
            "skopeo copy failed with exit code {:?}: {}",
            skopeo_output.status.code(),
            stderr
        ));
    }

    debug!(sl!(), "pull_image: image copied successfully");

    // Step 2: Use umoci to unpack the OCI image into a bundle
    debug!(sl!(), "pull_image: unpacking image with umoci");
    let umoci_output = Command::new("umoci")
        .arg("unpack")
        .arg("--image")
        .arg(format!("{}:latest", oci_dir.display()))
        .arg(&bundle_dir)
        .output()
        .context("Failed to execute umoci")?;

    if !umoci_output.status.success() {
        let stderr = String::from_utf8_lossy(&umoci_output.stderr);
        let stdout = String::from_utf8_lossy(&umoci_output.stdout);
        return Err(anyhow!(
            "umoci unpack failed with exit code {:?}:\nstdout: {}\nstderr: {}",
            umoci_output.status.code(),
            stdout,
            stderr
        ));
    }

    debug!(
        sl!(),
        "pull_image: image unpacked successfully to {:?}", bundle_dir
    );

    // Verify that the bundle was created correctly
    let rootfs_path = scoped_join(&bundle_dir, "rootfs")?;
    let config_path = scoped_join(&bundle_dir, "config.json")?;

    if !rootfs_path.exists() {
        return Err(anyhow!("rootfs directory not found at {:?}", rootfs_path));
    }

    if !config_path.exists() {
        return Err(anyhow!("config.json not found at {:?}", config_path));
    }

    // Return the bundle path
    Ok(bundle_dir.as_path().display().to_string())
}

pub fn remove_image_mount(cid: &str) -> Result<()> {
    let bundle_path = scoped_join(CONTAINER_BASE_TEST, cid)?;
    let rootfs_path = scoped_join(&bundle_path, "rootfs")?;

    // Try to unmount if it's a mount point (may not be needed with umoci)
    if rootfs_path.exists() {
        let _ = umount(&rootfs_path); // Ignore errors as it may not be mounted
    }

    // Clean up the bundle directory
    if bundle_path.exists() {
        fs::remove_dir_all(&bundle_path)?;
    }

    // Clean up the OCI image directory
    let oci_dir = scoped_join(IMAGE_WORK_DIR, format!("oci-{}", cid))?;
    if oci_dir.exists() {
        fs::remove_dir_all(&oci_dir)?;
    }

    // Try to clean up base directories if empty
    let _ = fs::remove_dir(CONTAINER_BASE_TEST);
    let _ = fs::remove_dir(IMAGE_WORK_DIR);

    Ok(())
}
