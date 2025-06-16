// Copyright (c) 2021 Alibaba Cloud
// Copyright (c) 2021, 2023 IBM Corporation
// Copyright (c) 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use safe_path::scoped_join;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use anyhow::{anyhow, bail, Context, Result};
use kata_sys_util::validate::verify_id;
use oci_spec::runtime as oci;

use crate::rpc::CONTAINER_BASE;

use kata_types::mount::KATA_VIRTUAL_VOLUME_IMAGE_GUEST_PULL;
use protocols::agent::Storage;

pub const KATA_IMAGE_WORK_DIR: &str = "/run/kata-containers/image/";
const CONFIG_JSON: &str = "config.json";
const KATA_PAUSE_BUNDLE: &str = "/pause_bundle";

const K8S_CONTAINER_TYPE_KEYS: [&str; 2] = [
    "io.kubernetes.cri.container-type",
    "io.kubernetes.cri-o.ContainerType",
];

// Convenience function to obtain the scope logger.
fn sl() -> slog::Logger {
    slog_scope::logger().new(o!("subsystem" => "image"))
}

// Function to copy a file if it does not exist at the destination
// This function creates a dir, writes a file and if necessary,
// overwrites an existing file.
fn copy_if_not_exists(src: &Path, dst: &Path) -> Result<()> {
    if let Some(dst_dir) = dst.parent() {
        fs::create_dir_all(dst_dir)?;
    }
    fs::copy(src, dst)?;
    Ok(())
}

/// get guest pause image process specification
fn get_pause_image_process() -> Result<oci::Process> {
    let guest_pause_bundle = Path::new(KATA_PAUSE_BUNDLE);
    if !guest_pause_bundle.exists() {
        bail!("Pause image not present in rootfs");
    }
    let guest_pause_config = scoped_join(guest_pause_bundle, CONFIG_JSON)?;

    let image_oci = oci::Spec::load(guest_pause_config.to_str().ok_or_else(|| {
        anyhow!(
            "Failed to load the guest pause image config from {:?}",
            guest_pause_config
        )
    })?)
    .context("load image config file")?;

    let image_oci_process = image_oci.process().as_ref().ok_or_else(|| {
            anyhow!("The guest pause image config does not contain a process specification. Please check the pause image.")
        })?;
    Ok(image_oci_process.clone())
}

/// pause image is packaged in rootfs
pub fn unpack_pause_image(cid: &str) -> Result<String> {
    verify_id(cid).context("The guest pause image cid contains invalid characters.")?;

    let guest_pause_bundle = Path::new(KATA_PAUSE_BUNDLE);
    if !guest_pause_bundle.exists() {
        bail!("Pause image not present in rootfs");
    }
    let guest_pause_config = scoped_join(guest_pause_bundle, CONFIG_JSON)?;
    info!(sl(), "use guest pause image cid {:?}", cid);

    let image_oci = oci::Spec::load(guest_pause_config.to_str().ok_or_else(|| {
        anyhow!(
            "Failed to load the guest pause image config from {:?}",
            guest_pause_config
        )
    })?)
    .context("load image config file")?;

    let image_oci_process = image_oci.process().as_ref().ok_or_else(|| {
            anyhow!("The guest pause image config does not contain a process specification. Please check the pause image.")
        })?;
    info!(
        sl(),
        "pause image oci process {:?}",
        image_oci_process.clone()
    );

    // Ensure that the args vector is not empty before accessing its elements.
    // Check the number of arguments.
    let args = if let Some(args_vec) = image_oci_process.args() {
        args_vec
    } else {
        bail!("The number of args should be greater than or equal to one! Please check the pause image.");
    };

    let pause_bundle = scoped_join(CONTAINER_BASE, cid)?;
    fs::create_dir_all(&pause_bundle)?;
    let pause_rootfs = scoped_join(&pause_bundle, "rootfs")?;
    fs::create_dir_all(&pause_rootfs)?;
    info!(sl(), "pause_rootfs {:?}", pause_rootfs);

    copy_if_not_exists(&guest_pause_config, &pause_bundle.join(CONFIG_JSON))?;
    let arg_path = Path::new(&args[0]).strip_prefix("/")?;
    copy_if_not_exists(
        &guest_pause_bundle.join("rootfs").join(arg_path),
        &pause_rootfs.join(arg_path),
    )?;
    Ok(pause_rootfs.display().to_string())
}

/// check whether the image is for sandbox or for container.
pub fn is_sandbox(image_metadata: &HashMap<String, String>) -> bool {
    let mut is_sandbox = false;
    for key in K8S_CONTAINER_TYPE_KEYS.iter() {
        if let Some(value) = image_metadata.get(key as &str) {
            if value == "sandbox" {
                is_sandbox = true;
                break;
            }
        }
    }
    is_sandbox
}

/// get_process overrides the OCI process spec with pause image process spec if needed
pub fn get_process(
    ocip: &oci::Process,
    oci: &oci::Spec,
    storages: Vec<Storage>,
) -> Result<oci::Process> {
    let mut guest_pull = false;
    for storage in storages {
        if storage.driver == KATA_VIRTUAL_VOLUME_IMAGE_GUEST_PULL {
            guest_pull = true;
            break;
        }
    }
    if guest_pull {
        if let Some(a) = oci.annotations() {
            if is_sandbox(a) {
                return get_pause_image_process();
            }
        }
    }

    Ok(ocip.clone())
}
