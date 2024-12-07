// Copyright 2021-2022 Alibaba Cloud. All Rights Reserved.
// Copyright 2018 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0
//
// Portions Copyright 2017 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the THIRD-PARTY file.

//! Constants and utilities for x86 CPU generic, system and model specific registers.

use std::mem;

use kvm_bindings::{kvm_fpu, kvm_msr_entry, kvm_regs, kvm_sregs, Msrs};
use kvm_ioctls::VcpuFd;
use vm_memory::{Address, Bytes, GuestAddress, GuestMemory};

use super::gdt::kvm_segment_from_gdt;
use super::msr;

/// Non-Executable bit in EFER MSR.
pub const EFER_NX: u64 = 0x800;
/// Long-mode active bit in EFER MSR.
pub const EFER_LMA: u64 = 0x400;
/// Long-mode enable bit in EFER MSR.
pub const EFER_LME: u64 = 0x100;

/// Protection mode enable bit in CR0.
pub const X86_CR0_PE: u64 = 0x1;
/// Paging enable bit in CR0.
pub const X86_CR0_PG: u64 = 0x8000_0000;
/// Physical Address Extension bit in CR4.
pub const X86_CR4_PAE: u64 = 0x20;

// MTRR constants.
const MTRR_ENABLED: u64 = 0x0800; // IA32_MTRR_DEF_TYPE MSR: E (MTRRs enabled) flag, bit 11
const MTRR_FIXED_RANGE_ENABLE: u64 = 0x0400;
const MTRR_MEM_TYPE_WB: u64 = 0x6;

/// Errors thrown while setting up x86_64 registers.
#[derive(Debug)]
pub enum Error {
    /// Failed to get SREGs for this CPU.
    GetStatusRegisters(kvm_ioctls::Error),
    /// Failed to set base registers for this CPU.
    SetBaseRegisters(kvm_ioctls::Error),
    /// Failed to configure the FPU.
    SetFPURegisters(kvm_ioctls::Error),
    /// Setting up MSRs failed.
    SetModelSpecificRegisters(kvm_ioctls::Error),
    /// Failed to set all MSRs.
    SetModelSpecificRegistersCount,
    /// Failed to set SREGs for this CPU.
    SetStatusRegisters(kvm_ioctls::Error),
    /// Writing the GDT to RAM failed.
    WriteGDT,
    /// Writing the IDT to RAM failed.
    WriteIDT,
}

type Result<T> = std::result::Result<T, Error>;

/// Configure Floating-Point Unit (FPU) registers for a given CPU.
///
/// # Arguments
///
/// * `vcpu` - Structure for the VCPU that holds the VCPU's fd.
pub fn setup_fpu(vcpu: &VcpuFd) -> Result<()> {
    let fpu: kvm_fpu = kvm_fpu {
        fcw: 0x37f,
        mxcsr: 0x1f80,
        ..Default::default()
    };

    vcpu.set_fpu(&fpu).map_err(Error::SetFPURegisters)
}

/// Configure Model Specific Registers (MSRs) for a given CPU.
///
/// # Arguments
///
/// * `vcpu` - Structure for the VCPU that holds the VCPU's fd.
pub fn setup_msrs(vcpu: &VcpuFd) -> Result<()> {
    let entry_vec = create_msr_entries();
    let kvm_msrs =
        Msrs::from_entries(&entry_vec).map_err(|_| Error::SetModelSpecificRegistersCount)?;

    vcpu.set_msrs(&kvm_msrs)
        .map_err(Error::SetModelSpecificRegisters)
        .and_then(|msrs_written| {
            if msrs_written as u32 != kvm_msrs.as_fam_struct_ref().nmsrs {
                Err(Error::SetModelSpecificRegistersCount)
            } else {
                Ok(msrs_written)
            }
        })?;
    Ok(())
}

