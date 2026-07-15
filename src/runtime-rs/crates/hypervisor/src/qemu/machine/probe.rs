// Copyright (c) NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashMap;
use std::ops::Range;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

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

// ──────────────────────────────────────────────────────────────────────────────
// Host topology prober
// ──────────────────────────────────────────────────────────────────────────────

/// NVIDIA PCI vendor ID.
const NVIDIA_VENDOR_ID: u32 = 0x10de;

/// PCI class codes for devices we care about.
/// The full 24-bit class code is: Class (8) | Subclass (8) | Prog-IF (8).
/// We match on the top 16 bits (Class | Subclass).
const CLASS_3D_CONTROLLER: u32 = 0x0302; // NVIDIA GPU (non-display)
const CLASS_VGA_CONTROLLER: u32 = 0x0300; // NVIDIA GPU (VGA-compatible)
const CLASS_NETWORK_CONTROLLER: u32 = 0x0200; // Ethernet / network
const CLASS_INFINIBAND_CONTROLLER: u32 = 0x0207; // InfiniBand (CX-7 etc.)

/// Reads a hex integer from a sysfs file, stripping leading "0x" and whitespace.
fn read_sysfs_hex(path: &Path) -> Result<u32> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("reading {}", path.display()))?;
    let trimmed = raw.trim().trim_start_matches("0x");
    u32::from_str_radix(trimmed, 16)
        .with_context(|| format!("parsing hex from {}", path.display()))
}

fn read_sysfs_i32(path: &Path) -> Result<i32> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("reading {}", path.display()))?;
    raw.trim()
        .parse::<i32>()
        .with_context(|| format!("parsing i32 from {}", path.display()))
}

/// Resolves the IOMMU-group number for a PCI device.
///
/// `/sys/bus/pci/devices/<BDF>/iommu_group` is a symlink that ends in
/// `.../iommu_groups/<N>`.  Returns `None` when the device has no IOMMU group
/// (kernel built without IOMMU support, or device not yet mapped).
fn iommu_group_of(dev_path: &Path) -> Option<u32> {
    let link = dev_path.join("iommu_group");
    let target = std::fs::read_link(&link).ok()?;
    target
        .file_name()
        .and_then(|n| n.to_str())
        .and_then(|n| n.parse::<u32>().ok())
}

/// Derives the canonical BDF string (`DDDD:BB:SS.F`) from a sysfs device path.
fn bdf_of(dev_path: &Path) -> Option<String> {
    dev_path
        .file_name()
        .and_then(|n| n.to_str())
        .map(|s| s.to_string())
}

/// Maps a NUMA node index to a socket/package index.
///
/// On single-socket Grace systems all NUMA nodes belong to socket 0.
/// On dual-socket or multi-chip systems the mapping is stored in
/// `/sys/devices/system/node/nodeN/cpumap` but deriving the socket from
/// `/sys/bus/pci/devices/<BDF>/numa_node` is enough for our purposes:
/// we assign each *unique* NUMA node a sequential socket ID.
fn numa_node_to_socket(node: i32, socket_map: &mut HashMap<i32, u32>) -> u32 {
    let next_id = socket_map.len() as u32;
    *socket_map.entry(node).or_insert(next_id)
}

/// Probe the current host and return the NVIDIA device topology.
///
/// Reads `/sys/bus/pci/devices/` to discover all NVIDIA GPUs and NICs,
/// groups them by IOMMU group (one group = one SMMU on aarch64 Grace),
/// and builds a `HostTopology` suitable for `Platform::apply_host_defaults`.
///
/// Returns `Ok(topo)` with empty `gpu_smmu_groups` if no NVIDIA devices are
/// found (e.g., on a plain x86 CI runner).
pub(crate) fn probe_host_topology() -> Result<HostTopology> {
    probe_host_topology_at(
        Path::new("/sys/bus/pci/devices"),
        Path::new("/sys/devices/system/cpu"),
        Path::new("/dev"),
    )
}

