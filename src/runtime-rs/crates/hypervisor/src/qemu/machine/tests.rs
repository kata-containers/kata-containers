// Copyright (c) NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0

use std::path::Path;

use super::platform::{
    BaseMachine, CpuConfig, CpuModel, Machine, MemoryBackend, NumaNode, Objects, Platform,
};
use super::probe::{
    EgmSocketInfo, GpuSmmuGroup, HostTopology, ProtectionDevice, SocketInfo, TdxQuoteSocket,
};
use super::q35::Q35;
use super::topology::{PciRootComplex, PciRootPort, PciTopology, VfioDevice, VfioDeviceKind};

// Each fixture file contains one Vec<String> element per line.
// Blank lines and lines starting with '#' are ignored.
fn load_fixture(name: &str) -> Vec<String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/qemu")
        .join(name);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("fixture {}: {e}", path.display()))
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(String::from)
        .collect()
}

fn check(topo: HostTopology, fixture: &str) {
    let mut platform =
        Platform::from_config_defaults("virt", 16 << 30).expect("Platform::from_config_defaults");
    platform.apply_host_defaults(&topo);
    let got = platform.to_qemu_args().expect("to_qemu_args");
    let want = load_fixture(fixture);
    assert_eq!(want, got);
}

fn single_socket(cpus: std::ops::Range<u32>) -> Vec<SocketInfo> {
    vec![SocketInfo {
        id: 0,
        cpu_range: cpus,
        host_node: None,
        mem_path: None,
        mem_size: None,
    }]
}

fn smmu_groups(addrs: &[&[&str]], socket: u32) -> Vec<GpuSmmuGroup> {
    addrs
        .iter()
        .map(|group| GpuSmmuGroup {
            pci_bus_addrs: group.iter().map(|s| s.to_string()).collect(),
            socket,
        })
        .collect()
}

// ---- Grace Config 1: single GPU, 1 SMMU, 9 NUMA nodes ----

#[test]
#[ignore = "Phase 4"]
fn grace_1_single_gpu() {
    check(
        HostTopology {
            sockets: single_socket(0..4),
            gpu_smmu_groups: smmu_groups(&[&["0008:06:00.0"]], 0),
            egm_sockets: vec![],
            numa_distances: vec![],
            pcie_root_port: 0,
            protection: None,
        },
        "grace_1_single_gpu.args",
    );
}

// ---- Grace Config 2: 4 GPUs, 1 GPU per SMMU, 33 NUMA nodes ----

#[test]
#[ignore = "Phase 4"]
fn grace_2_four_gpus_1_per_smmu() {
    check(
        HostTopology {
            sockets: single_socket(0..4),
            gpu_smmu_groups: smmu_groups(
                &[
                    &["0008:06:00.0"],
                    &["0009:06:00.0"],
                    &["0010:06:00.0"],
                    &["0011:06:00.0"],
                ],
                0,
            ),
            egm_sockets: vec![],
            numa_distances: vec![],
            pcie_root_port: 0,
            protection: None,
        },
        "grace_2_four_gpus_1_per_smmu.args",
    );
}

// ---- Grace Config 3: 4 GPUs, 2 GPUs per SMMU, 33 NUMA nodes ----

#[test]
#[ignore = "Phase 4"]
fn grace_3_four_gpus_2_per_smmu() {
    check(
        HostTopology {
            sockets: single_socket(0..4),
            gpu_smmu_groups: smmu_groups(
                &[
                    &["0008:06:00.0", "0009:06:00.0"],
                    &["0010:06:00.0", "0011:06:00.0"],
                ],
                0,
            ),
            egm_sockets: vec![],
            numa_distances: vec![],
            pcie_root_port: 0,
            protection: None,
        },
        "grace_3_four_gpus_2_per_smmu.args",
    );
}

// ---- Grace Config 4: GPU + NIC passthrough ----

#[test]
#[ignore = "Phase 4"]
fn grace_4_gpu_and_nic() {
    // NIC is represented differently; exact HostTopology shape TBD in Phase 4.
    // Fixture defines the expected output; this test drives the API design.
    check(
        HostTopology {
            sockets: single_socket(0..4),
            gpu_smmu_groups: smmu_groups(&[&["0008:06:00.0"]], 0),
            egm_sockets: vec![],
            numa_distances: vec![],
            pcie_root_port: 0,
            protection: None,
        },
        "grace_4_gpu_and_nic.args",
    );
}

