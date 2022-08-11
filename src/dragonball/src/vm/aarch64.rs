// Copyright (C) 2022 Alibaba Cloud. All rights reserved.
// Copyright 2018 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0
//
// Portions Copyright 2017 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the THIRD-PARTY file.

use std::collections::HashMap;
use std::fmt::Debug;
use std::ops::Deref;

use dbs_arch::gic::GICDevice;
use dbs_arch::{DeviceInfoForFDT, DeviceType};
use dbs_boot::InitrdConfig;
use dbs_utils::epoll_manager::EpollManager;
use dbs_utils::time::TimestampUs;
use linux_loader::loader::Cmdline;
use vm_memory::{GuestAddressSpace, GuestMemory};
use vmm_sys_util::eventfd::EventFd;

use super::{Vm, VmError};
use crate::address_space_manager::{GuestAddressSpaceImpl, GuestMemoryImpl};
use crate::error::{Error, StartMicroVmError};
use crate::event_manager::EventManager;

/// Configures the system and should be called once per vm before starting vcpu threads.
/// For aarch64, we only setup the FDT.
///
/// # Arguments
///
/// * `guest_mem` - The memory to be used by the guest.
/// * `cmdline` - The kernel commandline.
/// * `vcpu_mpidr` - Array of MPIDR register values per vcpu.
/// * `device_info` - A hashmap containing the attached devices for building FDT device nodes.
/// * `gic_device` - The GIC device.
/// * `initrd` - Information about an optional initrd.
fn configure_system<T: DeviceInfoForFDT + Clone + Debug, M: GuestMemory>(
    guest_mem: &M,
    cmdline: &str,
    vcpu_mpidr: Vec<u64>,
    device_info: Option<&HashMap<(DeviceType, String), T>>,
    gic_device: &Box<dyn GICDevice>,
    initrd: &Option<super::InitrdConfig>,
) -> super::Result<()> {
    dbs_boot::fdt::create_fdt(
        guest_mem,
        vcpu_mpidr,
        cmdline,
        device_info,
        gic_device,
        initrd,
    )
    .map_err(Error::BootSystem)?;
    Ok(())
}

#[cfg(target_arch = "aarch64")]
impl Vm {
    /// Gets a reference to the irqchip of the VM
    pub fn get_irqchip(&self) -> &Box<dyn GICDevice> {
        &self.irqchip_handle.as_ref().unwrap()
    }

    /// Creates the irq chip in-kernel device model.
    pub fn setup_interrupt_controller(&mut self) -> std::result::Result<(), StartMicroVmError> {
        let vcpu_count = self.vm_config.vcpu_count;

        self.irqchip_handle = Some(
            dbs_arch::gic::create_gic(&self.vm_fd, vcpu_count.into())
                .map_err(|e| StartMicroVmError::ConfigureVm(VmError::SetupGIC(e)))?,
        );

        Ok(())
    }

    /// Initialize the virtual machine instance.
    ///
    /// It initialize the virtual machine instance by:
    /// 1) initialize virtual machine global state and configuration.
    /// 2) create system devices, such as interrupt controller.
    /// 3) create and start IO devices, such as serial, console, block, net, vsock etc.
    /// 4) create and initialize vCPUs.
    /// 5) configure CPU power management features.
    /// 6) load guest kernel image.
    pub fn init_microvm(
        &mut self,
        epoll_mgr: EpollManager,
        vm_as: GuestAddressSpaceImpl,
        request_ts: TimestampUs,
    ) -> Result<(), StartMicroVmError> {
        let reset_eventfd =
            EventFd::new(libc::EFD_NONBLOCK).map_err(|_| StartMicroVmError::EventFd)?;
        self.reset_eventfd = Some(
            reset_eventfd
                .try_clone()
                .map_err(|_| StartMicroVmError::EventFd)?,
        );
        self.vcpu_manager()
            .map_err(StartMicroVmError::Vcpu)?
            .set_reset_event_fd(reset_eventfd)
            .map_err(StartMicroVmError::Vcpu)?;

        // On aarch64, the vCPUs need to be created (i.e call KVM_CREATE_VCPU) and configured before
        // setting up the IRQ chip because the `KVM_CREATE_VCPU` ioctl will return error if the IRQCHIP
        // was already initialized.
        // Search for `kvm_arch_vcpu_create` in arch/arm/kvm/arm.c.
        let kernel_loader_result = self.load_kernel(vm_as.memory().deref())?;
        self.vcpu_manager()
            .map_err(StartMicroVmError::Vcpu)?
            .create_boot_vcpus(request_ts, kernel_loader_result.kernel_load)
            .map_err(StartMicroVmError::Vcpu)?;
        self.setup_interrupt_controller()?;
        self.init_devices(epoll_mgr)?;

        Ok(())
    }

    /// Execute system architecture specific configurations.
    ///
    /// 1) set guest kernel boot parameters
    /// 2) setup FDT data structs.
    pub fn configure_system_arch(
        &self,
        vm_memory: &GuestMemoryImpl,
        cmdline: &Cmdline,
        initrd: Option<InitrdConfig>,
    ) -> std::result::Result<(), StartMicroVmError> {
        let vcpu_manager = self.vcpu_manager().map_err(StartMicroVmError::Vcpu)?;
        let vcpu_mpidr = vcpu_manager
            .vcpus()
            .into_iter()
            .map(|cpu| cpu.get_mpidr())
            .collect();
        let guest_memory = vm_memory.memory();

        configure_system(
            guest_memory,
            cmdline.as_str(),
            vcpu_mpidr,
            self.device_manager.get_mmio_device_info(),
            self.get_irqchip(),
            &initrd,
        )
        .map_err(StartMicroVmError::ConfigureSystem)
    }

    pub(crate) fn register_events(
        &mut self,
        event_mgr: &mut EventManager,
    ) -> std::result::Result<(), StartMicroVmError> {
        let reset_evt = self.get_reset_eventfd().ok_or(StartMicroVmError::EventFd)?;
        event_mgr
            .register_exit_eventfd(reset_evt)
            .map_err(|_| StartMicroVmError::RegisterEvent)?;

        Ok(())
    }
}
