// Copyright (c) 2023 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{anyhow, Context, Result};

use kata_types::mount::{
    get_volume_mount_info, join_path, DirectVolumeMountInfo, KATA_DIRECT_VOLUME_ROOT_PATH,
};

pub mod rawblock_volume;
pub mod spdk_volume;
pub mod vfio_volume;

pub const KATA_DIRECT_VOLUME_TYPE: &str = "directvol";
pub const KATA_VFIO_VOLUME_TYPE: &str = "vfiovol";
pub const KATA_SPDK_VOLUME_TYPE: &str = "spdkvol";
pub const KATA_SPOOL_VOLUME_TYPE: &str = "spoolvol";

// volume mount info load infomation from mountinfo.json
pub fn volume_mount_info(volume_path: &str) -> Result<DirectVolumeMountInfo> {
    get_volume_mount_info(volume_path)
}

// get direct volume path whose volume_path encoded with base64
pub fn get_direct_volume_path(volume_path: &str) -> Result<String> {
    let volume_full_path =
        join_path(KATA_DIRECT_VOLUME_ROOT_PATH, volume_path).context("failed to join path.")?;

    if volume_full_path.exists() {
        Ok(volume_full_path.display().to_string())
    } else {
        Err(anyhow!(format!(
            "direct volume path  {:?} Not Found",
            &volume_full_path
        )))
    }
}
