// Copyright (C) 2020-2022 Alibaba Cloud. All rights reserved.
// Copyright 2018 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0
//
// Portions Copyright 2017 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the THIRD-PARTY file.

use std::fs::File;
use std::sync::mpsc::{Receiver, Sender, TryRecvError};

use log::{debug, error, info, warn};

use crate::error::{Result, StartMicroVmError, StopMicrovmError};
use crate::event_manager::EventManager;
use crate::vm::{CpuTopology, KernelConfigInfo, VmConfigInfo};
use crate::vmm::Vmm;

use self::VmConfigError::*;
use self::VmmActionError::MachineConfig;

#[cfg(feature = "virtio-blk")]
pub use crate::device_manager::blk_dev_mgr::{
    BlockDeviceConfigInfo, BlockDeviceConfigUpdateInfo, BlockDeviceError, BlockDeviceMgr,
};
#[cfg(feature = "virtio-fs")]
pub use crate::device_manager::fs_dev_mgr::{
    FsDeviceConfigInfo, FsDeviceConfigUpdateInfo, FsDeviceError, FsDeviceMgr, FsMountConfigInfo,
};
#[cfg(feature = "virtio-net")]
pub use crate::device_manager::virtio_net_dev_mgr::{
    VirtioNetDeviceConfigInfo, VirtioNetDeviceConfigUpdateInfo, VirtioNetDeviceError,
    VirtioNetDeviceMgr,
};
#[cfg(feature = "virtio-vsock")]
pub use crate::device_manager::vsock_dev_mgr::{VsockDeviceConfigInfo, VsockDeviceError};

use super::*;

/// Wrapper for all errors associated with VMM actions.
#[derive(Debug, thiserror::Error)]
pub enum VmmActionError {
    /// Invalid virtual machine instance ID.
    #[error("the virtual machine instance ID is invalid")]
    InvalidVMID,

    /// Failed to hotplug, due to Upcall not ready.
    #[error("Upcall not ready, can't hotplug device.")]
    UpcallNotReady,

