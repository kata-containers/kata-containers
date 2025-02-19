// Copyright (C) 2020-2022 Alibaba Cloud. All rights reserved.
// Copyright 2018 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0
//
// Portions Copyright 2017 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the THIRD-PARTY file.

use std::fs::File;
use std::sync::{Arc, Mutex};

use crossbeam_channel::{unbounded, Receiver, Sender, TryRecvError};
use log::{debug, error, info, warn};
use tracing::instrument;

use crate::error::{Result, StartMicroVmError, StopMicrovmError};
use crate::event_manager::EventManager;
use crate::tracer::{DragonballTracer, TraceError, TraceInfo};
use crate::vcpu::VcpuManagerError;
use crate::vm::{CpuTopology, KernelConfigInfo, VmConfigInfo};
use crate::vmm::Vmm;

use crate::hypervisor_metrics::get_hypervisor_metrics;

use self::VmConfigError::*;
use self::VmmActionError::MachineConfig;

#[cfg(feature = "virtio-balloon")]
pub use crate::device_manager::balloon_dev_mgr::{BalloonDeviceConfigInfo, BalloonDeviceError};
#[cfg(any(feature = "virtio-blk", feature = "vhost-user-blk"))]
pub use crate::device_manager::blk_dev_mgr::{
    BlockDeviceConfigInfo, BlockDeviceConfigUpdateInfo, BlockDeviceError, BlockDeviceMgr,
};
#[cfg(any(feature = "virtio-fs", feature = "vhost-user-fs"))]
pub use crate::device_manager::fs_dev_mgr::{
    FsDeviceConfigInfo, FsDeviceConfigUpdateInfo, FsDeviceError, FsDeviceMgr, FsMountConfigInfo,
};
#[cfg(feature = "virtio-mem")]
pub use crate::device_manager::mem_dev_mgr::{MemDeviceConfigInfo, MemDeviceError};
#[cfg(feature = "host-device")]
use crate::device_manager::vfio_dev_mgr::{HostDeviceConfig, VfioDeviceError};
#[cfg(feature = "vhost-net")]
pub use crate::device_manager::vhost_net_dev_mgr::{
    VhostNetDeviceConfigInfo, VhostNetDeviceError, VhostNetDeviceMgr,
};
#[cfg(feature = "vhost-user-net")]
use crate::device_manager::vhost_user_net_dev_mgr::{
    VhostUserNetDeviceConfigInfo, VhostUserNetDeviceError, VhostUserNetDeviceMgr,
};
#[cfg(feature = "virtio-net")]
pub use crate::device_manager::virtio_net_dev_mgr::{
    VirtioNetDeviceConfigInfo, VirtioNetDeviceConfigUpdateInfo, VirtioNetDeviceError,
    VirtioNetDeviceMgr,
};
#[cfg(feature = "virtio-vsock")]
pub use crate::device_manager::vsock_dev_mgr::{VsockDeviceConfigInfo, VsockDeviceError};
#[cfg(feature = "hotplug")]
pub use crate::vcpu::{VcpuResizeError, VcpuResizeInfo};

use super::*;

/// Wrapper for all errors associated with VMM actions.
#[derive(Debug, thiserror::Error)]
pub enum VmmActionError {
    /// Invalid virtual machine instance ID.
    #[error("the virtual machine instance ID is invalid")]
    InvalidVMID,

    /// VM doesn't exist and can't get VM information.
    #[error("VM doesn't exist and can't get VM information")]
    VmNotExist,

    /// Failed to hotplug, due to Upcall not ready.
    #[error("Upcall not ready, can't hotplug device.")]
    UpcallServerNotReady,

    /// Error when get prometheus metrics.
    /// Currently does not distinguish between error types for metrics.
    #[error("failed to get hypervisor metrics")]
    GetHypervisorMetrics,

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

