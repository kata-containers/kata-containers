// Copyright (c) NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0

use std::path::Path;

use super::platform::{AcpiPciNodeLink, Machine, MemoryBackend, Platform};
use super::probe::{EgmSocketInfo, GpuSmmuGroup, HostTopology, SocketInfo};

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
        Platform::from_config_defaults(16 << 30).expect("Platform::from_config_defaults");
    platform.apply_host_defaults(&topo);
    let got = platform.to_qemu_args().expect("to_qemu_args");
    let want = load_fixture(fixture);
    assert_eq!(want, got);
}

fn single_socket(cpus: std::ops::Range<u32>) -> Vec<SocketInfo> {
    vec![SocketInfo {
        id: 0,
        cpu_range: cpus,
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

// ---- Phase 1: structural unit tests (not ignored) ----

#[test]
fn from_config_defaults_produces_virt_with_ram() {
    let p = Platform::from_config_defaults(16 << 30).expect("build");
    assert!(matches!(p.machine, Machine::Virt(_)));
    assert_eq!(p.objects.memory_backends.len(), 1);
    assert!(matches!(
        p.objects.memory_backends[0],
        MemoryBackend::Ram { size: s, .. } if s == 16 << 30
    ));
    assert!(p.pci.roots.is_empty());
    assert!(p.objects.iommufd.is_none());
}

#[test]
fn apply_host_defaults_single_gpu() {
    let mut p = Platform::from_config_defaults(16 << 30).unwrap();
    p.apply_host_defaults(&HostTopology {
        sockets: single_socket(0..4),
        gpu_smmu_groups: smmu_groups(&[&["0008:06:00.0"]], 0),
        egm_sockets: vec![],
    });

    assert!(p.objects.iommufd.is_some());
    assert_eq!(p.pci.roots.len(), 1);
    assert_eq!(p.pci.roots[0].root_ports.len(), 1);
    // 8 GenericInitiator NUMA nodes for 1 GPU
    let gi_count = p
        .objects
        .acpi_links
        .iter()
        .filter(|l| matches!(l, AcpiPciNodeLink::GenericInitiator { .. }))
        .count();
    assert_eq!(gi_count, 8);
    // No EGM links when egm_sockets is empty
    let egm_count = p
        .objects
        .acpi_links
        .iter()
        .filter(|l| matches!(l, AcpiPciNodeLink::EgmMemory { .. }))
        .count();
    assert_eq!(egm_count, 0);
}

#[test]
fn apply_host_defaults_four_gpus_two_per_smmu() {
    let mut p = Platform::from_config_defaults(16 << 30).unwrap();
    p.apply_host_defaults(&HostTopology {
        sockets: single_socket(0..4),
        gpu_smmu_groups: smmu_groups(
            &[
                &["0008:06:00.0", "0009:06:00.0"],
                &["0010:06:00.0", "0011:06:00.0"],
            ],
            0,
        ),
        egm_sockets: vec![],
    });

    // 2 PciRootComplexes, each with 2 ports
    assert_eq!(p.pci.roots.len(), 2);
    assert_eq!(p.pci.roots[0].root_ports.len(), 2);
    assert_eq!(p.pci.roots[1].root_ports.len(), 2);
    // 4 GPUs × 8 = 32 GenericInitiator nodes
    let gi_count = p
        .objects
        .acpi_links
        .iter()
        .filter(|l| matches!(l, AcpiPciNodeLink::GenericInitiator { .. }))
        .count();
    assert_eq!(gi_count, 32);
}

#[test]
fn apply_host_defaults_egm_adds_backends_and_links() {
    let mut p = Platform::from_config_defaults(16 << 30).unwrap();
    p.apply_host_defaults(&HostTopology {
        sockets: vec![
            SocketInfo { id: 0, cpu_range: 0..1 },
            SocketInfo { id: 1, cpu_range: 1..2 },
        ],
        gpu_smmu_groups: vec![
            GpuSmmuGroup { pci_bus_addrs: vec!["0008:06:00.0".into()], socket: 0 },
            GpuSmmuGroup { pci_bus_addrs: vec!["0009:06:00.0".into()], socket: 1 },
        ],
        egm_sockets: vec![
            EgmSocketInfo { path: "/dev/egm4".into(), socket: 0, total_size: 56896 << 20 },
            EgmSocketInfo { path: "/dev/egm5".into(), socket: 1, total_size: 56896 << 20 },
        ],
    });

    // Primary RAM + 2 EGM file backends
    assert_eq!(p.objects.memory_backends.len(), 3);
    assert!(matches!(p.objects.memory_backends[1], MemoryBackend::File { ref path, .. } if path == "/dev/egm4"));
    assert!(matches!(p.objects.memory_backends[2], MemoryBackend::File { ref path, .. } if path == "/dev/egm5"));

    // 2 GPUs × 1 EgmMemory link each
    let egm_count = p
        .objects
        .acpi_links
        .iter()
        .filter(|l| matches!(l, AcpiPciNodeLink::EgmMemory { .. }))
        .count();
    assert_eq!(egm_count, 2);
}

#[test]
fn with_hugepages_replaces_ram_backend() {
    let p = Platform::from_config_defaults(16 << 30).unwrap();
    let p = p.with_hugepages("/dev/hugepages");
    assert_eq!(p.objects.memory_backends.len(), 1);
    assert!(matches!(
        p.objects.memory_backends[0],
        MemoryBackend::File { ref path, prealloc: true, .. } if path == "/dev/hugepages"
    ));
}

#[test]
fn with_hugepages_preserves_egm_backends() {
    let mut p = Platform::from_config_defaults(16 << 30).unwrap();
    p.apply_host_defaults(&HostTopology {
        sockets: single_socket(0..4),
        gpu_smmu_groups: smmu_groups(&[&["0008:06:00.0"]], 0),
        egm_sockets: vec![EgmSocketInfo {
            path: "/dev/egm4".into(),
            socket: 0,
            total_size: 56896 << 20,
        }],
    });
    let p = p.with_hugepages("/dev/hugepages");

    // RAM backend swapped, EGM file backend preserved
    assert_eq!(p.objects.memory_backends.len(), 2);
    assert!(matches!(
        p.objects.memory_backends[0],
        MemoryBackend::File { ref path, prealloc: true, .. } if path == "/dev/hugepages"
    ));
    assert!(matches!(
        p.objects.memory_backends[1],
        MemoryBackend::File { ref path, .. } if path == "/dev/egm4"
    ));
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
    // emission.  The call is a stub until Phase 5 implements it, so the test
    // stays ignored, but the body already documents the required step.
    let topo = HostTopology {
        sockets: single_socket(0..4),
        gpu_smmu_groups: smmu_groups(&[&["0008:06:00.0"]], 0),
        egm_sockets: vec![],
    };
    let mut platform =
        Platform::from_config_defaults(16 << 30).expect("Platform::from_config_defaults");
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
                SocketInfo {
                    id: 0,
                    cpu_range: 0..1,
                },
                SocketInfo {
                    id: 1,
                    cpu_range: 1..2,
                },
                SocketInfo {
                    id: 2,
                    cpu_range: 2..3,
                },
                SocketInfo {
                    id: 3,
                    cpu_range: 3..4,
                },
            ],
            gpu_smmu_groups: vec![
                GpuSmmuGroup {
                    pci_bus_addrs: vec!["0008:06:00.0".into()],
                    socket: 0,
                },
                GpuSmmuGroup {
                    pci_bus_addrs: vec!["0009:06:00.0".into()],
                    socket: 1,
                },
                GpuSmmuGroup {
                    pci_bus_addrs: vec!["0010:06:00.0".into()],
                    socket: 2,
                },
                GpuSmmuGroup {
                    pci_bus_addrs: vec!["0011:06:00.0".into()],
                    socket: 3,
                },
            ],
            egm_sockets: vec![
                EgmSocketInfo {
                    path: "/dev/egm4".into(),
                    socket: 0,
                    total_size: 56896 << 20,
                },
                EgmSocketInfo {
                    path: "/dev/egm5".into(),
                    socket: 1,
                    total_size: 56896 << 20,
                },
                EgmSocketInfo {
                    path: "/dev/egm6".into(),
                    socket: 2,
                    total_size: 56896 << 20,
                },
                EgmSocketInfo {
                    path: "/dev/egm7".into(),
                    socket: 3,
                    total_size: 56896 << 20,
                },
            ],
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
                SocketInfo {
                    id: 0,
                    cpu_range: 0..2,
                },
                SocketInfo {
                    id: 1,
                    cpu_range: 2..4,
                },
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
                EgmSocketInfo {
                    path: "/dev/egm4".into(),
                    socket: 0,
                    total_size: 56896 << 20,
                },
                EgmSocketInfo {
                    path: "/dev/egm5".into(),
                    socket: 1,
                    total_size: 56896 << 20,
                },
            ],
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
// Blocked on Phase 3:
//   - Objects::protection for sev-snp-guest / tdx-guest
//   - Q35::kernel_irqchip and confidential_guest_support typed fields
//   - MemoryBackend::Ram { host_nodes, policy } for NUMA pinning
//   - Per-device iommufd (not shared iommufd0)
//   - VfioDevice vendor_id / device_id overrides

#[test]
#[ignore = "Phase 3: CoCo protection object + Q35 confidential-guest-support not yet implemented"]
fn q35_coco_snp_single_gpu() {
    // HostTopology shape TBD in Phase 3.
    // 1 socket, cpus 0-16, host-node 1; GPU 0000:e1:00.0 on pxb-pcie bus_nr=32.
    // sev-snp-guest: cbitpos=51, reduced-phys-bits=1, kernel-hashes=on, policy=196608.
    todo!("Phase 3")
}

// ---- Q35 x86_64: vanilla kata, 2-socket NUMA, 8 cold-plug root ports ----
//
// Production capture: DGX x86 host, 2026-07-07.  65 vCPUs, 73728M total,
// 36864M per socket pinned to host NUMA node via /dev/shm.  8 pcie-root-ports
// pre-provisioned on pcie.0 for GPU cold-plug (hot_plug_vfio=no-port).
//
// Blocked on Phase 3:
//   - Q35 machine in Platform::to_qemu_args (no gic-version, no highmem-mmio-size)
//   - MemoryBackend::File { host_nodes, policy } fields for NUMA SHM pinning
//   - Objects::numa_distances for -numa dist entries
//   - HostTopology fields for NUMA node + SHM path per socket

#[test]
#[ignore = "Phase 3: Q35 machine + NUMA SHM memory model not yet implemented"]
fn q35_vanilla_kata_x86() {
    // HostTopology shape TBD in Phase 3.
    // 2 sockets: socket 0 cpus 0-32 (host-node 0), socket 1 cpus 33-65 (host-node 1).
    // No gpu_smmu_groups, no egm_sockets.
    // 8 cold-plug root ports on pcie.0 (cold_plug_vfio=root-port, pcie_root_port=8);
    // NUMA distance 20 between the two nodes.
    todo!("Phase 3")
}
