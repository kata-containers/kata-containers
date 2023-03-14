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

use std::num::Wrapping;
use std::ops::Deref;
use std::sync::atomic::Ordering;

use vm_memory::{Address, Bytes, GuestAddress, GuestMemory};

use crate::defs::{VIRTQ_AVAIL_ELEMENT_SIZE, VIRTQ_AVAIL_RING_HEADER_SIZE};
use crate::{error, DescriptorChain, QueueState};

/// Consuming iterator over all available descriptor chain heads in the queue.
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
/// #    // The chains are (0, 1), (2, 3, 4) and (5, 6).
/// #    for i in 0..7 {
/// #        let flags = match i {
/// #            1 | 6 => 0,
/// #            2 | 5 => VIRTQ_DESC_F_NEXT | VIRTQ_DESC_F_WRITE,
/// #            4 => VIRTQ_DESC_F_WRITE,
/// #            _ => VIRTQ_DESC_F_NEXT,
/// #        };
/// #
/// #        let desc = Descriptor::new((0x1000 * (i + 1)) as u64, 0x1000, flags, i + 1);
/// #        vq.desc_table().store(i, desc);
/// #    }
/// #
/// #    vq.avail().ring().ref_at(0).unwrap().store(u16::to_le(0));
/// #    vq.avail().ring().ref_at(1).unwrap().store(u16::to_le(2));
/// #    vq.avail().ring().ref_at(2).unwrap().store(u16::to_le(5));
/// #    vq.avail().idx().store(u16::to_le(3));
/// #    q
/// # }
/// let m = &GuestMemoryMmap::<()>::from_ranges(&[(GuestAddress(0), 0x10000)]).unwrap();
/// // Populate the queue with descriptor chains and update the available ring accordingly.
/// let mut queue = populate_queue(m);
/// let mut i = queue.iter().unwrap();
///
/// {
///     let mut c = i.next().unwrap();
///     let _first_head_index = c.head_index();
///     // We should have two descriptors in the first chain.
///     let _desc1 = c.next().unwrap();
///     let _desc2 = c.next().unwrap();
/// }
///
/// {
///     let c = i.next().unwrap();
///     let _second_head_index = c.head_index();
///
///     let mut iter = c.writable();
///     // We should have two writable descriptors in the second chain.
///     let _desc1 = iter.next().unwrap();
///     let _desc2 = iter.next().unwrap();
/// }
///
/// {
///     let c = i.next().unwrap();
///     let _third_head_index = c.head_index();
///
///     let mut iter = c.readable();
///     // We should have one readable descriptor in the third chain.
///     let _desc1 = iter.next().unwrap();
/// }
/// // Let's go back one position in the available ring.
/// i.go_to_previous_position();
/// // We should be able to access again the third descriptor chain.
/// let c = i.next().unwrap();
/// let _third_head_index = c.head_index();
/// ```
#[derive(Debug)]
pub struct AvailIter<'b, M> {
    mem: M,
    desc_table: GuestAddress,
    avail_ring: GuestAddress,
    queue_size: u16,
    last_index: Wrapping<u16>,
    next_avail: &'b mut Wrapping<u16>,
}

impl<'b, M> AvailIter<'b, M>
where
    M: Deref,
    M::Target: GuestMemory,
{
    /// Create a new instance of `AvailInter`.
    ///
    /// # Arguments
    /// * `mem` - the `GuestMemory` object that can be used to access the queue buffers.
    /// * `idx` - the index of the available ring entry where the driver would put the next
    ///           available descriptor chain.
    /// * `state` - the `QueueState` object from which the needed data to create the `AvailIter` can
    ///             be retrieved.
    pub(crate) fn new(mem: M, idx: Wrapping<u16>, state: &'b mut QueueState) -> Self {
        AvailIter {
            mem,
            desc_table: state.desc_table,
            avail_ring: state.avail_ring,
            queue_size: state.size,
            last_index: idx,
            next_avail: &mut state.next_avail,
        }
    }

    /// Goes back one position in the available descriptor chain offered by the driver.
    ///
    /// Rust does not support bidirectional iterators. This is the only way to revert the effect
    /// of an iterator increment on the queue.
    ///
    /// Note: this method assumes there's only one thread manipulating the queue, so it should only
    /// be invoked in single-threaded context.
    pub fn go_to_previous_position(&mut self) {
        *self.next_avail -= Wrapping(1);
    }
}

