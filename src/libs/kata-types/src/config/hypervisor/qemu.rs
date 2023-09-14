// Copyright (c) 2019-2021 Alibaba Cloud
//
// SPDX-License-Identifier: Apache-2.0
//

use std::io::Result;
use std::path::Path;
use std::sync::Arc;

use super::{default, register_hypervisor_plugin};

use crate::config::default::MAX_QEMU_VCPUS;
use crate::config::default::MIN_QEMU_MEMORY_SIZE_MB;

use crate::config::hypervisor::VIRTIO_BLK_MMIO;
use crate::config::{ConfigPlugin, TomlConfig};
use crate::{eother, resolve_path, validate_path};

/// Hypervisor name for qemu, used to index `TomlConfig::hypervisor`.
pub const HYPERVISOR_NAME_QEMU: &str = "qemu";

/// Configuration information for qemu.
#[derive(Default, Debug)]
pub struct QemuConfig {}

impl QemuConfig {
    /// Create a new instance of `QemuConfig`.
    pub fn new() -> Self {
        QemuConfig {}
    }

    /// Register the qemu plugin.
    pub fn register(self) {
        let plugin = Arc::new(self);
        register_hypervisor_plugin(HYPERVISOR_NAME_QEMU, plugin);
    }
}

impl ConfigPlugin for QemuConfig {
    fn get_max_cpus(&self) -> u32 {
        MAX_QEMU_VCPUS
    }

    fn get_min_memory(&self) -> u32 {
        MIN_QEMU_MEMORY_SIZE_MB
    }
    fn name(&self) -> &str {
        HYPERVISOR_NAME_QEMU
    }

    /// Adjust the configuration information after loading from configuration file.
    fn adjust_config(&self, conf: &mut TomlConfig) -> Result<()> {
        if let Some(qemu) = conf.hypervisor.get_mut(HYPERVISOR_NAME_QEMU) {
            if qemu.path.is_empty() {
                qemu.path = default::DEFAULT_QEMU_BINARY_PATH.to_string();
            }
            resolve_path!(qemu.path, "Qemu binary path `{}` is invalid: {}")?;
            if qemu.boot_info.rootfs_type.is_empty() {
                qemu.boot_info.rootfs_type = default::DEFAULT_QEMU_ROOTFS_TYPE.to_string();
            }
            if qemu.ctlpath.is_empty() {
                qemu.ctlpath = default::DEFAULT_QEMU_CONTROL_PATH.to_string();
            }
            resolve_path!(qemu.ctlpath, "Qemu ctlpath `{}` is invalid: {}")?;

            if qemu.boot_info.kernel.is_empty() {
                qemu.boot_info.kernel = default::DEFAULT_QEMU_GUEST_KERNEL_IMAGE.to_string();
            }
            if qemu.boot_info.kernel_params.is_empty() {
                qemu.boot_info.kernel_params =
                    default::DEFAULT_QEMU_GUEST_KERNEL_PARAMS.to_string();
            }
            if qemu.boot_info.firmware.is_empty() {
                qemu.boot_info.firmware = default::DEFAULT_QEMU_FIRMWARE_PATH.to_string();
            }

            if qemu.device_info.default_bridges == 0 {
                qemu.device_info.default_bridges = default::DEFAULT_QEMU_PCI_BRIDGES;
            }

            if qemu.machine_info.machine_type.is_empty() {
                qemu.machine_info.machine_type = default::DEFAULT_QEMU_MACHINE_TYPE.to_string();
            }
            if qemu.machine_info.entropy_source.is_empty() {
                qemu.machine_info.entropy_source = default::DEFAULT_QEMU_ENTROPY_SOURCE.to_string();
            }

            if qemu.memory_info.default_memory == 0 {
                qemu.memory_info.default_memory = default::DEFAULT_QEMU_MEMORY_SIZE_MB;
            }
            if qemu.memory_info.memory_slots == 0 {
                qemu.memory_info.memory_slots = default::DEFAULT_QEMU_MEMORY_SLOTS;
            }
        }

        Ok(())
    }

    /// Validate the configuration information.
    fn validate(&self, conf: &TomlConfig) -> Result<()> {
        if let Some(qemu) = conf.hypervisor.get(HYPERVISOR_NAME_QEMU) {
            validate_path!(qemu.path, "QEMU binary path `{}` is invalid: {}")?;
            validate_path!(qemu.ctlpath, "QEMU control path `{}` is invalid: {}")?;
            if !qemu.jailer_path.is_empty() {
                return Err(eother!("Path for QEMU jailer should be empty"));
            }
            if !qemu.valid_jailer_paths.is_empty() {
                return Err(eother!("Valid Qemu jailer path list should be empty"));
            }

            if !qemu.blockdev_info.disable_block_device_use
                && qemu.blockdev_info.block_device_driver == VIRTIO_BLK_MMIO
            {
                return Err(eother!("Qemu doesn't support virtio-blk-mmio"));
            }

            if qemu.boot_info.kernel.is_empty() {
                return Err(eother!("Guest kernel image for qemu is empty"));
            }
            if qemu.boot_info.image.is_empty() && qemu.boot_info.initrd.is_empty() {
                return Err(eother!(
                    "Both guest boot image and initrd for qemu are empty"
                ));
            }

            if (qemu.cpu_info.default_vcpus > 0
                && qemu.cpu_info.default_vcpus as u32 > default::MAX_QEMU_VCPUS)
                || qemu.cpu_info.default_maxvcpus > default::MAX_QEMU_VCPUS
            {
                return Err(eother!(
                    "Qemu hypervisor can not support {} vCPUs",
                    qemu.cpu_info.default_maxvcpus
                ));
            }

            if qemu.device_info.default_bridges > default::MAX_QEMU_PCI_BRIDGES {
                return Err(eother!(
                    "Qemu hypervisor can not support {} PCI bridges",
                    qemu.device_info.default_bridges
                ));
            }

            if qemu.memory_info.default_memory < MIN_QEMU_MEMORY_SIZE_MB {
                return Err(eother!(
                    "Qemu hypervisor has minimal memory limitation {}",
                    MIN_QEMU_MEMORY_SIZE_MB
                ));
            }
        }

        Ok(())
    }
}
