// Copyright 2018 Amazon.com, Inc. or its affiliates. All Rights Reserved.
//
// Portions Copyright 2017 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE-BSD-3-Clause file.
//
// Copyright © 2019 Intel Corporation
//
// Copyright (C) 2020-2021 Alibaba Cloud. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause

use std::num::Wrapping;
use std::ops::Deref;
use std::sync::atomic::Ordering;

use vm_memory::GuestAddressSpace;

use crate::{
    AvailIter, Error, QueueGuard, QueueState, QueueStateGuard, QueueStateOwnedT, QueueStateT,
};

/// A convenient wrapper struct for a virtio queue, with associated `GuestMemory` object.
///
/// # Example
///
/// ```rust
/// use virtio_queue::{Queue, QueueState};
/// use vm_memory::{Bytes, GuestAddress, GuestAddressSpace, GuestMemoryMmap};
///
/// let m = GuestMemoryMmap::<()>::from_ranges(&[(GuestAddress(0), 0x10000)]).unwrap();
/// let mut queue = Queue::<&GuestMemoryMmap, QueueState>::new(&m, 1024);
///
/// // First, the driver sets up the queue; this set up is done via writes on the bus (PCI, MMIO).
/// queue.set_size(8);
/// queue.set_desc_table_address(Some(0x1000), None);
/// queue.set_avail_ring_address(Some(0x2000), None);
/// queue.set_used_ring_address(Some(0x3000), None);
/// queue.set_event_idx(true);
/// queue.set_ready(true);
/// // The user should check if the queue is valid before starting to use it.
/// assert!(queue.is_valid());
///
/// // Here the driver would add entries in the available ring and then update the `idx` field of
/// // the available ring (address = 0x2000 + 2).
/// m.write_obj(3, GuestAddress(0x2002));
///
/// loop {
///     queue.disable_notification().unwrap();
///
///     // Consume entries from the available ring.
///     while let Some(chain) = queue.iter().unwrap().next() {
///         // Process the descriptor chain, and then add an entry in the used ring and optionally
///         // notify the driver.
///         queue.add_used(chain.head_index(), 0x100).unwrap();
///
///         if queue.needs_notification().unwrap() {
///             // Here we would notify the driver it has new entries in the used ring to consume.
///         }
///     }
///     if !queue.enable_notification().unwrap() {
///         break;
///     }
/// }
///
/// // We can reset the queue at some point.
/// queue.reset();
/// // The queue should not be ready after reset.
/// assert!(!queue.ready());
/// ```
#[derive(Clone, Debug)]
pub struct Queue<M: GuestAddressSpace, S: QueueStateT = QueueState> {
    /// Guest memory object associated with the queue.
    pub mem: M,
    /// Virtio queue state.
    pub state: S,
}

impl<M: GuestAddressSpace, S: QueueStateT> Queue<M, S> {
    /// Construct an empty virtio queue with the given `max_size`.
    ///
    /// # Arguments
    /// * `mem` - the guest memory object that can be used to access the queue buffers.
    /// * `max_size` - the maximum size (and the default one) of the queue.
    pub fn new(mem: M, max_size: u16) -> Self {
        Queue {
            mem,
            state: S::new(max_size),
        }
    }

    /// Check whether the queue configuration is valid.
    pub fn is_valid(&self) -> bool {
        self.state.is_valid(self.mem.memory().deref())
    }

    /// Reset the queue to the initial state.
    pub fn reset(&mut self) {
        self.state.reset()
    }

    /// Get an exclusive reference to the underlying `QueueState` object.
    ///
    /// Logically this method will acquire the underlying lock protecting the `QueueState` Object.
    /// The lock will be released when the returned object gets dropped.
    pub fn lock(&mut self) -> <S as QueueStateGuard>::G {
        self.state.lock()
    }

    /// Get an exclusive reference to the underlying `QueueState` object with an associated
    /// `GuestMemory` object.
    ///
    /// Logically this method will acquire the underlying lock protecting the `QueueState` Object.
    /// The lock will be released when the returned object gets dropped.
    pub fn lock_with_memory(
        &mut self,
    ) -> QueueGuard<<M as GuestAddressSpace>::T, <S as QueueStateGuard>::G> {
        QueueGuard::new(self.state.lock(), self.mem.memory())
    }

