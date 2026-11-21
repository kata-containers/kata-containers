// Copyright (c) 2019 Ant Financial
// Copyright (c) 2024 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

#[cfg(target_arch = "s390x")]
use crate::ccw;
use crate::device::{
    pcipath_to_sysfs, DeviceContext, DeviceHandler, DeviceInfo, SpecUpdate, BLOCK,
};
#[cfg(target_arch = "s390x")]
use crate::linux_abi::CCW_ROOT_BUS_PATH;
use crate::linux_abi::{
    create_pci_root_bus_path, pcipath_from_dev_tree_path, SYSFS_DIR, SYSTEM_DEV_PATH,
};
use crate::pci;
use crate::sandbox::Sandbox;
use crate::uevent::{wait_for_uevent, Uevent, UeventMatcher};
use crate::AGENT_CONFIG;
use anyhow::{anyhow, Context, Result};
#[cfg(target_arch = "s390x")]
use std::str::FromStr;

#[cfg(target_arch = "s390x")]
use kata_types::device::DRIVER_BLK_CCW_TYPE;
use kata_types::device::{DRIVER_BLK_MMIO_TYPE, DRIVER_BLK_PCI_TYPE};
use protocols::agent::Device;
use regex::Regex;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::{sleep, Instant};
use tracing::instrument;

const MMIO_BLK_CAPACITY_POLL_INTERVAL: Duration = Duration::from_millis(5);

#[derive(Debug)]
pub struct VirtioBlkPciDeviceHandler {}

#[cfg(target_arch = "s390x")]
#[derive(Debug)]
pub struct VirtioBlkCcwDeviceHandler {}

#[derive(Debug)]
pub struct VirtioBlkMmioDeviceHandler {}

#[async_trait::async_trait]
impl DeviceHandler for VirtioBlkPciDeviceHandler {
    #[instrument]
    fn driver_types(&self) -> &[&str] {
        &[DRIVER_BLK_PCI_TYPE]
    }

    #[instrument]
    async fn device_handler(&self, device: &Device, ctx: &mut DeviceContext) -> Result<SpecUpdate> {
        let (root_complex, pcipath) = pcipath_from_dev_tree_path(&device.id)?;
        let vm_path = get_virtio_blk_pci_device_name(ctx.sandbox, root_complex, &pcipath).await?;

        Ok(DeviceInfo::new(&vm_path, true)
            .context("New device info")?
            .into())
    }
}

#[cfg(target_arch = "s390x")]
#[async_trait::async_trait]
impl DeviceHandler for VirtioBlkCcwDeviceHandler {
    #[instrument]
    fn driver_types(&self) -> &[&str] {
        &[DRIVER_BLK_CCW_TYPE]
    }

    #[cfg(target_arch = "s390x")]
    #[instrument]
    async fn device_handler(&self, device: &Device, ctx: &mut DeviceContext) -> Result<SpecUpdate> {
        let ccw_device = ccw::Device::from_str(&device.id)?;
        let vm_path = get_virtio_blk_ccw_device_name(ctx.sandbox, &ccw_device).await?;

        Ok(DeviceInfo::new(&vm_path, true)
            .context("New device info")?
            .into())
    }

    #[cfg(not(target_arch = "s390x"))]
    async fn device_handler(
        &self,
        _device: &Device,
        _ctx: &mut DeviceContext,
    ) -> Result<SpecUpdate> {
        Err(anyhow!("CCW is only supported on s390x"))
    }
}

#[async_trait::async_trait]
impl DeviceHandler for VirtioBlkMmioDeviceHandler {
    #[instrument]
    fn driver_types(&self) -> &[&str] {
        &[DRIVER_BLK_MMIO_TYPE]
    }

