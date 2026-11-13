// Copyright (c) 2026 NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::{rootfs::Rootfs, volume::Volume};
use anyhow::Result;
use hypervisor::device::device_manager::DeviceManager;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};

/// Device references a deleted container could not release.
#[derive(Default)]
pub(crate) struct OrphanedDeviceReferences {
    references: Mutex<OrphanedResources>,
}

#[derive(Default)]
struct OrphanedResources {
    devices: Vec<String>,
    rootfs: Vec<Arc<dyn Rootfs>>,
    volumes: Vec<Arc<dyn Volume>>,
}

impl std::fmt::Debug for OrphanedDeviceReferences {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OrphanedDeviceReferences")
            .finish_non_exhaustive()
    }
}

impl OrphanedDeviceReferences {
    pub(crate) async fn adopt(&self, device_ids: Vec<String>) {
        self.adopt_resources(device_ids, vec![], vec![]).await;
    }

    pub(crate) async fn adopt_resources(
        &self,
        devices: Vec<String>,
        rootfs: Vec<Arc<dyn Rootfs>>,
        volumes: Vec<Arc<dyn Volume>>,
    ) {
        let mut references = self.references.lock().await;
        references.devices.extend(devices);
        references.rootfs.extend(rootfs);
        references.volumes.extend(volumes);
    }

    // Drop each reference as soon as it is released, so a cancelled call
    // never releases one twice.
    pub(crate) async fn release(&self, device_manager: &RwLock<DeviceManager>) -> Result<()> {
        let mut references = self.references.lock().await;
        let mut failed = 0;
        let mut first_err = None;
        while failed < references.devices.len() {
            let device_id = references.devices[failed].clone();
            if !device_manager.read().await.has_device(&device_id) {
                warn!(
                    sl!(),
                    "orphaned reference to {} outlived its device", device_id
                );
                references.devices.remove(failed);
                continue;
            }
            match DeviceManager::remove_device(device_manager, &device_id).await {
                Ok(()) => {
                    references.devices.remove(failed);
                }
                Err(err) => {
                    first_err.get_or_insert(err);
                    failed += 1;
                }
            }
        }

        let device_failures = failed;
        let mut failed = 0;
        while failed < references.rootfs.len() {
            match references.rootfs[failed].cleanup(device_manager).await {
                Ok(()) => {
                    references.rootfs.remove(failed);
                }
                Err(err) => {
                    first_err.get_or_insert(err);
                    failed += 1;
                }
            }
        }
        let rootfs_failures = failed;
        let mut failed = 0;
        while failed < references.volumes.len() {
            match references.volumes[failed].cleanup(device_manager).await {
                Ok(()) => {
                    references.volumes.remove(failed);
                }
                Err(err) => {
                    first_err.get_or_insert(err);
                    failed += 1;
                }
            }
        }
        let failed = device_failures + rootfs_failures + failed;

        match first_err {
            Some(err) => Err(err.context(format!(
                "failed to release {} orphaned device reference(s)",
                failed
            ))),
            None => Ok(()),
        }
    }