/// Configure base registers for a given CPU.
///
/// # Arguments
///
/// * `vcpu` - Structure for the VCPU that holds the VCPU's fd.
/// * `boot_ip` - Starting instruction pointer.
/// * `rsp` - Value for RSP register
/// * `rbp` - Value for RBP register
/// * `rsi` - Value for RSI register
pub fn setup_regs(vcpu: &VcpuFd, boot_ip: u64, rsp: u64, rbp: u64, rsi: u64) -> Result<()> {
    let regs: kvm_regs = kvm_regs {
        rflags: 0x0000_0000_0000_0002u64,
        rip: boot_ip,
        rsp,
        rbp,
        rsi,
        ..Default::default()
    };

    vcpu.set_regs(&regs).map_err(Error::SetBaseRegisters)
}

/// Configures the segment registers for a given CPU.
///
/// # Arguments
///
/// * `mem` - The memory that will be passed to the guest.
/// * `vcpu` - Structure for the VCPU that holds the VCPU's fd.
/// * `pgtable_addr` - Address of the vcpu pgtable.
/// * `gdt_table` - Content of the global descriptor table.
/// * `gdt_addr` - Address of the global descriptor table.
/// * `idt_addr` - Address of the interrupt descriptor table.
pub fn setup_sregs<M: GuestMemory>(
    mem: &M,
    vcpu: &VcpuFd,
    pgtable_addr: GuestAddress,
    gdt_table: &[u64],
    gdt_addr: u64,
    idt_addr: u64,
) -> Result<()> {
    let mut sregs: kvm_sregs = vcpu.get_sregs().map_err(Error::GetStatusRegisters)?;
    configure_segments_and_sregs(mem, &mut sregs, pgtable_addr, gdt_table, gdt_addr, idt_addr)?;
    vcpu.set_sregs(&sregs).map_err(Error::SetStatusRegisters)
}

fn configure_segments_and_sregs<M: GuestMemory>(
    mem: &M,
    sregs: &mut kvm_sregs,
    pgtable_addr: GuestAddress,
    gdt_table: &[u64],
    gdt_addr: u64,
    idt_addr: u64,
) -> Result<()> {
    assert!(gdt_table.len() >= 4);
    let code_seg = kvm_segment_from_gdt(gdt_table[1], 1);
    let data_seg = kvm_segment_from_gdt(gdt_table[2], 2);
    let tss_seg = kvm_segment_from_gdt(gdt_table[3], 3);

    // Write segments
    write_gdt_table(gdt_table, gdt_addr, mem)?;
    sregs.gdt.base = gdt_addr;
    sregs.gdt.limit = std::mem::size_of_val(gdt_table) as u16 - 1;

    write_idt_value(0, idt_addr, mem)?;
    sregs.idt.base = idt_addr;
    sregs.idt.limit = mem::size_of::<u64>() as u16 - 1;

    sregs.cs = code_seg;
    sregs.ds = data_seg;
    sregs.es = data_seg;
    sregs.fs = data_seg;
    sregs.gs = data_seg;
    sregs.ss = data_seg;
    sregs.tr = tss_seg;

    /* 64-bit protected mode */
    sregs.cr0 |= X86_CR0_PE;
    sregs.cr3 = pgtable_addr.raw_value();
    sregs.cr4 |= X86_CR4_PAE;
    sregs.cr0 |= X86_CR0_PG;
    sregs.efer |= EFER_LME | EFER_LMA;

    Ok(())
}

fn write_gdt_table<M: GuestMemory>(gdt_table: &[u64], gdt_addr: u64, guest_mem: &M) -> Result<()> {
    let boot_gdt_addr = GuestAddress(gdt_addr);
    for (index, entry) in gdt_table.iter().enumerate() {
        let addr = guest_mem
            .checked_offset(boot_gdt_addr, index * mem::size_of::<u64>())
            .ok_or(Error::WriteGDT)?;
        guest_mem
            .write_obj(*entry, addr)
            .map_err(|_| Error::WriteGDT)?;
    }
    Ok(())
}

fn write_idt_value<M: GuestMemory>(idt_table: u64, idt_addr: u64, guest_mem: &M) -> Result<()> {
    let boot_idt_addr = GuestAddress(idt_addr);
    guest_mem
        .write_obj(idt_table, boot_idt_addr)
        .map_err(|_| Error::WriteIDT)
}