    /// Get the maximum size of the virtio queue.
    pub fn max_size(&self) -> u16 {
        self.state.max_size()
    }

    /// Configure the queue size for the virtio queue.
    ///
    /// # Arguments
    /// * `size` - the queue size; it should be a power of two, different than 0 and less than or
    ///            equal to the value reported by `max_size()`, otherwise the queue size remains the
    ///            default one (which is the maximum one).
    pub fn set_size(&mut self, size: u16) {
        self.state.set_size(size);
    }

    /// Check whether the queue is ready to be processed.
    pub fn ready(&self) -> bool {
        self.state.ready()
    }

    /// Configure the queue to the `ready for processing` state.
    ///
    /// # Arguments
    /// * `ready` - a boolean to indicate whether the queue is ready to be used or not.
    pub fn set_ready(&mut self, ready: bool) {
        self.state.set_ready(ready)
    }

    /// Set the descriptor table address for the queue.
    ///
    /// The descriptor table address is 64-bit, the corresponding part will be updated if 'low'
    /// and/or `high` is `Some` and valid.
    ///
    /// # Arguments
    /// * `low` - an optional value for the lowest 32 bits of the address.
    /// * `high` - an optional value for the highest 32 bits of the address.
    pub fn set_desc_table_address(&mut self, low: Option<u32>, high: Option<u32>) {
        self.state.set_desc_table_address(low, high);
    }

    /// Set the available ring address for the queue.
    ///
    /// The available ring address is 64-bit, the corresponding part will be updated if 'low'
    /// and/or `high` is `Some` and valid.
    ///
    /// # Arguments
    /// * `low` - an optional value for the lowest 32 bits of the address.
    /// * `high` - an optional value for the highest 32 bits of the address.
    pub fn set_avail_ring_address(&mut self, low: Option<u32>, high: Option<u32>) {
        self.state.set_avail_ring_address(low, high);
    }

    /// Set the used ring address for the queue.
    ///
    /// The used ring address is 64-bit, the corresponding part will be updated if 'low'
    /// and/or `high` is `Some` and valid.
    ///
    /// # Arguments
    /// * `low` - an optional value for the lowest 32 bits of the address.
    /// * `high` - an optional value for the highest 32 bits of the address.
    pub fn set_used_ring_address(&mut self, low: Option<u32>, high: Option<u32>) {
        self.state.set_used_ring_address(low, high);
    }

    /// Enable/disable the VIRTIO_F_RING_EVENT_IDX feature for interrupt coalescing.
    ///
    /// # Arguments
    /// * `enabled` - a boolean to indicate whether the VIRTIO_F_RING_EVENT_IDX feature was
    ///               successfully negotiated or not.
    pub fn set_event_idx(&mut self, enabled: bool) {
        self.state.set_event_idx(enabled)
    }

    /// Read the `idx` field from the available ring.
    ///
    /// # Arguments
    /// * `order` - the memory ordering used to access the `idx` field from memory.
    pub fn avail_idx(&self, order: Ordering) -> Result<Wrapping<u16>, Error> {
        self.state.avail_idx(self.mem.memory().deref(), order)
    }

    /// Reads the `idx` field from the used ring.
    ///
    /// # Arguments
    /// * `order` - the memory ordering used to access the `idx` field from memory.
    pub fn used_idx(&self, order: Ordering) -> Result<Wrapping<u16>, Error> {
        self.state.used_idx(self.mem.memory().deref(), order)
    }

    /// Put a used descriptor head into the used ring.
    ///
    /// # Arguments
    /// * `head_index` - the index of the used descriptor chain.
    /// * `len` - the total length of the descriptor chain which was used (written to).
    pub fn add_used(&mut self, head_index: u16, len: u32) -> Result<(), Error> {
        self.state
            .add_used(self.mem.memory().deref(), head_index, len)
    }

    /// Enable notification events from the guest driver.
    ///
    /// Return true if one or more descriptors can be consumed from the available ring after
    /// notifications were enabled (and thus it's possible there will be no corresponding
    /// notification).
    pub fn enable_notification(&mut self) -> Result<bool, Error> {
        self.state.enable_notification(self.mem.memory().deref())
    }

