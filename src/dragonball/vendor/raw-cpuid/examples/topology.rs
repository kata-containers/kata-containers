//! An example that uses CPUID to determine the system topology.
//!
//! Intel Topology is a pretty complicated subject (unfortunately):
//! https://software.intel.com/en-us/articles/intel-64-architecture-processor-topology-enumeration/
extern crate core_affinity;
extern crate raw_cpuid;

use raw_cpuid::{CpuId, ExtendedTopologyLevel, TopologyType};
use std::convert::TryInto;
use std::thread;

/// Runs CPU ID on every core in the system (to gather all APIC IDs).
///
/// # Note
/// This won't work on macOS, apparently there is no guarantee after setting the affinity
/// that a thread will really execute on the pinned core.
fn gather_all_xapic_ids() -> Vec<u8> {
    let core_ids = core_affinity::get_core_ids().unwrap();

    // Create a thread for each active CPU core.
    core_ids
        .into_iter()
        .map(|id| {
            thread::spawn(move || {
                // Pin this thread to a single CPU core.
                core_affinity::set_for_current(id);
                // Do more work after this.
                let cpuid = CpuId::new();
                cpuid
                    .get_feature_info()
                    .map_or_else(|| 0, |finfo| finfo.initial_local_apic_id())
            })
            .join()
            .unwrap_or(0)
        })
        .collect::<Vec<_>>()
}

/// Runs CPU ID on every core in the system (to gather all x2APIC IDs).
///
/// # Note
/// This won't work on macOS, apparently there is no guarantee after setting the affinity
/// that a thread will really execute on the pinned core.
fn gather_all_x2apic_ids() -> Vec<u32> {
    let core_ids = core_affinity::get_core_ids().unwrap();

    // Create a thread for each active CPU core.
    core_ids
        .into_iter()
        .map(|id| {
            thread::spawn(move || {
                // Pin this thread to a single CPU core.
                core_affinity::set_for_current(id);
                // Do more work after this.
                let cpuid = CpuId::new();
                cpuid.get_extended_topology_info().map_or_else(
                    || 0,
                    |mut topiter| topiter.next().as_ref().unwrap().x2apic_id(),
                )
            })
            .join()
            .unwrap_or(0)
        })
        .collect::<Vec<_>>()
}

fn enumerate_with_x2apic_ids() {
    let cpuid = CpuId::new();
    let mut smt_x2apic_shift: u32 = 0;
    let mut core_x2apic_shift: u32 = 0;

    cpuid.get_extended_topology_info().map_or_else(
        || println!("No topology information available."),
        |topoiter| {
            let topology: Vec<ExtendedTopologyLevel> = topoiter.collect();
            for topolevel in topology.iter() {
                match topolevel.level_type() {
                    TopologyType::SMT => {
                        smt_x2apic_shift = topolevel.shift_right_for_next_apic_id();
                    }
                    TopologyType::Core => {
                        core_x2apic_shift = topolevel.shift_right_for_next_apic_id();
                    }
                    _ => panic!("Topology category not supported."),
                };
            }
        },
    );

    println!("Enumeration of all cores in the system (with x2APIC IDs):");
    let mut all_x2apic_ids: Vec<u32> = gather_all_x2apic_ids();
    all_x2apic_ids.sort_unstable();
    for x2apic_id in all_x2apic_ids {
        let smt_select_mask = !(u32::max_value() << smt_x2apic_shift);
        let core_select_mask = (!((u32::max_value()) << core_x2apic_shift)) ^ smt_select_mask;
        let pkg_select_mask = u32::max_value() << core_x2apic_shift;

        let smt_id = x2apic_id & smt_select_mask;
        let core_id = (x2apic_id & core_select_mask) >> smt_x2apic_shift;
        let pkg_id = (x2apic_id & pkg_select_mask) >> core_x2apic_shift;

        println!(
            "x2APIC#{} (pkg: {}, core: {}, smt: {})",
            x2apic_id, pkg_id, core_id, smt_id
        );
    }
}

fn cpuid_bits_needed(count: u8) -> u8 {
    let mut mask: u8 = 0x80;
    let mut cnt: u8 = 8;

    while (cnt > 0) && ((mask & count) != mask) {
        mask >>= 1;
        cnt -= 1;
    }

    cnt
}

fn get_processor_limits() -> (u8, u8) {
    let cpuid = CpuId::new();

    // This is for AMD processors:
    if let Some(info) = cpuid.get_processor_capacity_feature_info() {
        let max_logical_processor_ids = info.num_phys_threads();
        let smt_max_cores_for_package = info.apic_id_size();

        return (
            max_logical_processor_ids.try_into().unwrap(),
            smt_max_cores_for_package.try_into().unwrap(),
        );
    }
    // This is for Intel processors:
    else if let Some(cparams) = cpuid.get_cache_parameters() {
        let max_logical_processor_ids = cpuid
            .get_feature_info()
            .map_or_else(|| 1, |finfo| finfo.max_logical_processor_ids());

        let mut smt_max_cores_for_package: u8 = 1;
        for (ecx, cache) in cparams.enumerate() {
            if ecx == 0 {
                smt_max_cores_for_package = cache.max_cores_for_package() as u8;
            }
        }

        return (
            max_logical_processor_ids as u8,
            smt_max_cores_for_package as u8,
        );
    }

    unreachable!("Example doesn't support this CPU")
}

fn enumerate_with_xapic_ids() {
    let (max_logical_processor_ids, smt_max_cores_for_package) = get_processor_limits();

    let smt_mask_width: u8 = cpuid_bits_needed(
        (max_logical_processor_ids.next_power_of_two() / smt_max_cores_for_package) - 1,
    );
    let smt_select_mask: u8 = !(u8::max_value() << smt_mask_width);
    let core_mask_width: u8 = cpuid_bits_needed(smt_max_cores_for_package - 1);
    let core_only_select_mask =
        (!(u8::max_value() << (core_mask_width + smt_mask_width))) ^ smt_select_mask;
    let pkg_select_mask = u8::max_value() << (core_mask_width + smt_mask_width);

    println!("Enumeration of all cores in the system (with APIC IDs):");
    let mut all_xapic_ids: Vec<u8> = gather_all_xapic_ids();
    all_xapic_ids.sort_unstable();

    for xapic_id in all_xapic_ids {
        let smt_id = xapic_id & smt_select_mask;
        let core_id = (xapic_id & core_only_select_mask) >> smt_mask_width;
        let pkg_id = (xapic_id & pkg_select_mask) >> (core_mask_width + smt_mask_width);

        println!(
            "APIC#{} (pkg: {}, core: {}, smt: {})",
            xapic_id, pkg_id, core_id, smt_id
        );
    }
}

fn main() {
    let cpuid = CpuId::new();

    cpuid.get_processor_brand_string().map_or_else(
        || println!("CPU model identifier not available."),
        |pbs| println!("CPU Model is: {}", pbs.as_str()),
    );
    cpuid.get_extended_topology_info().map_or_else(
        || println!("No topology information available."),
        |topoiter| {
            let mut topology: Vec<ExtendedTopologyLevel> = topoiter.collect();
            topology.reverse();

            for topolevel in topology.iter() {
                let typ = match topolevel.level_type() {
                    TopologyType::SMT => "SMT-threads",
                    TopologyType::Core => "cores",
                    _ => panic!("Topology category not supported."),
                };

                println!(
                    "At level {} the CPU has: {} {}",
                    topolevel.level_number(),
                    topolevel.processors(),
                    typ,
                );
            }
        },
    );

    println!();
    enumerate_with_xapic_ids();

    println!();
    enumerate_with_x2apic_ids();
}
