// Copyright (C) 2021 Alibaba Cloud. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use std::io::{self, Read, Seek, SeekFrom};
use std::ops::Deref;
use std::os::unix::io::RawFd;

use std::sync::{Arc, Mutex, RwLock};

use dbs_address_space::AddressSpace;
#[cfg(target_arch = "aarch64")]
use dbs_arch::gic::GICDevice;
#[cfg(target_arch = "aarch64")]
use dbs_arch::pmu::PmuError;
use dbs_boot::InitrdConfig;
use dbs_utils::epoll_manager::EpollManager;
use dbs_utils::time::TimestampUs;
use kvm_ioctls::VmFd;
use linux_loader::loader::{KernelLoader, KernelLoaderResult};
use seccompiler::BpfProgram;
use serde_derive::{Deserialize, Serialize};
use slog::{error, info};
use vm_memory::{Bytes, GuestAddress, GuestAddressSpace};
use vmm_sys_util::eventfd::EventFd;

#[cfg(all(feature = "hotplug", feature = "dbs-upcall"))]
use dbs_upcall::{DevMgrService, UpcallClient};
#[cfg(feature = "hotplug")]
use std::sync::mpsc::Sender;

use crate::address_space_manager::{
    AddressManagerError, AddressSpaceMgr, AddressSpaceMgrBuilder, GuestAddressSpaceImpl,
    GuestMemoryImpl,
};
use crate::api::v1::{InstanceInfo, InstanceState};
use crate::device_manager::console_manager::DmesgWriter;
use crate::device_manager::{DeviceManager, DeviceMgrError, DeviceOpContext};
use crate::error::{LoadInitrdError, Result, StartMicroVmError, StopMicrovmError};
use crate::event_manager::EventManager;
use crate::kvm_context::KvmContext;
use crate::resource_manager::ResourceManager;
use crate::vcpu::{VcpuManager, VcpuManagerError};
#[cfg(feature = "hotplug")]
use crate::vcpu::{VcpuResizeError, VcpuResizeInfo};
#[cfg(target_arch = "aarch64")]
use dbs_arch::gic::Error as GICError;

mod kernel_config;
pub use self::kernel_config::KernelConfigInfo;

#[cfg(target_arch = "aarch64")]
#[path = "aarch64.rs"]
mod aarch64;

#[cfg(target_arch = "x86_64")]
#[path = "x86_64.rs"]
mod x86_64;

/// Errors associated with virtual machine instance related operations.
#[derive(Debug, thiserror::Error)]
pub enum VmError {
    /// Cannot configure the IRQ.
    #[error("failed to configure IRQ fot the virtual machine: {0}")]
    Irq(#[source] kvm_ioctls::Error),

    /// Cannot configure the microvm.
    #[error("failed to initialize the virtual machine: {0}")]
    VmSetup(#[source] kvm_ioctls::Error),

    /// Cannot setup GIC
    #[cfg(target_arch = "aarch64")]
    #[error("failed to configure GIC")]
    SetupGIC(GICError),

    /// Cannot setup pmu device
    #[cfg(target_arch = "aarch64")]
    #[error("failed to setup pmu device")]
    SetupPmu(#[source] PmuError),
}

/// Configuration information for user defined NUMA nodes.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct NumaRegionInfo {
    /// memory size for this region (unit: MiB)
    pub size: u64,
    /// numa node id on host for this region
    pub host_numa_node_id: Option<u32>,
    /// numa node id on guest for this region
    pub guest_numa_node_id: Option<u32>,
    /// vcpu ids belonging to this region
    pub vcpu_ids: Vec<u32>,
}

/// Information for cpu topology to guide guest init
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CpuTopology {
    /// threads per core to indicate hyperthreading is enabled or not
    pub threads_per_core: u8,
    /// cores per die to guide guest cpu topology init
    pub cores_per_die: u8,
    /// dies per socket to guide guest cpu topology
    pub dies_per_socket: u8,
    /// number of sockets
    pub sockets: u8,
}

impl Default for CpuTopology {
    fn default() -> Self {
        CpuTopology {
            threads_per_core: 1,
            cores_per_die: 1,
            dies_per_socket: 1,
            sockets: 1,
        }
    }
}

/// Configuration information for virtual machine instance.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VmConfigInfo {
    /// Number of vcpu to start.
    pub vcpu_count: u8,
    /// Max number of vcpu can be added
    pub max_vcpu_count: u8,
    /// cpu power management.
    pub cpu_pm: String,
    /// cpu topology information
    pub cpu_topology: CpuTopology,
    /// vpmu support level
    pub vpmu_feature: u8,

    /// Memory type that can be either hugetlbfs or shmem, default is shmem
    pub mem_type: String,
    /// Memory file path
    pub mem_file_path: String,
    /// The memory size in MiB.
    pub mem_size_mib: usize,

    /// sock path
    pub serial_path: Option<String>,

    /// Enable PCI device hotplug or not
    pub pci_hotplug_enabled: bool,
}

impl Default for VmConfigInfo {
    fn default() -> Self {
        VmConfigInfo {
            vcpu_count: 1,
            max_vcpu_count: 1,
            cpu_pm: String::from("on"),
            cpu_topology: CpuTopology {
                threads_per_core: 1,
                cores_per_die: 1,
                dies_per_socket: 1,
                sockets: 1,
            },
            vpmu_feature: 0,
            mem_type: String::from("shmem"),
            mem_file_path: String::from(""),
            mem_size_mib: 128,
            serial_path: None,
            pci_hotplug_enabled: false,
        }
    }
}

/// Struct to manage resources and control states of an virtual machine instance.
///
/// An `Vm` instance holds a resources assigned to a virtual machine instance, such as CPU, memory,
/// devices etc. When an `Vm` instance gets deconstructed, all resources assigned should be
/// released.
///
/// We have explicit build the object model as:
///  |---Vmm API Server--<-1:1-> HTTP API Server
///  |        |----------<-1:1-> Shimv2/CRI API Server
///  |
/// Vmm <-1:N-> Vm <-1:1-> Address Space Manager <-1:N-> GuestMemory
///  ^           ^---1:1-> Device Manager <-1:N-> Device
///  |           ^---1:1-> Resource Manager
///  |           ^---1:N-> Vcpu
///  |---<-1:N-> Event Manager
pub struct Vm {
    epoll_manager: EpollManager,
    kvm: KvmContext,
    shared_info: Arc<RwLock<InstanceInfo>>,

