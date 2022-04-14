// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::path::Path;

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use hypervisor::{device, Hypervisor};

use super::Endpoint;
use crate::network::utils::{self, link};

pub const SYS_PCI_DEVICES_PATH: &str = "/sys/bus/pci/devices";

#[derive(Debug)]
pub struct VendorDevice {
    vendor_id: String,
    device_id: String,
}

impl VendorDevice {
    pub fn new(vendor_id: &str, device_id: &str) -> Result<Self> {
        if vendor_id.is_empty() || device_id.is_empty() {
            return Err(anyhow!(
                "invalid parameters vendor_id {} device_id {}",
                vendor_id,
                device_id
            ));
        }
        Ok(Self {
            vendor_id: vendor_id.to_string(),
            device_id: device_id.to_string(),
        })
    }

    pub fn vendor_device_id(&self) -> String {
        format!("{}_{}", &self.vendor_id, &self.device_id)
    }
}

#[derive(Debug)]
pub struct PhysicalEndpoint {
    iface_name: String,
    hard_addr: String,
    bdf: String,
    driver: String,
    vendor_device_id: VendorDevice,
}

impl PhysicalEndpoint {
    pub fn new(name: &str, hardware_addr: &[u8]) -> Result<Self> {
        let driver_info = link::get_driver_info(name).context("get driver info")?;
        let bdf = driver_info.bus_info;
        let sys_pci_devices_path = Path::new(SYS_PCI_DEVICES_PATH);
        // get driver by following symlink /sys/bus/pci/devices/$bdf/driver
        let driver_path = sys_pci_devices_path.join(&bdf).join("driver");
        let link = driver_path.read_link().context("read link")?;
        let driver = link
            .file_name()
            .map_or(String::new(), |v| v.to_str().unwrap().to_owned());

        // get vendor and device id from pci space (sys/bus/pci/devices/$bdf)
        let iface_device_path = sys_pci_devices_path.join(&bdf).join("device");
        let device_id = std::fs::read_to_string(&iface_device_path)
            .context(format!("read device path {:?}", &iface_device_path))?;

        let iface_vendor_path = sys_pci_devices_path.join(&bdf).join("vendor");
        let vendor_id = std::fs::read_to_string(&iface_vendor_path)
            .context(format!("read vendor path {:?}", &iface_vendor_path))?;

        Ok(Self {
            iface_name: name.to_string(),
            hard_addr: utils::get_mac_addr(hardware_addr).context("get mac addr")?,
            vendor_device_id: VendorDevice::new(&vendor_id, &device_id)
                .context("new vendor device")?,
            driver,
            bdf,
        })
    }
}

#[async_trait]
impl Endpoint for PhysicalEndpoint {
    async fn name(&self) -> String {
        self.iface_name.clone()
    }

    async fn hardware_addr(&self) -> String {
        self.hard_addr.clone()
    }

    async fn attach(&self, hypervisor: &dyn Hypervisor) -> Result<()> {
        // bind physical interface from host driver and bind to vfio
        device::bind_device_to_vfio(
            &self.bdf,
            &self.driver,
            &self.vendor_device_id.vendor_device_id(),
        )
        .context(format!(
            "bind physical endpoint from {} to vfio",
            &self.driver
        ))?;

        // set vfio's bus type, pci or mmio. Mostly use pci by default.
        let mode = match self.driver.as_str() {
            "virtio-pci" => "mmio",
            _ => "pci",
        };

        // add vfio device
        let d = device::Device::Vfio(device::VfioConfig {
            id: format!("physical_nic_{}", self.name().await),
            sysfs_path: "".to_string(),
            bus_slot_func: self.bdf.clone(),
            mode: device::VfioBusMode::new(mode)
                .context(format!("new vfio bus mode {:?}", mode))?,
        });
        hypervisor.add_device(d).await.context("add device")?;
        Ok(())
    }

    // detach for physical endpoint unbinds the physical network interface from vfio-pci
    // and binds it back to the saved host driver.
    async fn detach(&self, _hypervisor: &dyn Hypervisor) -> Result<()> {
        // bind back the physical network interface to host.
        // we need to do this even if a new network namespace has not
        // been created by virt-containers.

        // we do not need to enter the network namespace to bind back the
        // physical interface to host driver.
        device::bind_device_to_host(
            &self.bdf,
            &self.driver,
            &self.vendor_device_id.vendor_device_id(),
        )
        .context(format!(
            "bind physical endpoint device from vfio to {}",
            &self.driver
        ))?;
        Ok(())
    }
}