// ---- Grace Config 5: vCMDQ, hugepages backing ----

#[test]
#[ignore = "Phase 5"]
fn grace_5_vcmdq() {
    let topo = HostTopology {
        sockets: single_socket(0..4),
        gpu_smmu_groups: smmu_groups(&[&["0008:06:00.0"]], 0),
        egm_sockets: vec![],
        numa_distances: vec![],
        pcie_root_port: 0,
        protection: None,
    };
    let mut platform = Platform::from_config_defaults("virt", 16 << 30).expect("build");
    platform.apply_host_defaults(&topo);
    let platform = platform.with_hugepages("/dev/hugepages/");
    let got = platform.to_qemu_args().expect("to_qemu_args");
    let want = load_fixture("grace_5_vcmdq.args");
    assert_eq!(want, got);
}

// ---- Grace Config 6: vEGM, 1 GPU per socket, 4 sockets ----

#[test]
#[ignore = "Phase 5"]
fn grace_6_vegm_1_per_socket() {
    check(
        HostTopology {
            sockets: vec![
                SocketInfo { id: 0, cpu_range: 0..1, host_node: None, mem_path: None, mem_size: None },
                SocketInfo { id: 1, cpu_range: 1..2, host_node: None, mem_path: None, mem_size: None },
                SocketInfo { id: 2, cpu_range: 2..3, host_node: None, mem_path: None, mem_size: None },
                SocketInfo { id: 3, cpu_range: 3..4, host_node: None, mem_path: None, mem_size: None },
            ],
            gpu_smmu_groups: vec![
                GpuSmmuGroup { pci_bus_addrs: vec!["0008:06:00.0".into()], socket: 0 },
                GpuSmmuGroup { pci_bus_addrs: vec!["0009:06:00.0".into()], socket: 1 },
                GpuSmmuGroup { pci_bus_addrs: vec!["0010:06:00.0".into()], socket: 2 },
                GpuSmmuGroup { pci_bus_addrs: vec!["0011:06:00.0".into()], socket: 3 },
            ],
            egm_sockets: vec![
                EgmSocketInfo { path: "/dev/egm4".into(), socket: 0, total_size: 56896 << 20 },
                EgmSocketInfo { path: "/dev/egm5".into(), socket: 1, total_size: 56896 << 20 },
                EgmSocketInfo { path: "/dev/egm6".into(), socket: 2, total_size: 56896 << 20 },
                EgmSocketInfo { path: "/dev/egm7".into(), socket: 3, total_size: 56896 << 20 },
            ],
            numa_distances: vec![],
            pcie_root_port: 0,
            protection: None,
        },
        "grace_6_vegm_1_per_socket.args",
    );
}

// ---- Grace Config 7: vEGM, 2 GPUs per socket, 2 sockets ----

#[test]
#[ignore = "Phase 5"]
fn grace_7_vegm_2_per_socket() {
    check(
        HostTopology {
            sockets: vec![
                SocketInfo { id: 0, cpu_range: 0..2, host_node: None, mem_path: None, mem_size: None },
                SocketInfo { id: 1, cpu_range: 2..4, host_node: None, mem_path: None, mem_size: None },
            ],
            gpu_smmu_groups: vec![
                GpuSmmuGroup {
                    pci_bus_addrs: vec!["0008:06:00.0".into(), "0009:06:00.0".into()],
                    socket: 0,
                },
                GpuSmmuGroup {
                    pci_bus_addrs: vec!["0010:06:00.0".into(), "0011:06:00.0".into()],
                    socket: 1,
                },
            ],
            egm_sockets: vec![
                EgmSocketInfo { path: "/dev/egm4".into(), socket: 0, total_size: 56896 << 20 },
                EgmSocketInfo { path: "/dev/egm5".into(), socket: 1, total_size: 56896 << 20 },
            ],
            numa_distances: vec![],
            pcie_root_port: 0,
            protection: None,
        },
        "grace_7_vegm_2_per_socket.args",
    );
}

// ---- Q35 CoCo (SEV-SNP) + single GPU — AMD EPYC host, H100 80GB ----
//
// Production capture: AMD EPYC host, 2026-07-13.  17 vCPUs, 57344M, single
// NUMA node pinned to host-node 1 via memory-backend-ram (not file-backed).
// sev-snp-guest object emitted before -machine.  iommufd is per-device.
// vfio-pci (not nohotplug); x-pci-vendor-id/device-id for CoCo attestation.
//
// Platform is built directly (not via apply_host_defaults) because the
// per-device iommufd UUIDs are assigned by the runtime at VM launch time
// and cannot be derived from HostTopology alone.  apply_host_defaults for
// CoCo topology will be wired end-to-end in Phase 4.