    address_space: AddressSpaceMgr,
    /// device manager for Dragonball
    pub device_manager: DeviceManager,
    dmesg_fifo: Option<Box<dyn io::Write + Send>>,
    kernel_config: Option<KernelConfigInfo>,
    logger: slog::Logger,
    reset_eventfd: Option<EventFd>,
    resource_manager: Arc<ResourceManager>,
    vcpu_manager: Option<Arc<Mutex<VcpuManager>>>,
    vm_config: VmConfigInfo,
    vm_fd: Arc<VmFd>,

    start_instance_request_ts: u64,
    start_instance_request_cpu_ts: u64,
    start_instance_downtime: u64,

    // Arm specific fields.
    // On aarch64 we need to keep around the fd obtained by creating the VGIC device.
    #[cfg(target_arch = "aarch64")]
    irqchip_handle: Option<Box<dyn GICDevice>>,

    #[cfg(all(feature = "hotplug", feature = "dbs-upcall"))]
    upcall_client: Option<Arc<UpcallClient<DevMgrService>>>,
}

impl Vm {
    /// Constructs a new `Vm` instance using the given `Kvm` instance.
    pub fn new(
        kvm_fd: Option<RawFd>,
        api_shared_info: Arc<RwLock<InstanceInfo>>,
        epoll_manager: EpollManager,
    ) -> Result<Self> {
        let id = api_shared_info.read().unwrap().id.clone();
        let logger = slog_scope::logger().new(slog::o!("id" => id));
        let kvm = KvmContext::new(kvm_fd)?;
        let vm_fd = Arc::new(kvm.create_vm()?);
        let resource_manager = Arc::new(ResourceManager::new(Some(kvm.max_memslots())));
        let device_manager = DeviceManager::new(
            vm_fd.clone(),
            resource_manager.clone(),
            epoll_manager.clone(),
            &logger,
            api_shared_info.clone(),
        );

        Ok(Vm {
            epoll_manager,
            kvm,
            shared_info: api_shared_info,

            address_space: AddressSpaceMgr::default(),
            device_manager,
            dmesg_fifo: None,
            kernel_config: None,
            logger,
            reset_eventfd: None,
            resource_manager,
            vcpu_manager: None,
            vm_config: Default::default(),
            vm_fd,

            start_instance_request_ts: 0,
            start_instance_request_cpu_ts: 0,
            start_instance_downtime: 0,

            #[cfg(target_arch = "aarch64")]
            irqchip_handle: None,
            #[cfg(all(feature = "hotplug", feature = "dbs-upcall"))]
            upcall_client: None,
        })
    }

    /// Gets a reference to the device manager by this VM.
    pub fn device_manager(&self) -> &DeviceManager {
        &self.device_manager
    }

    /// Gets a mutable reference to the device manager by this VM.
    pub fn device_manager_mut(&mut self) -> &mut DeviceManager {
        &mut self.device_manager
    }

    /// Get a reference to EpollManager.
    pub fn epoll_manager(&self) -> &EpollManager {
        &self.epoll_manager
    }

