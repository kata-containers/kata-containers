// Copyright (C) 2020-2022 Alibaba Cloud. All rights reserved.
// Copyright 2018 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0
//
// Portions Copyright 2017 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the THIRD-PARTY file.

use std::collections::HashMap;
use std::convert::TryInto;
use std::ops::Deref;

use dbs_address_space::AddressSpace;
use dbs_boot::{add_e820_entry, bootparam, layout, mptable, BootParamsWrapper, InitrdConfig};
use dbs_utils::epoll_manager::EpollManager;
use dbs_utils::time::TimestampUs;
use kvm_bindings::{kvm_irqchip, kvm_pit_config, kvm_pit_state2, KVM_PIT_SPEAKER_DUMMY};
use linux_loader::cmdline::Cmdline;
use linux_loader::configurator::{linux::LinuxBootConfigurator, BootConfigurator, BootParams};
use slog::info;
use vm_memory::{Address, GuestAddress, GuestAddressSpace, GuestMemory};

use crate::address_space_manager::{GuestAddressSpaceImpl, GuestMemoryImpl};
use crate::error::{Error, Result, StartMicroVmError};
use crate::event_manager::EventManager;
use crate::vm::{Vm, VmError};

/// Configures the system and should be called once per vm before starting vcpu
/// threads.
///
/// # Arguments
///
/// * `guest_mem` - The memory to be used by the guest.
/// * `cmdline_addr` - Address in `guest_mem` where the kernel command line was
///   loaded.
/// * `cmdline_size` - Size of the kernel command line in bytes including the
///   null terminator.
/// * `initrd` - Information about where the ramdisk image was loaded in the
///   `guest_mem`.
/// * `boot_cpus` - Number of virtual CPUs the guest will have at boot time.
/// * `max_cpus` - Max number of virtual CPUs the guest will have.
/// * `rsv_mem_bytes` - Reserve memory from microVM..
#[allow(clippy::too_many_arguments)]
fn configure_system<M: GuestMemory>(
    guest_mem: &M,
    address_space: Option<&AddressSpace>,
    cmdline_addr: GuestAddress,
    cmdline_size: usize,
    initrd: &Option<InitrdConfig>,
    boot_cpus: u8,
    max_cpus: u8,
    pci_legacy_irqs: Option<&HashMap<u8, u8>>,
) -> super::Result<()> {
    const KERNEL_BOOT_FLAG_MAGIC: u16 = 0xaa55;
    const KERNEL_HDR_MAGIC: u32 = 0x5372_6448;
    const KERNEL_LOADER_OTHER: u8 = 0xff;
    const KERNEL_MIN_ALIGNMENT_BYTES: u32 = 0x0100_0000; // Must be non-zero.

    let mmio_start = GuestAddress(layout::MMIO_LOW_START);
    let mmio_end = GuestAddress(layout::MMIO_LOW_END);
    let himem_start = GuestAddress(layout::HIMEM_START);

    // Note that this puts the mptable at the last 1k of Linux's 640k base RAM
    mptable::setup_mptable(guest_mem, boot_cpus, max_cpus, pci_legacy_irqs)
        .map_err(Error::MpTableSetup)?;

    let mut params: BootParamsWrapper = BootParamsWrapper(bootparam::boot_params::default());

    params.0.hdr.type_of_loader = KERNEL_LOADER_OTHER;
    params.0.hdr.boot_flag = KERNEL_BOOT_FLAG_MAGIC;
    params.0.hdr.header = KERNEL_HDR_MAGIC;
    params.0.hdr.cmd_line_ptr = cmdline_addr.raw_value() as u32;
    params.0.hdr.cmdline_size = cmdline_size as u32;
    params.0.hdr.kernel_alignment = KERNEL_MIN_ALIGNMENT_BYTES;
    if let Some(initrd_config) = initrd {
        params.0.hdr.ramdisk_image = initrd_config.address.raw_value() as u32;
        params.0.hdr.ramdisk_size = initrd_config.size as u32;
    }

    add_e820_entry(&mut params.0, 0, layout::EBDA_START, bootparam::E820_RAM)
        .map_err(Error::BootSystem)?;

    let mem_end = address_space.ok_or(Error::AddressSpace)?.last_addr();
    if mem_end < mmio_start {
        add_e820_entry(
            &mut params.0,
            himem_start.raw_value(),
            // it's safe to use unchecked_offset_from because
            // mem_end > himem_start
            mem_end.unchecked_offset_from(himem_start) + 1,
            bootparam::E820_RAM,
        )
        .map_err(Error::BootSystem)?;
    } else {
        add_e820_entry(
            &mut params.0,
            himem_start.raw_value(),
            // it's safe to use unchecked_offset_from because
            // end_32bit_gap_start > himem_start
            mmio_start.unchecked_offset_from(himem_start),
            bootparam::E820_RAM,
        )
        .map_err(Error::BootSystem)?;
        if mem_end > mmio_end {
            add_e820_entry(
                &mut params.0,
                mmio_end.raw_value() + 1,
                // it's safe to use unchecked_offset_from because mem_end > mmio_end
                mem_end.unchecked_offset_from(mmio_end),
                bootparam::E820_RAM,
            )
            .map_err(Error::BootSystem)?;
        }
    }

    LinuxBootConfigurator::write_bootparams(
        &BootParams::new(&params, GuestAddress(layout::ZERO_PAGE_START)),
        guest_mem,
    )
    .map_err(|_| Error::ZeroPageSetup)
}

