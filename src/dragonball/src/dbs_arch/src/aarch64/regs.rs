// Copyright 2019 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0
//
// Portions Copyright 2017 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the THIRD-PARTY file.

//! Constants and utilities for aarch64 CPU generic, system and model specific registers.

use std::{mem, result};

use kvm_bindings::*;
use kvm_ioctls::VcpuFd;
use memoffset::offset_of;
use vmm_sys_util;

/// Errors thrown while setting aarch64 registers.
#[derive(Debug)]
pub enum Error {
    /// Failed to get core register (PC, PSTATE or general purpose ones).
    GetCoreRegister(kvm_ioctls::Error),
    /// Failed to set core register (PC, PSTATE or general purpose ones).
    SetCoreRegister(kvm_ioctls::Error),
    /// Failed to get a system register.
    GetSysRegister(kvm_ioctls::Error),
    /// Failed to get the register list.
    GetRegList(kvm_ioctls::Error),
    /// Failed to get a system register.
    SetRegister(kvm_ioctls::Error),
    /// Failed to init fam reglist
    FamRegister(vmm_sys_util::fam::Error),
}
type Result<T> = result::Result<T, Error>;

#[allow(non_upper_case_globals)]
// PSR (Processor State Register) bits.
// Taken from arch/arm64/include/uapi/asm/ptrace.h.
const PSR_MODE_EL1h: u64 = 0x0000_0005;
const PSR_F_BIT: u64 = 0x0000_0040;
const PSR_I_BIT: u64 = 0x0000_0080;
const PSR_A_BIT: u64 = 0x0000_0100;
const PSR_D_BIT: u64 = 0x0000_0200;
// Taken from arch/arm64/kvm/inject_fault.c.
const PSTATE_FAULT_BITS_64: u64 = PSR_MODE_EL1h | PSR_A_BIT | PSR_F_BIT | PSR_I_BIT | PSR_D_BIT;

// Following are macros that help with getting the ID of a aarch64 core register.
// The core register are represented by the user_pt_regs structure. Look for it in
// arch/arm64/include/uapi/asm/ptrace.h.

macro_rules! arm64_core_reg {
    ($reg: tt) => {
        // As per `kvm_arm_copy_reg_indices`, the id of a core register can be obtained like this:
        // `const u64 core_reg = KVM_REG_ARM64 | KVM_REG_SIZE_U64 | KVM_REG_ARM_CORE | i`, where i is obtained with:
        // `for (i = 0; i < sizeof(struct kvm_regs) / sizeof(__u32); i++) {`
        // We are using here `user_pt_regs` since this structure contains the core register and it is at
        // the start of `kvm_regs`.
        // struct kvm_regs {
        //	struct user_pt_regs regs;	/* sp = sp_el0 */
        //
        //	__u64	sp_el1;
        //	__u64	elr_el1;
        //
        //	__u64	spsr[KVM_NR_SPSR];
        //
        //	struct user_fpsimd_state fp_regs;
        //};
        // struct user_pt_regs {
        //	__u64		regs[31];
        //	__u64		sp;
        //	__u64		pc;
        //	__u64		pstate;
        //};
        // In our implementation we need: pc, pstate and user_pt_regs->regs[0].
        KVM_REG_ARM64 as u64
            | KVM_REG_SIZE_U64 as u64
            | u64::from(KVM_REG_ARM_CORE)
            | ((offset_of!(user_pt_regs, $reg) / mem::size_of::<u32>()) as u64)
    };
}

// This macro computes the ID of a specific ARM64 system register similar to how
// the kernel C macro does.
// https://elixir.bootlin.com/linux/v4.20.17/source/arch/arm64/include/uapi/asm/kvm.h#L203
macro_rules! arm64_sys_reg {
    ($name: tt, $op0: tt, $op1: tt, $crn: tt, $crm: tt, $op2: tt) => {
        const $name: u64 = KVM_REG_ARM64 as u64
            | KVM_REG_SIZE_U64 as u64
            | KVM_REG_ARM64_SYSREG as u64
            | ((($op0 as u64) << KVM_REG_ARM64_SYSREG_OP0_SHIFT)
                & KVM_REG_ARM64_SYSREG_OP0_MASK as u64)
            | ((($op1 as u64) << KVM_REG_ARM64_SYSREG_OP1_SHIFT)
                & KVM_REG_ARM64_SYSREG_OP1_MASK as u64)
            | ((($crn as u64) << KVM_REG_ARM64_SYSREG_CRN_SHIFT)
                & KVM_REG_ARM64_SYSREG_CRN_MASK as u64)
            | ((($crm as u64) << KVM_REG_ARM64_SYSREG_CRM_SHIFT)
                & KVM_REG_ARM64_SYSREG_CRM_MASK as u64)
            | ((($op2 as u64) << KVM_REG_ARM64_SYSREG_OP2_SHIFT)
                & KVM_REG_ARM64_SYSREG_OP2_MASK as u64);
    };
}

