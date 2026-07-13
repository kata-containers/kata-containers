// Copyright (c) NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0

use anyhow::Result;
use kata_types::config::hypervisor::Hypervisor as HypervisorConfig;

use super::{
    probe::HostTopology, pseries::Pseries, q35::Q35, s390x::S390xCcwVirtio, topology::PciTopology,
    virt::Virt,
};

pub(crate) struct Platform {
    pub machine: Machine,
    pub pci: PciTopology,
    pub objects: Objects,
}

pub(crate) enum Machine {
    Q35(Q35),
    Virt(Virt),
    Pseries(Pseries),
    S390xCcwVirtio(S390xCcwVirtio),
}

/// Fields common to all machine types.
pub(crate) struct BaseMachine {
    pub accel: String,
    /// ID of the primary memory backend, written as `-machine memory-backend=<id>`.
    /// `None` for topologies that supply memory per NUMA node via `-numa node,memdev=`
    /// rather than a single machine-wide backend (e.g. multi-socket vEGM).
    pub memory_backend: Option<String>,
}

pub(crate) struct Objects {
    pub iommufd: Option<IommufdBackend>,
    pub memory_backends: Vec<MemoryBackend>,
    pub thread_contexts: Vec<ThreadContext>,
    pub acpi_links: Vec<AcpiPciNodeLink>,
    pub rng: Option<ObjectRngRandom>,
}

pub(crate) struct IommufdBackend {
    pub id: String,
}

pub(crate) enum MemoryBackend {
    Ram {
        id: String,
        size: u64,
    },
    /// File-backed memory.
    ///   path = "/dev/hugepages/" -- hugepages-backed guest RAM (vCMDQ)
    ///   path = "/dev/egmN"      -- per-socket EGM region (vEGM)
    File {
        id: String,
        size: u64,
        path: String,
        prealloc: bool,
        share: bool,
    },
}

pub(crate) enum AcpiPciNodeLink {
    /// Emitted 8x per GPU; the GPU driver uses these to online GPU memory.
    GenericInitiator {
        id: String,
        pci_dev: String,
        node: u32,
    },
    /// Emitted 1x per GPU; links the GPU to the per-socket EGM backend.
    /// `node` is the CpuMem NUMA node of the socket, not a GPU initiator node.
    EgmMemory {
        id: String,
        pci_dev: String,
        node: u32,
    },
}

pub(crate) struct ThreadContext {
    pub id: String,
}

pub(crate) struct ObjectRngRandom {
    pub id: String,
    pub filename: String,
}

impl Platform {
    pub fn from_config(_config: &HypervisorConfig) -> Result<Self> {
        todo!("Phase 1")
    }

    /// Test-only constructor; produces a minimal Virt platform with RAM-backed
    /// memory. Replaced in Phase 1 by from_config once the builder exists.
    #[cfg(test)]
    pub(crate) fn from_config_defaults(_memory_size: u64) -> Result<Self> {
        todo!("Phase 1")
    }

    pub fn apply_host_defaults(&mut self, _topo: &HostTopology) {
        todo!("Phase 4")
    }

    pub fn with_hugepages(self, _path: &str) -> Self {
        todo!("Phase 3")
    }

    /// Emit the complete QEMU command-line argument list.
    ///
    /// Emission order:
    ///   1. iommufd object
    ///   2. remaining objects (memory backends, thread contexts, rng)
    ///   3. -machine (picks up memory-backend=)
    ///   4. CpuMem -numa node entries (cpus= + memdev=)
    ///   5. GPU initiator -numa node entries (8 per GPU, no cpus/memdev)
    ///   6. EGM / hotplug -numa node entries (memory-only)
    ///   7. PciTopology (pxb-pcie, arm-smmuv3, root ports, vfio devices)
    ///   8. acpi_links (acpi-generic-initiator x8 per GPU, acpi-egm-memory x1 per GPU)
    pub fn to_qemu_args(&self) -> Result<Vec<String>> {
        todo!("Phase 2+")
    }
}
