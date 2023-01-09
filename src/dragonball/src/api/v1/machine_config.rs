// Copyright (C) 2022 Alibaba Cloud. All rights reserved.
// Copyright 2018 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

/// We only support this number of vcpus for now. Mostly because we have set all vcpu related metrics as u8
/// and breaking u8 will take extra efforts.
pub const MAX_SUPPORTED_VCPUS: u8 = 254;

/// Memory hotplug value should have alignment in this size (unit: MiB)
pub const MEMORY_HOTPLUG_ALIGHMENT: u8 = 64;

/// Errors associated with configuring the microVM.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum VmConfigError {
    /// Cannot update the configuration of the microvm post boot.
    #[error("update operation is not allowed after boot")]
    UpdateNotAllowedPostBoot,

    /// The max vcpu count is invalid.
    #[error("the vCPU number shouldn't large than {}", MAX_SUPPORTED_VCPUS)]
    VcpuCountExceedsMaximum,

    /// The vcpu count is invalid. When hyperthreading is enabled, the `cpu_count` must be either
    /// 1 or an even number.
    #[error(
        "the vCPU number '{0}' can only be 1 or an even number when hyperthreading is enabled"
    )]
    InvalidVcpuCount(u8),

    /// The threads_per_core is invalid. It should be either 1 or 2.
    #[error("the threads_per_core number '{0}' can only be 1 or 2")]
    InvalidThreadsPerCore(u8),

    /// The cores_per_die is invalid. It should be larger than 0.
    #[error("the cores_per_die number '{0}' can only be larger than 0")]
    InvalidCoresPerDie(u8),

    /// The dies_per_socket is invalid. It should be larger than 0.
    #[error("the dies_per_socket number '{0}' can only be larger than 0")]
    InvalidDiesPerSocket(u8),

    /// The socket number is invalid. It should be either 1 or 2.
    #[error("the socket number '{0}' can only be 1 or 2")]
    InvalidSocket(u8),

    /// max vcpu count inferred from cpu topology(threads_per_core * cores_per_die * dies_per_socket * sockets) should be larger or equal to vcpu_count
    #[error("the max vcpu count inferred from cpu topology '{0}' (threads_per_core * cores_per_die * dies_per_socket * sockets) should be larger or equal to vcpu_count")]
    InvalidCpuTopology(u8),

    /// The max vcpu count is invalid.
    #[error(
        "the max vCPU number '{0}' shouldn't less than vCPU count and can only be 1 or an even number when hyperthreading is enabled"
    )]
    InvalidMaxVcpuCount(u8),

    /// The memory size is invalid. The memory can only be an unsigned integer.
    #[error("the memory size 0x{0:x}MiB is invalid")]
    InvalidMemorySize(usize),

    /// The hotplug memory size is invalid. The memory can only be an unsigned integer.
    #[error(
        "the hotplug memory size '{0}' (MiB) is invalid, must be multiple of {}",
        MEMORY_HOTPLUG_ALIGHMENT
    )]
    InvalidHotplugMemorySize(usize),

    /// The memory type is invalid.
    #[error("the memory type '{0}' is invalid")]
    InvalidMemType(String),

    /// The memory file path is invalid.
    #[error("the memory file path is invalid")]
    InvalidMemFilePath(String),

    /// NUMA region memory size is invalid
    #[error("Total size of memory in NUMA regions: {0}, should matches memory size in config")]
    InvalidNumaRegionMemorySize(usize),

    /// NUMA region vCPU count is invalid
    #[error("Total counts of vCPUs in NUMA regions: {0}, should matches max vcpu count in config")]
    InvalidNumaRegionCpuCount(u16),

    /// NUMA region vCPU count is invalid
    #[error("Max id of vCPUs in NUMA regions: {0}, should matches max vcpu count in config")]
    InvalidNumaRegionCpuMaxId(u16),
}
