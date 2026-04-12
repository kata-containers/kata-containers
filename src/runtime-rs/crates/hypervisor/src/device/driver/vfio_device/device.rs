// Copyright (c) 2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{Context, Result};
use async_trait::async_trait;
use kata_sys_util::pcilibs::get_bars_max_addressable_memory;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::device::pci_path::PciPath;
use crate::device::topology::{PCIePort, PCIeTopology};
use crate::device::util::{do_decrease_count, do_increase_count};
use crate::device::{Device, DeviceType, PCIeDevice};
use crate::vfio_device::core::{discover_vfio_device, discover_vfio_group_device, VfioDevice};
use crate::Hypervisor;

/// Identifies a specific port on a PCI bus: (bus_name, bus_slot, port_id)
/// bus_name = rp<port_id>
pub type BusPortId = (String, u32, u32);

#[derive(Debug, Default, Clone)]
pub struct VfioDeviceBase {
    /// Host device path, typically /dev/vfio/N (legacy)
    pub host_path: String,

    /// Primary PCI Bus-Device-Function (BDF) address
    pub host_bdf: String,

    /// All BDFs belonging to the same logical device or IOMMU group
    pub host_bdfs: Vec<String>,

    /// The bus and port ID to which the device is attached (e.g., ("pci.1", 2))
    pub bus_port_id: BusPortId,

    /// Specifies the PCIe port type (e.g., Root Port, Downstream Port)
    pub port: PCIePort,

    /// Character device node for the IOMMU group (/dev/vfio/X)
    pub iommu_group_devnode: PathBuf,

    /// Character device node for the specific VFIO device (/dev/vfio/devices/vfioX)
    pub iommu_device_node: Option<PathBuf>,

    /// The guest-side PCI path representing the device's BDF address in the VM
    pub guest_pci_path: Option<PciPath>,

    /// Device classification: "block" or "char"
    pub dev_type: String,

    /// Underlying bus architecture: "pci" or "ccw"
    pub bus_type: String,

    /// Represents the device's path as it appears inside the VM guest,
    /// independent of the host container's mount namespace.
    /// format: Option<(device_index, path_name)>
    pub virt_path: Option<(u64, String)>,

    /// Prefix used for host device identification. Examples:
    /// - Physical Endpoint: "physical_nic_"
    /// - Mediated Device:  "vfio_mdev_"
    /// - PCI Passthrough:  "vfio_device_"
    /// - VFIO Volume:     "vfio_vol_"
    /// - VFIO NVMe:       "vfio_nvme_"
    pub hostdev_prefix: String,
}

#[derive(Debug, Default, Clone)]
pub struct VfioDeviceModern {
    pub device_id: String,
    pub device: VfioDevice,
    pub config: VfioDeviceBase,

    /// Configuration options passed to the vfio-pci handler in kata-agent
    pub device_options: Vec<String>,

    /// Indicates if the host device has been allocated to a specific guest
    pub is_allocated: bool,

    /// Reference count for active attachments
    pub attach_count: u64,

    /// Maximum addressable memory reserved for MMIO BARs
    pub memory_reserve: u64,

    /// Maximum addressable memory reserved for 64-bit prefetchable BARs
    pub pref64_reserve: u64,
}

/// Path used for [`discover_vfio_group_device`] when `iommu_device_node` is unset.
/// CDI cold-plug often only fills `host_path`; `iommu_group_devnode` may still be empty until
/// device_manager copies `host_path` — treat those as the same node.
fn vfio_modern_group_discovery_path(base: &VfioDeviceBase) -> PathBuf {
    if !base.iommu_group_devnode.as_os_str().is_empty() {
        base.iommu_group_devnode.clone()
    } else {
        PathBuf::from(base.host_path.trim())
    }
}

impl VfioDeviceModern {
    pub fn new(device_id: String, base: &VfioDeviceBase) -> Result<Self> {
        // For modern VFIO devices, we require the specific device cdev path to be provided in the configuration.
        // This allows us to directly discover the device context without needing to resolve group devices.
        // If the device node is not provided, we can optionally fallback to group device discovery,
        // but this is less efficient and may not be supported in all environments.
        let device = if let Some(ref node) = base.iommu_device_node {
            if !node.as_os_str().is_empty() {
                discover_vfio_device(node)?
            } else {
                discover_vfio_group_device(vfio_modern_group_discovery_path(base))?
            }
        } else {
            discover_vfio_group_device(vfio_modern_group_discovery_path(base))?
        };
        let (memory_reserve, pref64_reserve) = get_bars_max_addressable_memory();

        Ok(Self {
            device_id,
            device,
            config: base.clone(),
            device_options: Vec::new(),
            is_allocated: false,
            attach_count: 0,
            memory_reserve,
            pref64_reserve,
        })
    }
}

/// Thread-safe handle for managing modern VFIO devices using asynchronous locking.
#[derive(Clone, Debug)]
pub struct VfioDeviceModernHandle {
    pub inner: Arc<Mutex<VfioDeviceModern>>,
}

impl VfioDeviceModernHandle {
    pub fn new(device_id: String, base: &VfioDeviceBase) -> Result<Self> {
        let vfio_device = VfioDeviceModern::new(device_id, base)?;
        Ok(Self {
            inner: Arc::new(Mutex::new(vfio_device)),
        })
    }

