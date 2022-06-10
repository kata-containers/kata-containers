// Copyright (c) 2021 Alibaba Cloud
//
// SPDX-License-Identifier: Apache-2.0
//

//! Default configuration values.
#![allow(missing_docs)]

use lazy_static::lazy_static;

lazy_static! {
    /// Default configuration file paths, vendor may extend the list
    pub static ref DEFAULT_RUNTIME_CONFIGURATIONS: Vec::<&'static str> = vec![
        "/etc/kata-containers/configuration.toml",
        "/usr/share/defaults/kata-containers/configuration.toml",
    ];
}
pub const DEFAULT_AGENT_NAME: &str = "kata";

pub const DEFAULT_INTERNETWORKING_MODEL: &str = "tcfilter";

pub const DEFAULT_BLOCK_DEVICE_TYPE: &str = "virtio-blk";
pub const DEFAULT_VHOST_USER_STORE_PATH: &str = "/var/run/vhost-user";
pub const DEFAULT_BLOCK_NVDIMM_MEM_OFFSET: u64 = 0;

pub const DEFAULT_SHARED_FS_TYPE: &str = "virtio-9p";
pub const DEFAULT_VIRTIO_FS_CACHE_MODE: &str = "none";
pub const DEFAULT_VIRTIO_FS_DAX_SIZE_MB: u32 = 1024;
pub const DEFAULT_SHARED_9PFS_SIZE: u32 = 128 * 1024;
pub const MIN_SHARED_9PFS_SIZE: u32 = 4 * 1024;
pub const MAX_SHARED_9PFS_SIZE: u32 = 8 * 1024 * 1024;

pub const DEFAULT_GUEST_HOOK_PATH: &str = "/opt";

pub const DEFAULT_GUEST_VCPUS: u32 = 1;

// Default configuration for Dragonball
pub const DEFAULT_DB_GUEST_KERNEL_IMAGE: &str = "vmlinuz";
pub const DEFAULT_DB_GUEST_KERNEL_PARAMS: &str = "";
pub const DEFAULT_DB_ENTROPY_SOURCE: &str = "/dev/urandom";
pub const DEFAULT_DB_MEMORY_SIZE: u32 = 128;
pub const DEFAULT_DB_MEMORY_SLOTS: u32 = 128;
pub const MAX_DB_VCPUS: u32 = 256;
pub const MIN_DB_MEMORY_SIZE: u32 = 64;
// Default configuration for qemu
pub const DEFAULT_QEMU_BINARY_PATH: &str = "qemu";
pub const DEFAULT_QEMU_CONTROL_PATH: &str = "";
pub const DEFAULT_QEMU_MACHINE_TYPE: &str = "q35";
pub const DEFAULT_QEMU_ENTROPY_SOURCE: &str = "/dev/urandom";
pub const DEFAULT_QEMU_GUEST_KERNEL_IMAGE: &str = "vmlinuz";
pub const DEFAULT_QEMU_GUEST_KERNEL_PARAMS: &str = "";
pub const DEFAULT_QEMU_FIRMWARE_PATH: &str = "";
pub const DEFAULT_QEMU_MEMORY_SIZE: u32 = 128;
pub const DEFAULT_QEMU_MEMORY_SLOTS: u32 = 128;
pub const DEFAULT_QEMU_PCI_BRIDGES: u32 = 2;
pub const MAX_QEMU_PCI_BRIDGES: u32 = 5;
pub const MAX_QEMU_VCPUS: u32 = 256;
pub const MIN_QEMU_MEMORY_SIZE: u32 = 64;