    /// The action `ConfigureBootSource` failed either because of bad user input or an internal
    /// error.
    #[error("failed to configure boot source for VM: {0}")]
    BootSource(#[source] BootSourceConfigError),

    /// The action `StartMicroVm` failed either because of bad user input or an internal error.
    #[error("failed to boot the VM: {0}")]
    StartMicroVm(#[source] StartMicroVmError),

    /// The action `StopMicroVm` failed either because of bad user input or an internal error.
    #[error("failed to shutdown the VM: {0}")]
    StopMicrovm(#[source] StopMicrovmError),

    /// One of the actions `GetVmConfiguration` or `SetVmConfiguration` failed either because of bad
    /// input or an internal error.
    #[error("failed to set configuration for the VM: {0}")]
    MachineConfig(#[source] VmConfigError),

    #[cfg(feature = "virtio-vsock")]
    /// The action `InsertVsockDevice` failed either because of bad user input or an internal error.
    #[error("failed to add virtio-vsock device: {0}")]
    Vsock(#[source] VsockDeviceError),

    #[cfg(feature = "virtio-blk")]
    /// Block device related errors.
    #[error("virtio-blk device error: {0}")]
    Block(#[source] BlockDeviceError),

    #[cfg(feature = "virtio-net")]
    /// Net device related errors.
    #[error("virtio-net device error: {0}")]
    VirtioNet(#[source] VirtioNetDeviceError),

    #[cfg(feature = "virtio-fs")]
    /// The action `InsertFsDevice` failed either because of bad user input or an internal error.
    #[error("virtio-fs device: {0}")]
    FsDevice(#[source] FsDeviceError),
}

/// This enum represents the public interface of the VMM. Each action contains various
/// bits of information (ids, paths, etc.).
#[derive(Clone, Debug, PartialEq)]
pub enum VmmAction {
    /// Configure the boot source of the microVM using `BootSourceConfig`.
    /// This action can only be called before the microVM has booted.
    ConfigureBootSource(BootSourceConfig),

    /// Launch the microVM. This action can only be called before the microVM has booted.
    StartMicroVm,

    /// Shutdown the vmicroVM. This action can only be called after the microVM has booted.
    /// When vmm is used as the crate by the other process, which is need to
    /// shutdown the vcpu threads and destory all of the object.
    ShutdownMicroVm,

    /// Get the configuration of the microVM.
    GetVmConfiguration,

    /// Set the microVM configuration (memory & vcpu) using `VmConfig` as input. This
    /// action can only be called before the microVM has booted.
    SetVmConfiguration(VmConfigInfo),

    #[cfg(feature = "virtio-vsock")]
    /// Add a new vsock device or update one that already exists using the
    /// `VsockDeviceConfig` as input. This action can only be called before the microVM has
    /// booted. The response is sent using the `OutcomeSender`.
    InsertVsockDevice(VsockDeviceConfigInfo),

    #[cfg(feature = "virtio-blk")]
    /// Add a new block device or update one that already exists using the `BlockDeviceConfig` as
    /// input. This action can only be called before the microVM has booted.
    InsertBlockDevice(BlockDeviceConfigInfo),

    #[cfg(feature = "virtio-blk")]
    /// Remove a new block device for according to given drive_id
    RemoveBlockDevice(String),

    #[cfg(feature = "virtio-blk")]
    /// Update a block device, after microVM start. Currently, the only updatable properties
    /// are the RX and TX rate limiters.
    UpdateBlockDevice(BlockDeviceConfigUpdateInfo),

    #[cfg(feature = "virtio-net")]
    /// Add a new network interface config or update one that already exists using the
    /// `NetworkInterfaceConfig` as input. This action can only be called before the microVM has
    /// booted. The response is sent using the `OutcomeSender`.
    InsertNetworkDevice(VirtioNetDeviceConfigInfo),

    #[cfg(feature = "virtio-net")]
    /// Update a network interface, after microVM start. Currently, the only updatable properties
    /// are the RX and TX rate limiters.
    UpdateNetworkInterface(VirtioNetDeviceConfigUpdateInfo),

    #[cfg(feature = "virtio-fs")]
    /// Add a new shared fs device or update one that already exists using the
    /// `FsDeviceConfig` as input. This action can only be called before the microVM has
    /// booted.
    InsertFsDevice(FsDeviceConfigInfo),

    #[cfg(feature = "virtio-fs")]
    /// Attach a new virtiofs Backend fs or detach an existing virtiofs Backend fs using the
    /// `FsMountConfig` as input. This action can only be called _after_ the microVM has
    /// booted.
    ManipulateFsBackendFs(FsMountConfigInfo),

    #[cfg(feature = "virtio-fs")]
    /// Update fs rate limiter, after microVM start.
    UpdateFsDevice(FsDeviceConfigUpdateInfo),
}

/// The enum represents the response sent by the VMM in case of success. The response is either
/// empty, when no data needs to be sent, or an internal VMM structure.
#[derive(Debug)]
pub enum VmmData {
    /// No data is sent on the channel.
    Empty,
    /// The microVM configuration represented by `VmConfigInfo`.
    MachineConfiguration(Box<VmConfigInfo>),
}

/// Request data type used to communicate between the API and the VMM.
pub type VmmRequest = Box<VmmAction>;

/// Data type used to communicate between the API and the VMM.
pub type VmmRequestResult = std::result::Result<VmmData, VmmActionError>;

/// Response data type used to communicate between the API and the VMM.
pub type VmmResponse = Box<VmmRequestResult>;

/// VMM Service to handle requests from the API server.
///
/// There are two levels of API servers as below:
/// API client <--> VMM API Server <--> VMM Core
pub struct VmmService {
    from_api: Receiver<VmmRequest>,
    to_api: Sender<VmmResponse>,
    machine_config: VmConfigInfo,
}

impl VmmService {
    /// Create a new VMM API server instance.
    pub fn new(from_api: Receiver<VmmRequest>, to_api: Sender<VmmResponse>) -> Self {
        VmmService {
            from_api,
            to_api,
            machine_config: VmConfigInfo::default(),
        }
    }

    /// Handle requests from the HTTP API Server and send back replies.
    pub fn run_vmm_action(&mut self, vmm: &mut Vmm, event_mgr: &mut EventManager) -> Result<()> {
        let request = match self.from_api.try_recv() {
            Ok(t) => *t,
            Err(TryRecvError::Empty) => {
                warn!("Got a spurious notification from api thread");
                return Ok(());
            }
            Err(TryRecvError::Disconnected) => {
                panic!("The channel's sending half was disconnected. Cannot receive data.");
            }
        };
        debug!("receive vmm action: {:?}", request);

        let response = match request {
            VmmAction::ConfigureBootSource(boot_source_body) => {
                self.configure_boot_source(vmm, boot_source_body)
            }
            VmmAction::StartMicroVm => self.start_microvm(vmm, event_mgr),
            VmmAction::ShutdownMicroVm => self.shutdown_microvm(vmm),
            VmmAction::GetVmConfiguration => Ok(VmmData::MachineConfiguration(Box::new(
                self.machine_config.clone(),
            ))),
            VmmAction::SetVmConfiguration(machine_config) => {
                self.set_vm_configuration(vmm, machine_config)
            }
            #[cfg(feature = "virtio-vsock")]
            VmmAction::InsertVsockDevice(vsock_cfg) => self.add_vsock_device(vmm, vsock_cfg),
            #[cfg(feature = "virtio-blk")]
            VmmAction::InsertBlockDevice(block_device_config) => {
                self.add_block_device(vmm, event_mgr, block_device_config)
            }
            #[cfg(feature = "virtio-blk")]
            VmmAction::UpdateBlockDevice(blk_update) => {
                self.update_blk_rate_limiters(vmm, blk_update)
            }
            #[cfg(feature = "virtio-blk")]
            VmmAction::RemoveBlockDevice(drive_id) => {
                self.remove_block_device(vmm, event_mgr, &drive_id)
            }
            #[cfg(feature = "virtio-net")]
            VmmAction::InsertNetworkDevice(virtio_net_cfg) => {
                self.add_virtio_net_device(vmm, event_mgr, virtio_net_cfg)
            }
            #[cfg(feature = "virtio-net")]
            VmmAction::UpdateNetworkInterface(netif_update) => {
                self.update_net_rate_limiters(vmm, netif_update)
            }
            #[cfg(feature = "virtio-fs")]
            VmmAction::InsertFsDevice(fs_cfg) => self.add_fs_device(vmm, fs_cfg),

            #[cfg(feature = "virtio-fs")]
            VmmAction::ManipulateFsBackendFs(fs_mount_cfg) => {
                self.manipulate_fs_backend_fs(vmm, fs_mount_cfg)
            }
            #[cfg(feature = "virtio-fs")]
            VmmAction::UpdateFsDevice(fs_update_cfg) => {
                self.update_fs_rate_limiters(vmm, fs_update_cfg)
            }
        };

        debug!("send vmm response: {:?}", response);
        self.send_response(response)
    }

    fn send_response(&self, result: VmmRequestResult) -> Result<()> {
        self.to_api
            .send(Box::new(result))
            .map_err(|_| ())
            .expect("vmm: one-shot API result channel has been closed");

        Ok(())
    }

    fn configure_boot_source(
        &self,
        vmm: &mut Vmm,
        boot_source_config: BootSourceConfig,
    ) -> VmmRequestResult {
        use super::BootSourceConfigError::{
            InvalidInitrdPath, InvalidKernelCommandLine, InvalidKernelPath,
            UpdateNotAllowedPostBoot,
        };
        use super::VmmActionError::BootSource;

        let vm = vmm.get_vm_mut().ok_or(VmmActionError::InvalidVMID)?;
        if vm.is_vm_initialized() {
            return Err(BootSource(UpdateNotAllowedPostBoot));
        }

        let kernel_file = File::open(&boot_source_config.kernel_path)
            .map_err(|e| BootSource(InvalidKernelPath(e)))?;

        let initrd_file = match boot_source_config.initrd_path {
            None => None,
            Some(ref path) => Some(File::open(path).map_err(|e| BootSource(InvalidInitrdPath(e)))?),
        };

        let mut cmdline = linux_loader::cmdline::Cmdline::new(dbs_boot::layout::CMDLINE_MAX_SIZE);
        let boot_args = boot_source_config
            .boot_args
            .clone()
            .unwrap_or_else(|| String::from(DEFAULT_KERNEL_CMDLINE));
        cmdline
            .insert_str(boot_args)
            .map_err(|e| BootSource(InvalidKernelCommandLine(e)))?;

        let kernel_config = KernelConfigInfo::new(kernel_file, initrd_file, cmdline);
        vm.set_kernel_config(kernel_config);

        Ok(VmmData::Empty)
    }

    fn start_microvm(&mut self, vmm: &mut Vmm, event_mgr: &mut EventManager) -> VmmRequestResult {
        use self::StartMicroVmError::MicroVMAlreadyRunning;
        use self::VmmActionError::StartMicroVm;

        let vmm_seccomp_filter = vmm.vmm_seccomp_filter();
        let vcpu_seccomp_filter = vmm.vcpu_seccomp_filter();
        let vm = vmm.get_vm_mut().ok_or(VmmActionError::InvalidVMID)?;
        if vm.is_vm_initialized() {
            return Err(StartMicroVm(MicroVMAlreadyRunning));
        }

        vm.start_microvm(event_mgr, vmm_seccomp_filter, vcpu_seccomp_filter)
            .map(|_| VmmData::Empty)
            .map_err(StartMicroVm)
    }

    fn shutdown_microvm(&mut self, vmm: &mut Vmm) -> VmmRequestResult {
        vmm.event_ctx.exit_evt_triggered = true;

        Ok(VmmData::Empty)
    }

    /// Set virtual machine configuration.
    pub fn set_vm_configuration(
        &mut self,
        vmm: &mut Vmm,
        machine_config: VmConfigInfo,
    ) -> VmmRequestResult {
        let vm = vmm.get_vm_mut().ok_or(VmmActionError::InvalidVMID)?;
        if vm.is_vm_initialized() {
            return Err(MachineConfig(UpdateNotAllowedPostBoot));
        }

        // If the check is successful, set it up together.
        let mut config = vm.vm_config().clone();
        if config.vcpu_count != machine_config.vcpu_count {
            let vcpu_count = machine_config.vcpu_count;
            // Check that the vcpu_count value is >=1.
            if vcpu_count == 0 {
                return Err(MachineConfig(InvalidVcpuCount(vcpu_count)));
            }
            config.vcpu_count = vcpu_count;
        }

        if config.cpu_topology != machine_config.cpu_topology {
            let cpu_topology = &machine_config.cpu_topology;
            config.cpu_topology = handle_cpu_topology(cpu_topology, config.vcpu_count)?.clone();
        } else {
            // the same default
            let mut default_cpu_topology = CpuTopology {
                threads_per_core: 1,
                cores_per_die: config.vcpu_count,
                dies_per_socket: 1,
                sockets: 1,
            };
            if machine_config.max_vcpu_count > config.vcpu_count {
                default_cpu_topology.cores_per_die = machine_config.max_vcpu_count;
            }
            config.cpu_topology = default_cpu_topology;
        }
        let cpu_topology = &config.cpu_topology;
        let max_vcpu_from_topo = cpu_topology.threads_per_core
            * cpu_topology.cores_per_die
            * cpu_topology.dies_per_socket
            * cpu_topology.sockets;
        // If the max_vcpu_count inferred by cpu_topology is not equal to
        // max_vcpu_count, max_vcpu_count will be changed. currently, max vcpu size
        // is used when cpu_topology is not defined and help define the cores_per_die
        // for the default cpu topology.
        let mut max_vcpu_count = machine_config.max_vcpu_count;
        if max_vcpu_count < config.vcpu_count {
            return Err(MachineConfig(InvalidMaxVcpuCount(max_vcpu_count)));
        }
        if max_vcpu_from_topo != max_vcpu_count {
            max_vcpu_count = max_vcpu_from_topo;
            info!("Since max_vcpu_count is not equal to cpu topo information, we have changed the max vcpu count to {}", max_vcpu_from_topo);
        }
        config.max_vcpu_count = max_vcpu_count;

        config.cpu_pm = machine_config.cpu_pm;
        config.mem_type = machine_config.mem_type;

        let mem_size_mib_value = machine_config.mem_size_mib;
        // Support 1TB memory at most, 2MB aligned for huge page.
        if mem_size_mib_value == 0 || mem_size_mib_value > 0x10_0000 || mem_size_mib_value % 2 != 0
        {
            return Err(MachineConfig(InvalidMemorySize(mem_size_mib_value)));
        }
        config.mem_size_mib = mem_size_mib_value;

        config.mem_file_path = machine_config.mem_file_path.clone();

        if config.mem_type == "hugetlbfs" && config.mem_file_path.is_empty() {
            return Err(MachineConfig(InvalidMemFilePath("".to_owned())));
        }
        config.vpmu_feature = machine_config.vpmu_feature;

        let vm_id = vm.shared_info().read().unwrap().id.clone();
        let serial_path = match machine_config.serial_path {
            Some(value) => value,
            None => {
                if config.serial_path.is_none() {
                    String::from("/run/dragonball/") + &vm_id + "_com1"
                } else {
                    // Safe to unwrap() because we have checked it has a value.
                    config.serial_path.as_ref().unwrap().clone()
                }
            }
        };
        config.serial_path = Some(serial_path);

        vm.set_vm_config(config.clone());
        self.machine_config = config;

        Ok(VmmData::Empty)
    }

    #[cfg(feature = "virtio-vsock")]
    fn add_vsock_device(&self, vmm: &mut Vmm, config: VsockDeviceConfigInfo) -> VmmRequestResult {
        let vm = vmm.get_vm_mut().ok_or(VmmActionError::InvalidVMID)?;
        if vm.is_vm_initialized() {
            return Err(VmmActionError::Vsock(
                VsockDeviceError::UpdateNotAllowedPostBoot,
            ));
        }

        // VMADDR_CID_ANY (-1U) means any address for binding;
        // VMADDR_CID_HYPERVISOR (0) is reserved for services built into the hypervisor;
        // VMADDR_CID_RESERVED (1) must not be used;
        // VMADDR_CID_HOST (2) is the well-known address of the host.
        if config.guest_cid <= 2 {
            return Err(VmmActionError::Vsock(VsockDeviceError::GuestCIDInvalid(
                config.guest_cid,
            )));
        }

        info!("add_vsock_device: {:?}", config);
        let ctx = vm.create_device_op_context(None).map_err(|e| {
            info!("create device op context error: {:?}", e);
            VmmActionError::Vsock(VsockDeviceError::UpdateNotAllowedPostBoot)
        })?;

        vm.device_manager_mut()
            .vsock_manager
            .insert_device(ctx, config)
            .map(|_| VmmData::Empty)
            .map_err(VmmActionError::Vsock)
    }

    #[cfg(feature = "virtio-blk")]
    // Only call this function as part of the API.
    // If the drive_id does not exist, a new Block Device Config is added to the list.
    fn add_block_device(
        &mut self,
        vmm: &mut Vmm,
        event_mgr: &mut EventManager,
        config: BlockDeviceConfigInfo,
    ) -> VmmRequestResult {
        let vm = vmm.get_vm_mut().ok_or(VmmActionError::InvalidVMID)?;
        let ctx = vm
            .create_device_op_context(Some(event_mgr.epoll_manager()))
            .map_err(|e| {
                if let StartMicroVmError::UpcallNotReady = e {
                    return VmmActionError::UpcallNotReady;
                }
                VmmActionError::Block(BlockDeviceError::UpdateNotAllowedPostBoot)
            })?;

        BlockDeviceMgr::insert_device(vm.device_manager_mut(), ctx, config)
            .map(|_| VmmData::Empty)
            .map_err(VmmActionError::Block)
    }

    #[cfg(feature = "virtio-blk")]
    /// Updates configuration for an emulated net device as described in `config`.
    fn update_blk_rate_limiters(
        &mut self,
        vmm: &mut Vmm,
        config: BlockDeviceConfigUpdateInfo,
    ) -> VmmRequestResult {
        let vm = vmm.get_vm_mut().ok_or(VmmActionError::InvalidVMID)?;

        BlockDeviceMgr::update_device_ratelimiters(vm.device_manager_mut(), config)
            .map(|_| VmmData::Empty)
            .map_err(VmmActionError::Block)
    }

    #[cfg(feature = "virtio-blk")]
    // Remove the device
    fn remove_block_device(
        &mut self,
        vmm: &mut Vmm,
        event_mgr: &mut EventManager,
        drive_id: &str,
    ) -> VmmRequestResult {
        let vm = vmm.get_vm_mut().ok_or(VmmActionError::InvalidVMID)?;
        let ctx = vm
            .create_device_op_context(Some(event_mgr.epoll_manager()))
            .map_err(|_| VmmActionError::Block(BlockDeviceError::UpdateNotAllowedPostBoot))?;

        BlockDeviceMgr::remove_device(vm.device_manager_mut(), ctx, drive_id)
            .map(|_| VmmData::Empty)
            .map_err(VmmActionError::Block)
    }

    #[cfg(feature = "virtio-net")]
    fn add_virtio_net_device(
        &mut self,
        vmm: &mut Vmm,
        event_mgr: &mut EventManager,
        config: VirtioNetDeviceConfigInfo,
    ) -> VmmRequestResult {
        let vm = vmm.get_vm_mut().ok_or(VmmActionError::InvalidVMID)?;
        let ctx = vm
            .create_device_op_context(Some(event_mgr.epoll_manager()))
            .map_err(|e| {
                if let StartMicroVmError::MicroVMAlreadyRunning = e {
                    VmmActionError::VirtioNet(VirtioNetDeviceError::UpdateNotAllowedPostBoot)
                } else if let StartMicroVmError::UpcallNotReady = e {
                    VmmActionError::UpcallNotReady
                } else {
                    VmmActionError::StartMicroVm(e)
                }
            })?;

        VirtioNetDeviceMgr::insert_device(vm.device_manager_mut(), ctx, config)
            .map(|_| VmmData::Empty)
            .map_err(VmmActionError::VirtioNet)
    }

    #[cfg(feature = "virtio-net")]
    fn update_net_rate_limiters(
        &mut self,
        vmm: &mut Vmm,
        config: VirtioNetDeviceConfigUpdateInfo,
    ) -> VmmRequestResult {
        let vm = vmm.get_vm_mut().ok_or(VmmActionError::InvalidVMID)?;

        VirtioNetDeviceMgr::update_device_ratelimiters(vm.device_manager_mut(), config)
            .map(|_| VmmData::Empty)
            .map_err(VmmActionError::VirtioNet)
    }

    #[cfg(feature = "virtio-fs")]
    fn add_fs_device(&mut self, vmm: &mut Vmm, config: FsDeviceConfigInfo) -> VmmRequestResult {
        let vm = vmm.get_vm_mut().ok_or(VmmActionError::InvalidVMID)?;
        let hotplug = vm.is_vm_initialized();
        if !cfg!(feature = "hotplug") && hotplug {
            return Err(VmmActionError::FsDevice(
                FsDeviceError::UpdateNotAllowedPostBoot,
            ));
        }

        let ctx = vm.create_device_op_context(None).map_err(|e| {
            info!("create device op context error: {:?}", e);
            VmmActionError::FsDevice(FsDeviceError::UpdateNotAllowedPostBoot)
        })?;
        FsDeviceMgr::insert_device(vm.device_manager_mut(), ctx, config)
            .map(|_| VmmData::Empty)
            .map_err(VmmActionError::FsDevice)
    }

    #[cfg(feature = "virtio-fs")]
    fn manipulate_fs_backend_fs(
        &self,
        vmm: &mut Vmm,
        config: FsMountConfigInfo,
    ) -> VmmRequestResult {
        let vm = vmm.get_vm_mut().ok_or(VmmActionError::InvalidVMID)?;

        if !vm.is_vm_initialized() {
            return Err(VmmActionError::FsDevice(FsDeviceError::MicroVMNotRunning));
        }

        FsDeviceMgr::manipulate_backend_fs(vm.device_manager_mut(), config)
            .map(|_| VmmData::Empty)
            .map_err(VmmActionError::FsDevice)
    }

    #[cfg(feature = "virtio-fs")]
    fn update_fs_rate_limiters(
        &self,
        vmm: &mut Vmm,
        config: FsDeviceConfigUpdateInfo,
    ) -> VmmRequestResult {
        let vm = vmm.get_vm_mut().ok_or(VmmActionError::InvalidVMID)?;

        if !vm.is_vm_initialized() {
            return Err(VmmActionError::FsDevice(FsDeviceError::MicroVMNotRunning));
        }

        FsDeviceMgr::update_device_ratelimiters(vm.device_manager_mut(), config)
            .map(|_| VmmData::Empty)
            .map_err(VmmActionError::FsDevice)
    }
}

fn handle_cpu_topology(
    cpu_topology: &CpuTopology,
    vcpu_count: u8,
) -> std::result::Result<&CpuTopology, VmmActionError> {
    // Check if dies_per_socket, cores_per_die, threads_per_core and socket number is valid
    if cpu_topology.threads_per_core < 1 || cpu_topology.threads_per_core > 2 {
        return Err(MachineConfig(InvalidThreadsPerCore(
            cpu_topology.threads_per_core,
        )));
    }
    let vcpu_count_from_topo = cpu_topology
        .sockets
        .checked_mul(cpu_topology.dies_per_socket)
        .ok_or(MachineConfig(VcpuCountExceedsMaximum))?
        .checked_mul(cpu_topology.cores_per_die)
        .ok_or(MachineConfig(VcpuCountExceedsMaximum))?
        .checked_mul(cpu_topology.threads_per_core)
        .ok_or(MachineConfig(VcpuCountExceedsMaximum))?;
    if vcpu_count_from_topo > MAX_SUPPORTED_VCPUS {
        return Err(MachineConfig(VcpuCountExceedsMaximum));
    }
    if vcpu_count_from_topo < vcpu_count {
        return Err(MachineConfig(InvalidCpuTopology(vcpu_count_from_topo)));
    }

    Ok(cpu_topology)
}