    #[cfg(test)]
    async fn len(&self) -> usize {
        {
            let references = self.references.lock().await;
            references.devices.len() + references.rootfs.len() + references.volumes.len()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::device_manager;
    use hypervisor::{
        device::{device_manager::do_handle_device, DeviceConfig, DeviceType},
        BlockConfigModern,
    };

    #[rstest::rstest]
    #[case::shared_device_is_released(2, true, Some(1), 0)]
    #[case::refused_removal_is_kept(1, true, Some(1), 1)]
    #[case::reference_without_device_is_dropped(1, false, Some(1), 0)]
    #[tokio::test]
    async fn orphaned_references_are_retried(
        #[case] owners: usize,
        #[case] device_exists: bool,
        #[case] count_after: Option<u64>,
        #[case] still_orphaned: usize,
    ) {
        let d = device_manager().await;
        let config = DeviceConfig::BlockCfgModern(BlockConfigModern {
            path_on_host: "/dev/orphaned-test-device".to_string(),
            driver_option: "virtio-blk-pci".to_string(),
            ..Default::default()
        });
        let mut device = None;
        for _ in 0..owners {
            match do_handle_device(&d, &config).await.unwrap() {
                DeviceType::BlockModern(dev) => device = Some(dev),
                other => panic!("unexpected device {}", other),
            }
        }
        let device = device.unwrap();
        let device_id = device.lock().await.device_id.clone();
        let orphans = OrphanedDeviceReferences::default();
        orphans
            .adopt(vec![if device_exists {
                device_id.clone()
            } else {
                "deleted-device".to_string()
            }])
            .await;

        let released = orphans.release(&d).await;

        assert_eq!(released.is_ok(), still_orphaned == 0);
        let count = d
            .read()
            .await
            .has_device(&device_id)
            .then_some(device.lock().await.attach_count);
        assert_eq!(count, count_after);
        assert_eq!(orphans.len().await, still_orphaned);
    }
    struct PendingMount {
        fail: Arc<std::sync::atomic::AtomicBool>,
        id: String,
    }

    impl PendingMount {
        async fn release(&self, manager: &RwLock<DeviceManager>) -> Result<()> {
            if self.fail.load(std::sync::atomic::Ordering::SeqCst) {
                anyhow::bail!("scripted mount cleanup failure");
            }
            DeviceManager::remove_device(manager, &self.id).await
        }
    }

    #[async_trait::async_trait]
    impl Rootfs for PendingMount {
        async fn get_guest_rootfs_path(&self) -> Result<String> {
            Ok(String::new())
        }
        async fn get_rootfs_mount(&self) -> Result<Vec<oci_spec::runtime::Mount>> {
            Ok(vec![])
        }
        async fn get_storage(&self) -> Option<Vec<agent::Storage>> {
            None
        }
        async fn get_device_ids(&self) -> Result<Vec<String>> {
            Ok(vec![self.id.clone()])
        }
        async fn cleanup(&self, manager: &RwLock<DeviceManager>) -> Result<()> {
            self.release(manager).await
        }
    }

    #[async_trait::async_trait]
    impl Volume for PendingMount {
        fn get_volume_mount(&self) -> Result<Vec<oci_spec::runtime::Mount>> {
            Ok(vec![])
        }
        fn get_storage(&self) -> Result<Vec<agent::Storage>> {
            Ok(vec![])
        }
        fn get_device_id(&self) -> Result<Option<String>> {
            Ok(Some(self.id.clone()))
        }
        async fn cleanup(&self, manager: &RwLock<DeviceManager>) -> Result<()> {
            self.release(manager).await
        }
    }

    #[tokio::test]
    async fn failed_rootfs_and_volume_cleanup_keep_their_owners() {
        use std::sync::atomic::{AtomicBool, Ordering};
        let manager = device_manager().await;
        let config = DeviceConfig::BlockCfgModern(BlockConfigModern {
            path_on_host: "/dev/orphaned-mount-device".into(),
            driver_option: "virtio-blk-pci".into(),
            ..Default::default()
        });
        let mut device = None;
        // Container device, rootfs, volume, and a sibling's reference.
        for _ in 0..4 {
            let DeviceType::BlockModern(block) = do_handle_device(&manager, &config).await.unwrap()
            else {
                unreachable!()
            };
            device = Some(block);
        }
        let device = device.unwrap();
        let id = device.lock().await.device_id.clone();
        let fail = Arc::new(AtomicBool::new(true));
        let rootfs = Arc::new(PendingMount {
            fail: fail.clone(),
            id: id.clone(),
        });
        let volume = Arc::new(PendingMount {
            fail: fail.clone(),
            id: id.clone(),
        });
        let rootfs_owner = Arc::downgrade(&rootfs);
        let volume_owner = Arc::downgrade(&volume);
        let orphans = OrphanedDeviceReferences::default();
        orphans
            .adopt_resources(vec![id], vec![rootfs], vec![volume])
            .await;
        assert!(orphans.release(&manager).await.is_err());
        assert_eq!(device.lock().await.attach_count, 3);
        assert_eq!(orphans.len().await, 2);
        assert!(rootfs_owner.upgrade().is_some());
        assert!(volume_owner.upgrade().is_some());

        fail.store(false, Ordering::SeqCst);
        orphans.release(&manager).await.unwrap();
        orphans.release(&manager).await.unwrap();
        assert_eq!(device.lock().await.attach_count, 1);
        assert_eq!(orphans.len().await, 0);
        assert!(rootfs_owner.upgrade().is_none());
        assert!(volume_owner.upgrade().is_none());
    }
}