/// Testable variant that accepts sysfs root paths.
pub(crate) fn probe_host_topology_at(
    pci_root: &Path,
    cpu_root: &Path,
    dev_root: &Path,
) -> Result<HostTopology> {
    // ── 1. Walk /sys/bus/pci/devices and collect NVIDIA devices ─────────────
    let mut gpu_groups: HashMap<u32, Vec<(String, i32)>> = HashMap::new(); // group_id → [(BDF, numa_node)]
    let mut nic_groups: HashMap<u32, Vec<(String, i32)>> = HashMap::new();

    let dir = std::fs::read_dir(pci_root)
        .with_context(|| format!("opening {}", pci_root.display()))?;

    for entry in dir.flatten() {
        let dev_path = entry.path();

        // vendor — skip non-NVIDIA
        let vendor_path = dev_path.join("vendor");
        let Ok(vendor) = read_sysfs_hex(&vendor_path) else {
            continue;
        };
        if vendor != NVIDIA_VENDOR_ID {
            continue;
        }

        let bdf = match bdf_of(&dev_path) {
            Some(b) => b,
            None => continue,
        };

        // class — top 16 bits only
        let class_path = dev_path.join("class");
        let class24 = match read_sysfs_hex(&class_path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let class16 = class24 >> 8;

        // numa_node — treat -1 (no affinity) as node 0
        let numa_node = read_sysfs_i32(&dev_path.join("numa_node")).unwrap_or(0);
        let numa_node = if numa_node < 0 { 0 } else { numa_node };

        let iommu_group = match iommu_group_of(&dev_path) {
            Some(g) => g,
            None => {
                // No IOMMU group — skip; device isn't available for passthrough.
                continue;
            }
        };

        match class16 {
            CLASS_3D_CONTROLLER | CLASS_VGA_CONTROLLER => {
                gpu_groups
                    .entry(iommu_group)
                    .or_default()
                    .push((bdf, numa_node));
            }
            CLASS_NETWORK_CONTROLLER | CLASS_INFINIBAND_CONTROLLER => {
                nic_groups
                    .entry(iommu_group)
                    .or_default()
                    .push((bdf, numa_node));
            }
            _ => {} // NVSwitch, Audio, etc. — not handled at this level
        }
    }

    // ── 2. Convert raw groups → GpuSmmuGroup, sorted for deterministic output ──
    let mut socket_map: HashMap<i32, u32> = HashMap::new();

    let mut gpu_smmu_groups: Vec<(u32 /* group_id */, GpuSmmuGroup)> = gpu_groups
        .into_iter()
        .map(|(group_id, mut devs)| {
            devs.sort_by(|a, b| a.0.cmp(&b.0)); // sort BDFs
            let socket = numa_node_to_socket(devs[0].1, &mut socket_map);
            (
                group_id,
                GpuSmmuGroup {
                    pci_bus_addrs: devs.into_iter().map(|(bdf, _)| bdf).collect(),
                    socket,
                },
            )
        })
        .collect();
    gpu_smmu_groups.sort_by_key(|(gid, _)| *gid);

    let mut nic_smmu_groups: Vec<(u32, GpuSmmuGroup)> = nic_groups
        .into_iter()
        .map(|(group_id, mut devs)| {
            devs.sort_by(|a, b| a.0.cmp(&b.0));
            let socket = numa_node_to_socket(devs[0].1, &mut socket_map);
            (
                group_id,
                GpuSmmuGroup {
                    pci_bus_addrs: devs.into_iter().map(|(bdf, _)| bdf).collect(),
                    socket,
                },
            )
        })
        .collect();
    nic_smmu_groups.sort_by_key(|(gid, _)| *gid);

    // ── 3. Build SocketInfo list ─────────────────────────────────────────────
    // Derive CPU ranges from /sys/devices/system/cpu/cpuN/topology/physical_package_id
    // Fall back to a single socket covering all online CPUs when unavailable.
    let sockets = build_socket_info(cpu_root, &socket_map);

    // ── 4. EGM detection: /dev/egmN devices ─────────────────────────────────
    let egm_sockets = probe_egm_devices(dev_root);

    Ok(HostTopology {
        sockets,
        gpu_smmu_groups: gpu_smmu_groups.into_iter().map(|(_, g)| g).collect(),
        nic_smmu_groups: nic_smmu_groups.into_iter().map(|(_, g)| g).collect(),
        egm_sockets,
        numa_distances: vec![],
        pcie_root_port: 0,
        protection: None,
    })
}

/// Reads `/sys/devices/system/cpu/` to build SocketInfo per physical package.
///
/// Each unique `physical_package_id` becomes a socket.  If the topology files
/// are unavailable we fall back to a single socket with an empty CPU range.
fn build_socket_info(cpu_root: &Path, socket_map: &HashMap<i32, u32>) -> Vec<SocketInfo> {
    // package_id → sorted list of CPU indices
    let mut packages: HashMap<u32, Vec<u32>> = HashMap::new();

    if let Ok(dir) = std::fs::read_dir(cpu_root) {
        for entry in dir.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            // Only look at cpuN directories (skip cpufreq, cpuidle, etc.)
            if !name_str.starts_with("cpu")
                || !name_str[3..].chars().all(|c| c.is_ascii_digit())
            {
                continue;
            }
            let cpu_idx: u32 = match name_str[3..].parse() {
                Ok(n) => n,
                Err(_) => continue,
            };
            let pkg_path = entry.path().join("topology/physical_package_id");
            let pkg_id: u32 = match read_sysfs_hex(&pkg_path)
                .or_else(|_| read_sysfs_i32(&pkg_path).map(|v| v as u32))
            {
                Ok(v) => v,
                Err(_) => 0,
            };
            packages.entry(pkg_id).or_default().push(cpu_idx);
        }
    }

    if packages.is_empty() {
        // Fallback: single socket, unknown CPU range
        return vec![SocketInfo {
            id: 0,
            cpu_range: 0..1,
            host_node: None,
            mem_path: None,
            mem_size: None,
        }];
    }

    let mut infos: Vec<SocketInfo> = packages
        .into_iter()
        .map(|(pkg_id, mut cpus)| {
            cpus.sort_unstable();
            let first = *cpus.first().unwrap();
            let last = *cpus.last().unwrap();
            // Find the NUMA node for this package using the inverse socket_map
            let host_node = socket_map
                .iter()
                .find(|(_, &sid)| sid == pkg_id)
                .map(|(&node, _)| node as u32);
            SocketInfo {
                id: pkg_id,
                cpu_range: first..(last + 1),
                host_node,
                mem_path: None,
                mem_size: None,
            }
        })
        .collect();
    infos.sort_by_key(|s| s.id);
    infos
}

