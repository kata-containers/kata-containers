// Copyright (c) 2025 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//
// Description: Boot UVM for testing container storages/volumes.

use crate::vm::share_fs_utils;
use anyhow::{anyhow, Context, Result};
use kata_sys_util::mount;
use kata_types::config::TomlConfig;
use nix::mount::MsFlags;
use protocols::agent::Storage;
use slog::info;
use std::fs;

// constants for container rootfs share
const GUEST_SHARED_PATH: &str = "/run/kata-containers/shared/containers";
const ROOTFS: &str = "rootfs";
const VIRTIO_SHARE_FS_TYPE: &str = "virtiofs";

// Helper function to parse a configuration file.
pub fn load_config(config_file: &str) -> Result<TomlConfig> {
    info!(sl!(), "Load kata configuration file {}", config_file);

    let (mut toml_config, _) = TomlConfig::load_from_file(config_file)
        .context("Failed to load kata configuration file")?;

    // Update the agent kernel params in hypervisor config
    update_agent_kernel_params(&mut toml_config)?;

    // validate configuration and return the error
    toml_config.validate()?;

    info!(sl!(), "parsed config content {:?}", &toml_config);
    Ok(toml_config)
}

pub fn to_kernel_string(key: String, val: String) -> Result<String> {
    if key.is_empty() && val.is_empty() {
        Err(anyhow!("Empty key and value"))
    } else if key.is_empty() {
        Err(anyhow!("Empty key"))
    } else if val.is_empty() {
        Ok(key.to_string())
    } else {
        Ok(format!("{}{}{}", key, "=", val))
    }
}

pub fn get_virtiofs_storage() -> Storage {
    Storage {
        driver: String::from(share_fs_utils::VIRTIO_FS),
        driver_options: Vec::new(),
        source: String::from(share_fs_utils::MOUNT_GUEST_TAG),
        fstype: String::from(VIRTIO_SHARE_FS_TYPE),
        options: vec![String::from("nodev")],
        mount_point: String::from(GUEST_SHARED_PATH),
        ..Default::default()
    }
}

pub fn share_rootfs(bundle_dir: &str, host_path: &str, id: &str) -> Result<String> {
    info!(sl!(), "share_rootfs");

    // prepare rootfs string on host
    let rootfs_host_path = get_host_share_path(host_path, id);
    info!(sl!(), "share_rootfs:: target: {}", rootfs_host_path);

    let rootfs_src_path = format!("{}/{}", bundle_dir, ROOTFS);

    // Mount the src path to shared path
    mount::bind_mount_unchecked(
        &rootfs_src_path,
        &rootfs_host_path,
        false,
        MsFlags::MS_SLAVE,
    )
    .with_context(|| {
        format!(
            "share_rootfs:: failed to bind mount {} to {}",
            &rootfs_src_path, &rootfs_host_path
        )
    })?;

    // Return the guest equivalent path
    let guest_rootfs_path = format!("{}/{}", String::from(GUEST_SHARED_PATH), id);

    info!(sl!(), "share_rootfs:: guest path {}", guest_rootfs_path);

    Ok(guest_rootfs_path)
}

pub fn unshare_rootfs(host_path: &str, id: &str) -> Result<()> {
    info!(sl!(), "unshare_rootfs");

    let rootfs_host_path = get_host_share_path(host_path, id);
    mount::umount_timeout(&rootfs_host_path, 0).context("unshare_rootfs:: umount rootfs")?;

    if let Ok(md) = fs::metadata(&rootfs_host_path) {
        if md.is_dir() {
            fs::remove_dir(&rootfs_host_path)
                .context("unshare_rootfs:: remove the rootfs mount point as a dir")?;
        }
    }

    Ok(())
}

fn update_agent_kernel_params(config: &mut TomlConfig) -> Result<()> {
    let mut params = vec![];
    if let Ok(kv) = config.get_agent_kernel_params() {
        for (k, v) in kv.into_iter() {
            if let Ok(s) = to_kernel_string(k.to_owned(), v.to_owned()) {
                params.push(s);
            }
        }
        if let Some(h) = config.hypervisor.get_mut(&config.runtime.hypervisor_name) {
            h.boot_info.add_kernel_params(params);
        }
    }
    Ok(())
}

// Create the container rootfs host share path
fn get_host_share_path(host_path: &str, id: &str) -> String {
    format!("{}/{}/{}", host_path, id, ROOTFS)
}
