// Copyright (c) 2023 Alibaba Cloud
// Copyright (c) 2023 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

// @extra-virtiofs Defination
// @Annotation: io.katacontainers.config.hypervisor.extra_virtiofs
// @ContentFormat:
// <virtiofs_device_01>:<arg01,arg02,...>;<virtiofs_device_02>:<arg01,arg02,...>;...
// @Example:
// "virtiofs_device_01:-o no_open,cache=none --thread-pool-size=1;virtiofs_device_02:-o no_open,cache=always,writeback,cache_symlinks --thread-pool-size=10"
// @Configuration.toml
// [hypervisor.XXX]
// extra_virtiofs = "virtiofs_device_01:-o no_open,cache=none --thread-pool-size=1;virtiofs_device_02: -o no_open,cache=always,writeback,cache_symlinks --thread-pool-size=10"

// @special_volumes
// @Annotation: io.katacontainers.config.runtime.special_volumes
// @ContentFormat: <virtiofs_device_01>:<container_path01,container_path02,...>;<virtiofs_device_02>:<container_path03,container_path04,...>;...

// @UseCase
// @Special_volumes
//    --annotation "io.katacontainers.config.hypervisor.extra_virtiofs=..." \
//    --annotation "io.katacontainers.config.hypervisor.special_volumes=..." \
//    --mount type=bind,src=host_path01,dst=container_path01,options=rbind:ro
//
// @default:
// host mount path: /run/kata-containers/shared/sandboxes/<sid>/rw/passthrough/sharefs-special-volumes/dirX
// guest mount path: /run/kata-containers/shared/containers/passthrough/sharefs-special-volumes/dirX
// @extra_virtiofs:
// host mount path: /run/kata-containers/shared/sandboxes/<sid>/<virtiofs_device>/rw/passthrough/sharefs-special-volumes/dirX
// guest mount path: /run/kata-containers/shared/<virtiofs_device>/passthrough/sharefs-special-volumes/dirX
// docker run -v src:dest --annotation extra_virtiofs ... --annotation special_volumes ...
// sudo ctr run -t --rm --runtime io.containerd.kata.v2 \
//     --mount type=bind,src=host_path01,dst=container_path01,options=rbind:ro \
//     "$image" kata-v02 /bin/bash

use std::{path::Path, sync::Arc};

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use nix::mount::MsFlags;
use tokio::sync::RwLock;

use hypervisor::device::device_manager::DeviceManager;
use kata_sys_util::mount::bind_mount_unchecked;

use super::{Volume, BIND};
use crate::share_fs::{
    get_host_shared_subpath, ShareFs, KATA_GUEST_SHARED, KATA_GUEST_SHARE_DIR, MULTI_VIRTIOFS,
    PASSTHROUGH_FS_DIR,
};

pub const SANDBOX_SPECIAL_VOLUMES: &str = "sharefs-special-volumes";
pub const DEFAULT_VIRTIOFS_DEVICE: &str = "default";

#[derive(Debug, Default)]
pub struct SpecialVolume {
    sandbox_id: String,
}

impl SpecialVolume {
    pub fn new(sid: &str) -> Self {
        Self {
            sandbox_id: sid.to_owned(),
        }
    }

    fn do_handle_sharefs_special_volumes(
        &self,
        source: &str,
        container_path: &str,
        virtiofs_device: &str,
    ) -> Result<String> {
        // bind mount secret/configmap source to guest as sandbox level volume with directory name `dirX`,
        // host mount path: /run/kata-containers/shared/sandboxes/<sid>/rw/passthrough/sharefs-special-volumes
        // guest mount path: /run/kata-containers/shared/containers/passthrough/sharefs-special-volumes
        // host_mounts_target: /run/kata-containers/shared/sandboxes/<sid>/<virtiofs_device>/rw/passthrough/sharefs-special-volumes/dirX
        let host_mount_target = get_host_shared_subpath(
            self.sandbox_id.as_str(),
            Some(virtiofs_device),
            SANDBOX_SPECIAL_VOLUMES,
            false,
        )
        .join(container_path);
        if host_mount_target.exists() {
            warn!(
                sl!(),
                "sharefs-special-mount: {:?}:{:?} has been mounted, no need to mount again",
                virtiofs_device,
                source
            );

            return Ok(String::new());
        }

        // default virtiofs, guest mount path:
        // /run/kata-containers/shared/containers/passthrough/sharefs-special-volumes/dirX
        // extra virtiofs, guest mount path:
        // /run/kata-containers/shared/<virtiofs-device>/passthrough/sharefs-special-volumes/dirX
        let guest_mount_path = match virtiofs_device {
            DEFAULT_VIRTIOFS_DEVICE => {
                format!(
                    "{}/{}/{}/{}",
                    KATA_GUEST_SHARE_DIR,
                    PASSTHROUGH_FS_DIR,
                    SANDBOX_SPECIAL_VOLUMES,
                    container_path // dirX
                )
            }
            _ => {
                format!(
                    "{}/{}/{}/{}/{}",
                    KATA_GUEST_SHARED,
                    virtiofs_device,
                    PASSTHROUGH_FS_DIR,
                    SANDBOX_SPECIAL_VOLUMES,
                    container_path
                )
            }
        };

        debug!(
            sl!(),
            "sharefs-special_volumes: {:?} bind mount to {:?}, guest mount path: {:?}",
            &source,
            host_mount_target.display(),
            guest_mount_path
        );

        bind_mount_unchecked(
            Path::new(source),
            &host_mount_target,
            true,
            MsFlags::MS_SLAVE,
        )
        .context("bind mount")?;

        Ok(guest_mount_path)
    }
}

