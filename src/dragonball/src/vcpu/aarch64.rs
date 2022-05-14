// Copyright (C) 2022 Alibaba Cloud. All rights reserved.
// Copyright 2018 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0
//
// Portions Copyright 2017 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the THIRD-PARTY file.

use std::sync::mpsc::{channel, Sender};
use std::sync::Arc;

use crate::IoManagerCached;
use dbs_utils::time::TimestampUs;
use kvm_ioctls::{VcpuFd, VmFd};
use vm_memory::GuestAddress;
use vmm_sys_util::eventfd::EventFd;

use crate::address_space_manager::GuestAddressSpaceImpl;
use crate::vcpu::vcpu_impl::{Result, Vcpu, VcpuStateEvent};
use crate::vcpu::VcpuConfig;

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
        _vcpu_config: &VcpuConfig,
        vm_fd: &VmFd,
        vm_as: &GuestAddressSpaceImpl,
        kernel_load_addr: Option<GuestAddress>,
        _pgtable_addr: Option<GuestAddress>,
    ) -> Result<()> {
        // TODO: add arm vcpu configure() function. issue: #4445
        Ok(())
    }

    /// Gets the MPIDR register value.
    pub fn get_mpidr(&self) -> u64 {
        self.mpidr
    }
}