#[test]
fn q35_coco_snp_single_gpu() {
    let platform = Platform {
        machine: Machine::Q35(Q35 {
            base: BaseMachine {
                accel: "kvm".to_owned(),
                memory_backend: None,
                cpu: CpuConfig { model: CpuModel::Host { extra_features: vec![] } },
            },
            kernel_irqchip: Some("split".to_owned()),
            confidential_guest_support: Some("snp".to_owned()),
            intel_iommu: None,
        }),
        pci: PciTopology {
            default_bus: Some("pcie.0".to_owned()),
            roots: vec![PciRootComplex {
                id: "pxb-numa0".to_owned(),
                bus_nr: 32,
                numa_node: Some(0),
                iommu: None,
                root_ports: vec![PciRootPort {
                    id: "rp-numa0-0".to_owned(),
                    chassis: 10,
                    slot: Some(0),
                    multifunction: Some(false),
                    io_reserve: None,
                    device: Some(VfioDevice {
                        id: "vfio-ab57592a4d2482201".to_owned(),
                        host: "0000:e1:00.0".to_owned(),
                        rombar: None,
                        kind: VfioDeviceKind::GpuPci,
                        iommufd_id: Some("iommufdvfio-ab57592a4d2482201".to_owned()),
                        pci_vendor_id: Some(0x10de),
                        pci_device_id: Some(0x2321),
                    }),
                }],
            }],
            pcie_root_port: vec![],
        },
        objects: Objects {
            iommufd: None,
            memory_backends: vec![MemoryBackend::Ram {
                id: "numa-mem0".to_owned(),
                size: 57344 << 20,
                host_nodes: Some(1),
                policy: Some("bind".to_owned()),
            }],
            numa_nodes: vec![NumaNode {
                nodeid: 0,
                memdev: Some("numa-mem0".to_owned()),
                cpus: Some(0..17),
            }],
            numa_distances: vec![],
            thread_contexts: vec![],
            acpi_links: vec![],
            rng: None,
            protection: Some(ProtectionDevice::SevSnp {
                id: "snp".to_owned(),
                cbitpos: 51,
                reduced_phys_bits: 1,
                kernel_hashes: true,
                policy: 196608,
                host_data: Some("CexG7r8OExqKVdTzwFteO3U9GZRYx7lslmObi8SdcVA=".to_owned()),
            }),
        },
    };

    let got = platform.to_qemu_args().expect("to_qemu_args");
    let want = load_fixture("q35_coco_snp_single_gpu.args");
    assert_eq!(want, got);
}

// ---- Q35 vanilla (non-CoCo) + 8 GPUs + 4 NVSwitches — HGX H100 PPCIE ----
//
// Production capture: same host as the TDX capture, 2026-07-15.
// Same physical topology (8 H100 GPUs + 4 NVSwitches, 2 NUMA nodes, /dev/shm
// memory, NUMA distance 21) but without CoCo:
//   - No protection object; -machine q35,accel=kvm (no kernel_irqchip)
//   - memory-backend-file via /dev/shm (not memory-backend-ram)
//   - Per-device iommufd retained (modern VFIO interface, CoCo-independent)
//   - x-pci-vendor-id/device-id retained (kata applies them for any GPU passthrough)
//   - NUMA node 1 is memory-only; the single vCPU lives on node 0

