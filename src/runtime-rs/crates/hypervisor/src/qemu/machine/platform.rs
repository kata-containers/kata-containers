// Copyright (c) NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0

use anyhow::{bail, Result};
use kata_types::config::hypervisor::Hypervisor as HypervisorConfig;

use super::{
    probe::{HostTopology, SocketInfo},
    pseries::Pseries,
    q35::Q35,
    s390x::S390xCcwVirtio,
    topology::{BusIommu, PciRootComplex, PciRootPort, PciTopology, SmmuV3Config, VfioDevice, VfioDeviceKind},
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

pub(crate) struct BaseMachine {
    pub accel: String,
    /// ID of the primary memory backend, written as `-machine memory-backend=<id>`.
    /// `None` for topologies that supply memory per NUMA node via `-numa node,memdev=`
    /// rather than a single machine-wide backend (e.g. multi-socket vEGM).
    pub memory_backend: Option<String>,
    pub cpu: CpuConfig,
}

/// CPU model selection and additive feature flags.
///
/// The model choice is an attestation identity for CoCo guests, not merely a
/// performance knob.  See ARCHITECTURE.md §"CPU model and attestation identity"
/// for the full rationale and the issues that drove this split (#12210, #12329,
/// #12382).
pub(crate) struct CpuConfig {
    pub model: CpuModel,
}

pub(crate) enum CpuModel {
    /// `-cpu host[,<extra_features>]`
    ///
    /// Exposes the full host CPU to the guest.  Correct for vanilla KVM and
    /// TDX.  Not suitable for SNP: the guest CPUID family/model/stepping
    /// changes per physical node, so attestation reference values differ
    /// across a mixed Milan/Genoa/Turin fleet (#12329).
    Host { extra_features: Vec<String> },

    /// `-cpu EPYC-v4[,<extra_features>]`
    ///
    /// Pins the guest CPUID to a fixed AMD model, giving deterministic
    /// attestation across all AMD nodes in the fleet regardless of actual
    /// silicon generation.  `extra_features` re-enables the AVX-512 and
    /// VAES extensions that EPYC-v4 strips by default, recovering ~2x
    /// AES-GCM throughput without changing the attestation identity (#12382).
    EpycV4 { extra_features: Vec<String> },
}

/// AVX-512 and vectorised-AES extensions stripped by EPYC-v4 that should be
/// re-enabled for SNP guests to recover hardware crypto throughput.
///
/// Without these, AES-GCM throughput is ~4 GB/s; with them it returns to
/// ~8 GB/s, which matters when an H100 GPU is the bottleneck (#12382).
pub(crate) const SNP_CRYPTO_FEATURES: &[&str] = &[
    "+vaes",
    "+vpclmulqdq",
    "+avx512f",
    "+avx512dq",
    "+avx512bw",
    "+avx512vl",
    "+avx512cd",
    "+avx512ifma",
    "+avx512vbmi",
    "+avx512vbmi2",
    "+avx512vnni",
    "+avx512bitalg",
    "+avx512-vpopcntdq",
    "+avx512-bf16",
];

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

const PRIMARY_RAM_ID: &str = "ram0";

impl Platform {
    pub fn from_config(config: &HypervisorConfig) -> Result<Self> {
        let memory_size = u64::from(config.memory_info.default_memory) << 20;
        Self::build(&config.machine_info.machine_type, memory_size)
    }

    #[cfg(test)]
    pub(crate) fn from_config_defaults(memory_size: u64) -> Result<Self> {
        Self::build("virt", memory_size)
    }

    fn build(machine_type: &str, memory_size: u64) -> Result<Self> {
        let base = BaseMachine {
            accel: "kvm".to_owned(),
            memory_backend: Some(PRIMARY_RAM_ID.to_owned()),
            cpu: CpuConfig {
                model: CpuModel::Host { extra_features: vec![] },
            },
        };

        let machine = match machine_type {
            "q35" => Machine::Q35(Q35 {
                base,
                kernel_irqchip: Some("on".to_owned()),
                intel_iommu: None,
            }),
            "virt" => Machine::Virt(Virt {
                base,
                gic_version: None,
                ras: false,
                highmem_mmio_size: None,
            }),
            "pseries" => Machine::Pseries(Pseries { base }),
            "s390-ccw-virtio" => Machine::S390xCcwVirtio(S390xCcwVirtio { base }),
            other => bail!("unknown machine type: {other}"),
        };

        Ok(Self {
            machine,
            pci: PciTopology { default_bus: None, roots: vec![] },
            objects: Objects {
                iommufd: None,
                memory_backends: vec![MemoryBackend::Ram {
                    id: PRIMARY_RAM_ID.to_owned(),
                    size: memory_size,
                }],
                thread_contexts: vec![],
                acpi_links: vec![],
                rng: None,
            },
        })
    }

    pub fn apply_host_defaults(&mut self, topo: &HostTopology) {
        let has_gpus = topo.gpu_smmu_groups.iter().any(|g| !g.pci_bus_addrs.is_empty());
        if has_gpus {
            self.objects.iommufd = Some(IommufdBackend { id: "iommufd0".to_owned() });
        }

        for (idx, egm) in topo.egm_sockets.iter().enumerate() {
            self.objects.memory_backends.push(MemoryBackend::File {
                id: format!("egm{idx}"),
                size: egm.total_size,
                path: egm.path.clone(),
                prealloc: false,
                share: true,
            });
        }

        let n_cpu_nodes = topo.sockets.len();
        let mut gpu_idx = 0usize;

        for (group_idx, group) in topo.gpu_smmu_groups.iter().enumerate() {
            // 0x40 is the conventional start for pxb-pcie to avoid the primary bus range.
            let bus_nr = 0x40u8.saturating_add((group_idx as u8).saturating_mul(0x20));
            let cpu_mem_node = socket_numa_node(&topo.sockets, group.socket);
            let has_egm = topo.egm_sockets.iter().any(|e| e.socket == group.socket);

            let mut root_ports = Vec::new();
            for pci_addr in &group.pci_bus_addrs {
                let dev_id = format!("gpu{gpu_idx}");
                let port_id = format!("rp{gpu_idx}");

                for i in 0..8u32 {
                    let node = (n_cpu_nodes + gpu_idx * 8 + i as usize) as u32;
                    self.objects.acpi_links.push(AcpiPciNodeLink::GenericInitiator {
                        id: format!("{dev_id}_{i}"),
                        pci_dev: dev_id.clone(),
                        node,
                    });
                }

                if has_egm {
                    self.objects.acpi_links.push(AcpiPciNodeLink::EgmMemory {
                        id: format!("egm_{dev_id}"),
                        pci_dev: dev_id.clone(),
                        node: cpu_mem_node,
                    });
                }

                root_ports.push(PciRootPort {
                    id: port_id,
                    chassis: (gpu_idx + 1) as u8,
                    device: Some(VfioDevice {
                        id: dev_id,
                        host: pci_addr.clone(),
                        rombar: false,
                        kind: VfioDeviceKind::Gpu,
                    }),
                });

                gpu_idx += 1;
            }

            self.pci.roots.push(PciRootComplex {
                id: format!("pxb{group_idx}"),
                bus_nr,
                numa_node: Some(cpu_mem_node),
                iommu: if root_ports.is_empty() {
                    None
                } else {
                    Some(BusIommu::SmmuV3(SmmuV3Config::default()))
                },
                root_ports,
            });
        }
    }

    pub fn with_hugepages(self, path: &str) -> Self {
        let backends = self
            .objects
            .memory_backends
            .into_iter()
            .map(|b| match b {
                MemoryBackend::Ram { id, size } => MemoryBackend::File {
                    id,
                    size,
                    path: path.to_owned(),
                    prealloc: true,
                    share: false,
                },
                other => other,
            })
            .collect();

        Platform {
            objects: Objects { memory_backends: backends, ..self.objects },
            ..self
        }
    }

    pub fn to_qemu_args(&self) -> Result<Vec<String>> {
        todo!("Phase 2+")
    }
}

// Socket IDs are not guaranteed contiguous; use position to get a dense NUMA node number.
fn socket_numa_node(sockets: &[SocketInfo], socket_id: u32) -> u32 {
    sockets
        .iter()
        .position(|s| s.id == socket_id)
        .unwrap_or(socket_id as usize) as u32
}
