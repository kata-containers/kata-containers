// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

//! CpuMemResource is a simple wrapper of a subset of cpuinfo, meminfo and potentially
//! some other fields related to resource management. CpuMemResource is added since many
//! components need to query the current cpu/mem state without TomlConfig. This can now be
//! accessed through ResourceManager

use std::sync::Arc;

use kata_types::config::TomlConfig;

use anyhow::{Context, Result};

/// Cpu information wrapper
#[derive(Default, Debug, Clone, Copy)]
pub struct CpuResource {
    /// The default vcpu # when boot
    default_vcpu: i32,

    /// Current number of vCPUs
    /// when boot, the number is set to *default_vcpus*
    current_vcpu: i32,
    // TODO(shuoyu.ys): cpuset mapping relation
}

impl CpuResource {
    pub fn new(config: Arc<TomlConfig>) -> Result<Self> {
        let hypervisor_name = config.runtime.hypervisor_name.clone();
        let hypervisor_config = config
            .hypervisor
            .get(&hypervisor_name)
            .context("failed to get hypervisor")?;
        Ok(Self {
            default_vcpu: hypervisor_config.cpu_info.default_vcpus,
            current_vcpu: hypervisor_config.cpu_info.default_vcpus,
        })
    }
}

/// Memory information wrapper
#[derive(Default, Debug, Clone, Copy)]
pub struct MemResource {
    /// The default vcpu # when boot
    default_mem_mb: u32,

    /// Current memory size, in MiB
    /// when boot, the number is set to *default_memory*
    current_mem_mb: u32,

    /// Whether enable swap in guest
    enable_guest_swap: bool,
}

impl MemResource {
    pub fn new(config: Arc<TomlConfig>) -> Result<Self> {
        let hypervisor_name = config.runtime.hypervisor_name.clone();
        let hypervisor_config = config
            .hypervisor
            .get(&hypervisor_name)
            .context("failed to get hypervisor")?;
        Ok(Self {
            default_mem_mb: hypervisor_config.memory_info.default_memory,
            current_mem_mb: hypervisor_config.memory_info.default_memory,
            enable_guest_swap: hypervisor_config.memory_info.enable_guest_swap,
        })
    }
}

/// Generic sandbox Cpu and Memory information wrapper, used as a query intermediate
#[derive(Default, Debug, Clone, Copy)]
pub struct CpuMemResource {
    cpu_resource: CpuResource,
    mem_resource: MemResource,
}

impl CpuMemResource {
    pub fn new(config: Arc<TomlConfig>) -> Result<Self> {
        Ok(Self {
            cpu_resource: CpuResource::new(config.clone())?,
            mem_resource: MemResource::new(config)?,
        })
    }

    pub fn default_vcpu(&self) -> Result<i32> {
        Ok(self.cpu_resource.default_vcpu)
    }

    pub fn current_vcpu(&self) -> Result<i32> {
        Ok(self.cpu_resource.current_vcpu)
    }

    pub fn default_mem_mb(&self) -> Result<u32> {
        Ok(self.mem_resource.default_mem_mb)
    }

    pub fn current_mem_mb(&self) -> Result<u32> {
        Ok(self.mem_resource.current_mem_mb)
    }

    pub fn update_current_vcpu(&mut self, vcpu: i32) -> Result<()> {
        self.cpu_resource.current_vcpu = vcpu;
        Ok(())
    }

    pub fn update_current_mem_mb(&mut self, mem: u32) -> Result<()> {
        self.mem_resource.current_mem_mb = mem;
        Ok(())
    }

    pub fn use_guest_swap(&self) -> Result<bool> {
        Ok(self.mem_resource.enable_guest_swap)
    }
}
