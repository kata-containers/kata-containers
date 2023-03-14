//! Example that displays information about the caches.
extern crate raw_cpuid;
use raw_cpuid::{CacheType, CpuId};

fn main() {
    let cpuid = CpuId::new();
    cpuid.get_cache_parameters().map_or_else(
        || println!("No cache parameter information available"),
        |cparams| {
            for cache in cparams {
                let size = cache.associativity()
                    * cache.physical_line_partitions()
                    * cache.coherency_line_size()
                    * cache.sets();

                let typ = match cache.cache_type() {
                    CacheType::Data => "Instruction-Cache",
                    CacheType::Instruction => "Data-Cache",
                    CacheType::Unified => "Unified-Cache",
                    _ => "Unknown cache type",
                };

                let associativity = if cache.is_fully_associative() {
                    "fully associative".to_string()
                } else {
                    format!("{}-way associativity", cache.associativity())
                };

                let size_repr = if size > 1024 * 1024 {
                    format!("{} MiB", size / (1024 * 1024))
                } else {
                    format!("{} KiB", size / 1024)
                };

                let mapping = if cache.has_complex_indexing() {
                    "hash-based-mapping"
                } else {
                    "direct-mapped"
                };

                println!(
                    "L{} {}: ({}, {}, {})",
                    cache.level(),
                    typ,
                    size_repr,
                    associativity,
                    mapping
                );
            }
        },
    );
}
