// Copyright (C) 2022 Alibaba Cloud. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Device manager to manage IO devices for a virtual machine.

use std::io;
use std::sync::{Arc, Mutex, MutexGuard};

use arc_swap::ArcSwap;
use dbs_address_space::AddressSpace;
#[cfg(target_arch = "aarch64")]
use dbs_arch::{DeviceType, MMIODeviceInfo};
use dbs_device::device_manager::{Error as IoManagerError, IoManager, IoManagerContext};
use dbs_device::resources::Resource;
use dbs_device::DeviceIo;
use dbs_interrupt::KvmIrqManager;
use dbs_legacy_devices::ConsoleHandler;
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

#[cfg(all(feature = "hotplug", feature = "dbs-upcall"))]
use dbs_upcall::{
    DevMgrRequest, DevMgrService, MmioDevRequest, UpcallClient, UpcallClientError,
    UpcallClientRequest, UpcallClientResponse,
};
#[cfg(feature = "hotplug")]
use dbs_virtio_devices::vsock::backend::VsockInnerConnector;

use crate::address_space_manager::GuestAddressSpaceImpl;
use crate::error::StartMicrovmError;
use crate::resource_manager::ResourceManager;
use crate::vm::{KernelConfigInfo, Vm};
use crate::IoManagerCached;

/// Virtual machine console device manager.
pub mod console_manager;
/// Console Manager for virtual machines console device.
pub use self::console_manager::ConsoleManager;

mod legacy;
pub use self::legacy::{Error as LegacyDeviceError, LegacyDeviceManager};

#[cfg(feature = "virtio-vsock")]
/// Device manager for user-space vsock devices.
pub mod vsock_dev_mgr;
#[cfg(feature = "virtio-vsock")]
use self::vsock_dev_mgr::VsockDeviceMgr;

#[cfg(feature = "virtio-blk")]
/// virtio-block device manager
pub mod blk_dev_mgr;
#[cfg(feature = "virtio-blk")]
use self::blk_dev_mgr::BlockDeviceMgr;

#[cfg(feature = "virtio-net")]
/// Device manager for virtio-net devices.
pub mod virtio_net_dev_mgr;
#[cfg(feature = "virtio-net")]
use self::virtio_net_dev_mgr::VirtioNetDeviceMgr;

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
}

/// Specialized version of `std::result::Result` for device manager operations.
pub type Result<T> = ::std::result::Result<T, DeviceMgrError>;

/// Type of the dragonball virtio devices.
#[cfg(feature = "dbs-virtio-devices")]
pub type DbsVirtioDevice = Box<
    dyn VirtioDevice<GuestAddressSpaceImpl, virtio_queue::QueueState, vm_memory::GuestRegionMmap>,
>;

/// Type of the dragonball virtio mmio devices.
#[cfg(feature = "dbs-virtio-devices")]
pub type DbsMmioV2Device =
    MmioV2Device<GuestAddressSpaceImpl, virtio_queue::QueueState, vm_memory::GuestRegionMmap>;

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

    #[cfg(all(feature = "hotplug", feature = "dbs-upcall"))]
    upcall_client: Option<Arc<UpcallClient<DevMgrService>>>,
    #[cfg(feature = "dbs-virtio-devices")]
    virtio_devices: Vec<Arc<DbsMmioV2Device>>,
}

