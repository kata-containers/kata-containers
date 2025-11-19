// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//
#![allow(non_camel_case_types)]
#![allow(missing_docs)]

use std::os::fd::RawFd;

use kvm_bindings::KVMIO;
use kvm_ioctls::Cap;
use thiserror::Error;
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

/// TDX related error
#[derive(Error, Debug)]
pub enum TdxError {
    /// TDX ioctl command failure
    #[error("Failed to run TDX ioctl command: {0}")]
    TdxIoctlError(#[source] TdxIoctlError),

    /// TDVF related error
    #[error("TDVF Error: {0}")]
    TdvfError(#[source] td_shim::TdvfError),

    /// TDX VM is not supported
    #[error("TDX VM is not supported")]
    TdxVmNotSupported,

    /// Out of memory
    #[error("Failed to allocate memory: {0}")]
    OutOfMemory(#[source] std::io::Error),
}

/// Do prechecks for capabilities before creating a TDX VM
pub fn tdx_precheck_pre_create_vm(kvm_fd: &RawFd) -> Result<(), TdxError> {
    let supported_vm_types =
        unsafe { ioctl_with_val(kvm_fd, KVM_CHECK_EXTENSION(), KVM_CAP_VM_TYPES) } as u64;
    if supported_vm_types & (1 << KVM_X86_TDX_VM) == 0 {
        return Err(TdxError::TdxVmNotSupported);
    }

    Ok(())
}

/// Do prechecks for capabilities after creating a TDX VM
/// This should be done before issuing any other TDX ioctl functions
pub fn tdx_precheck_post_create_vm(_kvm_fd: &RawFd) -> Result<(), TdxError> {

    //
    // TODO: Add capability precheck for KVM_CAP_SPLIT_IRQCHIP, 
    // KVM_CAP_USER_MEMORY2, KVM_CAP_MEMORY_ATTRIBUTES and KVM_CAP_GUEST_MEMFD
    //

    Ok(())
}

/// Get max vCPU number allowed for a TDX VM
/// Please use VM fd for querying, since the result might be different from querying with KVM fd
pub fn tdx_get_max_vcpus(vm_fd: &RawFd) -> usize {
    unsafe { ioctl_with_val(vm_fd, KVM_CHECK_EXTENSION(), Cap::MaxVcpus as u64) as usize }
}