#[allow(clippy::vec_init_then_push)]
fn create_msr_entries() -> Vec<kvm_msr_entry> {
    let mut entries = Vec::<kvm_msr_entry>::new();

    entries.push(kvm_msr_entry {
        index: msr::MSR_IA32_SYSENTER_CS,
        data: 0x0,
        ..Default::default()
    });
    entries.push(kvm_msr_entry {
        index: msr::MSR_IA32_SYSENTER_ESP,
        data: 0x0,
        ..Default::default()
    });
    entries.push(kvm_msr_entry {
        index: msr::MSR_IA32_SYSENTER_EIP,
        data: 0x0,
        ..Default::default()
    });
    entries.push(kvm_msr_entry {
        index: msr::MSR_MTRRdefType,
        data: MTRR_ENABLED | MTRR_FIXED_RANGE_ENABLE | MTRR_MEM_TYPE_WB,
        ..Default::default()
    });
    // x86_64 specific msrs, we only run on x86_64 not x86.
    entries.push(kvm_msr_entry {
        index: msr::MSR_STAR,
        data: 0x0,
        ..Default::default()
    });
    entries.push(kvm_msr_entry {
        index: msr::MSR_CSTAR,
        data: 0x0,
        ..Default::default()
    });
    entries.push(kvm_msr_entry {
        index: msr::MSR_KERNEL_GS_BASE,
        data: 0x0,
        ..Default::default()
    });
    entries.push(kvm_msr_entry {
        index: msr::MSR_SYSCALL_MASK,
        data: 0x0,
        ..Default::default()
    });
    entries.push(kvm_msr_entry {
        index: msr::MSR_LSTAR,
        data: 0x0,
        ..Default::default()
    });
    // end of x86_64 specific code
    entries.push(kvm_msr_entry {
        index: msr::MSR_IA32_TSC,
        data: 0x0,
        ..Default::default()
    });
    entries.push(kvm_msr_entry {
        index: msr::MSR_IA32_MISC_ENABLE,
        data: u64::from(msr::MSR_IA32_MISC_ENABLE_FAST_STRING),
        ..Default::default()
    });

    entries
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::x86_64::gdt::gdt_entry;
    use kvm_ioctls::Kvm;
    use vm_memory::{Bytes, GuestAddress, GuestMemoryMmap};

    const BOOT_GDT_OFFSET: u64 = 0x500;
    const BOOT_IDT_OFFSET: u64 = 0x520;
    const BOOT_STACK_POINTER: u64 = 0x100_0000;
    const ZERO_PAGE_START: u64 = 0x7_C000;
    const BOOT_GDT_MAX: usize = 4;
    const PML4_START: u64 = 0x9000;

    fn create_guest_mem() -> GuestMemoryMmap {
        GuestMemoryMmap::from_ranges(&[(GuestAddress(0), 0x10000)]).unwrap()
    }

    fn read_u64(gm: &GuestMemoryMmap, offset: u64) -> u64 {
        let read_addr = GuestAddress(offset);
        gm.read_obj(read_addr).unwrap()
    }

    fn validate_segments_and_sregs(gm: &GuestMemoryMmap, sregs: &kvm_sregs) {
        assert_eq!(0x0, read_u64(gm, BOOT_GDT_OFFSET));
        assert_eq!(0xaf_9b00_0000_ffff, read_u64(gm, BOOT_GDT_OFFSET + 8));
        assert_eq!(0xcf_9300_0000_ffff, read_u64(gm, BOOT_GDT_OFFSET + 16));
        assert_eq!(0x8f_8b00_0000_ffff, read_u64(gm, BOOT_GDT_OFFSET + 24));
        assert_eq!(0x0, read_u64(gm, BOOT_IDT_OFFSET));

        assert_eq!(0, sregs.cs.base);
        assert_eq!(0xfffff, sregs.ds.limit);
        assert_eq!(0x10, sregs.es.selector);
        assert_eq!(1, sregs.fs.present);
        assert_eq!(1, sregs.gs.g);
        assert_eq!(0, sregs.ss.avl);
        assert_eq!(0, sregs.tr.base);
        assert_eq!(0xfffff, sregs.tr.limit);
        assert_eq!(0, sregs.tr.avl);
        assert!(sregs.cr0 & X86_CR0_PE != 0);
        assert!(sregs.efer & EFER_LME != 0 && sregs.efer & EFER_LMA != 0);
    }

    #[test]
    fn test_configure_segments_and_sregs() {
        let mut sregs: kvm_sregs = Default::default();
        let gm = create_guest_mem();
        let gdt_table: [u64; BOOT_GDT_MAX] = [
            gdt_entry(0, 0, 0),            // NULL
            gdt_entry(0xa09b, 0, 0xfffff), // CODE
            gdt_entry(0xc093, 0, 0xfffff), // DATA
            gdt_entry(0x808b, 0, 0xfffff), // TSS
        ];
        configure_segments_and_sregs(
            &gm,
            &mut sregs,
            GuestAddress(PML4_START),
            &gdt_table,
            BOOT_GDT_OFFSET,
            BOOT_IDT_OFFSET,
        )
        .unwrap();

        validate_segments_and_sregs(&gm, &sregs);
    }

    #[test]
    fn test_setup_fpu() {
        let kvm = Kvm::new().unwrap();
        let vm = kvm.create_vm().unwrap();
        let vcpu = vm.create_vcpu(0).unwrap();
        setup_fpu(&vcpu).unwrap();

        let expected_fpu: kvm_fpu = kvm_fpu {
            fcw: 0x37f,
            mxcsr: 0x1f80,
            ..Default::default()
        };
        let actual_fpu: kvm_fpu = vcpu.get_fpu().unwrap();
        assert_eq!(expected_fpu.fcw, actual_fpu.fcw);
        // Setting the mxcsr register from kvm_fpu inside setup_fpu does not influence anything.
        // See 'kvm_arch_vcpu_ioctl_set_fpu' from arch/x86/kvm/x86.c.
        // The mxcsr will stay 0 and the assert below fails. Decide whether or not we should
        // remove it at all.
        // assert!(expected_fpu.mxcsr == actual_fpu.mxcsr);
    }

    #[test]
    #[allow(clippy::cast_ptr_alignment)]
    fn test_setup_msrs() {
        let kvm = Kvm::new().unwrap();
        let vm = kvm.create_vm().unwrap();
        let vcpu = vm.create_vcpu(0).unwrap();
        setup_msrs(&vcpu).unwrap();

        // This test will check against the last MSR entry configured (the tenth one).
        // See create_msr_entries() for details.
        let test_kvm_msrs_entry = [kvm_msr_entry {
            index: msr::MSR_IA32_MISC_ENABLE,
            ..Default::default()
        }];
        let mut kvm_msrs = Msrs::from_entries(&test_kvm_msrs_entry).unwrap();

        // kvm_ioctls::get_msrs() returns the number of msrs that it succeeded in reading.
        // We only want to read one in this test case scenario.
        let read_nmsrs = vcpu.get_msrs(&mut kvm_msrs).unwrap();
        // Validate it only read one.
        assert_eq!(read_nmsrs, 1);

        // Official entries that were setup when we did setup_msrs. We need to assert that the
        // tenth one (i.e the one with index msr_index::MSR_IA32_MISC_ENABLE has the data we
        // expect.
        let entry_vec = create_msr_entries();
        assert_eq!(entry_vec[10], kvm_msrs.as_slice()[0]);
    }

    #[test]
    fn test_setup_regs() {
        let kvm = Kvm::new().unwrap();
        let vm = kvm.create_vm().unwrap();
        let vcpu = vm.create_vcpu(0).unwrap();

        let expected_regs: kvm_regs = kvm_regs {
            rflags: 0x0000_0000_0000_0002u64,
            rip: 1,
            rsp: BOOT_STACK_POINTER,
            rbp: BOOT_STACK_POINTER,
            rsi: ZERO_PAGE_START,
            ..Default::default()
        };

        setup_regs(
            &vcpu,
            expected_regs.rip,
            BOOT_STACK_POINTER,
            BOOT_STACK_POINTER,
            ZERO_PAGE_START,
        )
        .unwrap();

        let actual_regs: kvm_regs = vcpu.get_regs().unwrap();
        assert_eq!(actual_regs, expected_regs);
    }
}
