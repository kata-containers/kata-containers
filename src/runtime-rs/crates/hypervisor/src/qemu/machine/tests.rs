// Copyright (c) NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0

use std::path::Path;

use super::platform::{
    BaseMachine, CpuConfig, CpuModel, Machine, MemoryBackend, NumaNode, Objects, Platform,
};
use super::probe::{EgmSocketInfo, GpuSmmuGroup, HostTopology, ProtectionDevice, SocketInfo};
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
    // Placeholder until Phase 4: HostTopology cannot represent NIC passthrough
    // yet, so this topology covers only the GPU half and the test stays
    // ignored.  Phase 4 adds nic_smmu_groups and rewrites this body; the
    // fixture already defines the full expected output (GPU on pcie.1, NIC on
    // pcie.2 with no acpi-generic-initiator links).
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
    // Config 5 backs guest RAM with hugepages: with_hugepages() swaps the
    // primary backend to mem-path=/dev/hugepages/ with prealloc=on before
    // emission.
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
                host_data: Some("S0FUQS1TWU5USEVUSUMtSE9TVC1EQVRBLTAwMDAwMDA=".to_owned()),
            }),
        },
    };

    let got = platform.to_qemu_args().expect("to_qemu_args");
    let want = load_fixture("q35_coco_snp_single_gpu.args");
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
