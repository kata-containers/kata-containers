// Copyright (C) 2022 Alibaba Cloud. All rights reserved.
// Copyright 2018 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0
//
// Portions Copyright 2017 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the THIRD-PARTY file.
#![allow(dead_code)]
use kvm_bindings::KVM_API_VERSION;
use kvm_ioctls::{Cap, Kvm, VmFd};
use std::os::unix::io::{FromRawFd, RawFd};

use crate::error::{Error, Result};

/// Describes a KVM context that gets attached to the micro VM instance.
/// It gives access to the functionality of the KVM wrapper as long as every required
/// KVM capability is present on the host.
pub struct KvmContext {
    kvm: Kvm,
    max_memslots: usize,
    #[cfg(target_arch = "x86_64")]
    supported_msrs: kvm_bindings::MsrList,
}

impl KvmContext {
    /// Create a new KVM context object, using the provided `kvm_fd` if one is presented.
    pub fn new(kvm_fd: Option<RawFd>) -> Result<Self> {
        let kvm = if let Some(fd) = kvm_fd {
            // Safe because we expect kvm_fd to contain a valid fd number when is_some() == true.
            unsafe { Kvm::from_raw_fd(fd) }
        } else {
            Kvm::new().map_err(Error::Kvm)?
        };

        if kvm.get_api_version() != KVM_API_VERSION as i32 {
            return Err(Error::KvmApiVersion(kvm.get_api_version()));
        }

        Self::check_cap(&kvm, Cap::Irqchip)?;
        Self::check_cap(&kvm, Cap::Irqfd)?;
        Self::check_cap(&kvm, Cap::Ioeventfd)?;
        Self::check_cap(&kvm, Cap::UserMemory)?;
        #[cfg(target_arch = "x86_64")]
        Self::check_cap(&kvm, Cap::SetTssAddr)?;

        #[cfg(target_arch = "x86_64")]
        let supported_msrs = dbs_arch::msr::supported_guest_msrs(&kvm).map_err(Error::GuestMSRs)?;
        let max_memslots = kvm.get_nr_memslots();

        Ok(KvmContext {
            kvm,
            max_memslots,
            #[cfg(target_arch = "x86_64")]
            supported_msrs,
        })
    }

    /// Get underlying KVM object to access kvm-ioctls interfaces.
    pub fn kvm(&self) -> &Kvm {
        &self.kvm
    }

    /// Get the maximum number of memory slots reported by this KVM context.
    pub fn max_memslots(&self) -> usize {
        self.max_memslots
    }

    /// Create a virtual machine object.
    pub fn create_vm(&self) -> Result<VmFd> {
        self.kvm.create_vm().map_err(Error::Kvm)
    }

    /// Get the max vcpu count supported by kvm
    pub fn get_max_vcpus(&self) -> usize {
        self.kvm.get_max_vcpus()
    }

    fn check_cap(kvm: &Kvm, cap: Cap) -> std::result::Result<(), Error> {
        if !kvm.check_extension(cap) {
            return Err(Error::KvmCap(cap));
        }
        Ok(())
    }
}

#[cfg(target_arch = "x86_64")]
mod x86_64 {
    use super::*;
    use dbs_arch::msr::*;
    use kvm_bindings::{kvm_msr_entry, CpuId, MsrList, Msrs};
    use std::collections::HashSet;

    impl KvmContext {
        /// Get information about supported CPUID of x86 processor.
        pub fn supported_cpuid(
            &self,
            max_entries_count: usize,
        ) -> std::result::Result<CpuId, kvm_ioctls::Error> {
            self.kvm.get_supported_cpuid(max_entries_count)
        }

        /// Get information about supported MSRs of x86 processor.
        pub fn supported_msrs(
            &self,
            _max_entries_count: usize,
        ) -> std::result::Result<MsrList, kvm_ioctls::Error> {
            Ok(self.supported_msrs.clone())
        }

