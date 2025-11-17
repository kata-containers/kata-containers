// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

#![allow(missing_docs)]
#![allow(non_camel_case_types)]

use super::*;

use kvm_bindings::{
    __u32, __u64, kvm_cpuid2, kvm_cpuid_entry2, CpuId, KVMIO, KVM_MAX_CPUID_ENTRIES,
};
use std::os::unix::io::RawFd;
use thiserror::Error;
use vmm_sys_util::ioctl::ioctl_with_ref;
use vmm_sys_util::{ioctl_ioc_nr, ioctl_iowr_nr};

ioctl_iowr_nr!(KVM_MEMORY_ENCRYPT_OP, KVMIO, 0xba, std::os::raw::c_ulong);

/// TDX module related errors.
#[derive(Error, Debug)]
pub enum TdxIoctlError {
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
    /// Failed to get TDX CPUID.
    #[error("Failed to get TDX CpuId: {0}")]
    TdxGetCpuid(#[source] std::io::Error),
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
    FinalizeVm = 4,
    /// Get CPUID for TD
    GetCpuid = 5,
}

#[repr(C)]
#[derive(Debug, Default)]
struct kvm_tdx_cmd {
    id: __u32,
    flags: __u32,
    data: __u64,
    hw_error: __u64,
}

#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of kvm_tdx_cmd"][::std::mem::size_of::<kvm_tdx_cmd>() - 24usize];
    ["Alignment of kvm_tdx_cmd"][::std::mem::align_of::<kvm_tdx_cmd>() - 8usize];
    ["Offset of field: kvm_tdx_cmd::id"][::std::mem::offset_of!(kvm_tdx_cmd, id) - 0usize];
    ["Offset of field: kvm_tdx_cmd::flags"][::std::mem::offset_of!(kvm_tdx_cmd, flags) - 4usize];
    ["Offset of field: kvm_tdx_cmd::data"][::std::mem::offset_of!(kvm_tdx_cmd, data) - 8usize];
    ["Offset of field: kvm_tdx_cmd::hw_error"]
        [::std::mem::offset_of!(kvm_tdx_cmd, hw_error) - 16usize];
};

#[repr(C)]
#[derive(Debug)]
struct kvm_tdx_capabilities {
    supported_attrs: __u64,
    supported_xfam: __u64,
    reserved: [__u64; 254usize],
    cpuid: kvm_cpuid2,
}

impl Default for kvm_tdx_capabilities {
    fn default() -> Self {
        Self {
            supported_attrs: 0__u64,
            supported_xfam: 0__u64,
            reserved: [0__u64; 254usize],
            cpuid: Default::default(),
        }
    }
}

const _: () = {
    ["Size of kvm_tdx_capabilities"][::std::mem::size_of::<kvm_tdx_capabilities>() - 2056usize];
    ["Alignment of kvm_tdx_capabilities"][::std::mem::align_of::<kvm_tdx_capabilities>() - 8usize];
    ["Offset of field: kvm_tdx_capabilities::supported_attrs"]
        [::std::mem::offset_of!(kvm_tdx_capabilities, supported_attrs) - 0usize];
    ["Offset of field: kvm_tdx_capabilities::supported_xfam"]
        [::std::mem::offset_of!(kvm_tdx_capabilities, supported_xfam) - 8usize];
    ["Offset of field: kvm_tdx_capabilities::reserved"]
        [::std::mem::offset_of!(kvm_tdx_capabilities, reserved) - 16usize];
    ["Offset of field: kvm_tdx_capabilities::cpuid"]
        [::std::mem::offset_of!(kvm_tdx_capabilities, cpuid) - 2048usize];
};

#[repr(C)]
#[derive(Debug, Default)]
struct kvm_tdx_init_vm {
    attributes: __u64,
    xfam: __u64,
    mrconfigid: [__u64; 6usize],
    mrowner: [__u64; 6usize],
    mrownerconfig: [__u64; 6usize],
    reserved: [__u64; 12usize],
    cpuid: kvm_cpuid2,
}

