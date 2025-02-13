// Copyright 2021 Alibaba Cloud. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

//! VM boot related constants and utilities for `aarch64` architecture.

use vm_fdt::Error as VmFdtError;
use vm_memory::{Address, GuestAddress, GuestMemory, GuestMemoryError};

/// Magic addresses externally used to lay out aarch64 VMs.
pub mod layout;

/// FDT is used to inform the guest kernel of device tree information.
pub mod fdt;

/// Helper structs for constructing  fdt.
pub mod fdt_utils;

/// Default (smallest) memory page size for the supported architectures.
pub const PAGE_SIZE: usize = 4096;

/// Errors thrown while configuring the Flattened Device Tree for aarch64.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Failure in creating FDT
    #[error("create fdt fail: {0}")]
    CreateFdt(#[from] VmFdtError),
    /// Failure in writing FDT in memory.
    #[error("write fdt to memory fail: {0}")]
    WriteFDTToMemory(#[from] GuestMemoryError),
    /// Failed to compute the initrd address.
    #[error("Failed to compute the initrd address.")]
    InitrdAddress,
    /// Invalid arguments
    #[error("invalid arguments")]
    InvalidArguments,
}

/// Returns the memory address where the kernel could be loaded.
pub fn get_kernel_start() -> u64 {
    layout::DRAM_MEM_START
}

/// Auxiliary function to get the address where the device tree blob is loaded.
pub fn get_fdt_addr<M: GuestMemory>(mem: &M) -> u64 {
    // If the memory allocated is smaller than the size allocated for the FDT,
    // we return the start of the DRAM so that
    // we allow the code to try and load the FDT.
    if let Some(offset) = mem.last_addr().checked_sub(layout::FDT_MAX_SIZE as u64 - 1) {
        if mem.address_in_range(offset) {
            return offset.raw_value();
        }
    }
    layout::DRAM_MEM_START
}

/// Returns the memory address where the initrd could be loaded.
pub fn initrd_load_addr<M: GuestMemory>(guest_mem: &M, initrd_size: u64) -> super::Result<u64> {
    let round_to_pagesize = |size| (size + (PAGE_SIZE as u64 - 1)) & !(PAGE_SIZE as u64 - 1);
    match GuestAddress(get_fdt_addr(guest_mem)).checked_sub(round_to_pagesize(initrd_size)) {
        Some(offset) => {
            if guest_mem.address_in_range(offset) {
                Ok(offset.raw_value())
            } else {
                Err(Error::InitrdAddress)
            }
        }
        None => Err(Error::InitrdAddress),
    }
}

#[cfg(test)]
pub mod tests {
    use dbs_arch::{DeviceInfoForFDT, Error as ArchError};

    const LEN: u64 = 4096;

    #[derive(Clone, Debug, PartialEq)]
    pub struct MMIODeviceInfo {
        addr: u64,
        irq: u32,
    }

    impl MMIODeviceInfo {
        pub fn new(addr: u64, irq: u32) -> Self {
            MMIODeviceInfo { addr, irq }
        }
    }

    impl DeviceInfoForFDT for MMIODeviceInfo {
        fn addr(&self) -> u64 {
            self.addr
        }
        fn irq(&self) -> std::result::Result<u32, ArchError> {
            Ok(self.irq)
        }
        fn length(&self) -> u64 {
            LEN
        }
        fn get_device_id(&self) -> Option<u32> {
            None
        }
    }
}
