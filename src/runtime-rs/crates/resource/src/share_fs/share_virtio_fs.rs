// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::path::Path;

use anyhow::{Context, Result};
use hypervisor::{device, Hypervisor};
use kata_sys_util::mount;

use super::utils;

pub(crate) const MOUNT_GUEST_TAG: &str = "kataShared";
pub(crate) const PASSTHROUGH_FS_DIR: &str = "passthrough";

pub(crate) const FS_TYPE_VIRTIO_FS: &str = "virtiofs";
pub(crate) const KATA_VIRTIO_FS_DEV_TYPE: &str = "virtio-fs";

const VIRTIO_FS_SOCKET: &str = "virtiofsd.sock";

pub(crate) fn generate_sock_path(root: &str) -> String {
    let socket_path = Path::new(root).join(VIRTIO_FS_SOCKET);
    socket_path.to_str().unwrap().to_string()
}

pub(crate) async fn prepare_virtiofs(
    h: &dyn Hypervisor,
    fs_type: &str,
    id: &str,
    root: &str,
) -> Result<()> {
    let host_ro_dest = utils::get_host_ro_shared_path(id);
    utils::ensure_dir_exist(&host_ro_dest)?;

    let host_rw_dest = utils::get_host_rw_shared_path(id);
    utils::ensure_dir_exist(&host_rw_dest)?;

    mount::bind_mount_unchecked(&host_rw_dest, &host_ro_dest, true)
        .context("bind mount shared_fs directory")?;

    let share_fs_device = device::Device::ShareFsDevice(device::ShareFsDeviceConfig {
        sock_path: generate_sock_path(root),
        mount_tag: String::from(MOUNT_GUEST_TAG),
        host_path: String::from(host_ro_dest.to_str().unwrap()),
        fs_type: fs_type.to_string(),
        queue_size: 0,
        queue_num: 0,
    });
    h.add_device(share_fs_device).await.context("add device")?;
    Ok(())
}
