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

use std::convert::TryFrom;
use std::fmt::{self, Debug};
use std::mem::size_of;
use std::ops::Deref;

use vm_memory::{Address, Bytes, GuestAddress, GuestMemory};

use crate::defs::VIRTQ_DESCRIPTOR_SIZE;
use crate::{Descriptor, Error};

/// A virtio descriptor chain.
#[derive(Clone, Debug)]
pub struct DescriptorChain<M> {
    mem: M,
    desc_table: GuestAddress,
    queue_size: u16,
    head_index: u16,
    next_index: u16,
    ttl: u16,
    is_indirect: bool,
}

impl<M> DescriptorChain<M>
where
    M: Deref,
    M::Target: GuestMemory,
{
    fn with_ttl(
        mem: M,
        desc_table: GuestAddress,
        queue_size: u16,
        ttl: u16,
        head_index: u16,
    ) -> Self {
        DescriptorChain {
            mem,
            desc_table,
            queue_size,
            head_index,
            next_index: head_index,
            ttl,
            is_indirect: false,
        }
    }

    /// Create a new `DescriptorChain` instance.
    ///
    /// # Arguments
    /// * `mem` - the `GuestMemory` object that can be used to access the buffers pointed to by the
    ///           descriptor chain.
    /// * `desc_table` - the address of the descriptor table.
    /// * `queue_size` - the size of the queue, which is also the maximum size of a descriptor
    ///                  chain.
    /// * `head_index` - the descriptor index of the chain head.
    pub(crate) fn new(mem: M, desc_table: GuestAddress, queue_size: u16, head_index: u16) -> Self {
        Self::with_ttl(mem, desc_table, queue_size, queue_size, head_index)
    }

    /// Get the descriptor index of the chain head.
    pub fn head_index(&self) -> u16 {
        self.head_index
    }

    /// Return a `GuestMemory` object that can be used to access the buffers pointed to by the
    /// descriptor chain.
    pub fn memory(&self) -> &M::Target {
        self.mem.deref()
    }

    /// Return an iterator that only yields the readable descriptors in the chain.
    pub fn readable(self) -> DescriptorChainRwIter<M> {
        DescriptorChainRwIter {
            chain: self,
            writable: false,
        }
    }

    /// Return an iterator that only yields the writable descriptors in the chain.
    pub fn writable(self) -> DescriptorChainRwIter<M> {
        DescriptorChainRwIter {
            chain: self,
            writable: true,
        }
    }

    // Alters the internal state of the `DescriptorChain` to switch iterating over an
    // indirect descriptor table defined by `desc`.
    fn switch_to_indirect_table(&mut self, desc: Descriptor) -> Result<(), Error> {
        // Check the VIRTQ_DESC_F_INDIRECT flag (i.e., is_indirect) is not set inside
        // an indirect descriptor.
        // (see VIRTIO Spec, Section 2.6.5.3.1 Driver Requirements: Indirect Descriptors)
        if self.is_indirect {
            return Err(Error::InvalidIndirectDescriptor);
        }

        // Check the target indirect descriptor table is correctly aligned.
        if desc.addr().raw_value() & (VIRTQ_DESCRIPTOR_SIZE as u64 - 1) != 0
            || desc.len() & (VIRTQ_DESCRIPTOR_SIZE as u32 - 1) != 0
        {
            return Err(Error::InvalidIndirectDescriptorTable);
        }

        // It is safe to do a plain division since we checked above that desc.len() is a multiple of
        // VIRTQ_DESCRIPTOR_SIZE, and VIRTQ_DESCRIPTOR_SIZE is != 0.
        let table_len = (desc.len() as usize) / VIRTQ_DESCRIPTOR_SIZE;
        if table_len > usize::from(u16::MAX) {
            return Err(Error::InvalidIndirectDescriptorTable);
        }

        self.desc_table = desc.addr();
        // try_from cannot fail as we've checked table_len above
        self.queue_size = u16::try_from(table_len).expect("invalid table_len");
        self.next_index = 0;
        self.ttl = self.queue_size;
        self.is_indirect = true;

        Ok(())
    }
}

