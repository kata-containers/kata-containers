// Copyright (C) 2022 Alibaba Cloud. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Device manager to manage IO devices for a virtual machine.

#[cfg(target_arch = "aarch64")]
use std::collections::HashMap;

use std::io;
use std::sync::{Arc, Mutex, MutexGuard, RwLock};

use arc_swap::ArcSwap;
use dbs_address_space::AddressSpace;
#[cfg(target_arch = "aarch64")]
use dbs_arch::{DeviceType, MMIODeviceInfo};
use dbs_device::device_manager::{Error as IoManagerError, IoManager, IoManagerContext};
#[cfg(target_arch = "aarch64")]
use dbs_device::resources::DeviceResources;
use dbs_device::resources::Resource;
use dbs_device::DeviceIo;
use dbs_interrupt::KvmIrqManager;
use dbs_legacy_devices::ConsoleHandler;
#[cfg(all(feature = "host-device", target_arch = "aarch64"))]
use dbs_pci::PciBusResources;
use dbs_utils::epoll_manager::EpollManager;
use kvm_ioctls::VmFd;

#[cfg(feature = "dbs-virtio-devices")]
use dbs_device::resources::ResourceConstraint;
#[cfg(feature = "dbs-virtio-devices")]
use dbs_virtio_devices as virtio;
#[cfg(feature = "dbs-virtio-devices")]
use dbs_virtio_devices::{
    mmio::{
        MmioV2Device, DRAGONBALL_FEATURE_INTR_USED, DRAGONBALL_FEATURE_PER_QUEUE_NOTIFY,
        DRAGONBALL_MMIO_DOORBELL_SIZE, MMIO_DEFAULT_CFG_SIZE,
    },
    VirtioDevice,
};

#[cfg(feature = "host-device")]
use dbs_pci::VfioPciDevice;
#[cfg(all(feature = "hotplug", feature = "dbs-upcall"))]
use dbs_upcall::{
    DevMgrRequest, DevMgrService, MmioDevRequest, PciDevRequest, UpcallClient, UpcallClientError,
    UpcallClientRequest, UpcallClientResponse,
};
#[cfg(feature = "hotplug")]
use dbs_virtio_devices::vsock::backend::VsockInnerConnector;

use crate::address_space_manager::GuestAddressSpaceImpl;
use crate::api::v1::InstanceInfo;
#[cfg(feature = "host-device")]
use crate::device_manager::vfio_dev_mgr::PciSystemManager;
use crate::error::StartMicroVmError;
use crate::resource_manager::ResourceManager;
use crate::vm::{KernelConfigInfo, Vm, VmConfigInfo};
use crate::IoManagerCached;

/// Virtual machine console device manager.
pub mod console_manager;
/// Console Manager for virtual machines console device.
pub use self::console_manager::ConsoleManager;

mod legacy;
pub use self::legacy::{Error as LegacyDeviceError, LegacyDeviceManager};

#[cfg(target_arch = "aarch64")]
pub use self::legacy::aarch64::{COM1_NAME, COM2_NAME, RTC_NAME};

#[cfg(feature = "virtio-vsock")]
/// Device manager for user-space vsock devices.
pub mod vsock_dev_mgr;
#[cfg(feature = "virtio-vsock")]
use self::vsock_dev_mgr::VsockDeviceMgr;

#[cfg(any(feature = "virtio-blk", feature = "vhost-user-blk"))]
/// virtio-block device manager
pub mod blk_dev_mgr;
#[cfg(any(feature = "virtio-blk", feature = "vhost-user-blk"))]
use self::blk_dev_mgr::BlockDeviceMgr;

#[cfg(feature = "virtio-net")]
/// Device manager for virtio-net devices.
pub mod virtio_net_dev_mgr;
#[cfg(feature = "virtio-net")]
use self::virtio_net_dev_mgr::VirtioNetDeviceMgr;

#[cfg(any(feature = "virtio-fs", feature = "vhost-user-fs"))]
/// virtio-block device manager
pub mod fs_dev_mgr;
#[cfg(any(feature = "virtio-fs", feature = "vhost-user-fs"))]
use self::fs_dev_mgr::FsDeviceMgr;
#[cfg(feature = "virtio-fs")]
mod memory_region_handler;
#[cfg(feature = "virtio-fs")]
pub use self::memory_region_handler::*;

#[cfg(feature = "virtio-mem")]
/// Device manager for virtio-mem devices.
pub mod mem_dev_mgr;
#[cfg(feature = "virtio-mem")]
use self::mem_dev_mgr::MemDeviceMgr;

#[cfg(feature = "virtio-balloon")]
/// Device manager for virtio-balloon devices.
pub mod balloon_dev_mgr;
#[cfg(feature = "virtio-balloon")]
use self::balloon_dev_mgr::BalloonDeviceMgr;

#[cfg(feature = "vhost-net")]
/// Device manager for vhost-net devices.
pub mod vhost_net_dev_mgr;
#[cfg(feature = "vhost-net")]
use self::vhost_net_dev_mgr::VhostNetDeviceMgr;
#[cfg(feature = "host-device")]
/// Device manager for PCI/MMIO VFIO devices.
pub mod vfio_dev_mgr;
#[cfg(feature = "host-device")]
use self::vfio_dev_mgr::VfioDeviceMgr;

#[cfg(feature = "vhost-user-net")]
/// Device manager for vhost-user-net devices.
pub mod vhost_user_net_dev_mgr;
#[cfg(feature = "vhost-user-net")]
use self::vhost_user_net_dev_mgr::VhostUserNetDeviceMgr;

macro_rules! info(
    ($l:expr, $($args:tt)+) => {
        slog::info!($l, $($args)+; slog::o!("subsystem" => "device_manager"))
    };
);

/// Errors related to device manager operations.
#[derive(Debug, thiserror::Error)]
pub enum DeviceMgrError {
    /// Invalid operation.
    #[error("invalid device manager operation")]
    InvalidOperation,

    /// Failed to get device resource.
    #[error("failed to get device assigned resources")]
    GetDeviceResource,

