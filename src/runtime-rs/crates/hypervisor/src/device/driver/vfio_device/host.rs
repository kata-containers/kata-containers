// Copyright (c) 2022-2023 Alibaba Cloud
// Copyright (c) 2022-2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0

use std::{ffi::OsStr, fs, path::Path, process::Command};

use anyhow::{anyhow, Context, Result};

const SYS_BUS_PCI_DRIVER_PROBE: &str = "/sys/bus/pci/drivers_probe";
const SYS_BUS_PCI_DEVICES: &str = "/sys/bus/pci/devices";
const SYS_KERN_IOMMU_GROUPS: &str = "/sys/kernel/iommu_groups";
const VFIO_PCI_DRIVER: &str = "vfio-pci";
const VFIO_PCI_DRIVER_NEW_ID: &str = "/sys/bus/pci/drivers/vfio-pci/new_id";
const VFIO_PCI_DRIVER_UNBIND: &str = "/sys/bus/pci/drivers/vfio-pci/unbind";
const SYS_CLASS_IOMMU: &str = "/sys/class/iommu";
const INTEL_IOMMU_PREFIX: &str = "dmar";
const AMD_IOMMU_PREFIX: &str = "ivhd";
const ARM_IOMMU_PREFIX: &str = "smmu";

fn check_iommu_enabled() -> Result<bool> {
    let element = fs::read_dir(SYS_CLASS_IOMMU)?.filter_map(|e| e.ok()).last();
    let element = element.ok_or_else(|| anyhow!("iommu is not enabled"))?;
    let name = element.file_name().to_string_lossy().into_owned();
    Ok(name.starts_with(INTEL_IOMMU_PREFIX)
        || name.starts_with(AMD_IOMMU_PREFIX)
        || name.starts_with(ARM_IOMMU_PREFIX))
}

fn override_driver(bdf: &str, driver: &str) -> Result<()> {
    let driver_override = format!("/sys/bus/pci/devices/{bdf}/driver_override");
    fs::write(&driver_override, driver)
        .with_context(|| format!("write {driver} to {driver_override}"))?;
    Ok(())
}

fn is_equal_driver(bdf: &str, expected: &str) -> bool {
    let driver_file = Path::new(SYS_BUS_PCI_DEVICES).join(bdf).join("driver");
    fs::read_link(driver_file)
        .ok()
        .and_then(|path| path.file_name().map(|name| name == expected))
        .unwrap_or(false)
}

/// Bind a PCI device to vfio-pci. The final argument is retained for API
/// compatibility with the physical-endpoint call site.
pub fn bind_device_to_vfio(bdf: &str, host_driver: &str, _vendor_device_id: &str) -> Result<()> {
    if !Path::new(VFIO_PCI_DRIVER_NEW_ID).exists() {
        let status = Command::new("modprobe")
            .arg(VFIO_PCI_DRIVER)
            .status()
            .context("run modprobe vfio-pci")?;
        if !status.success() {
            return Err(anyhow!("modprobe vfio-pci failed with {status}"));
        }
    }

    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        let cmdline = fs::read_to_string("/proc/cmdline").context("read /proc/cmdline")?;
        if cmdline.contains("iommu=off") || !cmdline.contains("iommu=") {
            return Err(anyhow!("iommu isn't set on kernel cmdline"));
        }
    }

    if !check_iommu_enabled().context("check iommu")? {
        return Err(anyhow!("IOMMU not enabled"));
    }
    if is_equal_driver(bdf, VFIO_PCI_DRIVER) {
        return Ok(());
    }

    override_driver(bdf, VFIO_PCI_DRIVER)?;
    let unbind_path = format!("/sys/bus/pci/devices/{bdf}/driver/unbind");
    fs::write(&unbind_path, bdf).with_context(|| format!("unbind {bdf} from {host_driver}"))?;
    fs::write(SYS_BUS_PCI_DRIVER_PROBE, bdf)
        .with_context(|| format!("probe {bdf} with vfio-pci"))?;
    Ok(())
}

/// Rebind a PCI device from vfio-pci to its original host driver.
pub fn bind_device_to_host(bdf: &str, host_driver: &str, _vendor_device_id: &str) -> Result<()> {
    if is_equal_driver(bdf, host_driver) {
        return Ok(());
    }
    override_driver(bdf, host_driver)?;
    fs::write(VFIO_PCI_DRIVER_UNBIND, bdf)
        .with_context(|| format!("unbind {bdf} from vfio-pci"))?;
    fs::write(SYS_BUS_PCI_DRIVER_PROBE, bdf)
        .with_context(|| format!("probe {bdf} with {host_driver}"))?;
    Ok(())
}

fn normalize_device_bdf(bdf: &str) -> String {
    if bdf.split(':').count() == 2 {
        format!("0000:{bdf}")
    } else {
        bdf.to_string()
    }
}

pub fn get_vfio_iommu_group(bdf: String) -> Result<String> {
    let bdf = normalize_device_bdf(&bdf);
    let group_link = Path::new(SYS_BUS_PCI_DEVICES)
        .join(&bdf)
        .join("iommu_group");
    if !group_link.exists() {
        return Err(anyhow!(
            "IOMMU group for {bdf} not found; bind the device to vfio-pci first"
        ));
    }
    let target =
        fs::read_link(&group_link).with_context(|| format!("read IOMMU group link for {bdf}"))?;
    let group = target
        .file_name()
        .and_then(OsStr::to_str)
        .ok_or_else(|| anyhow!("invalid IOMMU group link {target:?}"))?;
    if !Path::new(SYS_KERN_IOMMU_GROUPS)
        .join(group)
        .join("devices")
        .join(&bdf)
        .exists()
    {
        return Err(anyhow!("device {bdf} is absent from IOMMU group {group}"));
    }
    Ok(format!("/dev/vfio/{group}"))
}

/// Resolve either an existing VFIO device path or a PCI BDF to the legacy
/// group device path used by discovery.
pub fn get_vfio_device(device: String) -> Result<String> {
    let components: Vec<&str> = device.split(&[':', '.'][..]).collect();
    if (3..5).contains(&components.len()) {
        get_vfio_iommu_group(device)
    } else {
        Ok(device)
    }
}
