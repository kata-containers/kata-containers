// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

/// Ioctl wrapper over guest memfd related KVM calls
use std::os::fd::RawFd;

use kvm_bindings::{__u32, __u64, KVMIO};
use vmm_sys_util::ioctl::ioctl_with_ref;
use vmm_sys_util::{ioctl_ioc_nr, ioctl_iow_nr, ioctl_iowr_nr};

ioctl_iowr_nr!(KVM_CREATE_GUEST_MEMFD, KVMIO, 0xd4, kvm_create_guest_memfd);
ioctl_iow_nr!(
    KVM_SET_USER_MEMORY_REGION2,
    KVMIO,
    0x49,
    kvm_userspace_memory_region2
);
ioctl_iow_nr!(
    KVM_SET_MEMORY_ATTRIBUTES,
    KVMIO,
    0xd2,
    kvm_memory_attributes
);

pub const KVM_MEM_GUEST_MEMFD: u32 = 1u32 << 2;

pub const KVM_MEMORY_ATTRIBUTE_PRIVATE: u64 = 1u64 << 3;

#[repr(C)]
#[derive(Debug, Default)]
#[allow(non_camel_case_types)]
struct kvm_create_guest_memfd {
    size: __u64,
    flags: __u64,
    reserved: [__u64; 6usize],
}

#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of kvm_create_guest_memfd"][::std::mem::size_of::<kvm_create_guest_memfd>() - 64usize];
    ["Alignment of kvm_create_guest_memfd"]
        [::std::mem::align_of::<kvm_create_guest_memfd>() - 8usize];
    ["Offset of field: kvm_create_guest_memfd::size"]
        [::std::mem::offset_of!(kvm_create_guest_memfd, size) - 0usize];
    ["Offset of field: kvm_create_guest_memfd::flags"]
        [::std::mem::offset_of!(kvm_create_guest_memfd, flags) - 8usize];
    ["Offset of field: kvm_create_guest_memfd::reserved"]
        [::std::mem::offset_of!(kvm_create_guest_memfd, reserved) - 16usize];
};

/// KVM_CREATE_GUEST_MEMFD
/// Create anonymous guest memfd bound to a specific VM. The memfd should NOT be mmap-ed by VMM,
/// but by calling KVM_SET_USER_MEMORY_REGION2 afterwards
///
/// # Arguments
///
/// * `vm_fd` - File descriptor to perform VM ioctl. The memfd created will be bound to this VM.
/// * `size` - Size of memfd that needs to be created
/// * `flags` - Currently no valid flag is supported by KVM. Should be 0.
pub fn kvm_create_guest_memfd(vm_fd: &RawFd, size: u64, flags: u64) -> Result<i32, std::io::Error> {
    let create_guest_memfd = kvm_create_guest_memfd {
        size,
        flags,
        ..Default::default()
    };
    let memfd = unsafe { ioctl_with_ref(vm_fd, KVM_CREATE_GUEST_MEMFD(), &create_guest_memfd) };
    if memfd < 0 {
        return Err(std::io::Error::last_os_error());
    }

    Ok(memfd)
}

#[repr(C)]
#[derive(Debug, Default)]
#[allow(non_camel_case_types)]
struct kvm_userspace_memory_region2 {
    slot: __u32,
    flags: __u32,
    guest_phys_addr: __u64,
    memory_size: __u64,
    userspace_addr: __u64,
    guest_memfd_offset: __u64,
    guest_memfd: __u32,
    pad1: __u32,
    pad2: [__u64; 14usize],
}

#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of kvm_userspace_memory_region2"]
        [::std::mem::size_of::<kvm_userspace_memory_region2>() - 160usize];
    ["Alignment of kvm_userspace_memory_region2"]
        [::std::mem::align_of::<kvm_userspace_memory_region2>() - 8usize];
    ["Offset of field: kvm_userspace_memory_region2::slot"]
        [::std::mem::offset_of!(kvm_userspace_memory_region2, slot) - 0usize];
    ["Offset of field: kvm_userspace_memory_region2::flags"]
        [::std::mem::offset_of!(kvm_userspace_memory_region2, flags) - 4usize];
    ["Offset of field: kvm_userspace_memory_region2::guest_phys_addr"]
        [::std::mem::offset_of!(kvm_userspace_memory_region2, guest_phys_addr) - 8usize];
    ["Offset of field: kvm_userspace_memory_region2::memory_size"]
        [::std::mem::offset_of!(kvm_userspace_memory_region2, memory_size) - 16usize];
    ["Offset of field: kvm_userspace_memory_region2::userspace_addr"]
        [::std::mem::offset_of!(kvm_userspace_memory_region2, userspace_addr) - 24usize];
    ["Offset of field: kvm_userspace_memory_region2::guest_memfd_offset"]
        [::std::mem::offset_of!(kvm_userspace_memory_region2, guest_memfd_offset) - 32usize];
    ["Offset of field: kvm_userspace_memory_region2::guest_memfd"]
        [::std::mem::offset_of!(kvm_userspace_memory_region2, guest_memfd) - 40usize];
    ["Offset of field: kvm_userspace_memory_region2::pad1"]
        [::std::mem::offset_of!(kvm_userspace_memory_region2, pad1) - 44usize];
    ["Offset of field: kvm_userspace_memory_region2::pad2"]
        [::std::mem::offset_of!(kvm_userspace_memory_region2, pad2) - 48usize];
};