impl DeviceOpContext {
    pub(crate) fn new(
        epoll_mgr: Option<EpollManager>,
        device_mgr: &DeviceManager,
        vm_as: Option<GuestAddressSpaceImpl>,
        address_space: Option<AddressSpace>,
        is_hotplug: bool,
    ) -> Self {
        let irq_manager = device_mgr.irq_manager.clone();
        let res_manager = device_mgr.res_manager.clone();

        let vm_fd = device_mgr.vm_fd.clone();
        let io_context = DeviceManagerContext {
            io_manager: device_mgr.io_manager.clone(),
            io_lock: device_mgr.io_lock.clone(),
        };
        let logger = device_mgr.logger.new(slog::o!());

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
            #[cfg(all(feature = "hotplug", feature = "dbs-upcall"))]
            upcall_client: None,
            #[cfg(feature = "dbs-virtio-devices")]
            virtio_devices: Vec::new(),
        }
    }

    pub(crate) fn create_boot_ctx(vm: &Vm, epoll_mgr: Option<EpollManager>) -> Self {
        Self::new(epoll_mgr, vm.device_manager(), None, None, false)
    }

    pub(crate) fn get_vm_as(&self) -> Result<GuestAddressSpaceImpl> {
        match self.vm_as.as_ref() {
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
}

#[cfg(not(feature = "hotplug"))]
impl DeviceOpContext {
    pub(crate) fn insert_hotplug_mmio_device(
        &self,
        _dev: &Arc<dyn DeviceIo>,
        _callback: Option<()>,
    ) -> Result<()> {
        Err(DeviceMgrError::InvalidOperation)
    }

    pub(crate) fn remove_hotplug_mmio_device(
        &self,
        _dev: &Arc<dyn DeviceIo>,
        _callback: Option<()>,
    ) -> Result<()> {
        Err(DeviceMgrError::InvalidOperation)
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

    pub(crate) con_manager: ConsoleManager,
    pub(crate) legacy_manager: Option<LegacyDeviceManager>,
    #[cfg(target_arch = "aarch64")]
    pub(crate) mmio_device_info: HashMap<(DeviceType, String), MMIODeviceInfo>,
    #[cfg(feature = "virtio-vsock")]
    pub(crate) vsock_manager: VsockDeviceMgr,

    #[cfg(feature = "virtio-blk")]
    // If there is a Root Block Device, this should be added as the first element of the list.
    // This is necessary because we want the root to always be mounted on /dev/vda.
    pub(crate) block_manager: BlockDeviceMgr,

    #[cfg(feature = "virtio-net")]
    pub(crate) virtio_net_manager: VirtioNetDeviceMgr,
}

impl DeviceManager {
    /// Create a new device manager instance.
    pub fn new(
        vm_fd: Arc<VmFd>,
        res_manager: Arc<ResourceManager>,
        epoll_manager: EpollManager,
        logger: &slog::Logger,
    ) -> Self {
        DeviceManager {
            io_manager: Arc::new(ArcSwap::new(Arc::new(IoManager::new()))),
            io_lock: Arc::new(Mutex::new(())),
            irq_manager: Arc::new(KvmIrqManager::new(vm_fd.clone())),
            res_manager,
            vm_fd,
            logger: logger.new(slog::o!()),

            con_manager: ConsoleManager::new(epoll_manager, logger),
            legacy_manager: None,
            #[cfg(target_arch = "aarch64")]
            mmio_device_info: HashMap::new(),
            #[cfg(feature = "virtio-vsock")]
            vsock_manager: VsockDeviceMgr::default(),
            #[cfg(feature = "virtio-blk")]
            block_manager: BlockDeviceMgr::default(),
            #[cfg(feature = "virtio-net")]
            virtio_net_manager: VirtioNetDeviceMgr::default(),
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
    ) -> std::result::Result<(), StartMicrovmError> {
        #[cfg(target_arch = "x86_64")]
        {
            let mut tx = ctx.io_context.begin_tx();
            let legacy_manager =
                LegacyDeviceManager::create_manager(&mut tx.io_manager, Some(self.vm_fd.clone()));

            match legacy_manager {
                Ok(v) => {
                    self.legacy_manager = Some(v);
                    ctx.io_context.commit_tx(tx);
                }
                Err(e) => {
                    ctx.io_context.cancel_tx(tx);
                    return Err(StartMicrovmError::LegacyDevice(e));
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
    ) -> std::result::Result<(), StartMicrovmError> {
        // Connect serial ports to the console and dmesg_fifo.
        self.set_guest_kernel_log_stream(dmesg_fifo)
            .map_err(|_| StartMicrovmError::EventFd)?;

        info!(self.logger, "init console path: {:?}", com1_sock_path);
        if let Some(path) = com1_sock_path {
            if let Some(legacy_manager) = self.legacy_manager.as_ref() {
                let com1 = legacy_manager.get_com1_serial();
                self.con_manager
                    .create_socket_console(com1, path)
                    .map_err(StartMicrovmError::DeviceManager)?;
            }
        } else if let Some(legacy_manager) = self.legacy_manager.as_ref() {
            let com1 = legacy_manager.get_com1_serial();
            self.con_manager
                .create_stdio_console(com1)
                .map_err(StartMicrovmError::DeviceManager)?;
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
        com1_sock_path: Option<String>,
        dmesg_fifo: Option<Box<dyn io::Write + Send>>,
        address_space: Option<&AddressSpace>,
    ) -> std::result::Result<(), StartMicrovmError> {
        let mut ctx = DeviceOpContext::new(
            Some(epoll_mgr),
            self,
            Some(vm_as),
            address_space.cloned(),
            false,
        );

        self.create_legacy_devices(&mut ctx)?;
        self.init_legacy_devices(dmesg_fifo, com1_sock_path, &mut ctx)?;

        #[cfg(feature = "virtio-blk")]
        self.block_manager
            .attach_devices(&mut ctx)
            .map_err(StartMicrovmError::BlockDeviceError)?;

        #[cfg(feature = "virtio-net")]
        self.virtio_net_manager
            .attach_devices(&mut ctx)
            .map_err(StartMicrovmError::VirtioNetDeviceError)?;

        #[cfg(feature = "virtio-vsock")]
        self.vsock_manager.attach_devices(&mut ctx)?;

        #[cfg(feature = "virtio-blk")]
        self.block_manager
            .generate_kernel_boot_args(kernel_config)
            .map_err(StartMicrovmError::DeviceManager)?;
        ctx.generate_kernel_boot_args(kernel_config)
            .map_err(StartMicrovmError::DeviceManager)?;

        Ok(())
    }

    /// Start all registered devices when booting the associated virtual machine.
    pub fn start_devices(&mut self) -> std::result::Result<(), StartMicrovmError> {
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
        );

        #[cfg(feature = "virtio-blk")]
        self.block_manager.remove_devices(&mut ctx)?;
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
}

#[cfg(feature = "hotplug")]
impl DeviceManager {
    /// Get Unix Domain Socket path for the vsock device.
    pub fn get_vsock_inner_connector(&mut self) -> Option<VsockInnerConnector> {
        #[cfg(feature = "virtio-vsock")]
        {
            self.vsock_manager
                .get_default_connector()
                .map(|d| Some(d))
                .unwrap_or(None)
        }
        #[cfg(not(feature = "virtio-vsock"))]
        {
            return None;
        }
    }
}