#[test]
fn q35_vanilla_8gpu_4nvswitch() {
    use VfioDeviceKind::{GpuPci, NvSwitch};

    fn rp(
        id: &str,
        chassis: u8,
        slot: u8,
        host: &str,
        vfio_id: &str,
        iommufd_id: &str,
        kind: VfioDeviceKind,
        device_id: u16,
    ) -> PciRootPort {
        PciRootPort {
            id: id.to_owned(),
            chassis,
            slot: Some(slot),
            multifunction: Some(false),
            io_reserve: None,
            device: Some(VfioDevice {
                id: vfio_id.to_owned(),
                host: host.to_owned(),
                rombar: None,
                kind,
                iommufd_id: Some(iommufd_id.to_owned()),
                pci_vendor_id: Some(0x10de),
                pci_device_id: Some(device_id),
            }),
        }
    }

    let platform = Platform {
        machine: Machine::Q35(Q35 {
            base: BaseMachine {
                accel: "kvm".to_owned(),
                memory_backend: None,
                cpu: CpuConfig { model: CpuModel::Host { extra_features: vec![] } },
            },
            kernel_irqchip: None,
            confidential_guest_support: None,
            intel_iommu: None,
        }),
        pci: PciTopology {
            default_bus: Some("pcie.0".to_owned()),
            roots: vec![
                PciRootComplex {
                    id: "pxb-numa0".to_owned(),
                    bus_nr: 32,
                    numa_node: Some(0),
                    iommu: None,
                    root_ports: vec![
                        rp("rp-numa0-0", 10, 0, "0000:1b:00.0", "vfio-5c7a307d86e7ae830",    "iommufdvfio-5c7a307d86e7ae830",    GpuPci,   0x2330),
                        rp("rp-numa0-1", 10, 1, "0000:43:00.0", "vfio-98077fd3aa014b541",    "iommufdvfio-98077fd3aa014b541",    GpuPci,   0x2330),
                        rp("rp-numa0-2", 10, 2, "0000:52:00.0", "vfio-35b65d16a30068022",    "iommufdvfio-35b65d16a30068022",    GpuPci,   0x2330),
                        rp("rp-numa0-3", 10, 3, "0000:61:00.0", "vfio-331446c1d73050393",    "iommufdvfio-331446c1d73050393",    GpuPci,   0x2330),
                        rp("rp-numa0-4", 10, 4, "0000:07:00.0", "vfio-b91919c2fa001cd18",    "iommufdvfio-b91919c2fa001cd18",    NvSwitch, 0x22a3),
                        rp("rp-numa0-5", 10, 5, "0000:08:00.0", "vfio-fab9e6a06185873e9",    "iommufdvfio-fab9e6a06185873e9",    NvSwitch, 0x22a3),
                        rp("rp-numa0-6", 10, 6, "0000:09:00.0", "vfio-3a77ef115d3f0ab610",   "iommufdvfio-3a77ef115d3f0ab610",   NvSwitch, 0x22a3),
                        rp("rp-numa0-7", 10, 7, "0000:0a:00.0", "vfio-2e0a336d490089a111",   "iommufdvfio-2e0a336d490089a111",   NvSwitch, 0x22a3),
                    ],
                },
                PciRootComplex {
                    id: "pxb-numa1".to_owned(),
                    bus_nr: 64,
                    numa_node: Some(1),
                    iommu: None,
                    root_ports: vec![
                        rp("rp-numa1-0", 11, 0, "0000:9d:00.0", "vfio-7f57f8dea75cdeca4",    "iommufdvfio-7f57f8dea75cdeca4",    GpuPci, 0x2330),
                        rp("rp-numa1-1", 11, 1, "0000:c3:00.0", "vfio-4461976d3de811155",    "iommufdvfio-4461976d3de811155",    GpuPci, 0x2330),
                        rp("rp-numa1-2", 11, 2, "0000:d1:00.0", "vfio-dc1944aed29728336",    "iommufdvfio-dc1944aed29728336",    GpuPci, 0x2330),
                        rp("rp-numa1-3", 11, 3, "0000:df:00.0", "vfio-5c974cdc2dcfe4cb7",    "iommufdvfio-5c974cdc2dcfe4cb7",    GpuPci, 0x2330),
                    ],
                },
            ],
            pcie_root_port: vec![],
        },
        objects: Objects {
            iommufd: None,
            memory_backends: vec![
                MemoryBackend::File {
                    id: "numa-mem0".to_owned(),
                    size: 4096 << 20,
                    path: "/dev/shm".to_owned(),
                    prealloc: false,
                    share: true,
                    host_nodes: Some(0),
                    policy: Some("bind".to_owned()),
                    is_egm: false,
                },
                MemoryBackend::File {
                    id: "numa-mem1".to_owned(),
                    size: 4096 << 20,
                    path: "/dev/shm".to_owned(),
                    prealloc: false,
                    share: true,
                    host_nodes: Some(1),
                    policy: Some("bind".to_owned()),
                    is_egm: false,
                },
            ],
            numa_nodes: vec![
                NumaNode { nodeid: 0, memdev: Some("numa-mem0".to_owned()), cpus: Some(0..1) },
                NumaNode { nodeid: 1, memdev: Some("numa-mem1".to_owned()), cpus: None },
            ],
            numa_distances: vec![(0, 1, 21), (1, 0, 21)],
            thread_contexts: vec![],
            acpi_links: vec![],
            rng: None,
            protection: None,
        },
    };

    let got = platform.to_qemu_args().expect("to_qemu_args");
    let want = load_fixture("q35_vanilla_8gpu_4nvswitch.args");
    assert_eq!(want, got);
}

