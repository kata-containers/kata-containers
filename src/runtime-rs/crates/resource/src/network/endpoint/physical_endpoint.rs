// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::path::Path;
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use hypervisor::device::device_manager::{do_handle_device, DeviceManager};
use hypervisor::device::DeviceConfig;
use hypervisor::{device::driver, Hypervisor};
use hypervisor::{get_vfio_device, VfioConfig};
use tokio::sync::RwLock;

use super::endpoint_persist::{EndpointState, PhysicalEndpointState};
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
    d: Arc<RwLock<DeviceManager>>,
}

impl PhysicalEndpoint {
    pub fn new(name: &str, hardware_addr: &[u8], d: Arc<RwLock<DeviceManager>>) -> Result<Self> {
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
            .with_context(|| format!("read device path {:?}", &iface_device_path))?;

        let iface_vendor_path = sys_pci_devices_path.join(&bdf).join("vendor");
        let vendor_id = std::fs::read_to_string(&iface_vendor_path)
            .with_context(|| format!("read vendor path {:?}", &iface_vendor_path))?;

        Ok(Self {
            iface_name: name.to_string(),
            hard_addr: utils::get_mac_addr(hardware_addr).context("get mac addr")?,
            vendor_device_id: VendorDevice::new(&vendor_id, &device_id)
                .context("new vendor device")?,
            driver,
            bdf,
            d,
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

    async fn attach(&self) -> Result<()> {
        // bind physical interface from host driver and bind to vfio
        driver::bind_device_to_vfio(
            &self.bdf,
            &self.driver,
            &self.vendor_device_id.vendor_device_id(),
        )
        .with_context(|| format!("bind physical endpoint from {} to vfio", &self.driver))?;

        let vfio_device = get_vfio_device(self.bdf.clone()).context("get vfio device failed.")?;
        let vfio_dev_config = &mut VfioConfig {
            host_path: vfio_device.clone(),
            dev_type: "pci".to_string(),
            hostdev_prefix: "physical_nic_".to_owned(),
            ..Default::default()
        };

        // create and insert VFIO device into Kata VM
        do_handle_device(&self.d, &DeviceConfig::VfioCfg(vfio_dev_config.clone()))
            .await
            .context("do handle device failed.")?;

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
        driver::bind_device_to_host(
            &self.bdf,
            &self.driver,
            &self.vendor_device_id.vendor_device_id(),
        )
        .with_context(|| {
            format!(
                "bind physical endpoint device from vfio to {}",
                &self.driver
            )
        })?;
        Ok(())
    }

    async fn save(&self) -> Option<EndpointState> {
        Some(EndpointState {
            physical_endpoint: Some(PhysicalEndpointState {
                bdf: self.bdf.clone(),
                driver: self.driver.clone(),
                vendor_id: self.vendor_device_id.vendor_id.clone(),
                device_id: self.vendor_device_id.device_id.clone(),
                hard_addr: self.hard_addr.clone(),
            }),
            ..Default::default()
        })
    }
}
