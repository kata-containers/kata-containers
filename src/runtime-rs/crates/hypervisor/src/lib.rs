// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

#[macro_use]
extern crate slog;

logging::logger_with_subsystem!(sl, "hypervisor");

pub mod device;
pub mod hypervisor_persist;
pub use device::*;
pub mod dragonball;
mod kernel_param;
pub mod qemu;
pub use kernel_param::Param;
mod utils;
use std::collections::HashMap;

#[cfg(feature = "cloud-hypervisor")]
pub mod ch;

use anyhow::Result;
use async_trait::async_trait;
use hypervisor_persist::HypervisorState;
use kata_types::capabilities::Capabilities;
use kata_types::config::hypervisor::Hypervisor as HypervisorConfig;

pub use kata_types::config::hypervisor::HYPERVISOR_NAME_CH;

// Config which driver to use as vm root dev
const VM_ROOTFS_DRIVER_BLK: &str = "virtio-blk";
const VM_ROOTFS_DRIVER_PMEM: &str = "virtio-pmem";

//Configure the root corresponding to the driver
const VM_ROOTFS_ROOT_BLK: &str = "/dev/vda1";
const VM_ROOTFS_ROOT_PMEM: &str = "/dev/pmem0p1";

// Config which filesystem to use as rootfs type
const VM_ROOTFS_FILESYSTEM_EXT4: &str = "ext4";
const VM_ROOTFS_FILESYSTEM_XFS: &str = "xfs";
const VM_ROOTFS_FILESYSTEM_EROFS: &str = "erofs";

// before using hugepages for VM, we need to mount hugetlbfs
// /dev/hugepages will be the mount point
// mkdir -p /dev/hugepages
// mount -t hugetlbfs none /dev/hugepages
const DEV_HUGEPAGES: &str = "/dev/hugepages";
pub const HUGETLBFS: &str = "hugetlbfs";
const SHMEM: &str = "shmem";
const HUGE_SHMEM: &str = "hugeshmem";

pub const HYPERVISOR_DRAGONBALL: &str = "dragonball";
pub const HYPERVISOR_QEMU: &str = "qemu";

#[derive(PartialEq, Debug, Clone)]
pub(crate) enum VmmState {
    NotReady,
    VmmServerReady,
    VmRunning,
}

// vcpu mapping from vcpu number to thread number
#[derive(Debug, Default)]
pub struct VcpuThreadIds {
    pub vcpus: HashMap<u32, u32>,
}

#[async_trait]
pub trait Hypervisor: Send + Sync {
    // vm manager
    async fn prepare_vm(&self, id: &str, netns: Option<String>) -> Result<()>;
    async fn start_vm(&self, timeout: i32) -> Result<()>;
    async fn stop_vm(&self) -> Result<()>;
    async fn pause_vm(&self) -> Result<()>;
    async fn save_vm(&self) -> Result<()>;
    async fn resume_vm(&self) -> Result<()>;

    // device manager
    async fn add_device(&self, device: device::Device) -> Result<()>;
    async fn remove_device(&self, device: device::Device) -> Result<()>;

    // utils
    async fn get_agent_socket(&self) -> Result<String>;
    async fn disconnect(&self);
    async fn hypervisor_config(&self) -> HypervisorConfig;
    async fn get_thread_ids(&self) -> Result<VcpuThreadIds>;
    async fn get_pids(&self) -> Result<Vec<u32>>;
    async fn get_vmm_master_tid(&self) -> Result<u32>;
    async fn get_ns_path(&self) -> Result<String>;
    async fn cleanup(&self) -> Result<()>;
    async fn check(&self) -> Result<()>;
    async fn get_jailer_root(&self) -> Result<String>;
    async fn save_state(&self) -> Result<HypervisorState>;
    async fn capabilities(&self) -> Result<Capabilities>;
}