    #[instrument]
    async fn device_handler(&self, device: &Device, ctx: &mut DeviceContext) -> Result<SpecUpdate> {
        if device.vm_path.is_empty() {
            return Err(anyhow!("Invalid path for virtio mmio blk device"));
        }
        wait_for_virtio_blk_mmio_device(ctx.sandbox, &device.vm_path)
            .await
            .context("failed to wait for virtio-mmio block device")?;

        Ok(DeviceInfo::new(device.vm_path(), true)
            .context("New device info")?
            .into())
    }
}

#[instrument]
pub async fn get_virtio_blk_pci_device_name(
    sandbox: &Arc<Mutex<Sandbox>>,
    root_complex: &str,
    pcipath: &pci::Path,
) -> Result<String> {
    let root_bus_sysfs = format!("{}{}", SYSFS_DIR, create_pci_root_bus_path(root_complex));
    let sysfs_rel_path = pcipath_to_sysfs(&root_bus_sysfs, pcipath)?;
    let matcher = VirtioBlkPciMatcher::new(&sysfs_rel_path, root_complex);

    let uev = wait_for_uevent(sandbox, matcher).await?;
    Ok(format!("{}/{}", SYSTEM_DEV_PATH, uev.devname))
}

fn parse_mmio_devname(devpath: &str) -> Result<&str> {
    let devname = devpath
        .strip_prefix("/dev/")
        .ok_or_else(|| anyhow!("Storage source '{}' must start with /dev/", devpath))?;
    if devname.is_empty() || devname == "." || devname == ".." || devname.contains('/') {
        return Err(anyhow!(
            "Invalid virtio-mmio block device path '{}'",
            devpath
        ));
    }
    Ok(devname)
}

#[instrument]
pub async fn get_virtio_blk_mmio_device_name(
    sandbox: &Arc<Mutex<Sandbox>>,
    devpath: &str,
) -> Result<()> {
    let devname = parse_mmio_devname(devpath)?;

    let matcher = VirtioBlkMmioMatcher::new(devname);
    let uev = wait_for_uevent(sandbox, matcher)
        .await
        .context("failed to wait for uevent")?;
    if uev.devname != devname {
        return Err(anyhow!(
            "Unexpected device name {} for mmio device (expected {})",
            uev.devname,
            devname
        ));
    }
    Ok(())
}

/// Wait until a virtio-mmio block device node exists AND reports a non-zero
/// sector count in sysfs (`/sys/class/block/<devname>/size`).
///
/// Hypervisors such as Firecracker pre-attach zero-length placeholder drives
/// at boot (`/dev/vdb`..`/dev/vdi` with `0` sectors) and later swap in the
/// container backing image via `PATCH /drives/drive_N`, which triggers an
/// asynchronous `VIRTIO_MMIO_INT_CONFIG` interrupt in the guest. Because the
/// `/dev/vdX` node already exists and Linux `set_capacity_and_notify()` does
/// not emit a `change` uevent for `0 -> N` capacity transitions, checking path
/// existence alone races with the guest `virtio_blk` capacity update and causes
/// `mount(2)` to fail with `EIO` (`EXT4-fs: unable to read superblock`).
#[instrument]
pub async fn wait_for_virtio_blk_mmio_device(
    sandbox: &Arc<Mutex<Sandbox>>,
    devpath: &str,
) -> Result<()> {
    let devname = parse_mmio_devname(devpath)?;
    wait_for_virtio_blk_mmio_device_in(
        sandbox,
        Path::new(SYSTEM_DEV_PATH),
        Path::new(SYSFS_DIR),
        devname,
        AGENT_CONFIG.hotplug_timeout,
        MMIO_BLK_CAPACITY_POLL_INTERVAL,
    )
    .await
}

async fn wait_for_virtio_blk_mmio_device_in(
    sandbox: &Arc<Mutex<Sandbox>>,
    dev_root: &Path,
    sysfs_dir: &Path,
    devname: &str,
    timeout: Duration,
    poll_interval: Duration,
) -> Result<()> {
    let start = Instant::now();
    let dev_node = dev_root.join(devname);

    if !dev_node.exists() {
        let canonical_devpath = format!("/dev/{devname}");
        get_virtio_blk_mmio_device_name(sandbox, &canonical_devpath)
            .await
            .context("failed to get mmio device name")?;
    }

    let remaining_timeout = timeout.saturating_sub(start.elapsed());
    wait_for_non_zero_block_capacity(sysfs_dir, devname, remaining_timeout, poll_interval).await?;

    Ok(())
}

