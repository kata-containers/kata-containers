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
    /// Written as pxb-pcie `numa_node=N`. Required on Grace; omitting it
    /// triggers "Unknown NUMA node; performance will be reduced" in the guest.
    pub numa_node: Option<u32>,
    /// Bus-attached IOMMU. Intel IOMMU is a Q35-global device; see Q35::intel_iommu.
    pub iommu: Option<BusIommu>,
    /// One entry per passthrough device sharing this SMMU.
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
    /// Emits 8 acpi-generic-initiator objects after the device.
    Gpu,
    /// No acpi-generic-initiator objects.
    Nic,
}

/// IOMMU that attaches to a specific PCIe expander bus (pxb-pcie).
/// Intel IOMMU is a Q35-global device and is not represented here.
pub(crate) enum BusIommu {
    SmmuV3(SmmuV3Config),
}

pub(crate) struct SmmuV3Config {
    pub accel: bool,
    pub ats: bool,
    pub pasid: bool,
    pub oas: u8,
    pub ril: bool,
    /// Enable SMMU command-queue virtualisation. Requires physically
    /// contiguous guest memory (hugepages or EGM).
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
