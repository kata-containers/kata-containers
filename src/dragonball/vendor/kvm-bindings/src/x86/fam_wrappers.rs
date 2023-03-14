// Copyright 2019 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use vmm_sys_util::fam::{FamStruct, FamStructWrapper};

use x86::bindings::*;

/// Maximum number of CPUID entries that can be returned by a call to KVM ioctls.
///
/// See arch/x86/include/asm/kvm_host.h
pub const KVM_MAX_CPUID_ENTRIES: usize = 80;

/// Maximum number of MSRs KVM supports (See arch/x86/kvm/x86.c).
pub const KVM_MAX_MSR_ENTRIES: usize = 256;

// Implement the FamStruct trait for kvm_cpuid2.
generate_fam_struct_impl!(
    kvm_cpuid2,
    kvm_cpuid_entry2,
    entries,
    u32,
    nent,
    KVM_MAX_CPUID_ENTRIES
);

/// Wrapper over the `kvm_cpuid2` structure.
///
/// The `kvm_cpuid2` structure contains a flexible array member. For details check the
/// [KVM API](https://www.kernel.org/doc/Documentation/virtual/kvm/api.txt)
/// documentation on `kvm_cpuid2`. To provide safe access to
/// the array elements, this type is implemented using
/// [FamStructWrapper](../vmm_sys_util/fam/struct.FamStructWrapper.html).
pub type CpuId = FamStructWrapper<kvm_cpuid2>;

// Implement the FamStruct trait for kvm_msrs.
generate_fam_struct_impl!(
    kvm_msrs,
    kvm_msr_entry,
    entries,
    u32,
    nmsrs,
    KVM_MAX_MSR_ENTRIES
);

/// Wrapper over the `kvm_msrs` structure.
///
/// The `kvm_msrs` structure contains a flexible array member. For details check the
/// [KVM API](https://www.kernel.org/doc/Documentation/virtual/kvm/api.txt)
/// documentation on `kvm_msrs`. To provide safe access to
/// the array elements, this type is implemented using
/// [FamStructWrapper](../vmm_sys_util/fam/struct.FamStructWrapper.html).
pub type Msrs = FamStructWrapper<kvm_msrs>;

// Implement the FamStruct trait for kvm_msr_list.
generate_fam_struct_impl!(kvm_msr_list, u32, indices, u32, nmsrs, KVM_MAX_MSR_ENTRIES);

/// Wrapper over the `kvm_msr_list` structure.
///
/// The `kvm_msr_list` structure contains a flexible array member. For details check the
/// [KVM API](https://www.kernel.org/doc/Documentation/virtual/kvm/api.txt)
/// documentation on `kvm_msr_list`. To provide safe access to
/// the array elements, this type is implemented using
/// [FamStructWrapper](../vmm_sys_util/fam/struct.FamStructWrapper.html).
pub type MsrList = FamStructWrapper<kvm_msr_list>;
