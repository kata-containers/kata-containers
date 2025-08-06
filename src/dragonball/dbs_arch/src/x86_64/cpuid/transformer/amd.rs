// Copyright 2021 Alibaba Cloud. All Rights Reserved.
// Copyright 2019 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use kvm_bindings::KVM_CPUID_FLAG_SIGNIFCANT_INDEX;

use super::super::bit_helper::BitHelper;
use super::super::cpu_leaf;
use super::*;

// Largest extended function. It has to be larger than 0x8000001d (Extended Cache Topology).
const LARGEST_EXTENDED_FN: u32 = 0x8000_001f;
// This value allows at most 256 logical threads within a package. But we currently only support
// less than or equal to 254vcpus.
// See also the documentation for leaf_0x80000008::ecx::THREAD_ID_SIZE_BITRANGE
const THREAD_ID_MAX_SIZE: u32 = 8;
// This value means there is 1 node per processor.
// See also the documentation for leaf_0x8000001e::ecx::NODES_PER_PROCESSOR_BITRANGE.
const NODES_PER_PROCESSOR: u32 = 0;

fn update_structured_extended_entry(
    entry: &mut CpuIdEntry,
    _vm_spec: &VmSpec,
) -> Result<(), Error> {
    use cpu_leaf::leaf_0x7::index0::*;

    // according to the EPYC PPR, only the leaf 0x7 with index 0 contains the
    // structured extended feature identifiers
    if entry.index == 0 {
        // KVM sets this bit no matter what but this feature is not supported by hardware
        entry.edx.write_bit(edx::ARCH_CAPABILITIES_BITINDEX, false);
    }

    Ok(())
}

fn update_largest_extended_fn_entry(
    entry: &mut CpuIdEntry,
    _vm_spec: &VmSpec,
) -> Result<(), Error> {
    use cpu_leaf::leaf_0x80000000::*;

    // KVM sets the largest extended function to 0x80000000. Change it to 0x8000001f
    // Since we also use the leaf 0x8000001d (Extended Cache Topology).
    entry
        .eax
        .write_bits_in_range(&eax::LARGEST_EXTENDED_FN_BITRANGE, LARGEST_EXTENDED_FN);

    Ok(())
}

fn update_extended_feature_info_entry(
    entry: &mut CpuIdEntry,
    _vm_spec: &VmSpec,
) -> Result<(), Error> {
    use crate::cpuid::cpu_leaf::leaf_0x80000001::*;

    // set the Topology Extension bit since we use the Extended Cache Topology leaf
    entry.ecx.write_bit(ecx::TOPOEXT_INDEX, true);

    Ok(())
}

fn update_amd_features_entry(entry: &mut CpuIdEntry, vm_spec: &VmSpec) -> Result<(), Error> {
    use cpu_leaf::leaf_0x80000008::*;

    // We don't support more then 254 threads right now.
    entry
        .ecx
        .write_bits_in_range(&ecx::THREAD_ID_SIZE_BITRANGE, THREAD_ID_MAX_SIZE)
        .write_bits_in_range(&ecx::NUM_THREADS_BITRANGE, u32::from(vm_spec.cpu_count - 1));

    Ok(())
}

fn update_extended_cache_topology_entry(
    entry: &mut CpuIdEntry,
    vm_spec: &VmSpec,
) -> Result<(), Error> {
    entry.flags |= KVM_CPUID_FLAG_SIGNIFCANT_INDEX;

    common::update_cache_parameters_entry(entry, vm_spec)
}

fn update_extended_apic_id_entry(entry: &mut CpuIdEntry, vm_spec: &VmSpec) -> Result<(), Error> {
    use crate::cpuid::cpu_leaf::leaf_0x8000001e::*;

    let mut core_id = u32::from(vm_spec.cpu_id);
    // When hyper-threading is enabled each pair of 2 consecutive logical CPUs
    // will have the same core id since they represent 2 threads in the same core.
    // For Example:
    // logical CPU 0 -> core id: 0
    // logical CPU 1 -> core id: 0
    // logical CPU 2 -> core id: 1
    // logical CPU 3 -> core id: 1
    if vm_spec.threads_per_core == 2 {
        core_id /= 2;
    }

    entry
        .eax
        // the Extended APIC ID is the id of the current logical CPU
        .write_bits_in_range(&eax::EXTENDED_APIC_ID_BITRANGE, u32::from(vm_spec.cpu_id));

    entry
        .ebx
        .write_bits_in_range(&ebx::CORE_ID_BITRANGE, core_id)
        .write_bits_in_range(
            &ebx::THREADS_PER_CORE_BITRANGE,
            u32::from(vm_spec.threads_per_core - 1),
        );

    entry
        .ecx
        .write_bits_in_range(&ecx::NODES_PER_PROCESSOR_BITRANGE, NODES_PER_PROCESSOR)
        // Put all the cpus in the same node.
        .write_bits_in_range(&ecx::NODE_ID_BITRANGE, 0);

    Ok(())
}

#[derive(Default)]
pub struct AmdCpuidTransformer {}