    /// Get eventfd for exit notification.
    pub fn get_reset_eventfd(&self) -> Option<&EventFd> {
        self.reset_eventfd.as_ref()
    }

    /// Set guest kernel boot configurations.
    pub fn set_kernel_config(&mut self, kernel_config: KernelConfigInfo) {
        self.kernel_config = Some(kernel_config);
    }

    /// Get virtual machine shared instance information.
    pub fn shared_info(&self) -> &Arc<RwLock<InstanceInfo>> {
        &self.shared_info
    }

    /// Gets a reference to the address_space.address_space for guest memory owned by this VM.
    pub fn vm_address_space(&self) -> Option<&AddressSpace> {
        self.address_space.get_address_space()
    }

    /// Gets a reference to the address space for guest memory owned by this VM.
    ///
    /// Note that `GuestMemory` does not include any device memory that may have been added after
    /// this VM was constructed.
    pub fn vm_as(&self) -> Option<&GuestAddressSpaceImpl> {
        self.address_space.get_vm_as()
    }

    /// Get a immutable reference to the virtual machine configuration information.
    pub fn vm_config(&self) -> &VmConfigInfo {
        &self.vm_config
    }

    /// Set the virtual machine configuration information.
    pub fn set_vm_config(&mut self, config: VmConfigInfo) {
        self.vm_config = config;
    }

    /// Gets a reference to the kvm file descriptor owned by this VM.
    pub fn vm_fd(&self) -> &VmFd {
        &self.vm_fd
    }

    /// returns true if system upcall service is ready
    pub fn is_upcall_client_ready(&self) -> bool {
        #[cfg(all(feature = "hotplug", feature = "dbs-upcall"))]
        {
            if let Some(upcall_client) = self.upcall_client() {
                return upcall_client.is_ready();
            }
        }

        false
    }

    /// Check whether the VM has been initialized.
    pub fn is_vm_initialized(&self) -> bool {
        let instance_state = {
            // Use expect() to crash if the other thread poisoned this lock.
            let shared_info = self.shared_info.read()
                .expect("Failed to determine if instance is initialized because shared info couldn't be read due to poisoned lock");
            shared_info.state
        };
        instance_state != InstanceState::Uninitialized
    }

    /// Check whether the VM instance is running.
    pub fn is_vm_running(&self) -> bool {
        let instance_state = {
            // Use expect() to crash if the other thread poisoned this lock.
            let shared_info = self.shared_info.read()
                .expect("Failed to determine if instance is initialized because shared info couldn't be read due to poisoned lock");
            shared_info.state
        };
        instance_state == InstanceState::Running
    }

    /// Save VM instance exit state
    pub fn vm_exit(&self, exit_code: i32) {
        if let Ok(mut info) = self.shared_info.write() {
            info.state = InstanceState::Exited(exit_code);
        } else {
            error!(
                self.logger,
                "Failed to save exit state, couldn't be written due to poisoned lock"
            );
        }
    }

    /// Create device operation context.
    /// vm is not running, return false
    /// vm is running, but hotplug feature is not enable, return error
    /// vm is running, but upcall initialize failed, return error
    /// vm is running, upcall initialize OK, return true
    pub fn create_device_op_context(
        &mut self,
        epoll_mgr: Option<EpollManager>,
    ) -> std::result::Result<DeviceOpContext, StartMicroVmError> {
        if !self.is_vm_initialized() {
            Ok(DeviceOpContext::create_boot_ctx(self, epoll_mgr))
        } else {
            self.create_device_hotplug_context(epoll_mgr)
        }
    }

    pub(crate) fn check_health(&self) -> std::result::Result<(), StartMicroVmError> {
        if self.kernel_config.is_none() {
            return Err(StartMicroVmError::MissingKernelConfig);
        }
        Ok(())
    }

    pub(crate) fn get_dragonball_info(&self) -> (String, String) {
        let guard = self.shared_info.read().unwrap();
        let instance_id = guard.id.clone();
        let dragonball_version = guard.vmm_version.clone();

        (dragonball_version, instance_id)
    }

    pub(crate) fn stop_prealloc(&mut self) -> std::result::Result<(), StartMicroVmError> {
        if self.address_space.is_initialized() {
            return self
                .address_space
                .wait_prealloc(true)
                .map_err(StartMicroVmError::AddressManagerError);
        }

        Err(StartMicroVmError::AddressManagerError(
            AddressManagerError::GuestMemoryNotInitialized,
        ))
    }
}

impl Vm {
    pub(crate) fn init_vcpu_manager(
        &mut self,
        vm_as: GuestAddressSpaceImpl,
        vcpu_seccomp_filter: BpfProgram,
    ) -> std::result::Result<(), VcpuManagerError> {
        let vcpu_manager = VcpuManager::new(
            self.vm_fd.clone(),
            &self.kvm,
            &self.vm_config,
            vm_as,
            vcpu_seccomp_filter,
            self.shared_info.clone(),
            self.device_manager.io_manager(),
            self.epoll_manager.clone(),
        )?;
        self.vcpu_manager = Some(vcpu_manager);

        Ok(())
    }

