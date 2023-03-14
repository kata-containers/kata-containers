// Copyright 2018 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Portions Copyright 2017 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the THIRD-PARTY file.
#![deny(missing_docs)]

//! A safe wrapper around the kernel's KVM interface.
//!
//! This crate offers safe wrappers for:
//! - [system ioctls](struct.Kvm.html) using the `Kvm` structure
//! - [VM ioctls](struct.VmFd.html) using the `VmFd` structure
//! - [vCPU ioctls](struct.VcpuFd.html) using the `VcpuFd` structure
//! - [device ioctls](struct.DeviceFd.html) using the `DeviceFd` structure
//!
//! # Platform support
//!
//! - x86_64
//! - arm64 (experimental)
//!
//! **NOTE:** The list of available ioctls is not extensive.
//!
//! # Example - Running a VM on x86_64
//!
//! In this example we are creating a Virtual Machine (VM) with one vCPU.
//! On the vCPU we are running machine specific code. This example is based on
//! the [LWN article](https://lwn.net/Articles/658511/) on using the KVM API.
//! The aarch64 example was modified accordingly.
//!
//! To get code running on the vCPU we are going through the following steps:
//!
//! 1. Instantiate KVM. This is used for running
//!    [system specific ioctls](struct.Kvm.html).
//! 2. Use the KVM object to create a VM. The VM is used for running
//!    [VM specific ioctls](struct.VmFd.html).
//! 3. Initialize the guest memory for the created VM. In this dummy example we
//!    are adding only one memory region and write the code in one memory page.
//! 4. Create a vCPU using the VM object. The vCPU is used for running
//!    [vCPU specific ioctls](struct.VcpuFd.html).
//! 5. Setup architectural specific general purpose registers and special registers. For
//!    details about how and why these registers are set, please check the
//!    [LWN article](https://lwn.net/Articles/658511/) on which this example is
//!    built.
//! 6. Run the vCPU code in a loop and check the
//!    [exit reasons](enum.VcpuExit.html).
//!
//!
//! ```rust
//! extern crate kvm_ioctls;
//! extern crate kvm_bindings;
//!
//! use kvm_ioctls::VcpuExit;
//! use kvm_ioctls::{Kvm, VcpuFd, VmFd};
//!
//! fn main() {
//!     use std::io::Write;
//!     use std::ptr::null_mut;
//!     use std::slice;
//!
//!     use kvm_bindings::kvm_userspace_memory_region;
//!     use kvm_bindings::KVM_MEM_LOG_DIRTY_PAGES;
//!
//!     let mem_size = 0x4000;
//!     let guest_addr = 0x1000;
//!     let asm_code: &[u8];
//!
//!     // Setting up architectural dependent values.
//!     #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
//!     {
//!         asm_code = &[
//!             0xba, 0xf8, 0x03, /* mov $0x3f8, %dx */
//!             0x00, 0xd8, /* add %bl, %al */
//!             0x04, b'0', /* add $'0', %al */
//!             0xee, /* out %al, %dx */
//!             0xec, /* in %dx, %al */
//!             0xc6, 0x06, 0x00, 0x80,
//!             0x00, /* movl $0, (0x8000); This generates a MMIO Write. */
//!             0x8a, 0x16, 0x00, 0x80, /* movl (0x8000), %dl; This generates a MMIO Read. */
//!             0xf4, /* hlt */
//!         ];
//!     }
//!     #[cfg(target_arch = "aarch64")]
//!     {
//!         asm_code = &[
//!             0x01, 0x00, 0x00, 0x10, /* adr x1, <this address> */
//!             0x22, 0x10, 0x00, 0xb9, /* str w2, [x1, #16]; write to this page */
//!             0x02, 0x00, 0x00, 0xb9, /* str w2, [x0]; This generates a MMIO Write. */
//!             0x00, 0x00, 0x00,
//!             0x14, /* b <this address>; shouldn't get here, but if so loop forever */
//!         ];
//!     }
//!
//!     // 1. Instantiate KVM.
//!     let kvm = Kvm::new().unwrap();
//!
//!     // 2. Create a VM.
//!     let vm = kvm.create_vm().unwrap();
//!
//!     // 3. Initialize Guest Memory.
//!     let load_addr: *mut u8 = unsafe {
//!         libc::mmap(
//!             null_mut(),
//!             mem_size,
//!             libc::PROT_READ | libc::PROT_WRITE,
//!             libc::MAP_ANONYMOUS | libc::MAP_SHARED | libc::MAP_NORESERVE,
//!             -1,
//!             0,
//!         ) as *mut u8
//!     };
//!
//!     let slot = 0;
//!     // When initializing the guest memory slot specify the
//!     // `KVM_MEM_LOG_DIRTY_PAGES` to enable the dirty log.
//!     let mem_region = kvm_userspace_memory_region {
//!         slot,
//!         guest_phys_addr: guest_addr,
//!         memory_size: mem_size as u64,
//!         userspace_addr: load_addr as u64,
//!         flags: KVM_MEM_LOG_DIRTY_PAGES,
//!     };
//!     unsafe { vm.set_user_memory_region(mem_region).unwrap() };
//!
//!     // Write the code in the guest memory. This will generate a dirty page.
//!     unsafe {
//!         let mut slice = slice::from_raw_parts_mut(load_addr, mem_size);
//!         slice.write(&asm_code).unwrap();
//!     }
//!
//!     // 4. Create one vCPU.
//!     let vcpu_fd = vm.create_vcpu(0).unwrap();
//!
//!     // 5. Initialize general purpose and special registers.
//!     #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
//!     {
//!         // x86_64 specific registry setup.
//!         let mut vcpu_sregs = vcpu_fd.get_sregs().unwrap();
//!         vcpu_sregs.cs.base = 0;
//!         vcpu_sregs.cs.selector = 0;
//!         vcpu_fd.set_sregs(&vcpu_sregs).unwrap();
//!
//!         let mut vcpu_regs = vcpu_fd.get_regs().unwrap();
//!         vcpu_regs.rip = guest_addr;
//!         vcpu_regs.rax = 2;
//!         vcpu_regs.rbx = 3;
//!         vcpu_regs.rflags = 2;
//!         vcpu_fd.set_regs(&vcpu_regs).unwrap();
//!     }
//!
//!     #[cfg(target_arch = "aarch64")]
//!     {
//!         // aarch64 specific registry setup.
//!         let mut kvi = kvm_bindings::kvm_vcpu_init::default();
//!         vm.get_preferred_target(&mut kvi).unwrap();
//!         vcpu_fd.vcpu_init(&kvi).unwrap();
//!
//!         let core_reg_base: u64 = 0x6030_0000_0010_0000;
//!         let mmio_addr: u64 = guest_addr + mem_size as u64;
//!         vcpu_fd.set_one_reg(core_reg_base + 2 * 32, guest_addr); // set PC
//!         vcpu_fd.set_one_reg(core_reg_base + 2 * 0, mmio_addr); // set X0
//!     }
//!
//!     // 6. Run code on the vCPU.
//!     loop {
//!         match vcpu_fd.run().expect("run failed") {
//!             VcpuExit::IoIn(addr, data) => {
//!                 println!(
//!                     "Received an I/O in exit. Address: {:#x}. Data: {:#x}",
//!                     addr, data[0],
//!                 );
//!             }
//!             VcpuExit::IoOut(addr, data) => {
//!                 println!(
//!                     "Received an I/O out exit. Address: {:#x}. Data: {:#x}",
//!                     addr, data[0],
//!                 );
//!             }
//!             VcpuExit::MmioRead(addr, data) => {
//!                 println!("Received an MMIO Read Request for the address {:#x}.", addr,);
//!             }
//!             VcpuExit::MmioWrite(addr, data) => {
//!                 println!("Received an MMIO Write Request to the address {:#x}.", addr,);
//!                 // The code snippet dirties 1 page when it is loaded in memory
//!                 let dirty_pages_bitmap = vm.get_dirty_log(slot, mem_size).unwrap();
//!                 let dirty_pages = dirty_pages_bitmap
//!                     .into_iter()
//!                     .map(|page| page.count_ones())
//!                     .fold(0, |dirty_page_count, i| dirty_page_count + i);
//!                 assert_eq!(dirty_pages, 1);
//!                 // Since on aarch64 there is not halt instruction,
//!                 // we break immediately after the last known instruction
//!                 // of the asm code example so that we avoid an infinite loop.
//!                 #[cfg(target_arch = "aarch64")]
//!                 break;
//!             }
//!             VcpuExit::Hlt => {
//!                 break;
//!             }
//!             r => panic!("Unexpected exit reason: {:?}", r),
//!         }
//!     }
//! }
//! ```

extern crate kvm_bindings;
extern crate libc;
#[macro_use]
extern crate vmm_sys_util;

#[macro_use]
mod kvm_ioctls;
mod cap;
mod ioctls;

pub use cap::Cap;
pub use ioctls::device::DeviceFd;
pub use ioctls::system::Kvm;
pub use ioctls::vcpu::{VcpuExit, VcpuFd};
pub use ioctls::vm::{IoEventAddress, NoDatamatch, VmFd};
// The following example is used to verify that our public
// structures are exported properly.
/// # Example
///
/// ```
/// #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
/// use kvm_ioctls::{Error, KvmRunWrapper};
/// ```
pub use ioctls::KvmRunWrapper;
pub use vmm_sys_util::errno::Error;
