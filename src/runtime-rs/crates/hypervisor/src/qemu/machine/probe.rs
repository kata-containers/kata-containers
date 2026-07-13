// Copyright (c) 2025 NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0

use std::ops::Range;

/// Host topology discovered from sysfs and IOMMU group layout.
/// Consumed by Platform::apply_host_defaults to build PciTopology and Objects.
pub(crate) struct HostTopology {
    pub sockets: Vec<SocketInfo>,
    pub gpu_smmu_groups: Vec<GpuSmmuGroup>,
    pub egm_sockets: Vec<EgmSocketInfo>,
}

pub(crate) struct SocketInfo {
    pub id: u32,
    pub cpu_range: Range<u32>,
}

/// GPUs sharing a physical SMMU must be placed on the same pxb-pcie + arm-smmuv3.
/// Grouping is derived from /sys/kernel/iommu_groups.
pub(crate) struct GpuSmmuGroup {
    pub pci_bus_addrs: Vec<String>,
    pub socket: u32,
}

/// One entry per /dev/egmN device created by the nvgrace-egm kernel module.
pub(crate) struct EgmSocketInfo {
    pub path: String,
    pub socket: u32,
    pub total_size: u64,
}