    /// get the cpu manager's reference
    pub(crate) fn vcpu_manager(
        &self,
    ) -> std::result::Result<std::sync::MutexGuard<'_, VcpuManager>, VcpuManagerError> {
        self.vcpu_manager
            .as_ref()
            .ok_or(VcpuManagerError::VcpuManagerNotInitialized)
            .map(|mgr| mgr.lock().unwrap())
    }

    /// Pause all vcpus and record the instance downtime
    pub fn pause_all_vcpus_with_downtime(&mut self) -> std::result::Result<(), VcpuManagerError> {
        let ts = TimestampUs::default();
        self.start_instance_downtime = ts.time_us;

        self.vcpu_manager()?.pause_all_vcpus()?;

        Ok(())
    }

    /// Resume all vcpus and calc the intance downtime
    pub fn resume_all_vcpus_with_downtime(&mut self) -> std::result::Result<(), VcpuManagerError> {
        self.vcpu_manager()?.resume_all_vcpus()?;

        if self.start_instance_downtime != 0 {
            let now = TimestampUs::default();
            let downtime = now.time_us - self.start_instance_downtime;
            info!(self.logger, "VM: instance downtime: {} us", downtime);
            self.start_instance_downtime = 0;
            if let Ok(mut info) = self.shared_info.write() {
                info.last_instance_downtime = downtime;
            } else {
                error!(self.logger, "Failed to update live upgrade downtime, couldn't be written due to poisoned lock");
            }
        }

        Ok(())
    }

    pub(crate) fn init_devices(
        &mut self,
        epoll_manager: EpollManager,
    ) -> std::result::Result<(), StartMicroVmError> {
        info!(self.logger, "VM: initializing devices ...");

        let kernel_config = self
            .kernel_config
            .as_mut()
            .ok_or(StartMicroVmError::MissingKernelConfig)?;

        info!(self.logger, "VM: create interrupt manager");
        self.device_manager
            .create_interrupt_manager()
            .map_err(StartMicroVmError::DeviceManager)?;

        info!(self.logger, "VM: create devices");
        let vm_as =
            self.address_space
                .get_vm_as()
                .ok_or(StartMicroVmError::AddressManagerError(
                    AddressManagerError::GuestMemoryNotInitialized,
                ))?;
        self.device_manager.create_devices(
            vm_as.clone(),
            epoll_manager,
            kernel_config,
            self.dmesg_fifo.take(),
            self.address_space.address_space(),
            &self.vm_config,
        )?;

        info!(self.logger, "VM: start devices");
        self.device_manager.start_devices(vm_as)?;

        info!(self.logger, "VM: initializing devices done");
        Ok(())
    }

    /// Remove devices when shutdown vm
    pub fn remove_devices(&mut self) -> std::result::Result<(), StopMicrovmError> {
        info!(self.logger, "VM: remove devices");
        let vm_as = self
            .address_space
            .get_vm_as()
            .ok_or(StopMicrovmError::GuestMemoryNotInitialized)?;

        self.device_manager
            .remove_devices(
                vm_as.clone(),
                self.epoll_manager.clone(),
                self.address_space.address_space(),
            )
            .map_err(StopMicrovmError::DeviceManager)
    }

    /// Remove upcall client when the VM is destoryed.
    #[cfg(feature = "dbs-upcall")]
    pub fn remove_upcall(&mut self) -> std::result::Result<(), StopMicrovmError> {
        self.upcall_client = None;
        Ok(())
    }

    /// Reset the console into canonical mode.
    pub fn reset_console(&self) -> std::result::Result<(), DeviceMgrError> {
        self.device_manager.reset_console()
    }

    pub(crate) fn init_dmesg_logger(&mut self) {
        let writer = self.dmesg_logger();
        self.dmesg_fifo = Some(writer);
    }

    /// dmesg write to logger
    fn dmesg_logger(&self) -> Box<dyn io::Write + Send> {
        Box::new(DmesgWriter::new(&self.logger))
    }

