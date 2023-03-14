// Copyright (C) 2020 Alibaba Cloud Computing. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0 OR BSD-3-Clause
#![cfg(feature = "backend-mmap")]

pub use criterion::{black_box, Criterion};

use vm_memory::bitmap::Bitmap;
use vm_memory::{GuestAddress, GuestMemory, GuestMemoryMmap};

const REGION_SIZE: usize = 0x10_0000;
const REGIONS_COUNT: u64 = 256;

pub fn benchmark_for_guest_memory(c: &mut Criterion) {
    benchmark_find_region(c);
}

fn find_region<B>(mem: &GuestMemoryMmap<B>)
where
    B: Bitmap + 'static,
{
    for i in 0..REGIONS_COUNT {
        let _ = mem
            .find_region(black_box(GuestAddress(i * REGION_SIZE as u64)))
            .unwrap();
    }
}

fn benchmark_find_region(c: &mut Criterion) {
    let memory = super::create_guest_memory_mmap(REGION_SIZE, REGIONS_COUNT);

    c.bench_function("find_region", |b| {
        b.iter(|| find_region(black_box(&memory)))
    });
}