impl Vm {
    /// Get the status of in-kernel PIT.
    pub fn get_pit_state(&self) -> Result<kvm_pit_state2> {
        self.vm_fd
            .get_pit2()
            .map_err(|e| Error::Vm(VmError::Irq(e)))
    }

    /// Set the status of in-kernel PIT.
    pub fn set_pit_state(&self, pit_state: &kvm_pit_state2) -> Result<()> {
        self.vm_fd
            .set_pit2(pit_state)
            .map_err(|e| Error::Vm(VmError::Irq(e)))
    }

    /// Get the status of in-kernel ioapic.
    pub fn get_irqchip_state(&self, chip_id: u32) -> Result<kvm_irqchip> {
        let mut irqchip: kvm_irqchip = kvm_irqchip {
            chip_id,
            ..kvm_irqchip::default()
        };
        self.vm_fd
            .get_irqchip(&mut irqchip)
            .map(|_| irqchip)
            .map_err(|e| Error::Vm(VmError::Irq(e)))
    }

    /// Set the status of in-kernel ioapic.
    pub fn set_irqchip_state(&self, irqchip: &kvm_irqchip) -> Result<()> {
        self.vm_fd
            .set_irqchip(irqchip)
            .map_err(|e| Error::Vm(VmError::Irq(e)))
    }
}

impl Vm {
    /// Initialize the virtual machine instance.
    ///
    /// It initialize the virtual machine instance by:
    /// 1) initialize virtual machine global state and configuration.
    /// 2) create system devices, such as interrupt controller, PIT etc.
    /// 3) create and start IO devices, such as serial, console, block, net, vsock etc.
    /// 4) create and initialize vCPUs.
    /// 5) configure CPU power management features.
    /// 6) load guest kernel image.
    pub fn init_microvm(
        &mut self,
        epoll_mgr: EpollManager,
        vm_as: GuestAddressSpaceImpl,
        request_ts: TimestampUs,
    ) -> std::result::Result<(), StartMicroVmError> {
        info!(self.logger, "VM: start initializing microvm ...");

        self.init_tss()?;
        // For x86_64 we need to create the interrupt controller before calling `KVM_CREATE_VCPUS`
        // while on aarch64 we need to do it the other way around.
        self.setup_interrupt_controller()?;
        self.create_pit()?;
        self.init_devices(epoll_mgr)?;

        let reset_event_fd = self.device_manager.get_reset_eventfd().unwrap();
        self.vcpu_manager()
            .map_err(StartMicroVmError::Vcpu)?
            .set_reset_event_fd(reset_event_fd)
            .map_err(StartMicroVmError::Vcpu)?;

        if self.vm_config.cpu_pm == "on" {
            // TODO: add cpu_pm support. issue #4590.
            info!(self.logger, "VM: enable CPU disable_idle_exits capability");
        }

        let vm_memory = vm_as.memory();
        let kernel_loader_result = self.load_kernel(vm_memory.deref())?;
        self.vcpu_manager()
            .map_err(StartMicroVmError::Vcpu)?
            .create_boot_vcpus(request_ts, kernel_loader_result.kernel_load)
            .map_err(StartMicroVmError::Vcpu)?;

        info!(self.logger, "VM: initializing microvm done");
        Ok(())
    }

