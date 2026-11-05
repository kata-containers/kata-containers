// Copyright (c) 2022-2023 Alibaba Cloud
// Copyright (c) 2022-2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0

use std::{ffi::OsStr, fs, path::Path, process::Command};

use anyhow::{anyhow, Context, Result};
use pcilibs_rs::{vfio, Sysfs};

const VFIO_PCI_DRIVER: &str = "vfio-pci";
const VFIO_PCI_DRIVER_NEW_ID: &str = "/sys/bus/pci/drivers/vfio-pci/new_id";
/// Bind a PCI device to vfio-pci.
pub fn bind_device_to_vfio(bdf: &str, host_driver: &str) -> Result<()> {
    if !Path::new(VFIO_PCI_DRIVER_NEW_ID).exists() {
        let status = Command::new("modprobe")
            .arg(VFIO_PCI_DRIVER)
            .status()
            .context("run modprobe vfio-pci")?;
        if !status.success() {
            return Err(anyhow!("modprobe vfio-pci failed with {status}"));
        }
    }

    get_vfio_iommu_group(bdf.to_owned()).context("check device IOMMU group")?;
    let sysfs = Sysfs::default();
    let original_driver = vfio::current_driver(&sysfs, bdf);
    if original_driver.as_deref() == Some(VFIO_PCI_DRIVER) {
        return Ok(());
    }

    let result = (|| {
        vfio::unbind(&sysfs, bdf).with_context(|| format!("unbind {bdf} from {host_driver}"))?;
        vfio::bind(&sysfs, bdf, VFIO_PCI_DRIVER)
            .with_context(|| format!("probe {bdf} with vfio-pci"))?;
        Ok(())
    })();
    if let Err(err) = result {
        if original_driver.as_deref() != Some(host_driver) {
            return Err(err);
        }
        let rollback = (|| {
            match vfio::current_driver(&sysfs, bdf).as_deref() {
                Some(driver) if driver == host_driver => return Ok(()),
                Some(VFIO_PCI_DRIVER) => vfio::unbind(&sysfs, bdf)?,
                Some(driver) => return Err(anyhow!("device {bdf} is held by {driver}")),
                None => {}
            }
            vfio::bind(&sysfs, bdf, host_driver)?;
            Ok(())
        })();
        return match rollback {
            Ok(()) => Err(err),
            Err(rollback_err) => Err(err.context(format!(
                "restore {bdf} to {host_driver} also failed: {rollback_err:#}"
            ))),
        };
    }
    Ok(())
}

/// Rebind a PCI device from vfio-pci to its original host driver.
pub fn bind_device_to_host(bdf: &str, host_driver: &str) -> Result<()> {
    bind_device_to_host_in(&Sysfs::default(), bdf, host_driver)
}

fn bind_device_to_host_in(sysfs: &Sysfs, bdf: &str, host_driver: &str) -> Result<()> {
    let current_driver = vfio::current_driver(sysfs, bdf);
    if current_driver.as_deref() == Some(host_driver) {
        return Ok(());
    }
    // An interrupted detach may have already unbound the device. Unbind
    // whichever driver still holds it, or proceed directly to the probe.
    vfio::unbind(sysfs, bdf).with_context(|| format!("unbind {bdf} before host driver probe"))?;
    vfio::bind(sysfs, bdf, host_driver)
        .with_context(|| format!("probe {bdf} with {host_driver}"))?;
    Ok(())
}

pub fn get_vfio_iommu_group(bdf: String) -> Result<String> {
    let sysfs = Sysfs::default();
    let device_path = sysfs
        .device(&bdf)
        .ok_or_else(|| anyhow!("invalid PCI BDF: {bdf}"))?;
    let bdf = device_path
        .file_name()
        .and_then(OsStr::to_str)
        .context("PCI device path has no BDF")?;

    let group_link = device_path.join("iommu_group");
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

    if !sysfs
        .iommu_groups()
        .join(group)
        .join("devices")
        .join(bdf)
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    const BDF: &str = "0000:01:00.0";

    #[test]
    fn a_half_rebound_device_only_needs_the_probe() {
        let dir = tempfile::tempdir().unwrap();
        let sysfs = Sysfs::new(dir.path());
        let device = sysfs.device(BDF).unwrap();
        fs::create_dir_all(&device).unwrap();
        fs::write(device.join("driver_override"), "vfio-pci").unwrap();
        fs::write(sysfs.drivers_probe(), "").unwrap();

        bind_device_to_host_in(&sysfs, BDF, "mlx5_core").unwrap();

        assert_eq!(fs::read_to_string(sysfs.drivers_probe()).unwrap(), BDF);
        assert_eq!(
            fs::read_to_string(device.join("driver_override")).unwrap(),
            "mlx5_core"
        );
    }

    #[test]
    fn a_device_already_on_its_host_driver_is_left_alone() {
        let dir = tempfile::tempdir().unwrap();
        let sysfs = Sysfs::new(dir.path());
        let device = sysfs.device(BDF).unwrap();
        let driver = sysfs.driver("mlx5_core").unwrap();
        fs::create_dir_all(&device).unwrap();
        fs::create_dir_all(&driver).unwrap();
        symlink(driver, device.join("driver")).unwrap();

        bind_device_to_host_in(&sysfs, BDF, "mlx5_core").unwrap();
        assert!(!device.join("driver_override").exists());
        assert!(!sysfs.drivers_probe().exists());
    }
}
