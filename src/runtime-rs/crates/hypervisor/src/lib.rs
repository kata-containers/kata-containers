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
pub use device::driver::*;
use device::DeviceType;
#[cfg(all(
    feature = "dragonball",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
pub mod dragonball;
// Firecracker upstream only releases binaries for x86_64 and aarch64
// (see https://github.com/firecracker-microvm/firecracker/releases), so there
// is no point compiling the in-tree driver on other architectures. Use the
// same architecture gate as `ch` (further down in this file) for consistency.
#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
pub mod firecracker;
mod kernel_param;
pub mod qemu;
pub mod remote;
pub mod selinux;
pub use kernel_param::Param;
pub mod utils;
use std::collections::HashMap;

#[cfg(all(
    feature = "cloud-hypervisor",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
pub mod ch;

use anyhow::Result;
use async_trait::async_trait;
use hypervisor_persist::HypervisorState;
use kata_types::capabilities::{Capabilities, CapabilityBits};
use kata_types::config::hypervisor::Hypervisor as HypervisorConfig;

pub use kata_types::config::hypervisor::HYPERVISOR_NAME_CH;

// Config which driver to use as vm root dev
const VM_ROOTFS_DRIVER_BLK: &str = "virtio-blk-pci";
const VM_ROOTFS_DRIVER_BLK_CCW: &str = "virtio-blk-ccw";
const VM_ROOTFS_DRIVER_PMEM: &str = "virtio-pmem";
const VM_ROOTFS_DRIVER_MMIO: &str = "virtio-blk-mmio";

//Configure the root corresponding to the driver
const VM_ROOTFS_ROOT_BLK: &str = "/dev/vda1";
const VM_ROOTFS_ROOT_PMEM: &str = "/dev/pmem0p1";

// before using hugepages for VM, we need to mount hugetlbfs
// /dev/hugepages will be the mount point
// mkdir -p /dev/hugepages
// mount -t hugetlbfs none /dev/hugepages
pub const HUGETLBFS: &str = "hugetlbfs";
// Constants required for Dragonball VMM when enabled.
// Gated on both feature and arch so they activate together with `pub mod
// dragonball;` above (the dragonball crate only builds on x86_64/aarch64).
#[cfg(all(
    feature = "dragonball",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
const DEV_HUGEPAGES: &str = "/dev/hugepages";
#[cfg(all(
    feature = "dragonball",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
const SHMEM: &str = "shmem";
#[cfg(all(
    feature = "dragonball",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
const HUGE_SHMEM: &str = "hugeshmem";

pub const HYPERVISOR_DRAGONBALL: &str = "dragonball";
pub const HYPERVISOR_QEMU: &str = "qemu";
pub const HYPERVISOR_FIRECRACKER: &str = "firecracker";
pub const HYPERVISOR_REMOTE: &str = "remote";

pub const DEFAULT_HYBRID_VSOCK_NAME: &str = "kata.hvsock";
pub const JAILER_ROOT: &str = "root";

/// default hotplug timeout
#[allow(dead_code)]
const DEFAULT_HOTPLUG_TIMEOUT: u64 = 250;

// Used only by the dragonball, cloud-hypervisor and firecracker drivers, all
// of which are gated to `target_arch = "x86_64"|"aarch64"`. Without the gate
// here, `cargo clippy --all-features -- -D warnings` (i.e. what `make check`
// runs via utils.mk's `standard_rust_check`) fails on s390x/ppc64le/riscv64gc
// with `enum VmmState is never used`.
#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
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

#[derive(Debug, Default)]
pub struct MemoryConfig {
    pub slot: u32,
    pub size_mb: u32,
    pub addr: u64,
    pub probe: bool,
}

#[async_trait]
pub trait Hypervisor: std::fmt::Debug + Send + Sync {
    // vm manager
    async fn prepare_vm(
        &self,
        id: &str,
        netns: Option<String>,
        annotations: &HashMap<String, String>,
        selinux_label: Option<String>,
    ) -> Result<()>;
    async fn start_vm(&self, timeout: i32) -> Result<()>;
    async fn stop_vm(&self) -> Result<()>;
    async fn wait_vm(&self) -> Result<i32>;
    async fn pause_vm(&self) -> Result<()>;
    async fn save_vm(&self) -> Result<()>;
    async fn resume_vm(&self) -> Result<()>;
    async fn resize_vcpu(&self, old_vcpus: u32, new_vcpus: u32) -> Result<(u32, u32)>; // returns (old_vcpus, new_vcpus)
    async fn resize_memory(&self, new_mem_mb: u32) -> Result<(u32, MemoryConfig)>;

    // device manager
    async fn add_device(&self, device: DeviceType) -> Result<DeviceType>;
    async fn remove_device(&self, device: DeviceType) -> Result<()>;
    async fn update_device(&self, device: DeviceType) -> Result<()>;

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
    async fn get_hypervisor_metrics(&self) -> Result<String>;
    async fn set_capabilities(&self, flag: CapabilityBits);
    async fn set_guest_memory_block_size(&self, size: u32);
    async fn guest_memory_block_size(&self) -> u32;
    async fn get_passfd_listener_addr(&self) -> Result<(String, u32)>;
}
