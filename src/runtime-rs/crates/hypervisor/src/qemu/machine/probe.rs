// Copyright (c) NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0

use std::collections::{BTreeMap, HashMap};
use std::convert::TryFrom;
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

/// Equal share of `total` for the `i`-th of `n` consumers, rounded down to a
/// whole MiB except for the last one, which takes the remainder so the shares
/// add up to `total` exactly (QEMU requires memdev sizes to sum to `-m`).
pub(crate) fn equal_share(total: u64, n: usize, i: usize) -> u64 {
    const MIB: u64 = 1 << 20;
    if n == 0 {
        return total;
    }
    let share = total / n as u64 / MIB * MIB;
    if i + 1 == n {
        total - share * (n as u64 - 1)
    } else {
        share
    }
}

impl HostTopology {
    /// Replace the host CPU indices the prober recorded with guest vCPU ranges:
    /// `max_vcpus` guest CPUs laid out contiguously over the sockets in socket
    /// order, earlier sockets taking the remainder.  `-numa node,cpus=` names
    /// guest CPUs, and every possible vCPU (up to maxcpus) needs a node so that
    /// hot-plugged CPUs have somewhere to land.
    pub(crate) fn map_guest_vcpus(&mut self, max_vcpus: u32) {
        let n = self.sockets.len() as u32;
        if n == 0 {
            return;
        }
        let (base, rem) = (max_vcpus / n, max_vcpus % n);
        let mut start = 0u32;
        for (i, socket) in self.sockets.iter_mut().enumerate() {
            let count = base + u32::from((i as u32) < rem);
            socket.cpu_range = start..start + count;
            start += count;
        }
    }

    /// Give every socket without an explicit `mem_size` an equal share of the
    /// guest RAM that the explicitly sized sockets leave over.
    pub(crate) fn fill_guest_memory(&mut self, total_bytes: u64) {
        let claimed: u64 = self.sockets.iter().filter_map(|s| s.mem_size).sum();
        let open: Vec<usize> = self
            .sockets
            .iter()
            .enumerate()
            .filter(|(_, s)| s.mem_size.is_none())
            .map(|(i, _)| i)
            .collect();
        let remaining = total_bytes.saturating_sub(claimed);
        for (k, idx) in open.iter().enumerate() {
            self.sockets[*idx].mem_size = Some(equal_share(remaining, open.len(), k));
        }
    }

