// Copyright (c) 2023 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use hypervisor::device::device_manager::DeviceManager;
use kata_sys_util::mount::{get_mount_path, get_mount_type};
use kata_types::mount::{record_sandbox_id, DirectVolumeMountInfo};
use nix::sys::{stat, stat::SFlag};
use oci_spec::runtime as oci;
use tokio::sync::RwLock;

use crate::volume::{
    direct_volumes::{
        get_direct_volume_path, rawblock_volume, spdk_volume, vfio_volume, volume_mount_info,
        KATA_DIRECT_VOLUME_TYPE, KATA_SPDK_VOLUME_TYPE, KATA_SPOOL_VOLUME_TYPE,
        KATA_VFIO_VOLUME_TYPE,
    },
    utils::KATA_MOUNT_BIND_TYPE,
    Volume,
};

// Keep the host identity alongside the generated guest mount. The resource
// manager retains volume objects after container cleanup, so invalidate the
// association once the underlying volume has been successfully detached.
struct DirectVolume {
    volume: Arc<dyn Volume>,
    host_path: String,
    guest_path: Option<String>,
    active: AtomicBool,
}

impl DirectVolume {
    fn new(volume: Arc<dyn Volume>, host_path: String) -> Result<Self> {
        let guest_path = volume
            .get_storage()?
            .into_iter()
            .next()
            .map(|storage| storage.mount_point);
        Ok(Self {
            volume,
            host_path,
            guest_path,
            active: AtomicBool::new(true),
        })
    }
}

#[async_trait]
impl Volume for DirectVolume {
    fn get_volume_mount(&self) -> Result<Vec<oci::Mount>> {
        self.volume.get_volume_mount()
    }

    fn get_storage(&self) -> Result<Vec<agent::Storage>> {
        self.volume.get_storage()
    }

    fn get_device_id(&self) -> Result<Option<String>> {
        self.volume.get_device_id()
    }

    fn get_guest_mount_path(&self, host_path: &str) -> Option<&str> {
        if self.active.load(Ordering::Acquire) && self.host_path == host_path {
            self.guest_path.as_deref()
        } else {
            None
        }
    }

    async fn cleanup(&self, device_manager: &RwLock<DeviceManager>) -> Result<()> {
        self.volume.cleanup(device_manager).await?;
        self.active.store(false, Ordering::Release);
        Ok(())
    }
}

enum DirectVolumeType {
    RawBlock,
    Spdk,
    Vfio,
}

fn to_volume_type(volume_type: &str) -> DirectVolumeType {
    match volume_type {
        KATA_SPDK_VOLUME_TYPE | KATA_SPOOL_VOLUME_TYPE => DirectVolumeType::Spdk,
        KATA_VFIO_VOLUME_TYPE => DirectVolumeType::Vfio,
        _ => DirectVolumeType::RawBlock,
    }
}

pub(crate) async fn handle_direct_volume(
    d: &RwLock<DeviceManager>,
    m: &oci::Mount,
    read_only: bool,
    sid: &str,
) -> Result<Option<Arc<dyn Volume>>> {
    // In the direct volume scenario, we check if the source of a mount is in the
    // /run/kata-containers/shared/direct-volumes/SID path by iterating over all the mounts.
    // If the source is not in the path with error kind *NotFound*, we ignore the error
    // and we treat it as block volume with oci Mount.type *bind*. Just fill in the block
    // volume info in the DirectVolumeMountInfo
    let volume_path = get_mount_path(m.source());
    let mount_info: DirectVolumeMountInfo = match volume_mount_info(&volume_path) {
        Ok(mount_info) => mount_info,
        Err(e) => {
            // First, We need filter the non-io::ErrorKind.
            if !e.is::<std::io::ErrorKind>() {
                return Err(anyhow!(format!(
                    "unexpected error occurs when parse mount info for {:?}, with error {:?}",
                    m.source(),
                    e.to_string()
                )));
            }

            // Second, we need filter non-NotFound error.
            // Safe to unwrap here, as the error is of type std::io::ErrorKind.
            let error_kind = e.downcast_ref::<std::io::ErrorKind>().unwrap();
            if *error_kind != std::io::ErrorKind::NotFound {
                return Err(anyhow!(format!(
                    "failed to parse volume mount info for {:?}, with error {:?}",
                    m.source(),
                    e.to_string()
                )));
            }

            // Third, if the case is *NotFound* , we just return Ok(None).
            return Ok(None);
        }
    };

    // Record the association between this sandbox and the direct volume so that
    // `kata-ctl direct-volume {stats,resize}` can locate the owning sandbox by
    // scanning the direct-volume directory. This mirrors runtime-go's
    // `createBlockDevices` -> `volume.RecordSandboxID`. A failure here must not
    // abort volume setup (matches Go behaviour: log and continue), it only means
    // online stats/resize via kata-ctl will be unavailable.
    if let Err(e) = record_sandbox_id(sid, &volume_path) {
        warn!(
            sl!(),
            "failed to record sandbox id {} for direct volume {}: {:?}", sid, volume_path, e
        );
    }

    let direct_volume: Arc<dyn Volume> = match to_volume_type(mount_info.volume_type.as_str()) {
        DirectVolumeType::RawBlock => Arc::new(
            rawblock_volume::RawblockVolume::new(d, m, &mount_info, read_only, sid)
                .await
                .with_context(|| format!("new sid {:?} rawblock volume {:?}", sid, m))?,
        ),
        DirectVolumeType::Spdk => Arc::new(
            spdk_volume::SPDKVolume::new(d, m, &mount_info, read_only, sid)
                .await
                .with_context(|| format!("create spdk volume {m:?}"))?,
        ),
        DirectVolumeType::Vfio => Arc::new(
            vfio_volume::VfioVolume::new(d, m, &mount_info, read_only, sid)
                .await
                .with_context(|| format!("new vfio volume {m:?}"))?,
        ),
    };

    Ok(Some(Arc::new(DirectVolume::new(
        direct_volume,
        mount_info.device,
    )?)))
}

