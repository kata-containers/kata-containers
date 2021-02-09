// Copyright 2018 PingCAP, Inc.
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

#[path = "../src/atomic64.rs"]
mod atomic64;

use crate::atomic64::*;
use test::Bencher;

#[bench]
fn bench_atomic_f64(b: &mut Bencher) {
    let val = AtomicF64::new(0.0);
    b.iter(|| {
        val.inc_by(12.0);
    });
}

#[bench]
fn bench_atomic_i64(b: &mut Bencher) {
    let val = AtomicI64::new(0);
    b.iter(|| {
        val.inc_by(12);
    });
}