    pub(crate) fn init_guest_memory(&mut self) -> std::result::Result<(), StartMicroVmError> {
        info!(self.logger, "VM: initializing guest memory...");
        // We are not allowing reinitialization of vm guest memory.
        if self.address_space.is_initialized() {
            return Ok(());
        }

        // vcpu boot up require local memory. reserve 100 MiB memory
        let mem_size = (self.vm_config.mem_size_mib as u64) << 20;

        let mem_type = self.vm_config.mem_type.clone();
        let mut mem_file_path = String::from("");
        if mem_type == "hugetlbfs" {
            mem_file_path = self.vm_config.mem_file_path.clone();
            let shared_info = self.shared_info.read()
                    .expect("Failed to determine if instance is initialized because shared info couldn't be read due to poisoned lock");
            mem_file_path.push_str("/dragonball/");
            mem_file_path.push_str(shared_info.id.as_str());
        }

        let mut vcpu_ids: Vec<u32> = Vec::new();
        for i in 0..self.vm_config().max_vcpu_count {
            vcpu_ids.push(i as u32);
        }

        // init default regions.
        let mut numa_regions = Vec::with_capacity(1);
        let numa_node = NumaRegionInfo {
            size: self.vm_config.mem_size_mib as u64,
            host_numa_node_id: None,
            guest_numa_node_id: Some(0),
            vcpu_ids,
        };
        numa_regions.push(numa_node);

        info!(
            self.logger,
            "VM: mem_type:{} mem_file_path:{}, mem_size:{}, numa_regions:{:?}",
            mem_type,
            mem_file_path,
            mem_size,
            numa_regions,
        );

        let mut address_space_param = AddressSpaceMgrBuilder::new(&mem_type, &mem_file_path)
            .map_err(StartMicroVmError::AddressManagerError)?;
        address_space_param.set_kvm_vm_fd(self.vm_fd.clone());
        self.address_space
            .create_address_space(&self.resource_manager, &numa_regions, address_space_param)
            .map_err(StartMicroVmError::AddressManagerError)?;

        info!(self.logger, "VM: initializing guest memory done");
        Ok(())
    }

    fn init_configure_system(
        &mut self,
        vm_as: &GuestAddressSpaceImpl,
    ) -> std::result::Result<(), StartMicroVmError> {
        let vm_memory = vm_as.memory();
        let kernel_config = self
            .kernel_config
            .as_ref()
            .ok_or(StartMicroVmError::MissingKernelConfig)?;
        //let cmdline = kernel_config.cmdline.clone();
        let initrd: Option<InitrdConfig> = match kernel_config.initrd_file() {
            Some(f) => {
                let initrd_file = f.try_clone();
                if initrd_file.is_err() {
                    return Err(StartMicroVmError::InitrdLoader(
                        LoadInitrdError::ReadInitrd(io::Error::from(io::ErrorKind::InvalidData)),
                    ));
                }
                let res = self.load_initrd(vm_memory.deref(), &mut initrd_file.unwrap())?;
                Some(res)
            }
            None => None,
        };

        self.configure_system_arch(vm_memory.deref(), kernel_config.kernel_cmdline(), initrd)
    }

    /// Loads the initrd from a file into the given memory slice.
    ///
    /// * `vm_memory` - The guest memory the initrd is written to.
    /// * `image` - The initrd image.
    ///
    /// Returns the result of initrd loading
    fn load_initrd<F>(
        &self,
        vm_memory: &GuestMemoryImpl,
        image: &mut F,
    ) -> std::result::Result<InitrdConfig, LoadInitrdError>
    where
        F: Read + Seek,
    {
        use crate::error::LoadInitrdError::*;

        let size: usize;
        // Get the image size
        match image.seek(SeekFrom::End(0)) {
            Err(e) => return Err(ReadInitrd(e)),
            Ok(0) => {
                return Err(ReadInitrd(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Initrd image seek returned a size of zero",
                )))
            }
            Ok(s) => size = s as usize,
        };
        // Go back to the image start
        image.seek(SeekFrom::Start(0)).map_err(ReadInitrd)?;

        // Get the target address
        let address = dbs_boot::initrd_load_addr(vm_memory, size as u64).map_err(|_| LoadInitrd)?;

        // Load the image into memory
        vm_memory
            .read_from(GuestAddress(address), image, size)
            .map_err(|_| LoadInitrd)?;