pub(crate) fn is_direct_volume(m: &oci::Mount) -> Result<bool> {
    let mnt_type = get_mount_type(m);
    let mount_type = mnt_type.as_str();

    // Filter the non-bind volume and non-direct-vol volume
    let vol_types = [
        KATA_MOUNT_BIND_TYPE,
        KATA_DIRECT_VOLUME_TYPE,
        KATA_VFIO_VOLUME_TYPE,
        KATA_SPDK_VOLUME_TYPE,
        KATA_SPOOL_VOLUME_TYPE,
    ];
    if !vol_types.contains(&mount_type) {
        return Ok(false);
    }

    match get_direct_volume_path(get_mount_path(m.source()).as_str()) {
        Ok(directvol_path) => {
            let fstat = stat::stat(directvol_path.as_str())
                .context(format!("stat mount source {directvol_path} failed."))?;
            Ok(SFlag::from_bits_truncate(fstat.st_mode) == SFlag::S_IFDIR)
        }
        Err(_) => Ok(false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::volume::VolumeResource;

    struct TestVolume {
        storage: agent::Storage,
        fail_cleanup: bool,
    }

    #[async_trait]
    impl Volume for TestVolume {
        fn get_volume_mount(&self) -> Result<Vec<oci::Mount>> {
            Ok(vec![])
        }

        fn get_storage(&self) -> Result<Vec<agent::Storage>> {
            Ok(vec![self.storage.clone()])
        }

        fn get_device_id(&self) -> Result<Option<String>> {
            Ok(None)
        }

        async fn cleanup(&self, _: &RwLock<DeviceManager>) -> Result<()> {
            if self.fail_cleanup {
                return Err(anyhow!("detach failed"));
            }
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_direct_volume_guest_path_lifecycle() {
        let devices = crate::test_utils::device_manager().await;
        let resources = VolumeResource::new();
        for (host, guest, fail_cleanup) in [
            (
                "/tmp/one.img",
                "/run/kata-containers/shared/one/data",
                false,
            ),
            ("/tmp/two.img", "/run/kata-containers/shared/two/data", true),
            (
                "/tmp/one.img",
                "/run/kata-containers/shared/replacement/data",
                false,
            ),
        ] {
            let volume = Arc::new(
                DirectVolume::new(
                    Arc::new(TestVolume {
                        storage: agent::Storage {
                            fs_type: "ext4".to_owned(),
                            mount_point: guest.to_owned(),
                            ..Default::default()
                        },
                        fail_cleanup,
                    }),
                    host.to_owned(),
                )
                .unwrap(),
            );
            resources.inner.write().await.volumes.push(volume.clone());
            assert_eq!(
                resources.direct_volume_guest_path(host).await.unwrap(),
                guest
            );
            assert!(resources
                .direct_volume_guest_path("/tmp/unknown.img")
                .await
                .is_err());
            assert!(resources.direct_volume_guest_path(guest).await.is_err());
            assert_eq!(volume.cleanup(&devices).await.is_err(), fail_cleanup);
            assert_eq!(
                resources.direct_volume_guest_path(host).await.is_ok(),
                fail_cleanup
            );
        }
    }
}
