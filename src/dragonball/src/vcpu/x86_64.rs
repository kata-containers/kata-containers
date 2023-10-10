// Copyright (C) 2022 Alibaba Cloud. All rights reserved.
// Copyright 2018 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0
//
// Portions Copyright 2017 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the THIRD-PARTY file.

use std::sync::mpsc::{channel, Sender};
use std::sync::Arc;

use dbs_arch::cpuid::{process_cpuid, VmSpec};
use dbs_arch::gdt::gdt_entry;
use dbs_utils::metric::IncMetric;
use dbs_utils::time::TimestampUs;
use kvm_bindings::CpuId;
use kvm_ioctls::{VcpuFd, VmFd};
use log::error;
use vm_memory::{Address, GuestAddress, GuestAddressSpace};
use vmm_sys_util::eventfd::EventFd;

use crate::address_space_manager::GuestAddressSpaceImpl;
use crate::metric::VcpuMetrics;
use crate::vcpu::vcpu_impl::{Result, Vcpu, VcpuError, VcpuStateEvent};
use crate::vcpu::VcpuConfig;
use crate::IoManagerCached;

impl Vcpu {
    /// Constructs a new VCPU for `vm`.
    ///
    /// # Arguments
    ///
    /// * `id` - Represents the CPU number between [0, max vcpus).
    /// * `vcpu_fd` - The kvm `VcpuFd` for the vcpu.
    /// * `io_mgr` - The io-manager used to access port-io and mmio devices.
    /// * `cpuid` - The `CpuId` listing the supported capabilities of this vcpu.
    /// * `exit_evt` - An `EventFd` that will be written into when this vcpu
    ///   exits.
    /// * `vcpu_state_event` - The eventfd which can notify vmm state of some
    ///   vcpu should change.
    /// * `vcpu_state_sender` - The channel to send state change message from
    ///   vcpu thread to vmm thread.
    /// * `create_ts` - A timestamp used by the vcpu to calculate its lifetime.
    /// * `support_immediate_exit` -  whether kvm used supports immediate_exit flag.
    #[allow(clippy::too_many_arguments)]
    pub fn new_x86_64(
        id: u8,
        vcpu_fd: Arc<VcpuFd>,
        io_mgr: IoManagerCached,
        cpuid: CpuId,
        exit_evt: EventFd,
        vcpu_state_event: EventFd,
        vcpu_state_sender: Sender<VcpuStateEvent>,
        create_ts: TimestampUs,
        support_immediate_exit: bool,
    ) -> Result<Self> {
        let (event_sender, event_receiver) = channel();
        let (response_sender, response_receiver) = channel();
        // Initially the cpuid per vCPU is the one supported by this VM.
        Ok(Vcpu {
            fd: vcpu_fd,
            id,
            io_mgr,
            create_ts,
            event_receiver,
            event_sender: Some(event_sender),
            response_receiver: Some(response_receiver),
            response_sender,
            vcpu_state_event,
            vcpu_state_sender,
            exit_evt,
            support_immediate_exit,
            metrics: Arc::new(VcpuMetrics::default()),
            cpuid,
        })
    }

    /// Configures a x86_64 specific vcpu and should be called once per vcpu.
    ///
    /// # Arguments
    ///
    /// * `vm_config` - The machine configuration of this microvm needed for the CPUID configuration.
    /// * `vm_fd` - The kvm `VmFd` for the virtual machine this vcpu will get attached to.
    /// * `vm_memory` - The guest memory used by this microvm.
    /// * `kernel_start_addr` - Offset from `guest_mem` at which the kernel starts.
    /// * `pgtable_addr` - pgtable address for ap vcpu
    pub fn configure(
        &mut self,
        vcpu_config: &VcpuConfig,
        _vm_fd: &VmFd,
        vm_as: &GuestAddressSpaceImpl,
        kernel_start_addr: Option<GuestAddress>,
        _pgtable_addr: Option<GuestAddress>,
    ) -> Result<()> {
        self.set_cpuid(vcpu_config)?;

        dbs_arch::regs::setup_msrs(&self.fd).map_err(VcpuError::MSRSConfiguration)?;
        if let Some(start_addr) = kernel_start_addr {
            dbs_arch::regs::setup_regs(
                &self.fd,
                start_addr.raw_value(),
                dbs_boot::layout::BOOT_STACK_POINTER,
                dbs_boot::layout::BOOT_STACK_POINTER,
                dbs_boot::layout::ZERO_PAGE_START,
            )
            .map_err(VcpuError::REGSConfiguration)?;
            dbs_arch::regs::setup_fpu(&self.fd).map_err(VcpuError::FPUConfiguration)?;
            let gdt_table: [u64; dbs_boot::layout::BOOT_GDT_MAX] = [
                gdt_entry(0, 0, 0),            // NULL
                gdt_entry(0xa09b, 0, 0xfffff), // CODE
                gdt_entry(0xc093, 0, 0xfffff), // DATA
                gdt_entry(0x808b, 0, 0xfffff), // TSS
            ];
            let pgtable_addr =
                dbs_boot::setup_identity_mapping(&*vm_as.memory()).map_err(VcpuError::PageTable)?;
            dbs_arch::regs::setup_sregs(
                &*vm_as.memory(),
                &self.fd,
                pgtable_addr,
                &gdt_table,
                dbs_boot::layout::BOOT_GDT_OFFSET,
                dbs_boot::layout::BOOT_IDT_OFFSET,
            )
            .map_err(VcpuError::SREGSConfiguration)?;
        }
        dbs_arch::interrupts::set_lint(&self.fd).map_err(VcpuError::LocalIntConfiguration)?;

        Ok(())
    }

    fn set_cpuid(&mut self, vcpu_config: &VcpuConfig) -> Result<()> {
        let cpuid_vm_spec = VmSpec::new(
            self.id,
            vcpu_config.max_vcpu_count,
            vcpu_config.threads_per_core,
            vcpu_config.cores_per_die,
            vcpu_config.dies_per_socket,
            vcpu_config.vpmu_feature,
        )
        .map_err(VcpuError::CpuId)?;
        process_cpuid(&mut self.cpuid, &cpuid_vm_spec).map_err(|e| {
            self.metrics.filter_cpuid.inc();
            error!("Failure in configuring CPUID for vcpu {}: {:?}", self.id, e);
            VcpuError::CpuId(e)
        })?;

        self.fd
            .set_cpuid2(&self.cpuid)
            .map_err(VcpuError::SetSupportedCpusFailed)
    }
}