    /// Execute system architecture specific configurations.
    ///
    /// 1) set guest kernel boot parameters
    /// 2) setup BIOS configuration data structs, mainly implement the MPSpec.
    pub fn configure_system_arch(
        &self,
        vm_memory: &GuestMemoryImpl,
        cmdline: &Cmdline,
        initrd: Option<InitrdConfig>,
    ) -> std::result::Result<(), StartMicroVmError> {
        let cmdline_addr = GuestAddress(dbs_boot::layout::CMDLINE_START);
        linux_loader::loader::load_cmdline(vm_memory, cmdline_addr, cmdline)
            .map_err(StartMicroVmError::LoadCommandline)?;

        let cmdline_size = cmdline
            .as_cstring()
            .map_err(StartMicroVmError::ProcessCommandlne)?
            .as_bytes_with_nul()
            .len();

        #[cfg(feature = "host-device")]
        {
            // Don't expect poisoned lock here.
            let vfio_manager = self.device_manager.vfio_manager.lock().unwrap();
            configure_system(
                vm_memory,
                self.address_space.address_space(),
                cmdline_addr,
                cmdline_size,
                &initrd,
                self.vm_config.vcpu_count,
                self.vm_config.max_vcpu_count,
                vfio_manager.get_pci_legacy_irqs(),
            )
            .map_err(StartMicroVmError::ConfigureSystem)
        }

        #[cfg(not(feature = "host-device"))]
        configure_system(
            vm_memory,
            self.address_space.address_space(),
            cmdline_addr,
            cmdline_size,
            &initrd,
            self.vm_config.vcpu_count,
            self.vm_config.max_vcpu_count,
            None,
        )
        .map_err(StartMicroVmError::ConfigureSystem)
    }

    /// Initializes the guest memory.
    pub(crate) fn init_tss(&mut self) -> std::result::Result<(), StartMicroVmError> {
        self.vm_fd
            .set_tss_address(dbs_boot::layout::KVM_TSS_ADDRESS.try_into().unwrap())
            .map_err(|e| StartMicroVmError::ConfigureVm(VmError::VmSetup(e)))
    }

    /// Creates the irq chip and an in-kernel device model for the PIT.
    pub(crate) fn setup_interrupt_controller(
        &mut self,
    ) -> std::result::Result<(), StartMicroVmError> {
        self.vm_fd
            .create_irq_chip()
            .map_err(|e| StartMicroVmError::ConfigureVm(VmError::VmSetup(e)))
    }

    /// Creates an in-kernel device model for the PIT.
    pub(crate) fn create_pit(&self) -> std::result::Result<(), StartMicroVmError> {
        info!(self.logger, "VM: create pit");
        // We need to enable the emulation of a dummy speaker port stub so that writing to port 0x61
        // (i.e. KVM_SPEAKER_BASE_ADDRESS) does not trigger an exit to user space.
        let pit_config = kvm_pit_config {
            flags: KVM_PIT_SPEAKER_DUMMY,
            ..kvm_pit_config::default()
        };

        // Safe because we know that our file is a VM fd, we know the kernel will only read the
        // correct amount of memory from our pointer, and we verify the return result.
        self.vm_fd
            .create_pit2(pit_config)
            .map_err(|e| StartMicroVmError::ConfigureVm(VmError::VmSetup(e)))
    }

    pub(crate) fn register_events(
        &mut self,
        event_mgr: &mut EventManager,
    ) -> std::result::Result<(), StartMicroVmError> {
        let reset_evt = self
            .device_manager
            .get_reset_eventfd()
            .map_err(StartMicroVmError::DeviceManager)?;
        event_mgr
            .register_exit_eventfd(&reset_evt)
            .map_err(|_| StartMicroVmError::RegisterEvent)?;
        self.reset_eventfd = Some(reset_evt);

        Ok(())
    }
}