/// KVM_SET_USER_MEMORY_REGION2
/// Create a guest physical memory slot, and mmap guest memfd into the guest
///
/// # Arguments
///
/// * `vm_fd` - File descriptor to perform VM ioctl, the target VM for the new memory slot
/// * `slot` - Memory slot ID
/// * `userspace_addr` - Start address of host memory that should map to target guest memory slot
/// * `guest_phys_addr` - Start address of guest physical memory that should be mapped
/// * `memory_size` - Size of guest physical memory to be mapped. In bytes
/// * `guest_memfd` - Guest memfd to map the guest memory slot. Must be created via KVM_CREATE_GUEST_MEMFD with the same vm_fd
/// * `guest_memfd_offset` - Offset from the start of memfd to begin mapping memory slot
/// * `flags` - Mapping to guest memfd will take effect only if KVM_MEM_GUEST_MEMFD is set
#[allow(clippy::too_many_arguments)]
pub fn kvm_set_user_memory_region2(
    vm_fd: &RawFd,
    slot: u32,
    userspace_addr: u64,
    guest_phys_addr: u64,
    memory_size: u64,
    guest_memfd: u32,
    guest_memfd_offset: u64,
    flags: u32,
) -> Result<(), std::io::Error> {
    let userspace_memory_region = kvm_userspace_memory_region2 {
        slot,
        flags,
        guest_phys_addr,
        memory_size,
        userspace_addr,
        guest_memfd_offset,
        guest_memfd,
        ..Default::default()
    };

    let ret = unsafe {
        ioctl_with_ref(
            vm_fd,
            KVM_SET_USER_MEMORY_REGION2(),
            &userspace_memory_region,
        )
    };

    if ret < 0 {
        return Err(std::io::Error::last_os_error());
    }

    Ok(())
}

#[repr(C)]
#[derive(Debug, Default)]
#[allow(non_camel_case_types)]
struct kvm_memory_attributes {
    address: __u64,
    size: __u64,
    attributes: __u64,
    flags: __u64,
}

#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of kvm_memory_attributes"][::std::mem::size_of::<kvm_memory_attributes>() - 32usize];
    ["Alignment of kvm_memory_attributes"]
        [::std::mem::align_of::<kvm_memory_attributes>() - 8usize];
    ["Offset of field: kvm_memory_attributes::address"]
        [::std::mem::offset_of!(kvm_memory_attributes, address) - 0usize];
    ["Offset of field: kvm_memory_attributes::size"]
        [::std::mem::offset_of!(kvm_memory_attributes, size) - 8usize];
    ["Offset of field: kvm_memory_attributes::attributes"]
        [::std::mem::offset_of!(kvm_memory_attributes, attributes) - 16usize];
    ["Offset of field: kvm_memory_attributes::flags"]
        [::std::mem::offset_of!(kvm_memory_attributes, flags) - 24usize];
};

/// KVM_SET_MEMORY_ATTRIBUTES
/// Set memory attributes for a range of guest physical memory
/// The ioctl will only take effect for ranges in memory slot created by KVM_SET_USER_MEMORY_REGION2
/// with KVM_MEM_GUEST_MEMFD flag set.
///
/// # Arguments
///
/// * `vm_fd` - File descriptor to perform VM ioctl
/// * `guest_address` - Start address of the range
/// * `size` - Size of the range in bytes
/// * `attributes` - New attributes for the range. Currently only KVM_MEMORY_ATTRIBUTE_PRIVATE is allowed
/// * `flags` - Currently no valid flag is supported. Should be 0.
pub fn kvm_set_memory_attributes(
    vm_fd: &RawFd,
    guest_address: u64,
    size: u64,
    attributes: u64,
    flags: u64,
) -> Result<(), std::io::Error> {
    let memory_attributes = kvm_memory_attributes {
        address: guest_address,
        size,
        attributes,
        flags,
    };
    let ret = unsafe { ioctl_with_ref(vm_fd, KVM_SET_MEMORY_ATTRIBUTES(), &memory_attributes) };

    if ret < 0 {
        return Err(std::io::Error::last_os_error());
    }

    Ok(())
}
