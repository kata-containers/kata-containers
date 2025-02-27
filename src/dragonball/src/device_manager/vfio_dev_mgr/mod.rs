// Copyright 2023 Alibaba, Inc. or its affiliates. All Rights Reserved.
// Copyright 2018 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0
//
// Portions Copyright 2017 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the THIRD-PARTY file.
//! Device manager for host passthrough devices.
// we allow missing_doc temporaily, because rust can't use this declariation in marco

#![allow(missing_docs)]
mod pci_vfio;
pub use pci_vfio::PciSystemManager;

use std::collections::HashMap;
use std::ops::Deref;
use std::os::fd::RawFd;
use std::path::Path;
use std::sync::{Arc, Weak};

use crossbeam_channel::Sender;
use dbs_device::resources::Resource::LegacyIrq;
use dbs_device::resources::{DeviceResources, Resource, ResourceConstraint};
use dbs_device::DeviceIo;
use dbs_interrupt::KvmIrqManager;
use dbs_pci::{VfioPciDevice, VENDOR_NVIDIA};
use dbs_upcall::{DevMgrResponse, UpcallClientResponse};
use kvm_ioctls::{DeviceFd, VmFd};
use log::{debug, error};
use serde_derive::{Deserialize, Serialize};
use vfio_ioctls::{VfioContainer, VfioDevice};
use vm_memory::{
    Address, GuestAddressSpace, GuestMemory, GuestMemoryRegion, GuestRegionMmap,
    MemoryRegionAddress,
};

use super::StartMicroVmError;
use crate::address_space_manager::{GuestAddressSpaceImpl, GuestMemoryImpl};
use crate::config_manager::{ConfigItem, DeviceConfigInfo, DeviceConfigInfos};
use crate::device_manager::{DeviceManagerContext, DeviceMgrError, DeviceOpContext};
use crate::resource_manager::{ResourceError, ResourceManager};

// The flag of whether to use the shared irq.
const USE_SHARED_IRQ: bool = true;

/// Errors associated with the operations allowed on a host device
#[derive(Debug, thiserror::Error)]
pub enum VfioDeviceError {
    /// Internal error.
    #[error("VFIO subsystem internal error")]
    InternalError,

    /// The virtual machine instance ID is invalid.
    #[error("the virtual machine instance ID is invalid")]
    InvalidVMID,

    /// Cannot open host device due to invalid bus::slot::function
    #[error("can't open host device for VFIO")]
    CannotOpenVfioDevice,

    /// The Context Identifier is already in use.
    #[error("the device ID {0} already exists")]
    DeviceIDAlreadyExist(String),

    /// Host device string (bus::slot::function) is already in use
    #[error("device '{0}' is already in use")]
    DeviceAlreadyInUse(String),

    /// The configuration of vfio device is invalid.
    #[error("The configuration of vfio device is invalid")]
    InvalidConfig,

    /// No resource available
    #[error("no resource available for VFIO device")]
    NoResource,

    /// Cannot perform the requested operation after booting the microVM
    #[error("update operation is not allowed after boot")]
    UpdateNotAllowedPostBoot,