    /// Disable notification events from the guest driver.
    pub fn disable_notification(&mut self) -> Result<(), Error> {
        self.state.disable_notification(self.mem.memory().deref())
    }

    /// Check whether a notification to the guest is needed.
    ///
    /// Please note this method has side effects: once it returns `true`, it considers the
    /// driver will actually be notified, remember the associated index in the used ring, and
    /// won't return `true` again until the driver updates `used_event` and/or the notification
    /// conditions hold once more.
    pub fn needs_notification(&mut self) -> Result<bool, Error> {
        self.state.needs_notification(self.mem.memory().deref())
    }

    /// Return the index of the next entry in the available ring.
    pub fn next_avail(&self) -> u16 {
        self.state.next_avail()
    }

    /// Returns the index for the next descriptor in the used ring.
    pub fn next_used(&self) -> u16 {
        self.state.next_used()
    }

    /// Set the index of the next entry in the available ring.
    ///
    /// # Arguments
    /// * `next_avail` - the index of the next available ring entry.
    pub fn set_next_avail(&mut self, next_avail: u16) {
        self.state.set_next_avail(next_avail);
    }

    /// Sets the index for the next descriptor in the used ring.
    ///
    /// # Arguments
    /// * `next_used` - the index of the next used ring entry.
    pub fn set_next_used(&mut self, next_used: u16) {
        self.state.set_next_used(next_used);
    }
}

impl<M: GuestAddressSpace> Queue<M, QueueState> {
    /// A consuming iterator over all available descriptor chain heads offered by the driver.
    pub fn iter(&mut self) -> Result<AvailIter<'_, M::T>, Error> {
        self.state.iter(self.mem.memory())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::defs::{
        DEFAULT_AVAIL_RING_ADDR, DEFAULT_DESC_TABLE_ADDR, DEFAULT_USED_RING_ADDR,
        VIRTQ_DESC_F_NEXT, VIRTQ_DESC_F_WRITE, VIRTQ_USED_F_NO_NOTIFY,
    };
    use crate::mock::MockSplitQueue;
    use crate::Descriptor;

    use vm_memory::{Address, Bytes, GuestAddress, GuestMemoryMmap};

    #[test]
    fn test_queue_is_valid() {
        let m = &GuestMemoryMmap::<()>::from_ranges(&[(GuestAddress(0), 0x10000)]).unwrap();
        let vq = MockSplitQueue::new(m, 16);
        let mut q = vq.create_queue(m);

        // q is currently valid
        assert!(q.is_valid());

        // shouldn't be valid when not marked as ready
        q.set_ready(false);
        assert!(!q.ready());
        assert!(!q.is_valid());
        q.set_ready(true);

        // shouldn't be allowed to set a size > max_size
        q.set_size(q.max_size() << 1);
        assert_eq!(q.state.size, q.max_size());

        // or set the size to 0
        q.set_size(0);
        assert_eq!(q.state.size, q.max_size());

        // or set a size which is not a power of 2
        q.set_size(11);
        assert_eq!(q.state.size, q.max_size());

        // but should be allowed to set a size if 0 < size <= max_size and size is a power of two
        q.set_size(4);
        assert_eq!(q.state.size, 4);
        q.state.size = q.max_size();

        // shouldn't be allowed to set an address that breaks the alignment constraint
        q.set_desc_table_address(Some(0xf), None);
        assert_eq!(q.state.desc_table.0, vq.desc_table_addr().0);
        // should be allowed to set an aligned out of bounds address
        q.set_desc_table_address(Some(0xffff_fff0), None);
        assert_eq!(q.state.desc_table.0, 0xffff_fff0);
        // but shouldn't be valid
        assert!(!q.is_valid());
        // but should be allowed to set a valid description table address
        q.set_desc_table_address(Some(0x10), None);
        assert_eq!(q.state.desc_table.0, 0x10);
        assert!(q.is_valid());
        let addr = vq.desc_table_addr().0;
        q.set_desc_table_address(Some(addr as u32), Some((addr >> 32) as u32));

        // shouldn't be allowed to set an address that breaks the alignment constraint
        q.set_avail_ring_address(Some(0x1), None);
        assert_eq!(q.state.avail_ring.0, vq.avail_addr().0);
        // should be allowed to set an aligned out of bounds address
        q.set_avail_ring_address(Some(0xffff_fffe), None);
        assert_eq!(q.state.avail_ring.0, 0xffff_fffe);
        // but shouldn't be valid
        assert!(!q.is_valid());
        // but should be allowed to set a valid available ring address
        q.set_avail_ring_address(Some(0x2), None);
        assert_eq!(q.state.avail_ring.0, 0x2);
        assert!(q.is_valid());
        let addr = vq.avail_addr().0;
        q.set_avail_ring_address(Some(addr as u32), Some((addr >> 32) as u32));

        // shouldn't be allowed to set an address that breaks the alignment constraint
        q.set_used_ring_address(Some(0x3), None);
        assert_eq!(q.state.used_ring.0, vq.used_addr().0);
        // should be allowed to set an aligned out of bounds address
        q.set_used_ring_address(Some(0xffff_fffc), None);
        assert_eq!(q.state.used_ring.0, 0xffff_fffc);
        // but shouldn't be valid
        assert!(!q.is_valid());
        // but should be allowed to set a valid used ring address
        q.set_used_ring_address(Some(0x4), None);
        assert_eq!(q.state.used_ring.0, 0x4);
        let addr = vq.used_addr().0;
        q.set_used_ring_address(Some(addr as u32), Some((addr >> 32) as u32));
        assert!(q.is_valid());
    }