        Ok(InitrdConfig {
            address: GuestAddress(address),
            size,
        })
    }

    fn load_kernel(
        &mut self,
        vm_memory: &GuestMemoryImpl,
    ) -> std::result::Result<KernelLoaderResult, StartMicroVmError> {
        // This is the easy way out of consuming the value of the kernel_cmdline.
        let kernel_config = self
            .kernel_config
            .as_mut()
            .ok_or(StartMicroVmError::MissingKernelConfig)?;
        let high_mem_addr = GuestAddress(dbs_boot::get_kernel_start());

        #[cfg(target_arch = "x86_64")]
        return linux_loader::loader::elf::Elf::load(
            vm_memory,
            None,
            kernel_config.kernel_file_mut(),
            Some(high_mem_addr),
        )
        .map_err(StartMicroVmError::KernelLoader);

        #[cfg(target_arch = "aarch64")]
        return linux_loader::loader::pe::PE::load(
            vm_memory,
            Some(GuestAddress(dbs_boot::get_kernel_start())),
            kernel_config.kernel_file_mut(),
            Some(high_mem_addr),
        )
        .map_err(StartMicroVmError::KernelLoader);
    }

    /// Set up the initial microVM state and start the vCPU threads.
    ///
    /// This is the main entrance of the Vm object, to bring up the virtual machine instance into
    /// running state.
    pub fn start_microvm(
        &mut self,
        event_mgr: &mut EventManager,
        vmm_seccomp_filter: BpfProgram,
        vcpu_seccomp_filter: BpfProgram,
    ) -> std::result::Result<(), StartMicroVmError> {
        info!(self.logger, "VM: received instance start command");
        if self.is_vm_initialized() {
            return Err(StartMicroVmError::MicroVMAlreadyRunning);
        }

        let request_ts = TimestampUs::default();
        self.start_instance_request_ts = request_ts.time_us;
        self.start_instance_request_cpu_ts = request_ts.cputime_us;

        self.init_dmesg_logger();
        self.check_health()?;

        // Use expect() to crash if the other thread poisoned this lock.
        self.shared_info
            .write()
            .expect("Failed to start microVM because shared info couldn't be written due to poisoned lock")
            .state = InstanceState::Starting;

        self.init_guest_memory()?;
        let vm_as = self
            .vm_as()
            .cloned()
            .ok_or(StartMicroVmError::AddressManagerError(
                AddressManagerError::GuestMemoryNotInitialized,
            ))?;

        self.init_vcpu_manager(vm_as.clone(), vcpu_seccomp_filter)
            .map_err(StartMicroVmError::Vcpu)?;
        self.init_microvm(event_mgr.epoll_manager(), vm_as.clone(), request_ts)?;
        self.init_configure_system(&vm_as)?;
        #[cfg(feature = "dbs-upcall")]
        self.init_upcall()?;

        info!(self.logger, "VM: register events");
        self.register_events(event_mgr)?;

        info!(self.logger, "VM: start vcpus");
        self.vcpu_manager()
            .map_err(StartMicroVmError::Vcpu)?
            .start_boot_vcpus(vmm_seccomp_filter)
            .map_err(StartMicroVmError::Vcpu)?;

        // Use expect() to crash if the other thread poisoned this lock.
        self.shared_info
            .write()
            .expect("Failed to start microVM because shared info couldn't be written due to poisoned lock")
            .state = InstanceState::Running;

        info!(self.logger, "VM started");
        Ok(())
    }
}

#[cfg(feature = "hotplug")]
impl Vm {
    #[cfg(feature = "dbs-upcall")]
    /// initialize upcall client for guest os
    fn new_upcall(&mut self) -> std::result::Result<(), StartMicroVmError> {
        // get vsock inner connector for upcall
        let inner_connector = self
            .device_manager
            .get_vsock_inner_connector()
            .ok_or(StartMicroVmError::UpcallMissVsock)?;
        let mut upcall_client = UpcallClient::new(
            inner_connector,
            self.epoll_manager.clone(),
            DevMgrService::default(),
        )
        .map_err(StartMicroVmError::UpcallInitError)?;

        upcall_client
            .connect()
            .map_err(StartMicroVmError::UpcallConnectError)?;
        self.upcall_client = Some(Arc::new(upcall_client));

        info!(self.logger, "upcall client init success");
        Ok(())
    }

    #[cfg(feature = "dbs-upcall")]
    fn init_upcall(&mut self) -> std::result::Result<(), StartMicroVmError> {
        info!(self.logger, "VM upcall init");
        if let Err(e) = self.new_upcall() {
            info!(
                self.logger,
                "VM upcall init failed, no support hotplug: {}", e
            );
            Err(e)
        } else {
            self.vcpu_manager()
                .map_err(StartMicroVmError::Vcpu)?
                .set_upcall_channel(self.upcall_client().clone());
            Ok(())
        }
    }

    #[cfg(feature = "dbs-upcall")]
    /// Get upcall client.
    pub fn upcall_client(&self) -> &Option<Arc<UpcallClient<DevMgrService>>> {
        &self.upcall_client
    }

    #[cfg(feature = "dbs-upcall")]
    fn create_device_hotplug_context(
        &self,
        epoll_mgr: Option<EpollManager>,
    ) -> std::result::Result<DeviceOpContext, StartMicroVmError> {
        if self.upcall_client().is_none() {
            Err(StartMicroVmError::UpcallMissVsock)
        } else if self.is_upcall_client_ready() {
            Ok(DeviceOpContext::create_hotplug_ctx(self, epoll_mgr))
        } else {
            Err(StartMicroVmError::UpcallServerNotReady)
        }
    }

