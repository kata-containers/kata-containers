// Copyright (c) 2022-2023 Alibaba Cloud
// Copyright (c) 2022-2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0

use std::{ffi::OsStr, fs, path::Path, process::Command};

use anyhow::{anyhow, Context, Result};
use pcilibs_rs::{normalize_bdf, vfio, Sysfs};

const SYS_BUS_PCI_DEVICES: &str = "/sys/bus/pci/devices";
const SYS_KERN_IOMMU_GROUPS: &str = "/sys/kernel/iommu_groups";
const VFIO_PCI_DRIVER: &str = "vfio-pci";
const VFIO_PCI_DRIVER_NEW_ID: &str = "/sys/bus/pci/drivers/vfio-pci/new_id";
const SYS_CLASS_IOMMU: &str = "/sys/class/iommu";
const INTEL_IOMMU_PREFIX: &str = "dmar";
const AMD_IOMMU_PREFIX: &str = "ivhd";
const ARM_IOMMU_PREFIX: &str = "smmu";

fn check_iommu_enabled() -> Result<bool> {
    Ok(fs::read_dir(SYS_CLASS_IOMMU)?
        .filter_map(|entry| entry.ok())
        .any(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            name.starts_with(INTEL_IOMMU_PREFIX)
                || name.starts_with(AMD_IOMMU_PREFIX)
                || name.starts_with(ARM_IOMMU_PREFIX)
        }))
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

    if !check_iommu_enabled().context("check iommu")? {
        return Err(anyhow!("IOMMU not enabled"));
    }
    get_vfio_iommu_group(bdf.to_owned()).context("check device IOMMU group")?;
    let sysfs = Sysfs::default();
    if vfio::current_driver(&sysfs, bdf).as_deref() == Some(VFIO_PCI_DRIVER) {
        return Ok(());
    }

    vfio::unbind(&sysfs, bdf).with_context(|| format!("unbind {bdf} from {host_driver}"))?;
    vfio::bind(&sysfs, bdf, VFIO_PCI_DRIVER)
        .with_context(|| format!("probe {bdf} with vfio-pci"))?;
    Ok(())
}

/// Rebind a PCI device from vfio-pci to its original host driver.
pub fn bind_device_to_host(bdf: &str, host_driver: &str, _vendor_device_id: &str) -> Result<()> {
    let sysfs = Sysfs::default();
    let current_driver = vfio::current_driver(&sysfs, bdf);
    if current_driver.as_deref() == Some(host_driver) {
        return Ok(());
    }
    if current_driver.as_deref() != Some(VFIO_PCI_DRIVER) {
        return Err(anyhow!("device {bdf} is not bound to vfio-pci"));
    }
    vfio::unbind(&sysfs, bdf).with_context(|| format!("unbind {bdf} from vfio-pci"))?;
    vfio::bind(&sysfs, bdf, host_driver)
        .with_context(|| format!("probe {bdf} with {host_driver}"))?;
    Ok(())
}

pub fn get_vfio_iommu_group(bdf: String) -> Result<String> {
    let bdf = normalize_bdf(&bdf).ok_or_else(|| anyhow!("invalid PCI BDF: {bdf}"))?;
    let group_link = Path::new(SYS_BUS_PCI_DEVICES)
        .join(&bdf)
        .join("iommu_group");
    if !group_link.exists() {
        return Err(anyhow!(
            "IOMMU group for {bdf} not found; check that IOMMU is enabled for this device"
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