    /// Keep only the passthrough devices named in `keep` (BDFs, compared
    /// case-insensitively) and drop groups that end up empty.  The prober sees
    /// every NVIDIA device on the host; a sandbox only gets the ones its pod
    /// was allocated.
    pub(crate) fn retain_devices(&mut self, keep: &[String]) {
        let keep: Vec<String> = keep.iter().map(|b| b.to_ascii_lowercase()).collect();
        for groups in [&mut self.gpu_smmu_groups, &mut self.nic_smmu_groups] {
            for group in groups.iter_mut() {
                group
                    .pci_bus_addrs
                    .retain(|addr| keep.contains(&addr.to_ascii_lowercase()));
            }
            groups.retain(|group| !group.pci_bus_addrs.is_empty());
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
    let raw =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let trimmed = raw.trim().trim_start_matches("0x");
    u32::from_str_radix(trimmed, 16).with_context(|| format!("parsing hex from {}", path.display()))
}

fn read_sysfs_i32(path: &Path) -> Result<i32> {
    let raw =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
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

/// Name of the physical IOMMU a PCI device sits behind, from the
/// `/sys/bus/pci/devices/<BDF>/iommu` symlink (e.g. `smmu3.0x0000000005000000`
/// on Grace, `dmar0` on Intel).  `None` when the kernel exposes no such link.
fn iommu_unit_of(dev_path: &Path) -> Option<String> {
    let target = std::fs::read_link(dev_path.join("iommu")).ok()?;
    target
        .file_name()
        .and_then(|n| n.to_str())
        .map(str::to_owned)
}

/// Derives the canonical BDF string (`DDDD:BB:SS.F`) from a sysfs device path.
fn bdf_of(dev_path: &Path) -> Option<String> {
    dev_path
        .file_name()
        .and_then(|n| n.to_str())
        .map(|s| s.to_string())
}

/// Resolve PCI affinity using the CPU's node links, not its package ID.
/// Package IDs are opaque (and large on Grace), not host NUMA node numbers.
fn numa_node_to_socket(node: i32, sockets: &[SocketInfo]) -> Result<u32> {
    if let Some(socket) = sockets.iter().find(|s| s.host_node == Some(node as u32)) {
        return Ok(socket.id);
    }
    if sockets.len() == 1 && sockets[0].host_node.is_none() {
        return Ok(sockets[0].id);
    }
    anyhow::bail!("PCI NUMA node {node} has no matching CPU NUMA node")
}

/// Probe the current host and return the NVIDIA device topology.
///
/// Reads `/sys/bus/pci/devices/` to discover all NVIDIA GPUs and NICs,
/// groups them by the physical SMMU each device sits behind (the `iommu`
/// symlink; the IOMMU group is the fallback when the kernel exposes no such
/// link), and builds a `HostTopology` suitable for
/// `Platform::apply_host_defaults`.  Devices behind one host SMMU must share
/// one `arm-smmuv3` in the guest, and an IOMMU group is an isolation boundary
/// rather than a translation unit: two GPUs can sit in separate groups behind
/// the same SMMU.
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
    // complex key → [(BDF, numa_node)]; the key is the physical SMMU when the
    // kernel exposes it, otherwise the IOMMU group
    let mut gpu_groups: HashMap<String, Vec<(String, i32)>> = HashMap::new();
    let mut nic_groups: HashMap<String, Vec<(String, i32)>> = HashMap::new();

    let dir =
        std::fs::read_dir(pci_root).with_context(|| format!("opening {}", pci_root.display()))?;

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
        let complex_key = iommu_unit_of(&dev_path).unwrap_or_else(|| format!("group{iommu_group}"));

        match class16 {
            CLASS_3D_CONTROLLER | CLASS_VGA_CONTROLLER => {
                gpu_groups
                    .entry(complex_key)
                    .or_default()
                    .push((bdf, numa_node));
            }
            CLASS_NETWORK_CONTROLLER | CLASS_INFINIBAND_CONTROLLER => {
                nic_groups
                    .entry(complex_key)
                    .or_default()
                    .push((bdf, numa_node));
            }
            _ => {} // NVSwitch, Audio, etc. — not handled at this level
        }
    }

    // ── 2. Convert raw groups → GpuSmmuGroup, sorted for deterministic output ──
    let sockets = build_socket_info(cpu_root);

    let mut gpu_smmu_groups: Vec<(String /* complex key */, GpuSmmuGroup)> = gpu_groups
        .into_iter()
        .map(|(group_id, mut devs)| {
            devs.sort_by(|a, b| a.0.cmp(&b.0)); // sort BDFs
            let socket = numa_node_to_socket(devs[0].1, &sockets)?;
            Ok((
                group_id,
                GpuSmmuGroup {
                    pci_bus_addrs: devs.into_iter().map(|(bdf, _)| bdf).collect(),
                    socket,
                },
            ))
        })
        .collect::<Result<_>>()?;
    // Order complexes by their first BDF, not by IOMMU group id: BDFs are
    // stable across boots, group ids follow enumeration order.
    gpu_smmu_groups.sort_by(|a, b| a.1.pci_bus_addrs[0].cmp(&b.1.pci_bus_addrs[0]));

    let mut nic_smmu_groups: Vec<(String, GpuSmmuGroup)> = nic_groups
        .into_iter()
        .map(|(group_id, mut devs)| {
            devs.sort_by(|a, b| a.0.cmp(&b.0));
            let socket = numa_node_to_socket(devs[0].1, &sockets)?;
            Ok((
                group_id,
                GpuSmmuGroup {
                    pci_bus_addrs: devs.into_iter().map(|(bdf, _)| bdf).collect(),
                    socket,
                },
            ))
        })
        .collect::<Result<_>>()?;
    nic_smmu_groups.sort_by(|a, b| a.1.pci_bus_addrs[0].cmp(&b.1.pci_bus_addrs[0]));

    // ── 3. EGM detection: /dev/egmN devices ─────────────────────────────────
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
/// Keep host NUMA affinity from cpuN/nodeN separately from the decimal package
/// ID. A package spanning multiple host nodes gets one memory domain per node.
/// Sorted keys make guest socket IDs independent of sysfs/HashMap iteration.
fn build_socket_info(cpu_root: &Path) -> Vec<SocketInfo> {
    let mut packages: BTreeMap<(u32, Option<u32>), Vec<u32>> = BTreeMap::new();

    if let Ok(dir) = std::fs::read_dir(cpu_root) {
        for entry in dir.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            // Only look at cpuN directories (skip cpufreq, cpuidle, etc.)
            if !name_str.starts_with("cpu") || !name_str[3..].chars().all(|c| c.is_ascii_digit()) {
                continue;
            }
            let cpu_idx: u32 = match name_str[3..].parse() {
                Ok(n) => n,
                Err(_) => continue,
            };
            let pkg_path = entry.path().join("topology/physical_package_id");
            let pkg_id = read_sysfs_i32(&pkg_path)
                .ok()
                .and_then(|id| u32::try_from(id).ok())
                .unwrap_or_default();
            let host_node = std::fs::read_dir(entry.path()).ok().and_then(|entries| {
                entries.flatten().find_map(|entry| {
                    entry
                        .file_name()
                        .to_str()?
                        .strip_prefix("node")?
                        .parse::<u32>()
                        .ok()
                })
            });
            packages
                .entry((pkg_id, host_node))
                .or_default()
                .push(cpu_idx);
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

    packages
        .into_iter()
        .enumerate()
        .map(|(id, ((_, host_node), mut cpus))| {
            cpus.sort_unstable();
            let first = *cpus.first().unwrap();
            let last = *cpus.last().unwrap();
            SocketInfo {
                id: id as u32,
                cpu_range: first..(last + 1),
                host_node,
                mem_path: None,
                mem_size: None,
            }
        })
        .collect()
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
