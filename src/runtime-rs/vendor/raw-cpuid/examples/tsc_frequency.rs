//! An example that determines the time stamp counter frequency (RDTSC, RDTSCP) .
extern crate raw_cpuid;

use std::time;

const MHZ_TO_HZ: u64 = 1000000;
const KHZ_TO_HZ: u64 = 1000;

#[cfg(target_arch = "x86_64")]
#[rustversion::nightly]
use core::arch::x86_64::_rdtsc as rdtsc;

#[cfg(target_arch = "x86")]
#[rustversion::nightly]
use core::arch::x86::_rdtsc as rdtsc;

#[rustversion::not(nightly)]
unsafe fn rdtsc() -> u64 {
    0
}

fn main() {
    let cpuid = raw_cpuid::CpuId::new();
    let has_tsc = cpuid
        .get_feature_info()
        .map_or(false, |finfo| finfo.has_tsc());

    let has_invariant_tsc = cpuid
        .get_advanced_power_mgmt_info()
        .map_or(false, |efinfo| efinfo.has_invariant_tsc());

    let tsc_frequency_hz = cpuid.get_tsc_info().map(|tinfo| {
        if tinfo.nominal_frequency() != 0 {
            tinfo.tsc_frequency()
        } else if tinfo.numerator() != 0 && tinfo.denominator() != 0 {
            // Skylake and Kabylake don't report the crystal clock, approximate with base frequency:
            cpuid
                .get_processor_frequency_info()
                .map(|pinfo| pinfo.processor_base_frequency() as u64 * MHZ_TO_HZ)
                .map(|cpu_base_freq_hz| {
                    let crystal_hz =
                        cpu_base_freq_hz * tinfo.denominator() as u64 / tinfo.numerator() as u64;
                    crystal_hz * tinfo.numerator() as u64 / tinfo.denominator() as u64
                })
        } else {
            None
        }
    });

    if has_tsc {
        // Try to figure out TSC frequency with CPUID
        println!(
            "TSC Frequency is: {} ({})",
            match tsc_frequency_hz {
                Some(x) => format!("{} Hz", x.unwrap_or(0)),
                None => String::from("unknown"),
            },
            if has_invariant_tsc {
                "invariant"
            } else {
                "TSC frequency varies with speed-stepping"
            }
        );

        // Check if we run in a VM and the hypervisor can give us the TSC frequency
        cpuid.get_hypervisor_info().map(|hv| {
            hv.tsc_frequency().map(|tsc_khz| {
                let virtual_tsc_frequency_hz = tsc_khz as u64 * KHZ_TO_HZ;
                println!(
                    "Hypervisor reports TSC Frequency at: {} Hz",
                    virtual_tsc_frequency_hz
                );
            })
        });

        // Determine TSC frequency by measuring it (loop for a second, record ticks)
        let one_second = time::Duration::from_secs(1);
        let now = time::Instant::now();
        let start = unsafe { rdtsc() };
        if start > 0 {
            loop {
                if now.elapsed() >= one_second {
                    break;
                }
            }
            let end = unsafe { rdtsc() };
            println!(
                "Empirical measurement of TSC frequency was: {} Hz",
                (end - start)
            );
        } else {
            // Don't have rdtsc on stable!
        }
    } else {
        println!("System does not have a TSC.")
    }
}