    #[test]
    fn test_add_used() {
        let m = &GuestMemoryMmap::<()>::from_ranges(&[(GuestAddress(0), 0x10000)]).unwrap();
        let vq = MockSplitQueue::new(m, 16);
        let mut q = vq.create_queue(m);

        assert_eq!(q.used_idx(Ordering::Acquire).unwrap(), Wrapping(0));
        assert_eq!(u16::from_le(vq.used().idx().load()), 0);

        // index too large
        assert!(q.add_used(16, 0x1000).is_err());
        assert_eq!(u16::from_le(vq.used().idx().load()), 0);

        // should be ok
        q.add_used(1, 0x1000).unwrap();
        assert_eq!(q.state.next_used, Wrapping(1));
        assert_eq!(q.used_idx(Ordering::Acquire).unwrap(), Wrapping(1));
        assert_eq!(u16::from_le(vq.used().idx().load()), 1);

        let x = vq.used().ring().ref_at(0).unwrap().load();
        assert_eq!(x.id(), 1);
        assert_eq!(x.len(), 0x1000);
    }

    #[test]
    fn test_reset_queue() {
        let m = &GuestMemoryMmap::<()>::from_ranges(&[(GuestAddress(0), 0x10000)]).unwrap();
        let vq = MockSplitQueue::new(m, 16);
        let mut q = vq.create_queue(m);

        q.set_size(8);
        // The address set by `MockSplitQueue` for the descriptor table is DEFAULT_DESC_TABLE_ADDR,
        // so let's change it for testing the reset.
        q.set_desc_table_address(Some(0x5000), None);
        // Same for `event_idx_enabled`, `next_avail` `next_used` and `signalled_used`.
        q.set_event_idx(true);
        q.set_next_avail(2);
        q.set_next_used(4);
        q.state.num_added = Wrapping(15);
        assert_eq!(q.state.size, 8);
        // `create_queue` also marks the queue as ready.
        assert!(q.state.ready);
        assert_ne!(q.state.desc_table, GuestAddress(DEFAULT_DESC_TABLE_ADDR));
        assert_ne!(q.state.avail_ring, GuestAddress(DEFAULT_AVAIL_RING_ADDR));
        assert_ne!(q.state.used_ring, GuestAddress(DEFAULT_USED_RING_ADDR));
        assert_ne!(q.state.next_avail, Wrapping(0));
        assert_ne!(q.state.next_used, Wrapping(0));
        assert_ne!(q.state.num_added, Wrapping(0));
        assert!(q.state.event_idx_enabled);

        q.reset();
        assert_eq!(q.state.size, 16);
        assert!(!q.state.ready);
        assert_eq!(q.state.desc_table, GuestAddress(DEFAULT_DESC_TABLE_ADDR));
        assert_eq!(q.state.avail_ring, GuestAddress(DEFAULT_AVAIL_RING_ADDR));
        assert_eq!(q.state.used_ring, GuestAddress(DEFAULT_USED_RING_ADDR));
        assert_eq!(q.state.next_avail, Wrapping(0));
        assert_eq!(q.state.next_used, Wrapping(0));
        assert_eq!(q.state.num_added, Wrapping(0));
        assert!(!q.state.event_idx_enabled);
    }

