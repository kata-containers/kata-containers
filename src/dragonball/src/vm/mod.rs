// Copyright (C) 2021 Alibaba Cloud. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use serde_derive::{Deserialize, Serialize};

mod kernel_config;
pub use self::kernel_config::KernelConfigInfo;

/// Configuration information for user defined NUMA nodes.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct NumaRegionInfo {
    /// memory size for this region (unit: MiB)
    pub size: u64,
    /// numa node id on host for this region
    pub host_numa_node_id: Option<u32>,
    /// numa node id on guest for this region
    pub guest_numa_node_id: Option<u32>,
    /// vcpu ids belonging to this region
    pub vcpu_ids: Vec<u32>,
}

/// Information for cpu topology to guide guest init
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CpuTopology {
    /// threads per core to indicate hyperthreading is enabled or not
    pub threads_per_core: u8,
    /// cores per die to guide guest cpu topology init
    pub cores_per_die: u8,
    /// dies per socket to guide guest cpu topology
    pub dies_per_socket: u8,
    /// number of sockets
    pub sockets: u8,
}

impl Default for CpuTopology {
    fn default() -> Self {
        CpuTopology {
            threads_per_core: 1,
            cores_per_die: 1,
            dies_per_socket: 1,
            sockets: 1,
        }
    }
}

/// Configuration information for virtual machine instance.
#[derive(Clone, Debug, PartialEq)]
pub struct VmConfigInfo {
    /// Number of vcpu to start.
    pub vcpu_count: u8,
    /// Max number of vcpu can be added
    pub max_vcpu_count: u8,
    /// Enable or disable hyperthreading.
    pub ht_enabled: bool,
    /// cpu power management.
    pub cpu_pm: String,
    /// cpu topology information
    pub cpu_topology: CpuTopology,
    /// vpmu support level
    pub vpmu_feature: u8,

    /// Memory type that can be either hugetlbfs or shmem, default is shmem
    pub mem_type: String,
    /// Memory file path
    pub mem_file_path: String,
    /// The memory size in MiB.
    pub mem_size_mib: usize,
    /// reserve memory bytes
    pub reserve_memory_bytes: u64,

    /// sock path
    pub serial_path: Option<String>,
}

impl Default for VmConfigInfo {
    fn default() -> Self {
        VmConfigInfo {
            vcpu_count: 1,
            max_vcpu_count: 1,
            ht_enabled: false,
            cpu_pm: String::from("on"),
            cpu_topology: CpuTopology {
                threads_per_core: 1,
                cores_per_die: 1,
                dies_per_socket: 1,
                sockets: 1,
            },
            vpmu_feature: 0,
            mem_type: String::from("shmem"),
            mem_file_path: String::from(""),
            mem_size_mib: 128,
            reserve_memory_bytes: 0,
            serial_path: None,
        }
    }
}