const _: () = {
    ["Size of kvm_tdx_init_vm"][::std::mem::size_of::<kvm_tdx_init_vm>() - 264usize];
    ["Alignment of kvm_tdx_init_vm"][::std::mem::align_of::<kvm_tdx_init_vm>() - 8usize];
    ["Offset of field: kvm_tdx_init_vm::attributes"]
        [::std::mem::offset_of!(kvm_tdx_init_vm, attributes) - 0usize];
    ["Offset of field: kvm_tdx_init_vm::xfam"]
        [::std::mem::offset_of!(kvm_tdx_init_vm, xfam) - 8usize];
    ["Offset of field: kvm_tdx_init_vm::mrconfigid"]
        [::std::mem::offset_of!(kvm_tdx_init_vm, mrconfigid) - 16usize];
    ["Offset of field: kvm_tdx_init_vm::mrowner"]
        [::std::mem::offset_of!(kvm_tdx_init_vm, mrowner) - 64usize];
    ["Offset of field: kvm_tdx_init_vm::mrownerconfig"]
        [::std::mem::offset_of!(kvm_tdx_init_vm, mrownerconfig) - 112usize];
    ["Offset of field: kvm_tdx_init_vm::reserved"]
        [::std::mem::offset_of!(kvm_tdx_init_vm, reserved) - 160usize];
    ["Offset of field: kvm_tdx_init_vm::cpuid"]
        [::std::mem::offset_of!(kvm_tdx_init_vm, cpuid) - 256usize];
};

#[repr(C)]
#[derive(Debug, Default)]
struct kvm_tdx_init_mem_region {
    source_addr: __u64,
    gpa: __u64,
    nr_pages: __u64,
}

const _: () = {
    ["Size of kvm_tdx_init_mem_region"][::std::mem::size_of::<kvm_tdx_init_mem_region>() - 24usize];
    ["Alignment of kvm_tdx_init_mem_region"]
        [::std::mem::align_of::<kvm_tdx_init_mem_region>() - 8usize];
    ["Offset of field: kvm_tdx_init_mem_region::source_addr"]
        [::std::mem::offset_of!(kvm_tdx_init_mem_region, source_addr) - 0usize];
    ["Offset of field: kvm_tdx_init_mem_region::gpa"]
        [::std::mem::offset_of!(kvm_tdx_init_mem_region, gpa) - 8usize];
    ["Offset of field: kvm_tdx_init_mem_region::nr_pages"]
        [::std::mem::offset_of!(kvm_tdx_init_mem_region, nr_pages) - 16usize];
};