    /// Failed to create kvm device
    #[error("failed to create kvm device: {0:?}")]
    CreateKvmDevice(#[source] vmm_sys_util::errno::Error),

    /// Failed to restore vfio mlock count
    #[error("failure while restoring vfio mlock count: {0:?}")]
    RestoreMlockCount(#[source] std::io::Error),

    /// Failure in device manager while managing VFIO device
    #[error("failure in device manager while managing VFIO device, {0:?}")]
    VfioDeviceMgr(#[source] DeviceMgrError),

    /// Failure in VFIO IOCTL subsystem.
    #[error("failure while configuring VFIO device, {0:?}")]
    VfioIoctlError(#[source] vfio_ioctls::VfioError),

    /// Failure in VFIO PCI subsystem.
    #[error("failure while managing PCI VFIO device: {0:?}")]
    VfioPciError(#[source] dbs_pci::VfioPciError),

    /// Failure in PCI subsystem.
    #[error("PCI subsystem failed to manage the device: {0:?}")]
    PciError(#[source] dbs_pci::Error),

    /// Failed to get vfio host info
    #[error("PCI get host info failed: {0}")]
    GetHostInfo(String),

    /// Invalid PCI device ID
    #[error("invalid PCI device ID: {0}")]
    InvalidDeviceID(u32),

    /// Failed to allocate device resource
    #[error("failure while allocate device resource: {0:?}")]
    AllocateDeviceResource(#[source] ResourceError),

    /// Failed to free device resource
    #[error("failure while freeing device resource: {0:?}")]
    FreeDeviceResource(#[source] ResourceError),

    /// Vfio container not found
    #[error("vfio container not found")]
    VfioContainerNotFound,

    /// Generic IO error.
    #[error("Generic IO error, {0}")]
    IoError(#[source] std::io::Error),
}

type Result<T> = std::result::Result<T, VfioDeviceError>;

/// Host info for vfio device
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct VfioDeviceHostInfo {
    pub group_id: u32,
    pub group_fd: RawFd,
    pub device_fd: RawFd,
}

/// Configuration information for a VFIO PCI device.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, Default)]
pub struct VfioPciDeviceConfig {
    /// PCI device information: "bus:slot:function"
    pub bus_slot_func: String,
    /// PCI vendor and device id
    /// high 16bit : low 16bit = device_id : vendor_id
    pub vendor_device_id: u32,
    /// Deice ID used in guest, guest_dev_id = slot
    pub guest_dev_id: Option<u8>,
    /// Clique ID for Nvidia GPUs and RDMA NICs
    pub clique_id: Option<u8>,
}

impl VfioPciDeviceConfig {
    /// default pci domain is 0
    pub fn host_pci_domain(&self) -> u32 {
        0
    }

    pub fn valid_vendor_device(&self) -> bool {
        if self.vendor_device_id == 0 {
            return true;
        }
        // vendor_device_id high 16bit : low 16bit = device_id : vendor_id
        self.vendor_device_id != 0
            && (self.vendor_device_id & 0xffff) != 0
            && ((self.vendor_device_id >> 16) & 0xffff) != 0
    }
}

/// Configuration for a specific Vfio Device
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum VfioDevConfig {
    Pci(VfioPciDeviceConfig),
}

impl Default for VfioDevConfig {
    fn default() -> Self {
        Self::Pci(Default::default())
    }
}

/// Configuration information for a VFIO device.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize, Default)]
pub struct HostDeviceConfig {
    /// Unique identifier of the hostdev
    pub hostdev_id: String,
    /// Sysfs path for device
    pub sysfs_path: String,
    /// Device specific config
    pub dev_config: VfioPciDeviceConfig,
}

impl ConfigItem for HostDeviceConfig {
    type Err = VfioDeviceError;

    fn id(&self) -> &str {
        &self.hostdev_id
    }

    fn check_conflicts(&self, other: &Self) -> Result<()> {
        if self.hostdev_id == other.hostdev_id {
            return Err(VfioDeviceError::DeviceIDAlreadyExist(
                self.hostdev_id.clone(),
            ));
        }

        if !self.sysfs_path.is_empty() && self.sysfs_path == other.sysfs_path {
            return Err(VfioDeviceError::DeviceAlreadyInUse(self.sysfs_path.clone()));
        }

        if !self.dev_config.bus_slot_func.is_empty()
            && self.dev_config.bus_slot_func == other.dev_config.bus_slot_func
        {
            return Err(VfioDeviceError::DeviceAlreadyInUse(
                self.dev_config.bus_slot_func.clone(),
            ));
        }

        Ok(())
    }
}

/// Vfio device info
pub type VfioDeviceInfo = DeviceConfigInfo<HostDeviceConfig>;

/// A device manager to manage all VFIO devices.
pub struct VfioDeviceMgr {
    vm_fd: Arc<VmFd>,
    info_list: DeviceConfigInfos<HostDeviceConfig>,
    locked_vm_size: u64,
    vfio_container: Option<Arc<VfioContainer>>,
    pci_vfio_manager: Option<Arc<PciSystemManager>>,
    pci_legacy_irqs: Option<HashMap<u8, u8>>,
    nvidia_shared_irq: Option<u32>,
    logger: slog::Logger,
}

impl VfioDeviceMgr {
    /// Create a new VFIO device manager.
    pub fn new(vm_fd: Arc<VmFd>, logger: &slog::Logger) -> Self {
        VfioDeviceMgr {
            vm_fd,
            info_list: DeviceConfigInfos::new(),
            locked_vm_size: 0,
            vfio_container: None,
            pci_vfio_manager: None,
            pci_legacy_irqs: Some(HashMap::new()),
            nvidia_shared_irq: None,
            logger: logger.new(slog::o!()),
        }
    }

    /// Insert or update a VFIO device into the manager.ig)?;
    pub fn insert_device(
        &mut self,
        ctx: &mut DeviceOpContext,
        config: &mut HostDeviceConfig,
    ) -> Result<()> {
        if !cfg!(feature = "hotplug") && ctx.is_hotplug {
            return Err(VfioDeviceError::UpdateNotAllowedPostBoot);
        }
        slog::info!(
            ctx.logger(),
            "add VFIO device configuration";
            "subsystem" => "vfio_dev_mgr",
            "hostdev_id" => &config.hostdev_id,
            "bdf" => &config.dev_config.bus_slot_func,
        );
        let device_index = self.info_list.insert_or_update(config)?;
        // Handle device hotplug case
        if ctx.is_hotplug {
            slog::info!(
                ctx.logger(),
                "attach VFIO device";
                "subsystem" => "vfio_dev_mgr",
                "hostdev_id" => &config.hostdev_id,
                "bdf" => &config.dev_config.bus_slot_func,
            );
            self.add_device(ctx, config, device_index)?;
        }

        Ok(())
    }

    /// Attach all configured VFIO device to the virtual machine instance.
    pub fn attach_devices(
        &mut self,
        ctx: &mut DeviceOpContext,
    ) -> std::result::Result<(), StartMicroVmError> {
        // create and attach pci root bus
        #[cfg(all(feature = "hotplug", feature = "host-device"))]
        if ctx.pci_hotplug_enabled {
            let _ = self
                .create_pci_manager(
                    ctx.irq_manager.clone(),
                    ctx.io_context.clone(),
                    ctx.res_manager.clone(),
                )
                .map_err(StartMicroVmError::CreateVfioDevice)?;
        }
        for (idx, info) in self.info_list.clone().iter().enumerate() {
            self.create_device(&info.config, ctx, idx)
                .map_err(StartMicroVmError::CreateVfioDevice)?;
        }
        Ok(())
    }

    pub fn remove_device(&mut self, ctx: &mut DeviceOpContext, hostdev_id: &str) -> Result<()> {
        if !cfg!(feature = "hotplug") {
            return Err(VfioDeviceError::UpdateNotAllowedPostBoot);
        }

        slog::info!(
            ctx.logger(),
            "remove VFIO device";
            "subsystem" => "vfio_dev_mgr",
            "hostdev_id" => hostdev_id,
        );
        let device_index = self
            .get_index_of_hostdev_id(hostdev_id)
            .ok_or(VfioDeviceError::InvalidConfig)?;
        let mut info = self
            .info_list
            .remove(device_index)
            .ok_or(VfioDeviceError::InvalidConfig)?;

        self.remove_vfio_device(ctx, &mut info)
    }

    /// prepare to remove device
    pub fn prepare_remove_device(
        &self,
        ctx: &DeviceOpContext,
        hostdev_id: &str,
        result_sender: Sender<Option<i32>>,
    ) -> Result<()> {
        if !cfg!(feature = "hotplug") {
            return Err(VfioDeviceError::UpdateNotAllowedPostBoot);
        }

        slog::info!(
            ctx.logger(),
            "prepare remove VFIO device";
            "subsystem" => "vfio_dev_mgr",
            "hostdev_id" => hostdev_id,
        );

        let device_index = self
            .get_index_of_hostdev_id(hostdev_id)
            .ok_or(VfioDeviceError::InvalidConfig)?;

        let info = &self.info_list[device_index];
        if let Some(dev) = info.device.as_ref() {
            let callback: Option<Box<dyn Fn(UpcallClientResponse) + Send>> =
                Some(Box::new(move |result| match result {
                    UpcallClientResponse::DevMgr(response) => {
                        if let DevMgrResponse::Other(resp) = response {
                            if let Err(e) = result_sender.send(Some(resp.result)) {
                                error!("send upcall result failed, due to {:?}!", e);
                            }
                        }
                    }
                    UpcallClientResponse::UpcallReset => {
                        if let Err(e) = result_sender.send(None) {
                            error!("send upcall result failed, due to {:?}!", e);
                        }
                    }
                    #[allow(unreachable_patterns)]
                    _ => {
                        debug!("this arm should only be triggered under test");
                    }
                }));
            ctx.remove_hotplug_pci_device(dev, callback)
                .map_err(VfioDeviceError::VfioDeviceMgr)?
        }
        Ok(())
    }

    fn remove_vfio_device(
        &mut self,
        ctx: &mut DeviceOpContext,
        info: &mut DeviceConfigInfo<HostDeviceConfig>,
    ) -> Result<()> {
        let device = info.device.take().ok_or(VfioDeviceError::InvalidConfig)?;
        self.remove_pci_vfio_device(&device, ctx)?;
        Ok(())
    }

    /// Start all VFIO devices.
    pub fn start_devices(&mut self, vm_as: &GuestAddressSpaceImpl) -> Result<()> {
        if self.vfio_container.is_some() {
            let vm_memory = vm_as.memory();
            self.register_memory(vm_memory.deref())?;
        }
        Ok(())
    }

    pub(crate) fn get_kvm_dev_fd(&self) -> Result<DeviceFd> {
        let mut kvm_vfio_dev = kvm_bindings::kvm_create_device {
            type_: kvm_bindings::kvm_device_type_KVM_DEV_TYPE_VFIO,
            fd: 0,
            flags: 0,
        };
        let kvm_dev_fd = self
            .vm_fd
            .create_device(&mut kvm_vfio_dev)
            .map_err(|e| VfioDeviceError::IoError(std::io::Error::from_raw_os_error(e.errno())))?;
        Ok(kvm_dev_fd)
    }

    /// Get vfio container object. You should call get_vfio_manager to get vfio_manager Firstly.
    pub fn get_vfio_container(&mut self) -> Result<Arc<VfioContainer>> {
        if let Some(vfio_container) = self.vfio_container.as_ref() {
            Ok(vfio_container.clone())
        } else {
            let kvm_dev_fd = Arc::new(self.get_kvm_dev_fd()?);
            let vfio_container =
                Arc::new(VfioContainer::new(kvm_dev_fd).map_err(VfioDeviceError::VfioIoctlError)?);
            self.vfio_container = Some(vfio_container.clone());

            Ok(vfio_container)
        }
    }

    fn create_device(
        &mut self,
        cfg: &HostDeviceConfig,
        ctx: &mut DeviceOpContext,
        idx: usize,
    ) -> Result<Arc<dyn DeviceIo>> {
        let sysfs_path = Self::build_sysfs_path(cfg)?;
        let device = self.attach_pci_vfio_device(ctx, sysfs_path, &cfg.dev_config)?;
        self.info_list[idx].device = Some(device.clone());
        Ok(device)
    }

    fn add_device(
        &mut self,
        ctx: &mut DeviceOpContext,
        cfg: &mut HostDeviceConfig,
        idx: usize,
    ) -> Result<()> {
        let dev = self.create_device(cfg, ctx, idx)?;
        if self.locked_vm_size == 0 && self.vfio_container.is_some() {
            let vm_as = ctx
                .get_vm_as()
                .map_err(|_| VfioDeviceError::InternalError)?;
            let vm_memory = vm_as.memory();

            self.register_memory(vm_memory.deref())?;
        }
        let slot = ctx
            .insert_hotplug_pci_device(&dev, None)
            .map_err(VfioDeviceError::VfioDeviceMgr)?;

        cfg.dev_config.guest_dev_id = Some(slot);

        Ok(())
    }

    /// Gets the index of the device with the specified `hostdev_id` if it exists in the list.
    fn get_index_of_hostdev_id(&self, id: &str) -> Option<usize> {
        self.info_list
            .iter()
            .position(|info| info.config.id().eq(id))
    }

    /// Register guest memory to the VFIO container.
    ///
    /// # Arguments
    /// * `guest_mem`: guest memory configuration object.
    pub(crate) fn register_memory(&mut self, vm_memory: &GuestMemoryImpl) -> Result<()> {
        for region in vm_memory.iter() {
            self.register_memory_region(region)?;
        }
        Ok(())
    }

    pub(crate) fn register_memory_region(&mut self, region: &GuestRegionMmap) -> Result<()> {
        let gpa = region.start_addr().raw_value();
        let size = region.len();
        let user_addr = region
            .get_host_address(MemoryRegionAddress(0))
            .expect("guest memory region should be mapped and has HVA.")
            as u64;
        let readonly = region.prot() & libc::PROT_WRITE == 0;
        self.register_region(gpa, size, user_addr, readonly)
    }

    pub(crate) fn register_region(
        &mut self,
        iova: u64,
        size: u64,
        user_addr: u64,
        readonly: bool,
    ) -> Result<()> {
        slog::info!(
            self.logger,
            "map guest physical memory";
            "subsystem" => "vfio_dev_mgr",
            "iova" => iova,
            "size" => size,
            "user_addr" => user_addr,
            "readonly" => readonly,
        );
        //FIXME: add readonly flag when related commit is pushed to upstream vfio-ioctls
        self.get_vfio_container()?
            .vfio_dma_map(iova, size, user_addr)
            .map_err(VfioDeviceError::VfioIoctlError)?;
        self.locked_vm_size += size;
        Ok(())
    }

    /// Clear locked size because iommu table is cleared
    pub(crate) fn clear_locked_size(&mut self) {
        self.locked_vm_size = 0;
    }

    pub(crate) fn unregister_region(&mut self, region: &GuestRegionMmap) -> Result<()> {
        let gpa = region.start_addr().raw_value();
        let size = region.len();

        self.get_vfio_container()?
            .vfio_dma_unmap(gpa, size)
            .map_err(VfioDeviceError::VfioIoctlError)?;

        self.locked_vm_size -= size;
        Ok(())
    }

    pub(crate) fn update_memory(&mut self, region: &GuestRegionMmap) -> Result<()> {
        if self.locked_vm_size != 0 {
            self.register_memory_region(region)?;
        }
        Ok(())
    }

    pub(crate) fn build_sysfs_path(cfg: &HostDeviceConfig) -> Result<String> {
        if cfg.sysfs_path.is_empty() {
            let (bdf, domain) = (
                &cfg.dev_config.bus_slot_func,
                cfg.dev_config.host_pci_domain(),
            );
            let len = bdf.split(':').count();
            if len == 0 {
                Err(VfioDeviceError::InvalidConfig)
            } else if len == 2 {
                Ok(format!("/sys/bus/pci/devices/{:04}:{}", domain, bdf))
            } else {
                Ok(format!("/sys/bus/pci/devices/{}", bdf))
            }
        } else {
            Ok(cfg.sysfs_path.clone())
        }
    }

    /// Get all PCI devices' legacy irqs
    pub fn get_pci_legacy_irqs(&self) -> Option<&HashMap<u8, u8>> {
        self.pci_legacy_irqs.as_ref()
    }
}

impl VfioDeviceMgr {
    pub(super) fn attach_pci_vfio_device(
        &mut self,
        ctx: &mut DeviceOpContext,
        sysfs_path: String,
        cfg: &VfioPciDeviceConfig,
    ) -> Result<Arc<dyn DeviceIo>> {
        slog::info!(
            ctx.logger(),
            "attach vfio pci device";
            "subsystem" => "vfio_dev_mgr",
             "host_bdf" => &cfg.bus_slot_func,
        );
        // safe to get pci_manager
        let pci_manager = self.create_pci_manager(
            ctx.irq_manager.clone(),
            ctx.io_context.clone(),
            ctx.res_manager.clone(),
        )?;
        let pci_bus = pci_manager.pci_root_bus();
        let id = pci_manager
            .new_device_id(cfg.guest_dev_id)
            .ok_or(VfioDeviceError::NoResource)?;
        slog::info!(
            ctx.logger(),
            "PCI:{} vfio pci device id: {}, vendor_device: 0x{:x}",
            &sysfs_path, id, cfg.vendor_device_id;
            "subsystem" => "vfio_dev_mgr",
            "guest_bdf" => id,
        );
        if !cfg.valid_vendor_device() {
            return Err(VfioDeviceError::InvalidConfig);
        }
        let vfio_container = self.get_vfio_container()?;
        let vfio_dev = VfioDevice::new(Path::new(&sysfs_path), vfio_container.clone())
            .map_err(VfioDeviceError::VfioIoctlError)?;
        // Use Weak::clone to break cycle reference:
        //
        // reference 1: VfioPciDevice reference to PciBus
        // reference 2: VfioPciDevice -> PciManager -> PciBus -> VfioPciDevice
        let vfio_pci_device = Arc::new(
            VfioPciDevice::create(
                id,
                sysfs_path,
                Arc::downgrade(&pci_bus),
                vfio_dev,
                Arc::downgrade(self.get_pci_manager().unwrap()),
                ctx.vm_fd.clone(),
                cfg.vendor_device_id,
                cfg.clique_id,
                vfio_container,
            )
            .map_err(VfioDeviceError::VfioPciError)?,
        );
        let mut requires = Vec::new();
        vfio_pci_device.get_resource_requirements(&mut requires);
        let vendor_id = vfio_pci_device.vendor_id();
        if vendor_id == VENDOR_NVIDIA && self.nvidia_shared_irq.is_some() {
            requires.retain(|x| !matches!(x, ResourceConstraint::LegacyIrq { irq: _ }));
        }
        let mut resource = ctx
            .res_manager
            .allocate_device_resources(&requires, USE_SHARED_IRQ)
            .or(Err(VfioDeviceError::NoResource))?;
        if vendor_id == VENDOR_NVIDIA {
            if let Some(irq) = self.nvidia_shared_irq {
                resource.append(LegacyIrq(irq));
            } else {
                self.nvidia_shared_irq = resource.get_legacy_irq();
            }
        }
        vfio_pci_device
            .activate(
                Arc::downgrade(&vfio_pci_device) as Weak<dyn DeviceIo>,
                resource,
            )
            .map_err(VfioDeviceError::VfioPciError)?;
        if let Some(irq) = vfio_pci_device.get_assigned_resources().get_legacy_irq() {
            self.pci_legacy_irqs
                .as_mut()
                .map(|v| v.insert(vfio_pci_device.device_id(), irq as u8));
        }
        //  PciBus reference to VfioPciDevice
        pci_bus
            .register_device(vfio_pci_device.clone())
            .map_err(VfioDeviceError::PciError)?;
        Ok(vfio_pci_device)
    }

    fn remove_pci_vfio_device(
        &mut self,
        device: &Arc<dyn DeviceIo>,
        ctx: &mut DeviceOpContext,
    ) -> Result<()> {
        // safe to unwrap because type is decided
        let vfio_pci_device = device
            .as_any()
            .downcast_ref::<VfioPciDevice<PciSystemManager>>()
            .unwrap();

        let device_id = vfio_pci_device.device_id() as u32;

        // safe to unwrap because pci vfio manager is already created
        let _ = self
            .pci_vfio_manager
            .as_mut()
            .unwrap()
            .free_device_id(device_id)
            .ok_or(VfioDeviceError::InvalidDeviceID(device_id))?;

        let resources = vfio_pci_device.get_assigned_resources();
        let vendor_id = vfio_pci_device.vendor_id();
        let filtered_resources = if vendor_id == VENDOR_NVIDIA {
            let mut filtered_resources = DeviceResources::new();
            for resource in resources.get_all_resources() {
                if let Resource::LegacyIrq(_) = resource {
                    continue;
                } else {
                    filtered_resources.append(resource.clone())
                }
            }
            filtered_resources
        } else {
            resources
        };

        ctx.res_manager
            .free_device_resources(&filtered_resources)
            .map_err(VfioDeviceError::FreeDeviceResource)?;

        vfio_pci_device
            .clear_device()
            .map_err(VfioDeviceError::VfioPciError)?;

        Ok(())
    }

    pub(crate) fn create_pci_manager(
        &mut self,
        irq_manager: Arc<KvmIrqManager>,
        io_context: DeviceManagerContext,
        res_manager: Arc<ResourceManager>,
    ) -> Result<&mut Arc<PciSystemManager>> {
        if self.pci_vfio_manager.is_none() {
            let mut mgr = PciSystemManager::new(irq_manager, io_context, res_manager.clone())?;
            let requirements = mgr.resource_requirements();
            let resources = res_manager
                .allocate_device_resources(&requirements, USE_SHARED_IRQ)
                .or(Err(VfioDeviceError::NoResource))?;
            mgr.activate(resources)?;
            self.pci_vfio_manager = Some(Arc::new(mgr));
        }
        Ok(self.pci_vfio_manager.as_mut().unwrap())
    }

    /// Get the PCI manager to support PCI device passthrough
    pub fn get_pci_manager(&mut self) -> Option<&mut Arc<PciSystemManager>> {
        self.pci_vfio_manager.as_mut()
    }
}

#[cfg(all(test, feature = "test-mock"))]
mod tests {
    use kvm_ioctls::Kvm;
    use logger::LOGGER;
    use vm_memory::{GuestAddress, GuestMemoryMmap, MmapRegion};

    use super::*;
    use crate::config_manager::DeviceConfigInfo;
    use crate::test_utils::tests::create_vm_for_test;

    type VfioDeviceInfo = DeviceConfigInfo<VfioDeviceConfigInfo, VfioDeviceError>;

    fn get_vfio_dev_mgr() -> VfioDeviceMgr {
        let kvm = Kvm::new().unwrap();
        let vm_fd = Arc::new(kvm.create_vm().unwrap());
        let logger = Arc::new(LOGGER.new_logger(slog::o!()));
        VfioDeviceMgr::new(vm_fd, &logger)
    }

    #[test]
    fn test_register_memory() {
        let mut mgr = get_vfio_dev_mgr();
        // mock for vfio_dma_map.
        let mut vfio_container = VfioContainer::default();
        vfio_container.vfio_dma_map = true;
        vfio_container.vfio_dma_unmap = true;
        mgr.vfio_container = Some(Arc::new(vfio_container));
        let region_size = 0x1000;
        let region1 =
            GuestRegionMmap::new(MmapRegion::new(region_size).unwrap(), GuestAddress(0x4000))
                .unwrap();
        let region2 =
            GuestRegionMmap::new(MmapRegion::new(region_size).unwrap(), GuestAddress(0xc000))
                .unwrap();
        let regions = vec![region1, region2];
        let gmm = Arc::new(GuestMemoryMmap::from_regions(regions).unwrap());
        assert!(mgr.register_memory(&gmm.clone()).is_ok());
        assert_eq!(mgr.locked_vm_size, region_size as u64 * 2);
        for region in gmm.iter() {
            mgr.unregister_region(region).unwrap();
        }
        assert_eq!(mgr.locked_vm_size, 0);
    }

    #[test]
    fn test_register_region() {
        let kvm = Kvm::new().unwrap();
        let vm_fd = Arc::new(kvm.create_vm().unwrap());
        let logger = Arc::new(LOGGER.new_logger(slog::o!()));
        let mut mgr = VfioDeviceMgr::new(vm_fd, &logger);
        // mock for vfio_dma_map.
        let mut vfio_container = VfioContainer::default();
        vfio_container.vfio_dma_map = true;
        vfio_container.vfio_dma_unmap = true;
        mgr.vfio_container = Some(Arc::new(vfio_container));
        let region_size = 0x400000;
        let region =
            GuestRegionMmap::new(MmapRegion::new(region_size).unwrap(), GuestAddress(0x0000))
                .unwrap();
        let gpa = region.start_addr().raw_value();
        let size = region.len() as u64;
        let user_addr = region.get_host_address(MemoryRegionAddress(0)).unwrap() as u64;
        let readonly = region.prot() & libc::PROT_WRITE == 0;
        mgr.register_region(gpa, size, user_addr, readonly).unwrap();
        assert_eq!(mgr.locked_vm_size, region_size as u64);
        assert!(mgr.unregister_region(&region).is_ok());
        assert_eq!(mgr.locked_vm_size, 0);
    }

    #[test]
    fn test_vfio_attach_pci_vfio_devices() {
        let vm = create_vm_for_test();
        let mut mgr = vm.device_manager.vfio_manager.lock().unwrap();
        let config = VfioDeviceConfigInfo {
            hostdev_id: "hostdev_1".to_string(),
            sysfs_path: "uuid1".to_string(),
            bus_slot_func: "0:0:1".to_string(),
            mode: "pci".to_string(),
            vendor_device_id: 0,
            guest_dev_id: None,
            clique_id: None,
        };
        let mut device_op_ctx = DeviceOpContext::new(
            Some(vm.epoll_manager.clone()),
            &vm.device_manager,
            Some(vm.vm_as().unwrap().clone()),
            vm.address_space.address_space.clone(),
            false,
            None,
            vm.address_space.get_base_to_slot_map(),
            vm.shared_info().clone(),
        );
        // Invalid resources.
        assert!(matches!(
            mgr.attach_pci_vfio_devices(&mut device_op_ctx, &config),
            Err(VfioDeviceError::VfioPciError(_))
        ));
    }
}