impl<'b, M> Iterator for AvailIter<'b, M>
where
    M: Clone + Deref,
    M::Target: GuestMemory,
{
    type Item = DescriptorChain<M>;

    fn next(&mut self) -> Option<Self::Item> {
        if *self.next_avail == self.last_index {
            return None;
        }

        // These two operations can not overflow an u64 since they're working with relatively small
        // numbers compared to u64::MAX.
        let elem_off =
            u64::from(self.next_avail.0.checked_rem(self.queue_size)?) * VIRTQ_AVAIL_ELEMENT_SIZE;
        let offset = VIRTQ_AVAIL_RING_HEADER_SIZE + elem_off;

        let addr = self.avail_ring.checked_add(offset)?;
        let head_index: u16 = self
            .mem
            .load(addr, Ordering::Acquire)
            .map(u16::from_le)
            .map_err(|_| error!("Failed to read from memory {:x}", addr.raw_value()))
            .ok()?;

        *self.next_avail += Wrapping(1);

        Some(DescriptorChain::new(
            self.mem.clone(),
            self.desc_table,
            self.queue_size,
            head_index,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::defs::{VIRTQ_DESC_F_NEXT, VIRTQ_DESC_F_WRITE};
    use crate::mock::MockSplitQueue;
    use crate::{Descriptor, QueueStateT};
    use vm_memory::GuestMemoryMmap;

    #[test]
    fn test_descriptor_and_iterator() {
        let m = &GuestMemoryMmap::<()>::from_ranges(&[(GuestAddress(0), 0x10000)]).unwrap();
        let vq = MockSplitQueue::new(m, 16);

        let mut q = vq.create_queue(m);

        // q is currently valid
        assert!(q.is_valid());

        // the chains are (0, 1), (2, 3, 4) and (5, 6)
        for j in 0..7 {
            let flags = match j {
                1 | 6 => 0,
                2 | 5 => VIRTQ_DESC_F_NEXT | VIRTQ_DESC_F_WRITE,
                4 => VIRTQ_DESC_F_WRITE,
                _ => VIRTQ_DESC_F_NEXT,
            };

            let desc = Descriptor::new((0x1000 * (j + 1)) as u64, 0x1000, flags, j + 1);
            vq.desc_table().store(j, desc).unwrap();
        }

        vq.avail().ring().ref_at(0).unwrap().store(u16::to_le(0));
        vq.avail().ring().ref_at(1).unwrap().store(u16::to_le(2));
        vq.avail().ring().ref_at(2).unwrap().store(u16::to_le(5));
        vq.avail().idx().store(u16::to_le(3));

        let mut i = q.iter().unwrap();

        {
            let c = i.next().unwrap();
            assert_eq!(c.head_index(), 0);

            let mut iter = c;
            assert!(iter.next().is_some());
            assert!(iter.next().is_some());
            assert!(iter.next().is_none());
            assert!(iter.next().is_none());
        }

        {
            let c = i.next().unwrap();
            assert_eq!(c.head_index(), 2);

            let mut iter = c.writable();
            assert!(iter.next().is_some());
            assert!(iter.next().is_some());
            assert!(iter.next().is_none());
            assert!(iter.next().is_none());
        }

        {
            let c = i.next().unwrap();
            assert_eq!(c.head_index(), 5);

            let mut iter = c.readable();
            assert!(iter.next().is_some());
            assert!(iter.next().is_none());
            assert!(iter.next().is_none());
        }
    }

    #[test]
    fn test_iterator() {
        let m = &GuestMemoryMmap::<()>::from_ranges(&[(GuestAddress(0), 0x10000)]).unwrap();
        let vq = MockSplitQueue::new(m, 16);

        let mut q = vq.create_queue(m);

        q.state.size = q.state.max_size;
        q.state.desc_table = vq.desc_table_addr();
        q.state.avail_ring = vq.avail_addr();
        q.state.used_ring = vq.used_addr();
        assert!(q.is_valid());

        {
            // an invalid queue should return an iterator with no next
            q.state.ready = false;
            let mut i = q.iter().unwrap();
            assert!(i.next().is_none());
        }

        q.state.ready = true;

        // now let's create two simple descriptor chains
        // the chains are (0, 1) and (2, 3, 4)
        {
            for j in 0..5u16 {
                let flags = match j {
                    1 | 4 => 0,
                    _ => VIRTQ_DESC_F_NEXT,
                };

                let desc = Descriptor::new((0x1000 * (j + 1)) as u64, 0x1000, flags, j + 1);
                vq.desc_table().store(j, desc).unwrap();
            }

            vq.avail().ring().ref_at(0).unwrap().store(u16::to_le(0));
            vq.avail().ring().ref_at(1).unwrap().store(u16::to_le(2));
            vq.avail().idx().store(u16::to_le(2));

            let mut i = q.iter().unwrap();

            {
                let mut c = i.next().unwrap();
                assert_eq!(c.head_index(), 0);

                c.next().unwrap();
                assert!(c.next().is_some());
                assert!(c.next().is_none());
                assert_eq!(c.head_index(), 0);
            }

            {
                let mut c = i.next().unwrap();
                assert_eq!(c.head_index(), 2);

                c.next().unwrap();
                c.next().unwrap();
                c.next().unwrap();
                assert!(c.next().is_none());
                assert_eq!(c.head_index(), 2);
            }

            // also test go_to_previous_position() works as expected
            {
                assert!(i.next().is_none());
                i.go_to_previous_position();
                let mut c = q.iter().unwrap().next().unwrap();
                c.next().unwrap();
                c.next().unwrap();
                c.next().unwrap();
                assert!(c.next().is_none());
            }
        }
    }

    #[test]
    fn test_regression_iterator_division() {
        // This is a regression test that tests that the iterator does not try to divide
        // by 0 when the queue size is 0
        let m = &GuestMemoryMmap::<()>::from_ranges(&[(GuestAddress(0), 0x10000)]).unwrap();
        let vq = MockSplitQueue::new(m, 1);
        // This input was generated by the fuzzer, both for the QueueState and the Descriptor
        let descriptors: Vec<Descriptor> = vec![Descriptor::new(
            14178673876262995140,
            3301229764,
            50372,
            50372,
        )];
        vq.build_desc_chain(&descriptors).unwrap();

        let mut q = QueueState {
            max_size: 38,
            next_avail: Wrapping(0),
            next_used: Wrapping(0),
            event_idx_enabled: false,
            num_added: Wrapping(0),
            size: 0,
            ready: false,
            desc_table: GuestAddress(12837708984796196),
            avail_ring: GuestAddress(0),
            used_ring: GuestAddress(9943947977301164032),
        };

        assert!(q.pop_descriptor_chain(m).is_none());
    }
}