/// Discovers EGM backing devices under `/dev/egmN`.
///
/// EGM size is read from `/sys/class/misc/egmN/size` (bytes).
/// The NUMA node of the underlying PCIe device maps to a socket via
/// `/sys/class/misc/egmN/device/numa_node`.
fn probe_egm_devices(dev_root: &Path) -> Vec<EgmSocketInfo> {
    let mut result = Vec::new();

    let dir = match std::fs::read_dir(dev_root) {
        Ok(d) => d,
        Err(_) => return result,
    };

    for entry in dir.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if !name_str.starts_with("egm") || !name_str[3..].chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        let egm_idx: u32 = match name_str[3..].parse() {
            Ok(n) => n,
            Err(_) => continue,
        };

        let path = dev_root.join(name_str.as_ref());
        // Size from /sys/class/misc/egmN/size
        let sys_misc = PathBuf::from(format!("/sys/class/misc/egm{egm_idx}"));
        let size_bytes: u64 = read_sysfs_hex(&sys_misc.join("size"))
            .map(|v| v as u64)
            .unwrap_or(0);

        // NUMA node from /sys/class/misc/egmN/device/numa_node
        let numa_node = read_sysfs_i32(&sys_misc.join("device/numa_node")).unwrap_or(0);
        let socket = if numa_node < 0 { 0 } else { numa_node as u32 };

        result.push(EgmSocketInfo {
            path: path.to_string_lossy().into_owned(),
            socket,
            total_size: size_bytes,
        });
    }

    result.sort_by_key(|e| e.socket);
    result
}
