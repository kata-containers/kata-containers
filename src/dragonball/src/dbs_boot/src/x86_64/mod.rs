// Copyright 2021 Alibaba Cloud. All Rights Reserved.
// Copyright 2018 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0
//
// Portions Copyright 2017 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the THIRD-PARTY file.

//! VM boot related constants and utilities for `x86_64` architecture.

use dbs_arch::gdt::gdt_entry;
use vm_memory::{Address, ByteValued, Bytes, GuestAddress, GuestMemory, GuestMemoryRegion};

use self::layout::{BOOT_GDT_ADDRESS, BOOT_GDT_MAX, BOOT_IDT_ADDRESS};
use super::Result;

/// Magic addresses externally used to lay out x86_64 VMs.
pub mod layout;

/// Structure definitions for SMP machines following the Intel Multiprocessing Specification 1.1 and 1.4.
pub mod mpspec;

/// MP Table configurations used for defining VM boot status.
pub mod mptable;

/// Guest boot parameters used for config guest information.
pub mod bootparam;

/// Default (smallest) memory page size for the supported architectures.
pub const PAGE_SIZE: usize = 4096;

/// Boot parameters wrapper for ByteValue trait
// This is a workaround to the Rust enforcement specifying that any implementation of a foreign
// trait (in this case `ByteValued`) where:
// *    the type that is implementing the trait is foreign or
// *    all of the parameters being passed to the trait (if there are any) are also foreign
// is prohibited.
#[repr(transparent)]
#[derive(Copy, Clone, Default)]
pub struct BootParamsWrapper(pub bootparam::boot_params);

// It is safe to initialize BootParamsWrap which is a wrapper over `boot_params` (a series of ints).
unsafe impl ByteValued for BootParamsWrapper {}

/// Errors thrown while configuring x86_64 system.
#[derive(Debug, Eq, PartialEq, thiserror::Error)]
pub enum Error {
    /// Invalid e820 setup params.
    #[error("invalid e820 setup parameters")]
    E820Configuration,

