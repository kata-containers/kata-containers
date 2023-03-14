// Copyright 2020 Amazon.com, Inc. or its affiliates. All Rights Reserved.
//
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE-BSD-3-Clause file.
//
// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
extern crate criterion;
extern crate linux_loader;
extern crate vm_memory;

use linux_loader::configurator::fdt::FdtBootConfigurator;
use linux_loader::configurator::{BootConfigurator, BootParams};
use vm_memory::{ByteValued, GuestAddress, GuestMemoryMmap};

use criterion::{black_box, Criterion};

const MEM_SIZE: usize = 0x100_0000;
const FDT_MAX_SIZE: usize = 0x20;

fn create_guest_memory() -> GuestMemoryMmap {
    GuestMemoryMmap::from_ranges(&[(GuestAddress(0x0), MEM_SIZE)]).unwrap()
}

#[derive(Clone, Copy, Default)]
pub struct FdtPlaceholder([u8; FDT_MAX_SIZE]);

unsafe impl ByteValued for FdtPlaceholder {}

fn build_fdt_boot_params() -> BootParams {
    let fdt = FdtPlaceholder([0u8; FDT_MAX_SIZE]);
    let fdt_addr = GuestAddress((MEM_SIZE - FDT_MAX_SIZE - 1) as u64);
    BootParams::new::<FdtPlaceholder>(&fdt, fdt_addr)
}

pub fn criterion_benchmark(c: &mut Criterion) {
    let guest_mem = create_guest_memory();
    let fdt_boot_params = build_fdt_boot_params();
    c.bench_function("configure_fdt", |b| {
        b.iter(|| {
            black_box(FdtBootConfigurator::write_bootparams::<GuestMemoryMmap>(
                &fdt_boot_params,
                &guest_mem,
            ))
            .unwrap();
        })
    });
}