// ---- Q35 CoCo (TDX) + 8 GPUs + 4 NVSwitches — Intel TDX host, HGX H100 PPCIE ----
//
// Production capture: Intel TDX host, 2026-07-15.  1 vCPU, 8192M, 2 NUMA nodes.
// 4 H100 SXM5 GPUs (0x10de:0x2330) + 4 NVSwitch 3.0 (0x10de:0x22a3) on
// pxb-numa0 (bus_nr=32, chassis=10), 4 more H100 GPUs on pxb-numa1 (bus_nr=64,
// chassis=11).  Node 1 is memory-only (no cpus= field).  NUMA distance 21.
// TDX object emitted as JSON (not key=value); includes vsock quote-generation-socket.
// NVSwitches use root-port + vfio-pci topology — NOT x3130 switch-port hierarchy.
//
// Platform is built directly (not via apply_host_defaults) because per-device
// iommufd UUIDs are assigned by the runtime at VM launch time and because
// NvSwitch vs GPU classification is not yet in HostTopology (Phase 4).

#[test]
fn q35_coco_tdx_8gpu_4nvswitch() {
    use VfioDeviceKind::{GpuPci, NvSwitch};

    fn rp(
        id: &str,
        chassis: u8,
        slot: u8,
        host: &str,
        vfio_id: &str,
        iommufd_id: &str,
        kind: VfioDeviceKind,
        device_id: u16,
    ) -> PciRootPort {
        PciRootPort {
            id: id.to_owned(),
            chassis,
            slot: Some(slot),
            multifunction: Some(false),
            io_reserve: None,
            device: Some(VfioDevice {
                id: vfio_id.to_owned(),
                host: host.to_owned(),
                rombar: None,
                kind,
                iommufd_id: Some(iommufd_id.to_owned()),
                pci_vendor_id: Some(0x10de),
                pci_device_id: Some(device_id),
            }),
        }
    }

    let platform = Platform {
        machine: Machine::Q35(Q35 {
            base: BaseMachine {
                accel: "kvm".to_owned(),
                memory_backend: None,
                cpu: CpuConfig { model: CpuModel::Host { extra_features: vec![] } },
            },
            kernel_irqchip: Some("split".to_owned()),
            confidential_guest_support: Some("tdx".to_owned()),
            intel_iommu: None,
        }),
        pci: PciTopology {
            default_bus: Some("pcie.0".to_owned()),
            roots: vec![
                PciRootComplex {
                    id: "pxb-numa0".to_owned(),
                    bus_nr: 32,
                    numa_node: Some(0),
                    iommu: None,
                    root_ports: vec![
                        rp("rp-numa0-0", 10, 0, "0000:1b:00.0", "vfio-4a182f997e753b150",  "iommufdvfio-4a182f997e753b150",  GpuPci,   0x2330),
                        rp("rp-numa0-1", 10, 1, "0000:43:00.0", "vfio-03bafd69f614ffb21",  "iommufdvfio-03bafd69f614ffb21",  GpuPci,   0x2330),
                        rp("rp-numa0-2", 10, 2, "0000:52:00.0", "vfio-947368d0843e55b42",  "iommufdvfio-947368d0843e55b42",  GpuPci,   0x2330),
                        rp("rp-numa0-3", 10, 3, "0000:61:00.0", "vfio-41adfe2466817df03",  "iommufdvfio-41adfe2466817df03",  GpuPci,   0x2330),
                        rp("rp-numa0-4", 10, 4, "0000:07:00.0", "vfio-7eff1447579be32f8",  "iommufdvfio-7eff1447579be32f8",  NvSwitch, 0x22a3),
                        rp("rp-numa0-5", 10, 5, "0000:08:00.0", "vfio-bfaa424dfcf1b24d9",  "iommufdvfio-bfaa424dfcf1b24d9",  NvSwitch, 0x22a3),
                        rp("rp-numa0-6", 10, 6, "0000:09:00.0", "vfio-585d94b2d0bd3b6b10", "iommufdvfio-585d94b2d0bd3b6b10", NvSwitch, 0x22a3),
                        rp("rp-numa0-7", 10, 7, "0000:0a:00.0", "vfio-44e4edb8e24e522911", "iommufdvfio-44e4edb8e24e522911", NvSwitch, 0x22a3),
                    ],
                },
                PciRootComplex {
                    id: "pxb-numa1".to_owned(),
                    bus_nr: 64,
                    numa_node: Some(1),
                    iommu: None,
                    root_ports: vec![
                        rp("rp-numa1-0", 11, 0, "0000:9d:00.0", "vfio-fd42e60b81a629b24",  "iommufdvfio-fd42e60b81a629b24",  GpuPci, 0x2330),
                        rp("rp-numa1-1", 11, 1, "0000:c3:00.0", "vfio-c3b4a6ef942a66a45",  "iommufdvfio-c3b4a6ef942a66a45",  GpuPci, 0x2330),
                        rp("rp-numa1-2", 11, 2, "0000:d1:00.0", "vfio-4ababdb3c8421bf06",  "iommufdvfio-4ababdb3c8421bf06",  GpuPci, 0x2330),
                        rp("rp-numa1-3", 11, 3, "0000:df:00.0", "vfio-2e0f5a646c1bbd557",  "iommufdvfio-2e0f5a646c1bbd557",  GpuPci, 0x2330),
                    ],
                },
            ],
            pcie_root_port: vec![],
        },
        objects: Objects {
            iommufd: None,
            memory_backends: vec![
                MemoryBackend::Ram {
                    id: "numa-mem0".to_owned(),
                    size: 4096 << 20,
                    host_nodes: Some(0),
                    policy: Some("bind".to_owned()),
                },
                MemoryBackend::Ram {
                    id: "numa-mem1".to_owned(),
                    size: 4096 << 20,
                    host_nodes: Some(1),
                    policy: Some("bind".to_owned()),
                },
            ],
            numa_nodes: vec![
                NumaNode { nodeid: 0, memdev: Some("numa-mem0".to_owned()), cpus: Some(0..1) },
                // Node 1 is memory-only: the single vCPU lives on node 0.
                NumaNode { nodeid: 1, memdev: Some("numa-mem1".to_owned()), cpus: None },
            ],
            numa_distances: vec![(0, 1, 21), (1, 0, 21)],
            thread_contexts: vec![],
            acpi_links: vec![],
            rng: None,
            protection: Some(ProtectionDevice::Tdx {
                id: "tdx".to_owned(),
                quote_generation_socket: Some(TdxQuoteSocket {
                    ty: "vsock".to_owned(),
                    cid: "2".to_owned(),
                    port: "4050".to_owned(),
                }),
            }),
        },
    };

    let got = platform.to_qemu_args().expect("to_qemu_args");
    let want = load_fixture("q35_coco_tdx_8gpu_4nvswitch.args");
    assert_eq!(want, got);
}

