// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::path::Path;

use anyhow::{Context, Result};
use hypervisor::{
    device::{
        driver::{
            ShareFsDevice, ShareFsMountConfig, ShareFsMountDevice, ShareFsMountType,
            ShareFsOperation,
        },
        DeviceType,
    },
    Hypervisor, ShareFsDeviceConfig,
};
use kata_sys_util::mount;
use nix::mount::MsFlags;

use super::{utils, PASSTHROUGH_FS_DIR};

pub(crate) const MOUNT_GUEST_TAG: &str = "kataShared";

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

    mount::bind_mount_unchecked(&host_rw_dest, &host_ro_dest, true, MsFlags::MS_SLAVE)
        .context("bind mount shared_fs directory")?;

    let share_fs_device = ShareFsDevice {
        config: ShareFsDeviceConfig {
            sock_path: generate_sock_path(root),
            mount_tag: String::from(MOUNT_GUEST_TAG),
            host_path: String::from(host_ro_dest.to_str().unwrap()),
            fs_type: fs_type.to_string(),
            queue_size: 0,
            queue_num: 0,
            options: vec![],
        },
    };
    h.add_device(DeviceType::ShareFs(share_fs_device))
        .await
        .context("add device")?;
    Ok(())
}

pub(crate) async fn setup_inline_virtiofs(id: &str, h: &dyn Hypervisor) -> Result<()> {
    // - source is the absolute path of PASSTHROUGH_FS_DIR on host, e.g.
    //   /run/kata-containers/shared/sandboxes/<sid>/passthrough
    // - mount point is the path relative to KATA_GUEST_SHARE_DIR in guest
    let mnt = format!("/{}", PASSTHROUGH_FS_DIR);

    let rw_source = utils::get_host_rw_shared_path(id).join(PASSTHROUGH_FS_DIR);
    utils::ensure_dir_exist(&rw_source).context("ensure directory exist")?;

    let ro_source = utils::get_host_ro_shared_path(id).join(PASSTHROUGH_FS_DIR);
    let source = String::from(ro_source.to_str().unwrap());

    let virtio_fs = ShareFsMountDevice {
        config: ShareFsMountConfig {
            source: source.clone(),
            fstype: ShareFsMountType::PASSTHROUGH,
            mount_point: mnt,
            config: None,
            tag: String::from(MOUNT_GUEST_TAG),
            op: ShareFsOperation::Mount,
            prefetch_list_path: None,
        },
    };
    h.add_device(DeviceType::ShareFsMount(virtio_fs))
        .await
        .with_context(|| format!("fail to attach passthrough fs {:?}", source))
}

pub async fn rafs_mount(
    h: &dyn Hypervisor,
    rafs_meta: String,
    rafs_mnt: String,
    config_content: String,
    prefetch_list_path: Option<String>,
) -> Result<()> {
    info!(
        sl!(),
        "Attaching rafs meta file {} to virtio-fs device, rafs mount point {}", rafs_meta, rafs_mnt
    );
    let virtio_fs = ShareFsMountDevice {
        config: ShareFsMountConfig {
            source: rafs_meta.clone(),
            fstype: ShareFsMountType::RAFS,
            mount_point: rafs_mnt,
            config: Some(config_content),
            tag: String::from(MOUNT_GUEST_TAG),
            op: ShareFsOperation::Mount,
            prefetch_list_path,
        },
    };
    h.add_device(DeviceType::ShareFsMount(virtio_fs))
        .await
        .with_context(|| format!("fail to attach rafs {:?}", rafs_meta))?;
    Ok(())
}
