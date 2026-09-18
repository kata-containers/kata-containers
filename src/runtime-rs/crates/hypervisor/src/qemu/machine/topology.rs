// Copyright (c) NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0

pub(crate) struct PciTopology {
    pub default_bus: Option<String>,
    pub roots: Vec<PciRootComplex>,
    /// Pre-provisioned root ports emitted on the default bus at VM creation time.
    /// Empty at boot; filled cold (static cmdline) or hot (QMP `device_add`).
    /// Driven by `HostTopology::pcie_root_port`.  Contrast with `roots`, which
    /// are pxb-pcie complexes for static GPU passthrough (cold-plug root-port).
    pub pcie_root_port: Vec<PciRootPort>,
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
    /// `slot=N` — required for Q35 root ports; absent on aarch64 Grace ports.
    pub slot: Option<u8>,
    /// `multifunction=on/off` — required for Q35; absent on Grace.
    pub multifunction: Option<bool>,
    /// `io-reserve=N` — required for aarch64 Grace ports; absent on Q35.
    pub io_reserve: Option<u32>,
    pub device: Option<VfioDevice>,
}

pub(crate) struct VfioDevice {
    pub id: String,
    pub host: String,
    pub rombar: Option<bool>,
    pub kind: VfioDeviceKind,
    /// When set, an `-object iommufd,id=<iommufd_id>` is emitted immediately before
    /// this device and referenced in the device string.  Used for CoCo x86 passthrough;
    /// Grace uses a single shared `iommufd0` in `Objects::iommufd` instead.
    pub iommufd_id: Option<String>,
    /// `x-pci-vendor-id` override required for CoCo measured-boot attestation (#12329).
    pub pci_vendor_id: Option<u16>,
    /// `x-pci-device-id` override required for CoCo measured-boot attestation (#12329).
    pub pci_device_id: Option<u16>,
}

pub(crate) enum VfioDeviceKind {
    /// `vfio-pci-nohotplug` — aarch64 Grace static binding.
    Gpu,
    /// `vfio-pci` — x86 Q35 / CoCo GPU passthrough.
    GpuPci,
    /// `vfio-pci` — NVSwitch (DGX/HGX fabric chip).
    ///
    /// Uses the same device string as GpuPci; distinguished here so probers
    /// and emitters can identify device type without re-reading PCI IDs.
    NvSwitch,
    Nic,
}

/// Intel IOMMU is Q35-global and is not represented here; see Q35::intel_iommu.
pub(crate) enum BusIommu {
    SmmuV3(SmmuV3Config),
}

pub(crate) struct SmmuV3Config {
    pub id: String,
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
            id: String::new(),
            accel: true,
            ats: true,
            pasid: true,
            oas: 48,
            ril: false,
            cmdqv: false,
        }
    }
}
