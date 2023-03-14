// Copyright 2020 Amazon.com, Inc. or its affiliates. All Rights Reserved.
//
// SPDX-License-Identifier: Apache-2.0 OR BSD-3-Clause
#![cfg(feature = "backend-mmap")]

extern crate criterion;
extern crate vm_memory;

use std::fs::{File, OpenOptions};
use std::io::Cursor;
use std::mem::size_of;
use std::path::Path;

use criterion::{black_box, Criterion};

use vm_memory::{ByteValued, Bytes, GuestAddress, GuestMemory};

const REGION_SIZE: usize = 0x8000_0000;
const REGIONS_COUNT: u64 = 8;
const ACCESS_SIZE: usize = 0x200;

#[repr(C)]
#[derive(Copy, Clone, Default)]
struct SmallDummy {
    a: u32,
    b: u32,
}
unsafe impl ByteValued for SmallDummy {}

#[repr(C)]
#[derive(Copy, Clone, Default)]
struct BigDummy {
    elements: [u64; 12],
}

unsafe impl ByteValued for BigDummy {}

fn make_image(size: usize) -> Vec<u8> {
    let mut image: Vec<u8> = Vec::with_capacity(size);
    for i in 0..size {
        // We just want some different numbers here, so the conversion is OK.
        image.push(i as u8);
    }
    image
}

enum AccessKind {
    // The parameter represents the index of the region where the access should happen.
    // Indices are 0-based.
    InRegion(u64),
    // The parameter represents the index of the first region (i.e. where the access starts).
    CrossRegion(u64),
}

impl AccessKind {
    fn make_offset(&self, access_size: usize) -> u64 {
        match *self {
            AccessKind::InRegion(idx) => REGION_SIZE as u64 * idx,
            AccessKind::CrossRegion(idx) => {
                REGION_SIZE as u64 * (idx + 1) - (access_size as u64 / 2)
            }
        }
    }
}

pub fn benchmark_for_mmap(c: &mut Criterion) {
    let memory = super::create_guest_memory_mmap(REGION_SIZE, REGIONS_COUNT);

    // Just a sanity check.
    assert_eq!(
        memory.last_addr(),
        GuestAddress(REGION_SIZE as u64 * REGIONS_COUNT - 0x01)
    );

    let some_small_dummy = SmallDummy {
        a: 0x1111_2222,
        b: 0x3333_4444,
    };

    let some_big_dummy = BigDummy {
        elements: [0x1111_2222_3333_4444; 12],
    };

    let mut image = make_image(ACCESS_SIZE);
    let buf = &mut [0u8; ACCESS_SIZE];
    let mut file = File::open(Path::new("/dev/zero")).expect("Could not open /dev/zero");
    let mut file_to_write = OpenOptions::new()
        .write(true)
        .open("/dev/null")
        .expect("Could not open /dev/null");

    let accesses = &[
        AccessKind::InRegion(0),
        AccessKind::CrossRegion(0),
        AccessKind::CrossRegion(REGIONS_COUNT - 2),
        AccessKind::InRegion(REGIONS_COUNT - 1),
    ];

    for access in accesses {
        let offset = access.make_offset(ACCESS_SIZE);
        let address = GuestAddress(offset);

        // Check performance for read operations.
        c.bench_function(format!("read_from_{:#0X}", offset).as_str(), |b| {
            b.iter(|| {
                black_box(&memory)
                    .read_from(address, &mut Cursor::new(&image), ACCESS_SIZE)
                    .unwrap()
            })
        });

        c.bench_function(format!("read_from_file_{:#0X}", offset).as_str(), |b| {
            b.iter(|| {
                black_box(&memory)
                    .read_from(address, &mut file, ACCESS_SIZE)
                    .unwrap()
            })
        });

        c.bench_function(format!("read_exact_from_{:#0X}", offset).as_str(), |b| {
            b.iter(|| {
                black_box(&memory)
                    .read_exact_from(address, &mut Cursor::new(&mut image), ACCESS_SIZE)
                    .unwrap()
            })
        });

        c.bench_function(
            format!("read_entire_slice_from_{:#0X}", offset).as_str(),
            |b| b.iter(|| black_box(&memory).read_slice(buf, address).unwrap()),
        );

        c.bench_function(format!("read_slice_from_{:#0X}", offset).as_str(), |b| {
            b.iter(|| black_box(&memory).read(buf, address).unwrap())
        });

        let obj_off = access.make_offset(size_of::<SmallDummy>());
        let obj_addr = GuestAddress(obj_off);

        c.bench_function(
            format!("read_small_obj_from_{:#0X}", obj_off).as_str(),
            |b| b.iter(|| black_box(&memory).read_obj::<SmallDummy>(obj_addr).unwrap()),
        );

        let obj_off = access.make_offset(size_of::<BigDummy>());
        let obj_addr = GuestAddress(obj_off);

        c.bench_function(format!("read_big_obj_from_{:#0X}", obj_off).as_str(), |b| {
            b.iter(|| black_box(&memory).read_obj::<BigDummy>(obj_addr).unwrap())
        });

        // Check performance for write operations.
        c.bench_function(format!("write_to_{:#0X}", offset).as_str(), |b| {
            b.iter(|| {
                black_box(&memory)
                    .write_to(address, &mut Cursor::new(&mut image), ACCESS_SIZE)
                    .unwrap()
            })
        });

        c.bench_function(format!("write_to_file_{:#0X}", offset).as_str(), |b| {
            b.iter(|| {
                black_box(&memory)
                    .write_to(address, &mut file_to_write, ACCESS_SIZE)
                    .unwrap()
            })
        });

        c.bench_function(format!("write_exact_to_{:#0X}", offset).as_str(), |b| {
            b.iter(|| {
                black_box(&memory)
                    .write_all_to(address, &mut Cursor::new(&mut image), ACCESS_SIZE)
                    .unwrap()
            })
        });

        c.bench_function(
            format!("write_entire_slice_to_{:#0X}", offset).as_str(),
            |b| b.iter(|| black_box(&memory).write_slice(buf, address).unwrap()),
        );

        c.bench_function(format!("write_slice_to_{:#0X}", offset).as_str(), |b| {
            b.iter(|| black_box(&memory).write(buf, address).unwrap())
        });

        let obj_off = access.make_offset(size_of::<SmallDummy>());
        let obj_addr = GuestAddress(obj_off);

        c.bench_function(
            format!("write_small_obj_to_{:#0X}", obj_off).as_str(),
            |b| {
                b.iter(|| {
                    black_box(&memory)
                        .write_obj::<SmallDummy>(some_small_dummy, obj_addr)
                        .unwrap()
                })
            },
        );

        let obj_off = access.make_offset(size_of::<BigDummy>());
        let obj_addr = GuestAddress(obj_off);

        c.bench_function(format!("write_big_obj_to_{:#0X}", obj_off).as_str(), |b| {
            b.iter(|| {
                black_box(&memory)
                    .write_obj::<BigDummy>(some_big_dummy, obj_addr)
                    .unwrap()
            })
        });
    }
}
