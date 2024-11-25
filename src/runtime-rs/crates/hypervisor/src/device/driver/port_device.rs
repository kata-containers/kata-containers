// Copyright (c) 2024 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

//use std::collections::HashMap;

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;

use crate::device::{
    hypervisor,
    topology::{
        PCIePort, PCIePortBusPrefix, PCIeTopology, TopologyPortDevice, DEFAULT_PCIE_ROOT_BUS,
    },
    Device, DeviceType,
};

#[derive(Debug, Default, Clone)]
pub struct PortDeviceConfig {
    pub port_type: PCIePort,
    pub total_ports: u32,
    pub memsz_reserve: u64,
    pub pref64_reserve: u64,
}

impl PortDeviceConfig {
    pub fn new(port_type: PCIePort, total_ports: u32) -> Self {
        Self {
            port_type,
            total_ports,
            // FIXME:
            // A method to automatically determine the maximum memory size
            // based on all vfio devices' information on the host is coming soon.
            memsz_reserve: 33554432_u64,
            pref64_reserve: 536870912_u64,
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
            if let Some(devices) = topology
                .pcie_devices_per_port
                .get_mut(&self.config.port_type)
            {
                match self.config.port_type {
                    PCIePort::RootPort => {
                        for index in 0..topology.pcie_root_ports {
                            let rp_id = format!("{}{}", PCIePortBusPrefix::RootPort, index);
                            devices.push(TopologyPortDevice::new(
                                &rp_id,
                                DEFAULT_PCIE_ROOT_BUS,
                                PCIePort::RootPort,
                                false,
                            ))
                        }
                    }
                    PCIePort::SwitchPort => {
                        for index in 0..topology.pcie_switch_ports {
                            let swdp_id =
                                format!("{}{}", PCIePortBusPrefix::SwitchDownstreamPort, index);
                            devices.push(TopologyPortDevice::new(
                                &swdp_id,
                                DEFAULT_PCIE_ROOT_BUS,
                                PCIePort::SwitchPort,
                                false,
                            ))
                        }
                    }
                    _ => return Err(anyhow!("unspported pcie port type")),
                };
            }
            info!(sl!(), "add_device for PortDevice: {:?}", self.clone());
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
