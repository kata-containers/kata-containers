// Copyright 2021 Alibaba Cloud. All Rights Reserved.
// Copyright 2019 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use super::super::bit_helper::BitHelper;
use super::super::cpu_leaf;
use super::*;

fn update_deterministic_cache_entry(entry: &mut CpuIdEntry, vm_spec: &VmSpec) -> Result<(), Error> {
    use cpu_leaf::leaf_0x4::*;

    common::update_cache_parameters_entry(entry, vm_spec)?;

    // If leaf_0xB or leaf_0x1F is enabled, leaf0x4 won't be used to generate topology information.
    // In most cases, we could have leaf_0xB in our host cpu. But we keep the leaf_0x4 eax[26,31]
    // to prevent rare cases.
    if vm_spec.cpu_count <= 64 {
        entry.eax.write_bits_in_range(
            &eax::MAX_CORES_PER_PACKAGE_BITRANGE,
            u32::from(vm_spec.cpu_count - 1),
        );
    }

    Ok(())
}

fn update_power_management_entry(entry: &mut CpuIdEntry, _vm_spec: &VmSpec) -> Result<(), Error> {
    // disable pstate feature
    entry.eax = 0;
    entry.ebx = 0;
    entry.ecx = 0;
    entry.edx = 0;

    Ok(())
}

fn update_perf_mon_entry(entry: &mut CpuIdEntry, vm_spec: &VmSpec) -> Result<(), Error> {
    use cpu_leaf::leaf_0xa::*;

    // Architectural Performance Monitor Leaf
    match vm_spec.vpmu_feature {
        VpmuFeatureLevel::Disabled => {
            // Disable PMU
            entry.eax = 0;
            entry.ebx = 0;
            entry.ecx = 0;
            entry.edx = 0;
        }
        VpmuFeatureLevel::LimitedlyEnabled => {
            // Allow minimal vpmu ability (only instuctions and cycles pmu).
            entry.eax.write_bits_in_range(&eax::PMC_VERSION_ID, 2);
            entry.eax.write_bits_in_range(&eax::BIT_LEN_PMEVENT, 7);

            // 0(false) means support for the targeted performance monitoring event
            entry.ebx.write_bit(ebx::CORE_CYCLES_BITINDEX, false);
            entry.ebx.write_bit(ebx::REF_CYCLES_BITINDEX, false);
            entry.ebx.write_bit(ebx::INST_RETIRED_BITINDEX, false);
            entry.ebx.write_bit(ebx::BR_INST_RETIRED_BITINDEX, true);
            entry.ebx.write_bit(ebx::LLC_MISSES_BITINDEX, true);
            entry.ebx.write_bit(ebx::LLC_REF_BITINDEX, true);
            entry.ebx.write_bit(ebx::BR_MIS_RETIRED_BITINDEX, true);
        }
        VpmuFeatureLevel::FullyEnabled => {
            // Allow all supported vpmu ability
            entry.eax.write_bits_in_range(&eax::PMC_VERSION_ID, 2);
            entry.eax.write_bits_in_range(&eax::BIT_LEN_PMEVENT, 7);

            // 0(false) means support for the targeted performance monitoring event
            entry.ebx.write_bit(ebx::CORE_CYCLES_BITINDEX, false);
            entry.ebx.write_bit(ebx::REF_CYCLES_BITINDEX, false);
            entry.ebx.write_bit(ebx::INST_RETIRED_BITINDEX, false);
            entry.ebx.write_bit(ebx::BR_INST_RETIRED_BITINDEX, false);
            entry.ebx.write_bit(ebx::LLC_MISSES_BITINDEX, false);
            entry.ebx.write_bit(ebx::LLC_REF_BITINDEX, false);
            entry.ebx.write_bit(ebx::BR_MIS_RETIRED_BITINDEX, false);
        }
    };
    Ok(())
}

#[derive(Default)]
pub struct IntelCpuidTransformer {}

impl IntelCpuidTransformer {
    pub fn new() -> Self {
        Default::default()
    }
}