        // It's very sensible to manipulate MSRs, so please be careful to change code below.
        fn build_msrs_list(kvm: &Kvm) -> Result<Msrs> {
            let mut mset: HashSet<u32> = HashSet::new();
            let supported_msr_list = kvm.get_msr_index_list().map_err(super::Error::Kvm)?;
            for msr in supported_msr_list.as_slice() {
                mset.insert(*msr);
            }

            let mut msrs = vec![
                MSR_IA32_APICBASE,
                MSR_IA32_SYSENTER_CS,
                MSR_IA32_SYSENTER_ESP,
                MSR_IA32_SYSENTER_EIP,
                MSR_IA32_CR_PAT,
            ];

            let filters_list = vec![
                MSR_STAR,
                MSR_VM_HSAVE_PA,
                MSR_TSC_AUX,
                MSR_IA32_TSC_ADJUST,
                MSR_IA32_TSCDEADLINE,
                MSR_IA32_MISC_ENABLE,
                MSR_IA32_BNDCFGS,
                MSR_IA32_SPEC_CTRL,
            ];
            for msr in filters_list {
                if mset.contains(&msr) {
                    msrs.push(msr);
                }
            }

            // TODO: several msrs are optional.

            // TODO: Since our guests don't support nested-vmx, LMCE nor SGX for now.
            // msrs.push(MSR_IA32_FEATURE_CONTROL);

            msrs.push(MSR_CSTAR);
            msrs.push(MSR_KERNEL_GS_BASE);
            msrs.push(MSR_SYSCALL_MASK);
            msrs.push(MSR_LSTAR);
            msrs.push(MSR_IA32_TSC);

            msrs.push(MSR_KVM_SYSTEM_TIME_NEW);
            msrs.push(MSR_KVM_WALL_CLOCK_NEW);

            // FIXME: check if it's supported.
            msrs.push(MSR_KVM_ASYNC_PF_EN);
            msrs.push(MSR_KVM_PV_EOI_EN);
            msrs.push(MSR_KVM_STEAL_TIME);

            msrs.push(MSR_CORE_PERF_FIXED_CTR_CTRL);
            msrs.push(MSR_CORE_PERF_GLOBAL_CTRL);
            msrs.push(MSR_CORE_PERF_GLOBAL_STATUS);
            msrs.push(MSR_CORE_PERF_GLOBAL_OVF_CTRL);

            const MAX_FIXED_COUNTERS: u32 = 3;
            for i in 0..MAX_FIXED_COUNTERS {
                msrs.push(MSR_CORE_PERF_FIXED_CTR0 + i);
            }

            // FIXME: skip MCE for now.

            let mtrr_msrs = vec![
                MSR_MTRRdefType,
                MSR_MTRRfix64K_00000,
                MSR_MTRRfix16K_80000,
                MSR_MTRRfix16K_A0000,
                MSR_MTRRfix4K_C0000,
                MSR_MTRRfix4K_C8000,
                MSR_MTRRfix4K_D0000,
                MSR_MTRRfix4K_D8000,
                MSR_MTRRfix4K_E0000,
                MSR_MTRRfix4K_E8000,
                MSR_MTRRfix4K_F0000,
                MSR_MTRRfix4K_F8000,
            ];
            for mtrr in mtrr_msrs {
                msrs.push(mtrr);
            }

            const MSR_MTRRCAP_VCNT: u32 = 8;
            for i in 0..MSR_MTRRCAP_VCNT {
                msrs.push(0x200 + 2 * i);
                msrs.push(0x200 + 2 * i + 1);
            }

            let msrs: Vec<kvm_msr_entry> = msrs
                .iter()
                .map(|reg| kvm_msr_entry {
                    index: *reg,
                    reserved: 0,
                    data: 0,
                })
                .collect();

            Msrs::from_entries(&msrs).map_err(super::Error::Msr)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs::File;
    use std::os::unix::fs::MetadataExt;
    use std::os::unix::io::{AsRawFd, FromRawFd};

    use kvm_ioctls::Kvm;
    use test_utils::skip_if_not_root;

    use super::*;

    #[test]
    fn test_create_kvm_context() {
        skip_if_not_root!();

        let c = KvmContext::new(None).unwrap();

        assert!(c.max_memslots >= 32);

        let kvm = Kvm::new().unwrap();
        let f = std::mem::ManuallyDrop::new(unsafe { File::from_raw_fd(kvm.as_raw_fd()) });
        let m1 = f.metadata().unwrap();
        let m2 = File::open("/dev/kvm").unwrap().metadata().unwrap();

        assert_eq!(m1.dev(), m2.dev());
        assert_eq!(m1.ino(), m2.ino());
    }

    #[cfg(target_arch = "x86_64")]
    #[test]
    fn test_get_supported_cpu_id() {
        skip_if_not_root!();

        let c = KvmContext::new(None).unwrap();

        let _ = c
            .supported_cpuid(kvm_bindings::KVM_MAX_CPUID_ENTRIES)
            .expect("failed to get supported CPUID");
        assert!(c.supported_cpuid(0).is_err());
    }

    #[test]
    fn test_create_vm() {
        skip_if_not_root!();

        let c = KvmContext::new(None).unwrap();

        let _ = c.create_vm().unwrap();
    }
}
