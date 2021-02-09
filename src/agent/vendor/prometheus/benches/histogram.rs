// Copyright 2016 PingCAP, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// See the License for the specific language governing permissions and
// limitations under the License.

#![feature(test)]

extern crate test;

use prometheus::{Histogram, HistogramOpts, HistogramVec};
use test::Bencher;

#[bench]
fn bench_histogram_with_label_values(b: &mut Bencher) {
    let histogram = HistogramVec::new(
        HistogramOpts::new("benchmark_histogram", "A histogram to benchmark it."),
        &["one", "two", "three"],
    )
    .unwrap();
    b.iter(|| {
        histogram
            .with_label_values(&["eins", "zwei", "drei"])
            .observe(3.1415)
    })
}

#[bench]
fn bench_histogram_no_labels(b: &mut Bencher) {
    let histogram = Histogram::with_opts(HistogramOpts::new(
        "benchmark_histogram",
        "A histogram to benchmark it.",
    ))
    .unwrap();
    b.iter(|| histogram.observe(3.1415))
}

#[bench]
fn bench_histogram_timer(b: &mut Bencher) {
    let histogram = Histogram::with_opts(HistogramOpts::new(
        "benchmark_histogram_timer",
        "A histogram to benchmark it.",
    ))
    .unwrap();
    b.iter(|| histogram.start_timer())
}

#[bench]
#[cfg(feature = "nightly")]
fn bench_histogram_coarse_timer(b: &mut Bencher) {
    let histogram = Histogram::with_opts(HistogramOpts::new(
        "benchmark_histogram_timer",
        "A histogram to benchmark it.",
    ))
    .unwrap();
    b.iter(|| histogram.start_coarse_timer())
}

#[bench]
fn bench_histogram_local(b: &mut Bencher) {
    let histogram = Histogram::with_opts(HistogramOpts::new(
        "benchmark_histogram_local",
        "A histogram to benchmark it.",
    ))
    .unwrap();
    let local = histogram.local();
    b.iter(|| local.observe(3.1415));
    local.flush();
}

#[bench]
fn bench_local_histogram_timer(b: &mut Bencher) {
    let histogram = Histogram::with_opts(HistogramOpts::new(
        "benchmark_histogram_local_timer",
        "A histogram to benchmark it.",
    ))
    .unwrap();
    let local = histogram.local();
    b.iter(|| local.start_timer());
    local.flush();
}

#[bench]
#[cfg(feature = "nightly")]
fn bench_local_histogram_coarse_timer(b: &mut Bencher) {
    let histogram = Histogram::with_opts(HistogramOpts::new(
        "benchmark_histogram_timer",
        "A histogram to benchmark it.",
    ))
    .unwrap();
    let local = histogram.local();
    b.iter(|| local.start_coarse_timer());
    local.flush();
}
