// Copyright 2020 Amazon.com, Inc. or its affiliates. All Rights Reserved.
//
// SPDX-License-Identifier: Apache-2.0 OR BSD-3-Clause

extern crate criterion;

pub use criterion::{black_box, criterion_group, criterion_main, Criterion};
#[cfg(feature = "backend-mmap")]
use vm_memory::{GuestAddress, GuestMemoryMmap};

mod guest_memory;
mod mmap;
mod volatile;

use volatile::benchmark_for_volatile;

#[cfg(feature = "backend-mmap")]
// Use this function with caution. It does not check against overflows
// and `GuestMemoryMmap::from_ranges` errors.
fn create_guest_memory_mmap(size: usize, count: u64) -> GuestMemoryMmap<()> {
    let mut regions: Vec<(GuestAddress, usize)> = Vec::new();
    for i in 0..count {
        regions.push((GuestAddress(i * size as u64), size));
    }

    GuestMemoryMmap::from_ranges(regions.as_slice()).unwrap()
}

pub fn criterion_benchmark(_c: &mut Criterion) {
    #[cfg(feature = "backend-mmap")]
    mmap::benchmark_for_mmap(_c);
}

pub fn benchmark_guest_memory(_c: &mut Criterion) {
    #[cfg(feature = "backend-mmap")]
    guest_memory::benchmark_for_guest_memory(_c)
}

criterion_group! {
    name = benches;
    config = Criterion::default().sample_size(200).measurement_time(std::time::Duration::from_secs(50));
    targets = criterion_benchmark, benchmark_guest_memory, benchmark_for_volatile
}

criterion_main! {
    benches,
}