    /// Resize MicroVM vCPU number
    #[cfg(feature = "dbs-upcall")]
    pub fn resize_vcpu(
        &mut self,
        config: VcpuResizeInfo,
        sync_tx: Option<Sender<bool>>,
    ) -> std::result::Result<(), VcpuResizeError> {
        if self.upcall_client().is_none() {
            Err(VcpuResizeError::UpcallClientMissing)
        } else if self.is_upcall_client_ready() {
            if let Some(vcpu_count) = config.vcpu_count {
                self.vcpu_manager()
                    .map_err(VcpuResizeError::Vcpu)?
                    .resize_vcpu(vcpu_count, sync_tx)?;

                self.vm_config.vcpu_count = vcpu_count;
            }
            Ok(())
        } else {
            Err(VcpuResizeError::UpcallServerNotReady)
        }
    }

    // We will support hotplug without upcall in future stages.
    #[cfg(not(feature = "dbs-upcall"))]
    fn create_device_hotplug_context(
        &self,
        _epoll_mgr: Option<EpollManager>,
    ) -> std::result::Result<DeviceOpContext, StartMicroVmError> {
        Err(StartMicroVmError::MicroVMAlreadyRunning)
    }
}

#[cfg(not(feature = "hotplug"))]
impl Vm {
    fn init_upcall(&mut self) -> std::result::Result<(), StartMicroVmError> {
        Ok(())
    }

    fn create_device_hotplug_context(
        &self,
        _epoll_mgr: Option<EpollManager>,
    ) -> std::result::Result<DeviceOpContext, StartMicroVmError> {
        Err(StartMicroVmError::MicroVMAlreadyRunning)
    }
}

#[cfg(test)]
pub mod tests {
    #[cfg(target_arch = "aarch64")]
    use dbs_boot::layout::GUEST_MEM_START;
    #[cfg(target_arch = "x86_64")]
    use kvm_ioctls::VcpuExit;
    use linux_loader::cmdline::Cmdline;
    use test_utils::skip_if_not_root;
    use vm_memory::GuestMemory;
    use vmm_sys_util::tempfile::TempFile;

    use super::*;
    use crate::test_utils::tests::create_vm_for_test;

    impl Vm {
        pub fn set_instance_state(&mut self, mstate: InstanceState) {
            self.shared_info
            .write()
            .expect("Failed to start microVM because shared info couldn't be written due to poisoned lock")
            .state = mstate;
        }
    }

    pub fn create_vm_instance() -> Vm {
        let instance_info = Arc::new(RwLock::new(InstanceInfo::default()));
        let epoll_manager = EpollManager::default();
        Vm::new(None, instance_info, epoll_manager).unwrap()
    }

    #[test]
    fn test_create_vm_instance() {
        skip_if_not_root!();
        let vm = create_vm_instance();
        assert!(vm.check_health().is_err());
        assert!(vm.kernel_config.is_none());
        assert!(vm.get_reset_eventfd().is_none());
        assert!(!vm.is_vm_initialized());
        assert!(!vm.is_vm_running());
        assert!(vm.reset_console().is_ok());
    }