// Constant imported from the Linux kernel:
// https://elixir.bootlin.com/linux/v4.20.17/source/arch/arm64/include/asm/sysreg.h#L135
arm64_sys_reg!(MPIDR_EL1, 3, 0, 0, 0, 5);

/// Configure core registers for a given CPU.
///
/// # Arguments
///
/// * `vcpu` - Structure for the VCPU that holds the VCPU's fd.
/// * `cpu_id` - Index of current vcpu.
/// * `boot_ip` - Starting instruction pointer.
/// * `mem` - Reserved DRAM for current VM.
pub fn setup_regs(vcpu: &VcpuFd, cpu_id: u8, boot_ip: u64, fdt_address: u64) -> Result<()> {
    // Get the register index of the PSTATE (Processor State) register.
    vcpu.set_one_reg(arm64_core_reg!(pstate), PSTATE_FAULT_BITS_64 as u128)
        .map_err(Error::SetCoreRegister)?;

    // Other vCPUs are powered off initially awaiting PSCI wakeup.
    if cpu_id == 0 {
        // Setting the PC (Processor Counter) to the current program address (kernel address).
        vcpu.set_one_reg(arm64_core_reg!(pc), boot_ip as u128)
            .map_err(Error::SetCoreRegister)?;

        // Last mandatory thing to set -> the address pointing to the FDT (also called DTB).
        // "The device tree blob (dtb) must be placed on an 8-byte boundary and must
        // not exceed 2 megabytes in size." -> https://www.kernel.org/doc/Documentation/arm64/booting.txt.
        // We are choosing to place it the end of DRAM. See `get_fdt_addr`.
        vcpu.set_one_reg(arm64_core_reg!(regs), fdt_address as u128)
            .map_err(Error::SetCoreRegister)?;
    }
    Ok(())
}

/// Specifies whether a particular register is a system register or not.
/// The kernel splits the registers on aarch64 in core registers and system registers.
/// So, below we get the system registers by checking that they are not core registers.
///
/// # Arguments
///
/// * `regid` - The index of the register we are checking.
pub fn is_system_register(regid: u64) -> bool {
    if (regid & KVM_REG_ARM_COPROC_MASK as u64) == KVM_REG_ARM_CORE as u64 {
        return false;
    }

    let size = regid & KVM_REG_SIZE_MASK;
    if size != KVM_REG_SIZE_U32 && size != KVM_REG_SIZE_U64 {
        panic!("Unexpected register size for system register {}", size);
    }
    true
}

/// Read the MPIDR - Multiprocessor Affinity Register.
///
/// # Arguments
///
/// * `vcpu` - Structure for the VCPU that holds the VCPU's fd.
pub fn read_mpidr(vcpu: &VcpuFd) -> Result<u64> {
    vcpu.get_one_reg(MPIDR_EL1)
        .map(|value| value as u64)
        .map_err(Error::GetSysRegister)
}

#[cfg(test)]
mod tests {
    use super::*;
    use kvm_ioctls::Kvm;

    #[test]
    fn test_setup_regs() {
        let kvm = Kvm::new().unwrap();
        let vm = kvm.create_vm().unwrap();
        let vcpu = vm.create_vcpu(0).unwrap();
        match setup_regs(&vcpu, 0, 0x0, crate::gic::GIC_REG_END_ADDRESS).unwrap_err() {
            Error::SetCoreRegister(ref e) => assert_eq!(e.errno(), libc::ENOEXEC),
            _ => panic!("Expected to receive Error::SetCoreRegister"),
        }
        let mut kvi: kvm_bindings::kvm_vcpu_init = kvm_bindings::kvm_vcpu_init::default();
        vm.get_preferred_target(&mut kvi).unwrap();
        vcpu.vcpu_init(&kvi).unwrap();

        assert!(setup_regs(&vcpu, 0, 0x0, crate::gic::GIC_REG_END_ADDRESS).is_ok());
    }

    #[test]
    fn test_read_mpidr() {
        let kvm = Kvm::new().unwrap();
        let vm = kvm.create_vm().unwrap();
        let vcpu = vm.create_vcpu(0).unwrap();
        let mut kvi: kvm_bindings::kvm_vcpu_init = kvm_bindings::kvm_vcpu_init::default();
        vm.get_preferred_target(&mut kvi).unwrap();

        // Must fail when vcpu is not initialized yet.
        assert!(read_mpidr(&vcpu).is_err());

        vcpu.vcpu_init(&kvi).unwrap();
        assert_eq!(read_mpidr(&vcpu).unwrap(), 0x80000000);
    }
}
