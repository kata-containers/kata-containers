// Copyright (c) 2021 Alibaba Cloud
//
// SPDX-License-Identifier: Apache-2.0
//

//! Default configuration values.
#![allow(missing_docs)]

use crate::config::agent::AGENT_NAME_KATA;
use crate::config::hypervisor::HYPERVISOR_NAME_DRAGONBALL;
use crate::config::runtime::RUNTIME_NAME_VIRTCONTAINER;
use lazy_static::lazy_static;

lazy_static! {
    /// Default configuration file paths, vendor may extend the list
    pub static ref DEFAULT_RUNTIME_CONFIGURATIONS: Vec::<&'static str> = vec![
        "/etc/kata-containers/configuration.toml",
        "/usr/share/defaults/kata-containers/configuration.toml",
    ];
}
pub const DEFAULT_AGENT_NAME: &str = "kata-agent";
pub const DEFAULT_AGENT_VSOCK_PORT: u32 = 1024;
pub const DEFAULT_AGENT_LOG_PORT: u32 = 1025;
pub const DEFAULT_AGENT_TYPE_NAME: &str = AGENT_NAME_KATA;

pub const DEFAULT_RUNTIME_NAME: &str = RUNTIME_NAME_VIRTCONTAINER;
pub const DEFAULT_HYPERVISOR: &str = HYPERVISOR_NAME_DRAGONBALL;

pub const DEFAULT_INTERNETWORKING_MODEL: &str = "tcfilter";

pub const DEFAULT_BLOCK_DEVICE_TYPE: &str = "virtio-blk";
pub const DEFAULT_VHOST_USER_STORE_PATH: &str = "/var/run/vhost-user";
pub const DEFAULT_BLOCK_NVDIMM_MEM_OFFSET: u64 = 0;

pub const DEFAULT_SHARED_FS_TYPE: &str = "virtio-fs";
pub const DEFAULT_VIRTIO_FS_CACHE_MODE: &str = "none";
pub const DEFAULT_VIRTIO_FS_DAX_SIZE_MB: u32 = 1024;
pub const DEFAULT_SHARED_9PFS_SIZE_MB: u32 = 128 * 1024;
pub const MIN_SHARED_9PFS_SIZE_MB: u32 = 4 * 1024;
pub const MAX_SHARED_9PFS_SIZE_MB: u32 = 8 * 1024 * 1024;

pub const DEFAULT_GUEST_HOOK_PATH: &str = "/opt/kata/hooks";

pub const DEFAULT_GUEST_VCPUS: u32 = 1;

// Default configuration for dragonball
pub const DEFAULT_DRAGONBALL_GUEST_KERNEL_IMAGE: &str = "vmlinuz";
pub const DEFAULT_DRAGONBALL_GUEST_KERNEL_PARAMS: &str = "";
pub const DEFAULT_DRAGONBALL_ENTROPY_SOURCE: &str = "/dev/urandom";
pub const DEFAULT_DRAGONBALL_MEMORY_SIZE_MB: u32 = 128;
pub const DEFAULT_DRAGONBALL_MEMORY_SLOTS: u32 = 128;
pub const MAX_DRAGONBALL_VCPUS: u32 = 256;
pub const MIN_DRAGONBALL_MEMORY_SIZE_MB: u32 = 64;
// Default configuration for qemu
pub const DEFAULT_QEMU_BINARY_PATH: &str = "/usr/bin/qemu-system-x86_64";
pub const DEFAULT_QEMU_CONTROL_PATH: &str = "";
pub const DEFAULT_QEMU_MACHINE_TYPE: &str = "q35";
pub const DEFAULT_QEMU_ENTROPY_SOURCE: &str = "/dev/urandom";
pub const DEFAULT_QEMU_GUEST_KERNEL_IMAGE: &str = "vmlinuz";
pub const DEFAULT_QEMU_GUEST_KERNEL_PARAMS: &str = "";
pub const DEFAULT_QEMU_FIRMWARE_PATH: &str = "";
pub const DEFAULT_QEMU_MEMORY_SIZE_MB: u32 = 128;
pub const DEFAULT_QEMU_MEMORY_SLOTS: u32 = 128;
pub const DEFAULT_QEMU_PCI_BRIDGES: u32 = 2;
pub const MAX_QEMU_PCI_BRIDGES: u32 = 5;
pub const MAX_QEMU_VCPUS: u32 = 256;
pub const MIN_QEMU_MEMORY_SIZE_MB: u32 = 64;