    #[test]
    fn test_vm_init_guest_memory() {
        skip_if_not_root!();
        let vm_config = VmConfigInfo {
            vcpu_count: 1,
            max_vcpu_count: 3,
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

        let mut vm = create_vm_instance();
        vm.set_vm_config(vm_config);
        assert!(vm.init_guest_memory().is_ok());
        let vm_memory = vm.address_space.vm_memory().unwrap();

        assert_eq!(vm_memory.num_regions(), 1);
        #[cfg(target_arch = "x86_64")]
        assert_eq!(vm_memory.last_addr(), GuestAddress(0xffffff));
        #[cfg(target_arch = "aarch64")]
        assert_eq!(
            vm_memory.last_addr(),
            GuestAddress(GUEST_MEM_START + 0xffffff)
        );

        // Reconfigure an already configured vm will be ignored and just return OK.
        let vm_config = VmConfigInfo {
            vcpu_count: 1,
            max_vcpu_count: 3,
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
        vm.set_vm_config(vm_config);
        assert!(vm.init_guest_memory().is_ok());
        let vm_memory = vm.address_space.vm_memory().unwrap();
        assert_eq!(vm_memory.num_regions(), 1);
        #[cfg(target_arch = "x86_64")]
        assert_eq!(vm_memory.last_addr(), GuestAddress(0xffffff));
        #[cfg(target_arch = "aarch64")]
        assert_eq!(
            vm_memory.last_addr(),
            GuestAddress(GUEST_MEM_START + 0xffffff)
        );

        #[cfg(target_arch = "x86_64")]
        let obj_addr = GuestAddress(0xf0);
        #[cfg(target_arch = "aarch64")]
        let obj_addr = GuestAddress(GUEST_MEM_START + 0xf0);
        vm_memory.write_obj(67u8, obj_addr).unwrap();
        let read_val: u8 = vm_memory.read_obj(obj_addr).unwrap();
        assert_eq!(read_val, 67u8);
    }

    #[test]
    fn test_vm_create_devices() {
        skip_if_not_root!();
        let epoll_mgr = EpollManager::default();
        let vmm = Arc::new(Mutex::new(crate::vmm::tests::create_vmm_instance(
            epoll_mgr.clone(),
        )));

        let mut guard = vmm.lock().unwrap();
        let vm = guard.get_vm_mut().unwrap();

        let vm_config = VmConfigInfo {
            vcpu_count: 1,
            max_vcpu_count: 3,
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

        vm.set_vm_config(vm_config);
        assert!(vm.init_guest_memory().is_ok());
        assert!(vm.setup_interrupt_controller().is_ok());

        let vm_memory = vm.address_space.vm_memory().unwrap();
        assert_eq!(vm_memory.num_regions(), 1);
        #[cfg(target_arch = "x86_64")]
        assert_eq!(vm_memory.last_addr(), GuestAddress(0xffffff));
        #[cfg(target_arch = "aarch64")]
        assert_eq!(
            vm_memory.last_addr(),
            GuestAddress(GUEST_MEM_START + 0xffffff)
        );

        let kernel_file = TempFile::new().unwrap();
        let cmd_line = Cmdline::new(64).unwrap();

        vm.set_kernel_config(KernelConfigInfo::new(
            kernel_file.into_file(),
            None,
            cmd_line,
        ));

        vm.init_devices(epoll_mgr).unwrap();
    }

    #[test]
    fn test_vm_delete_devices() {
        skip_if_not_root!();
        let mut vm = create_vm_for_test();
        let epoll_mgr = EpollManager::default();

        vm.setup_interrupt_controller().unwrap();
        vm.init_devices(epoll_mgr).unwrap();
        assert!(vm.remove_devices().is_ok());
    }

    #[cfg(target_arch = "x86_64")]
    #[test]
    fn test_run_code() {
        skip_if_not_root!();

        use std::io::{self, Write};
        // This example is based on https://lwn.net/Articles/658511/
        let code = [
            0xba, 0xf8, 0x03, /* mov $0x3f8, %dx */
            0x00, 0xd8, /* add %bl, %al */
            0x04, b'0', /* add $'0', %al */
            0xee, /* out %al, (%dx) */
            0xb0, b'\n', /* mov $'\n', %al */
            0xee,  /* out %al, (%dx) */
            0xf4,  /* hlt */
        ];
        let load_addr = GuestAddress(0x1000);
        let instance_info = Arc::new(RwLock::new(InstanceInfo::default()));
        let epoll_manager = EpollManager::default();
        let mut vm = Vm::new(None, instance_info, epoll_manager).unwrap();

        let vcpu_count = 1;
        let vm_config = VmConfigInfo {
            vcpu_count,
            max_vcpu_count: 1,
            cpu_pm: "off".to_string(),
            mem_type: "shmem".to_string(),
            mem_file_path: "".to_string(),
            mem_size_mib: 10,
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

        vm.set_vm_config(vm_config);
        vm.init_guest_memory().unwrap();

        let vm_memory = vm.address_space.vm_memory().unwrap();
        vm_memory.write_obj(code, load_addr).unwrap();

        let vcpu_fd = vm.vm_fd().create_vcpu(0).unwrap();
        let mut vcpu_sregs = vcpu_fd.get_sregs().unwrap();
        assert_ne!(vcpu_sregs.cs.base, 0);
        assert_ne!(vcpu_sregs.cs.selector, 0);
        vcpu_sregs.cs.base = 0;
        vcpu_sregs.cs.selector = 0;
        vcpu_fd.set_sregs(&vcpu_sregs).unwrap();

        let mut vcpu_regs = vcpu_fd.get_regs().unwrap();

        vcpu_regs.rip = 0x1000;
        vcpu_regs.rax = 2;
        vcpu_regs.rbx = 3;
        vcpu_regs.rflags = 2;
        vcpu_fd.set_regs(&vcpu_regs).unwrap();

        match vcpu_fd.run().expect("run failed") {
            VcpuExit::IoOut(0x3f8, data) => {
                assert_eq!(data.len(), 1);
                io::stdout().write_all(data).unwrap();
            }
            VcpuExit::Hlt => {
                io::stdout().write_all(b"KVM_EXIT_HLT\n").unwrap();
            }
            r => panic!("unexpected exit reason: {:?}", r),
        }
    }
}
