// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::os::fd::RawFd;

use kvm_bindings::KVMIO;
use kvm_ioctls::Cap;
use vmm_sys_util::ioctl::ioctl_with_val;
use vmm_sys_util::{ioctl_io_nr, ioctl_ioc_nr};

#[cfg(target_arch = "x86_64")]
pub mod tdx_ioctls;
pub use tdx_ioctls::*;

#[cfg(target_arch = "x86_64")]
pub mod td_shim;

pub const KVM_X86_TDX_VM: u64 = 5;

pub const KVM_CAP_VM_TYPES: u64 = 235;

pub const KVM_TDX_MEASURE_MEMORY_REGION: u32 = 1u32 << 0;

ioctl_io_nr!(KVM_CHECK_EXTENSION, KVMIO, 0x03);

pub fn is_tdx_supported(kvm_fd: &RawFd) -> bool {
    let supported_vm_types =
        unsafe { ioctl_with_val(kvm_fd, KVM_CHECK_EXTENSION(), KVM_CAP_VM_TYPES) } as u64;
    supported_vm_types & (1 << KVM_X86_TDX_VM) > 0
}

pub fn get_max_vcpus(vm_fd: &RawFd) -> usize {
    unsafe { ioctl_with_val(vm_fd, KVM_CHECK_EXTENSION(), Cap::MaxVcpus as u64) as usize }
}
