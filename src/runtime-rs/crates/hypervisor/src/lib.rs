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
pub use kernel_param::Param;
mod utils;
use std::collections::HashMap;

use anyhow::Result;
use async_trait::async_trait;
use hypervisor_persist::HypervisorState;
use kata_types::config::hypervisor::Hypervisor as HypervisorConfig;

// Config which driver to use as vm root dev
const VM_ROOTFS_DRIVER_BLK: &str = "virtio-blk";
const VM_ROOTFS_DRIVER_PMEM: &str = "virtio-pmem";

pub const HYPERVISOR_DRAGONBALL: &str = "dragonball";
#[derive(PartialEq)]
pub(crate) enum VmmState {
    NotReady,
    VmmServerReady,
    VmRunning,
}

// vcpu mapping from vcpu number to thread number
#[derive(Debug)]
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
    async fn cleanup(&self) -> Result<()>;
    async fn check(&self) -> Result<()>;
    async fn get_jailer_root(&self) -> Result<String>;
    async fn save_state(&self) -> Result<HypervisorState>;
}