    #[test]
    fn test_needs_notification() {
        let m = &GuestMemoryMmap::<()>::from_ranges(&[(GuestAddress(0), 0x10000)]).unwrap();
        let qsize = 16;
        let vq = MockSplitQueue::new(m, qsize);
        let mut q = vq.create_queue(m);
        let avail_addr = vq.avail_addr();

        // It should always return true when EVENT_IDX isn't enabled.
        for i in 0..qsize {
            q.state.next_used = Wrapping(i);
            assert!(q.needs_notification().unwrap());
        }

        m.write_obj::<u16>(
            u16::to_le(4),
            avail_addr.unchecked_add(4 + qsize as u64 * 2),
        )
        .unwrap();
        q.state.set_event_idx(true);

        // Incrementing up to this value causes an `u16` to wrap back to 0.
        let wrap = u32::from(u16::MAX) + 1;

        for i in 0..wrap + 12 {
            q.state.next_used = Wrapping(i as u16);
            // `num_added` needs to be at least `1` to represent the fact that new descriptor
            // chains have be added to the used ring since the last time `needs_notification`
            // returned.
            q.state.num_added = Wrapping(1);
            // Let's test wrapping around the maximum index value as well.
            let expected = i == 5 || i == (5 + wrap);
            assert_eq!((q.needs_notification().unwrap(), i), (expected, i));
        }

        m.write_obj::<u16>(
            u16::to_le(8),
            avail_addr.unchecked_add(4 + qsize as u64 * 2),
        )
        .unwrap();

        // Returns `false` because the current `used_event` value is behind both `next_used` and
        // the value of `next_used` at the time when `needs_notification` last returned (which is
        // computed based on `num_added` as described in the comments for `needs_notification`.
        assert!(!q.needs_notification().unwrap());

        m.write_obj::<u16>(
            u16::to_le(15),
            avail_addr.unchecked_add(4 + qsize as u64 * 2),
        )
        .unwrap();

        q.state.num_added = Wrapping(1);
        assert!(!q.needs_notification().unwrap());

        q.state.next_used = Wrapping(15);
        q.state.num_added = Wrapping(1);
        assert!(!q.needs_notification().unwrap());

        q.state.next_used = Wrapping(16);
        q.state.num_added = Wrapping(1);
        assert!(q.needs_notification().unwrap());

        // Calling `needs_notification` again immediately returns `false`.
        assert!(!q.needs_notification().unwrap());

        m.write_obj::<u16>(
            u16::to_le(u16::MAX - 3),
            avail_addr.unchecked_add(4 + qsize as u64 * 2),
        )
        .unwrap();
        q.state.next_used = Wrapping(u16::MAX - 2);
        q.state.num_added = Wrapping(1);
        // Returns `true` because, when looking at circular sequence of indices of the used ring,
        // the value we wrote in the `used_event` appears between the "old" value of `next_used`
        // (i.e. `next_used` - `num_added`) and the current `next_used`, thus suggesting that we
        // need to notify the driver.
        assert!(q.needs_notification().unwrap());
    }

