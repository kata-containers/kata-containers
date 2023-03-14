// Copyright © 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

//! Fuse extension protocol messages to support virtio-fs.

#![allow(missing_docs)]

use bitflags::bitflags;
use vm_memory::ByteValued;

bitflags! {
    /// Flags for Setupmapping request.
    pub struct SetupmappingFlags: u64 {
        /// Mapping with write permission
        const WRITE = 0x1;
        /// Mapping with read permission
        const READ = 0x2;
    }
}

/// Setup file mapping request message for virtio-fs.
#[repr(C)]
#[derive(Debug, Default, Copy, Clone)]
pub struct SetupmappingIn {
    /// File handler.
    pub fh: u64,
    /// File offset.
    pub foffset: u64,
    /// Length to map.
    pub len: u64,
    /// Mapping flags
    pub flags: u64,
    /// Mapping offset in the DAX window.
    pub moffset: u64,
}

unsafe impl ByteValued for SetupmappingIn {}

/// Remove file mapping request message header for virtio-fs.
#[repr(C)]
#[derive(Debug, Default, Copy, Clone)]
pub struct RemovemappingIn {
    /// Number of `RemovemappingOne` entries in the message payload.
    pub count: u32,
}

unsafe impl ByteValued for RemovemappingIn {}

/// Remove file mapping request payload entry for virtio-fs.
#[repr(C)]
#[derive(Debug, Default, Copy, Clone)]
pub struct RemovemappingOne {
    /// Mapping offset in the DAX window.
    pub moffset: u64,
    /// Length to unmap.
    pub len: u64,
}

unsafe impl ByteValued for RemovemappingOne {}