impl<M> Iterator for DescriptorChain<M>
where
    M: Deref,
    M::Target: GuestMemory,
{
    type Item = Descriptor;

    /// Return the next descriptor in this descriptor chain, if there is one.
    ///
    /// Note that this is distinct from the next descriptor chain returned by
    /// [`AvailIter`](struct.AvailIter.html), which is the head of the next
    /// _available_ descriptor chain.
    fn next(&mut self) -> Option<Self::Item> {
        if self.ttl == 0 || self.next_index >= self.queue_size {
            return None;
        }

        let desc_addr = self
            .desc_table
            // The multiplication can not overflow an u64 since we are multiplying an u16 with a
            // small number.
            .checked_add(self.next_index as u64 * size_of::<Descriptor>() as u64)?;

        // The guest device driver should not touch the descriptor once submitted, so it's safe
        // to use read_obj() here.
        let desc = self.mem.read_obj::<Descriptor>(desc_addr).ok()?;

        if desc.refers_to_indirect_table() {
            self.switch_to_indirect_table(desc).ok()?;
            return self.next();
        }

        if desc.has_next() {
            self.next_index = desc.next();
            // It's ok to decrement `self.ttl` here because we check at the start of the method
            // that it's greater than 0.
            self.ttl -= 1;
        } else {
            self.ttl = 0;
        }

        Some(desc)
    }
}

/// An iterator for readable or writable descriptors.
#[derive(Clone)]
pub struct DescriptorChainRwIter<M> {
    chain: DescriptorChain<M>,
    writable: bool,
}

impl<M> Iterator for DescriptorChainRwIter<M>
where
    M: Deref,
    M::Target: GuestMemory,
{
    type Item = Descriptor;

    /// Return the next readable/writeable descriptor (depending on the `writable` value) in this
    /// descriptor chain, if there is one.
    ///
    /// Note that this is distinct from the next descriptor chain returned by
    /// [`AvailIter`](struct.AvailIter.html), which is the head of the next
    /// _available_ descriptor chain.
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.chain.next() {
                Some(v) => {
                    if v.is_write_only() == self.writable {
                        return Some(v);
                    }
                }
                None => return None,
            }
        }
    }
}

