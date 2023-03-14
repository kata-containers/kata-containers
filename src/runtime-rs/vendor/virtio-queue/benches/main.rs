// Copyright 2020 Amazon.com, Inc. or its affiliates. All Rights Reserved.
//
// SPDX-License-Identifier: Apache-2.0 OR BSD-3-Clause

extern crate criterion;

mod queue;

use criterion::{criterion_group, criterion_main, Criterion};

use queue::benchmark_queue;

criterion_group! {
    name = benches;
    config = Criterion::default().sample_size(200).measurement_time(std::time::Duration::from_secs(20));
    targets = benchmark_queue
}

criterion_main! {
    benches,
}
