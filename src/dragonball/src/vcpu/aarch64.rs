// Copyright (C) 2022 Alibaba Cloud. All rights reserved.
// Copyright 2018 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0
//
// Portions Copyright 2017 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the THIRD-PARTY file.

use std::ops::Deref;
use std::sync::mpsc::{channel, Sender};
use std::sync::Arc;

use dbs_arch::{regs, VpmuFeatureLevel};
use dbs_boot::get_fdt_addr;
use dbs_utils::time::TimestampUs;
use kvm_ioctls::{VcpuFd, VmFd};
use vm_memory::{Address, GuestAddress, GuestAddressSpace};
use vmm_sys_util::eventfd::EventFd;

use crate::address_space_manager::GuestAddressSpaceImpl;
use crate::metric::VcpuMetrics;
use crate::vcpu::vcpu_impl::{Result, Vcpu, VcpuError, VcpuStateEvent};
use crate::vcpu::VcpuConfig;
use crate::IoManagerCached;

#[allow(unused)]
impl Vcpu {
    /// Constructs a new VCPU for `vm`.
    ///
    /// # Arguments
    ///
    /// * `id` - Represents the CPU number between [0, max vcpus).
    /// * `vcpu_fd` - The kvm `VcpuFd` for the vcpu.
    /// * `io_mgr` - The io-manager used to access port-io and mmio devices.
    /// * `exit_evt` - An `EventFd` that will be written into when this vcpu
    ///   exits.
    /// * `vcpu_state_event` - The eventfd which can notify vmm state of some
    ///   vcpu should change.
    /// * `vcpu_state_sender` - The channel to send state change message from
    ///   vcpu thread to vmm thread.
    /// * `create_ts` - A timestamp used by the vcpu to calculate its lifetime.
    /// * `support_immediate_exit` -  whether kvm uses supports immediate_exit flag.
    #[allow(clippy::too_many_arguments)]
    pub fn new_aarch64(
        id: u8,
        vcpu_fd: Arc<VcpuFd>,
        io_mgr: IoManagerCached,
        exit_evt: EventFd,
        vcpu_state_event: EventFd,
        vcpu_state_sender: Sender<VcpuStateEvent>,
        create_ts: TimestampUs,
        support_immediate_exit: bool,
    ) -> Result<Self> {
        let (event_sender, event_receiver) = channel();
        let (response_sender, response_receiver) = channel();

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
            support_immediate_exit,
            mpidr: 0,
            exit_evt,
            metrics: Arc::new(VcpuMetrics::default()),
        })
    }

    /// Configures an aarch64 specific vcpu.
    ///
    /// # Arguments
    ///
    /// * `vcpu_config` - vCPU config for this vCPU status
    /// * `vm_fd` - The kvm `VmFd` for this microvm.
    /// * `vm_as` - The guest memory address space used by this microvm.
    /// * `kernel_load_addr` - Offset from `guest_mem` at which the kernel is loaded.
    /// * `_pgtable_addr` - pgtable address for ap vcpu (not used in aarch64)
    pub fn configure(
        &mut self,
        vcpu_config: &VcpuConfig,
        vm_fd: &VmFd,
        vm_as: &GuestAddressSpaceImpl,
        kernel_load_addr: Option<GuestAddress>,
        _pgtable_addr: Option<GuestAddress>,
    ) -> Result<()> {
        let mut kvi: kvm_bindings::kvm_vcpu_init = kvm_bindings::kvm_vcpu_init::default();

        // This reads back the kernel's preferred target type.
        vm_fd
            .get_preferred_target(&mut kvi)
            .map_err(VcpuError::VcpuArmPreferredTarget)?;
        // We already checked that the capability is supported.
        kvi.features[0] |= 1 << kvm_bindings::KVM_ARM_VCPU_PSCI_0_2;
        // Non-boot cpus are powered off initially.
        if self.id > 0 {
            kvi.features[0] |= 1 << kvm_bindings::KVM_ARM_VCPU_POWER_OFF;
        }
        if vcpu_config.vpmu_feature == VpmuFeatureLevel::FullyEnabled {
            kvi.features[0] |= 1 << kvm_bindings::KVM_ARM_VCPU_PMU_V3;
        }

        self.fd.vcpu_init(&kvi).map_err(VcpuError::VcpuArmInit)?;

        if let Some(address) = kernel_load_addr {
            regs::setup_regs(
                &self.fd,
                self.id,
                address.raw_value(),
                get_fdt_addr(vm_as.memory().deref()),
            )
            .map_err(VcpuError::REGSConfiguration)?;
        }

        self.mpidr = regs::read_mpidr(&self.fd).map_err(VcpuError::REGSConfiguration)?;

        Ok(())
    }

    /// Gets the MPIDR register value.
    pub fn get_mpidr(&self) -> u64 {
        self.mpidr
    }
}
