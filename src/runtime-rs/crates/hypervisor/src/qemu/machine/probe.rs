// Copyright (c) NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0

use std::ops::Range;

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
pub(crate) struct GpuSmmuGroup {
    pub pci_bus_addrs: Vec<String>,
    pub socket: u32,
}

pub(crate) struct EgmSocketInfo {
    pub path: String,
    pub socket: u32,
    pub total_size: u64,
}
