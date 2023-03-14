// Copyright 2020 Amazon.com, Inc. or its affiliates. All Rights Reserved.
//
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE-BSD-3-Clause file.
//
// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause

#![cfg(any(target_arch = "x86", target_arch = "x86_64"))]

extern crate linux_loader;
extern crate vm_memory;

use std::fs::File;
use std::io::{Cursor, Read};

use linux_loader::configurator::pvh::PvhBootConfigurator;
use linux_loader::configurator::{BootConfigurator, BootParams};
#[cfg(feature = "bzimage")]
use linux_loader::loader::bzimage::BzImage;
use linux_loader::loader::elf::start_info::{hvm_memmap_table_entry, hvm_start_info};
use linux_loader::loader::elf::Elf;
use linux_loader::loader::KernelLoader;
use vm_memory::{Address, GuestAddress, GuestMemoryMmap};

use criterion::{black_box, Criterion};

const MEM_SIZE: usize = 0x100_0000;
const E820_RAM: u32 = 1;
const XEN_HVM_START_MAGIC_VALUE: u32 = 0x336ec578;

fn create_guest_memory() -> GuestMemoryMmap {
    GuestMemoryMmap::from_ranges(&[(GuestAddress(0x0), MEM_SIZE)]).unwrap()
}

fn create_elf_pvh_image() -> Vec<u8> {
    include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/loader/x86_64/elf/test_elfnote.bin"
    ))
    .to_vec()
}

fn build_boot_params() -> (hvm_start_info, Vec<hvm_memmap_table_entry>) {
    let mut start_info = hvm_start_info::default();
    let memmap_entry = hvm_memmap_table_entry {
        addr: 0x7000,
        size: 0,
        type_: E820_RAM,
        reserved: 0,
    };
    start_info.magic = XEN_HVM_START_MAGIC_VALUE;
    start_info.version = 1;
    start_info.nr_modules = 0;
    start_info.memmap_entries = 0;
    (start_info, vec![memmap_entry])
}

fn build_pvh_boot_params() -> BootParams {
    let (mut start_info, memmap_entries) = build_boot_params();
    // Address in guest memory where the `start_info` struct will be written.
    let start_info_addr = GuestAddress(0x6000);
    // Address in guest memory where the memory map will be written.
    let memmap_addr = GuestAddress(0x7000);
    start_info.memmap_paddr = memmap_addr.raw_value();
    // Write boot parameters in guest memory.
    let mut boot_params = BootParams::new::<hvm_start_info>(&start_info, start_info_addr);
    boot_params.set_sections::<hvm_memmap_table_entry>(&memmap_entries, memmap_addr);
    boot_params
}

#[cfg(feature = "bzimage")]
fn download_resources() {
    use std::process::Command;

    let command = "./.buildkite/download_resources.sh";
    let status = Command::new(command).status().unwrap();
    if !status.success() {
        panic!("Cannot run build script");
    }
}

#[cfg(feature = "bzimage")]
fn create_bzimage() -> Vec<u8> {
    download_resources();
    let mut v = Vec::new();
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/loader/x86_64/bzimage/bzimage"
    );
    let mut f = File::open(path).unwrap();
    f.read_to_end(&mut v).unwrap();

    v
}

pub fn criterion_benchmark(c: &mut Criterion) {
    let guest_mem = create_guest_memory();

    let elf_pvh_image = create_elf_pvh_image();
    let pvh_boot_params = build_pvh_boot_params();

    c.bench_function("load_elf_pvh", |b| {
        b.iter(|| {
            black_box(Elf::load(
                &guest_mem,
                None,
                &mut Cursor::new(&elf_pvh_image),
                None,
            ))
            .unwrap();
        })
    });

    c.bench_function("configure_pvh", |b| {
        b.iter(|| {
            black_box(PvhBootConfigurator::write_bootparams::<GuestMemoryMmap>(
                &pvh_boot_params,
                &guest_mem,
            ))
            .unwrap();
        })
    });
}

#[cfg(feature = "bzimage")]
pub fn criterion_benchmark_bzimage(c: &mut Criterion) {
    let guest_mem = create_guest_memory();
    let bzimage = create_bzimage();

    c.bench_function("load_bzimage", |b| {
        b.iter(|| {
            black_box(BzImage::load(
                &guest_mem,
                None,
                &mut Cursor::new(&bzimage),
                None,
            ))
            .unwrap();
        })
    });
}