    pub fn arc(&self) -> Arc<Mutex<VfioDeviceModern>> {
        self.inner.clone()
    }

    /// Scoped read access: Executes a closure within the device lock.
    pub async fn with<R>(&self, f: impl FnOnce(&VfioDeviceModern) -> R) -> R {
        let guard = self.inner.lock().await;
        f(&guard)
    }

    /// Scoped write access: Executes a mutating closure within the device lock.
    pub async fn with_mut<R>(&self, f: impl FnOnce(&mut VfioDeviceModern) -> R) -> R {
        let mut guard = self.inner.lock().await;
        f(&mut guard)
    }

    pub async fn device_id(&self) -> String {
        self.inner.lock().await.device_id.clone()
    }

    pub async fn vfio_config(&self) -> VfioDeviceBase {
        self.inner.lock().await.config.clone()
    }

    pub async fn vfio_device(&self) -> VfioDevice {
        self.inner.lock().await.device.clone()
    }

    pub async fn attach_count(&self) -> u64 {
        self.inner.lock().await.attach_count
    }

    pub async fn set_allocated(&self, allocated: bool) {
        self.inner.lock().await.is_allocated = allocated;
    }

    pub async fn update_config(&self, cfg: VfioDeviceBase) {
        self.inner.lock().await.config = cfg;
    }
}

#[async_trait]
impl Device for VfioDeviceModernHandle {
    /// Attaches the VFIO device to the hypervisor and registers it in the PCIe topology.
    async fn attach(
        &mut self,
        pcie_topo: &mut Option<&mut PCIeTopology>,
        h: &dyn Hypervisor,
    ) -> Result<()> {
        // Check if device is already attached
        if self
            .increase_attach_count()
            .await
            .context("failed to increase attach count")?
        {
            warn!(
                sl!(),
                "The device {:?} is already attached; multi-attach is not allowed.",
                self.device_id().await
            );
            return Ok(());
        }

        // Register the device in the virtual PCIe topology if provided
        match pcie_topo {
            Some(topo) => self.register(topo).await?,
            None => return Ok(()),
        }

        // Request Hypervisor to perform the actual hardware passthrough
        if let Err(e) = h.add_device(DeviceType::VfioModern(self.arc())).await {
            error!(sl!(), "failed to attach vfio device: {:?}", e);

            // Rollback state on failure
            self.decrease_attach_count().await?;
            if let Some(topo) = pcie_topo {
                self.unregister(topo).await?;
            }
            return Err(e);
        }
        info!(
            sl!(),
            "vfio device {:?} attached successfully",
            self.device_id().await
        );
        Ok(())
    }

    /// Detaches the VFIO device from the hypervisor and releases topology resources.
    async fn detach(
        &mut self,
        pcie_topo: &mut Option<&mut PCIeTopology>,
        h: &dyn Hypervisor,
    ) -> Result<Option<u64>> {
        // Only proceed with detachment if reference count reaches zero
        if self
            .decrease_attach_count()
            .await
            .context("failed to decrease attach count")?
        {
            return Ok(None);
        }

        if let Err(e) = h
            .remove_device(DeviceType::VfioModern(self.inner.clone()))
            .await
        {
            // Rollback: increment count if hypervisor fails to remove the device
            self.increase_attach_count().await?;
            return Err(e);
        }

        // Retrieve device index if a virtual path exists
        let virt = self.with(|d| d.config.virt_path.clone()).await;
        let device_index = virt.map(|(idx, _)| idx);

        // Unregister from PCIe topology
        if let Some(topo) = pcie_topo {
            self.unregister(topo).await?;
        }

        Ok(device_index)
    }

    async fn update(&mut self, _h: &dyn Hypervisor) -> Result<()> {
        // Updates are typically not required for VFIO passthrough devices
        Ok(())
    }

    async fn increase_attach_count(&mut self) -> Result<bool> {
        let mut guard = self.inner.lock().await;
        do_increase_count(&mut guard.attach_count)
    }

    async fn decrease_attach_count(&mut self) -> Result<bool> {
        let mut guard = self.inner.lock().await;
        do_decrease_count(&mut guard.attach_count)
    }

    async fn get_device_info(&self) -> DeviceType {
        DeviceType::VfioModern(self.arc())
    }
}

#[async_trait]
impl PCIeDevice for VfioDeviceModernHandle {
    /// Reserves a bus and port in the PCIe topology for this device.
    async fn register(&mut self, topo: &mut PCIeTopology) -> Result<()> {
        let device_id = self.device_id().await;
        let port_type = self.with(|d| d.config.port).await;

        // Reserve the bus based on the specified port type
        let bus_port_id = match topo.reserve_bus_for_device(&device_id, port_type)? {
            Some(id) => id,
            None => return Err(anyhow::anyhow!("can not get bus port")),
        };

        self.with_mut(|d| {
            d.config.bus_port_id = bus_port_id;
            d.is_allocated = true;
        })
        .await;

        Ok(())
    }

    /// Releases the reserved PCIe resources and resets attachment state.
    async fn unregister(&mut self, topo: &mut PCIeTopology) -> Result<()> {
        let device_id = self.device_id().await;
        topo.release_bus_for_device(&device_id)?;

        self.with_mut(|d| {
            d.is_allocated = false;
            d.config.bus_port_id.0.clear();
            d.config.guest_pci_path = None;
        })
        .await;

        Ok(())
    }
}