#[derive(Debug, Default)]
pub(crate) struct ShareFsSpecialVolume {
    special_volume: SpecialVolume,
    mount: oci::Mount,
    storages: Option<agent::Storage>,
}

// --mount type=bind,src=host_path01,dst=container_path01,options=rbind:ro
// setup for sandbox volume for sharefs_special_volumes, return whether this volume
// should be mount into container
// @source: mount.source, e.g. host_path
// @destination: mount.destination, e.g. container_path
// @fs_type: mount.fstype, e.g. bind
impl ShareFsSpecialVolume {
    pub async fn new(
        // share_fs: &Option<Arc<dyn ShareFs>>,
        m: &oci::Mount,
        sid: &str,
        _cid: &str,
        _readonly: bool,
    ) -> Result<Self> {
        let mut sharefs_volume = ShareFsSpecialVolume {
            special_volume: SpecialVolume::new(sid),
            ..Default::default()
        };

        // let virtiofs_device = {
        //     let mounted_info_guard = if let Some(sharefs) = share_fs.clone() {
        //         sharefs.mounted_info_set()
        //     } else {
        //         return Ok(sharefs_volume);
        //     };

        //     let volume_devices = if let Some(mounted_info) = mounted_info_guard
        //         .lock()
        //         .await
        //         .get(MULTI_VIRTIOFS)
        //         .cloned()
        //     {
        //         mounted_info.volume_devices
        //     } else {
        //         return Err(anyhow!("volume devices is empty, something goes wrong."));
        //     };

        //     // It's safe to unwrap it here as the result of volume_devices is always OK here.
        //     // And we get the virtiofs device with the destination given in mount.destination.
        //     if let Some(device) = volume_devices.unwrap().get(&m.destination) {
        //         device.clone()
        //     } else {
        //         return Err(anyhow!("virtiofs device not found."));
        //     }
        // };

        // destination format <virtiofs_device@/container_path>,
        // split it, get virtiofs device and specified container path
        let tokens: Vec<&str> = m.destination.split('@').collect();
        if tokens.len() != 2 {
            return Err(anyhow!(
                "special volume destination: {:?} is invalid",
                m.destination
            ));
        }
        let (virtiofs_device, container_path) = (tokens[0], tokens[1]);

        // do check source path is valid
        if !Path::new(&m.source).exists() {
            return Err(anyhow!(
                "special volume source: {:?} not exists.",
                &m.source
            ));
        }

        let guest_mount_path = sharefs_volume
            .special_volume
            .do_handle_sharefs_special_volumes(&m.source, container_path, virtiofs_device)?;
        // .do_handle_sharefs_special_volumes(&m.source, &m.destination, &virtiofs_device)?;

        sharefs_volume.mount = oci::Mount {
            r#type: BIND.to_string(),
            destination: m.destination.to_owned(),
            source: guest_mount_path,
            options: m.options.clone(),
        };
        sharefs_volume.storages = None;

        Ok(sharefs_volume)
    }
}

#[async_trait]
impl Volume for ShareFsSpecialVolume {
    fn get_volume_mount(&self) -> Result<Vec<oci::Mount>> {
        Ok(vec![self.mount.clone()])
    }

    fn get_storage(&self) -> Result<Vec<agent::Storage>> {
        let s = if let Some(s) = self.storages.as_ref() {
            vec![s.clone()]
        } else {
            vec![]
        };

        Ok(s)
    }

    async fn cleanup(&self, _device_manager: &RwLock<DeviceManager>) -> Result<()> {
        // TODO: implement it !
        Ok(())
    }

    fn get_device_id(&self) -> Result<Option<String>> {
        // TODO: virtiofs device needs to be managed by device manager.
        Ok(None)
    }
}

// --mount type=bind,src=host_path,dest=virtio_device01@container_path
// (1) If mount.r#type is bind, mount.dest contains '@' and split it, getting only two parts;
// (2) Do double check the container path in volume_devices, and its device is just the virtiofs device.
// (3) Otherwise, it's false;
pub(crate) async fn is_sharefs_special_volume(
    share_fs: &Option<Arc<dyn ShareFs>>,
    m: &oci::Mount,
) -> bool {
    if m.r#type.as_str() != BIND {
        return false;
    }

    // destination format <virtiofs_device@/container_path>,
    // split it, get virtiofs device and specified container path
    let tokens: Vec<&str> = m.destination.split('@').collect();
    if tokens.len() != 2 {
        return false;
    }
    let (virtiofs_device, container_path) = (tokens[0], tokens[1]);

    let mounted_info_guard = if let Some(sharefs) = share_fs.clone() {
        sharefs.mounted_info_set()
    } else {
        return false;
    };

    let volume_devices =
        if let Some(mounted_info) = mounted_info_guard.lock().await.get(MULTI_VIRTIOFS).cloned() {
            mounted_info.volume_devices
        } else {
            return false;
        };

    // It's safe to unwrap it here as the result of volume_devices is always OK here.
    // And we get the virtiofs device with the destination given in mount.destination.
    if let Some(target_device) = volume_devices.unwrap().get(container_path) {
        target_device.clone().as_str() == virtiofs_device
    } else {
        return false;
    }
}
