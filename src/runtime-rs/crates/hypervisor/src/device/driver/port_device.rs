// Copyright (c) 2024 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

//use std::collections::HashMap;

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;

use crate::device::{
    hypervisor,
    topology::{PCIePort, PCIePortBusPrefix, PCIeTopology, TopologyPortDevice},
    Device, DeviceType,
};

#[derive(Debug, Default, Clone)]
pub struct PortDeviceConfig {
    pub port_type: PCIePort,
    pub total_ports: u32,
    pub machine_type: String,
    pub mem_size_32bit: u64,
    pub mem_size_64bit: u64,
}

impl PortDeviceConfig {
    pub fn new(port_type: PCIePort, total_ports: u32) -> Self {
        Self {
            port_type,
            total_ports,
            machine_type: "QemuQ35".to_string(),
            mem_size_32bit: 2097152_u64,
            mem_size_64bit: 4194304_u64,
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct PCIePortDevice {
    /// device id for sharefs device in device manager
    pub device_id: String,

    /// config for sharefs device
    pub config: PortDeviceConfig,
}

impl PCIePortDevice {
    pub fn new(device_id: &str, config: &PortDeviceConfig) -> Self {
        Self {
            device_id: device_id.to_string(),
            config: config.clone(),
        }
    }
}

#[async_trait]
impl Device for PCIePortDevice {
    async fn attach(
        &mut self,
        pcie_topo: &mut Option<&mut PCIeTopology>,
        h: &dyn hypervisor,
    ) -> Result<()> {
        if let Some(topology) = pcie_topo {
            for index in 0..self.config.total_ports {
                if let Some(devices) = topology
                    .pcie_devices_per_port
                    .get_mut(&self.config.port_type)
                {
                    let id = match self.config.port_type {
                        PCIePort::RootPort => format!("{}{}", PCIePortBusPrefix::RootPort, index),
                        PCIePort::SwitchPort => {
                            format!("{}{}", PCIePortBusPrefix::SwitchPort, index)
                        }
                        _ => return Err(anyhow!("unspported pcie port type")),
                    };
                    devices.push(TopologyPortDevice {
                        id,
                        bus: "pcie.0".to_string(),
                        port: self.config.port_type,
                        occupied: false,
                    })
                }
            }

            h.add_device(DeviceType::PortDevice(self.clone()))
                .await
                .context("add port devices.")?;
        }

        Ok(())
    }

    async fn detach(
        &mut self,
        _pcie_topo: &mut Option<&mut PCIeTopology>,
        _h: &dyn hypervisor,
    ) -> Result<Option<u64>> {
        Ok(None)
    }

    async fn update(&mut self, _h: &dyn hypervisor) -> Result<()> {
        Ok(())
    }

    async fn get_device_info(&self) -> DeviceType {
        DeviceType::PortDevice(self.clone())
    }

    async fn increase_attach_count(&mut self) -> Result<bool> {
        Ok(false)
    }

    async fn decrease_attach_count(&mut self) -> Result<bool> {
        Ok(false)
    }
}
