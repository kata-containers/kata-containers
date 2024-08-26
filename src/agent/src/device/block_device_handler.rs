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
use crate::linux_abi::{create_pci_root_bus_path, SYSFS_DIR, SYSTEM_DEV_PATH};
use crate::pci;
use crate::sandbox::Sandbox;
use crate::uevent::{wait_for_uevent, Uevent, UeventMatcher};
use anyhow::{anyhow, Context, Result};
use kata_types::device::{DRIVER_BLK_CCW_TYPE, DRIVER_BLK_MMIO_TYPE, DRIVER_BLK_PCI_TYPE};
use protocols::agent::Device;
use regex::Regex;
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::instrument;

#[derive(Debug)]
pub struct VirtioBlkPciDeviceHandler {}

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
        let pcipath = pci::Path::from_str(&device.id)?;
        let vm_path = get_virtio_blk_pci_device_name(ctx.sandbox, &pcipath).await?;

        Ok(DeviceInfo::new(&vm_path, true)
            .context("New device info")?
            .into())
    }
}

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
        if !Path::new(&device.vm_path).exists() {
            get_virtio_blk_mmio_device_name(ctx.sandbox, &device.vm_path.to_string())
                .await
                .context("failed to get mmio device name")?;
        }

        Ok(DeviceInfo::new(device.vm_path(), true)
            .context("New device info")?
            .into())
    }
}

#[instrument]
pub async fn get_virtio_blk_pci_device_name(
    sandbox: &Arc<Mutex<Sandbox>>,
    pcipath: &pci::Path,
) -> Result<String> {
    let root_bus_sysfs = format!("{}{}", SYSFS_DIR, create_pci_root_bus_path());
    let sysfs_rel_path = pcipath_to_sysfs(&root_bus_sysfs, pcipath)?;
    let matcher = VirtioBlkPciMatcher::new(&sysfs_rel_path);

    let uev = wait_for_uevent(sandbox, matcher).await?;
    Ok(format!("{}/{}", SYSTEM_DEV_PATH, &uev.devname))
}

#[instrument]
pub async fn get_virtio_blk_mmio_device_name(
    sandbox: &Arc<Mutex<Sandbox>>,
    devpath: &str,
) -> Result<()> {
    let devname = devpath
        .strip_prefix("/dev/")
        .ok_or_else(|| anyhow!("Storage source '{}' must start with /dev/", devpath))?;

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
        .ok_or_else(|| anyhow!("CCW device name {} is not valid UTF-8", &devname))
}

#[derive(Debug)]
pub struct VirtioBlkPciMatcher {
    rex: Regex,
}

