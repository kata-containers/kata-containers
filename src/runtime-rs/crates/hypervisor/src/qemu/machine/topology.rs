// Copyright (c) NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0

pub(crate) struct PciTopology {
    pub default_bus: Option<String>,
    pub roots: Vec<PciRootComplex>,
}

pub(crate) struct PciRootComplex {
    pub id: String,
    pub bus_nr: u8,
    /// Omitting this triggers "Unknown NUMA node; performance will be reduced" in the guest kernel.
    pub numa_node: Option<u32>,
    /// Intel IOMMU is Q35-global, not bus-attached; see Q35::intel_iommu.
    pub iommu: Option<BusIommu>,
    pub root_ports: Vec<PciRootPort>,
}

pub(crate) struct PciRootPort {
    pub id: String,
    pub chassis: u8,
    pub device: Option<VfioDevice>,
}

pub(crate) struct VfioDevice {
    pub id: String,
    pub host: String,
    pub rombar: bool,
    pub kind: VfioDeviceKind,
}

pub(crate) enum VfioDeviceKind {
    Gpu,
    Nic,
}

/// Intel IOMMU is Q35-global and is not represented here; see Q35::intel_iommu.
pub(crate) enum BusIommu {
    SmmuV3(SmmuV3Config),
}

pub(crate) struct SmmuV3Config {
    pub accel: bool,
    pub ats: bool,
    pub pasid: bool,
    pub oas: u8,
    pub ril: bool,
    /// Requires physically contiguous guest memory (hugepages or EGM).
    pub cmdqv: bool,
}

impl Default for SmmuV3Config {
    fn default() -> Self {
        Self {
            accel: true,
            ats: true,
            pasid: true,
            oas: 48,
            ril: false,
            cmdqv: false,
        }
    }
}
