// Copyright (c) NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0

use std::ops::Range;

use anyhow::{bail, Result};
use kata_types::config::hypervisor::Hypervisor as HypervisorConfig;

use super::{
    probe::{HostTopology, ProtectionDevice, SocketInfo},
    pseries::Pseries,
    q35::Q35,
    s390x::S390xCcwVirtio,
    topology::{
        BusIommu, PciRootComplex, PciRootPort, PciTopology, SmmuV3Config, VfioDevice,
        VfioDeviceKind,
    },
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

impl Machine {
    fn base(&self) -> &BaseMachine {
        match self {
            Machine::Q35(m) => &m.base,
            Machine::Virt(m) => &m.base,
            Machine::Pseries(m) => &m.base,
            Machine::S390xCcwVirtio(m) => &m.base,
        }
    }

    fn base_mut(&mut self) -> &mut BaseMachine {
        match self {
            Machine::Q35(m) => &mut m.base,
            Machine::Virt(m) => &mut m.base,
            Machine::Pseries(m) => &mut m.base,
            Machine::S390xCcwVirtio(m) => &mut m.base,
        }
    }
}

pub(crate) struct BaseMachine {
    pub accel: String,
    /// ID of the primary memory backend, written as `-machine memory-backend=<id>`.
    /// `None` for topologies that supply memory per NUMA node via `-numa node,memdev=`
    /// rather than a single machine-wide backend (e.g. multi-socket vEGM).
    /// Q35 never uses this field; virt uses it for the single-backend case.
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
    /// Shared iommufd object used by all Grace GPU devices.
    /// For CoCo x86, per-device iommufd is stored on `VfioDevice::iommufd_id` instead.
    pub iommufd: Option<IommufdBackend>,
    pub memory_backends: Vec<MemoryBackend>,
    pub numa_nodes: Vec<NumaNode>,
    /// `-numa dist` entries; emitted after all NUMA nodes.
    pub numa_distances: Vec<(u32, u32, u32)>,
    pub thread_contexts: Vec<ThreadContext>,
    pub acpi_links: Vec<AcpiPciNodeLink>,
    pub rng: Option<ObjectRngRandom>,
    /// CoCo protection object (`sev-snp-guest` or `tdx-guest`); emitted before `-machine`.
    pub protection: Option<ProtectionDevice>,
}

pub(crate) struct IommufdBackend {
    pub id: String,
}

pub(crate) enum MemoryBackend {
    Ram {
        id: String,
        size: u64,
        /// `host-nodes=N` — NUMA pinning for Q35 CoCo RAM-backed memory.
        host_nodes: Option<u32>,
        /// `policy=bind` — always paired with `host_nodes`.
        policy: Option<String>,
    },
    /// File-backed memory.
    ///   path = "/dev/shm"        -- NUMA-pinned SHM for vanilla Q35 (host_nodes set)
    ///   path = "/dev/hugepages/" -- hugepages-backed guest RAM (vCMDQ)
    ///   path = "/dev/egmN"      -- per-socket EGM region (vEGM)
    File {
        id: String,
        size: u64,
        path: String,
        prealloc: bool,
        share: bool,
        /// `host-nodes=N,policy=bind` — set for Q35 SHM; absent for hugepages/EGM.
        host_nodes: Option<u32>,
        policy: Option<String>,
        is_egm: bool,
    },
}

pub(crate) struct NumaNode {
    pub nodeid: u32,
    pub memdev: Option<String>,
    pub cpus: Option<Range<u32>>,
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

const PRIMARY_RAM_ID: &str = "m0";

impl Platform {
    pub fn from_config(config: &HypervisorConfig) -> Result<Self> {
        let memory_size = u64::from(config.memory_info.default_memory) << 20;
        Self::build(&config.machine_info.machine_type, memory_size)
    }

    #[cfg(test)]
    pub(crate) fn from_config_defaults(machine_type: &str, memory_size: u64) -> Result<Self> {
        Self::build(machine_type, memory_size)
    }

    fn build(machine_type: &str, memory_size: u64) -> Result<Self> {
        let virt_base = BaseMachine {
            accel: "kvm".to_owned(),
            memory_backend: Some(PRIMARY_RAM_ID.to_owned()),
            cpu: CpuConfig { model: CpuModel::Host { extra_features: vec![] } },
        };

        let machine = match machine_type {
            "q35" => {
                // Q35 machine line does not carry memory-backend=; memory is
                // expressed via Objects::memory_backends and -numa node,memdev=.
                let base = BaseMachine {
                    accel: "kvm".to_owned(),
                    memory_backend: None,
                    cpu: CpuConfig { model: CpuModel::Host { extra_features: vec![] } },
                };
                Machine::Q35(Q35 {
                    base,
                    kernel_irqchip: None,
                    confidential_guest_support: None,
                    intel_iommu: None,
                })
            }
            "virt" => Machine::Virt(Virt {
                base: virt_base,
                gic_version: Some(3),
                ras: true,
                highmem_mmio_size: Some(4 << 40),
            }),
            "pseries" => Machine::Pseries(Pseries { base: virt_base }),
            "s390-ccw-virtio" => Machine::S390xCcwVirtio(S390xCcwVirtio { base: virt_base }),
            other => bail!("unknown machine type: {other}"),
        };

        Ok(Self {
            machine,
            pci: PciTopology {
                default_bus: Some("pcie.0".to_owned()),
                roots: vec![],
                pcie_root_port: vec![],
            },
            objects: Objects {
                iommufd: None,
                memory_backends: vec![MemoryBackend::Ram {
                    id: PRIMARY_RAM_ID.to_owned(),
                    size: memory_size,
                    host_nodes: None,
                    policy: None,
                }],
                numa_nodes: vec![],
                numa_distances: vec![],
                thread_contexts: vec![],
                acpi_links: vec![],
                rng: None,
                protection: None,
            },
        })
    }

    pub(crate) fn apply_host_defaults(&mut self, topo: &HostTopology) {
        // Protection device drives CoCo machine flags and the preamble object.
        if let Some(ref prot) = topo.protection {
            if let Machine::Q35(ref mut q) = self.machine {
                q.kernel_irqchip = Some("split".to_owned());
                q.confidential_guest_support = Some(prot.id().to_owned());
            }
            self.objects.protection = Some(prot.clone());
        }

        match &self.machine {
            Machine::Q35(_) => self.apply_q35_defaults(topo),
            Machine::Virt(_) => self.apply_virt_defaults(topo),
            _ => {}
        }
    }

    fn apply_q35_defaults(&mut self, topo: &HostTopology) {
        self.objects.memory_backends.clear();
        self.objects.numa_nodes.clear();

        for (i, socket) in topo.sockets.iter().enumerate() {
            let id = format!("numa-mem{i}");
            let size = socket.mem_size.unwrap_or(0);

            let backend = if let Some(ref path) = socket.mem_path {
                MemoryBackend::File {
                    id: id.clone(),
                    size,
                    path: path.clone(),
                    prealloc: false,
                    share: true,
                    host_nodes: socket.host_node,
                    policy: socket.host_node.map(|_| "bind".to_owned()),
                    is_egm: false,
                }
            } else {
                MemoryBackend::Ram {
                    id: id.clone(),
                    size,
                    host_nodes: socket.host_node,
                    policy: socket.host_node.map(|_| "bind".to_owned()),
                }
            };

            self.objects.memory_backends.push(backend);
            self.objects.numa_nodes.push(NumaNode {
                nodeid: i as u32,
                memdev: Some(id),
                cpus: Some(socket.cpu_range.clone()),
            });
        }

        self.objects.numa_distances = topo.numa_distances.clone();

        // Pre-provisioned cold-plug root ports on pcie.0.
        self.pci.pcie_root_port = (0..topo.pcie_root_port)
            .map(|i| PciRootPort {
                id: format!("rp{i}"),
                chassis: 0,
                slot: Some(i as u8),
                multifunction: Some(false),
                io_reserve: None,
                device: None,
            })
            .collect();

        let has_gpus = topo.gpu_smmu_groups.iter().any(|g| !g.pci_bus_addrs.is_empty());
        if !has_gpus {
            return;
        }

        let mut gpu_idx = 0usize;

        for (group_idx, group) in topo.gpu_smmu_groups.iter().enumerate() {
            // 32-bus spacing between pxb complexes: each pxb may have up to 31
            // subordinate buses (one per root port + potential downstream buses).
            // Production captures show bus_nr=32 for pxb-numa0, 64 for pxb-numa1.
            let bus_nr = 32u8 + (group_idx as u8) * 32;

            let cpu_mem_node = socket_numa_node(&topo.sockets, group.socket);
            let pxb_id = format!("pxb-numa{group_idx}");

            let mut root_ports = Vec::new();
            for (port_idx, pci_addr) in group.pci_bus_addrs.iter().enumerate() {
                // slot and chassis are per-pxb-relative; chassis increments per complex.
                let rp_id = format!("rp-numa{group_idx}-{port_idx}");
                root_ports.push(PciRootPort {
                    id: rp_id,
                    chassis: 10 + group_idx as u8,
                    slot: Some(port_idx as u8),
                    multifunction: Some(false),
                    io_reserve: None,
                    device: Some(VfioDevice {
                        id: format!("dev{gpu_idx}"),
                        host: pci_addr.clone(),
                        rombar: None,
                        kind: VfioDeviceKind::GpuPci,
                        iommufd_id: None,
                        pci_vendor_id: None,
                        pci_device_id: None,
                    }),
                });
                gpu_idx += 1;
            }

            self.pci.roots.push(PciRootComplex {
                id: pxb_id,
                bus_nr,
                numa_node: Some(cpu_mem_node),
                iommu: None,
                root_ports,
            });
        }
    }

    fn apply_virt_defaults(&mut self, topo: &HostTopology) {
        let has_gpus = topo.gpu_smmu_groups.iter().any(|g| !g.pci_bus_addrs.is_empty());
        if !has_gpus {
            return;
        }

        self.objects.iommufd = Some(IommufdBackend { id: "iommufd0".to_owned() });

        let has_egm = !topo.egm_sockets.is_empty();

        if has_egm {
            self.machine.base_mut().memory_backend = None;
            for (idx, egm) in topo.egm_sockets.iter().enumerate() {
                self.objects.memory_backends.push(MemoryBackend::File {
                    id: format!("egm{idx}"),
                    size: egm.total_size,
                    path: egm.path.clone(),
                    prealloc: true,
                    share: true,
                    host_nodes: None,
                    policy: None,
                    is_egm: true,
                });
            }
        }

        let total_gpus: usize = topo.gpu_smmu_groups.iter().map(|g| g.pci_bus_addrs.len()).sum();
        let n_cpu_nodes = topo.sockets.len() as u32;

        for (socket_idx, socket) in topo.sockets.iter().enumerate() {
            let memdev = if has_egm {
                topo.egm_sockets
                    .iter()
                    .position(|e| e.socket == socket.id)
                    .map(|egm_pos| format!("m{egm_pos}"))
            } else if socket_idx == 0 {
                Some(PRIMARY_RAM_ID.to_owned())
            } else {
                None
            };
            self.objects.numa_nodes.push(NumaNode {
                nodeid: socket_idx as u32,
                memdev,
                cpus: Some(socket.cpu_range.clone()),
            });
        }

        for i in 0..(total_gpus as u32 * 8) {
            self.objects.numa_nodes.push(NumaNode {
                nodeid: n_cpu_nodes + i,
                memdev: None,
                cpus: None,
            });
        }

        let mut gpu_idx = 0usize;
        let mut port_idx = 1usize;
        let mut bus_nr_running: u8 = 0;

        for (group_idx, group) in topo.gpu_smmu_groups.iter().enumerate() {
            let n_ports = group.pci_bus_addrs.len();
            let bus_nr = 1u8 + bus_nr_running;
            bus_nr_running += if n_ports <= 1 { 1 } else { n_ports as u8 * 4 };

            let cpu_mem_node = socket_numa_node(&topo.sockets, group.socket);
            let group_has_egm = topo.egm_sockets.iter().any(|e| e.socket == group.socket);
            let pxb_id = format!("pcie.{}", group_idx + 1);

            let mut root_ports = Vec::new();
            for pci_addr in &group.pci_bus_addrs {
                let dev_id = format!("dev{gpu_idx}");
                let rp_id = format!("pcie.port{port_idx}");

                for i in 0..8u32 {
                    self.objects.acpi_links.push(AcpiPciNodeLink::GenericInitiator {
                        id: format!("gi{}", gpu_idx * 8 + i as usize),
                        pci_dev: dev_id.clone(),
                        node: n_cpu_nodes + (gpu_idx as u32) * 8 + i,
                    });
                }

                if group_has_egm {
                    self.objects.acpi_links.push(AcpiPciNodeLink::EgmMemory {
                        id: format!("egm{gpu_idx}"),
                        pci_dev: dev_id.clone(),
                        node: cpu_mem_node,
                    });
                }

                root_ports.push(PciRootPort {
                    id: rp_id,
                    chassis: (gpu_idx + 1) as u8,
                    slot: None,
                    multifunction: None,
                    io_reserve: Some(0),
                    device: Some(VfioDevice {
                        id: dev_id,
                        host: pci_addr.clone(),
                        rombar: Some(false),
                        kind: VfioDeviceKind::Gpu,
                        iommufd_id: None,
                        pci_vendor_id: None,
                        pci_device_id: None,
                    }),
                });

                gpu_idx += 1;
                port_idx += 1;
            }

            self.pci.roots.push(PciRootComplex {
                id: pxb_id,
                bus_nr,
                numa_node: Some(cpu_mem_node),
                iommu: if root_ports.is_empty() {
                    None
                } else {
                    Some(BusIommu::SmmuV3(SmmuV3Config {
                        id: format!("smmuv3.{}", group_idx + 1),
                        ..SmmuV3Config::default()
                    }))
                },
                root_ports,
            });
        }
    }

    pub(crate) fn with_hugepages(self, path: &str) -> Self {
        let backends = self
            .objects
            .memory_backends
            .into_iter()
            .map(|b| match b {
                MemoryBackend::Ram { id, size, .. } => MemoryBackend::File {
                    id,
                    size,
                    path: path.to_owned(),
                    prealloc: true,
                    share: true,
                    host_nodes: None,
                    policy: None,
                    is_egm: false,
                },
                other => other,
            })
            .collect();

        let roots = self
            .pci
            .roots
            .into_iter()
            .map(|mut root| {
                if let Some(BusIommu::SmmuV3(ref mut smmu)) = root.iommu {
                    smmu.cmdqv = true;
                }
                root
            })
            .collect();

        Platform {
            pci: PciTopology { roots, ..self.pci },
            objects: Objects { memory_backends: backends, ..self.objects },
            ..self
        }
    }

    /// Emit the complete QEMU command-line argument list.
    ///
    /// Dispatch by machine type: Q35 and virt/Grace have different emission
    /// ordering because virt requires `memory-backend=` on the machine line
    /// (backends must precede machine), whereas Q35 does not.
    pub(crate) fn to_qemu_args(&self) -> Result<Vec<String>> {
        match &self.machine {
            Machine::Q35(_) => self.emit_q35_args(),
            Machine::Virt(_) => self.emit_virt_args(),
            Machine::Pseries(_) => todo!("pSeries args"),
            Machine::S390xCcwVirtio(_) => todo!("s390x args"),
        }
    }

    /// Q35 emission order:
    ///   1. protection object (sev-snp-guest / tdx-guest), if any
    ///   2. -machine q35,...
    ///   3. memory backends + NUMA nodes, interleaved per socket
    ///   4. -numa dist entries
    ///   5. pxb-pcie GPU roots (pxb + root ports + per-device iommufd + vfio)
    ///   6. cold-plug root ports on pcie.0
    fn emit_q35_args(&self) -> Result<Vec<String>> {
        let mut args: Vec<String> = Vec::new();

        if let Some(ref prot) = self.objects.protection {
            args.push("-object".to_owned());
            args.push(emit_protection(prot));
        }

        args.push("-machine".to_owned());
        args.push(emit_machine(&self.machine));

        for (backend, node) in
            self.objects.memory_backends.iter().zip(self.objects.numa_nodes.iter())
        {
            args.push("-object".to_owned());
            args.push(emit_backend(backend, backend_id(backend)));
            args.push("-numa".to_owned());
            args.push(emit_numa_node(node));
        }

        for &(src, dst, val) in &self.objects.numa_distances {
            args.push("-numa".to_owned());
            args.push(format!("dist,src={src},dst={dst},val={val}"));
        }

        let default_bus = self.pci.default_bus.as_deref().unwrap_or("pcie.0");
        for root in &self.pci.roots {
            args.push("-device".to_owned());
            args.push(emit_pxb(root, default_bus));

            for port in &root.root_ports {
                args.push("-device".to_owned());
                args.push(emit_root_port(port, &root.id));

                if let Some(vfio) = &port.device {
                    if let Some(ref ifd_id) = vfio.iommufd_id {
                        args.push("-object".to_owned());
                        args.push(format!("iommufd,id={ifd_id}"));
                    }
                    args.push("-device".to_owned());
                    args.push(emit_vfio_q35(vfio, &port.id));
                }
            }
        }

        for port in &self.pci.pcie_root_port {
            args.push("-device".to_owned());
            args.push(emit_root_port(port, default_bus));
        }

        Ok(args)
    }

    /// virt / Grace emission order (unchanged from Phase 2):
    ///   1. iommufd object
    ///   2. memory backends
    ///   3. -machine (references memory-backend=)
    ///   4. NUMA nodes
    ///   5. pxb-pcie + arm-smmuv3 + root ports + vfio
    ///   6. acpi_links (GenericInitiators then EgmMemory)
    fn emit_virt_args(&self) -> Result<Vec<String>> {
        let mut args: Vec<String> = Vec::new();

        if let Some(ifd) = &self.objects.iommufd {
            args.push("-object".to_owned());
            args.push(format!("iommufd,id={}", ifd.id));
        }

        let has_egm = self.machine.base().memory_backend.is_none()
            && self.objects.memory_backends.len() > 1;

        if has_egm {
            for (idx, backend) in self.objects.memory_backends[1..].iter().enumerate() {
                args.push("-object".to_owned());
                args.push(emit_backend(backend, &format!("m{idx}")));
            }
        } else {
            for backend in &self.objects.memory_backends {
                args.push("-object".to_owned());
                args.push(emit_backend(backend, backend_id(backend)));
            }
        }

        args.push("-machine".to_owned());
        args.push(emit_machine(&self.machine));

        for node in &self.objects.numa_nodes {
            args.push("-numa".to_owned());
            args.push(emit_numa_node(node));
        }

        let default_bus = self.pci.default_bus.as_deref().unwrap_or("pcie.0");
        for root in &self.pci.roots {
            args.push("-device".to_owned());
            args.push(emit_pxb(root, default_bus));

            if let Some(BusIommu::SmmuV3(smmu)) = &root.iommu {
                args.push("-device".to_owned());
                args.push(emit_smmu(smmu, &root.id));
            }

            for port in &root.root_ports {
                args.push("-device".to_owned());
                args.push(emit_root_port(port, &root.id));

                if let Some(vfio) = &port.device {
                    args.push("-device".to_owned());
                    args.push(emit_vfio_grace(vfio, &port.id, self.objects.iommufd.as_ref()));
                }
            }
        }

        for link in self.objects.acpi_links.iter() {
            if let AcpiPciNodeLink::GenericInitiator { id, pci_dev, node } = link {
                args.push("-object".to_owned());
                args.push(format!("acpi-generic-initiator,id={id},pci-dev={pci_dev},node={node}"));
            }
        }
        for link in self.objects.acpi_links.iter() {
            if let AcpiPciNodeLink::EgmMemory { id, pci_dev, node } = link {
                args.push("-object".to_owned());
                args.push(format!("acpi-egm-memory,id={id},pci-dev={pci_dev},node={node}"));
            }
        }

        Ok(args)
    }
}

fn format_memory(size: u64) -> String {
    if size % (1 << 40) == 0 {
        format!("{}T", size >> 40)
    } else if size % (1 << 30) == 0 {
        format!("{}G", size >> 30)
    } else {
        format!("{}M", size >> 20)
    }
}

fn backend_id(backend: &MemoryBackend) -> &str {
    match backend {
        MemoryBackend::Ram { id, .. } => id,
        MemoryBackend::File { id, .. } => id,
    }
}

fn emit_backend(backend: &MemoryBackend, id: &str) -> String {
    match backend {
        MemoryBackend::Ram { size, host_nodes, policy, .. } => {
            let mut s = format!("memory-backend-ram,id={id},size={}", format_memory(*size));
            if let Some(hn) = host_nodes {
                s.push_str(&format!(",host-nodes={hn}"));
            }
            if let Some(pol) = policy {
                s.push_str(&format!(",policy={pol}"));
            }
            s
        }
        MemoryBackend::File { size, path, is_egm: true, .. } => {
            format!(
                "memory-backend-file,id={id},size={},mem-path={path},prealloc=on,share=on",
                format_memory(*size)
            )
        }
        MemoryBackend::File { size, path, is_egm: false, host_nodes, policy, .. } => {
            let mut s =
                format!("memory-backend-file,id={id},size={},mem-path={path}", format_memory(*size));
            if let Some(hn) = host_nodes {
                s.push_str(&format!(",host-nodes={hn}"));
            }
            if let Some(pol) = policy {
                s.push_str(&format!(",policy={pol}"));
            }
            s.push_str(",share=on");
            s
        }
    }
}

fn emit_protection(prot: &ProtectionDevice) -> String {
    match prot {
        ProtectionDevice::SevSnp {
            id,
            cbitpos,
            reduced_phys_bits,
            kernel_hashes,
            policy,
            host_data,
        } => {
            let mut s = format!(
                "sev-snp-guest,id={id},cbitpos={cbitpos},reduced-phys-bits={reduced_phys_bits},\
                 kernel-hashes={},policy={policy}",
                if *kernel_hashes { "on" } else { "off" },
            );
            if let Some(hd) = host_data {
                s.push_str(&format!(",host-data={hd}"));
            }
            s
        }
        ProtectionDevice::Tdx { id, quote_generation_socket } => {
            // QEMU's key=value parser cannot represent nested objects, so
            // tdx-guest must be expressed as a JSON -object argument.
            let mut json = format!(r#"{{"qom-type":"tdx-guest","id":"{id}""#);
            if let Some(sock) = quote_generation_socket {
                json.push_str(&format!(
                    r#","quote-generation-socket":{{"type":"{}","cid":"{}","port":"{}"}}"#,
                    sock.ty, sock.cid, sock.port
                ));
            }
            json.push('}');
            json
        }
    }
}

fn emit_machine(machine: &Machine) -> String {
    match machine {
        Machine::Q35(q) => {
            let mut s = format!("q35,accel={}", q.base.accel);
            if let Some(ki) = &q.kernel_irqchip {
                s.push_str(&format!(",kernel_irqchip={ki}"));
            }
            if let Some(cgs) = &q.confidential_guest_support {
                s.push_str(&format!(",confidential-guest-support={cgs}"));
            }
            s
        }
        Machine::Virt(v) => {
            let mut s = format!("virt,accel={}", v.base.accel);
            if let Some(gv) = v.gic_version {
                s.push_str(&format!(",gic-version={gv}"));
            }
            s.push_str(if v.ras { ",ras=on" } else { ",ras=off" });
            if let Some(sz) = v.highmem_mmio_size {
                s.push_str(&format!(",highmem-mmio-size={}", format_memory(sz)));
            }
            if let Some(mb) = &v.base.memory_backend {
                s.push_str(&format!(",memory-backend={mb}"));
            }
            s
        }
        Machine::Pseries(_) => todo!("pSeries machine emission"),
        Machine::S390xCcwVirtio(_) => todo!("s390x machine emission"),
    }
}

fn emit_numa_node(node: &NumaNode) -> String {
    let mut s = "node".to_owned();
    if let Some(memdev) = &node.memdev {
        s.push_str(&format!(",memdev={memdev}"));
    }
    if let Some(cpus) = &node.cpus {
        if cpus.start + 1 == cpus.end {
            s.push_str(&format!(",cpus={}", cpus.start));
        } else {
            s.push_str(&format!(",cpus={}-{}", cpus.start, cpus.end - 1));
        }
    }
    s.push_str(&format!(",nodeid={}", node.nodeid));
    s
}

fn emit_pxb(root: &PciRootComplex, default_bus: &str) -> String {
    let mut s = format!("pxb-pcie,id={},bus={},bus_nr={}", root.id, default_bus, root.bus_nr);
    if let Some(nn) = root.numa_node {
        s.push_str(&format!(",numa_node={nn}"));
    }
    s
}

fn emit_smmu(smmu: &SmmuV3Config, pxb_id: &str) -> String {
    let mut s = format!(
        "arm-smmuv3,primary-bus={pxb_id},id={},accel={},ats={},ril={},pasid={},oas={}",
        smmu.id,
        if smmu.accel { "on" } else { "off" },
        if smmu.ats { "on" } else { "off" },
        if smmu.ril { "on" } else { "off" },
        if smmu.pasid { "on" } else { "off" },
        smmu.oas,
    );
    if smmu.cmdqv {
        s.push_str(",cmdqv=on");
    }
    s
}

fn emit_root_port(port: &PciRootPort, bus: &str) -> String {
    let mut s = format!("pcie-root-port,id={},bus={},chassis={}", port.id, bus, port.chassis);
    if let Some(slot) = port.slot {
        s.push_str(&format!(",slot={slot}"));
    }
    if let Some(mf) = port.multifunction {
        s.push_str(if mf { ",multifunction=on" } else { ",multifunction=off" });
    }
    if let Some(io) = port.io_reserve {
        s.push_str(&format!(",io-reserve={io}"));
    }
    s
}

/// Q35 vfio emission: host → id → [x-pci-vendor-id] → [x-pci-device-id] → bus → [iommufd]
fn emit_vfio_q35(vfio: &VfioDevice, port_id: &str) -> String {
    let device = match vfio.kind {
        VfioDeviceKind::Gpu => "vfio-pci-nohotplug",
        VfioDeviceKind::GpuPci | VfioDeviceKind::NvSwitch | VfioDeviceKind::Nic => "vfio-pci",
    };
    let mut s = format!("{device},host={},id={}", vfio.host, vfio.id);
    if let Some(vid) = vfio.pci_vendor_id {
        s.push_str(&format!(",x-pci-vendor-id={vid:#06x}"));
    }
    if let Some(did) = vfio.pci_device_id {
        s.push_str(&format!(",x-pci-device-id={did:#06x}"));
    }
    s.push_str(&format!(",bus={port_id}"));
    if let Some(ref ifd_id) = vfio.iommufd_id {
        s.push_str(&format!(",iommufd={ifd_id}"));
    }
    s
}

/// Grace/virt vfio emission: device → host → bus → [rombar] → id → [iommufd]
fn emit_vfio_grace(
    vfio: &VfioDevice,
    port_id: &str,
    iommufd: Option<&IommufdBackend>,
) -> String {
    let device = match vfio.kind {
        VfioDeviceKind::Gpu => "vfio-pci-nohotplug",
        VfioDeviceKind::GpuPci | VfioDeviceKind::NvSwitch | VfioDeviceKind::Nic => "vfio-pci",
    };
    let mut s = format!("{device},host={},bus={port_id}", vfio.host);
    if let Some(rombar) = vfio.rombar {
        s.push_str(if rombar { ",rombar=1" } else { ",rombar=0" });
    }
    s.push_str(&format!(",id={}", vfio.id));
    if let Some(ifd) = iommufd {
        s.push_str(&format!(",iommufd={}", ifd.id));
    }
    s
}

fn socket_numa_node(sockets: &[SocketInfo], socket_id: u32) -> u32 {
    sockets
        .iter()
        .position(|s| s.id == socket_id)
        .unwrap_or(socket_id as usize) as u32
}