impl VirtioBlkPciMatcher {
    pub fn new(relpath: &str) -> VirtioBlkPciMatcher {
        let root_bus = create_pci_root_bus_path();
        let re = format!(r"^{}{}/virtio[0-9]+/block/", root_bus, relpath);

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
            suffix: format!(r"/block/{}", devname),
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
        let re = format!(
            r"^{}/0\.[0-3]\.[0-9a-f]{{1,4}}/{}/virtio[0-9]+/block/",
            root_bus_path, device
        );
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

    #[tokio::test]
    #[allow(clippy::redundant_clone)]
    async fn test_virtio_blk_matcher() {
        let root_bus = create_pci_root_bus_path();
        let devname = "vda";

        let mut uev_a = crate::uevent::Uevent::default();
        let relpath_a = "/0000:00:0a.0";
        uev_a.action = crate::linux_abi::U_EVENT_ACTION_ADD.to_string();
        uev_a.subsystem = BLOCK.to_string();
        uev_a.devname = devname.to_string();
        uev_a.devpath = format!("{}{}/virtio4/block/{}", root_bus, relpath_a, devname);
        let matcher_a = VirtioBlkPciMatcher::new(relpath_a);

        let mut uev_b = uev_a.clone();
        let relpath_b = "/0000:00:0a.0/0000:00:0b.0";
        uev_b.devpath = format!("{}{}/virtio0/block/{}", root_bus, relpath_b, devname);
        let matcher_b = VirtioBlkPciMatcher::new(relpath_b);

        assert!(matcher_a.is_match(&uev_a));
        assert!(matcher_b.is_match(&uev_b));
        assert!(!matcher_b.is_match(&uev_a));
        assert!(!matcher_a.is_match(&uev_b));
    }

    #[cfg(target_arch = "s390x")]
    #[tokio::test]
    async fn test_virtio_blk_ccw_matcher() {
        let root_bus = CCW_ROOT_BUS_PATH;
        let subsystem = "block";
        let devname = "vda";
        let relpath = "0.0.0002";

        let mut uev = crate::uevent::Uevent::default();
        uev.action = crate::linux_abi::U_EVENT_ACTION_ADD.to_string();
        uev.subsystem = subsystem.to_string();
        uev.devname = devname.to_string();
        uev.devpath = format!(
            "{}/0.0.0001/{}/virtio1/{}/{}",
            root_bus, relpath, subsystem, devname
        );

        // Valid path
        let device = ccw::Device::from_str(relpath).unwrap();
        let matcher = VirtioBlkCCWMatcher::new(root_bus, &device);
        assert!(matcher.is_match(&uev));

        // Invalid paths
        uev.devpath = format!(
            "{}/0.0.0001/0.0.0003/virtio1/{}/{}",
            root_bus, subsystem, devname
        );
        assert!(!matcher.is_match(&uev));

        uev.devpath = format!("0.0.0001/{}/virtio1/{}/{}", relpath, subsystem, devname);
        assert!(!matcher.is_match(&uev));

        uev.devpath = format!(
            "{}/0.0.0001/{}/virtio/{}/{}",
            root_bus, relpath, subsystem, devname
        );
        assert!(!matcher.is_match(&uev));

        uev.devpath = format!("{}/0.0.0001/{}/virtio1", root_bus, relpath);
        assert!(!matcher.is_match(&uev));

        uev.devpath = format!(
            "{}/1.0.0001/{}/virtio1/{}/{}",
            root_bus, relpath, subsystem, devname
        );
        assert!(!matcher.is_match(&uev));

        uev.devpath = format!(
            "{}/0.4.0001/{}/virtio1/{}/{}",
            root_bus, relpath, subsystem, devname
        );
        assert!(!matcher.is_match(&uev));

        uev.devpath = format!(
            "{}/0.0.10000/{}/virtio1/{}/{}",
            root_bus, relpath, subsystem, devname
        );
        assert!(!matcher.is_match(&uev));
    }

    #[tokio::test]
    #[allow(clippy::redundant_clone)]
    async fn test_virtio_blk_mmio_matcher() {
        let devname_a = "vda";
        let devname_b = "vdb";
        let mut uev_a = crate::uevent::Uevent::default();
        uev_a.action = crate::linux_abi::U_EVENT_ACTION_ADD.to_string();
        uev_a.subsystem = BLOCK.to_string();
        uev_a.devname = devname_a.to_string();
        uev_a.devpath = format!(
            "/sys/devices/virtio-mmio-cmdline/virtio-mmio.0/virtio0/block/{}",
            devname_a
        );
        let matcher_a = VirtioBlkMmioMatcher::new(devname_a);

        let mut uev_b = uev_a.clone();
        uev_b.devpath = format!(
            "/sys/devices/virtio-mmio-cmdline/virtio-mmio.4/virtio4/block/{}",
            devname_b
        );
        let matcher_b = VirtioBlkMmioMatcher::new(devname_b);

        assert!(matcher_a.is_match(&uev_a));
        assert!(matcher_b.is_match(&uev_b));
        assert!(!matcher_b.is_match(&uev_a));
        assert!(!matcher_a.is_match(&uev_b));
    }
}
