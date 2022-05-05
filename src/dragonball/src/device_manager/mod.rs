// Copyright (C) 2022 Alibaba Cloud. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Device manager to manage IO devices for a virtual machine.

use std::io;
use std::sync::{Arc, Mutex, MutexGuard};

use arc_swap::ArcSwap;
use dbs_address_space::AddressSpace;
use dbs_device::device_manager::{Error as IoManagerError, IoManager, IoManagerContext};
use dbs_device::resources::Resource;
use dbs_device::DeviceIo;
use dbs_interrupt::KvmIrqManager;
use dbs_legacy_devices::ConsoleHandler;
use dbs_utils::epoll_manager::EpollManager;
use kvm_ioctls::VmFd;

use crate::address_space_manager::GuestAddressSpaceImpl;
use crate::error::StartMicrovmError;
use crate::resource_manager::ResourceManager;
use crate::vm::KernelConfigInfo;

/// Virtual machine console device manager.
pub mod console_manager;
/// Console Manager for virtual machines console device.
pub use self::console_manager::ConsoleManager;

mod legacy;
pub use self::legacy::{Error as LegacyDeviceError, LegacyDeviceManager};

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
}

/// Specialized version of `std::result::Result` for device manager operations.
pub type Result<T> = ::std::result::Result<T, DeviceMgrError>;

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
            #[cfg(feature = "dbs-virtio-devices")]
            virtio_devices: Vec::new(),
        }
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

    fn generate_kernel_boot_args(&mut self, kernel_config: &mut KernelConfigInfo) -> Result<()> {
        if !self.is_hotplug {
            return Err(DeviceMgrError::InvalidOperation);
        }

        #[cfg(feature = "dbs-virtio-devices")]
        let cmdline = kernel_config.kernel_cmdline_mut();

        #[cfg(feature = "dbs-virtio-devices")]
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

        Ok(())
    }
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
        }
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

        slog::info!(self.logger, "init console path: {:?}", com1_sock_path);
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

    /// Restore legacy devices
    pub fn restore_legacy_devices(
        &mut self,
        dmesg_fifo: Option<Box<dyn io::Write + Send>>,
        com1_sock_path: Option<String>,
    ) -> std::result::Result<(), StartMicrovmError> {
        self.set_guest_kernel_log_stream(dmesg_fifo)
            .map_err(|_| StartMicrovmError::EventFd)?;
        slog::info!(self.logger, "restore console path: {:?}", com1_sock_path);
        // TODO: restore console
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

        ctx.generate_kernel_boot_args(kernel_config)
            .map_err(StartMicrovmError::DeviceManager)?;

        Ok(())
    }

    #[cfg(target_arch = "x86_64")]
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
}
