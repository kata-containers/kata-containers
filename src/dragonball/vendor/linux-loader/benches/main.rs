// Copyright 2020 Amazon.com, Inc. or its affiliates. All Rights Reserved.
//
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE-BSD-3-Clause file.
//
// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause

extern crate criterion;
extern crate linux_loader;
extern crate vm_memory;

use criterion::{criterion_group, criterion_main, Criterion};

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
mod x86_64;
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
use x86_64::*;

#[cfg(target_arch = "aarch64")]
mod aarch64;
#[cfg(target_arch = "aarch64")]
use aarch64::*;

pub fn criterion_benchmark_nop(_: &mut Criterion) {}

criterion_group! {
    name = benches;
    config = Criterion::default().sample_size(500);
    targets = criterion_benchmark
}

#[cfg(all(any(target_arch = "x86", target_arch = "x86_64"), feature = "bzimage"))]
// Explicit (arch, feature) tuple required as clippy complains about
// `criterion_benchmark_bzimage` missing on aarch64.
criterion_group! {
    name = benches_bzimage;
    // Only ~125 runs fit in 5 seconds. Either extend the duration, or reduce
    // the number of iterations.
    config = Criterion::default().sample_size(100);
    targets = criterion_benchmark_bzimage
}

// NOP because the `criterion_main!` macro doesn't support cfg(feature)
// macro expansions.
#[cfg(any(target_arch = "aarch64", not(feature = "bzimage")))]
criterion_group! {
    name = benches_bzimage;
    // Sample size must be >= 10.
    // https://github.com/bheisler/criterion.rs/blob/0.3.0/src/lib.rs#L757
    config = Criterion::default().sample_size(10);
    targets = criterion_benchmark_nop
}

criterion_main! {
    benches,
    benches_bzimage
}