/// TDX related ioctl command
fn tdx_command(
    fd: &RawFd,
    command: TdxCommand,
    flags: u32,
    data: u64,
    hw_error: u64,
) -> std::result::Result<(), std::io::Error> {
    let cmd = kvm_tdx_cmd {
        id: command as __u32,
        flags,
        data,
        hw_error,
    };
    let ret = unsafe { ioctl_with_ref(fd, KVM_MEMORY_ENCRYPT_OP(), &cmd) };

    if ret < 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

fn vec_with_fam_field<T: Default, F>(count: usize) -> Vec<T> {
    let size_required = count * std::mem::size_of::<F>() + std::mem::size_of::<T>();
    let rounded = size_required.div_ceil(std::mem::size_of::<T>());
    let mut v = Vec::with_capacity(rounded);
    v.resize_with(rounded, T::default);
    v
}

#[derive(Clone)]
pub struct TdxCapabilities {
    pub supported_attrs: u64,
    pub supported_xfam: u64,
    pub cpu_id: CpuId,
}

/// Get TDX capabilities
pub fn tdx_get_caps(vm_fd: &RawFd) -> std::result::Result<TdxCapabilities, TdxIoctlError> {
    let defaults = CpuId::new(KVM_MAX_CPUID_ENTRIES).map_err(|e| {
        TdxIoctlError::TdxCapabilities(std::io::Error::new(std::io::ErrorKind::OutOfMemory, e))
    })?;
    let mut caps =
        vec_with_fam_field::<kvm_tdx_capabilities, kvm_cpuid_entry2>(KVM_MAX_CPUID_ENTRIES);
    caps[0].cpuid.nent = KVM_MAX_CPUID_ENTRIES as __u32;
    caps[0].cpuid.padding = 0;
    unsafe {
        let cpuid_entries = caps[0].cpuid.entries.as_mut_slice(KVM_MAX_CPUID_ENTRIES);
        cpuid_entries.copy_from_slice(defaults.as_slice());
    }
    tdx_command(
        vm_fd,
        TdxCommand::Capabilities,
        0,
        &caps[0] as *const _ as u64,
        0,
    )
    .map_err(TdxIoctlError::TdxCapabilities)?;
    let mut cpu_id = unsafe {
        CpuId::from_entries(caps[0].cpuid.entries.as_slice(caps[0].cpuid.nent as usize)).map_err(
            |e| {
                TdxIoctlError::TdxCapabilities(std::io::Error::new(
                    std::io::ErrorKind::OutOfMemory,
                    e,
                ))
            },
        )?
    };
    cpu_id.as_mut_fam_struct().nent = caps[0].cpuid.nent;
    cpu_id.as_mut_fam_struct().padding = 0;
    Ok(TdxCapabilities {
        supported_attrs: caps[0].supported_attrs,
        supported_xfam: caps[0].supported_xfam,
        cpu_id,
    })
}

pub fn tdx_init(
    vm_fd: &RawFd,
    caps: &TdxCapabilities,
    cpu_id: &CpuId,
) -> Result<(), TdxIoctlError> {
    let cpu_id = cpu_id.as_slice();
    let mut init_vm =
        vec_with_fam_field::<kvm_tdx_init_vm, kvm_cpuid_entry2>(KVM_MAX_CPUID_ENTRIES);
    init_vm[0].attributes = caps.supported_attrs;
    init_vm[0].xfam = caps.supported_xfam;
    init_vm[0].cpuid.nent = KVM_MAX_CPUID_ENTRIES as __u32;
    init_vm[0].cpuid.padding = 0;
    unsafe {
        let cpuid_entries = init_vm[0].cpuid.entries.as_mut_slice(cpu_id.len());
        cpuid_entries.copy_from_slice(cpu_id);
    }
    tdx_command(
        vm_fd,
        TdxCommand::InitVm,
        0,
        &init_vm[0] as *const _ as u64,
        0,
    )
    .map_err(TdxIoctlError::TdxInit)?;

    Ok(())
}

pub fn tdx_init_vcpu(vcpu_fd: &RawFd, hob_address: u64) -> Result<(), TdxIoctlError> {
    tdx_command(vcpu_fd, TdxCommand::InitVcpu, 0, hob_address, 0)
        .map_err(TdxIoctlError::TdxInitVcpu)?;

    Ok(())
}

pub fn tdx_init_mem_region(
    vcpu_fd: &RawFd,
    source_addr: u64,
    gpa: u64,
    nr_pages: u64,
    flags: u32,
) -> Result<(), TdxIoctlError> {
    let init_mem_region = kvm_tdx_init_mem_region {
        source_addr,
        gpa,
        nr_pages,
    };
    tdx_command(
        vcpu_fd,
        TdxCommand::InitMemRegion,
        flags & KVM_TDX_MEASURE_MEMORY_REGION,
        &init_mem_region as *const _ as u64,
        0,
    )
    .map_err(TdxIoctlError::TdxInitMemRegion)?;

    Ok(())
}

pub fn tdx_finalize(vm_fd: &RawFd) -> Result<(), TdxIoctlError> {
    tdx_command(vm_fd, TdxCommand::FinalizeVm, 0, 0, 0).map_err(TdxIoctlError::TdxFinalize)?;

    Ok(())
}

pub fn tdx_get_cpuid(vcpu_fd: &RawFd) -> Result<CpuId, TdxIoctlError> {
    let cpu_id = CpuId::new(KVM_MAX_CPUID_ENTRIES).map_err(|e| {
        TdxIoctlError::TdxGetCpuid(std::io::Error::new(std::io::ErrorKind::OutOfMemory, e))
    })?;
    tdx_command(
        vcpu_fd,
        TdxCommand::GetCpuid,
        0,
        cpu_id.as_fam_struct_ptr() as u64,
        0,
    )
    .map_err(TdxIoctlError::TdxGetCpuid)?;

    Ok(cpu_id)
}