    /// Error writing MP table to memory.
    #[error("failed to write MP table to guest memory")]
    MpTableSetup(#[source] mptable::Error),

    /// The zero page extends past the end of guest_mem.
    #[error("the guest zero page extends past the end of guest memory")]
    ZeroPagePastRamEnd,

    /// Error writing the zero page of guest memory.
    #[error("failed to write to guest zero page")]
    ZeroPageSetup,

    /// Failed to compute initrd address.
    #[error("invalid guest memory address for Initrd")]
    InitrdAddress,

    /// boot parameter setup fail.
    #[error("write boot parameter fail")]
    BootParamSetup,

    /// Empty AddressSpace from parameters.
    #[error("Empty AddressSpace from parameters")]
    AddressSpace,

    /// Writing PDPTE to RAM failed.
    #[error("Writing PDPTE to RAM failed.")]
    WritePDPTEAddress,

    /// Writing PDE to RAM failed.
    #[error("Writing PDE to RAM failed.")]
    WritePDEAddress,

    #[error("Writing PML4 to RAM failed.")]
    /// Writing PML4 to RAM failed.
    WritePML4Address,
}

/// Initialize the 1:1 identity mapping table for guest memory range [0..1G).
///
/// Also, return the pml4 address for sregs setting and AP boot
pub fn setup_identity_mapping<M: GuestMemory>(mem: &M) -> Result<GuestAddress> {
    // Puts PML4 right after zero page but aligned to 4k.
    let boot_pml4_addr = GuestAddress(layout::PML4_START);
    let boot_pdpte_addr = GuestAddress(layout::PDPTE_START);
    let boot_pde_addr = GuestAddress(layout::PDE_START);

    // Entry covering VA [0..512GB)
    mem.write_obj(boot_pdpte_addr.raw_value() | 0x03, boot_pml4_addr)
        .map_err(|_| Error::WritePML4Address)?;

    // Entry covering VA [0..1GB)
    mem.write_obj(boot_pde_addr.raw_value() | 0x03, boot_pdpte_addr)
        .map_err(|_| Error::WritePDPTEAddress)?;

    // 512 2MB entries together covering VA [0..1GB). Note we are assuming
    // CPU supports 2MB pages (/proc/cpuinfo has 'pse'). All modern CPUs do.
    for i in 0..512 {
        mem.write_obj((i << 21) + 0x83u64, boot_pde_addr.unchecked_add(i * 8))
            .map_err(|_| Error::WritePDEAddress)?;
    }

    // return the pml4 address that could be used for AP boot up and later sreg setting process.
    Ok(boot_pml4_addr)
}

/// Get information to configure GDT/IDT.
pub fn get_descriptor_config_info() -> ([u64; BOOT_GDT_MAX], u64, u64) {
    let gdt_table: [u64; BOOT_GDT_MAX] = [
        gdt_entry(0, 0, 0),            // NULL
        gdt_entry(0xa09b, 0, 0xfffff), // CODE
        gdt_entry(0xc093, 0, 0xfffff), // DATA
        gdt_entry(0x808b, 0, 0xfffff), // TSS
    ];

    (gdt_table, BOOT_GDT_ADDRESS, BOOT_IDT_ADDRESS)
}

/// Returns the memory address where the initrd could be loaded.
pub fn initrd_load_addr<M: GuestMemory>(guest_mem: &M, initrd_size: u64) -> Result<u64> {
    let lowmem_size = guest_mem
        .find_region(GuestAddress(0))
        .ok_or(Error::InitrdAddress)
        .map(|r| r.len())?;

    // For safety to avoid overlap, reserve 32M for kernel and boot params in low end.
    if lowmem_size < initrd_size + (32 << 20) {
        return Err(Error::InitrdAddress);
    }

    let align_to_pagesize = |address| address & !(PAGE_SIZE as u64 - 1);
    Ok(align_to_pagesize(lowmem_size - initrd_size))
}

/// Returns the memory address where the kernel could be loaded.
pub fn get_kernel_start() -> u64 {
    layout::HIMEM_START
}

/// Add an e820 region to the e820 map.
/// Returns Ok(()) if successful, or an error if there is no space left in the map.
pub fn add_e820_entry(
    params: &mut bootparam::boot_params,
    addr: u64,
    size: u64,
    mem_type: u32,
) -> Result<()> {
    if params.e820_entries >= params.e820_table.len() as u8 {
        return Err(Error::E820Configuration);
    }

    params.e820_table[params.e820_entries as usize].addr = addr;
    params.e820_table[params.e820_entries as usize].size = size;
    params.e820_table[params.e820_entries as usize].type_ = mem_type;
    params.e820_entries += 1;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bootparam::{boot_e820_entry, boot_params};
    use crate::layout::{PDE_START, PDPTE_START, PML4_START};
    use kvm_bindings::kvm_sregs;
    use kvm_ioctls::Kvm;
    use vm_memory::GuestMemoryMmap;

    const BOOT_GDT_OFFSET: u64 = 0x500;
    const BOOT_IDT_OFFSET: u64 = 0x520;

    fn read_u64(gm: &GuestMemoryMmap, offset: u64) -> u64 {
        let read_addr = GuestAddress(offset);
        gm.read_obj(read_addr).unwrap()
    }

    #[test]
    fn test_get_descriptor_config_info() {
        let (gdt_table, gdt_addr, idt_addr) = get_descriptor_config_info();

        assert_eq!(gdt_table.len(), BOOT_GDT_MAX);
        assert_eq!(gdt_addr, BOOT_GDT_ADDRESS);
        assert_eq!(idt_addr, BOOT_IDT_ADDRESS);
    }

    #[test]
    fn test_setup_identity_mapping() {
        let gm = GuestMemoryMmap::from_ranges(&[(GuestAddress(0), 0x10000)]).unwrap();
        setup_identity_mapping(&gm).unwrap();
        assert_eq!(0xa003, read_u64(&gm, PML4_START));
        assert_eq!(0xb003, read_u64(&gm, PDPTE_START));
        for i in 0..512 {
            assert_eq!((i << 21) + 0x83u64, read_u64(&gm, PDE_START + (i * 8)));
        }
    }

    #[test]
    fn test_write_boot_param() {
        const KERNEL_BOOT_FLAG_MAGIC: u16 = 0xaa55;
        const KERNEL_HDR_MAGIC: u32 = 0x5372_6448;
        const KERNEL_LOADER_OTHER: u8 = 0xff;
        const KERNEL_MIN_ALIGNMENT_BYTES: u32 = 0x0100_0000; // Must be non-zero.
        let mut params: BootParamsWrapper = BootParamsWrapper(bootparam::boot_params::default());

        params.0.hdr.type_of_loader = KERNEL_LOADER_OTHER;
        params.0.hdr.boot_flag = KERNEL_BOOT_FLAG_MAGIC;
        params.0.hdr.header = KERNEL_HDR_MAGIC;
        params.0.hdr.kernel_alignment = KERNEL_MIN_ALIGNMENT_BYTES;

        assert_eq!(params.0.hdr.type_of_loader, KERNEL_LOADER_OTHER);
        assert_eq!(
            unsafe { std::ptr::addr_of!(params.0.hdr.boot_flag).read_unaligned() },
            KERNEL_BOOT_FLAG_MAGIC
        );
        assert_eq!(
            unsafe { std::ptr::addr_of!(params.0.hdr.header).read_unaligned() },
            KERNEL_HDR_MAGIC
        );
        assert_eq!(
            unsafe { std::ptr::addr_of!(params.0.hdr.kernel_alignment).read_unaligned() },
            KERNEL_MIN_ALIGNMENT_BYTES
        );
    }

    fn validate_page_tables(
        gm: &GuestMemoryMmap,
        sregs: &kvm_sregs,
        existing_pgtable: Option<GuestAddress>,
    ) {
        assert_eq!(0xa003, read_u64(gm, PML4_START));
        assert_eq!(0xb003, read_u64(gm, PDPTE_START));
        for i in 0..512 {
            assert_eq!((i << 21) + 0x83u64, read_u64(gm, PDE_START + (i * 8)));
        }

        if let Some(pgtable_base) = existing_pgtable {
            assert_eq!(pgtable_base.raw_value(), sregs.cr3);
        } else {
            assert_eq!(PML4_START, sregs.cr3);
        }
        assert!(sregs.cr4 & dbs_arch::regs::X86_CR4_PAE != 0);
        assert!(sregs.cr0 & dbs_arch::regs::X86_CR0_PG != 0);
    }

    fn create_guest_mem() -> GuestMemoryMmap {
        GuestMemoryMmap::from_ranges(&[(GuestAddress(0), 0x10000)]).unwrap()
    }

    #[test]
    fn test_setup_page_tables() {
        let kvm = Kvm::new().unwrap();
        let vm = kvm.create_vm().unwrap();
        let vcpu = vm.create_vcpu(0).unwrap();
        let gm = create_guest_mem();
        let gdt_table: [u64; layout::BOOT_GDT_MAX] = [
            gdt_entry(0, 0, 0),            // NULL
            gdt_entry(0xa09b, 0, 0xfffff), // CODE
            gdt_entry(0xc093, 0, 0xfffff), // DATA
            gdt_entry(0x808b, 0, 0xfffff), // TSS
        ];

        let page_address = setup_identity_mapping(&gm).unwrap();
        dbs_arch::regs::setup_sregs(
            &gm,
            &vcpu,
            page_address,
            &gdt_table,
            BOOT_GDT_OFFSET,
            BOOT_IDT_OFFSET,
        )
        .unwrap();
        let sregs: kvm_sregs = vcpu.get_sregs().unwrap();
        validate_page_tables(&gm, &sregs, Some(page_address));
    }

    #[test]
    fn test_add_e820_entry() {
        let e820_table = [(boot_e820_entry {
            addr: 0x1,
            size: 4,
            type_: 1,
        }); 128];

        let expected_params = boot_params {
            e820_table,
            e820_entries: 1,
            ..Default::default()
        };

        let mut params: boot_params = Default::default();
        add_e820_entry(
            &mut params,
            e820_table[0].addr,
            e820_table[0].size,
            e820_table[0].type_,
        )
        .unwrap();
        assert_eq!(
            format!("{:?}", params.e820_table[0]),
            format!("{:?}", expected_params.e820_table[0])
        );
        assert_eq!(params.e820_entries, expected_params.e820_entries);

        // Exercise the scenario where the field storing the length of the e820 entry table is
        // is bigger than the allocated memory.
        params.e820_entries = params.e820_table.len() as u8 + 1;
        assert!(add_e820_entry(
            &mut params,
            e820_table[0].addr,
            e820_table[0].size,
            e820_table[0].type_
        )
        .is_err());
    }
}