impl CpuidTransformer for IntelCpuidTransformer {
    fn process_cpuid(&self, cpuid: &mut CpuId, vm_spec: &VmSpec) -> Result<(), Error> {
        common::use_host_cpuid_function(cpuid, cpu_leaf::leaf_0x0::LEAF_NUM, false)?;
        self.process_entries(cpuid, vm_spec)
    }

    fn entry_transformer_fn(&self, entry: &mut CpuIdEntry) -> Option<EntryTransformerFn> {
        use cpu_leaf::*;

        match entry.function {
            leaf_0x1::LEAF_NUM => Some(common::update_feature_info_entry),
            leaf_0x4::LEAF_NUM => Some(intel::update_deterministic_cache_entry),
            leaf_0x6::LEAF_NUM => Some(intel::update_power_management_entry),
            leaf_0xa::LEAF_NUM => Some(intel::update_perf_mon_entry),
            leaf_0xb::LEAF_NUM => Some(common::update_extended_topology_entry),
            leaf_0x1f::LEAF_NUM => Some(common::update_extended_topology_v2_entry),
            0x8000_0002..=0x8000_0004 => Some(common::update_brand_string_entry),
            _ => None,
        }
    }
}

#[cfg(test)]
mod test {
    use kvm_bindings::kvm_cpuid_entry2;

    use super::*;
    use crate::cpuid::transformer::VmSpec;

    #[test]
    fn test_update_perf_mon_entry() {
        use crate::cpuid::cpu_leaf::leaf_0xa::*;
        // Test when vpmu is off (level Disabled)
        let vm_spec =
            VmSpec::new(0, 1, 1, 1, 1, VpmuFeatureLevel::Disabled).expect("Error creating vm_spec");
        let entry = &mut kvm_cpuid_entry2 {
            function: LEAF_NUM,
            index: 0,
            flags: 0,
            eax: 1,
            ebx: 1,
            ecx: 1,
            edx: 1,
            padding: [0, 0, 0],
        };

        assert!(update_perf_mon_entry(entry, &vm_spec).is_ok());

        assert_eq!(entry.eax, 0);
        assert_eq!(entry.ebx, 0);
        assert_eq!(entry.ecx, 0);
        assert_eq!(entry.edx, 0);

        // Test when only instructions and cycles pmu are enabled (level LimitedlyEnabled)
        let vm_spec = VmSpec::new(0, 1, 1, 1, 1, VpmuFeatureLevel::LimitedlyEnabled)
            .expect("Error creating vm_spec");
        let entry = &mut kvm_cpuid_entry2 {
            function: 0x0,
            index: 0,
            flags: 0,
            eax: 0,
            ebx: 0,
            ecx: 0,
            edx: 0,
            padding: [0, 0, 0],
        };

        assert!(update_perf_mon_entry(entry, &vm_spec).is_ok());
        assert_eq!(entry.eax.read_bits_in_range(&eax::PMC_VERSION_ID), 2);
        assert_eq!(entry.eax.read_bits_in_range(&eax::BIT_LEN_PMEVENT), 7);

        assert!(!entry.ebx.read_bit(ebx::CORE_CYCLES_BITINDEX));
        assert!(!entry.ebx.read_bit(ebx::INST_RETIRED_BITINDEX));
        assert!(!entry.ebx.read_bit(ebx::REF_CYCLES_BITINDEX));
        assert!(entry.ebx.read_bit(ebx::LLC_REF_BITINDEX));
        assert!(entry.ebx.read_bit(ebx::LLC_MISSES_BITINDEX));
        assert!(entry.ebx.read_bit(ebx::BR_INST_RETIRED_BITINDEX));
        assert!(entry.ebx.read_bit(ebx::BR_MIS_RETIRED_BITINDEX));

        // Test when all vpmu features are enabled (level FullyEnabled)
        let vm_spec = VmSpec::new(0, 1, 1, 1, 1, VpmuFeatureLevel::FullyEnabled)
            .expect("Error creating vm_spec");
        let entry = &mut kvm_cpuid_entry2 {
            function: 0x0,
            index: 0,
            flags: 0,
            eax: 0,
            ebx: 0,
            ecx: 0,
            edx: 0,
            padding: [0, 0, 0],
        };

        assert!(update_perf_mon_entry(entry, &vm_spec).is_ok());

        assert_eq!(entry.eax.read_bits_in_range(&eax::PMC_VERSION_ID), 2);
        assert_eq!(entry.eax.read_bits_in_range(&eax::BIT_LEN_PMEVENT), 7);

        assert!(!entry.ebx.read_bit(ebx::CORE_CYCLES_BITINDEX));
        assert!(!entry.ebx.read_bit(ebx::INST_RETIRED_BITINDEX));
        assert!(!entry.ebx.read_bit(ebx::REF_CYCLES_BITINDEX));
        assert!(!entry.ebx.read_bit(ebx::LLC_REF_BITINDEX));
        assert!(!entry.ebx.read_bit(ebx::LLC_MISSES_BITINDEX));
        assert!(!entry.ebx.read_bit(ebx::BR_INST_RETIRED_BITINDEX));
        assert!(!entry.ebx.read_bit(ebx::BR_MIS_RETIRED_BITINDEX));
    }

