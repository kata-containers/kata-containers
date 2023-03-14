// Portions Copyright 2017 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE-BSD-3-Clause file.
//
// Copyright 2018 Amazon.com, Inc. or its affiliates. All Rights Reserved.
//
// Copyright © 2019 Intel Corporation
//
// Copyright (C) 2020-2021 Alibaba Cloud. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause

use vm_memory::{ByteValued, GuestAddress, Le16, Le32, Le64};

use crate::defs::{VIRTQ_DESC_F_INDIRECT, VIRTQ_DESC_F_NEXT, VIRTQ_DESC_F_WRITE};

/// A virtio descriptor constraints with C representation.
///
/// # Example
///
/// ```rust
/// # use virtio_queue::defs::{VIRTQ_DESC_F_NEXT, VIRTQ_DESC_F_WRITE};
/// # use virtio_queue::mock::MockSplitQueue;
/// use virtio_queue::{Descriptor, Queue};
/// use vm_memory::{GuestAddress, GuestMemoryMmap};
///
/// # fn populate_queue(m: &GuestMemoryMmap) -> Queue<&GuestMemoryMmap> {
/// #    let vq = MockSplitQueue::new(m, 16);
/// #    let mut q = vq.create_queue(m);
/// #
/// #    // We have only one chain: (0, 1).
/// #    let desc = Descriptor::new(0x1000, 0x1000, VIRTQ_DESC_F_NEXT, 1);
/// #    vq.desc_table().store(0, desc);
/// #    let desc = Descriptor::new(0x2000, 0x1000, VIRTQ_DESC_F_WRITE, 0);
/// #    vq.desc_table().store(1, desc);
/// #
/// #    vq.avail().ring().ref_at(0).unwrap().store(u16::to_le(0));
/// #    vq.avail().idx().store(u16::to_le(1));
/// #    q
/// # }
/// let m = &GuestMemoryMmap::<()>::from_ranges(&[(GuestAddress(0), 0x10000)]).unwrap();
/// // Populate the queue with descriptor chains and update the available ring accordingly.
/// let mut queue = populate_queue(m);
/// let mut i = queue.iter().unwrap();
/// let mut c = i.next().unwrap();
///
/// // Get the first descriptor and access its fields.
/// let desc = c.next().unwrap();
/// let _addr = desc.addr();
/// let _len = desc.len();
/// let _flags = desc.flags();
/// let _next = desc.next();
/// let _is_write_only = desc.is_write_only();
/// let _has_next = desc.has_next();
/// let _refers_to_ind_table = desc.refers_to_indirect_table();
/// ```
// Note that the `ByteValued` implementation of this structure expects the `Descriptor` to store
// only plain old data types.
#[repr(C)]
#[derive(Default, Clone, Copy, Debug)]
pub struct Descriptor {
    /// Guest physical address of device specific data.
    addr: Le64,

    /// Length of device specific data.
    len: Le32,

    /// Includes next, write, and indirect bits.
    flags: Le16,

    /// Index into the descriptor table of the next descriptor if flags has the `next` bit set.
    next: Le16,
}

#[allow(clippy::len_without_is_empty)]
impl Descriptor {
    /// Return the guest physical address of the descriptor buffer.
    pub fn addr(&self) -> GuestAddress {
        GuestAddress(self.addr.into())
    }

    /// Return the length of the descriptor buffer.
    pub fn len(&self) -> u32 {
        self.len.into()
    }

    /// Return the flags for this descriptor, including next, write and indirect bits.
    pub fn flags(&self) -> u16 {
        self.flags.into()
    }

    /// Return the value stored in the `next` field of the descriptor.
    pub fn next(&self) -> u16 {
        self.next.into()
    }

    /// Check whether this descriptor refers to a buffer containing an indirect descriptor table.
    pub fn refers_to_indirect_table(&self) -> bool {
        self.flags() & VIRTQ_DESC_F_INDIRECT != 0
    }

    /// Check whether the `VIRTQ_DESC_F_NEXT` is set for the descriptor.
    pub fn has_next(&self) -> bool {
        self.flags() & VIRTQ_DESC_F_NEXT != 0
    }

    /// Check if the driver designated this as a write only descriptor.
    ///
    /// If this is false, this descriptor is read only.
    /// Write only means the the emulated device can write and the driver can read.
    pub fn is_write_only(&self) -> bool {
        self.flags() & VIRTQ_DESC_F_WRITE != 0
    }
}

#[cfg(any(test, feature = "test-utils"))]
impl Descriptor {
    /// Create a new descriptor.
    ///
    /// # Arguments
    /// * `addr` - the guest physical address of the descriptor buffer.
    /// * `len` - the length of the descriptor buffer.
    /// * `flags` - the `flags` for the descriptor.
    /// * `next` - the `next` field of the descriptor.
    pub fn new(addr: u64, len: u32, flags: u16, next: u16) -> Self {
        Descriptor {
            addr: addr.into(),
            len: len.into(),
            flags: flags.into(),
            next: next.into(),
        }
    }