impl AmdCpuidTransformer {
    pub fn new() -> Self {
        Default::default()
    }
}

impl CpuidTransformer for AmdCpuidTransformer {
    fn process_cpuid(&self, cpuid: &mut CpuId, vm_spec: &VmSpec) -> Result<(), Error> {
        use cpu_leaf::*;

        common::use_host_cpuid_function(cpuid, leaf_0x0::LEAF_NUM, false)?;
        common::use_host_cpuid_function(cpuid, leaf_0x8000001d::LEAF_NUM, false)?;
        common::use_host_cpuid_function(cpuid, leaf_0x8000001d::LEAF_NUM, true)?;
        self.process_entries(cpuid, vm_spec)
    }

    fn entry_transformer_fn(&self, entry: &mut CpuIdEntry) -> Option<EntryTransformerFn> {
        use cpu_leaf::*;

        match entry.function {
            leaf_0x1::LEAF_NUM => Some(common::update_feature_info_entry),
            leaf_0x7::LEAF_NUM => Some(update_structured_extended_entry),
            leaf_0xb::LEAF_NUM => Some(common::update_extended_topology_entry),
            leaf_0x1f::LEAF_NUM => Some(common::update_extended_topology_v2_entry),
            leaf_0x80000000::LEAF_NUM => Some(update_largest_extended_fn_entry),
            leaf_0x80000001::LEAF_NUM => Some(update_extended_feature_info_entry),
            leaf_0x80000008::LEAF_NUM => Some(update_amd_features_entry),
            leaf_0x8000001d::LEAF_NUM => Some(update_extended_cache_topology_entry),
            leaf_0x8000001e::LEAF_NUM => Some(update_extended_apic_id_entry),
            0x8000_0002..=0x8000_0004 => Some(common::update_brand_string_entry),
            _ => None,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_transformer_construct() {
        use cpu_leaf::leaf_0x7::index0::*;

        let transformer = AmdCpuidTransformer::new();

        let vm_spec =
            VmSpec::new(0, 1, 1, 1, 1, VpmuFeatureLevel::Disabled).expect("Error creating vm_spec");
        let mut cpuid = CpuId::from_entries(&[CpuIdEntry {
            function: cpu_leaf::leaf_0x7::LEAF_NUM,
            index: 0,
            flags: 0,
            eax: 0,
            ebx: 0,
            ecx: 0,
            edx: *(0_u32).write_bit(edx::ARCH_CAPABILITIES_BITINDEX, true),
            padding: [0, 0, 0],
        }])
        .unwrap();

        transformer.process_cpuid(&mut cpuid, &vm_spec).unwrap();
    }

    #[test]
    fn test_update_structured_extended_entry() {
        use cpu_leaf::leaf_0x7::index0::*;

        // Check that if index == 0 the entry is processed
        let vm_spec =
            VmSpec::new(0, 1, 1, 1, 1, VpmuFeatureLevel::Disabled).expect("Error creating vm_spec");
        let entry = &mut CpuIdEntry {
            function: cpu_leaf::leaf_0x7::LEAF_NUM,
            index: 0,
            flags: 0,
            eax: 0,
            ebx: 0,
            ecx: 0,
            edx: *(0_u32).write_bit(edx::ARCH_CAPABILITIES_BITINDEX, true),
            padding: [0, 0, 0],
        };
        assert!(update_structured_extended_entry(entry, &vm_spec).is_ok());
        assert!(!entry.edx.read_bit(edx::ARCH_CAPABILITIES_BITINDEX));

        // Check that if index != 0 the entry is not processed
        entry.index = 1;
        entry.edx.write_bit(edx::ARCH_CAPABILITIES_BITINDEX, true);
        assert!(update_structured_extended_entry(entry, &vm_spec).is_ok());
        assert!(entry.edx.read_bit(edx::ARCH_CAPABILITIES_BITINDEX));
    }

    #[test]
    fn test_update_largest_extended_fn_entry() {
        use crate::cpuid::cpu_leaf::leaf_0x80000000::*;

        let vm_spec =
            VmSpec::new(0, 1, 1, 1, 1, VpmuFeatureLevel::Disabled).expect("Error creating vm_spec");
        let entry = &mut CpuIdEntry {
            function: LEAF_NUM,
            index: 0,
            flags: 0,
            eax: 0,
            ebx: 0,
            ecx: 0,
            edx: 0,
            padding: [0, 0, 0],
        };

        assert!(update_largest_extended_fn_entry(entry, &vm_spec).is_ok());

        assert_eq!(
            entry
                .eax
                .read_bits_in_range(&eax::LARGEST_EXTENDED_FN_BITRANGE),
            LARGEST_EXTENDED_FN
        );
    }

    #[test]
    fn test_update_extended_feature_info_entry() {
        use crate::cpuid::cpu_leaf::leaf_0x80000001::*;

        let vm_spec =
            VmSpec::new(0, 1, 1, 1, 1, VpmuFeatureLevel::Disabled).expect("Error creating vm_spec");
        let entry = &mut CpuIdEntry {
            function: LEAF_NUM,
            index: 0,
            flags: 0,
            eax: 0,
            ebx: 0,
            ecx: 0,
            edx: 0,
            padding: [0, 0, 0],
        };

        assert!(update_extended_feature_info_entry(entry, &vm_spec).is_ok());

        assert!(entry.ecx.read_bit(ecx::TOPOEXT_INDEX));
    }

    fn check_update_amd_features_entry(
        cpu_count: u8,
        threads_per_core: u8,
        cores_per_die: u8,
        dies_per_socket: u8,
    ) {
        use crate::cpuid::cpu_leaf::leaf_0x80000008::*;

        let vm_spec = VmSpec::new(
            0,
            cpu_count,
            threads_per_core,
            cores_per_die,
            dies_per_socket,
            VpmuFeatureLevel::Disabled,
        )
        .expect("Error creating vm_spec");
        let entry = &mut CpuIdEntry {
            function: LEAF_NUM,
            index: 0,
            flags: 0,
            eax: 0,
            ebx: 0,
            ecx: 0,
            edx: 0,
            padding: [0, 0, 0],
        };

        assert!(update_amd_features_entry(entry, &vm_spec).is_ok());

        assert_eq!(
            entry.ecx.read_bits_in_range(&ecx::NUM_THREADS_BITRANGE),
            u32::from(cpu_count - 1)
        );
        assert_eq!(
            entry.ecx.read_bits_in_range(&ecx::THREAD_ID_SIZE_BITRANGE),
            THREAD_ID_MAX_SIZE
        );
    }

    fn check_update_extended_apic_id_entry(
        cpu_id: u8,
        cpu_count: u8,
        expected_core_id: u32,
        expected_threads_per_core: u32,
        threads_per_core: u8,
        cores_per_die: u8,
        dies_per_socket: u8,
    ) {
        use crate::cpuid::cpu_leaf::leaf_0x8000001e::*;

        let vm_spec = VmSpec::new(
            cpu_id,
            cpu_count,
            threads_per_core,
            cores_per_die,
            dies_per_socket,
            VpmuFeatureLevel::Disabled,
        )
        .expect("Error creating vm_spec");
        let entry = &mut CpuIdEntry {
            function: LEAF_NUM,
            index: 0,
            flags: 0,
            eax: 0,
            ebx: 0,
            ecx: 0,
            edx: 0,
            padding: [0, 0, 0],
        };

        assert!(update_extended_apic_id_entry(entry, &vm_spec).is_ok());

        assert_eq!(
            entry
                .eax
                .read_bits_in_range(&eax::EXTENDED_APIC_ID_BITRANGE),
            u32::from(cpu_id)
        );

        assert_eq!(
            entry.ebx.read_bits_in_range(&ebx::CORE_ID_BITRANGE),
            expected_core_id
        );
        assert_eq!(
            entry
                .ebx
                .read_bits_in_range(&ebx::THREADS_PER_CORE_BITRANGE),
            expected_threads_per_core
        );

        assert_eq!(
            entry
                .ecx
                .read_bits_in_range(&ecx::NODES_PER_PROCESSOR_BITRANGE),
            NODES_PER_PROCESSOR
        );
        assert_eq!(entry.ecx.read_bits_in_range(&ecx::NODE_ID_BITRANGE), 0);
    }

    #[test]
    fn test_update_extended_cache_topology_entry() {
        let vm_spec =
            VmSpec::new(0, 1, 1, 1, 1, VpmuFeatureLevel::Disabled).expect("Error creating vm_spec");
        let entry = &mut CpuIdEntry {
            function: cpu_leaf::leaf_0x8000001d::LEAF_NUM,
            index: 0,
            flags: 0,
            eax: 0,
            ebx: 0,
            ecx: 0,
            edx: 0,
            padding: [0, 0, 0],
        };

        assert!(update_extended_cache_topology_entry(entry, &vm_spec).is_ok());

        assert_eq!(entry.flags & KVM_CPUID_FLAG_SIGNIFCANT_INDEX, 1);
    }

    #[test]
    fn test_1vcpu_ht_off() {
        check_update_amd_features_entry(1, 1, 1, 1);

        check_update_extended_apic_id_entry(0, 1, 0, 0, 1, 1, 1);
    }

    #[test]
    fn test_1vcpu_ht_on() {
        check_update_amd_features_entry(1, 2, 1, 1);

        check_update_extended_apic_id_entry(0, 1, 0, 1, 2, 1, 1);
    }

    #[test]
    fn test_2vcpu_ht_off() {
        check_update_amd_features_entry(2, 1, 2, 1);

        check_update_extended_apic_id_entry(0, 2, 0, 0, 1, 2, 1);
        check_update_extended_apic_id_entry(1, 2, 1, 0, 1, 2, 1);
    }

    #[test]
    fn test_2vcpu_ht_on() {
        check_update_amd_features_entry(2, 2, 2, 1);

        check_update_extended_apic_id_entry(0, 2, 0, 1, 2, 2, 1);
        check_update_extended_apic_id_entry(1, 2, 0, 1, 2, 2, 1);
    }
}
