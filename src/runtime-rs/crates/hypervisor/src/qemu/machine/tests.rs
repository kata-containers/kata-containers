// Copyright (c) NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0

use std::path::Path;

use super::platform::Platform;
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
    let mut platform = Platform::from_config_defaults(16 << 30).expect("Platform::from_config_defaults");
    platform.apply_host_defaults(&topo);
    let got = platform.to_qemu_args().expect("to_qemu_args");
    let want = load_fixture(fixture);
    assert_eq!(want, got);
}

fn single_socket(cpus: std::ops::Range<u32>) -> Vec<SocketInfo> {
    vec![SocketInfo { id: 0, cpu_range: cpus }]
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
    // NIC is represented differently; exact HostTopology shape TBD in Phase 4.
    // Fixture defines the expected output; this test drives the API design.
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
    check(
        HostTopology {
            sockets: single_socket(0..4),
            gpu_smmu_groups: smmu_groups(&[&["0008:06:00.0"]], 0),
            egm_sockets: vec![],
        },
        "grace_5_vcmdq.args",
    );
}

// ---- Grace Config 6: vEGM, 1 GPU per socket, 4 sockets ----

#[test]
#[ignore = "Phase 5"]
fn grace_6_vegm_1_per_socket() {
    check(
        HostTopology {
            sockets: vec![
                SocketInfo { id: 0, cpu_range: 0..1 },
                SocketInfo { id: 1, cpu_range: 1..2 },
                SocketInfo { id: 2, cpu_range: 2..3 },
                SocketInfo { id: 3, cpu_range: 3..4 },
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
                SocketInfo { id: 0, cpu_range: 0..2 },
                SocketInfo { id: 1, cpu_range: 2..4 },
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
        },
        "grace_7_vegm_2_per_socket.args",
    );
}