    #[cfg(any(feature = "virtio-blk", feature = "vhost-user-blk"))]
    /// Block device related errors.
    #[error("virtio-blk device error: {0}")]
    Block(#[source] BlockDeviceError),

    #[cfg(feature = "virtio-net")]
    /// Net device related errors.
    #[error("virtio-net device error: {0}")]
    VirtioNet(#[source] VirtioNetDeviceError),

    #[cfg(feature = "vhost-net")]
    #[error("vhost-net device error: {0:?}")]
    /// Vhost-net device relared errors.
    VhostNet(#[source] VhostNetDeviceError),

    #[error("vhost-user-net device error: {0:?}")]
    #[cfg(feature = "vhost-user-net")]
    /// Vhost-user-net device relared errors.
    VhostUserNet(#[source] VhostUserNetDeviceError),

    #[cfg(any(feature = "virtio-fs", feature = "vhost-user-fs"))]
    /// The action `InsertFsDevice` failed either because of bad user input or an internal error.
    #[error("virtio-fs device error: {0}")]
    FsDevice(#[source] FsDeviceError),

    #[cfg(feature = "hotplug")]
    /// The action `ResizeVcpu` Failed
    #[error("vcpu resize error : {0}")]
    ResizeVcpu(#[source] VcpuResizeError),

    /// Cannot access address space.
    #[error("Cannot access address space.")]
    AddressSpaceNotInitialized,

    #[cfg(feature = "virtio-mem")]
    /// Mem device related errors.
    #[error("virtio-mem device error: {0}")]
    Mem(#[source] MemDeviceError),

    #[cfg(feature = "virtio-balloon")]
    /// Balloon device related errors.
    #[error("virtio-balloon device error: {0}")]
    Balloon(#[source] BalloonDeviceError),
    /// Setup tracing Failed.
    #[error("Setup tracing failed: {0}")]
    SetupTracingFailed(#[source] TraceError),
    /// End tracing Failed.
    #[error("End tracing failed: {0}")]
    EndTracingFailed(#[source] TraceError),

    #[cfg(feature = "host-device")]
    /// The action `InsertHostDevice` failed either because of bad user input or an internal error.
    #[error("failed to add VFIO passthrough device: {0:?}")]
    HostDeviceConfig(#[source] VfioDeviceError),
    #[cfg(feature = "host-device")]
    /// The action 'RemoveHostDevice' failed because of vcpu manager internal error.
    #[error("remove host device error: {0}")]
    RemoveHostDevice(#[source] VcpuManagerError),
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

    /// Get Prometheus Metrics.
    GetHypervisorMetrics,

    /// Set the microVM configuration (memory & vcpu) using `VmConfig` as input. This
    /// action can only be called before the microVM has booted.
    SetVmConfiguration(VmConfigInfo),
    /// Set the VMM tracing.
    SetHypervisorTracing(TraceInfo),
    /// End VMM tracing.
    EndHypervisorTracing,

    #[cfg(feature = "virtio-vsock")]
    /// Add a new vsock device or update one that already exists using the
    /// `VsockDeviceConfig` as input. This action can only be called before the microVM has
    /// booted. The response is sent using the `OutcomeSender`.
    InsertVsockDevice(VsockDeviceConfigInfo),

    #[cfg(any(feature = "virtio-blk", feature = "vhost-user-blk"))]
    /// Add a new block device or update one that already exists using the `BlockDeviceConfig` as
    /// input. This action can only be called before the microVM has booted.
    InsertBlockDevice(BlockDeviceConfigInfo),

    #[cfg(any(feature = "virtio-blk", feature = "vhost-user-blk"))]
    /// Remove a new block device for according to given drive_id
    RemoveBlockDevice(String),

    #[cfg(any(feature = "virtio-blk", feature = "vhost-user-blk"))]
    /// Update a block device, after microVM start. Currently, the only updatable properties
    /// are the RX and TX rate limiters.
    UpdateBlockDevice(BlockDeviceConfigUpdateInfo),

    #[cfg(any(
        feature = "virtio-net",
        feature = "vhost-net",
        feature = "vhost-user-net"
    ))]
    /// Add a new network interface config or update one that already exists using the
    /// `NetworkInterfaceConfig` as input. This action can only be called before the microVM has
    /// booted. The response is sent using the `OutcomeSender`.
    InsertNetworkDevice(NetworkInterfaceConfig),

    #[cfg(feature = "virtio-net")]
    /// Update a network interface, after microVM start. Currently, the only updatable properties
    /// are the RX and TX rate limiters.
    /// TODO: vhost-net rate limiters aren't implemented, see:
    /// https://github.com/kata-containers/kata-containers/issues/8327
    UpdateNetworkInterface(VirtioNetDeviceConfigUpdateInfo),

    #[cfg(any(feature = "virtio-fs", feature = "vhost-user-fs"))]
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

    #[cfg(feature = "hotplug")]
    /// Resize Vcpu number in the guest.
    ResizeVcpu(VcpuResizeInfo),

    #[cfg(feature = "virtio-mem")]
    /// Add a new mem device or update one that already exists using the `MemDeviceConfig` as input.
    InsertMemDevice(MemDeviceConfigInfo),

    #[cfg(feature = "virtio-balloon")]
    /// Add a new balloon device or update one that already exists using the `BalloonDeviceConfig`
    /// as input.
    InsertBalloonDevice(BalloonDeviceConfigInfo),

    #[cfg(feature = "host-device")]
    /// Add a VFIO assignment host device or update that already exists
    InsertHostDevice(HostDeviceConfig),

    #[cfg(feature = "host-device")]
    /// Prepare to remove a VFIO assignment host device that already exists
    PrepareRemoveHostDevice(String),

    #[cfg(feature = "host-device")]
    /// Add a VFIO assignment host device or update that already exists
    RemoveHostDevice(String),
}

/// The enum represents the response sent by the VMM in case of success. The response is either
/// empty, when no data needs to be sent, or an internal VMM structure.
#[derive(Debug)]
pub enum VmmData {
    /// No data is sent on the channel.
    Empty,
    /// The microVM configuration represented by `VmConfigInfo`.
    MachineConfiguration(Box<VmConfigInfo>),
    /// Prometheus Metrics represented by String.
    HypervisorMetrics(String),
    /// Return vfio device's slot number in guest.
    VfioDeviceData(Option<u8>),
    /// Sync Hotplug
    SyncHotplug((Sender<Option<i32>>, Receiver<Option<i32>>)),
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
    tracer: Arc<Mutex<DragonballTracer>>,
}

impl VmmService {
    /// Create a new VMM API server instance.
    pub fn new(from_api: Receiver<VmmRequest>, to_api: Sender<VmmResponse>) -> Self {
        VmmService {
            from_api,
            to_api,
            machine_config: VmConfigInfo::default(),
            tracer: Arc::new(Mutex::new(DragonballTracer::new())),
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
            VmmAction::GetHypervisorMetrics => self.get_hypervisor_metrics(),
            VmmAction::SetVmConfiguration(machine_config) => {
                self.set_vm_configuration(vmm, machine_config)
            }
            VmmAction::SetHypervisorTracing(trace_info) => self.setup_tracing(trace_info),
            VmmAction::EndHypervisorTracing => self.end_tracing(),
            #[cfg(feature = "virtio-vsock")]
            VmmAction::InsertVsockDevice(vsock_cfg) => self.add_vsock_device(vmm, vsock_cfg),
            #[cfg(any(feature = "virtio-blk", feature = "vhost-user-blk"))]
            VmmAction::InsertBlockDevice(block_device_config) => {
                self.add_block_device(vmm, event_mgr, block_device_config)
            }
            #[cfg(any(feature = "virtio-blk", feature = "vhost-user-blk"))]
            VmmAction::UpdateBlockDevice(blk_update) => {
                self.update_blk_rate_limiters(vmm, blk_update)
            }
            #[cfg(any(feature = "virtio-blk", feature = "vhost-user-blk"))]
            VmmAction::RemoveBlockDevice(drive_id) => {
                self.remove_block_device(vmm, event_mgr, &drive_id)
            }
            #[cfg(any(
                feature = "virtio-net",
                feature = "vhost-net",
                feature = "vhost-user-net"
            ))]
            VmmAction::InsertNetworkDevice(config) => match config.backend {
                #[cfg(feature = "virtio-net")]
                Backend::Virtio(_) => self.add_virtio_net_device(vmm, event_mgr, config.into()),
                #[cfg(feature = "vhost-net")]
                Backend::Vhost(_) => self.add_vhost_net_device(vmm, event_mgr, config.into()),
                #[cfg(feature = "vhost-user-net")]
                Backend::VhostUser(_) => {
                    self.add_vhost_user_net_device(vmm, event_mgr, config.into())
                }
            },
            #[cfg(feature = "virtio-net")]
            VmmAction::UpdateNetworkInterface(netif_update) => {
                self.update_net_rate_limiters(vmm, netif_update)
            }
            #[cfg(any(feature = "virtio-fs", feature = "vhost-user-fs"))]
            VmmAction::InsertFsDevice(fs_cfg) => self.add_fs_device(vmm, fs_cfg),

            #[cfg(feature = "virtio-fs")]
            VmmAction::ManipulateFsBackendFs(fs_mount_cfg) => {
                self.manipulate_fs_backend_fs(vmm, fs_mount_cfg)
            }
            #[cfg(any(feature = "virtio-fs", feature = "vhost-user-fs"))]
            VmmAction::UpdateFsDevice(fs_update_cfg) => {
                self.update_fs_rate_limiters(vmm, fs_update_cfg)
            }
            #[cfg(feature = "hotplug")]
            VmmAction::ResizeVcpu(vcpu_resize_cfg) => self.resize_vcpu(vmm, vcpu_resize_cfg),
            #[cfg(feature = "virtio-mem")]
            VmmAction::InsertMemDevice(mem_cfg) => self.add_mem_device(vmm, event_mgr, mem_cfg),
            #[cfg(feature = "virtio-balloon")]
            VmmAction::InsertBalloonDevice(balloon_cfg) => {
                self.add_balloon_device(vmm, event_mgr, balloon_cfg)
            }
            #[cfg(feature = "host-device")]
            VmmAction::InsertHostDevice(mut hostdev_cfg) => {
                self.add_vfio_device(vmm, &mut hostdev_cfg)
            }
            #[cfg(feature = "host-device")]
            VmmAction::PrepareRemoveHostDevice(hostdev_id) => {
                self.prepare_remove_vfio_device(vmm, &hostdev_id)
            }
            #[cfg(feature = "host-device")]
            VmmAction::RemoveHostDevice(hostdev_cfg) => self.remove_vfio_device(vmm, &hostdev_cfg),
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

    #[instrument(skip(self))]
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

        let mut cmdline = linux_loader::cmdline::Cmdline::new(dbs_boot::layout::CMDLINE_MAX_SIZE)
            .map_err(|err| BootSource(InvalidKernelCommandLine(err)))?;
        let boot_args = boot_source_config
            .boot_args
            .unwrap_or_else(|| String::from(DEFAULT_KERNEL_CMDLINE));
        cmdline
            .insert_str(boot_args)
            .map_err(|e| BootSource(InvalidKernelCommandLine(e)))?;

        let kernel_config = KernelConfigInfo::new(kernel_file, initrd_file, cmdline);
        vm.set_kernel_config(kernel_config);

        Ok(VmmData::Empty)
    }

    #[instrument(skip(self, event_mgr))]
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

    #[instrument(skip(self))]
    fn shutdown_microvm(&mut self, vmm: &mut Vmm) -> VmmRequestResult {
        vmm.event_ctx.exit_evt_triggered = true;

        Ok(VmmData::Empty)
    }

    /// Get prometheus metrics.
    #[instrument(skip(self))]
    fn get_hypervisor_metrics(&self) -> VmmRequestResult {
        get_hypervisor_metrics()
            .map_err(|_| VmmActionError::GetHypervisorMetrics)
            .map(VmmData::HypervisorMetrics)
    }

    /// Set virtual machine configuration.
    #[instrument(skip(self))]
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

        // If serial_path is:
        // - None, legacy_manager will create_stdio_console.
        // - Some(path), legacy_manager will create_socket_console on that path.
        config.serial_path = machine_config.serial_path;

        config.pci_hotplug_enabled = machine_config.pci_hotplug_enabled;

        vm.set_vm_config(config.clone());
        self.machine_config = config;

        Ok(VmmData::Empty)
    }

    /// Setup dragonball tracing.
    fn setup_tracing(&self, trace_info: TraceInfo) -> VmmRequestResult {
        let mut tracer = self.tracer.lock().unwrap();
        tracer
            .setup_tracing(trace_info)
            .map(|_| VmmData::Empty)
            .map_err(VmmActionError::SetupTracingFailed)
    }

    /// Get dragonball tracer.
    pub fn tracer(&self) -> Arc<Mutex<DragonballTracer>> {
        self.tracer.clone()
    }

    /// end dragonball tracing.
    #[instrument(skip(self))]
    fn end_tracing(&self) -> VmmRequestResult {
        let mut tracer = self.tracer.lock().unwrap();
        tracer
            .end_tracing()
            .map(|_| VmmData::Empty)
            .map_err(VmmActionError::EndTracingFailed)
    }

    #[cfg(feature = "virtio-vsock")]
    #[instrument(skip(self))]
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

    #[cfg(any(feature = "virtio-blk", feature = "vhost-user-blk"))]
    // Only call this function as part of the API.
    // If the drive_id does not exist, a new Block Device Config is added to the list.
    #[instrument(skip(self, event_mgr))]
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
                if let StartMicroVmError::UpcallServerNotReady = e {
                    return VmmActionError::UpcallServerNotReady;
                }
                VmmActionError::Block(BlockDeviceError::UpdateNotAllowedPostBoot)
            })?;

        vm.device_manager_mut()
            .block_manager
            .insert_device(ctx, config)
            .map(|_| VmmData::Empty)
            .map_err(VmmActionError::Block)
    }

    #[cfg(any(feature = "virtio-blk", feature = "vhost-user-blk"))]
    /// Updates configuration for an emulated net device as described in `config`.
    #[instrument(skip(self))]
    fn update_blk_rate_limiters(
        &mut self,
        vmm: &mut Vmm,
        config: BlockDeviceConfigUpdateInfo,
    ) -> VmmRequestResult {
        let vm = vmm.get_vm_mut().ok_or(VmmActionError::InvalidVMID)?;

        vm.device_manager_mut()
            .block_manager
            .update_device_ratelimiters(config)
            .map(|_| VmmData::Empty)
            .map_err(VmmActionError::Block)
    }

    #[cfg(any(feature = "virtio-blk", feature = "vhost-user-blk"))]
    // Remove the device
    #[instrument(skip(self, event_mgr))]
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

        vm.device_manager_mut()
            .block_manager
            .remove_device(ctx, drive_id)
            .map(|_| VmmData::Empty)
            .map_err(VmmActionError::Block)
    }

    #[cfg(feature = "virtio-net")]
    #[instrument(skip(self, event_mgr))]
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
                } else if let StartMicroVmError::UpcallServerNotReady = e {
                    VmmActionError::UpcallServerNotReady
                } else {
                    VmmActionError::StartMicroVm(e)
                }
            })?;

        vm.device_manager_mut()
            .virtio_net_manager
            .insert_device(ctx, config)
            .map(|_| VmmData::Empty)
            .map_err(VmmActionError::VirtioNet)
    }

    #[cfg(feature = "virtio-net")]
    #[instrument(skip(self))]
    fn update_net_rate_limiters(
        &mut self,
        vmm: &mut Vmm,
        config: VirtioNetDeviceConfigUpdateInfo,
    ) -> VmmRequestResult {
        let vm = vmm.get_vm_mut().ok_or(VmmActionError::InvalidVMID)?;

        vm.device_manager_mut()
            .virtio_net_manager
            .update_device_ratelimiters(config)
            .map(|_| VmmData::Empty)
            .map_err(VmmActionError::VirtioNet)
    }

    #[cfg(feature = "vhost-net")]
    fn add_vhost_net_device(
        &mut self,
        vmm: &mut Vmm,
        event_mgr: &mut EventManager,
        config: VhostNetDeviceConfigInfo,
    ) -> VmmRequestResult {
        let vm = vmm.get_vm_mut().ok_or(VmmActionError::InvalidVMID)?;
        let ctx = vm
            .create_device_op_context(Some(event_mgr.epoll_manager()))
            .map_err(|err| match err {
                StartMicroVmError::MicroVMAlreadyRunning => {
                    VmmActionError::VhostNet(VhostNetDeviceError::UpdateNotAllowedPostBoot)
                }
                StartMicroVmError::UpcallServerNotReady => VmmActionError::UpcallServerNotReady,
                _ => VmmActionError::StartMicroVm(err),
            })?;
        VhostNetDeviceMgr::insert_device(vm.device_manager_mut(), ctx, config)
            .map(|_| VmmData::Empty)
            .map_err(VmmActionError::VhostNet)
    }

    #[cfg(feature = "vhost-user-net")]
    fn add_vhost_user_net_device(
        &mut self,
        vmm: &mut Vmm,
        event_mgr: &mut EventManager,
        config: VhostUserNetDeviceConfigInfo,
    ) -> VmmRequestResult {
        let vm = vmm.get_vm_mut().ok_or(VmmActionError::InvalidVMID)?;
        let ctx = vm
            .create_device_op_context(Some(event_mgr.epoll_manager()))
            .map_err(|err| {
                if let StartMicroVmError::MicroVMAlreadyRunning = err {
                    VmmActionError::VhostUserNet(VhostUserNetDeviceError::UpdateNotAllowedPostBoot)
                } else if let StartMicroVmError::UpcallServerNotReady = err {
                    VmmActionError::UpcallServerNotReady
                } else {
                    VmmActionError::StartMicroVm(err)
                }
            })?;
        VhostUserNetDeviceMgr::insert_device(vm.device_manager_mut(), ctx, config)
            .map(|_| VmmData::Empty)
            .map_err(VmmActionError::VhostUserNet)
    }

    #[cfg(any(feature = "virtio-fs", feature = "vhost-user-fs"))]
    #[instrument(skip(self))]
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
    #[instrument(skip(self))]
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
    #[instrument(skip(self))]
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

    #[cfg(feature = "host-device")]
    fn add_vfio_device(&self, vmm: &mut Vmm, config: &mut HostDeviceConfig) -> VmmRequestResult {
        let vm = vmm.get_vm_mut().ok_or(VmmActionError::HostDeviceConfig(
            VfioDeviceError::InvalidVMID,
        ))?;
        info!("add_vfio_device: {:?}", config);

        let mut ctx = vm.create_device_op_context(None).map_err(|e| {
            info!("create device op context error: {:?}", e);
            if let StartMicroVmError::MicroVMAlreadyRunning = e {
                VmmActionError::HostDeviceConfig(VfioDeviceError::UpdateNotAllowedPostBoot)
            } else if let StartMicroVmError::UpcallServerNotReady = e {
                VmmActionError::UpcallServerNotReady
            } else {
                VmmActionError::StartMicroVm(e)
            }
        })?;

        vm.device_manager()
            .vfio_manager
            .lock()
            .unwrap()
            .insert_device(&mut ctx, config)
            .map_err(VmmActionError::HostDeviceConfig)?;

        Ok(VmmData::VfioDeviceData(config.dev_config.guest_dev_id))
    }

    // using upcall to unplug the pci device in the guest
    #[cfg(feature = "host-device")]
    fn prepare_remove_vfio_device(&mut self, vmm: &mut Vmm, hostdev_id: &str) -> VmmRequestResult {
        let vm = vmm.get_vm_mut().ok_or(VmmActionError::HostDeviceConfig(
            VfioDeviceError::InvalidVMID,
        ))?;

        info!("prepare_remove_vfio_device: {:?}", hostdev_id);
        let ctx = vm.create_device_op_context(None).map_err(|e| {
            info!("create device op context error: {:?}", e);
            if let StartMicroVmError::MicroVMAlreadyRunning = e {
                VmmActionError::HostDeviceConfig(VfioDeviceError::UpdateNotAllowedPostBoot)
            } else if let StartMicroVmError::UpcallServerNotReady = e {
                VmmActionError::UpcallServerNotReady
            } else {
                VmmActionError::StartMicroVm(e)
            }
        })?;

        let (sender, receiver) = unbounded();

        // It is safe because we don't expect poison lock.
        let vfio_manager = vm.device_manager.vfio_manager.lock().unwrap();

        vfio_manager
            .prepare_remove_device(&ctx, hostdev_id, sender.clone())
            .map(|_| VmmData::SyncHotplug((sender, receiver)))
            .map_err(VmmActionError::HostDeviceConfig)
    }

    #[cfg(feature = "host-device")]
    fn remove_vfio_device(&self, vmm: &mut Vmm, hostdev_id: &str) -> VmmRequestResult {
        let vm = vmm.get_vm_mut().ok_or(VmmActionError::HostDeviceConfig(
            VfioDeviceError::InvalidVMID,
        ))?;

        info!("remove_vfio_device: {:?}", hostdev_id);
        let mut ctx = vm.create_device_op_context(None).map_err(|e| {
            info!("create device op context error: {:?}", e);
            if let StartMicroVmError::MicroVMAlreadyRunning = e {
                VmmActionError::HostDeviceConfig(VfioDeviceError::UpdateNotAllowedPostBoot)
            } else if let StartMicroVmError::UpcallServerNotReady = e {
                VmmActionError::UpcallServerNotReady
            } else {
                VmmActionError::StartMicroVm(e)
            }
        })?;

        // It is safe because we don't expect poison lock.
        let mut vfio_manager = vm.device_manager.vfio_manager.lock().unwrap();

        vfio_manager
            .remove_device(&mut ctx, hostdev_id)
            .map_err(VmmActionError::HostDeviceConfig)?;

        // we need to revalidate io_manager cache in all vcpus
        // in order to drop old io_manager and close device's fd
        vm.vcpu_manager()
            .map_err(VmmActionError::RemoveHostDevice)?
            .revalidate_all_vcpus_cache()
            .map_err(VmmActionError::RemoveHostDevice)?;

        // FIXME: we should clear corresponding information because vfio module in
        // host kernel will clear iommu table in this scenario.

        Ok(VmmData::Empty)
    }

    #[cfg(feature = "hotplug")]
    #[instrument(skip(self))]
    fn resize_vcpu(&mut self, vmm: &mut Vmm, config: VcpuResizeInfo) -> VmmRequestResult {
        if !cfg!(feature = "dbs-upcall") {
            warn!("We only support cpu resize through upcall server in the guest kernel now, please enable dbs-upcall feature.");
            return Ok(VmmData::Empty);
        }

        let vm = vmm.get_vm_mut().ok_or(VmmActionError::VmNotExist)?;

        if !vm.is_vm_initialized() {
            return Err(VmmActionError::ResizeVcpu(
                VcpuResizeError::UpdateNotAllowedPreBoot,
            ));
        }

        #[cfg(feature = "dbs-upcall")]
        vm.resize_vcpu(config, None).map_err(|e| {
            if let VcpuResizeError::UpcallServerNotReady = e {
                return VmmActionError::UpcallServerNotReady;
            }
            VmmActionError::ResizeVcpu(e)
        })?;

        Ok(VmmData::Empty)
    }

    #[cfg(feature = "virtio-mem")]
    #[instrument(skip(self, event_mgr))]
    fn add_mem_device(
        &mut self,
        vmm: &mut Vmm,
        event_mgr: &mut EventManager,
        config: MemDeviceConfigInfo,
    ) -> VmmRequestResult {
        let vm = vmm.get_vm_mut().ok_or(VmmActionError::InvalidVMID)?;

        let ctx = vm
            .create_device_op_context(Some(event_mgr.epoll_manager()))
            .map_err(|e| {
                if let StartMicroVmError::UpcallServerNotReady = e {
                    VmmActionError::UpcallServerNotReady
                } else {
                    VmmActionError::StartMicroVm(e)
                }
            })?;

        vm.device_manager_mut()
            .mem_manager
            .insert_or_update_device(ctx, config)
            .map(|_| VmmData::Empty)
            .map_err(VmmActionError::Mem)
    }

    #[cfg(feature = "virtio-balloon")]
    #[instrument(skip(self, event_mgr))]
    fn add_balloon_device(
        &mut self,
        vmm: &mut Vmm,
        event_mgr: &mut EventManager,
        config: BalloonDeviceConfigInfo,
    ) -> VmmRequestResult {
        let vm = vmm.get_vm_mut().ok_or(VmmActionError::InvalidVMID)?;

        if config.size_mib != 0 {
            info!("add_balloon_device: wait prealloc");
            vm.stop_prealloc().map_err(VmmActionError::StartMicroVm)?;
        }
        let ctx = vm
            .create_device_op_context(Some(event_mgr.epoll_manager()))
            .map_err(|e| {
                if let StartMicroVmError::UpcallServerNotReady = e {
                    VmmActionError::UpcallServerNotReady
                } else {
                    VmmActionError::StartMicroVm(e)
                }
            })?;

        vm.device_manager_mut()
            .balloon_manager
            .insert_or_update_device(ctx, config)
            .map(|_| VmmData::Empty)
            .map_err(VmmActionError::Balloon)
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

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use crossbeam_channel::unbounded;
    use dbs_utils::epoll_manager::EpollManager;
    use test_utils::skip_if_not_root;
    use vmm_sys_util::tempfile::TempFile;

    use super::*;
    use crate::vmm::tests::create_vmm_instance;

    struct TestData<'a> {
        req: Option<VmmAction>,
        vm_state: InstanceState,
        f: &'a dyn Fn(VmmRequestResult),
    }

    impl<'a> TestData<'a> {
        fn new(req: VmmAction, vm_state: InstanceState, f: &'a dyn Fn(VmmRequestResult)) -> Self {
            Self {
                req: Some(req),
                vm_state,
                f,
            }
        }

        fn check_request(&mut self) {
            let (to_vmm, from_api) = unbounded();
            let (to_api, from_vmm) = unbounded();

            let epoll_mgr = EpollManager::default();
            let vmm = Arc::new(Mutex::new(create_vmm_instance(epoll_mgr.clone())));
            let mut vservice = VmmService::new(from_api, to_api);

            let mut event_mgr = EventManager::new(&vmm, epoll_mgr).unwrap();
            let mut v = vmm.lock().unwrap();

            let vm = v.get_vm_mut().unwrap();
            vm.set_instance_state(self.vm_state);

            to_vmm.send(Box::new(self.req.take().unwrap())).unwrap();
            assert!(vservice.run_vmm_action(&mut v, &mut event_mgr).is_ok());

            let response = from_vmm.try_recv();
            assert!(response.is_ok());
            (self.f)(*response.unwrap());
        }
    }

    #[test]
    fn test_vmm_action_receive_unknown() {
        skip_if_not_root!();

        let (_to_vmm, from_api) = unbounded();
        let (to_api, _from_vmm) = unbounded();
        let epoll_mgr = EpollManager::default();
        let vmm = Arc::new(Mutex::new(create_vmm_instance(epoll_mgr.clone())));
        let mut vservice = VmmService::new(from_api, to_api);
        let mut event_mgr = EventManager::new(&vmm, epoll_mgr).unwrap();
        let mut v = vmm.lock().unwrap();

        assert!(vservice.run_vmm_action(&mut v, &mut event_mgr).is_ok());
    }

    #[should_panic]
    #[test]
    fn test_vmm_action_disconnected() {
        let (to_vmm, from_api) = unbounded();
        let (to_api, _from_vmm) = unbounded();
        let epoll_mgr = EpollManager::default();
        let vmm = Arc::new(Mutex::new(create_vmm_instance(epoll_mgr.clone())));
        let mut vservice = VmmService::new(from_api, to_api);
        let mut event_mgr = EventManager::new(&vmm, epoll_mgr).unwrap();
        let mut v = vmm.lock().unwrap();

        drop(to_vmm);
        vservice.run_vmm_action(&mut v, &mut event_mgr).unwrap();
    }

    #[test]
    fn test_vmm_action_config_boot_source() {
        skip_if_not_root!();

        let kernel_file = TempFile::new().unwrap();

        let tests = &mut [
            // invalid state
            TestData::new(
                VmmAction::ConfigureBootSource(BootSourceConfig::default()),
                InstanceState::Running,
                &|result| {
                    if let Err(VmmActionError::BootSource(
                        BootSourceConfigError::UpdateNotAllowedPostBoot,
                    )) = result
                    {
                        let err_string = format!("{}", result.unwrap_err());
                        let expected_err = String::from(
                            "failed to configure boot source for VM: \
                    the update operation is not allowed after boot",
                        );
                        assert_eq!(err_string, expected_err);
                    } else {
                        panic!();
                    }
                },
            ),
            // invalid kernel file path
            TestData::new(
                VmmAction::ConfigureBootSource(BootSourceConfig::default()),
                InstanceState::Uninitialized,
                &|result| {
                    if let Err(VmmActionError::BootSource(
                        BootSourceConfigError::InvalidKernelPath(_),
                    )) = result
                    {
                        let err_string = format!("{}", result.unwrap_err());
                        let expected_err = String::from(
                    "failed to configure boot source for VM: \
                    the kernel file cannot be opened due to invalid kernel path or invalid permissions: \
                    No such file or directory (os error 2)");
                        assert_eq!(err_string, expected_err);
                    } else {
                        panic!();
                    }
                },
            ),
            //success
            TestData::new(
                VmmAction::ConfigureBootSource(BootSourceConfig {
                    kernel_path: kernel_file.as_path().to_str().unwrap().to_string(),
                    ..Default::default()
                }),
                InstanceState::Uninitialized,
                &|result| {
                    assert!(result.is_ok());
                },
            ),
        ];

        for t in tests.iter_mut() {
            t.check_request();
        }
    }

    #[test]
    fn test_vmm_action_set_vm_configuration() {
        skip_if_not_root!();

        let tests = &mut [
            // invalid state
            TestData::new(
                VmmAction::SetVmConfiguration(VmConfigInfo::default()),
                InstanceState::Running,
                &|result| {
                    assert!(matches!(
                        result,
                        Err(VmmActionError::MachineConfig(
                            VmConfigError::UpdateNotAllowedPostBoot
                        ))
                    ));
                    let err_string = format!("{}", result.unwrap_err());
                    let expected_err = String::from(
                        "failed to set configuration for the VM: \
                    update operation is not allowed after boot",
                    );
                    assert_eq!(err_string, expected_err);
                },
            ),
            // invalid cpu count (0)
            TestData::new(
                VmmAction::SetVmConfiguration(VmConfigInfo {
                    vcpu_count: 0,
                    ..Default::default()
                }),
                InstanceState::Uninitialized,
                &|result| {
                    assert!(matches!(
                        result,
                        Err(VmmActionError::MachineConfig(
                            VmConfigError::InvalidVcpuCount(0)
                        ))
                    ));
                    let err_string = format!("{}", result.unwrap_err());
                    let expected_err = String::from(
                    "failed to set configuration for the VM: \
                    the vCPU number '0' can only be 1 or an even number when hyperthreading is enabled");
                    assert_eq!(err_string, expected_err);
                },
            ),
            // invalid max cpu count (too small)
            TestData::new(
                VmmAction::SetVmConfiguration(VmConfigInfo {
                    vcpu_count: 4,
                    max_vcpu_count: 2,
                    ..Default::default()
                }),
                InstanceState::Uninitialized,
                &|result| {
                    assert!(matches!(
                        result,
                        Err(VmmActionError::MachineConfig(
                            VmConfigError::InvalidMaxVcpuCount(2)
                        ))
                    ));
                    let err_string = format!("{}", result.unwrap_err());
                    let expected_err = String::from(
                    "failed to set configuration for the VM: \
                    the max vCPU number '2' shouldn't less than vCPU count and can only be 1 or an even number when hyperthreading is enabled");
                    assert_eq!(err_string, expected_err);
                },
            ),
            // invalid cpu topology (larger than 254)
            TestData::new(
                VmmAction::SetVmConfiguration(VmConfigInfo {
                    vcpu_count: 254,
                    cpu_topology: CpuTopology {
                        threads_per_core: 2,
                        cores_per_die: 128,
                        dies_per_socket: 1,
                        sockets: 1,
                    },
                    ..Default::default()
                }),
                InstanceState::Uninitialized,
                &|result| {
                    assert!(matches!(
                        result,
                        Err(VmmActionError::MachineConfig(
                            VmConfigError::VcpuCountExceedsMaximum
                        ))
                    ));
                    let err_string = format!("{}", result.unwrap_err());
                    let expected_err = String::from(
                        "failed to set configuration for the VM: \
                the vCPU number shouldn't large than 254",
                    );

                    assert_eq!(err_string, expected_err)
                },
            ),
            // cpu topology and max_vcpu_count are not matched - success
            TestData::new(
                VmmAction::SetVmConfiguration(VmConfigInfo {
                    vcpu_count: 16,
                    max_vcpu_count: 32,
                    cpu_topology: CpuTopology {
                        threads_per_core: 1,
                        cores_per_die: 128,
                        dies_per_socket: 1,
                        sockets: 1,
                    },
                    ..Default::default()
                }),
                InstanceState::Uninitialized,
                &|result| {
                    result.unwrap();
                },
            ),
            // invalid threads_per_core
            TestData::new(
                VmmAction::SetVmConfiguration(VmConfigInfo {
                    vcpu_count: 4,
                    max_vcpu_count: 4,
                    cpu_topology: CpuTopology {
                        threads_per_core: 4,
                        cores_per_die: 1,
                        dies_per_socket: 1,
                        sockets: 1,
                    },
                    ..Default::default()
                }),
                InstanceState::Uninitialized,
                &|result| {
                    assert!(matches!(
                        result,
                        Err(VmmActionError::MachineConfig(
                            VmConfigError::InvalidThreadsPerCore(4)
                        ))
                    ));
                    let err_string = format!("{}", result.unwrap_err());
                    let expected_err = String::from(
                        "failed to set configuration for the VM: \
                    the threads_per_core number '4' can only be 1 or 2",
                    );

                    assert_eq!(err_string, expected_err)
                },
            ),
            // invalid mem size
            TestData::new(
                VmmAction::SetVmConfiguration(VmConfigInfo {
                    mem_size_mib: 3,
                    ..Default::default()
                }),
                InstanceState::Uninitialized,
                &|result| {
                    assert!(matches!(
                        result,
                        Err(VmmActionError::MachineConfig(
                            VmConfigError::InvalidMemorySize(3)
                        ))
                    ));
                    let err_string = format!("{}", result.unwrap_err());
                    let expected_err = String::from(
                        "failed to set configuration for the VM: \
                    the memory size 0x3MiB is invalid",
                    );
                    assert_eq!(err_string, expected_err);
                },
            ),
            // invalid mem path
            TestData::new(
                VmmAction::SetVmConfiguration(VmConfigInfo {
                    mem_type: String::from("hugetlbfs"),
                    mem_file_path: String::from(""),
                    ..Default::default()
                }),
                InstanceState::Uninitialized,
                &|result| {
                    assert!(matches!(
                        result,
                        Err(VmmActionError::MachineConfig(
                            VmConfigError::InvalidMemFilePath(_)
                        ))
                    ));
                    let err_string = format!("{}", result.unwrap_err());
                    let expected_err = String::from(
                        "failed to set configuration for the VM: \
                    the memory file path is invalid",
                    );
                    assert_eq!(err_string, expected_err);
                },
            ),
            // success
            TestData::new(
                VmmAction::SetVmConfiguration(VmConfigInfo::default()),
                InstanceState::Uninitialized,
                &|result| {
                    assert!(result.is_ok());
                },
            ),
        ];

        for t in tests.iter_mut() {
            t.check_request();
        }
    }

    #[test]
    fn test_vmm_action_start_microvm() {
        skip_if_not_root!();

        let tests = &mut [
            // invalid state (running)
            TestData::new(VmmAction::StartMicroVm, InstanceState::Running, &|result| {
                assert!(matches!(
                    result,
                    Err(VmmActionError::StartMicroVm(
                        StartMicroVmError::MicroVMAlreadyRunning
                    ))
                ));
                let err_string = format!("{}", result.unwrap_err());
                let expected_err = String::from(
                    "failed to boot the VM: \
                    the virtual machine is already running",
                );
                assert_eq!(err_string, expected_err);
            }),
            // no kernel configuration
            TestData::new(
                VmmAction::StartMicroVm,
                InstanceState::Uninitialized,
                &|result| {
                    assert!(matches!(
                        result,
                        Err(VmmActionError::StartMicroVm(
                            StartMicroVmError::MissingKernelConfig
                        ))
                    ));
                    let err_string = format!("{}", result.unwrap_err());
                    let expected_err = String::from(
                        "failed to boot the VM: \
                    cannot start the virtual machine without kernel configuration",
                    );
                    assert_eq!(err_string, expected_err);
                },
            ),
        ];

        for t in tests.iter_mut() {
            t.check_request();
        }
    }

    #[test]
    fn test_vmm_action_shutdown_microvm() {
        skip_if_not_root!();

        let tests = &mut [
            // success
            TestData::new(
                VmmAction::ShutdownMicroVm,
                InstanceState::Uninitialized,
                &|result| {
                    assert!(result.is_ok());
                },
            ),
        ];

        for t in tests.iter_mut() {
            t.check_request();
        }
    }

    #[cfg(any(feature = "virtio-blk", feature = "vhost-user-blk"))]
    #[test]
    fn test_vmm_action_insert_block_device() {
        skip_if_not_root!();

        let dummy_file = TempFile::new().unwrap();
        let dummy_path = dummy_file.as_path().to_owned();

        let tests = &mut [
            // invalid state
            TestData::new(
                VmmAction::InsertBlockDevice(BlockDeviceConfigInfo::default()),
                InstanceState::Running,
                &|result| {
                    assert!(matches!(
                        result,
                        Err(VmmActionError::Block(
                            BlockDeviceError::UpdateNotAllowedPostBoot
                        ))
                    ));
                    let err_string = format!("{}", result.unwrap_err());
                    let expected_err = String::from(
                        "virtio-blk device error: \
                    block device does not support runtime update",
                    );
                    assert_eq!(err_string, expected_err);
                },
            ),
            // success
            TestData::new(
                VmmAction::InsertBlockDevice(BlockDeviceConfigInfo {
                    path_on_host: dummy_path,
                    device_type: crate::device_manager::blk_dev_mgr::BlockDeviceType::RawBlock,
                    is_root_device: true,
                    part_uuid: None,
                    is_read_only: false,
                    is_direct: false,
                    no_drop: false,
                    drive_id: String::from("1"),
                    rate_limiter: None,
                    num_queues: BlockDeviceConfigInfo::default_num_queues(),
                    queue_size: 256,
                    use_shared_irq: None,
                    use_generic_irq: None,
                }),
                InstanceState::Uninitialized,
                &|result| {
                    assert!(result.is_ok());
                },
            ),
        ];

        for t in tests.iter_mut() {
            t.check_request();
        }
    }

    #[cfg(any(feature = "virtio-blk", feature = "vhost-user-blk"))]
    #[test]
    fn test_vmm_action_update_block_device() {
        skip_if_not_root!();

        let tests = &mut [
            // invalid id
            TestData::new(
                VmmAction::UpdateBlockDevice(BlockDeviceConfigUpdateInfo {
                    drive_id: String::from("1"),
                    rate_limiter: None,
                }),
                InstanceState::Running,
                &|result| {
                    assert!(matches!(
                        result,
                        Err(VmmActionError::Block(BlockDeviceError::InvalidDeviceId(_)))
                    ));
                    let err_string = format!("{}", result.unwrap_err());
                    let expected_err = String::from(
                        "virtio-blk device error: \
                    invalid block device id '1'",
                    );
                    assert_eq!(err_string, expected_err);
                },
            ),
        ];

        for t in tests.iter_mut() {
            t.check_request();
        }
    }

    #[cfg(any(feature = "virtio-blk", feature = "vhost-user-blk"))]
    #[test]
    fn test_vmm_action_remove_block_device() {
        skip_if_not_root!();

        let tests = &mut [
            // invalid state
            TestData::new(
                VmmAction::RemoveBlockDevice(String::from("1")),
                InstanceState::Running,
                &|result| {
                    assert!(matches!(
                        result,
                        Err(VmmActionError::Block(
                            BlockDeviceError::UpdateNotAllowedPostBoot
                        ))
                    ));
                    let err_string = format!("{}", result.unwrap_err());
                    let expected_err = String::from(
                        "virtio-blk device error: \
                    block device does not support runtime update",
                    );
                    assert_eq!(err_string, expected_err);
                },
            ),
            // invalid id
            TestData::new(
                VmmAction::RemoveBlockDevice(String::from("1")),
                InstanceState::Uninitialized,
                &|result| {
                    assert!(matches!(
                        result,
                        Err(VmmActionError::Block(BlockDeviceError::InvalidDeviceId(_)))
                    ));
                    let err_string = format!("{}", result.unwrap_err());
                    let expected_err = String::from(
                        "virtio-blk device error: \
                    invalid block device id '1'",
                    );
                    assert_eq!(err_string, expected_err);
                },
            ),
        ];

        for t in tests.iter_mut() {
            t.check_request();
        }
    }

    #[cfg(feature = "virtio-fs")]
    #[test]
    fn test_vmm_action_insert_fs_device() {
        skip_if_not_root!();

        let tests = &mut [
            // invalid state
            TestData::new(
                VmmAction::InsertFsDevice(FsDeviceConfigInfo::default()),
                InstanceState::Running,
                &|result| {
                    assert!(matches!(
                        result,
                        Err(VmmActionError::FsDevice(
                            FsDeviceError::UpdateNotAllowedPostBoot
                        ))
                    ));
                    let err_string = format!("{}", result.unwrap_err());
                    let expected_err = String::from(
                        "virtio-fs device error: \
                    update operation is not allowed after boot",
                    );
                    assert_eq!(err_string, expected_err);
                },
            ),
            // success
            TestData::new(
                VmmAction::InsertFsDevice(FsDeviceConfigInfo::default()),
                InstanceState::Uninitialized,
                &|result| {
                    assert!(result.is_ok());
                },
            ),
        ];

        for t in tests.iter_mut() {
            t.check_request();
        }
    }

    #[cfg(feature = "virtio-fs")]
    #[test]
    fn test_vmm_action_manipulate_fs_device() {
        skip_if_not_root!();

        let tests = &mut [
            // invalid state
            TestData::new(
                VmmAction::ManipulateFsBackendFs(FsMountConfigInfo::default()),
                InstanceState::Uninitialized,
                &|result| {
                    assert!(matches!(
                        result,
                        Err(VmmActionError::FsDevice(FsDeviceError::MicroVMNotRunning))
                    ));
                    let err_string = format!("{}", result.unwrap_err());
                    let expected_err = String::from(
                        "virtio-fs device error: \
                    vm is not running when attaching a backend fs",
                    );
                    assert_eq!(err_string, expected_err);
                },
            ),
            // invalid backend
            TestData::new(
                VmmAction::ManipulateFsBackendFs(FsMountConfigInfo::default()),
                InstanceState::Running,
                &|result| {
                    assert!(matches!(
                        result,
                        Err(VmmActionError::FsDevice(
                            FsDeviceError::AttachBackendFailed(_)
                        ))
                    ));
                    let err_string = format!("{}", result.unwrap_err());
                    println!("{}", err_string);
                    let expected_err = String::from(
                        "virtio-fs device error: \
                    Fs device attach a backend fs failed",
                    );
                    assert_eq!(err_string, expected_err);
                },
            ),
        ];
        for t in tests.iter_mut() {
            t.check_request();
        }
    }

    #[cfg(feature = "virtio-net")]
    #[test]
    fn test_vmm_action_insert_network_device() {
        skip_if_not_root!();

        let tests = &mut [
            // hotplug unready
            TestData::new(
                VmmAction::InsertNetworkDevice(NetworkInterfaceConfig::default()),
                InstanceState::Running,
                &|result| {
                    assert!(matches!(
                        result,
                        Err(VmmActionError::StartMicroVm(
                            StartMicroVmError::UpcallMissVsock
                        ))
                    ));
                    let err_string = format!("{}", result.unwrap_err());
                    let expected_err = String::from(
                        "failed to boot the VM: \
                        the upcall client needs a virtio-vsock device for communication",
                    );
                    assert_eq!(err_string, expected_err);
                },
            ),
            // success
            TestData::new(
                VmmAction::InsertNetworkDevice(NetworkInterfaceConfig::default()),
                InstanceState::Uninitialized,
                &|result| {
                    assert!(result.is_ok());
                },
            ),
        ];

        for t in tests.iter_mut() {
            t.check_request();
        }
    }

    #[cfg(feature = "virtio-net")]
    #[test]
    fn test_vmm_action_update_network_interface() {
        skip_if_not_root!();

        let tests = &mut [
            // invalid id
            TestData::new(
                VmmAction::UpdateNetworkInterface(VirtioNetDeviceConfigUpdateInfo {
                    iface_id: String::from("1"),
                    rx_rate_limiter: None,
                    tx_rate_limiter: None,
                }),
                InstanceState::Running,
                &|result| {
                    assert!(matches!(
                        result,
                        Err(VmmActionError::VirtioNet(
                            VirtioNetDeviceError::InvalidIfaceId(_)
                        ))
                    ));
                    let err_string = format!("{}", result.unwrap_err());
                    let expected_err = String::from(
                        "virtio-net device error: \
                    invalid virtio-net iface id '1'",
                    );
                    assert_eq!(err_string, expected_err);
                },
            ),
        ];

        for t in tests.iter_mut() {
            t.check_request();
        }
    }

    #[cfg(feature = "virtio-vsock")]
    #[test]
    fn test_vmm_action_insert_vsock_device() {
        skip_if_not_root!();

        let tests = &mut [
            // invalid state
            TestData::new(
                VmmAction::InsertVsockDevice(VsockDeviceConfigInfo::default()),
                InstanceState::Running,
                &|result| {
                    assert!(matches!(
                        result,
                        Err(VmmActionError::Vsock(
                            VsockDeviceError::UpdateNotAllowedPostBoot
                        ))
                    ));
                    let err_string = format!("{}", result.unwrap_err());
                    let expected_err = String::from(
                        "failed to add virtio-vsock device: \
                    update operation is not allowed after boot",
                    );
                    assert_eq!(err_string, expected_err);
                },
            ),
            // invalid guest_cid
            TestData::new(
                VmmAction::InsertVsockDevice(VsockDeviceConfigInfo::default()),
                InstanceState::Uninitialized,
                &|result| {
                    assert!(matches!(
                        result,
                        Err(VmmActionError::Vsock(VsockDeviceError::GuestCIDInvalid(0)))
                    ));
                    let err_string = format!("{}", result.unwrap_err());
                    let expected_err = String::from(
                        "failed to add virtio-vsock device: \
                    the guest CID 0 is invalid",
                    );
                    assert_eq!(err_string, expected_err);
                },
            ),
            // success
            TestData::new(
                VmmAction::InsertVsockDevice(VsockDeviceConfigInfo {
                    guest_cid: 3,
                    ..Default::default()
                }),
                InstanceState::Uninitialized,
                &|result| {
                    assert!(result.is_ok());
                },
            ),
        ];

        for t in tests.iter_mut() {
            t.check_request();
        }
    }

    #[cfg(feature = "virtio-mem")]
    #[test]
    fn test_vmm_action_insert_mem_device() {
        skip_if_not_root!();

        let tests = &mut [
            // hotplug unready
            TestData::new(
                VmmAction::InsertMemDevice(MemDeviceConfigInfo::default()),
                InstanceState::Running,
                &|result| {
                    assert!(matches!(
                        result,
                        Err(VmmActionError::StartMicroVm(
                            StartMicroVmError::UpcallMissVsock
                        ))
                    ));
                    let err_string = format!("{}", result.unwrap_err());
                    let expected_err = String::from(
                        "failed to boot the VM: \
                        the upcall client needs a virtio-vsock device for communication",
                    );
                    assert_eq!(err_string, expected_err);
                },
            ),
            // success
            TestData::new(
                VmmAction::InsertMemDevice(MemDeviceConfigInfo::default()),
                InstanceState::Uninitialized,
                &|result| {
                    assert!(result.is_ok());
                },
            ),
        ];

        for t in tests.iter_mut() {
            t.check_request();
        }
    }

    #[cfg(feature = "virtio-balloon")]
    #[test]
    fn test_vmm_action_insert_balloon_device() {
        skip_if_not_root!();

        let tests = &mut [
            // hotplug unready
            TestData::new(
                VmmAction::InsertBalloonDevice(BalloonDeviceConfigInfo::default()),
                InstanceState::Running,
                &|result| {
                    assert!(matches!(
                        result,
                        Err(VmmActionError::StartMicroVm(
                            StartMicroVmError::UpcallMissVsock
                        ))
                    ));
                    let err_string = format!("{}", result.unwrap_err());
                    let expected_err = String::from(
                        "failed to boot the VM: \
                        the upcall client needs a virtio-vsock device for communication",
                    );
                    assert_eq!(err_string, expected_err);
                },
            ),
            // success
            TestData::new(
                VmmAction::InsertBalloonDevice(BalloonDeviceConfigInfo::default()),
                InstanceState::Uninitialized,
                &|result| {
                    assert!(result.is_ok());
                },
            ),
        ];

        for t in tests.iter_mut() {
            t.check_request();
        }
    }
}