    /// Set the guest physical address of the descriptor buffer.
    pub fn set_addr(&mut self, addr: u64) {
        self.addr = addr.into();
    }

    /// Set the length of the descriptor buffer.
    pub fn set_len(&mut self, len: u32) {
        self.len = len.into();
    }

    /// Set the flags for this descriptor.
    pub fn set_flags(&mut self, flags: u16) {
        self.flags = flags.into();
    }

    /// Set the value stored in the `next` field of the descriptor.
    pub fn set_next(&mut self, next: u16) {
        self.next = next.into();
    }
}

// This is safe because `Descriptor` contains only wrappers over POD types and all accesses through
// safe `vm-memory` API will validate any garbage that could be included in there.
unsafe impl ByteValued for Descriptor {}

/// Represents the contents of an element from the used virtqueue ring.
// Note that the `ByteValued` implementation of this structure expects the `VirtqUsedElem` to store
// only plain old data types.
#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct VirtqUsedElem {
    id: Le32,
    len: Le32,
}

impl VirtqUsedElem {
    /// Create a new `VirtqUsedElem` instance.
    ///
    /// # Arguments
    /// * `id` - the index of the used descriptor chain.
    /// * `len` - the total length of the descriptor chain which was used (written to).
    pub(crate) fn new(id: u32, len: u32) -> Self {
        VirtqUsedElem {
            id: id.into(),
            len: len.into(),
        }
    }
}

#[cfg(any(test, feature = "test-utils"))]
#[allow(clippy::len_without_is_empty)]
impl VirtqUsedElem {
    /// Get the index of the used descriptor chain.
    pub fn id(&self) -> u32 {
        self.id.into()
    }

    /// Get `length` field of the used ring entry.
    pub fn len(&self) -> u32 {
        self.len.into()
    }
}

// This is safe because `VirtqUsedElem` contains only wrappers over POD types and all accesses
// through safe `vm-memory` API will validate any garbage that could be included in there.
unsafe impl ByteValued for VirtqUsedElem {}

#[cfg(test)]
mod tests {
    use super::*;
    use memoffset::offset_of;
    use std::mem::{align_of, size_of};

    #[test]
    fn test_descriptor_offset() {
        assert_eq!(size_of::<Descriptor>(), 16);
        assert_eq!(offset_of!(Descriptor, addr), 0);
        assert_eq!(offset_of!(Descriptor, len), 8);
        assert_eq!(offset_of!(Descriptor, flags), 12);
        assert_eq!(offset_of!(Descriptor, next), 14);
        assert!(align_of::<Descriptor>() <= 16);
    }

    #[test]
    fn test_descriptor_getter_setter() {
        let mut desc = Descriptor::new(0, 0, 0, 0);

        desc.set_addr(0x1000);
        assert_eq!(desc.addr(), GuestAddress(0x1000));
        desc.set_len(0x2000);
        assert_eq!(desc.len(), 0x2000);
        desc.set_flags(VIRTQ_DESC_F_NEXT);
        assert_eq!(desc.flags(), VIRTQ_DESC_F_NEXT);
        assert!(desc.has_next());
        assert!(!desc.is_write_only());
        assert!(!desc.refers_to_indirect_table());
        desc.set_flags(VIRTQ_DESC_F_WRITE);
        assert_eq!(desc.flags(), VIRTQ_DESC_F_WRITE);
        assert!(!desc.has_next());
        assert!(desc.is_write_only());
        assert!(!desc.refers_to_indirect_table());
        desc.set_flags(VIRTQ_DESC_F_INDIRECT);
        assert_eq!(desc.flags(), VIRTQ_DESC_F_INDIRECT);
        assert!(!desc.has_next());
        assert!(!desc.is_write_only());
        assert!(desc.refers_to_indirect_table());
        desc.set_next(3);
        assert_eq!(desc.next(), 3);
    }

    #[test]
    fn test_descriptor_copy() {
        let e1 = Descriptor::new(1, 2, VIRTQ_DESC_F_NEXT, 3);
        let mut e2 = Descriptor::default();

        e2.as_mut_slice().copy_from_slice(e1.as_slice());
        assert_eq!(e1.addr(), e2.addr());
        assert_eq!(e1.len(), e2.len());
        assert_eq!(e1.flags(), e2.flags());
        assert_eq!(e1.next(), e2.next());
    }

    #[test]
    fn test_used_elem_offset() {
        assert_eq!(offset_of!(VirtqUsedElem, id), 0);
        assert_eq!(offset_of!(VirtqUsedElem, len), 4);
        assert_eq!(size_of::<VirtqUsedElem>(), 8);
    }

    #[test]
    fn test_used_elem_copy() {
        let e1 = VirtqUsedElem::new(3, 15);
        let mut e2 = VirtqUsedElem::new(0, 0);

        e2.as_mut_slice().copy_from_slice(e1.as_slice());
        assert_eq!(e1.id, e2.id);
        assert_eq!(e1.len, e2.len);
    }
}
