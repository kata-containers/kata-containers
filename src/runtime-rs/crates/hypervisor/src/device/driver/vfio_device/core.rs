// Copyright (c) 2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{anyhow, Context, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::path::{Path, PathBuf};

/// Path constants for VFIO and IOMMU sysfs/dev interfaces
const DEV_VFIO: &str = "/dev/vfio";
const SYS_IOMMU_GROUPS: &str = "/sys/kernel/iommu_groups";
const SYS_PCI_DEVS: &str = "/sys/bus/pci/devices";
const DEV_IOMMU: &str = "/dev/iommu";
const DEV_VFIO_DEVICES: &str = "/dev/vfio/devices";
const SYS_CLASS_VFIO_DEV: &str = "/sys/class/vfio-dev";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VfioIommufdBackend {
    /// Host global IOMMUFD device node (/dev/iommu)
    pub iommufd_dev: PathBuf,
    /// The per-device VFIO cdev nodes required for this assignment
    pub cdevs: Vec<VfioCdev>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct VfioDevice {
    pub id: String,
    pub device_type: VfioDeviceType,
    pub bus_mode: VfioBusMode,

    /// Metadata for Legacy VFIO backend
    pub iommu_group: Option<VfioGroup>,
    pub iommu_group_id: Option<u32>,

    /// Metadata for IOMMUFD backend
    pub iommufd: Option<VfioIommufdBackend>,

    /// Common device information
    pub devices: Vec<DeviceInfo>,
    /// The representative primary device for this assignment unit
    pub primary: DeviceInfo,
    pub labels: BTreeMap<String, String>,
    pub health: Health,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum VfioDeviceType {
    #[default]
    Normal,
    MediatedPci,
    MediatedAp,
    Error,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum VfioBusMode {
    #[default]
    Mmio,
    Pci,
    Ccw,
}

/// PCI Bus-Device-Function (BDF) Address representation
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BdfAddress {
    pub domain: u16,
    pub bus: u8,
    pub device: u8,
    pub function: u8,
}

impl BdfAddress {
    pub fn new(domain: u16, bus: u8, device: u8, function: u8) -> Self {
        Self {
            domain,
            bus,
            device,
            function,
        }
    }

    /// Parses a BDF string in formats like "0000:01:00.0" or "01:00.0"
    pub fn parse(s: &str) -> Result<Self> {
        let parts: Vec<&str> = s.split(':').collect();

        let (domain, bus_str, bus_dev_func) = match parts.len() {
            2 => (0u16, parts[0], parts[1]),
            3 => {
                let domain = u16::from_str_radix(parts[0], 16).context("Invalid domain hex")?;
                (domain, parts[1], parts[2])
            }
            _ => return Err(anyhow!("Invalid BDF format: {}", s)),
        };

        let bus = u8::from_str_radix(bus_str, 16).context("Invalid bus hex")?;

        let dev_func: Vec<&str> = bus_dev_func.split('.').collect();
        if dev_func.len() != 2 {
            return Err(anyhow!("Invalid device.function format"));
        }

        let device = u8::from_str_radix(dev_func[0], 16).context("Invalid device hex")?;
        let function = u8::from_str_radix(dev_func[1], 16).context("Invalid function hex")?;

        Ok(Self {
            domain,
            bus,
            device,
            function,
        })
    }

    pub fn to_short_string(&self) -> String {
        format!("{:02x}:{:02x}.{:x}", self.bus, self.device, self.function)
    }
}

impl std::fmt::Display for BdfAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:04x}:{:02x}:{:02x}.{:x}",
            self.domain, self.bus, self.device, self.function
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DeviceAddress {
    Pci(BdfAddress),
    Ccw(String),
    Mmio(String),
    MdevUuid(String),
}

impl std::fmt::Display for DeviceAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeviceAddress::Pci(bdf) => write!(f, "{bdf}"),
            DeviceAddress::Ccw(s) => write!(f, "{s}"),
            DeviceAddress::Mmio(s) => write!(f, "{s}"),
            DeviceAddress::MdevUuid(s) => write!(f, "{s}"),
        }
    }
}