    #[test]
    fn test_enable_disable_notification() {
        let m = &GuestMemoryMmap::<()>::from_ranges(&[(GuestAddress(0), 0x10000)]).unwrap();
        let vq = MockSplitQueue::new(m, 16);

        let mut q = vq.create_queue(m);
        let used_addr = vq.used_addr();

        assert!(!q.state.event_idx_enabled);

        q.enable_notification().unwrap();
        let v = m.read_obj::<u16>(used_addr).map(u16::from_le).unwrap();
        assert_eq!(v, 0);

        q.disable_notification().unwrap();
        let v = m.read_obj::<u16>(used_addr).map(u16::from_le).unwrap();
        assert_eq!(v, VIRTQ_USED_F_NO_NOTIFY);

        q.enable_notification().unwrap();
        let v = m.read_obj::<u16>(used_addr).map(u16::from_le).unwrap();
        assert_eq!(v, 0);

        q.set_event_idx(true);
        let avail_addr = vq.avail_addr();
        m.write_obj::<u16>(u16::to_le(2), avail_addr.unchecked_add(2))
            .unwrap();

        assert!(q.enable_notification().unwrap());
        q.state.next_avail = Wrapping(2);
        assert!(!q.enable_notification().unwrap());

        m.write_obj::<u16>(u16::to_le(8), avail_addr.unchecked_add(2))
            .unwrap();

        assert!(q.enable_notification().unwrap());
        q.state.next_avail = Wrapping(8);
        assert!(!q.enable_notification().unwrap());
    }

    #[test]
    fn test_consume_chains_with_notif() {
        let m = &GuestMemoryMmap::<()>::from_ranges(&[(GuestAddress(0), 0x10000)]).unwrap();
        let vq = MockSplitQueue::new(m, 16);

        let mut q = vq.create_queue(m);

        // q is currently valid.
        assert!(q.is_valid());

        // The chains are (0, 1), (2, 3, 4), (5, 6), (7, 8), (9, 10, 11, 12).
        for i in 0..13 {
            let flags = match i {
                1 | 4 | 6 | 8 | 12 => 0,
                _ => VIRTQ_DESC_F_NEXT,
            };

            let desc = Descriptor::new((0x1000 * (i + 1)) as u64, 0x1000, flags, i + 1);
            vq.desc_table().store(i, desc).unwrap();
        }

        vq.avail().ring().ref_at(0).unwrap().store(u16::to_le(0));
        vq.avail().ring().ref_at(1).unwrap().store(u16::to_le(2));
        vq.avail().ring().ref_at(2).unwrap().store(u16::to_le(5));
        vq.avail().ring().ref_at(3).unwrap().store(u16::to_le(7));
        vq.avail().ring().ref_at(4).unwrap().store(u16::to_le(9));
        // Let the device know it can consume chains with the index < 2.
        vq.avail().idx().store(u16::to_le(2));
        // No descriptor chains are consumed at this point.
        assert_eq!(q.next_avail(), 0);

        let mut i = 0;

        loop {
            i += 1;
            q.disable_notification().unwrap();

            while let Some(chain) = q.iter().unwrap().next() {
                // Process the descriptor chain, and then add entries to the
                // used ring.
                let head_index = chain.head_index();
                let mut desc_len = 0;
                chain.for_each(|d| {
                    if d.flags() & VIRTQ_DESC_F_WRITE == VIRTQ_DESC_F_WRITE {
                        desc_len += d.len();
                    }
                });
                q.add_used(head_index, desc_len).unwrap();
            }
            if !q.enable_notification().unwrap() {
                break;
            }
        }
        // The chains should be consumed in a single loop iteration because there's nothing updating
        // the `idx` field of the available ring in the meantime.
        assert_eq!(i, 1);
        // The next chain that can be consumed should have index 2.
        assert_eq!(q.next_avail(), 2);
        assert_eq!(q.next_used(), 2);
        // Let the device know it can consume one more chain.
        vq.avail().idx().store(u16::to_le(3));
        i = 0;

        loop {
            i += 1;
            q.disable_notification().unwrap();

            while let Some(chain) = q.iter().unwrap().next() {
                // Process the descriptor chain, and then add entries to the
                // used ring.
                let head_index = chain.head_index();
                let mut desc_len = 0;
                chain.for_each(|d| {
                    if d.flags() & VIRTQ_DESC_F_WRITE == VIRTQ_DESC_F_WRITE {
                        desc_len += d.len();
                    }
                });
                q.add_used(head_index, desc_len).unwrap();
            }

            // For the simplicity of the test we are updating here the `idx` value of the available
            // ring. Ideally this should be done on a separate thread.
            // Because of this update, the loop should be iterated again to consume the new
            // available descriptor chains.
            vq.avail().idx().store(u16::to_le(4));
            if !q.enable_notification().unwrap() {
                break;
            }
        }
        assert_eq!(i, 2);
        // The next chain that can be consumed should have index 4.
        assert_eq!(q.next_avail(), 4);
        assert_eq!(q.next_used(), 4);

        // Set an `idx` that is bigger than the number of entries added in the ring.
        // This is an allowed scenario, but the indexes of the chain will have unexpected values.
        vq.avail().idx().store(u16::to_le(7));
        loop {
            q.disable_notification().unwrap();

            while let Some(chain) = q.iter().unwrap().next() {
                // Process the descriptor chain, and then add entries to the
                // used ring.
                let head_index = chain.head_index();
                let mut desc_len = 0;
                chain.for_each(|d| {
                    if d.flags() & VIRTQ_DESC_F_WRITE == VIRTQ_DESC_F_WRITE {
                        desc_len += d.len();
                    }
                });
                q.add_used(head_index, desc_len).unwrap();
            }
            if !q.enable_notification().unwrap() {
                break;
            }
        }
        assert_eq!(q.next_avail(), 7);
        assert_eq!(q.next_used(), 7);
    }

