// Copyright 2020 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use vmm_sys_util::fam::{FamStruct, FamStructWrapper};

use arm64::bindings::*;

// There is no constant in the kernel as far as the maximum number
// of registers on arm, but KVM_GET_REG_LIST usually returns around 450.
const ARM64_REGS_MAX: usize = 500;

// Implement the FamStruct trait for kvm_reg_list.
generate_fam_struct_impl!(kvm_reg_list, u64, reg, u64, n, ARM64_REGS_MAX);

/// Wrapper over the `kvm_reg_list` structure.
///
/// The `kvm_reg_list` structure contains a flexible array member. For details check the
/// [KVM API](https://www.kernel.org/doc/Documentation/virtual/kvm/api.txt)
/// documentation on `kvm_reg_list`. To provide safe access to
/// the array elements, this type is implemented using
/// [FamStructWrapper](../vmm_sys_util/fam/struct.FamStructWrapper.html).
pub type RegList = FamStructWrapper<kvm_reg_list>;
