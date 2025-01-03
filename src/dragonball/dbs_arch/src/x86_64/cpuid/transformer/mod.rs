// Copyright 2021 Alibaba Cloud. All Rights Reserved.
// Copyright 2019 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use super::brand_string::{BrandString, Reg as BsReg};
use super::common::get_vendor_id;
use super::{CpuId, CpuIdEntry};
use crate::VpmuFeatureLevel;

pub mod amd;
pub mod common;
pub mod intel;

/// Structure containing the specifications of the VM
pub struct VmSpec {
    /// The vendor id of the CPU
    cpu_vendor_id: [u8; 12],
    /// The id of the current logical cpu in the range [0..cpu_count].
    cpu_id: u8,
    /// The total number of logical cpus (includes cpus that could be hotplugged).
    cpu_count: u8,
    /// The desired brand string for the guest.
    brand_string: BrandString,
    /// threads per core for cpu topology information
    threads_per_core: u8,
    /// cores per die for cpu topology information
    cores_per_die: u8,
    /// dies per socket for cpu topology information
    dies_per_socket: u8,
    /// if vpmu feature is Disabled, it means vpmu feature is off (by default)
    /// if vpmu feature is LimitedlyEnabled, it means minimal vpmu counters are supported (cycles and instructions)
    /// if vpmu feature is FullyEnabled, it means all vpmu counters are supported
    vpmu_feature: VpmuFeatureLevel,
}

impl VmSpec {
    /// Creates a new instance of VmSpec with the specified parameters
    /// The brand string is deduced from the vendor_id
    pub fn new(
        cpu_id: u8,
        cpu_count: u8,
        threads_per_core: u8,
        cores_per_die: u8,
        dies_per_socket: u8,
        vpmu_feature: VpmuFeatureLevel,
    ) -> Result<VmSpec, Error> {
        let cpu_vendor_id = get_vendor_id().map_err(Error::InternalError)?;
        let brand_string =
            BrandString::from_vendor_id(&cpu_vendor_id).map_err(Error::BrandString)?;

        Ok(VmSpec {
            cpu_vendor_id,
            cpu_id,
            cpu_count,
            brand_string,
            threads_per_core,
            cores_per_die,
            dies_per_socket,
            vpmu_feature,
        })
    }

    /// Returns an immutable reference to cpu_vendor_id
    pub fn cpu_vendor_id(&self) -> &[u8; 12] {
        &self.cpu_vendor_id
    }
}

/// Errors associated with processing the CPUID leaves.
#[derive(Debug, Clone)]
pub enum Error {
    /// Failed to parse CPU brand string
    BrandString(super::brand_string::Error),
    /// The CPU architecture is not supported
    CpuNotSupported,
    /// A FamStructWrapper operation has failed
    FamError(vmm_sys_util::fam::Error),
    /// A call to an internal helper method failed
    InternalError(super::common::Error),
    /// The maximum number of addressable logical CPUs cannot be stored in an `u8`.
    VcpuCountOverflow,
}

pub type EntryTransformerFn = fn(entry: &mut CpuIdEntry, vm_spec: &VmSpec) -> Result<(), Error>;

/// Generic trait that provides methods for transforming the cpuid
pub trait CpuidTransformer {
    /// Process the cpuid array and make the desired transformations.
    fn process_cpuid(&self, cpuid: &mut CpuId, vm_spec: &VmSpec) -> Result<(), Error> {
        self.process_entries(cpuid, vm_spec)
    }

    /// Iterate through all the cpuid entries and calls the associated transformer for each one.
    fn process_entries(&self, cpuid: &mut CpuId, vm_spec: &VmSpec) -> Result<(), Error> {
        for entry in cpuid.as_mut_slice().iter_mut() {
            let maybe_transformer_fn = self.entry_transformer_fn(entry);

            if let Some(transformer_fn) = maybe_transformer_fn {
                transformer_fn(entry, vm_spec)?;
            }
        }

        Ok(())
    }

    /// Get the associated transformer for a cpuid entry
    fn entry_transformer_fn(&self, _entry: &mut CpuIdEntry) -> Option<EntryTransformerFn> {
        None
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use kvm_bindings::kvm_cpuid_entry2;

    const PROCESSED_FN: u32 = 1;
    const EXPECTED_INDEX: u32 = 100;

    fn transform_entry(entry: &mut kvm_cpuid_entry2, _vm_spec: &VmSpec) -> Result<(), Error> {
        entry.index = EXPECTED_INDEX;

        Ok(())
    }

    struct MockCpuidTransformer {}

    impl CpuidTransformer for MockCpuidTransformer {
        fn entry_transformer_fn(&self, entry: &mut kvm_cpuid_entry2) -> Option<EntryTransformerFn> {
            match entry.function {
                PROCESSED_FN => Some(transform_entry),
                _ => None,
            }
        }
    }

    #[test]
    fn test_process_cpuid() {
        let num_entries = 5;

        let mut cpuid = CpuId::new(num_entries).unwrap();
        let vm_spec = VmSpec::new(0, 1, 1, 1, 1, VpmuFeatureLevel::Disabled);
        cpuid.as_mut_slice()[0].function = PROCESSED_FN;
        assert!(MockCpuidTransformer {}
            .process_cpuid(&mut cpuid, &vm_spec.unwrap())
            .is_ok());

        assert!(cpuid.as_mut_slice().len() == num_entries);
        for entry in cpuid.as_mut_slice().iter() {
            match entry.function {
                PROCESSED_FN => {
                    assert_eq!(entry.index, EXPECTED_INDEX);
                }
                _ => {
                    assert_ne!(entry.index, EXPECTED_INDEX);
                }
            }
        }
    }

    #[test]
    fn test_invalid_cpu_architecture_cpuid() {
        use crate::cpuid::process_cpuid;
        let num_entries = 5;

        let mut cpuid = CpuId::new(num_entries).unwrap();
        let mut vm_spec = VmSpec::new(0, 1, 1, 1, 1, VpmuFeatureLevel::Disabled).unwrap();

        vm_spec.cpu_vendor_id = [1; 12];
        assert!(process_cpuid(&mut cpuid, &vm_spec).is_err());
    }
}