    fn check_update_deterministic_cache_entry(
        cpu_count: u8,
        cache_level: u32,
        expected_max_cores_per_package: u32,
        threads_per_core: u8,
        cores_per_die: u8,
        dies_per_socket: u8,
    ) {
        use crate::cpuid::cpu_leaf::leaf_0x4::*;

        let vm_spec = VmSpec::new(
            0,
            cpu_count,
            threads_per_core,
            cores_per_die,
            dies_per_socket,
            VpmuFeatureLevel::Disabled,
        )
        .expect("Error creating vm_spec");
        let entry = &mut kvm_cpuid_entry2 {
            function: 0x0,
            index: 0,
            flags: 0,
            eax: *(0_u32).write_bits_in_range(&eax::CACHE_LEVEL_BITRANGE, cache_level),
            ebx: 0,
            ecx: 0,
            edx: 0,
            padding: [0, 0, 0],
        };

        assert!(update_deterministic_cache_entry(entry, &vm_spec).is_ok());

        assert!(
            entry
                .eax
                .read_bits_in_range(&eax::MAX_CORES_PER_PACKAGE_BITRANGE)
                == expected_max_cores_per_package
        );
    }

    #[test]
    fn test_1vcpu_ht_off() {
        // test update_deterministic_cache_entry
        // test L1
        check_update_deterministic_cache_entry(1, 1, 0, 1, 1, 1);
        // test L2
        check_update_deterministic_cache_entry(1, 2, 0, 1, 1, 1);
        // test L3
        check_update_deterministic_cache_entry(1, 3, 0, 1, 1, 1);
    }

    #[test]
    fn test_1vcpu_ht_on() {
        // test update_deterministic_cache_entry
        // test L1
        check_update_deterministic_cache_entry(1, 1, 0, 2, 1, 1);
        // test L2
        check_update_deterministic_cache_entry(1, 2, 0, 2, 1, 1);
        // test L3
        check_update_deterministic_cache_entry(1, 3, 0, 2, 1, 1);
    }

    #[test]
    fn test_2vcpu_ht_off() {
        // test update_deterministic_cache_entry
        // test L1
        check_update_deterministic_cache_entry(2, 1, 1, 1, 2, 1);
        // test L2
        check_update_deterministic_cache_entry(2, 2, 1, 1, 2, 1);
        // test L3
        check_update_deterministic_cache_entry(2, 3, 1, 1, 2, 1);
    }

    #[test]
    fn test_2vcpu_ht_on() {
        // test update_deterministic_cache_entry
        // test L1
        check_update_deterministic_cache_entry(2, 1, 1, 2, 2, 1);
        // test L2
        check_update_deterministic_cache_entry(2, 2, 1, 2, 2, 1);
        // test L3
        check_update_deterministic_cache_entry(2, 3, 1, 2, 2, 1);
    }
}