    /// Appending to kernel command line failed.
    #[error("failed to add kernel command line parameter for device: {0}")]
    Cmdline(#[source] linux_loader::cmdline::Error),

    /// Failed to manage console devices.
    #[error(transparent)]
    ConsoleManager(console_manager::ConsoleManagerError),

    /// Failed to create the device.
    #[error("failed to create virtual device: {0}")]
    CreateDevice(#[source] io::Error),

    /// Failed to perform an operation on the bus.
    #[error(transparent)]
    IoManager(IoManagerError),

    /// Failure from legacy device manager.
    #[error(transparent)]
    LegacyManager(legacy::Error),

    #[cfg(feature = "dbs-virtio-devices")]
    /// Error from Virtio subsystem.
    #[error(transparent)]
    Virtio(virtio::Error),

    #[cfg(all(feature = "hotplug", feature = "dbs-upcall"))]
    /// Failed to hotplug the device.
    #[error("failed to hotplug virtual device")]
    HotplugDevice(#[source] UpcallClientError),

    /// Failed to free device resource.
    #[error("failed to free device resources: {0}")]
    ResourceError(#[source] crate::resource_manager::ResourceError),

    #[cfg(feature = "host-device")]
    /// Error from Vfio Pci
    #[error("failed to do vfio pci operation: {0:?}")]
    VfioPci(#[source] dbs_pci::VfioPciError),
}

/// Specialized version of `std::result::Result` for device manager operations.
pub type Result<T> = ::std::result::Result<T, DeviceMgrError>;

/// Type of the dragonball virtio devices.
#[cfg(feature = "dbs-virtio-devices")]
pub type DbsVirtioDevice = Box<
    dyn VirtioDevice<GuestAddressSpaceImpl, virtio_queue::QueueSync, vm_memory::GuestRegionMmap>,
>;

/// Type of the dragonball virtio mmio devices.
#[cfg(feature = "dbs-virtio-devices")]
pub type DbsMmioV2Device =
    MmioV2Device<GuestAddressSpaceImpl, virtio_queue::QueueSync, vm_memory::GuestRegionMmap>;

/// Struct to support transactional operations for device management.
pub struct DeviceManagerTx {
    io_manager: IoManager,
    _io_lock: Arc<Mutex<()>>,
    _guard: MutexGuard<'static, ()>,
}

impl DeviceManagerTx {
    fn new(mgr_ctx: &DeviceManagerContext) -> Self {
        // Do not expect poisoned lock.
        let guard = mgr_ctx.io_lock.lock().unwrap();

        // It's really a heavy burden to carry on a lifetime parameter for MutexGuard.
        // So we play a tricky here that we hold a reference to the Arc<Mutex<()>> and transmute
        // the MutexGuard<'a, ()> to MutexGuard<'static, ()>.
        // It's safe because we hold a reference to the Mutex lock.
        let guard =
            unsafe { std::mem::transmute::<MutexGuard<'_, ()>, MutexGuard<'static, ()>>(guard) };

        DeviceManagerTx {
            io_manager: mgr_ctx.io_manager.load().as_ref().clone(),
            _io_lock: mgr_ctx.io_lock.clone(),
            _guard: guard,
        }
    }
}

/// Operation context for device management.
#[derive(Clone)]
pub struct DeviceManagerContext {
    io_manager: Arc<ArcSwap<IoManager>>,
    io_lock: Arc<Mutex<()>>,
}

impl DeviceManagerContext {
    /// Create a DeviceManagerContext object.
    pub fn new(io_manager: Arc<ArcSwap<IoManager>>, io_lock: Arc<Mutex<()>>) -> Self {
        DeviceManagerContext {
            io_manager,
            io_lock,
        }
    }
}

impl IoManagerContext for DeviceManagerContext {
    type Context = DeviceManagerTx;

    fn begin_tx(&self) -> Self::Context {
        DeviceManagerTx::new(self)
    }

    fn commit_tx(&self, context: Self::Context) {
        self.io_manager.store(Arc::new(context.io_manager));
    }

    fn cancel_tx(&self, context: Self::Context) {
        drop(context);
    }

    fn register_device_io(
        &self,
        ctx: &mut Self::Context,
        device: Arc<dyn DeviceIo>,
        resources: &[Resource],
    ) -> std::result::Result<(), dbs_device::device_manager::Error> {
        ctx.io_manager.register_device_io(device, resources)
    }

    fn unregister_device_io(
        &self,
        ctx: &mut Self::Context,
        resources: &[Resource],
    ) -> std::result::Result<(), dbs_device::device_manager::Error> {
        ctx.io_manager.unregister_device_io(resources)
    }
}

/// Context for device addition/removal operations.
pub struct DeviceOpContext {
    epoll_mgr: Option<EpollManager>,
    io_context: DeviceManagerContext,
    irq_manager: Arc<KvmIrqManager>,
    res_manager: Arc<ResourceManager>,
    vm_fd: Arc<VmFd>,
    vm_as: Option<GuestAddressSpaceImpl>,
    address_space: Option<AddressSpace>,
    logger: slog::Logger,
    is_hotplug: bool,
    #[cfg(all(feature = "hotplug", feature = "host-device"))]
    pci_hotplug_enabled: bool,

    #[cfg(all(feature = "hotplug", feature = "dbs-upcall"))]
    upcall_client: Option<Arc<UpcallClient<DevMgrService>>>,
    #[cfg(feature = "dbs-virtio-devices")]
    virtio_devices: Vec<Arc<DbsMmioV2Device>>,
    #[cfg(feature = "host-device")]
    vfio_manager: Option<Arc<Mutex<VfioDeviceMgr>>>,
    vm_config: Option<VmConfigInfo>,
    shared_info: Arc<RwLock<InstanceInfo>>,
}

impl DeviceOpContext {
    pub(crate) fn new(
        epoll_mgr: Option<EpollManager>,
        device_mgr: &DeviceManager,
        vm_as: Option<GuestAddressSpaceImpl>,
        address_space: Option<AddressSpace>,
        is_hotplug: bool,
        vm_config: Option<VmConfigInfo>,
        shared_info: Arc<RwLock<InstanceInfo>>,
    ) -> Self {
        let irq_manager = device_mgr.irq_manager.clone();
        let res_manager = device_mgr.res_manager.clone();

        let vm_fd = device_mgr.vm_fd.clone();
        let io_context = DeviceManagerContext {
            io_manager: device_mgr.io_manager.clone(),
            io_lock: device_mgr.io_lock.clone(),
        };
        let logger = device_mgr.logger.new(slog::o!());

        #[cfg(all(feature = "hotplug", feature = "host-device"))]
        let pci_hotplug_enabled = vm_config
            .clone()
            .map(|c| c.pci_hotplug_enabled)
            .unwrap_or(false);

        DeviceOpContext {
            epoll_mgr,
            io_context,
            irq_manager,
            res_manager,
            vm_fd,
            vm_as,
            address_space,
            logger,
            is_hotplug,
            #[cfg(all(feature = "hotplug", feature = "host-device"))]
            pci_hotplug_enabled,
            #[cfg(all(feature = "hotplug", feature = "dbs-upcall"))]
            upcall_client: None,
            #[cfg(feature = "dbs-virtio-devices")]
            virtio_devices: Vec::new(),
            vm_config,
            shared_info,
            #[cfg(feature = "host-device")]
            vfio_manager: None,
        }
    }

    pub(crate) fn create_boot_ctx(vm: &Vm, epoll_mgr: Option<EpollManager>) -> Self {
        Self::new(
            epoll_mgr,
            vm.device_manager(),
            None,
            None,
            false,
            Some(vm.vm_config().clone()),
            vm.shared_info().clone(),
        )
    }

    pub(crate) fn get_vm_as(&self) -> Result<GuestAddressSpaceImpl> {
        match self.vm_as.as_ref() {
            Some(v) => Ok(v.clone()),
            None => Err(DeviceMgrError::InvalidOperation),
        }
    }

    pub(crate) fn get_vm_config(&self) -> Result<VmConfigInfo> {
        match self.vm_config.as_ref() {
            Some(v) => Ok(v.clone()),
            None => Err(DeviceMgrError::InvalidOperation),
        }
    }

    pub(crate) fn get_address_space(&self) -> Result<AddressSpace> {
        match self.address_space.as_ref() {
            Some(v) => Ok(v.clone()),
            None => Err(DeviceMgrError::InvalidOperation),
        }
    }

    pub(crate) fn get_epoll_mgr(&self) -> Result<EpollManager> {
        match self.epoll_mgr.as_ref() {
            Some(v) => Ok(v.clone()),
            None => Err(DeviceMgrError::InvalidOperation),
        }
    }

    pub(crate) fn logger(&self) -> &slog::Logger {
        &self.logger
    }

    #[allow(unused_variables)]
    fn generate_kernel_boot_args(&mut self, kernel_config: &mut KernelConfigInfo) -> Result<()> {
        if self.is_hotplug {
            return Err(DeviceMgrError::InvalidOperation);
        }

        #[cfg(feature = "dbs-virtio-devices")]
        {
            let cmdline = kernel_config.kernel_cmdline_mut();

            for device in self.virtio_devices.iter() {
                let (mmio_base, mmio_size, irq) = DeviceManager::get_virtio_device_info(device)?;

                // as per doc, [virtio_mmio.]device=<size>@<baseaddr>:<irq> needs to be appended
                // to kernel commandline for virtio mmio devices to get recognized
                // the size parameter has to be transformed to KiB, so dividing hexadecimal value in
                // bytes to 1024; further, the '{}' formatting rust construct will automatically
                // transform it to decimal
                cmdline
                    .insert(
                        "virtio_mmio.device",
                        &format!("{}K@0x{:08x}:{}", mmio_size / 1024, mmio_base, irq),
                    )
                    .map_err(DeviceMgrError::Cmdline)?;
            }
        }

        Ok(())
    }

    #[cfg(target_arch = "aarch64")]
    fn generate_virtio_device_info(&self) -> Result<HashMap<(DeviceType, String), MMIODeviceInfo>> {
        let mut dev_info = HashMap::new();
        #[cfg(feature = "dbs-virtio-devices")]
        for device in self.virtio_devices.iter() {
            let (mmio_base, mmio_size, irq) = DeviceManager::get_virtio_mmio_device_info(device)?;
            let dev_type;
            let device_id;
            if let Some(mmiov2_device) = device.as_any().downcast_ref::<DbsMmioV2Device>() {
                dev_type = mmiov2_device.get_device_type();
                device_id = None;
            } else {
                return Err(DeviceMgrError::InvalidOperation);
            }
            dev_info.insert(
                (
                    DeviceType::Virtio(dev_type),
                    format!("virtio-{}@0x{:08x?}", dev_type, mmio_base),
                ),
                MMIODeviceInfo::new(mmio_base, mmio_size, vec![irq], device_id),
            );
        }
        Ok(dev_info)
    }
}

#[cfg(all(feature = "hotplug", not(feature = "dbs-upcall")))]
impl DeviceOpContext {
    pub(crate) fn insert_hotplug_mmio_device(
        &self,
        _dev: &Arc<DbsMmioV2Device>,
        _callback: Option<()>,
    ) -> Result<()> {
        Err(DeviceMgrError::InvalidOperation)
    }

    pub(crate) fn remove_hotplug_mmio_device(
        &self,
        _dev: &Arc<DbsMmioV2Device>,
        _callback: Option<()>,
    ) -> Result<()> {
        Err(DeviceMgrError::InvalidOperation)
    }
}

#[cfg(feature = "host-device")]
impl DeviceOpContext {
    pub(crate) fn set_vfio_manager(&mut self, vfio_device_mgr: Arc<Mutex<VfioDeviceMgr>>) {
        self.vfio_manager = Some(vfio_device_mgr);
    }
}

#[cfg(all(feature = "hotplug", feature = "dbs-upcall"))]
impl DeviceOpContext {
    pub(crate) fn create_hotplug_ctx(vm: &Vm, epoll_mgr: Option<EpollManager>) -> Self {
        let vm_as = vm.vm_as().expect("VM should have memory ready").clone();

        let mut ctx = Self::new(
            epoll_mgr,
            vm.device_manager(),
            Some(vm_as),
            vm.vm_address_space().cloned(),
            true,
            Some(vm.vm_config().clone()),
            vm.shared_info().clone(),
        );
        ctx.upcall_client = vm.upcall_client().clone();
        ctx
    }

    fn call_hotplug_device(
        &self,
        req: DevMgrRequest,
        callback: Option<Box<dyn Fn(UpcallClientResponse) + Send>>,
    ) -> Result<()> {
        if let Some(upcall_client) = self.upcall_client.as_ref() {
            if let Some(cb) = callback {
                upcall_client
                    .send_request(UpcallClientRequest::DevMgr(req), cb)
                    .map_err(DeviceMgrError::HotplugDevice)?;
            } else {
                upcall_client
                    .send_request_without_result(UpcallClientRequest::DevMgr(req))
                    .map_err(DeviceMgrError::HotplugDevice)?;
            }
            Ok(())
        } else {
            Err(DeviceMgrError::InvalidOperation)
        }
    }

    pub(crate) fn insert_hotplug_mmio_device(
        &self,
        dev: &Arc<DbsMmioV2Device>,
        callback: Option<Box<dyn Fn(UpcallClientResponse) + Send>>,
    ) -> Result<()> {
        if !self.is_hotplug {
            return Err(DeviceMgrError::InvalidOperation);
        }

        let (mmio_base, mmio_size, mmio_irq) = DeviceManager::get_virtio_device_info(dev)?;
        let req = DevMgrRequest::AddMmioDev(MmioDevRequest {
            mmio_base,
            mmio_size,
            mmio_irq,
        });

        self.call_hotplug_device(req, callback)
    }

    pub(crate) fn remove_hotplug_mmio_device(
        &self,
        dev: &Arc<DbsMmioV2Device>,
        callback: Option<Box<dyn Fn(UpcallClientResponse) + Send>>,
    ) -> Result<()> {
        if !self.is_hotplug {
            return Err(DeviceMgrError::InvalidOperation);
        }
        let (mmio_base, mmio_size, mmio_irq) = DeviceManager::get_virtio_device_info(dev)?;
        let req = DevMgrRequest::DelMmioDev(MmioDevRequest {
            mmio_base,
            mmio_size,
            mmio_irq,
        });

        self.call_hotplug_device(req, callback)
    }

    #[cfg(feature = "host-device")]
    pub(crate) fn insert_hotplug_pci_device(
        &self,
        dev: &Arc<dyn DeviceIo>,
        callback: Option<Box<dyn Fn(UpcallClientResponse) + Send>>,
    ) -> Result<u8> {
        if !self.is_hotplug || !self.pci_hotplug_enabled {
            return Err(DeviceMgrError::InvalidOperation);
        }

        let (busno, devfn) = DeviceManager::get_pci_device_info(dev)?;
        let req = DevMgrRequest::AddPciDev(PciDevRequest { busno, devfn });

        self.call_hotplug_device(req, callback)?;

        // Extract the slot number from devfn
        // Right shift by 3 to remove function bits (2:0) and
        // align slot bits (7:3) to the least significant position
        Ok(devfn >> 3)
    }

    #[cfg(feature = "host-device")]
    pub(crate) fn remove_hotplug_pci_device(
        &self,
        dev: &Arc<dyn DeviceIo>,
        callback: Option<Box<dyn Fn(UpcallClientResponse) + Send>>,
    ) -> Result<()> {
        if !self.is_hotplug || !self.pci_hotplug_enabled {
            return Err(DeviceMgrError::InvalidOperation);
        }
        let (busno, devfn) = DeviceManager::get_pci_device_info(dev)?;
        let req = DevMgrRequest::DelPciDev(PciDevRequest { busno, devfn });

        self.call_hotplug_device(req, callback)
    }
}

#[cfg(all(feature = "hotplug", feature = "acpi"))]
impl DeviceOpContext {
    // TODO: We will implement this when we develop ACPI virtualization
}

/// Device manager for virtual machines, which manages all device for a virtual machine.
pub struct DeviceManager {
    io_manager: Arc<ArcSwap<IoManager>>,
    io_lock: Arc<Mutex<()>>,
    irq_manager: Arc<KvmIrqManager>,
    res_manager: Arc<ResourceManager>,
    vm_fd: Arc<VmFd>,
    pub(crate) logger: slog::Logger,
    pub(crate) shared_info: Arc<RwLock<InstanceInfo>>,
    pub(crate) con_manager: ConsoleManager,
    pub(crate) legacy_manager: Option<LegacyDeviceManager>,
    #[cfg(target_arch = "aarch64")]
    pub(crate) mmio_device_info: HashMap<(DeviceType, String), MMIODeviceInfo>,
    #[cfg(feature = "virtio-vsock")]
    pub(crate) vsock_manager: VsockDeviceMgr,

    #[cfg(any(feature = "virtio-blk", feature = "vhost-user-blk"))]
    // If there is a Root Block Device, this should be added as the first element of the list.
    // This is necessary because we want the root to always be mounted on /dev/vda.
    pub(crate) block_manager: BlockDeviceMgr,

    #[cfg(feature = "virtio-net")]
    pub(crate) virtio_net_manager: VirtioNetDeviceMgr,

    #[cfg(any(feature = "virtio-fs", feature = "vhost-user-fs"))]
    fs_manager: Arc<Mutex<FsDeviceMgr>>,

    #[cfg(feature = "virtio-mem")]
    pub(crate) mem_manager: MemDeviceMgr,

    #[cfg(feature = "virtio-balloon")]
    pub(crate) balloon_manager: BalloonDeviceMgr,

    #[cfg(feature = "vhost-net")]
    vhost_net_manager: VhostNetDeviceMgr,

    #[cfg(feature = "vhost-user-net")]
    vhost_user_net_manager: VhostUserNetDeviceMgr,
    #[cfg(feature = "host-device")]
    pub(crate) vfio_manager: Arc<Mutex<VfioDeviceMgr>>,
}

impl DeviceManager {
    /// Create a new device manager instance.
    pub fn new(
        vm_fd: Arc<VmFd>,
        res_manager: Arc<ResourceManager>,
        epoll_manager: EpollManager,
        logger: &slog::Logger,
        shared_info: Arc<RwLock<InstanceInfo>>,
    ) -> Self {
        DeviceManager {
            io_manager: Arc::new(ArcSwap::new(Arc::new(IoManager::new()))),
            io_lock: Arc::new(Mutex::new(())),
            irq_manager: Arc::new(KvmIrqManager::new(vm_fd.clone())),
            res_manager,
            vm_fd: vm_fd.clone(),
            logger: logger.new(slog::o!()),
            shared_info,

            con_manager: ConsoleManager::new(epoll_manager, logger),
            legacy_manager: None,
            #[cfg(target_arch = "aarch64")]
            mmio_device_info: HashMap::new(),
            #[cfg(feature = "virtio-vsock")]
            vsock_manager: VsockDeviceMgr::default(),
            #[cfg(any(feature = "virtio-blk", feature = "vhost-user-blk"))]
            block_manager: BlockDeviceMgr::default(),
            #[cfg(feature = "virtio-net")]
            virtio_net_manager: VirtioNetDeviceMgr::default(),
            #[cfg(any(feature = "virtio-fs", feature = "vhost-user-fs"))]
            fs_manager: Arc::new(Mutex::new(FsDeviceMgr::default())),
            #[cfg(feature = "virtio-mem")]
            mem_manager: MemDeviceMgr::default(),
            #[cfg(feature = "virtio-balloon")]
            balloon_manager: BalloonDeviceMgr::default(),
            #[cfg(feature = "vhost-net")]
            vhost_net_manager: VhostNetDeviceMgr::default(),
            #[cfg(feature = "vhost-user-net")]
            vhost_user_net_manager: VhostUserNetDeviceMgr::default(),
            #[cfg(feature = "host-device")]
            vfio_manager: Arc::new(Mutex::new(VfioDeviceMgr::new(vm_fd, logger))),
        }
    }

    /// Get the underlying IoManager to dispatch IO read/write requests.
    pub fn io_manager(&self) -> IoManagerCached {
        IoManagerCached::new(self.io_manager.clone())
    }

    /// Create the underline interrupt manager for the device manager.
    pub fn create_interrupt_manager(&mut self) -> Result<()> {
        self.irq_manager
            .initialize()
            .map_err(DeviceMgrError::CreateDevice)
    }

    /// Get the underlying logger.
    pub fn logger(&self) -> &slog::Logger {
        &self.logger
    }

    /// Create legacy devices associted virtual machine
    #[allow(unused_variables)]
    pub fn create_legacy_devices(
        &mut self,
        ctx: &mut DeviceOpContext,
    ) -> std::result::Result<(), StartMicroVmError> {
        #[cfg(any(
            target_arch = "x86_64",
            all(target_arch = "aarch64", feature = "dbs-virtio-devices")
        ))]
        {
            let mut tx = ctx.io_context.begin_tx();
            let legacy_manager;

            #[cfg(target_arch = "x86_64")]
            {
                legacy_manager = LegacyDeviceManager::create_manager(
                    &mut tx.io_manager,
                    Some(self.vm_fd.clone()),
                );
            }

            #[cfg(target_arch = "aarch64")]
            #[cfg(feature = "dbs-virtio-devices")]
            {
                let resources = self.get_legacy_resources()?;
                legacy_manager = LegacyDeviceManager::create_manager(
                    &mut tx.io_manager,
                    Some(self.vm_fd.clone()),
                    &resources,
                );
            }

            match legacy_manager {
                Ok(v) => {
                    self.legacy_manager = Some(v);
                    ctx.io_context.commit_tx(tx);
                }
                Err(e) => {
                    ctx.io_context.cancel_tx(tx);
                    return Err(StartMicroVmError::LegacyDevice(e));
                }
            }
        }

        Ok(())
    }

    /// Init legacy devices with logger stream in associted virtual machine
    pub fn init_legacy_devices(
        &mut self,
        dmesg_fifo: Option<Box<dyn io::Write + Send>>,
        com1_sock_path: Option<String>,
        _ctx: &mut DeviceOpContext,
    ) -> std::result::Result<(), StartMicroVmError> {
        // Connect serial ports to the console and dmesg_fifo.
        self.set_guest_kernel_log_stream(dmesg_fifo)
            .map_err(|_| StartMicroVmError::EventFd)?;

        info!(self.logger, "init console path: {:?}", com1_sock_path);
        if let Some(legacy_manager) = self.legacy_manager.as_ref() {
            let com1 = legacy_manager.get_com1_serial();
            if let Some(path) = com1_sock_path {
                self.con_manager
                    .create_socket_console(com1, path)
                    .map_err(StartMicroVmError::DeviceManager)?;
            } else {
                self.con_manager
                    .create_stdio_console(com1)
                    .map_err(StartMicroVmError::DeviceManager)?;
            }
        }

        Ok(())
    }

    /// Set the stream for guest kernel log.
    ///
    /// Note: com2 is used for guest kernel logging.
    /// TODO: check whether it works with aarch64.
    pub fn set_guest_kernel_log_stream(
        &self,
        stream: Option<Box<dyn io::Write + Send>>,
    ) -> std::result::Result<(), io::Error> {
        if let Some(legacy) = self.legacy_manager.as_ref() {
            legacy
                .get_com2_serial()
                .lock()
                .unwrap()
                .set_output_stream(stream);
        }
        Ok(())
    }

    /// Reset the console into canonical mode.
    pub fn reset_console(&self) -> Result<()> {
        self.con_manager.reset_console()
    }

    /// Create all registered devices when booting the associated virtual machine.
    pub fn create_devices(
        &mut self,
        vm_as: GuestAddressSpaceImpl,
        epoll_mgr: EpollManager,
        kernel_config: &mut KernelConfigInfo,
        dmesg_fifo: Option<Box<dyn io::Write + Send>>,
        address_space: Option<&AddressSpace>,
        vm_config: &VmConfigInfo,
    ) -> std::result::Result<(), StartMicroVmError> {
        let mut ctx = DeviceOpContext::new(
            Some(epoll_mgr),
            self,
            Some(vm_as),
            address_space.cloned(),
            false,
            Some(vm_config.clone()),
            self.shared_info.clone(),
        );

        let com1_sock_path = vm_config.serial_path.clone();

        self.create_legacy_devices(&mut ctx)?;
        self.init_legacy_devices(dmesg_fifo, com1_sock_path, &mut ctx)?;

        #[cfg(any(feature = "virtio-blk", feature = "vhost-user-blk"))]
        self.block_manager
            .attach_devices(&mut ctx)
            .map_err(StartMicroVmError::BlockDeviceError)?;

        #[cfg(any(feature = "virtio-fs", feature = "vhost-user-fs"))]
        {
            let mut fs_manager = self.fs_manager.lock().unwrap();
            fs_manager
                .attach_devices(&mut ctx)
                .map_err(StartMicroVmError::FsDeviceError)?;
        }

        #[cfg(feature = "virtio-net")]
        self.virtio_net_manager
            .attach_devices(&mut ctx)
            .map_err(StartMicroVmError::VirtioNetDeviceError)?;

        #[cfg(feature = "virtio-vsock")]
        self.vsock_manager.attach_devices(&mut ctx)?;

        #[cfg(any(feature = "virtio-blk", feature = "vhost-user-blk"))]
        self.block_manager
            .generate_kernel_boot_args(kernel_config)
            .map_err(StartMicroVmError::DeviceManager)?;

        #[cfg(feature = "vhost-net")]
        self.vhost_net_manager
            .attach_devices(&mut ctx)
            .map_err(StartMicroVmError::VhostNetDeviceError)?;

        #[cfg(feature = "vhost-user-net")]
        self.vhost_user_net_manager
            .attach_devices(&mut ctx)
            .map_err(StartMicroVmError::VhostUserNetDeviceError)?;

        #[cfg(feature = "host-device")]
        {
            // It is safe bacause we don't expect poison lock.
            let mut vfio_manager = self.vfio_manager.lock().unwrap();
            vfio_manager.attach_devices(&mut ctx)?;
            ctx.set_vfio_manager(self.vfio_manager.clone())
        }

        // Ensure that all devices are attached before kernel boot args are
        // generated.
        ctx.generate_kernel_boot_args(kernel_config)
            .map_err(StartMicroVmError::DeviceManager)?;

        #[cfg(target_arch = "aarch64")]
        {
            let dev_info = ctx
                .generate_virtio_device_info()
                .map_err(StartMicroVmError::DeviceManager)?;
            self.mmio_device_info.extend(dev_info);
        }

        Ok(())
    }

    /// Start all registered devices when booting the associated virtual machine.
    pub fn start_devices(
        &mut self,
        vm_as: &GuestAddressSpaceImpl,
    ) -> std::result::Result<(), StartMicroVmError> {
        // It is safe because we don't expect poison lock.
        #[cfg(feature = "host-device")]
        self.vfio_manager
            .lock()
            .unwrap()
            .start_devices(vm_as)
            .map_err(StartMicroVmError::RegisterDMAAddress)?;
        Ok(())
    }

    /// Remove all devices when shutdown the associated virtual machine
    pub fn remove_devices(
        &mut self,
        vm_as: GuestAddressSpaceImpl,
        epoll_mgr: EpollManager,
        address_space: Option<&AddressSpace>,
    ) -> Result<()> {
        // create context for removing devices
        let mut ctx = DeviceOpContext::new(
            Some(epoll_mgr),
            self,
            Some(vm_as),
            address_space.cloned(),
            true,
            None,
            self.shared_info.clone(),
        );

        #[cfg(feature = "virtio-blk")]
        self.block_manager.remove_devices(&mut ctx)?;
        // FIXME: To acquire the full abilities for gracefully removing
        // virtio-net and virtio-vsock devices, updating dragonball-sandbox
        // is required.
        #[cfg(feature = "virtio-net")]
        self.virtio_net_manager.remove_devices(&mut ctx)?;
        #[cfg(feature = "virtio-vsock")]
        self.vsock_manager.remove_devices(&mut ctx)?;

        #[cfg(feature = "vhost-net")]
        self.vhost_net_manager.remove_devices(&mut ctx)?;

        Ok(())
    }
}

#[cfg(target_arch = "x86_64")]
impl DeviceManager {
    /// Get the underlying eventfd for vm exit notification.
    pub fn get_reset_eventfd(&self) -> Result<vmm_sys_util::eventfd::EventFd> {
        if let Some(legacy) = self.legacy_manager.as_ref() {
            legacy
                .get_reset_eventfd()
                .map_err(DeviceMgrError::LegacyManager)
        } else {
            Err(DeviceMgrError::LegacyManager(legacy::Error::EventFd(
                io::Error::from_raw_os_error(libc::ENOENT),
            )))
        }
    }
}

#[cfg(target_arch = "aarch64")]
impl DeviceManager {
    /// Return mmio device info for FDT build.
    pub fn get_mmio_device_info(&self) -> Option<&HashMap<(DeviceType, String), MMIODeviceInfo>> {
        Some(&self.mmio_device_info)
    }

    #[cfg(feature = "dbs-virtio-devices")]
    fn get_legacy_resources(
        &mut self,
    ) -> std::result::Result<HashMap<String, DeviceResources>, StartMicroVmError> {
        let mut resources = HashMap::new();
        let legacy_devices = vec![
            (DeviceType::Serial, String::from(COM1_NAME)),
            (DeviceType::Serial, String::from(COM2_NAME)),
            (DeviceType::RTC, String::from(RTC_NAME)),
        ];

        for (device_type, device_id) in legacy_devices {
            let res = self.allocate_mmio_device_resource()?;
            self.add_mmio_device_info(&res, device_type, device_id.clone(), None);
            resources.insert(device_id.clone(), res);
        }

        Ok(resources)
    }

    fn mmio_device_info_to_resources(
        &self,
        key: &(DeviceType, String),
    ) -> std::result::Result<DeviceResources, StartMicroVmError> {
        self.mmio_device_info
            .get(key)
            .map(|info| {
                let mut resources = DeviceResources::new();
                resources.append(Resource::LegacyIrq(info.irqs[0]));
                resources.append(Resource::MmioAddressRange {
                    base: info.base,
                    size: info.size,
                });
                resources
            })
            .ok_or(StartMicroVmError::DeviceManager(
                DeviceMgrError::GetDeviceResource,
            ))
    }

    #[cfg(feature = "dbs-virtio-devices")]
    fn allocate_mmio_device_resource(
        &self,
    ) -> std::result::Result<DeviceResources, StartMicroVmError> {
        let requests = vec![
            ResourceConstraint::MmioAddress {
                range: None,
                align: MMIO_DEFAULT_CFG_SIZE,
                size: MMIO_DEFAULT_CFG_SIZE,
            },
            ResourceConstraint::LegacyIrq { irq: None },
        ];

        self.res_manager
            .allocate_device_resources(&requests, false)
            .map_err(StartMicroVmError::AllocateResource)
    }

    fn add_mmio_device_info(
        &mut self,
        resource: &DeviceResources,
        device_type: DeviceType,
        device_id: String,
        msi_device_id: Option<u32>,
    ) {
        let (base, size) = resource.get_mmio_address_ranges()[0];
        let irq = resource.get_legacy_irq().unwrap();
        self.mmio_device_info.insert(
            (device_type, device_id),
            MMIODeviceInfo::new(base, size, vec![irq], msi_device_id),
        );
    }

    #[cfg(feature = "dbs-virtio-devices")]
    fn get_virtio_mmio_device_info(device: &Arc<DbsMmioV2Device>) -> Result<(u64, u64, u32)> {
        let resources = device.get_assigned_resources();
        let irq = resources
            .get_legacy_irq()
            .ok_or(DeviceMgrError::GetDeviceResource)?;

        if let Some(mmio_dev) = device.as_any().downcast_ref::<DbsMmioV2Device>() {
            if let Resource::MmioAddressRange { base, size } = mmio_dev.get_mmio_cfg_res() {
                return Ok((base, size, irq));
            }
        }

        Err(DeviceMgrError::GetDeviceResource)
    }

    /// Get pci bus resources for creating fdt.
    #[cfg(feature = "host-device")]
    pub fn get_pci_bus_resources(&self) -> Option<PciBusResources> {
        let mut vfio_dev_mgr = self.vfio_manager.lock().unwrap();
        let vfio_pci_mgr = vfio_dev_mgr.get_pci_manager();
        vfio_pci_mgr.as_ref()?;
        let pci_manager = vfio_pci_mgr.unwrap();
        let ecam_space = pci_manager.get_ecam_space();
        let bar_space = pci_manager.get_bar_space();
        Some(PciBusResources {
            ecam_space,
            bar_space,
        })
    }
}

#[cfg(feature = "dbs-virtio-devices")]
impl DeviceManager {
    fn get_virtio_device_info(device: &Arc<DbsMmioV2Device>) -> Result<(u64, u64, u32)> {
        let resources = device.get_assigned_resources();
        let irq = resources
            .get_legacy_irq()
            .ok_or(DeviceMgrError::GetDeviceResource)?;
        let mmio_address_range = device.get_trapped_io_resources().get_mmio_address_ranges();

        // Assume the first MMIO region is virtio configuration region.
        // Virtio-fs needs to pay attention to this assumption.
        if let Some(range) = mmio_address_range.into_iter().next() {
            Ok((range.0, range.1, irq))
        } else {
            Err(DeviceMgrError::GetDeviceResource)
        }
    }

    /// Create an Virtio MMIO transport layer device for the virtio backend device.
    pub fn create_mmio_virtio_device(
        device: DbsVirtioDevice,
        ctx: &mut DeviceOpContext,
        use_shared_irq: bool,
        use_generic_irq: bool,
    ) -> std::result::Result<Arc<DbsMmioV2Device>, DeviceMgrError> {
        let features = DRAGONBALL_FEATURE_INTR_USED | DRAGONBALL_FEATURE_PER_QUEUE_NOTIFY;
        DeviceManager::create_mmio_virtio_device_with_features(
            device,
            ctx,
            Some(features),
            use_shared_irq,
            use_generic_irq,
        )
    }

    /// Create an Virtio MMIO transport layer device for the virtio backend device with configure
    /// change notification enabled.
    pub fn create_mmio_virtio_device_with_device_change_notification(
        device: DbsVirtioDevice,
        ctx: &mut DeviceOpContext,
        use_shared_irq: bool,
        use_generic_irq: bool,
    ) -> std::result::Result<Arc<DbsMmioV2Device>, DeviceMgrError> {
        let features = DRAGONBALL_FEATURE_PER_QUEUE_NOTIFY;
        DeviceManager::create_mmio_virtio_device_with_features(
            device,
            ctx,
            Some(features),
            use_shared_irq,
            use_generic_irq,
        )
    }

    /// Create an Virtio MMIO transport layer device for the virtio backend device with specified
    /// features.
    pub fn create_mmio_virtio_device_with_features(
        device: DbsVirtioDevice,
        ctx: &mut DeviceOpContext,
        features: Option<u32>,
        use_shared_irq: bool,
        use_generic_irq: bool,
    ) -> std::result::Result<Arc<DbsMmioV2Device>, DeviceMgrError> {
        // Every emulated Virtio MMIO device needs a 4K configuration space,
        // and another 4K space for per queue notification.
        const MMIO_ADDRESS_DEFAULT: ResourceConstraint = ResourceConstraint::MmioAddress {
            range: None,
            align: 0,
            size: MMIO_DEFAULT_CFG_SIZE + DRAGONBALL_MMIO_DOORBELL_SIZE,
        };
        let mut requests = vec![MMIO_ADDRESS_DEFAULT];
        device.get_resource_requirements(&mut requests, use_generic_irq);
        let resources = ctx
            .res_manager
            .allocate_device_resources(&requests, use_shared_irq)
            .map_err(|_| DeviceMgrError::GetDeviceResource)?;

        let virtio_dev = match MmioV2Device::new(
            ctx.vm_fd.clone(),
            ctx.get_vm_as()?,
            ctx.get_address_space()?,
            ctx.irq_manager.clone(),
            device,
            resources,
            features,
        ) {
            Ok(d) => d,
            Err(e) => return Err(DeviceMgrError::Virtio(e)),
        };

        Self::register_mmio_virtio_device(Arc::new(virtio_dev), ctx)
    }

    /// Teardown the Virtio MMIO transport layer device associated with the virtio backend device.
    pub fn destroy_mmio_virtio_device(
        device: Arc<dyn DeviceIo>,
        ctx: &mut DeviceOpContext,
    ) -> std::result::Result<(), DeviceMgrError> {
        Self::destroy_mmio_device(device.clone(), ctx)?;

        let mmio_dev = device
            .as_any()
            .downcast_ref::<DbsMmioV2Device>()
            .ok_or(DeviceMgrError::InvalidOperation)?;

        mmio_dev.remove();

        Ok(())
    }

    fn destroy_mmio_device(
        device: Arc<dyn DeviceIo>,
        ctx: &mut DeviceOpContext,
    ) -> std::result::Result<(), DeviceMgrError> {
        // unregister IoManager
        Self::deregister_mmio_virtio_device(&device, ctx)?;

        // unregister Resource manager
        let resources = device.get_assigned_resources();
        ctx.res_manager
            .free_device_resources(&resources)
            .map_err(DeviceMgrError::ResourceError)?;

        Ok(())
    }

    /// Create an Virtio MMIO transport layer device for the virtio backend device.
    pub fn register_mmio_virtio_device(
        device: Arc<DbsMmioV2Device>,
        ctx: &mut DeviceOpContext,
    ) -> std::result::Result<Arc<DbsMmioV2Device>, DeviceMgrError> {
        let (mmio_base, mmio_size, irq) = Self::get_virtio_device_info(&device)?;
        info!(
            ctx.logger(),
            "create virtio mmio device 0x{:x}@0x{:x}, irq: 0x{:x}", mmio_size, mmio_base, irq
        );
        let resources = device.get_trapped_io_resources();

        let mut tx = ctx.io_context.begin_tx();
        if let Err(e) = ctx
            .io_context
            .register_device_io(&mut tx, device.clone(), &resources)
        {
            ctx.io_context.cancel_tx(tx);
            Err(DeviceMgrError::IoManager(e))
        } else {
            ctx.virtio_devices.push(device.clone());
            ctx.io_context.commit_tx(tx);
            Ok(device)
        }
    }

    /// Deregister a Virtio MMIO device from IoManager
    pub fn deregister_mmio_virtio_device(
        device: &Arc<dyn DeviceIo>,
        ctx: &mut DeviceOpContext,
    ) -> std::result::Result<(), DeviceMgrError> {
        let resources = device.get_trapped_io_resources();
        info!(
            ctx.logger(),
            "unregister mmio virtio device: {:?}", resources
        );
        let mut tx = ctx.io_context.begin_tx();
        if let Err(e) = ctx.io_context.unregister_device_io(&mut tx, &resources) {
            ctx.io_context.cancel_tx(tx);
            Err(DeviceMgrError::IoManager(e))
        } else {
            ctx.io_context.commit_tx(tx);
            Ok(())
        }
    }

    #[cfg(feature = "host-device")]
    fn get_pci_device_info(device: &Arc<dyn DeviceIo>) -> Result<(u8, u8)> {
        if let Some(pci_dev) = device
            .as_any()
            .downcast_ref::<VfioPciDevice<PciSystemManager>>()
        {
            // reference from kernel: include/uapi/linux/pci.h
            let busno = pci_dev.bus_id().map_err(DeviceMgrError::VfioPci)?;
            let slot = pci_dev.device_id();
            let func = 0;
            // The slot/function address of each device is encoded
            // in a single byte as follows:
            //
            // 7:3 = slot
            // 2:0 = function
            // together those 8 bits combined as devfn value
            let devfn = (((slot) & 0x1f) << 3) | ((func) & 0x07);

            return Ok((busno, devfn));
        }

        Err(DeviceMgrError::GetDeviceResource)
    }
}

#[cfg(feature = "hotplug")]
impl DeviceManager {
    /// Get Unix Domain Socket path for the vsock device.
    pub fn get_vsock_inner_connector(&mut self) -> Option<VsockInnerConnector> {
        #[cfg(feature = "virtio-vsock")]
        {
            self.vsock_manager
                .get_default_connector()
                .map(Some)
                .unwrap_or(None)
        }
        #[cfg(not(feature = "virtio-vsock"))]
        {
            return None;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use dbs_address_space::{AddressSpaceLayout, AddressSpaceRegion, AddressSpaceRegionType};
    use kvm_ioctls::Kvm;
    use test_utils::skip_if_not_root;
    use vm_memory::{GuestAddress, GuestUsize, MmapRegion};

    use super::*;
    #[cfg(target_arch = "x86_64")]
    use crate::vm::CpuTopology;

    pub const GUEST_PHYS_END: u64 = (1 << 46) - 1;
    pub const GUEST_MEM_START: u64 = 0;
    pub const GUEST_MEM_END: u64 = GUEST_PHYS_END >> 1;

    pub fn create_address_space() -> AddressSpace {
        let address_space_region = vec![Arc::new(AddressSpaceRegion::new(
            AddressSpaceRegionType::DefaultMemory,
            GuestAddress(0x0),
            0x1000 as GuestUsize,
        ))];
        let layout = AddressSpaceLayout::new(GUEST_PHYS_END, GUEST_MEM_START, GUEST_MEM_END);
        AddressSpace::from_regions(address_space_region, layout)
    }

    impl DeviceManager {
        pub fn new_test_mgr() -> Self {
            let kvm = Kvm::new().unwrap();
            let vm = kvm.create_vm().unwrap();
            let vm_fd = Arc::new(vm);
            let epoll_manager = EpollManager::default();
            let res_manager = Arc::new(ResourceManager::new(None));
            let logger = slog_scope::logger().new(slog::o!());
            let shared_info = Arc::new(RwLock::new(InstanceInfo::new(
                String::from("dragonball"),
                String::from("1"),
            )));

            DeviceManager {
                vm_fd: Arc::clone(&vm_fd),
                con_manager: ConsoleManager::new(epoll_manager, &logger),
                io_manager: Arc::new(ArcSwap::new(Arc::new(IoManager::new()))),
                io_lock: Arc::new(Mutex::new(())),
                irq_manager: Arc::new(KvmIrqManager::new(vm_fd.clone())),
                res_manager,

                legacy_manager: None,
                #[cfg(any(feature = "virtio-blk", feature = "vhost-user-blk"))]
                block_manager: BlockDeviceMgr::default(),
                #[cfg(any(feature = "virtio-fs", feature = "vhost-user-fs"))]
                fs_manager: Arc::new(Mutex::new(FsDeviceMgr::default())),
                #[cfg(feature = "virtio-net")]
                virtio_net_manager: VirtioNetDeviceMgr::default(),
                #[cfg(feature = "virtio-vsock")]
                vsock_manager: VsockDeviceMgr::default(),
                #[cfg(feature = "virtio-mem")]
                mem_manager: MemDeviceMgr::default(),
                #[cfg(feature = "virtio-balloon")]
                balloon_manager: BalloonDeviceMgr::default(),
                #[cfg(target_arch = "aarch64")]
                mmio_device_info: HashMap::new(),
                #[cfg(feature = "vhost-net")]
                vhost_net_manager: VhostNetDeviceMgr::default(),
                #[cfg(feature = "vhost-user-net")]
                vhost_user_net_manager: VhostUserNetDeviceMgr::default(),
                #[cfg(feature = "host-device")]
                vfio_manager: Arc::new(Mutex::new(VfioDeviceMgr::new(vm_fd, &logger))),

                logger,
                shared_info,
            }
        }
    }

    #[test]
    fn test_create_device_manager() {
        skip_if_not_root!();
        let mgr = DeviceManager::new_test_mgr();
        let _ = mgr.io_manager();
    }

    #[cfg(target_arch = "x86_64")]
    #[test]
    fn test_create_devices() {
        skip_if_not_root!();
        use crate::vm::VmConfigInfo;

        let epoll_manager = EpollManager::default();
        let vmm = Arc::new(Mutex::new(crate::vmm::tests::create_vmm_instance(
            epoll_manager.clone(),
        )));
        let event_mgr = crate::event_manager::EventManager::new(&vmm, epoll_manager).unwrap();
        let mut vm = crate::vm::tests::create_vm_instance();
        let vm_config = VmConfigInfo {
            vcpu_count: 1,
            max_vcpu_count: 1,
            cpu_pm: "off".to_string(),
            mem_type: "shmem".to_string(),
            mem_file_path: "".to_string(),
            mem_size_mib: 16,
            serial_path: None,
            cpu_topology: CpuTopology {
                threads_per_core: 1,
                cores_per_die: 1,
                dies_per_socket: 1,
                sockets: 1,
            },
            vpmu_feature: 0,
            pci_hotplug_enabled: false,
        };
        vm.set_vm_config(vm_config.clone());
        vm.init_guest_memory().unwrap();
        vm.setup_interrupt_controller().unwrap();
        let vm_as = vm.vm_as().cloned().unwrap();
        let kernel_temp_file = vmm_sys_util::tempfile::TempFile::new().unwrap();
        let kernel_file = kernel_temp_file.into_file();
        let mut cmdline = crate::vm::KernelConfigInfo::new(
            kernel_file,
            None,
            linux_loader::cmdline::Cmdline::new(0x1000).unwrap(),
        );

        let address_space = vm.vm_address_space().cloned();
        let mgr = vm.device_manager_mut();
        let guard = mgr.io_manager.load();
        let mut lcr = [0u8];
        // 0x3f8 is the adddress of serial device
        guard.pio_read(0x3f8 + 3, &mut lcr).unwrap_err();
        assert_eq!(lcr[0], 0x0);

        mgr.create_interrupt_manager().unwrap();
        mgr.create_devices(
            vm_as,
            event_mgr.epoll_manager(),
            &mut cmdline,
            None,
            address_space.as_ref(),
            &vm_config,
        )
        .unwrap();
        let guard = mgr.io_manager.load();
        guard.pio_read(0x3f8 + 3, &mut lcr).unwrap();
        assert_eq!(lcr[0], 0x3);
    }

    #[cfg(feature = "virtio-fs")]
    #[test]
    fn test_handler_insert_region() {
        skip_if_not_root!();

        use dbs_virtio_devices::VirtioRegionHandler;
        use lazy_static::__Deref;
        use vm_memory::{GuestAddressSpace, GuestMemory, GuestMemoryRegion};

        let vm = crate::test_utils::tests::create_vm_for_test();
        let ctx = DeviceOpContext::new(
            Some(vm.epoll_manager().clone()),
            vm.device_manager(),
            Some(vm.vm_as().unwrap().clone()),
            vm.vm_address_space().cloned(),
            true,
            Some(vm.vm_config().clone()),
            vm.shared_info().clone(),
        );
        let guest_addr = GuestAddress(*dbs_boot::layout::GUEST_MEM_END);

        let cache_len = 1024 * 1024 * 1024;
        let mmap_region = MmapRegion::build(
            None,
            cache_len as usize,
            libc::PROT_NONE,
            libc::MAP_ANONYMOUS | libc::MAP_NORESERVE | libc::MAP_PRIVATE,
        )
        .unwrap();

        let guest_mmap_region =
            Arc::new(vm_memory::GuestRegionMmap::new(mmap_region, guest_addr).unwrap());

        let mut handler = DeviceVirtioRegionHandler {
            vm_as: ctx.get_vm_as().unwrap(),
            address_space: ctx.address_space.as_ref().unwrap().clone(),
        };
        handler.insert_region(guest_mmap_region).unwrap();
        let mut find_region = false;
        let find_region_ptr = &mut find_region;

        let guard = vm.vm_as().unwrap().clone().memory();

        let mem = guard.deref();
        for region in mem.iter() {
            if region.start_addr() == guest_addr && region.len() == cache_len {
                *find_region_ptr = true;
            }
        }

        assert!(find_region);
    }
}
