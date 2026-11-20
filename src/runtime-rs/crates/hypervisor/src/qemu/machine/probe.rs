// Copyright (c) NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0

use std::ops::Range;

pub(crate) struct HostTopology {
    pub sockets: Vec<SocketInfo>,
    /// GPU devices: each group maps to one pxb-pcie + arm-smmuv3 complex.
    /// GPUs emit 8 acpi-generic-initiator NUMA nodes per device.
    pub gpu_smmu_groups: Vec<GpuSmmuGroup>,
    /// NIC devices: each group maps to its own pxb-pcie + arm-smmuv3.
    /// NICs do NOT emit acpi-generic-initiator links or NUMA initiator nodes.
    /// Allocated after all GPU pxb complexes in bus_nr ordering.
    pub nic_smmu_groups: Vec<GpuSmmuGroup>,
    pub egm_sockets: Vec<EgmSocketInfo>,
    /// `-numa dist` entries emitted after all NUMA nodes.  Each tuple is (src, dst, val).
    pub numa_distances: Vec<(u32, u32, u32)>,
    /// Minimum number of `pcie-root-port` slots to pre-provision on the Q35 default
    /// bus at VM creation time.  Mirrors the `pcie_root_port =` kata config field.
    /// Slots are empty at boot; devices are plugged in cold (before boot, by the
    /// legacy generator) or hot (via QMP `device_add` at runtime).
    /// See "VFIO Device Assignment Model" in ARCHITECTURE.md.
    pub pcie_root_port: u32,
    pub protection: Option<ProtectionDevice>,
}

pub(crate) struct SocketInfo {
    pub id: u32,
    pub cpu_range: Range<u32>,
    /// Host NUMA node to bind this socket's memory to via `policy=bind`.
    pub host_node: Option<u32>,
    /// File-backed memory path (e.g. `/dev/shm`).  `None` → `memory-backend-ram`.
    pub mem_path: Option<String>,
    /// Per-socket memory size in bytes.  `None` → use the Platform-level default.
    pub mem_size: Option<u64>,
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

/// CoCo hardware protection capability detected by the host probe.
///
/// Drives three platform decisions: the `-object <type>-guest` preamble, the
/// `kernel_irqchip=split` machine flag, and the `CpuModel` (EpycV4 for SNP,
/// Host for TDX).
#[derive(Clone)]
pub(crate) enum ProtectionDevice {
    SevSnp {
        id: String,
        cbitpos: u8,
        reduced_phys_bits: u8,
        kernel_hashes: bool,
        policy: u64,
        host_data: Option<String>,
    },
    Tdx {
        id: String,
        /// vsock address for the DCAP quote generation service.
        /// Absent on TDs that do not perform local attestation.
        quote_generation_socket: Option<TdxQuoteSocket>,
    },
}

/// vsock socket used by the TDX quote generation daemon (DCAP).
///
/// Emitted as a JSON sub-object in the `tdx-guest` `-object` argument because
/// QEMU's key=value parser cannot represent nested structures.
#[derive(Clone)]
pub(crate) struct TdxQuoteSocket {
    pub ty: String,   // "vsock"
    pub cid: String,  // guest CID, e.g. "2"
    pub port: String, // port number, e.g. "4050"
}

impl ProtectionDevice {
    pub(crate) fn id(&self) -> &str {
        match self {
            ProtectionDevice::SevSnp { id, .. } => id,
            ProtectionDevice::Tdx { id, .. } => id,
        }
    }
}
