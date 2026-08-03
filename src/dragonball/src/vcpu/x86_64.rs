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
use dbs_boot::FirmwareType;
use dbs_interrupt::InterruptManager;
use dbs_utils::metric::IncMetric;
use dbs_utils::time::TimestampUs;
use kvm_bindings::{
    kvm_debugregs, kvm_lapic_state, kvm_mp_state, kvm_msr_entry, kvm_regs, kvm_sregs,
    kvm_vcpu_events, kvm_xcrs, kvm_xsave, CpuId, Msrs, KVM_MAX_CPUID_ENTRIES, KVM_MAX_MSR_ENTRIES,
};
use kvm_ioctls::{VcpuFd, VmFd};
use log::{error, warn};
use serde_derive::{Deserialize, Serialize};
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
        vcpu_fd: VcpuFd,
        io_mgr: IoManagerCached,
        cpuid: CpuId,
        exit_evt: EventFd,
        vcpu_state_event: EventFd,
        vcpu_state_sender: Sender<VcpuStateEvent>,
        create_ts: TimestampUs,
        support_immediate_exit: bool,
        irq_manager: Arc<Box<dyn InterruptManager>>,
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
            irq_manager,
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
        firmware_type: Option<FirmwareType>,
    ) -> Result<()> {
        self.set_cpuid(vcpu_config)?;

        // tdshim will handle the initialization of MSR, regs and sregs
        if firmware_type == Some(FirmwareType::Tdshim) {
            return Ok(());
        }

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

/// Snapshot state of a x86_64 vCPU: guest-visible registers plus
/// KVM-internal state needed to transparently resume execution.
///
/// Compatibility policy (see `crate::snapshot`): only append new fields, with
/// `#[serde(default)]`; never remove or repurpose existing ones.
// No `Clone`: `kvm_xsave` contains an incomplete array field.
#[derive(Deserialize, Serialize)]
pub struct VcpuState {
    /// vCPU id.
    pub id: u8,
    /// CPUID configuration of this vCPU.
    pub cpuid: CpuId,
    /// Model specific registers, chunked by `KVM_MAX_MSR_ENTRIES`.
    pub msrs: Vec<Msrs>,
    /// General purpose registers.
    pub regs: kvm_regs,
    /// Special registers.
    pub sregs: kvm_sregs,
    /// XSAVE area (contains FPU/SSE/AVX state, superset of `kvm_fpu`).
    pub xsave: kvm_xsave,
    /// Extended control registers.
    pub xcrs: kvm_xcrs,
    /// Debug registers.
    pub debug_regs: kvm_debugregs,
    /// Local APIC state.
    pub lapic: kvm_lapic_state,
    /// Multiprocessing state.
    pub mp_state: kvm_mp_state,
    /// Pending vCPU events.
    pub vcpu_events: kvm_vcpu_events,
    /// Guest TSC frequency in kHz, if the host supports retrieving it.
    #[serde(default)]
    pub tsc_khz: Option<u32>,
}

impl<'a> dbs_snapshot::Persist<'a> for Vcpu {
    type State = VcpuState;
    type SaveArgs = &'a [u32];
    type RestoreArgs = ();
    type Error = VcpuError;

    /// Save the KVM state of this vCPU.
    ///
    /// The vCPU must be quiesced (not running) when this is called; use the
    /// vCPU manager's pause machinery first.
    ///
    /// # Arguments
    ///
    /// * `msr_index_list` - Indices of the MSRs to save, typically
    ///   `KvmContext::supported_msrs()` (pre-filtered for serializability).
    fn save_state(&mut self, msr_index_list: &'a [u32]) -> Result<VcpuState> {
        // Ordering matters (mirrors Firecracker):
        // - KVM_GET_MP_STATE calls kvm_apic_accept_events(), which might
        //   modify vCPU/LAPIC state, so it must run before everything else.
        // - KVM_GET_VCPU_EVENTS is read last as the other GET ioctls may
        //   modify internal pending-event state.
        let mp_state = self.fd.get_mp_state().map_err(VcpuError::Kvm)?;
        let regs = self.fd.get_regs().map_err(VcpuError::Kvm)?;
        let sregs = self.fd.get_sregs().map_err(VcpuError::Kvm)?;
        let xsave = self.fd.get_xsave().map_err(VcpuError::Kvm)?;
        let xcrs = self.fd.get_xcrs().map_err(VcpuError::Kvm)?;
        let debug_regs = self.fd.get_debug_regs().map_err(VcpuError::Kvm)?;
        let lapic = self.fd.get_lapic().map_err(VcpuError::Kvm)?;
        let cpuid = self
            .fd
            .get_cpuid2(KVM_MAX_CPUID_ENTRIES)
            .map_err(VcpuError::Kvm)?;
        let tsc_khz = self.fd.get_tsc_khz().ok();

        let mut msrs = Vec::new();
        for chunk in msr_index_list.chunks(KVM_MAX_MSR_ENTRIES) {
            let entries: Vec<kvm_msr_entry> = chunk
                .iter()
                .map(|index| kvm_msr_entry {
                    index: *index,
                    ..Default::default()
                })
                .collect();
            let mut chunk_msrs = Msrs::from_entries(&entries).map_err(VcpuError::Msr)?;
            let nmsrs = self.fd.get_msrs(&mut chunk_msrs).map_err(VcpuError::Kvm)?;
            if nmsrs != entries.len() {
                // KVM stops at the first unreadable MSR, leaving the rest of
                // the chunk zeroed; persisting it would write those zeros
                // into the restored guest. Refuse the save instead.
                return Err(VcpuError::MsrsIncomplete {
                    id: self.id,
                    processed: nmsrs,
                    requested: entries.len(),
                });
            }
            msrs.push(chunk_msrs);
        }

        let vcpu_events = self.fd.get_vcpu_events().map_err(VcpuError::Kvm)?;

        Ok(VcpuState {
            id: self.id,
            cpuid,
            msrs,
            regs,
            sregs,
            xsave,
            xcrs,
            debug_regs,
            lapic,
            mp_state,
            vcpu_events,
            tsc_khz,
        })
    }

    /// Restore the KVM state of this vCPU from a previously saved state.
    ///
    /// Must be called on a freshly created, unconfigured vCPU before it runs.
    fn restore_state(&mut self, state: &VcpuState, _args: ()) -> Result<()> {
        // Ordering matters (mirrors Firecracker): CPUID first as it gates
        // MSR/XSAVE validation, events last.
        self.cpuid = state.cpuid.clone();
        self.fd
            .set_cpuid2(&state.cpuid)
            .map_err(VcpuError::SetSupportedCpusFailed)?;
        for chunk in &state.msrs {
            let expected = chunk.as_fam_struct_ref().nmsrs as usize;
            let nmsrs = self.fd.set_msrs(chunk).map_err(VcpuError::Kvm)?;
            if nmsrs != expected {
                return Err(VcpuError::MsrsIncomplete {
                    id: self.id,
                    processed: nmsrs,
                    requested: expected,
                });
            }
        }
        self.fd.set_sregs(&state.sregs).map_err(VcpuError::Kvm)?;
        // SAFETY: the xsave area was obtained from KVM_GET_XSAVE with the
        // standard fixed-size `kvm_xsave` region, so it cannot exceed the
        // buffer the kernel copies from.
        unsafe {
            self.fd.set_xsave(&state.xsave).map_err(VcpuError::Kvm)?;
        }
        self.fd.set_xcrs(&state.xcrs).map_err(VcpuError::Kvm)?;
        self.fd
            .set_debug_regs(&state.debug_regs)
            .map_err(VcpuError::Kvm)?;
        self.fd.set_lapic(&state.lapic).map_err(VcpuError::Kvm)?;
        self.fd
            .set_mp_state(state.mp_state)
            .map_err(VcpuError::Kvm)?;
        self.fd.set_regs(&state.regs).map_err(VcpuError::Kvm)?;
        self.fd
            .set_vcpu_events(&state.vcpu_events)
            .map_err(VcpuError::Kvm)?;
        if let Some(tsc_khz) = state.tsc_khz {
            // Best effort: hosts without KVM_CAP_TSC_CONTROL cannot set it.
            if let Err(e) = self.fd.set_tsc_khz(tsc_khz) {
                warn!("vcpu {}: failed to restore TSC frequency: {}", self.id, e);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc::channel;

    use arc_swap::ArcSwap;
    use dbs_device::device_manager::IoManager;
    use dbs_interrupt::KvmIrqManager;
    use kvm_bindings::MsrList;
    use test_utils::skip_if_kvm_unaccessable;

    use super::*;
    use crate::kvm_context::KvmContext;

    // A vCPU on its own VM with an in-kernel irqchip (required by
    // KVM_GET/SET_LAPIC).
    fn create_vcpu_with_irqchip() -> (Vcpu, MsrList) {
        let kvm_context = KvmContext::new(None).unwrap();
        let vm = kvm_context.create_vm().unwrap();
        vm.create_irq_chip().unwrap();
        let msr_list = kvm_context.supported_msrs(0).unwrap();
        let supported_cpuid = kvm_context.supported_cpuid(KVM_MAX_CPUID_ENTRIES).unwrap();
        let vm = Arc::new(vm);
        let vcpu_fd = vm.create_vcpu(0).unwrap();
        let io_manager = IoManagerCached::new(Arc::new(ArcSwap::new(Arc::new(IoManager::new()))));
        let irq_manager: Arc<Box<dyn InterruptManager>> =
            Arc::new(Box::new(KvmIrqManager::new(vm)));
        let (tx, _rx) = channel();

        let vcpu = Vcpu::new_x86_64(
            0,
            vcpu_fd,
            io_manager,
            supported_cpuid,
            EventFd::new(libc::EFD_NONBLOCK).unwrap(),
            EventFd::new(libc::EFD_NONBLOCK).unwrap(),
            tx,
            TimestampUs::default(),
            false,
            irq_manager,
        )
        .unwrap();

        (vcpu, msr_list)
    }

    #[test]
    fn test_vcpu_save_restore_roundtrip() {
        skip_if_kvm_unaccessable!();

        let (mut src, msr_list) = create_vcpu_with_irqchip();

        // Plant recognizable register values on the source vCPU.
        let mut regs = src.fd.get_regs().unwrap();
        regs.rbx = 0xdbdb;
        regs.rip = 0x1000;
        src.fd.set_regs(&regs).unwrap();

        let state = dbs_snapshot::Persist::save_state(&mut src, msr_list.as_slice()).unwrap();
        assert_eq!(state.id, 0);
        assert_eq!(state.regs.rbx, 0xdbdb);
        assert_eq!(state.regs.rip, 0x1000);
        assert!(!state.msrs.is_empty());

        // The state itself must survive a JSON round-trip.
        let json = serde_json::to_string(&state).unwrap();
        let state: VcpuState = serde_json::from_str(&json).unwrap();
        assert_eq!(state.regs.rbx, 0xdbdb);
        assert_eq!(state.regs.rip, 0x1000);

        // Restore into a fresh vCPU on a fresh VM and verify the KVM state.
        let (mut dst, _) = create_vcpu_with_irqchip();
        dbs_snapshot::Persist::restore_state(&mut dst, &state, ()).unwrap();

        let dst_regs = dst.fd.get_regs().unwrap();
        assert_eq!(dst_regs.rbx, 0xdbdb);
        assert_eq!(dst_regs.rip, 0x1000);
        let dst_sregs = dst.fd.get_sregs().unwrap();
        assert_eq!(dst_sregs.cr0, state.sregs.cr0);
        assert_eq!(dst_sregs.cs.base, state.sregs.cs.base);
        assert_eq!(
            dst.fd.get_mp_state().unwrap().mp_state,
            state.mp_state.mp_state
        );
    }
}