async fn wait_for_non_zero_block_capacity(
    sysfs_dir: &Path,
    devname: &str,
    timeout: Duration,
    poll_interval: Duration,
) -> Result<u64> {
    let size_path = sysfs_dir.join("class/block").join(devname).join("size");
    let deadline = Instant::now() + timeout;
    let mut saw_zero_capacity = false;

    loop {
        match fs::read_to_string(&size_path) {
            Ok(raw) => {
                let sectors = raw.trim().parse::<u64>().with_context(|| {
                    format!(
                        "failed to parse sector count '{}' from {}",
                        raw.trim(),
                        size_path.display()
                    )
                })?;
                if sectors > 0 {
                    return Ok(sectors);
                }
                saw_zero_capacity = true;
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => {
                return Err(err).with_context(|| {
                    format!("failed to read block capacity from {}", size_path.display())
                });
            }
        }

        let now = Instant::now();
        if now >= deadline {
            if saw_zero_capacity {
                return Err(anyhow!(
                    "Timeout after {:?} waiting for virtio-mmio block device /dev/{} to report non-zero capacity via {}",
                    timeout,
                    devname,
                    size_path.display()
                ));
            }
            return Err(anyhow!(
                "Timeout after {:?} waiting for block capacity sysfs attribute {}",
                timeout,
                size_path.display()
            ));
        }

        let remaining = deadline.saturating_duration_since(now);
        sleep(poll_interval.min(remaining)).await;
    }
}

#[cfg(target_arch = "s390x")]
#[instrument]
pub async fn get_virtio_blk_ccw_device_name(
    sandbox: &Arc<Mutex<Sandbox>>,
    device: &ccw::Device,
) -> Result<String> {
    let matcher = VirtioBlkCCWMatcher::new(CCW_ROOT_BUS_PATH, device);
    let uev = wait_for_uevent(sandbox, matcher).await?;
    let devname = uev.devname;
    Path::new(SYSTEM_DEV_PATH)
        .join(&devname)
        .to_str()
        .map(String::from)
        .ok_or_else(|| anyhow!("CCW device name {} is not valid UTF-8", devname))
}

#[derive(Debug)]
pub struct VirtioBlkPciMatcher {
    rex: Regex,
}

impl VirtioBlkPciMatcher {
    pub fn new(relpath: &str, root_complex: &str) -> VirtioBlkPciMatcher {
        let root_bus = create_pci_root_bus_path(root_complex);
        // [^/]+$ ensures it only match the whole-disk uevent (e.g. block/vdx)
        let re = format!(r"^{root_bus}{relpath}/virtio[0-9]+/block/[^/]+$");

        VirtioBlkPciMatcher {
            rex: Regex::new(&re).expect("BUG: failed to compile VirtioBlkPciMatcher regex"),
        }
    }
}

impl UeventMatcher for VirtioBlkPciMatcher {
    fn is_match(&self, uev: &Uevent) -> bool {
        uev.subsystem == BLOCK && self.rex.is_match(&uev.devpath) && !uev.devname.is_empty()
    }
}

#[derive(Debug)]
pub struct VirtioBlkMmioMatcher {
    suffix: String,
}

impl VirtioBlkMmioMatcher {
    pub fn new(devname: &str) -> VirtioBlkMmioMatcher {
        VirtioBlkMmioMatcher {
            suffix: format!(r"/block/{devname}"),
        }
    }
}

impl UeventMatcher for VirtioBlkMmioMatcher {
    fn is_match(&self, uev: &Uevent) -> bool {
        uev.subsystem == BLOCK && uev.devpath.ends_with(&self.suffix) && !uev.devname.is_empty()
    }
}

#[cfg(target_arch = "s390x")]
#[derive(Debug)]
pub struct VirtioBlkCCWMatcher {
    rex: Regex,
}

#[cfg(target_arch = "s390x")]
impl VirtioBlkCCWMatcher {
    pub fn new(root_bus_path: &str, device: &ccw::Device) -> Self {
        let re =
            format!(r"^{root_bus_path}/0\.[0-3]\.[0-9a-f]{{1,4}}/{device}/virtio[0-9]+/block/");
        VirtioBlkCCWMatcher {
            rex: Regex::new(&re).expect("BUG: failed to compile VirtioBlkCCWMatcher regex"),
        }
    }
}

#[cfg(target_arch = "s390x")]
impl UeventMatcher for VirtioBlkCCWMatcher {
    fn is_match(&self, uev: &Uevent) -> bool {
        uev.action == "add" && self.rex.is_match(&uev.devpath) && !uev.devname.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device::test_helpers;
    use rstest::rstest;
    #[cfg(target_arch = "s390x")]
    use std::str::FromStr;

    // Test constants
    const TEST_DEVNAME: &str = "vda";
    const TEST_PCI_RELPATH: &str = "/0000:00:0a.0";
    const TEST_ROOT_COMPLEX: &str = "00";

    // Helper to create a standard PCI uevent for testing
    fn create_pci_uevent(
        devname: &str,
        relpath: &str,
        root_complex: &str,
        virtio_id: u32,
    ) -> crate::uevent::Uevent {
        let root_bus = create_pci_root_bus_path(root_complex);
        let mut uev = crate::uevent::Uevent::default();
        uev.action = crate::linux_abi::U_EVENT_ACTION_ADD.to_string();
        uev.subsystem = BLOCK.to_string();
        uev.devname = devname.to_string();
        uev.devpath = format!("{root_bus}{relpath}/virtio{virtio_id}/block/{devname}");
        uev
    }

    #[rstest]
    #[case::matcher_a_matches_uev_a("/0000:00:0a.0", "/0000:00:0a.0", 4, true)]
    #[case::matcher_b_matches_uev_b(
        "/0000:00:0a.0/0000:00:0b.0",
        "/0000:00:0a.0/0000:00:0b.0",
        0,
        true
    )]
    #[case::matcher_a_rejects_uev_b("/0000:00:0a.0", "/0000:00:0a.0/0000:00:0b.0", 0, false)]
    #[case::matcher_b_rejects_uev_a("/0000:00:0a.0/0000:00:0b.0", "/0000:00:0a.0", 4, false)]
    #[tokio::test]
    async fn test_virtio_blk_pci_matcher_basic_matching(
        #[case] matcher_relpath: &str,
        #[case] uevent_relpath: &str,
        #[case] virtio_id: u32,
        #[case] should_match: bool,
    ) {
        let matcher = VirtioBlkPciMatcher::new(matcher_relpath, TEST_ROOT_COMPLEX);
        let uev = create_pci_uevent(TEST_DEVNAME, uevent_relpath, TEST_ROOT_COMPLEX, virtio_id);

        assert_eq!(
            matcher.is_match(&uev),
            should_match,
            "Matcher for '{}' should {} uevent for '{}'",
            matcher_relpath,
            if should_match { "match" } else { "reject" },
            uevent_relpath
        );
    }

    #[rstest]
    #[case::partition_vda1("vda1", "vda1")]
    #[case::partition_vda91("vda91", "vda91")]
    #[tokio::test]
    async fn test_virtio_blk_pci_matcher_rejects_partitions(
        #[case] partition_devname: &str,
        #[case] partition_suffix: &str,
    ) {
        let root_bus = create_pci_root_bus_path(TEST_ROOT_COMPLEX);
        let matcher = VirtioBlkPciMatcher::new(TEST_PCI_RELPATH, TEST_ROOT_COMPLEX);
        let mut uev = create_pci_uevent(TEST_DEVNAME, TEST_PCI_RELPATH, TEST_ROOT_COMPLEX, 4);
        uev.devname = partition_devname.to_string();
        uev.devpath = format!(
            "{root_bus}{}/virtio4/block/{}/{}",
            TEST_PCI_RELPATH, TEST_DEVNAME, partition_suffix
        );

        assert!(
            !matcher.is_match(&uev),
            "Matcher should reject partition uevent for '{}'",
            partition_devname
        );
    }

    #[cfg(target_arch = "s390x")]
    #[tokio::test]
    async fn test_virtio_blk_ccw_matcher_valid_path() {
        let root_bus = CCW_ROOT_BUS_PATH;
        let subsystem = "block";
        let devname = "vda";
        let relpath = "0.0.0002";

        let mut uev = crate::uevent::Uevent::default();
        uev.action = crate::linux_abi::U_EVENT_ACTION_ADD.to_string();
        uev.subsystem = subsystem.to_string();
        uev.devname = devname.to_string();
        uev.devpath = format!("{root_bus}/0.0.0001/{relpath}/virtio1/{subsystem}/{devname}");

        let device = ccw::Device::from_str(relpath).unwrap();
        let matcher = VirtioBlkCCWMatcher::new(root_bus, &device);

        assert!(
            matcher.is_match(&uev),
            "Matcher should match valid CCW device path"
        );
    }

    #[cfg(target_arch = "s390x")]
    #[rstest]
    #[case::wrong_device_id(
        "/devices/css0/0.0.0001/0.0.0003/virtio1/block/vda",
        "Wrong device ID should be rejected"
    )]
    #[case::missing_root_bus(
        "0.0.0001/0.0.0002/virtio1/block/vda",
        "Missing root bus path should be rejected"
    )]
    #[case::missing_virtio_number(
        "/devices/css0/0.0.0001/0.0.0002/virtio/block/vda",
        "Missing virtio number should be rejected"
    )]
    #[case::incomplete_path(
        "/devices/css0/0.0.0001/0.0.0002/virtio1",
        "Incomplete path should be rejected"
    )]
    #[case::invalid_subchannel_set_high(
        "/devices/css0/1.0.0001/0.0.0002/virtio1/block/vda",
        "Invalid subchannel set (>0) should be rejected"
    )]
    #[case::invalid_subchannel_set_range(
        "/devices/css0/0.4.0001/0.0.0002/virtio1/block/vda",
        "Invalid subchannel set (>3) should be rejected"
    )]
    #[case::invalid_devno_range(
        "/devices/css0/0.0.10000/0.0.0002/virtio1/block/vda",
        "Invalid devno (>0xffff) should be rejected"
    )]
    #[tokio::test]
    async fn test_virtio_blk_ccw_matcher_invalid_paths(
        #[case] devpath: &str,
        #[case] description: &str,
    ) {
        let root_bus = CCW_ROOT_BUS_PATH;
        let subsystem = "block";
        let devname = "vda";
        let relpath = "0.0.0002";

        let mut uev = crate::uevent::Uevent::default();
        uev.action = crate::linux_abi::U_EVENT_ACTION_ADD.to_string();
        uev.subsystem = subsystem.to_string();
        uev.devname = devname.to_string();
        uev.devpath = devpath.to_string();

        let device = ccw::Device::from_str(relpath).unwrap();
        let matcher = VirtioBlkCCWMatcher::new(root_bus, &device);

        assert!(!matcher.is_match(&uev), "{}", description);
    }

    // Helper to create a standard uevent for testing
    fn create_mmio_uevent(devname: &str, mmio_id: u32, virtio_id: u32) -> crate::uevent::Uevent {
        let mut uev = crate::uevent::Uevent::default();
        uev.action = crate::linux_abi::U_EVENT_ACTION_ADD.to_string();
        uev.subsystem = BLOCK.to_string();
        uev.devname = devname.to_string();
        uev.devpath = format!(
            "/sys/devices/virtio-mmio-cmdline/virtio-mmio.{}/virtio{}/block/{}",
            mmio_id, virtio_id, devname
        );
        uev
    }

    #[rstest]
    #[case::vda_matches_vda("vda", "vda", 0, 0, true)]
    #[case::vdb_matches_vdb("vdb", "vdb", 4, 4, true)]
    #[case::vda_rejects_vdb("vda", "vdb", 0, 0, false)]
    #[case::vdb_rejects_vda("vdb", "vda", 4, 4, false)]
    #[tokio::test]
    async fn test_virtio_blk_mmio_matcher_basic_matching(
        #[case] matcher_devname: &str,
        #[case] uevent_devname: &str,
        #[case] mmio_id: u32,
        #[case] virtio_id: u32,
        #[case] should_match: bool,
    ) {
        let matcher = VirtioBlkMmioMatcher::new(matcher_devname);
        let uev = create_mmio_uevent(uevent_devname, mmio_id, virtio_id);

        assert_eq!(
            matcher.is_match(&uev),
            should_match,
            "Matcher for '{}' should {} uevent for '{}'",
            matcher_devname,
            if should_match { "match" } else { "reject" },
            uevent_devname
        );
    }

    #[rstest]
    #[case::wrong_subsystem(test_helpers::SUBSYSTEM_NET, "Wrong subsystem should be rejected")]
    #[tokio::test]
    async fn test_virtio_blk_mmio_matcher_wrong_subsystem(
        #[case] wrong_subsystem: &str,
        #[case] description: &str,
    ) {
        let matcher = VirtioBlkMmioMatcher::new(TEST_DEVNAME);
        let mut uev = create_mmio_uevent(TEST_DEVNAME, 0, 0);
        uev.subsystem = wrong_subsystem.to_string();

        assert!(!matcher.is_match(&uev), "{}", description);
    }

    #[tokio::test]
    async fn test_virtio_blk_mmio_matcher_empty_devname() {
        let matcher = VirtioBlkMmioMatcher::new(TEST_DEVNAME);
        let mut uev = create_mmio_uevent(TEST_DEVNAME, 0, 0);
        uev.devname = String::new();

        assert!(
            !matcher.is_match(&uev),
            "Matcher should reject uevent with empty devname"
        );
    }

    #[tokio::test]
    async fn test_virtio_blk_mmio_matcher_wrong_device_suffix() {
        let matcher = VirtioBlkMmioMatcher::new(TEST_DEVNAME);
        let mut uev = create_mmio_uevent(TEST_DEVNAME, 0, 0);
        // Modify to create the wrong suffix scenario
        uev.devpath =
            "/sys/devices/virtio-mmio-cmdline/virtio-mmio.0/virtio0/block/vdc".to_string();

        assert!(
            !matcher.is_match(&uev),
            "Matcher should reject uevent with wrong device suffix"
        );
    }

    #[tokio::test]
    async fn test_virtio_blk_pci_matcher_correct_match() {
        let matcher = VirtioBlkPciMatcher::new(TEST_PCI_RELPATH, TEST_ROOT_COMPLEX);
        let uev = create_pci_uevent(TEST_DEVNAME, TEST_PCI_RELPATH, TEST_ROOT_COMPLEX, 4);

        assert!(
            matcher.is_match(&uev),
            "Matcher should match correctly formatted uevent"
        );
    }

    #[rstest]
    #[case::wrong_subsystem(test_helpers::SUBSYSTEM_NET, "Wrong subsystem should be rejected")]
    #[tokio::test]
    async fn test_virtio_blk_pci_matcher_wrong_subsystem(
        #[case] wrong_subsystem: &str,
        #[case] description: &str,
    ) {
        let matcher = VirtioBlkPciMatcher::new(TEST_PCI_RELPATH, TEST_ROOT_COMPLEX);
        let mut uev = create_pci_uevent(TEST_DEVNAME, TEST_PCI_RELPATH, TEST_ROOT_COMPLEX, 4);
        uev.subsystem = wrong_subsystem.to_string();

        assert!(!matcher.is_match(&uev), "{}", description);
    }

    #[tokio::test]
    async fn test_virtio_blk_pci_matcher_empty_devname() {
        let matcher = VirtioBlkPciMatcher::new(TEST_PCI_RELPATH, TEST_ROOT_COMPLEX);
        let mut uev = create_pci_uevent(TEST_DEVNAME, TEST_PCI_RELPATH, TEST_ROOT_COMPLEX, 4);
        uev.devname = String::new();

        assert!(
            !matcher.is_match(&uev),
            "Matcher should reject uevent with empty devname"
        );
    }

    #[tokio::test]
    async fn test_virtio_blk_pci_matcher_missing_virtio_component() {
        let root_bus = create_pci_root_bus_path(TEST_ROOT_COMPLEX);
        let matcher = VirtioBlkPciMatcher::new(TEST_PCI_RELPATH, TEST_ROOT_COMPLEX);
        let mut uev = create_pci_uevent(TEST_DEVNAME, TEST_PCI_RELPATH, TEST_ROOT_COMPLEX, 4);
        uev.devpath = format!("{root_bus}{}/block/{}", TEST_PCI_RELPATH, TEST_DEVNAME);

        assert!(
            !matcher.is_match(&uev),
            "Matcher should reject path without virtio component"
        );
    }

    #[rstest]
    #[case::virtio4(4)]
    #[case::virtio99(99)]
    #[case::virtio0(0)]
    #[tokio::test]
    async fn test_virtio_blk_pci_matcher_accepts_any_virtio_number(#[case] virtio_id: u32) {
        let matcher = VirtioBlkPciMatcher::new(TEST_PCI_RELPATH, TEST_ROOT_COMPLEX);
        let uev = create_pci_uevent(TEST_DEVNAME, TEST_PCI_RELPATH, TEST_ROOT_COMPLEX, virtio_id);

        assert!(
            matcher.is_match(&uev),
            "Matcher should accept virtio{} number",
            virtio_id
        );
    }

    #[rstest]
    #[case::missing_dev_prefix("vdb")]
    #[case::empty_devname("/dev/")]
    #[case::dot("/dev/.")]
    #[case::dotdot("/dev/..")]
    #[case::nested_path("/dev/foo/vdb")]
    fn test_parse_mmio_devname_rejects_invalid_paths(#[case] invalid_path: &str) {
        assert!(
            parse_mmio_devname(invalid_path).is_err(),
            "Path '{}' should be rejected",
            invalid_path
        );
    }

    #[tokio::test]
    async fn test_wait_for_mmio_device_existing_nonzero_capacity_returns_immediately() {
        let tmp = tempfile::tempdir().unwrap();
        let dev_root = tmp.path().join("dev");
        let sysfs_dir = tmp.path().join("sys");
        let size_dir = sysfs_dir.join("class/block/vdb");
        fs::create_dir_all(&dev_root).unwrap();
        fs::create_dir_all(&size_dir).unwrap();
        fs::write(dev_root.join("vdb"), b"").unwrap();
        fs::write(size_dir.join("size"), "2097152\n").unwrap();

        let logger = slog::Logger::root(slog::Discard, o!());
        let sandbox = Arc::new(Mutex::new(Sandbox::new(&logger).unwrap()));

        wait_for_virtio_blk_mmio_device_in(
            &sandbox,
            &dev_root,
            &sysfs_dir,
            "vdb",
            Duration::from_millis(200),
            Duration::from_millis(5),
        )
        .await
        .expect("non-zero capacity device should be ready immediately");
    }

    #[tokio::test]
    async fn test_wait_for_mmio_device_placeholder_zero_to_nonzero_capacity_race() {
        let tmp = tempfile::tempdir().unwrap();
        let dev_root = tmp.path().join("dev");
        let sysfs_dir = tmp.path().join("sys");
        let size_dir = sysfs_dir.join("class/block/vdb");
        fs::create_dir_all(&dev_root).unwrap();
        fs::create_dir_all(&size_dir).unwrap();
        // Pre-attached Firecracker placeholder drive: /dev/vdb already exists with 0 sectors.
        fs::write(dev_root.join("vdb"), b"").unwrap();
        let size_file = size_dir.join("size");
        fs::write(&size_file, "0\n").unwrap();

        let updater_size_file = size_file.clone();
        let updater = tokio::spawn(async move {
            sleep(Duration::from_millis(30)).await;
            fs::write(updater_size_file, "2097152\n").unwrap();
        });

        let logger = slog::Logger::root(slog::Discard, o!());
        let sandbox = Arc::new(Mutex::new(Sandbox::new(&logger).unwrap()));

        let start = Instant::now();
        wait_for_virtio_blk_mmio_device_in(
            &sandbox,
            &dev_root,
            &sysfs_dir,
            "vdb",
            Duration::from_secs(1),
            Duration::from_millis(5),
        )
        .await
        .expect("should wait until placeholder drive capacity updates from 0 to non-zero");

        assert!(
            start.elapsed() >= Duration::from_millis(25),
            "must not return while placeholder drive still reports 0 sectors"
        );
        updater.await.unwrap();
        assert_eq!(fs::read_to_string(&size_file).unwrap().trim(), "2097152");
    }

    #[tokio::test]
    async fn test_wait_for_mmio_device_placeholder_stuck_at_zero_times_out() {
        let tmp = tempfile::tempdir().unwrap();
        let dev_root = tmp.path().join("dev");
        let sysfs_dir = tmp.path().join("sys");
        let size_dir = sysfs_dir.join("class/block/vdb");
        fs::create_dir_all(&dev_root).unwrap();
        fs::create_dir_all(&size_dir).unwrap();
        fs::write(dev_root.join("vdb"), b"").unwrap();
        fs::write(size_dir.join("size"), "0\n").unwrap();

        let logger = slog::Logger::root(slog::Discard, o!());
        let sandbox = Arc::new(Mutex::new(Sandbox::new(&logger).unwrap()));

        let err = wait_for_virtio_blk_mmio_device_in(
            &sandbox,
            &dev_root,
            &sysfs_dir,
            "vdb",
            Duration::from_millis(40),
            Duration::from_millis(5),
        )
        .await
        .unwrap_err();

        let msg = err.to_string();
        assert!(
            msg.contains("non-zero capacity"),
            "unexpected timeout error message: {}",
            msg
        );
    }

    #[tokio::test]
    async fn test_wait_for_mmio_device_missing_node_waits_for_uevent_and_capacity() {
        let tmp = tempfile::tempdir().unwrap();
        let dev_root = tmp.path().join("dev");
        let sysfs_dir = tmp.path().join("sys");
        let size_dir = sysfs_dir.join("class/block/vdc");
        fs::create_dir_all(&dev_root).unwrap();
        fs::create_dir_all(&size_dir).unwrap();
        fs::write(size_dir.join("size"), "1048576\n").unwrap();

        let logger = slog::Logger::root(slog::Discard, o!());
        let sandbox = Arc::new(Mutex::new(Sandbox::new(&logger).unwrap()));
        {
            let uev = create_mmio_uevent("vdc", 1, 1);
            let mut sb = sandbox.lock().await;
            sb.uevent_map.insert(uev.devpath.clone(), uev);
        }

        // /dev/vdc does not exist in dev_root, so it exercises uevent lookup + sysfs capacity check.
        wait_for_virtio_blk_mmio_device_in(
            &sandbox,
            &dev_root,
            &sysfs_dir,
            "vdc",
            Duration::from_millis(200),
            Duration::from_millis(5),
        )
        .await
        .expect("should resolve via uevent and verify non-zero capacity");
    }
}
