// Copyright (c) NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0

use std::ops::Range;

use anyhow::{bail, Result};
use kata_types::config::hypervisor::Hypervisor as HypervisorConfig;

use super::{
    probe::{HostTopology, SocketInfo},
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
    pub numa_nodes: Vec<NumaNode>,
    pub thread_contexts: Vec<ThreadContext>,
    pub acpi_links: Vec<AcpiPciNodeLink>,
    pub rng: Option<ObjectRngRandom>,
}

pub(crate) struct IommufdBackend {
    pub id: String,
}

pub(crate) enum MemoryBackend {
    Ram { id: String, size: u64 },
    File { id: String, size: u64, path: String, prealloc: bool, share: bool, is_egm: bool },
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
                gic_version: Some(3),
                ras: true,
                highmem_mmio_size: Some(4 << 40),
            }),
            "pseries" => Machine::Pseries(Pseries { base }),
            "s390-ccw-virtio" => Machine::S390xCcwVirtio(S390xCcwVirtio { base }),
            other => bail!("unknown machine type: {other}"),
        };

        Ok(Self {
            machine,
            pci: PciTopology { default_bus: Some("pcie.0".to_owned()), roots: vec![] },
            objects: Objects {
                iommufd: None,
                memory_backends: vec![MemoryBackend::Ram {
                    id: PRIMARY_RAM_ID.to_owned(),
                    size: memory_size,
                }],
                numa_nodes: vec![],
                thread_contexts: vec![],
                acpi_links: vec![],
                rng: None,
            },
        })
    }

    pub fn apply_host_defaults(&mut self, topo: &HostTopology) {
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
                    device: Some(VfioDevice {
                        id: dev_id,
                        host: pci_addr.clone(),
                        rombar: false,
                        kind: VfioDeviceKind::Gpu,
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
                    share: true,
                    is_egm: false,
                },
                other => other,
            })
            .collect();

        // Hugepages provides physically contiguous memory; enable cmdqv on all SMMUs.
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

    pub fn to_qemu_args(&self) -> Result<Vec<String>> {
        let mut args: Vec<String> = Vec::new();

        // iommufd object
        if let Some(ifd) = &self.objects.iommufd {
            args.push("-object".to_owned());
            args.push(format!("iommufd,id={}", ifd.id));
        }

        // Memory backends: EGM configs skip memory_backends[0] and re-index the rest as m0,m1,...
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

        // Machine
        args.push("-machine".to_owned());
        args.push(emit_machine(&self.machine));

        // NUMA nodes
        for node in &self.objects.numa_nodes {
            args.push("-numa".to_owned());
            args.push(emit_numa_node(node));
        }

        // PCI topology
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
                    args.push(emit_vfio(vfio, &port.id, self.objects.iommufd.as_ref()));
                }
            }
        }

        // ACPI links: GenericInitiators before EgmMemory (ACPI SRAT ordering)
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
        MemoryBackend::Ram { size, .. } => {
            format!("memory-backend-ram,size={},id={id}", format_memory(*size))
        }
        MemoryBackend::File { size, path, is_egm: true, .. } => {
            format!(
                "memory-backend-file,id={id},size={},mem-path={path},prealloc=on,share=on",
                format_memory(*size)
            )
        }
        MemoryBackend::File { size, path, is_egm: false, .. } => {
            format!(
                "memory-backend-file,id={id},size={},mem-path={path},prealloc=on,share=on",
                format_memory(*size)
            )
        }
    }
}

fn emit_machine(machine: &Machine) -> String {
    match machine {
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
        Machine::Q35(_) => todo!("Q35 machine emission"),
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
    let mut s = format!("pxb-pcie,id={},bus_nr={},bus={}", root.id, root.bus_nr, default_bus);
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

fn emit_root_port(port: &PciRootPort, pxb_id: &str) -> String {
    format!(
        "pcie-root-port,id={},bus={},chassis={},io-reserve=0",
        port.id, pxb_id, port.chassis
    )
}

fn emit_vfio(vfio: &VfioDevice, port_id: &str, iommufd: Option<&IommufdBackend>) -> String {
    let device = match vfio.kind {
        VfioDeviceKind::Gpu => "vfio-pci-nohotplug",
        VfioDeviceKind::Nic => "vfio-pci",
    };
    let rombar = if vfio.rombar { "1" } else { "0" };
    let mut s = format!("{device},host={},bus={port_id},rombar={rombar},id={}", vfio.host, vfio.id);
    if let Some(ifd) = iommufd {
        s.push_str(&format!(",iommufd={}", ifd.id));
    }
    s
}

// Socket IDs are not guaranteed contiguous; use position to get a dense NUMA node number.
fn socket_numa_node(sockets: &[SocketInfo], socket_id: u32) -> u32 {
    sockets
        .iter()
        .position(|s| s.id == socket_id)
        .unwrap_or(socket_id as usize) as u32
}
