// Copyright (c) 2022-2023 Alibaba Cloud
// Copyright (c) 2022-2023 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::{fs, path::Path};

use anyhow::{anyhow, Context, Result};

use crate::{
    share_fs::{do_get_guest_path, do_get_host_path},
    volume::share_fs_volume::generate_mount_path,
};
use kata_sys_util::eother;
use kata_types::mount::{get_volume_mount_info, DirectVolumeMountInfo};

pub const DEFAULT_VOLUME_FS_TYPE: &str = "ext4";
pub const KATA_MOUNT_BIND_TYPE: &str = "bind";
pub const KATA_DIRECT_VOLUME_TYPE: &str = "directvol";
pub const KATA_VFIO_VOLUME_TYPE: &str = "vfiovol";
pub const KATA_SPDK_VOLUME_TYPE: &str = "spdkvol";

// volume mount info load infomation from mountinfo.json
pub fn volume_mount_info(volume_path: &str) -> Result<DirectVolumeMountInfo> {
    get_volume_mount_info(volume_path)
}

pub fn get_file_name<P: AsRef<Path>>(src: P) -> Result<String> {
    let file_name = src
        .as_ref()
        .file_name()
        .map(|v| v.to_os_string())
        .ok_or_else(|| {
            eother!(
                "failed to get file name of path {}",
                src.as_ref().to_string_lossy()
            )
        })?
        .into_string()
        .map_err(|e| anyhow!("failed to convert to string {:?}", e))?;

    Ok(file_name)
}

pub(crate) async fn generate_shared_path(
    dest: String,
    read_only: bool,
    cid: &str,
    sid: &str,
) -> Result<String> {
    let file_name = get_file_name(&dest).context("failed to get file name.")?;
    let mount_name = generate_mount_path(cid, file_name.as_str());
    let guest_path = do_get_guest_path(&mount_name, cid, true, false);
    let host_path = do_get_host_path(&mount_name, sid, cid, true, read_only);

    if dest.starts_with("/dev") {
        fs::File::create(&host_path).context(format!("failed to create file {:?}", &host_path))?;
    } else {
        std::fs::create_dir_all(&host_path)
            .map_err(|e| anyhow!("failed to create dir {}: {:?}", host_path, e))?;
    }

    Ok(guest_path)
}
