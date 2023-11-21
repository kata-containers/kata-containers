// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::path::Path;

use anyhow::{Context, Result};
use nix::mount::MsFlags;
use tokio::sync::RwLock;

use hypervisor::{
    device::{
        device_manager::{do_handle_device, do_update_device, DeviceManager},
        driver::{ShareFsMountConfig, ShareFsMountOperation, ShareFsMountType},
        DeviceConfig,
    },
    ShareFsConfig,
};
use kata_sys_util::mount;

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
    d: &RwLock<DeviceManager>,
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

    let sharefs_config = ShareFsConfig {
        host_shared_path: host_ro_dest.display().to_string(),
        sock_path: generate_sock_path(root),
        mount_tag: String::from(MOUNT_GUEST_TAG),
        fs_type: fs_type.to_string(),
        queue_size: 0,
        queue_num: 0,
        options: vec![],
        mount_config: None,
    };

    // create and insert virtio-fs device into Guest
    do_handle_device(d, &DeviceConfig::ShareFsCfg(sharefs_config))
        .await
        .context("do add virtio-fs device failed.")?;

    Ok(())
}

pub(crate) async fn setup_inline_virtiofs(d: &RwLock<DeviceManager>, id: &str) -> Result<()> {
    // - source is the absolute path of PASSTHROUGH_FS_DIR on host, e.g.
    //   /run/kata-containers/shared/sandboxes/<sid>/passthrough
    // - mount point is the path relative to KATA_GUEST_SHARE_DIR in guest
    let mnt = format!("/{}", PASSTHROUGH_FS_DIR);

    let rw_source = utils::get_host_rw_shared_path(id).join(PASSTHROUGH_FS_DIR);
    utils::ensure_dir_exist(&rw_source).context("ensure directory exist")?;

    let host_ro_shared_path = utils::get_host_ro_shared_path(id);
    let source = host_ro_shared_path
        .join(PASSTHROUGH_FS_DIR)
        .display()
        .to_string();

    let virtiofs_mount = ShareFsMountConfig {
        source: source.clone(),
        fstype: ShareFsMountType::PASSTHROUGH,
        mount_point: mnt,
        config: None,
        tag: String::from(MOUNT_GUEST_TAG),
        op: ShareFsMountOperation::Mount,
        prefetch_list_path: None,
    };

    let sharefs_config = ShareFsConfig {
        host_shared_path: host_ro_shared_path.display().to_string(),
        mount_config: Some(virtiofs_mount),
        ..Default::default()
    };

    // update virtio-fs device with ShareFsMountConfig
    do_update_device(d, &DeviceConfig::ShareFsCfg(sharefs_config))
        .await
        .context("fail to attach passthrough fs.")?;

    Ok(())
}

pub async fn rafs_mount(
    d: &RwLock<DeviceManager>,
    sid: &str,
    rafs_meta: String,
    rafs_mnt: String,
    config_content: String,
    prefetch_list_path: Option<String>,
) -> Result<()> {
    info!(
        sl!(),
        "Attaching rafs meta file {} to virtio-fs device, rafs mount point {}", rafs_meta, rafs_mnt
    );

    let rafs_config = ShareFsMountConfig {
        source: rafs_meta.clone(),
        fstype: ShareFsMountType::RAFS,
        mount_point: rafs_mnt,
        config: Some(config_content),
        tag: String::from(MOUNT_GUEST_TAG),
        op: ShareFsMountOperation::Mount,
        prefetch_list_path,
    };

    let host_shared_path = utils::get_host_ro_shared_path(sid).display().to_string();
    let sharefs_config = ShareFsConfig {
        host_shared_path,
        mount_config: Some(rafs_config),
        ..Default::default()
    };

    // update virtio-fs device with ShareFsMountConfig
    do_update_device(d, &DeviceConfig::ShareFsCfg(sharefs_config))
        .await
        .with_context(|| format!("fail to attach rafs {:?}", rafs_meta))?;

    Ok(())
}
