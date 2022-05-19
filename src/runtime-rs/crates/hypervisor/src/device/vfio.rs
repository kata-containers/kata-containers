// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::{fs, path::Path, process::Command};

use anyhow::{anyhow, Context, Result};

fn override_driver(bdf: &str, driver: &str) -> Result<()> {
    let driver_override = format!("/sys/bus/pci/devices/{}/driver_override", bdf);
    fs::write(&driver_override, driver)
        .with_context(|| format!("echo {} > {}", driver, &driver_override))?;
    info!(sl!(), "echo {} > {}", driver, driver_override);
    Ok(())
}

const SYS_PCI_DEVICES_PATH: &str = "/sys/bus/pci/devices";
const PCI_DRIVER_PROBE: &str = "/sys/bus/pci/drivers_probe";
const VFIO_NEW_ID_PATH: &str = "/sys/bus/pci/drivers/vfio-pci/new_id";

pub const VFIO_PCI: &str = "vfio-pci";

#[derive(Debug)]
pub enum VfioBusMode {
    PCI,
    MMIO,
}

impl VfioBusMode {
    pub fn new(mode: &str) -> Result<Self> {
        Ok(match mode {
            "mmio" => VfioBusMode::MMIO,
            _ => VfioBusMode::PCI,
        })
    }
}

#[derive(Debug)]
pub struct VfioConfig {
    /// Unique identifier of the device
    pub id: String,

    /// Sysfs path for mdev bus type device
    pub sysfs_path: String,

    /// PCI device information: "bus:slot:function"
    pub bus_slot_func: String,

    /// Bus Mode, PCI or MMIO
    pub mode: VfioBusMode,
}

/// binds the device to vfio driver after unbinding from host.
/// Will be called by a network interface or a generic pcie device.
pub fn bind_device_to_vfio(bdf: &str, host_driver: &str, _vendor_device_id: &str) -> Result<()> {
    // modprobe vfio-pci
    if !Path::new(VFIO_NEW_ID_PATH).exists() {
        Command::new("modprobe")
            .arg(VFIO_PCI)
            .output()
            .expect("Failed to run modprobe vfio-pci");
    }

    // Arm does not need cmdline to open iommu, just set it through bios.
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        // check intel_iommu=on
        let cmdline = fs::read_to_string("/proc/cmdline").unwrap();
        if cmdline.contains("iommu=off") || !cmdline.contains("iommu=") {
            return Err(anyhow!("iommu isn't set on kernel cmdline"));
        }
    }

    // if it's already bound to vfio
    if is_equal_driver(bdf, VFIO_PCI) {
        info!(sl!(), "bdf : {} was already bound to vfio-pci", bdf);
        return Ok(());
    }

    info!(sl!(), "host driver : {}", host_driver);
    override_driver(bdf, VFIO_PCI).context("override driver")?;

    let unbind_path = format!("/sys/bus/pci/devices/{}/driver/unbind", bdf);

    // echo bdf > /sys/bus/pci/drivers/virtio-pci/unbind"
    fs::write(&unbind_path, bdf)
        .with_context(|| format!("Failed to echo {} > {}", bdf, &unbind_path))?;

    info!(sl!(), "{} is unbound from {}", bdf, host_driver);

    // echo bdf > /sys/bus/pci/drivers_probe
    fs::write(PCI_DRIVER_PROBE, bdf)
        .with_context(|| format!("Failed to echo {} > {}", bdf, PCI_DRIVER_PROBE))?;

    info!(sl!(), "echo {} > /sys/bus/pci/drivers_probe", bdf);
    Ok(())
}

pub fn is_equal_driver(bdf: &str, host_driver: &str) -> bool {
    let sys_pci_devices_path = Path::new(SYS_PCI_DEVICES_PATH);
    let driver_file = sys_pci_devices_path.join(bdf).join("driver");

    if driver_file.exists() {
        let driver_path = fs::read_link(driver_file).unwrap_or_default();
        let driver_name = driver_path
            .file_name()
            .map_or(String::new(), |v| v.to_str().unwrap().to_owned());
        return driver_name.eq(host_driver);
    }

    false
}

/// bind_device_to_host binds the device to the host driver after unbinding from vfio-pci.
pub fn bind_device_to_host(bdf: &str, host_driver: &str, _vendor_device_id: &str) -> Result<()> {
    // Unbind from vfio-pci driver to the original host driver

    info!(sl!(), "bind {} to {}", bdf, host_driver);

    // if it's already bound to host_driver
    if is_equal_driver(bdf, host_driver) {
        info!(
            sl!(),
            "bdf {} was already unbound to host driver {}", bdf, host_driver
        );
        return Ok(());
    }

    override_driver(bdf, host_driver).context("override driver")?;

    let unbind_path = "/sys/bus/pci/drivers/vfio-pci/unbind";

    // echo bdf > /sys/bus/pci/drivers/vfio-pci/unbind"
    std::fs::write(unbind_path, bdf).with_context(|| format!("echo {}> {}", bdf, unbind_path))?;
    info!(sl!(), "echo {} > {}", bdf, unbind_path);

    // echo bdf > /sys/bus/pci/drivers_probe
    std::fs::write(PCI_DRIVER_PROBE, bdf)
        .with_context(|| format!("echo {} > {}", bdf, PCI_DRIVER_PROBE))?;
    info!(sl!(), "echo {} > {}", bdf, PCI_DRIVER_PROBE);

    Ok(())
}
