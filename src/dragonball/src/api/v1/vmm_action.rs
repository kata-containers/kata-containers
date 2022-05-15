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
use vmm_sys_util::eventfd::EventFd;

use crate::error::{Result, StartMicrovmError, StopMicrovmError};
use crate::event_manager::EventManager;
use crate::vm::{CpuTopology, KernelConfigInfo, VmConfigInfo};
use crate::vmm::Vmm;

use super::*;

/// Wrapper for all errors associated with VMM actions.
#[derive(Debug, thiserror::Error)]
pub enum VmmActionError {
    /// Invalid virtual machine instance ID.
    #[error("the virtual machine instance ID is invalid")]
    InvalidVMID,

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

        let vm = vmm
            .get_vm_by_id_mut("")
            .ok_or(VmmActionError::InvalidVMID)?;
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
        use self::StartMicrovmError::MicroVMAlreadyRunning;
        use self::VmmActionError::StartMicrovm;

        let vmm_seccomp_filter = vmm.vmm_seccomp_filter();
        let vcpu_seccomp_filter = vmm.vcpu_seccomp_filter();
        let vm = vmm
            .get_vm_by_id_mut("")
            .ok_or(VmmActionError::InvalidVMID)?;
        if vm.is_vm_initialized() {
            return Err(StartMicrovm(MicroVMAlreadyRunning));
        }

        vm.start_microvm(event_mgr, vmm_seccomp_filter, vcpu_seccomp_filter)
            .map(|_| VmmData::Empty)
            .map_err(StartMicrovm)
    }

    fn shutdown_microvm(&mut self, vmm: &mut Vmm) -> VmmRequestResult {
        vmm.event_ctx.exit_evt_triggered = true;

        Ok(VmmData::Empty)
    }

    /// Set virtual machine configuration configurations.
    pub fn set_vm_configuration(
        &mut self,
        vmm: &mut Vmm,
        machine_config: VmConfigInfo,
    ) -> VmmRequestResult {
        use self::VmConfigError::*;
        use self::VmmActionError::MachineConfig;

        let vm = vmm
            .get_vm_by_id_mut("")
            .ok_or(VmmActionError::InvalidVMID)?;
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
            if vcpu_count_from_topo < config.vcpu_count {
                return Err(MachineConfig(InvalidCpuTopology(vcpu_count_from_topo)));
            }
            config.cpu_topology = cpu_topology.clone();
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

        let reserve_memory_bytes = machine_config.reserve_memory_bytes;
        // Reserved memory must be 2MB aligned and less than half of the total memory.
        if reserve_memory_bytes % 0x200000 != 0
            || reserve_memory_bytes > (config.mem_size_mib as u64) << 20
        {
            return Err(MachineConfig(InvalidReservedMemorySize(
                reserve_memory_bytes as usize >> 20,
            )));
        }
        config.reserve_memory_bytes = reserve_memory_bytes;
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
}
