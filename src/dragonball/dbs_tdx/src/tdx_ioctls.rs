// Copyright © 2019 Intel Corporation
//
// Copyright (c) 2023 Alibaba Cloud.
//
// SPDX-License-Identifier: Apache-2.0 OR BSD-3-Clause
//

use std::os::unix::io::RawFd;

use kvm_bindings::{CpuId, __IncompleteArrayField, KVMIO};
use thiserror::Error;
use vmm_sys_util::fam::{FamStruct, FamStructWrapper};
use vmm_sys_util::ioctl::ioctl_with_val;
use vmm_sys_util::{generate_fam_struct_impl, ioctl_ioc_nr, ioctl_iowr_nr};

/// Tdx capability list.
pub type TdxCaps = FamStructWrapper<TdxCapabilities>;

/// Cpuid configs entry counts.
const TDX1_MAX_NR_CPUID_CONFIGS: usize = 6;

generate_fam_struct_impl!(
    TdxCapabilities,
    TdxCpuidConfig,
    cpuid_configs,
    u32,
    nr_cpuid_configs,
    TDX1_MAX_NR_CPUID_CONFIGS
);

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, PartialEq)]
/// Tdx cpuid config.
pub struct TdxCpuidConfig {
    /// cpuid leaf
    pub leaf: u32,
    /// cpuid sub leaf
    pub sub_leaf: u32,
    /// eax
    pub eax: u32,
    /// ebx
    pub ebx: u32,
    /// ecx
    pub ecx: u32,
    /// edx
    pub edx: u32,
}

#[repr(C)]
#[derive(Default)]
/// Tdx capabilities.
pub struct TdxCapabilities {
    /// cpuid bits need to be fixed to 0.
    pub attrs_fixed0: u64,
    /// cpuid bits need to be fixed to 1.
    pub attrs_fixed1: u64,
    /// xfam bits need to be fixed to 0.
    pub xfam_fixed0: u64,
    /// xfam bits need to be fixed to 1.
    pub xfam_fixed1: u64,
    /// cpuid configs entry number.
    pub nr_cpuid_configs: u32,
    /// padding.
    pub padding: u32,
    /// cpuid config list
    pub cpuid_configs: __IncompleteArrayField<TdxCpuidConfig>,
}

ioctl_iowr_nr!(KVM_MEMORY_ENCRYPT_OP, KVMIO, 0xba, std::os::raw::c_ulong);
/// TDX module related errors.
#[derive(Error, Debug)]
pub enum TdxIoctlError {
    /// Failed to create TdxCaps
    #[error("Failed to create TdxCaps")]
    TdxCapabilitiesCreate,
    /// Failed to get TDX Capbilities
    #[error("Failed to get TDX Capbilities: {0}")]
    TdxCapabilities(#[source] std::io::Error),
    /// Failed to init TDX.
    #[error("Failed to init TDX: {0}")]
    TdxInit(#[source] std::io::Error),
    /// Failed to finalize TDX.
    #[error("Failed to finalize TDX: {0}")]
    TdxFinalize(#[source] std::io::Error),
    /// Failed to init TDX memory region.
    #[error("Failed to init TDX memory region: {0}")]
    TdxInitMemRegion(#[source] std::io::Error),
    /// Failed to init TDX vcpu.
    #[error("Failed to init TDX vcpu: {0}")]
    TdxInitVcpu(#[source] std::io::Error),
}

/// TDX related ioctl command
#[repr(u32)]
enum TdxCommand {
    /// Get Capability
    Capabilities = 0,
    /// Init TD
    InitVm = 1,
    /// Init vcpu for TD
    InitVcpu = 2,
    /// Init memory region for TD
    InitMemRegion = 3,
    /// Finalize TD
    Finalize = 4,
}

/// TDX related ioctl command
fn tdx_command(
    fd: &RawFd,
    command: TdxCommand,
    metadata: u32,
    data: u64,
) -> std::result::Result<(), std::io::Error> {
    #[repr(C)]
    struct TdxIoctlCmd {
        command: TdxCommand,
        metadata: u32,
        data: u64,
    }
    let cmd = TdxIoctlCmd {
        command,
        metadata,
        data,
    };
    let ret = unsafe {
        ioctl_with_val(
            fd,
            KVM_MEMORY_ENCRYPT_OP(),
            &cmd as *const TdxIoctlCmd as std::os::raw::c_ulong,
        )
    };
    if ret < 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

/// Init TDX
pub fn tdx_init(
    vm_fd: &RawFd,
    cpu_id: &CpuId,
    max_vcpus: u32,
) -> std::result::Result<(), TdxIoctlError> {
    #[repr(C)]
    struct TdxInitVm {
        max_vcpus: u32,
        tsc_khz: u32,
        attributes: u64,
        cpuid: u64,
        mrconfigid: [u64; 6],
        mrowner: [u64; 6],
        mrownerconfig: [u64; 6],
        reserved: [u64; 43],
    }
    let data = TdxInitVm {
        max_vcpus,
        tsc_khz: 0,
        attributes: 0, // TDX1_TD_ATTRIBUTE_DEBUG,
        cpuid: cpu_id.as_fam_struct_ptr() as u64,
        mrconfigid: [0; 6],
        mrowner: [0; 6],
        mrownerconfig: [0; 6],
        reserved: [0; 43],
    };
    tdx_command(vm_fd, TdxCommand::InitVm, 0, &data as *const _ as u64)
        .map_err(TdxIoctlError::TdxInit)
}

/// Finalize the TDX setup for this VM
pub fn tdx_finalize(vm_fd: &RawFd) -> std::result::Result<(), TdxIoctlError> {
    tdx_command(vm_fd, TdxCommand::Finalize, 0, 0).map_err(TdxIoctlError::TdxFinalize)
}

/// Initialize TDX memory Region
pub fn tdx_init_memory_region(
    vm_fd: &RawFd,
    host_address: u64,
    guest_address: u64,
    size: u64,
    measure: bool,
) -> std::result::Result<(), TdxIoctlError> {
    #[repr(C)]
    struct TdxInitMemRegion {
        host_address: u64,
        guest_address: u64,
        pages: u64,
    }
    let data = TdxInitMemRegion {
        host_address,
        guest_address,
        pages: size / 4096,
    };
    tdx_command(
        vm_fd,
        TdxCommand::InitMemRegion,
        if measure { 1 } else { 0 },
        &data as *const _ as u64,
    )
    .map_err(TdxIoctlError::TdxInitMemRegion)
}

/// Initialize TDX vcpu
pub fn tdx_init_vcpu(vcpu_fd: &RawFd, hob_address: u64) -> std::result::Result<(), TdxIoctlError> {
    tdx_command(vcpu_fd, TdxCommand::InitVcpu, 0, hob_address).map_err(TdxIoctlError::TdxInitVcpu)
}

/// Get tdx capabilities.
pub fn tdx_get_caps(kvm_fd: &RawFd) -> std::result::Result<TdxCaps, TdxIoctlError> {
    let mut tdx_caps = TdxCaps::new(TDX1_MAX_NR_CPUID_CONFIGS)
        .map_err(|_| TdxIoctlError::TdxCapabilitiesCreate)?;
    tdx_command(
        kvm_fd,
        TdxCommand::Capabilities,
        0,
        tdx_caps.as_mut_fam_struct_ptr() as *const _ as u64,
    )
    .map_err(TdxIoctlError::TdxCapabilities)?;
    Ok(tdx_caps)
}
