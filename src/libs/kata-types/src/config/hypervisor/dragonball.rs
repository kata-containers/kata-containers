// Copyright (c) 2019-2021 Alibaba Cloud
//
// SPDX-License-Identifier: Apache-2.0
//

use std::io::Result;
use std::path::Path;
use std::sync::Arc;
use std::u32;

use super::{default, register_hypervisor_plugin};
use crate::config::default::MAX_DRAGONBALL_VCPUS;
use crate::config::default::MIN_DRAGONBALL_MEMORY_SIZE_MB;
use crate::config::hypervisor::{
    VIRTIO_BLK_MMIO, VIRTIO_BLK_PCI, VIRTIO_FS, VIRTIO_FS_INLINE, VIRTIO_PMEM,
};
use crate::config::{ConfigPlugin, TomlConfig};
use crate::{eother, resolve_path, validate_path};

/// Hypervisor name for dragonball, used to index `TomlConfig::hypervisor`.
pub const HYPERVISOR_NAME_DRAGONBALL: &str = "dragonball";

/// Configuration information for dragonball.
#[derive(Default, Debug)]
pub struct DragonballConfig {}

impl DragonballConfig {
    /// Create a new instance of `DragonballConfig`.
    pub fn new() -> Self {
        DragonballConfig {}
    }

    /// Register the dragonball plugin.
    pub fn register(self) {
        let plugin = Arc::new(self);
        register_hypervisor_plugin(HYPERVISOR_NAME_DRAGONBALL, plugin);
    }
}

impl ConfigPlugin for DragonballConfig {
    fn get_max_cpus(&self) -> u32 {
        MAX_DRAGONBALL_VCPUS
    }
    fn get_min_memory(&self) -> u32 {
        MIN_DRAGONBALL_MEMORY_SIZE_MB
    }
    fn name(&self) -> &str {
        HYPERVISOR_NAME_DRAGONBALL
    }

    /// Adjust the configuration information after loading from configuration file.
    fn adjust_config(&self, conf: &mut TomlConfig) -> Result<()> {
        if let Some(db) = conf.hypervisor.get_mut(HYPERVISOR_NAME_DRAGONBALL) {
            resolve_path!(db.jailer_path, "dragonball jailer path {} is invalid: {}")?;

            if db.boot_info.kernel.is_empty() {
                db.boot_info.kernel = default::DEFAULT_DRAGONBALL_GUEST_KERNEL_IMAGE.to_string();
            }
            if db.boot_info.kernel_params.is_empty() {
                db.boot_info.kernel_params =
                    default::DEFAULT_DRAGONBALL_GUEST_KERNEL_PARAMS.to_string();
            }

            if db.cpu_info.default_maxvcpus > default::MAX_DRAGONBALL_VCPUS {
                db.cpu_info.default_maxvcpus = default::MAX_DRAGONBALL_VCPUS;
            }

            if db.cpu_info.default_vcpus as u32 > db.cpu_info.default_maxvcpus {
                db.cpu_info.default_vcpus = db.cpu_info.default_maxvcpus as i32;
            }

            if db.machine_info.entropy_source.is_empty() {
                db.machine_info.entropy_source =
                    default::DEFAULT_DRAGONBALL_ENTROPY_SOURCE.to_string();
            }

            if db.memory_info.default_memory == 0 {
                db.memory_info.default_memory = default::DEFAULT_DRAGONBALL_MEMORY_SIZE_MB;
            }
            if db.memory_info.memory_slots == 0 {
                db.memory_info.memory_slots = default::DEFAULT_DRAGONBALL_MEMORY_SLOTS;
            }
        }
        Ok(())
    }

    /// Validate the configuration information.
    fn validate(&self, conf: &TomlConfig) -> Result<()> {
        if let Some(db) = conf.hypervisor.get(HYPERVISOR_NAME_DRAGONBALL) {
            if !db.path.is_empty() {
                return Err(eother!("Path for dragonball hypervisor should be empty"));
            }
            if !db.valid_hypervisor_paths.is_empty() {
                return Err(eother!(
                    "Valid hypervisor path for dragonball hypervisor should be empty"
                ));
            }
            if !db.ctlpath.is_empty() {
                return Err(eother!("CtlPath for dragonball hypervisor should be empty"));
            }
            if !db.valid_ctlpaths.is_empty() {
                return Err(eother!("CtlPath for dragonball hypervisor should be empty"));
            }
            validate_path!(db.jailer_path, "dragonball jailer path {} is invalid: {}")?;
            if db.enable_iothreads {
                return Err(eother!("dragonball hypervisor doesn't support IO threads."));
            }

            if !db.blockdev_info.disable_block_device_use
                && db.blockdev_info.block_device_driver != VIRTIO_BLK_PCI
                && db.blockdev_info.block_device_driver != VIRTIO_BLK_MMIO
                && db.blockdev_info.block_device_driver != VIRTIO_PMEM
            {
                return Err(eother!(
                    "{} is unsupported block device type.",
                    db.blockdev_info.block_device_driver
                ));
            }

            if db.boot_info.kernel.is_empty() {
                return Err(eother!(
                    "Guest kernel image for dragonball hypervisor is empty"
                ));
            }

            if db.boot_info.image.is_empty() && db.boot_info.initrd.is_empty() {
                return Err(eother!(
                    "Both of guest boot image and initrd for dragonball hypervisor is empty"
                ));
            }

            if !db.boot_info.firmware.is_empty() {
                return Err(eother!(
                    "Firmware for dragonball hypervisor should be empty"
                ));
            }

            if (db.cpu_info.default_vcpus > 0
                && db.cpu_info.default_vcpus as u32 > default::MAX_DRAGONBALL_VCPUS)
                || db.cpu_info.default_maxvcpus > default::MAX_DRAGONBALL_VCPUS
            {
                return Err(eother!(
                    "dragonball hypervisor can not support {} vCPUs",
                    db.cpu_info.default_maxvcpus
                ));
            }

            if db.device_info.enable_iommu || db.device_info.enable_iommu_platform {
                return Err(eother!("dragonball hypervisor does not support vIOMMU"));
            }
            if db.device_info.hotplug_vfio_on_root_bus
                || db.device_info.default_bridges > 0
                || db.device_info.pcie_root_port > 0
            {
                return Err(eother!(
                    "dragonball hypervisor does not support PCI hotplug options"
                ));
            }

            if !db.machine_info.machine_type.is_empty() {
                return Err(eother!(
                    "dragonball hypervisor does not support machine_type"
                ));
            }
            if !db.machine_info.pflashes.is_empty() {
                return Err(eother!("dragonball hypervisor does not support pflashes"));
            }

            if db.memory_info.enable_guest_swap {
                return Err(eother!(
                    "dragonball hypervisor doesn't support enable_guest_swap"
                ));
            }

            if db.security_info.rootless {
                return Err(eother!(
                    "dragonball hypervisor does not support rootless mode"
                ));
            }

            if let Some(v) = db.shared_fs.shared_fs.as_ref() {
                if v != VIRTIO_FS && v != VIRTIO_FS_INLINE {
                    return Err(eother!("dragonball hypervisor doesn't support {}", v));
                }
            }

            if db.memory_info.default_memory < MIN_DRAGONBALL_MEMORY_SIZE_MB {
                return Err(eother!(
                    "dragonball hypervisor has minimal memory limitation {}",
                    MIN_DRAGONBALL_MEMORY_SIZE_MB
                ));
            }
        }

        Ok(())
    }
}