impl Default for DeviceAddress {
    fn default() -> Self {
        DeviceAddress::Pci(BdfAddress::default())
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    /// Logical address on the specific bus
    pub addr: DeviceAddress,

    /// Hardware identification (may be missing for non-PCI/mdev)
    pub vendor_id: Option<String>,
    pub device_id: Option<String>,
    pub class_code: Option<u32>,

    /// Active kernel driver (e.g., "vfio-pci")
    pub driver: Option<String>,

    /// Parent IOMMU group (critical for legacy passthrough)
    pub iommu_group_id: Option<u32>,

    /// Proximity to CPU/Memory (sysfs reports -1 for no specific node)
    pub numa_node: Option<i32>,

    /// Canonical path in sysfs
    pub sysfs_path: PathBuf,

    /// VFIO character device node (e.g., /dev/vfio/devices/vfio0)
    /// Only populated if the kernel/hardware supports device-centric VFIO
    pub vfio_cdev: Option<VfioCdev>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Health {
    #[default]
    Healthy,
    Unhealthy,
    Unknown,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct VfioGroup {
    pub group_id: u32,
    pub devnode: PathBuf,
    pub vfio_ctl: PathBuf,
    /// Aggregated VFIO cdev nodes for all devices within this group
    pub vfio_cdevs: Vec<PathBuf>,
    pub devices: Vec<DeviceInfo>,
    // primary device used for labeling and identification
    pub primary: DeviceInfo,
    pub labels: BTreeMap<String, String>,
    pub is_viable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VfioCdev {
    /// Instance name (e.g., "vfio0")
    pub name: String,
    /// Device node path (/dev/vfio/devices/vfio0)
    pub devnode: PathBuf,
    /// Character device major number
    pub major: Option<u32>,
    /// Character device minor number
    pub minor: Option<u32>,
    pub sysfs_path: PathBuf,
    /// Associated PCI BDF if applicable
    pub bdf: Option<String>,
    pub group_id: Option<u32>,
}

fn read_trim(path: impl AsRef<Path>) -> Option<String> {
    fs::read_to_string(path.as_ref())
        .ok()
        .map(|s| s.trim().to_string())
}

fn parse_i32(path: impl AsRef<Path>) -> Option<i32> {
    read_trim(path).and_then(|s| s.parse::<i32>().ok())
}

fn driver_name(pci_dev_path: &Path) -> Option<String> {
    let link = fs::read_link(pci_dev_path.join("driver")).ok()?;
    link.file_name().map(|n| n.to_string_lossy().to_string())
}

fn parse_bdf_str(s: &str) -> Result<BdfAddress> {
    // Standard format: "0000:65:00.0"
    let re = Regex::new(
        r"^(?P<d>[0-9a-fA-F]{4}):(?P<b>[0-9a-fA-F]{2}):(?P<dev>[0-9a-fA-F]{2})\.(?P<f>[0-7])$",
    )
    .unwrap();
    let cap = re
        .captures(s)
        .ok_or_else(|| anyhow!("invalid BDF format: {s}"))?;
    Ok(BdfAddress {
        domain: u16::from_str_radix(&cap["d"], 16)?,
        bus: u8::from_str_radix(&cap["b"], 16)?,
        device: u8::from_str_radix(&cap["dev"], 16)?,
        function: (cap["f"]).parse::<u8>()?,
    })
}

/// Scans sysfs to find all PCI devices belonging to a specific IOMMU group
fn discover_group_devices(group_id: u32) -> Result<Vec<DeviceInfo>> {
    let mut out = vec![];
    let group_dir = Path::new(SYS_IOMMU_GROUPS)
        .join(group_id.to_string())
        .join("devices");

    for ent in
        fs::read_dir(&group_dir).context(format!("Failed to read {}", group_dir.display()))?
    {
        let ent = ent?;
        let bdf_str = ent.file_name().to_string_lossy().to_string();
        let pci_path = Path::new(SYS_PCI_DEVS).join(&bdf_str);

        if !pci_path.exists() {
            continue;
        }

        let bdf = parse_bdf_str(&bdf_str)?;
        let vendor_id = read_trim(pci_path.join("vendor"));
        let device_id = read_trim(pci_path.join("device"));
        let class_code = read_trim(pci_path.join("class"))
            .as_deref()
            .and_then(parse_class_code_u32);
        let driver = driver_name(&pci_path);

        let numa_node =
            parse_i32(pci_path.join("numa_node")).and_then(|n| if n < 0 { None } else { Some(n) });

        out.push(DeviceInfo {
            addr: DeviceAddress::Pci(bdf),
            vendor_id,
            device_id,
            class_code,
            driver,
            iommu_group_id: Some(group_id),
            numa_node,
            sysfs_path: pci_path,
            vfio_cdev: None, // Populated later
        });
    }

    // Ensure deterministic ordering
    out.sort_by(|a, b| a.sysfs_path.cmp(&b.sysfs_path));
    Ok(out)
}

/// Generates descriptive labels for an IOMMU group (e.g., identifying GPUs)
fn build_group_labels(devs: &[DeviceInfo]) -> BTreeMap<String, String> {
    let mut labels = BTreeMap::new();
    let mut gpu = false;
    let mut vendor: Option<String> = None;

    for d in devs {
        if vendor.is_none() {
            vendor = d.vendor_id.clone();
        }

        // PCI Class Code layout: 0xBBSSPP (Base Class, Sub Class, Programming Interface)
        if let Some(class_code) = d.class_code {
            let base = ((class_code >> 16) & 0xff) as u8;
            let sub = ((class_code >> 8) & 0xff) as u8;

            // Base 0x03 = Display controller
            // Sub 0x00 = VGA compatible, 0x02 = 3D controller (NVIDIA/AMD)
            if base == 0x03 && (sub == 0x00 || sub == 0x02) {
                gpu = true;
            }
        }
    }

    if let Some(v) = vendor {
        labels.insert("vendor".into(), v);
    }
    labels.insert("gpu".into(), gpu.to_string());
    labels
}

/// Validates that an IOMMU group can be safely passed through.
/// Note: Bridges and Host Controllers in the group are ignored as they cannot be passed to guests.
fn validate_group_basic(devices: &[DeviceInfo]) -> bool {
    // Current minimal check: group must not be empty.
    // Production logic may include blacklisting specific device classes.
    for device in devices.iter() {
        if let DeviceAddress::Pci(bdf) = &device.addr {
            // filter host or PCI bridge
            let bdf_str = bdf.to_string();
            // Filter out devices that cannot be passed through (bridges, audio, etc.)
            if filter_bridge_device(&bdf_str, IOMMU_IGNORE).is_some() {
                continue;
            }
        }
    }

    !devices.is_empty()
}

fn get_device_property(device_bdf: &str, property: &str) -> Result<String> {
    let dev_sys_path = Path::new(SYS_PCI_DEVS).join(device_bdf);
    let cfg_path = fs::read_to_string(dev_sys_path.join(property)).with_context(|| {
        format!(
            "failed to read property {} for device {}",
            property, device_bdf
        )
    })?;

    Ok(cfg_path.trim().to_string())
}

/// PCI class bitmasks for devices that must be ignored when enumerating an IOMMU group.
/// Host Bridge: 0x0600, Audio device: 0x0403.
const IOMMU_IGNORE: &[u64] = &[0x0600, 0x403];

/// Filters for devices that cannot or should not be passed through within an IOMMU group
/// (Host/PCI bridges, audio controllers that share the GPU's IOMMU group, etc.).
fn filter_bridge_device(bdf: &str, bitmasks: &[u64]) -> Option<u64> {
    let device_class = get_device_property(bdf, "class").unwrap_or_default();

    if device_class.is_empty() {
        return None;
    }

    match device_class.parse::<u32>() {
        Ok(cid_u32) => {
            // PCI class code is 24 bits, shift right 8 to get base+sub class
            let class_code = u64::from(cid_u32) >> 8;
            for &bitmask in bitmasks {
                if class_code & bitmask == bitmask {
                    return Some(class_code);
                }
            }
            None
        }
        _ => None,
    }
}

fn parse_class_code_u32(s: &str) -> Option<u32> {
    let t = s.trim().strip_prefix("0x").unwrap_or(s.trim());
    u32::from_str_radix(t, 16).ok()
}

/// Determines device priority for selection as the 'Primary' device of a group.
/// GPUs take precedence, followed by Network and Storage controllers.
fn class_priority(class_code: Option<u32>) -> u8 {
    let Some(c) = class_code else { return 255 };
    let base = ((c >> 16) & 0xff) as u8;
    let sub = ((c >> 8) & 0xff) as u8;

    match (base, sub) {
        (0x03, 0x00) | (0x03, 0x02) => 0, // VGA/3D GPU
        (0x02, _) => 10,                  // Network controller
        (0x01, _) => 20,                  // Mass storage
        _ => 100,                         // Other
    }
}

/// Picks the most significant device in a group to act as the primary identifier.
fn select_primary_device(devs: &[DeviceInfo]) -> DeviceInfo {
    assert!(!devs.is_empty());

    devs.iter()
        .min_by(|a, b| {
            let pa = class_priority(a.class_code);
            let pb = class_priority(b.class_code);
            if pa != pb {
                return pa.cmp(&pb);
            }

            // Fallback to function number if classes are identical
            let fa = match &a.addr {
                DeviceAddress::Pci(bdf) => bdf.function,
                _ => u8::MAX,
            };
            let fb = match &b.addr {
                DeviceAddress::Pci(bdf) => bdf.function,
                _ => u8::MAX,
            };
            fa.cmp(&fb)
        })
        .cloned()
        .unwrap()
}

fn is_char_dev(p: &Path) -> bool {
    fs::metadata(p)
        .map(|m| m.file_type().is_char_device())
        .unwrap_or(false)
}

/// Extracts the IOMMU group ID from a PCI device's sysfs link.
fn vfio_group_id_from_pci(bdf: &str) -> Option<u32> {
    let link = fs::read_link(Path::new(SYS_PCI_DEVS).join(bdf).join("iommu_group")).ok()?;
    link.file_name()?.to_string_lossy().parse::<u32>().ok()
}

/// Locates the VFIO character device (cdev) for a given PCI BDF.
/// Path: /sys/bus/pci/devices/<bdf>/vfio-dev/vfioX
fn discover_vfio_cdev_for_pci(bdf: &str, gid: u32) -> Option<VfioCdev> {
    let pci_path = Path::new(SYS_PCI_DEVS).join(bdf);
    let vfio_dev_dir = pci_path.join("vfio-dev");
    let rd = fs::read_dir(&vfio_dev_dir).ok()?;
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        if !name.starts_with("vfio") {
            continue;
        }
        return discover_vfio_cdev_by_name(&name, Some(bdf.to_string()), Some(gid));
    }
    None
}

/// Extracts major/minor device numbers from a file's metadata.
fn stat_major_minor(path: &Path) -> Option<(u32, u32)> {
    let md = fs::metadata(path).ok()?;
    let rdev = md.rdev();
    Some((linux_major(rdev), linux_minor(rdev)))
}

fn discover_vfio_cdev_by_name(
    vfio_name: &str,
    bdf: Option<String>,
    gid: Option<u32>,
) -> Option<VfioCdev> {
    let devnode = Path::new(DEV_VFIO_DEVICES).join(vfio_name);
    if !is_char_dev(&devnode) {
        return None;
    }
    let (major, minor) = stat_major_minor(&devnode).unwrap_or((0, 0));
    Some(VfioCdev {
        name: vfio_name.to_string(),
        devnode,
        major: if major == 0 && minor == 0 {
            None
        } else {
            Some(major)
        },
        minor: if major == 0 && minor == 0 {
            None
        } else {
            Some(minor)
        },
        sysfs_path: Path::new(SYS_CLASS_VFIO_DEV).join(vfio_name),
        bdf,
        group_id: gid,
    })
}

/// Discovers the VFIO device context based on a /dev/vfio/devices/vfio<X> path.
pub fn discover_vfio_device(vfio_device: &Path) -> Result<VfioDevice> {
    if vfio_device.exists() && is_char_dev(vfio_device) {
        let vfio_name = vfio_device
            .file_name()
            .ok_or_else(|| anyhow!("Invalid vfio device path"))?
            .to_string_lossy()
            .to_string();

        // Resolve VFIO name to BDF via sysfs symlink
        let dev_link = fs::read_link(
            Path::new(SYS_CLASS_VFIO_DEV)
                .join(&vfio_name)
                .join("device"),
        )
        .with_context(|| format!("failed to read sysfs device link for {}", vfio_name))?;

        let bdf = dev_link
            .file_name()
            .ok_or_else(|| anyhow!("Malformed vfio-dev symlink for {}", vfio_name))?
            .to_string_lossy()
            .to_string();

        // Resolve BDF to IOMMU group. On iommufd-first hosts there is often no legacy
        // /dev/vfio/<gid> node — only /dev/vfio/devices/vfioX cdevs exist — so use the
        // cdev we were given as the group char dev when legacy is absent.
        let gid = vfio_group_id_from_pci(&bdf)
            .ok_or_else(|| anyhow!("could not resolve IOMMU group for {}", bdf))?;
        let legacy = Path::new(DEV_VFIO).join(gid.to_string());
        let group_devnode = if legacy.exists() && is_char_dev(&legacy) {
            legacy
        } else {
            vfio_device.to_path_buf()
        };
        discover_vfio_device_for_iommu_group(gid, group_devnode)
    } else {
        Err(anyhow!("vfio device {} not found", vfio_device.display()))
    }
}

fn parse_dev_vfio_group_id(s: &str) -> Option<u32> {
    // Extracts numeric ID from "/dev/vfio/12" or just "12"
    let base = Path::new(s).file_name()?.to_string_lossy();
    base.parse::<u32>().ok()
}

/// Per-device cdev under iommufd (`/dev/vfio/devices/vfioN`). Matches Go
/// `strings.HasPrefix(HostPath, IommufdDevPath)` in `pkg/device/drivers/vfio.go`, not only
/// [`Path::starts_with`]: component-wise path prefix can disagree with string prefix for some
/// `OsStr` forms, so we use the same string rule as the Go runtime.
fn is_iommufd_devices_cdev_path(path: &Path) -> bool {
    let s = path.to_string_lossy();
    if !s.starts_with(DEV_VFIO_DEVICES) {
        return false;
    }
    match s.as_bytes().get(DEV_VFIO_DEVICES.len()) {
        None => true,
        Some(b'/') => true,
        Some(_) => false,
    }
}

/// Main entry point: Discovers a VFIO device unit based on an IOMMU group path (/dev/vfio/<X>)
///
/// CDI / device plugins often pass the per-device cdev (`/dev/vfio/devices/vfioX`) as the only
/// host path; that is stored as `iommu_group_devnode` without setting `iommu_device_node`.
/// Treat those like [`discover_vfio_device`].
pub fn discover_vfio_group_device(host_path: PathBuf) -> Result<VfioDevice> {
    if is_iommufd_devices_cdev_path(&host_path) {
        return discover_vfio_device(&host_path);
    }
    let gid = parse_dev_vfio_group_id(&host_path.to_string_lossy())
        .ok_or_else(|| anyhow!("Invalid VFIO group path: {}", host_path.display()))?;
    discover_vfio_device_for_iommu_group(gid, host_path)
}

/// Builds [`VfioDevice`] for IOMMU group `gid`.
///
/// `group_devnode` is the char device used to represent the group for metadata/health:
/// typically `/dev/vfio/<gid>` (legacy) or `/dev/vfio/devices/vfioX` when legacy nodes are absent.
fn discover_vfio_device_for_iommu_group(gid: u32, group_devnode: PathBuf) -> Result<VfioDevice> {
    let vfio_ctl = Path::new(DEV_VFIO).join("vfio");
    if !vfio_ctl.exists() {
        return Err(anyhow!("VFIO control node missing: {}", vfio_ctl.display()));
    }

    let devnode = group_devnode;
    let mut devices = discover_group_devices(gid)?;
    if devices.is_empty() {
        return Err(anyhow!("IOMMU group {} contains no PCI devices", gid));
    }

    // Populate per-device VFIO cdevs (required for IOMMUFD backend)
    for d in devices.iter_mut() {
        if let DeviceAddress::Pci(bdf) = &d.addr {
            d.vfio_cdev = discover_vfio_cdev_for_pci(&bdf.to_string(), gid);
        }
    }

    let labels = build_group_labels(&devices);
    let is_viable = validate_group_basic(&devices);
    let primary_device = select_primary_device(&devices);

    let group = VfioGroup {
        group_id: gid,
        devnode: devnode.clone(),
        vfio_ctl: vfio_ctl.clone(),
        devices: devices.clone(),
        primary: primary_device.clone(),
        labels: labels.clone(),
        is_viable,
        vfio_cdevs: devices
            .iter()
            .filter_map(|d| d.vfio_cdev.as_ref().map(|c| c.devnode.clone()))
            .collect(),
    };

    // Construct IOMMUFD backend context (Best-effort discovery)
    let iommufd_backend = {
        let iommu_dev = PathBuf::from(DEV_IOMMU);
        if is_char_dev(&iommu_dev) {
            let mut cdevs: Vec<VfioCdev> =
                devices.iter().filter_map(|d| d.vfio_cdev.clone()).collect();
            cdevs.sort_by(|a, b| a.devnode.cmp(&b.devnode));
            cdevs.dedup_by(|a, b| a.devnode == b.devnode);
            if !cdevs.is_empty() {
                Some(VfioIommufdBackend {
                    iommufd_dev: iommu_dev,
                    cdevs,
                })
            } else {
                None
            }
        } else {
            None
        }
    };

    let health = if is_viable && devnode.exists() && is_char_dev(&devnode) {
        Health::Healthy
    } else {
        Health::Unhealthy
    };

    Ok(VfioDevice {
        id: format!("vfio-group-{}", gid),
        device_type: VfioDeviceType::Normal,
        bus_mode: VfioBusMode::Pci,
        iommu_group: Some(group),
        iommu_group_id: Some(gid),
        iommufd: iommufd_backend,
        devices,
        primary: primary_device,
        labels,
        health,
    })
}

/// Resolves an IOMMUFD-style VFIO device cdev (/dev/vfio/devices/vfioX)
/// back to its PCI BDF and IOMMU group ID.
#[allow(dead_code)]
pub fn vfio_cdev_to_bdf_and_group(vfio_cdev: impl AsRef<Path>) -> Result<(String, u32)> {
    let vfio_cdev = vfio_cdev.as_ref();

    let (major, minor) = major_minor_from_char_device(vfio_cdev).context(format!(
        "Failed to get major/minor for {}",
        vfio_cdev.display()
    ))?;

    // Map char device to its sysfs entry
    let sys_dev_char = PathBuf::from(format!("/sys/dev/char/{major}:{minor}"));
    let resolved = fs::canonicalize(&sys_dev_char)
        .context(format!("failed to canonicalize {}", sys_dev_char.display()))?;

    // Parse the sysfs path to find the associated PCI device
    let bdf = extract_last_pci_bdf(&resolved)
        .context(format!("no PCI BDF found in path {}", resolved.display()))?;

    // Get IOMMU group, with a fallback to manual path scanning if the symlink is missing
    let group_id = iommu_group_id_for_bdf(&bdf).or_else(|primary_err| {
        group_id_from_path(&resolved).map_err(|fallback_err| {
            anyhow!(
                "failed to resolve group for BDF {bdf}: {primary_err}; fallback scan also failed: {fallback_err}"
            )
        })
    })?;

    Ok((bdf, group_id))
}

/// Extract (major, minor) from a char device node.
/// Uses Linux's encoding macros (same logic as gnu libc major()/minor()).
fn major_minor_from_char_device(p: &Path) -> Result<(u32, u32)> {
    let md = fs::metadata(p).context(format!("stat failed for {}", p.display()))?;
    if !md.file_type().is_char_device() {
        return Err(anyhow!("{} is not a character device", p.display()));
    }

    let rdev = md.rdev();
    Ok((linux_major(rdev), linux_minor(rdev)))
}

/// Linux device number encoding (glibc-compatible).
#[inline]
fn linux_major(dev: u64) -> u32 {
    (((dev >> 8) & 0xfff) | ((dev >> 32) & 0xfffff000)) as u32
}

/// Linux device number encoding (glibc-compatible).
#[inline]
fn linux_minor(dev: u64) -> u32 {
    ((dev & 0xff) | ((dev >> 12) & 0xfffff00)) as u32
}

/// Extracts the final PCI BDF in a sysfs path string.
/// Handles nested bridge paths like: .../pci0000:00/0000:00:01.0/0000:01:00.0/vfio-dev/...
fn extract_last_pci_bdf(p: &Path) -> Result<String> {
    static RE: once_cell::sync::Lazy<Regex> = once_cell::sync::Lazy::new(|| {
        Regex::new(r"(?i)\b[0-9a-f]{4}:[0-9a-f]{2}:[0-9a-f]{2}\.[0-7]\b").unwrap()
    });

    let s = p.to_string_lossy();
    RE.find_iter(&s)
        .last()
        .map(|m| m.as_str().to_owned())
        .ok_or_else(|| anyhow!("no PCI BDF found in path: {}", s))
}

/// Resolve iommu group id from `/sys/bus/pci/devices/<BDF>/iommu_group`.
fn iommu_group_id_for_bdf(bdf: &str) -> Result<u32> {
    let iommu_link = PathBuf::from(format!("/sys/bus/pci/devices/{bdf}/iommu_group"));
    let target = fs::read_link(&iommu_link).context("failed to read iommu_group symlink")?;

    target
        .file_name()
        .ok_or_else(|| anyhow!("link target {} invalid", target.display()))?
        .to_string_lossy()
        .parse::<u32>()
        .context("failed to parse group ID from filename")
}

fn group_id_from_path(p: &Path) -> Result<u32> {
    static RE: once_cell::sync::Lazy<Regex> =
        once_cell::sync::Lazy::new(|| Regex::new(r"/iommu_groups/(\d+)(/|$)").unwrap());

    let s = p.to_string_lossy();
    let caps = RE
        .captures(&s)
        .ok_or_else(|| anyhow!("no iommu_groups component in path"))?;

    caps.get(1)
        .unwrap()
        .as_str()
        .parse::<u32>()
        .context("parse group id")
}

#[allow(dead_code)]
pub fn is_dev_vfio_group_path(host_path: &str) -> bool {
    let s = host_path.trim_end_matches('/');
    const PREFIX: &str = "/dev/vfio/";
    let rest = match s.strip_prefix(PREFIX) {
        Some(r) => r,
        None => return false,
    };

    // Valid if remainder is non-empty and contains only digits
    !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_digit())
}
