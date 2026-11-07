// Copyright (c) 2026 NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::Result;
use hypervisor::device::device_manager::DeviceManager;
use tokio::sync::{Mutex, RwLock};

/// Device references a deleted container could not release.
#[derive(Debug, Default)]
pub(crate) struct OrphanedDeviceReferences {
    references: Mutex<Vec<String>>,
}

impl OrphanedDeviceReferences {
    pub(crate) async fn adopt(&self, device_ids: Vec<String>) {
        self.references.lock().await.extend(device_ids);
    }

    // Drop each reference as soon as it is released, so a cancelled call
    // never releases one twice.
    pub(crate) async fn release(&self, device_manager: &RwLock<DeviceManager>) -> Result<()> {
        let mut references = self.references.lock().await;
        let mut device_manager = device_manager.write().await;
        let mut failed = 0;
        let mut first_err = None;
        while failed < references.len() {
            let device_id = references[failed].clone();
            if !device_manager.has_device(&device_id) {
                warn!(
                    sl!(),
                    "orphaned reference to {} outlived its device", device_id
                );
                references.remove(failed);
                continue;
            }
            match device_manager.try_remove_device(&device_id).await {
                Ok(()) => {
                    references.remove(failed);
                }
                Err(err) => {
                    first_err.get_or_insert(err);
                    failed += 1;
                }
            }
        }

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
        self.references.lock().await.len()
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
}
