// Copyright (c) 2026 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

//! Inner state for the OpenVMM hypervisor integration.

use std::collections::{HashMap, HashSet, VecDeque};

use crate::{device::DeviceType, hypervisor_persist::HypervisorState, VmmState};
use anyhow::Result;
use kata_types::{
    capabilities::{Capabilities, CapabilityBits},
    config::hypervisor::Hypervisor as HypervisorConfig,
};
use tokio::sync::mpsc;

use super::vmm_instance::VmmInstance;
use super::{
    OPENVMM_BLOCK_HOTPLUG_FIRST_DEVICE, OPENVMM_BLOCK_HOTPLUG_PORT_COUNT,
    OPENVMM_BLOCK_HOTPLUG_PORT_PREFIX,
};
use crate::device::pci_path::{PciPath, PciSlot};

#[derive(Clone, Debug)]
pub(crate) struct OpenVmmHotplugPort {
    /// Topology port name (e.g. "hp0"); matches the name declared in the PCIe
    /// topology at CreateVm and targeted by AddPcieDevice/RemovePcieDevice.
    pub(crate) name: String,
    /// Guest-visible PCI path of an endpoint hot-added into this port: "DD/00"
    /// (root-port device number on bus 0, then device 0 on its secondary bus).
    /// Reported to the agent so it can resolve /dev/vdX.
    pub(crate) pci_path: PciPath,
}

impl OpenVmmHotplugPort {
    fn new(index: u8) -> Self {
        let device = OPENVMM_BLOCK_HOTPLUG_FIRST_DEVICE + index;
        let pci_path = PciPath::new(vec![PciSlot::new(device), PciSlot::new(0)])
            .expect("openvmm hotplug port PCI path is non-empty");
        Self {
            name: format!("{}{}", OPENVMM_BLOCK_HOTPLUG_PORT_PREFIX, index),
            pci_path,
        }
    }
}

/// Inner state for the OpenVMM hypervisor.
#[allow(dead_code)]
pub(crate) struct OpenVmmInner {
    pub(crate) id: String,
    pub(crate) vm_path: String,
    pub(crate) jailer_root: String,
    pub(crate) netns: Option<String>,
    pub(crate) config: HypervisorConfig,
    pub(crate) state: VmmState,
    pub(crate) run_dir: String,
    pub(crate) pending_devices: Vec<DeviceType>,
    pub(crate) cached_block_devices: HashSet<String>,
    pub(crate) free_block_hotplug_ports: VecDeque<OpenVmmHotplugPort>,
    pub(crate) attached_block_hotplug_ports: HashMap<String, OpenVmmHotplugPort>,
    pub(crate) capabilities: Capabilities,
    pub(crate) guest_memory_block_size_mb: u32,
    pub(crate) vmm_instance: VmmInstance,
}

impl std::fmt::Debug for OpenVmmInner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpenVmmInner")
            .field("id", &self.id)
            .field("state", &self.state)
            .finish()
    }
}

impl OpenVmmInner {
    pub(crate) fn new(exit_notify: mpsc::Sender<i32>) -> Self {
        let mut capabilities = Capabilities::new();
        capabilities.set(CapabilityBits::BlockDeviceSupport | CapabilityBits::FsSharingSupport);

        OpenVmmInner {
            id: String::new(),
            vm_path: String::new(),
            jailer_root: String::new(),
            netns: None,
            config: HypervisorConfig::default(),
            state: VmmState::NotReady,
            run_dir: String::new(),
            pending_devices: Vec::new(),
            cached_block_devices: HashSet::new(),
            free_block_hotplug_ports: Self::default_block_hotplug_ports(),
            attached_block_hotplug_ports: HashMap::new(),
            capabilities,
            guest_memory_block_size_mb: 0,
            vmm_instance: VmmInstance::new(exit_notify),
        }
    }

    pub(crate) fn set_hypervisor_config(&mut self, config: HypervisorConfig) {
        self.config = config;
    }

    pub(crate) fn hypervisor_config(&self) -> HypervisorConfig {
        self.config.clone()
    }

    pub(crate) async fn capabilities(&self) -> Result<Capabilities> {
        Ok(self.capabilities.clone())
    }

    pub(crate) fn set_capabilities(&mut self, flag: CapabilityBits) {
        self.capabilities.set(flag);
    }

    pub(crate) fn set_guest_memory_block_size(&mut self, size: u32) {
        self.guest_memory_block_size_mb = size;
    }

    pub(crate) fn guest_memory_block_size_mb(&self) -> u32 {
        self.guest_memory_block_size_mb
    }

    pub(crate) async fn save(&self) -> Result<HypervisorState> {
        Ok(HypervisorState {
            hypervisor_type: "openvmm".to_string(),
            id: self.id.clone(),
            vm_path: self.vm_path.clone(),
            netns: self.netns.clone(),
            config: self.config.clone(),
            run_dir: self.run_dir.clone(),
            cached_block_devices: self.cached_block_devices.clone(),
            ..Default::default()
        })
    }

    pub(crate) async fn restore(
        exit_notify: mpsc::Sender<i32>,
        state: HypervisorState,
    ) -> Result<Self> {
        let mut inner = OpenVmmInner::new(exit_notify);
        inner.id = state.id;
        inner.vm_path = state.vm_path;
        inner.netns = state.netns;
        inner.config = state.config;
        inner.run_dir = state.run_dir;
        inner.cached_block_devices = state.cached_block_devices;
        inner.reset_block_hotplug_ports();
        Ok(inner)
    }

    pub(crate) fn reset_block_hotplug_ports(&mut self) {
        self.free_block_hotplug_ports = Self::default_block_hotplug_ports();
        self.attached_block_hotplug_ports.clear();
    }

    pub(crate) fn reserve_block_hotplug_port(
        &mut self,
        device_id: &str,
    ) -> Result<OpenVmmHotplugPort> {
        if let Some(port) = self.attached_block_hotplug_ports.get(device_id) {
            return Ok(port.clone());
        }

        let port = self
            .free_block_hotplug_ports
            .pop_front()
            .ok_or_else(|| anyhow::anyhow!("openvmm ran out of block hotplug PCIe ports"))?;

        self.attached_block_hotplug_ports
            .insert(device_id.to_string(), port.clone());

        Ok(port)
    }

    pub(crate) fn block_hotplug_port(&self, device_id: &str) -> Option<OpenVmmHotplugPort> {
        self.attached_block_hotplug_ports.get(device_id).cloned()
    }

    pub(crate) fn release_block_hotplug_port(
        &mut self,
        device_id: &str,
    ) -> Option<OpenVmmHotplugPort> {
        let port = self.attached_block_hotplug_ports.remove(device_id)?;
        self.free_block_hotplug_ports.push_front(port.clone());
        Some(port)
    }

    fn default_block_hotplug_ports() -> VecDeque<OpenVmmHotplugPort> {
        (0..OPENVMM_BLOCK_HOTPLUG_PORT_COUNT)
            .map(OpenVmmHotplugPort::new)
            .collect()
    }
}