// ---- Q35 x86_64: vanilla kata, 2-socket NUMA, 8 cold-plug root ports ----
//
// Production capture: DGX x86 host, 2026-07-07.  66 vCPUs, 73728M total,
// 36864M per socket pinned to host NUMA node via /dev/shm.  8 pcie-root-ports
// pre-provisioned on pcie.0 for GPU cold-plug (hot_plug_vfio=no-port).

#[test]
fn q35_vanilla_kata_x86() {
    let topo = HostTopology {
        sockets: vec![
            SocketInfo {
                id: 0,
                cpu_range: 0..33,
                host_node: Some(0),
                mem_path: Some("/dev/shm".into()),
                mem_size: Some(36864 << 20),
            },
            SocketInfo {
                id: 1,
                cpu_range: 33..66,
                host_node: Some(1),
                mem_path: Some("/dev/shm".into()),
                mem_size: Some(36864 << 20),
            },
        ],
        gpu_smmu_groups: vec![],
        egm_sockets: vec![],
        numa_distances: vec![(0, 1, 20), (1, 0, 20)],
        pcie_root_port: 8,
        protection: None,
    };

    let mut platform = Platform::from_config_defaults("q35", 0).expect("from_config_defaults");
    platform.apply_host_defaults(&topo);
    let got = platform.to_qemu_args().expect("to_qemu_args");
    let want = load_fixture("q35_vanilla_kata_x86.args");
    assert_eq!(want, got);
}