    #[test]
    fn test_invalid_avail_idx() {
        // This is a negative test for the following MUST from the spec: `A driver MUST NOT
        // decrement the available idx on a virtqueue (ie. there is no way to “unexpose” buffers).`.
        // We validate that for this misconfiguration, the device does not panic.
        let m = &GuestMemoryMmap::<()>::from_ranges(&[(GuestAddress(0), 0x10000)]).unwrap();
        let vq = MockSplitQueue::new(m, 16);

        let mut q = vq.create_queue(m);

        // q is currently valid.
        assert!(q.is_valid());

        // The chains are (0, 1), (2, 3, 4), (5, 6).
        for i in 0..7 {
            let flags = match i {
                1 | 4 | 6 => 0,
                _ => VIRTQ_DESC_F_NEXT,
            };

            let desc = Descriptor::new((0x1000 * (i + 1)) as u64, 0x1000, flags, i + 1);
            vq.desc_table().store(i, desc).unwrap();
        }

        vq.avail().ring().ref_at(0).unwrap().store(u16::to_le(0));
        vq.avail().ring().ref_at(1).unwrap().store(u16::to_le(2));
        vq.avail().ring().ref_at(2).unwrap().store(u16::to_le(5));
        // Let the device know it can consume chains with the index < 2.
        vq.avail().idx().store(u16::to_le(3));
        // No descriptor chains are consumed at this point.
        assert_eq!(q.next_avail(), 0);
        assert_eq!(q.next_used(), 0);

        loop {
            q.disable_notification().unwrap();

            while let Some(chain) = q.iter().unwrap().next() {
                // Process the descriptor chain, and then add entries to the
                // used ring.
                let head_index = chain.head_index();
                let mut desc_len = 0;
                chain.for_each(|d| {
                    if d.flags() & VIRTQ_DESC_F_WRITE == VIRTQ_DESC_F_WRITE {
                        desc_len += d.len();
                    }
                });
                q.add_used(head_index, desc_len).unwrap();
            }
            if !q.enable_notification().unwrap() {
                break;
            }
        }
        // The next chain that can be consumed should have index 3.
        assert_eq!(q.next_avail(), 3);
        assert_eq!(q.avail_idx(Ordering::Acquire).unwrap(), Wrapping(3));
        assert_eq!(q.next_used(), 3);
        assert_eq!(q.used_idx(Ordering::Acquire).unwrap(), Wrapping(3));
        assert!(q.lock().ready());

        // Decrement `idx` which should be forbidden. We don't enforce this thing, but we should
        // test that we don't panic in case the driver decrements it.
        vq.avail().idx().store(u16::to_le(1));

        loop {
            q.disable_notification().unwrap();

            while let Some(_chain) = q.iter().unwrap().next() {
                // In a real use case, we would do something with the chain here.
            }

            if !q.enable_notification().unwrap() {
                break;
            }
        }
    }
}
