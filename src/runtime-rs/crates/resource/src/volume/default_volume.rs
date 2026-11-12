// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use hypervisor::device::device_manager::DeviceManager;
use tokio::sync::RwLock;

use super::Volume;
use anyhow::Result;
use async_trait::async_trait;
use oci_spec::runtime as oci;

#[derive(Debug)]
pub(crate) struct DefaultVolume {
    mount: oci::Mount,
}

/// Pass mounts through to the guest without creating host resources.
impl DefaultVolume {
    pub fn new(mount: &oci::Mount) -> Result<Self> {
        Ok(Self {
            mount: mount.clone(),
        })
    }
}

#[async_trait]
impl Volume for DefaultVolume {
    fn get_volume_mount(&self) -> anyhow::Result<Vec<oci::Mount>> {
        Ok(vec![self.mount.clone()])
    }

    fn get_storage(&self) -> Result<Vec<agent::Storage>> {
        Ok(vec![])
    }

    async fn cleanup(&self, _device_manager: &RwLock<DeviceManager>) -> Result<()> {
        // These mounts are managed in the guest; there are no host resources to release.
        Ok(())
    }

    fn get_device_id(&self) -> Result<Option<String>> {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use slog::Drain;

    #[tokio::test]
    async fn test_default_volume_cleanup_is_a_quiet_noop() {
        let device_manager = crate::test_utils::device_manager().await;
        let logger = slog::Logger::root(
            slog::Filter::new(slog::Discard, |record| {
                assert!(
                    !matches!(
                        record.level(),
                        slog::Level::Warning | slog::Level::Error | slog::Level::Critical
                    ),
                    "unexpected cleanup diagnostic: {}",
                    record.msg()
                );
                true
            })
            .fuse(),
            slog::o!(),
        );

        for (typ, destination) in [
            ("proc", "/proc"),
            ("tmpfs", "/dev"),
            ("devpts", "/dev/pts"),
            ("mqueue", "/dev/mqueue"),
            ("sysfs", "/sys"),
            ("cgroup", "/sys/fs/cgroup"),
        ] {
            let mut mount = oci::Mount::default();
            mount.set_typ(Some(typ.to_string()));
            mount.set_source(Some(typ.into()));
            mount.set_destination(destination.into());
            mount.set_options(Some(vec!["nosuid".to_string(), "noexec".to_string()]));
            let volume = DefaultVolume::new(&mount).unwrap();

            slog_scope::scope(&logger, || {
                futures::executor::block_on(volume.cleanup(&device_manager)).unwrap();
                futures::executor::block_on(volume.cleanup(&device_manager)).unwrap();
            });

            assert_eq!(volume.get_volume_mount().unwrap(), vec![mount]);
            assert!(volume.get_storage().unwrap().is_empty());
            assert_eq!(volume.get_device_id().unwrap(), None);
        }
    }
}