// We can't derive Debug, because rustc doesn't generate the `M::T: Debug` constraint
impl<M> Debug for DescriptorChainRwIter<M>
where
    M: Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DescriptorChainRwIter")
            .field("chain", &self.chain)
            .field("writable", &self.writable)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::defs::{VIRTQ_DESC_F_INDIRECT, VIRTQ_DESC_F_NEXT};
    use crate::mock::{DescriptorTable, MockSplitQueue};
    use vm_memory::GuestMemoryMmap;

    #[test]
    fn test_checked_new_descriptor_chain() {
        let m = &GuestMemoryMmap::<()>::from_ranges(&[(GuestAddress(0), 0x10000)]).unwrap();
        let vq = MockSplitQueue::new(m, 16);

        assert!(vq.end().0 < 0x1000);

        // index >= queue_size
        assert!(
            DescriptorChain::<&GuestMemoryMmap>::new(m, vq.start(), 16, 16)
                .next()
                .is_none()
        );

        // desc_table address is way off
        assert!(
            DescriptorChain::<&GuestMemoryMmap>::new(m, GuestAddress(0x00ff_ffff_ffff), 16, 0)
                .next()
                .is_none()
        );

        {
            // the first desc has a normal len, and the next_descriptor flag is set
            // but the the index of the next descriptor is too large
            let desc = Descriptor::new(0x1000, 0x1000, VIRTQ_DESC_F_NEXT, 16);
            vq.desc_table().store(0, desc).unwrap();

            let mut c = DescriptorChain::<&GuestMemoryMmap>::new(m, vq.start(), 16, 0);
            c.next().unwrap();
            assert!(c.next().is_none());
        }

        // finally, let's test an ok chain
        {
            let desc = Descriptor::new(0x1000, 0x1000, VIRTQ_DESC_F_NEXT, 1);
            vq.desc_table().store(0, desc).unwrap();

            let desc = Descriptor::new(0x2000, 0x1000, 0, 0);
            vq.desc_table().store(1, desc).unwrap();

            let mut c = DescriptorChain::<&GuestMemoryMmap>::new(m, vq.start(), 16, 0);

            assert_eq!(
                c.memory() as *const GuestMemoryMmap,
                m as *const GuestMemoryMmap
            );

            assert_eq!(c.desc_table, vq.start());
            assert_eq!(c.queue_size, 16);
            assert_eq!(c.ttl, c.queue_size);

            let desc = c.next().unwrap();
            assert_eq!(desc.addr(), GuestAddress(0x1000));
            assert_eq!(desc.len(), 0x1000);
            assert_eq!(desc.flags(), VIRTQ_DESC_F_NEXT);
            assert_eq!(desc.next(), 1);
            assert_eq!(c.ttl, c.queue_size - 1);

            assert!(c.next().is_some());
            // The descriptor above was the last from the chain, so `ttl` should be 0 now.
            assert_eq!(c.ttl, 0);
            assert!(c.next().is_none());
            assert_eq!(c.ttl, 0);
        }
    }

    #[test]
    fn test_ttl_wrap_around() {
        const QUEUE_SIZE: u16 = 16;

        let m = &GuestMemoryMmap::<()>::from_ranges(&[(GuestAddress(0), 0x100000)]).unwrap();
        let vq = MockSplitQueue::new(m, QUEUE_SIZE);

        // Populate the entire descriptor table with entries. Only the last one should not have the
        // VIRTQ_DESC_F_NEXT set.
        for i in 0..QUEUE_SIZE - 1 {
            let desc = Descriptor::new(0x1000 * (i + 1) as u64, 0x1000, VIRTQ_DESC_F_NEXT, i + 1);
            vq.desc_table().store(i, desc).unwrap();
        }
        let desc = Descriptor::new((0x1000 * 16) as u64, 0x1000, 0, 0);
        vq.desc_table().store(QUEUE_SIZE - 1, desc).unwrap();

        let mut c = DescriptorChain::<&GuestMemoryMmap>::new(m, vq.start(), QUEUE_SIZE, 0);
        assert_eq!(c.ttl, c.queue_size);

        // Validate that `ttl` wraps around even when the entire descriptor table is populated.
        for i in 0..QUEUE_SIZE {
            let _desc = c.next().unwrap();
            assert_eq!(c.ttl, c.queue_size - i - 1);
        }
        assert!(c.next().is_none());
    }

    #[test]
    fn test_new_from_indirect_descriptor() {
        // This is testing that chaining an indirect table works as expected. It is also a negative
        // test for the following requirement from the spec:
        // `A driver MUST NOT set both VIRTQ_DESC_F_INDIRECT and VIRTQ_DESC_F_NEXT in flags.`. In
        // case the driver is setting both of these flags, we check that the device doesn't panic.
        let m = &GuestMemoryMmap::<()>::from_ranges(&[(GuestAddress(0), 0x10000)]).unwrap();
        let vq = MockSplitQueue::new(m, 16);
        let dtable = vq.desc_table();

        // Create a chain with one normal descriptor and one pointing to an indirect table.
        let desc = Descriptor::new(0x6000, 0x1000, VIRTQ_DESC_F_NEXT, 1);
        dtable.store(0, desc).unwrap();
        // The spec forbids setting both VIRTQ_DESC_F_INDIRECT and VIRTQ_DESC_F_NEXT in flags. We do
        // not currently enforce this rule, we just ignore the VIRTQ_DESC_F_NEXT flag.
        let desc = Descriptor::new(0x7000, 0x1000, VIRTQ_DESC_F_INDIRECT | VIRTQ_DESC_F_NEXT, 2);
        dtable.store(1, desc).unwrap();
        let desc = Descriptor::new(0x8000, 0x1000, 0, 0);
        dtable.store(2, desc).unwrap();

        let mut c: DescriptorChain<&GuestMemoryMmap> = DescriptorChain::new(m, vq.start(), 16, 0);

        // create an indirect table with 4 chained descriptors
        let idtable = DescriptorTable::new(m, GuestAddress(0x7000), 4);
        for i in 0..4u16 {
            let desc: Descriptor = if i < 3 {
                Descriptor::new(0x1000 * i as u64, 0x1000, VIRTQ_DESC_F_NEXT, i + 1)
            } else {
                Descriptor::new(0x1000 * i as u64, 0x1000, 0, 0)
            };
            idtable.store(i, desc).unwrap();
        }

        assert_eq!(c.head_index(), 0);
        // Consume the first descriptor.
        c.next().unwrap();

        // The chain logic hasn't parsed the indirect descriptor yet.
        assert!(!c.is_indirect);

        // Try to iterate through the indirect descriptor chain.
        for i in 0..4 {
            let desc = c.next().unwrap();
            assert!(c.is_indirect);
            if i < 3 {
                assert_eq!(desc.flags(), VIRTQ_DESC_F_NEXT);
                assert_eq!(desc.next(), i + 1);
            }
        }
        // Even though we added a new descriptor after the one that is pointing to the indirect
        // table, this descriptor won't be available when parsing the chain.
        assert!(c.next().is_none());
    }

    #[test]
    fn test_indirect_descriptor_err() {
        // We are testing here different misconfigurations of the indirect table. For these error
        // case scenarios, the iterator over the descriptor chain won't return a new descriptor.
        {
            let m = &GuestMemoryMmap::<()>::from_ranges(&[(GuestAddress(0), 0x10000)]).unwrap();
            let vq = MockSplitQueue::new(m, 16);

            // Create a chain with a descriptor pointing to an invalid indirect table: addr not a
            // multiple of descriptor size.
            let desc = Descriptor::new(0x1001, 0x1000, VIRTQ_DESC_F_INDIRECT, 0);
            vq.desc_table().store(0, desc).unwrap();

            let mut c: DescriptorChain<&GuestMemoryMmap> =
                DescriptorChain::new(m, vq.start(), 16, 0);

            assert!(c.next().is_none());
        }

        {
            let m = &GuestMemoryMmap::from_ranges(&[(GuestAddress(0), 0x10000)]).unwrap();
            let vq = MockSplitQueue::new(m, 16);

            // Create a chain with a descriptor pointing to an invalid indirect table: len not a
            // multiple of descriptor size.
            let desc = Descriptor::new(0x1000, 0x1001, VIRTQ_DESC_F_INDIRECT, 0);
            vq.desc_table().store(0, desc).unwrap();

            let mut c: DescriptorChain<&GuestMemoryMmap> =
                DescriptorChain::new(m, vq.start(), 16, 0);

            assert!(c.next().is_none());
        }

        {
            let m = &GuestMemoryMmap::from_ranges(&[(GuestAddress(0), 0x10000)]).unwrap();
            let vq = MockSplitQueue::new(m, 16);

            // Create a chain with a descriptor pointing to an invalid indirect table: table len >
            // u16::MAX.
            let desc = Descriptor::new(
                0x1000,
                (u16::MAX as u32 + 1) * VIRTQ_DESCRIPTOR_SIZE as u32,
                VIRTQ_DESC_F_INDIRECT,
                0,
            );
            vq.desc_table().store(0, desc).unwrap();

            let mut c: DescriptorChain<&GuestMemoryMmap> =
                DescriptorChain::new(m, vq.start(), 16, 0);

            assert!(c.next().is_none());
        }

        {
            let m = &GuestMemoryMmap::from_ranges(&[(GuestAddress(0), 0x10000)]).unwrap();
            let vq = MockSplitQueue::new(m, 16);

            // Create a chain with a descriptor pointing to an indirect table.
            let desc = Descriptor::new(0x1000, 0x1000, VIRTQ_DESC_F_INDIRECT, 0);
            vq.desc_table().store(0, desc).unwrap();
            // It's ok for an indirect descriptor to have flags = 0.
            let desc = Descriptor::new(0x3000, 0x1000, 0, 0);
            m.write_obj(desc, GuestAddress(0x1000)).unwrap();

            let mut c: DescriptorChain<&GuestMemoryMmap> =
                DescriptorChain::new(m, vq.start(), 16, 0);
            assert!(c.next().is_some());

            // But it's not allowed to have an indirect descriptor that points to another indirect
            // table.
            let desc = Descriptor::new(0x3000, 0x1000, VIRTQ_DESC_F_INDIRECT, 0);
            m.write_obj(desc, GuestAddress(0x1000)).unwrap();

            let mut c: DescriptorChain<&GuestMemoryMmap> =
                DescriptorChain::new(m, vq.start(), 16, 0);

            assert!(c.next().is_none());
        }
    }
}
