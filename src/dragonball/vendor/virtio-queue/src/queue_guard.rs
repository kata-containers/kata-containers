// Copyright (C) 2020-2021 Alibaba Cloud. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause

use std::num::Wrapping;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::Ordering;

use vm_memory::GuestMemory;

use crate::{AvailIter, DescriptorChain, Error, QueueState, QueueStateOwnedT, QueueStateT};

/// A guard object to exclusively access an `Queue` object.
///
/// The guard object holds an exclusive lock to the underlying `QueueState` object, with an
/// associated guest memory object. It helps to guarantee that the whole session is served
/// with the same guest memory object.
///
/// # Example
///
/// ```rust
/// use virtio_queue::{Queue, QueueState};
/// use vm_memory::{Bytes, GuestAddress, GuestAddressSpace, GuestMemoryMmap};
///
/// let m = GuestMemoryMmap::<()>::from_ranges(&[(GuestAddress(0), 0x10000)]).unwrap();
/// let mut queue = Queue::<&GuestMemoryMmap, QueueState>::new(&m, 1024);
/// let mut queue_guard = queue.lock_with_memory();
///
/// // First, the driver sets up the queue; this set up is done via writes on the bus (PCI, MMIO).
/// queue_guard.set_size(8);
/// queue_guard.set_desc_table_address(Some(0x1000), None);
/// queue_guard.set_avail_ring_address(Some(0x2000), None);
/// queue_guard.set_used_ring_address(Some(0x3000), None);
/// queue_guard.set_event_idx(true);
/// queue_guard.set_ready(true);
/// // The user should check if the queue is valid before starting to use it.
/// assert!(queue_guard.is_valid());
///
/// // Here the driver would add entries in the available ring and then update the `idx` field of
/// // the available ring (address = 0x2000 + 2).
/// m.write_obj(3, GuestAddress(0x2002));
///
/// loop {
///     queue_guard.disable_notification().unwrap();
///
///     // Consume entries from the available ring.
///     while let Some(chain) = queue_guard.iter().unwrap().next() {
///         // Process the descriptor chain, and then add an entry in the used ring and optionally
///         // notify the driver.
///         queue_guard.add_used(chain.head_index(), 0x100).unwrap();
///
///         if queue_guard.needs_notification().unwrap() {
///             // Here we would notify the driver it has new entries in the used ring to consume.
///         }
///     }
///     if !queue_guard.enable_notification().unwrap() {
///         break;
///     }
/// }
/// ```
pub struct QueueGuard<M, S> {
    state: S,
    mem: M,
}

impl<M, S> QueueGuard<M, S>
where
    M: Deref + Clone,
    M::Target: GuestMemory + Sized,
    S: DerefMut<Target = QueueState>,
{
    /// Create a new instance of `QueueGuard`.
    pub fn new(state: S, mem: M) -> Self {
        QueueGuard { state, mem }
    }

    /// Check whether the queue configuration is valid.
    pub fn is_valid(&self) -> bool {
        self.state.is_valid(self.mem.deref())
    }

    /// Reset the queue to the initial state.
    pub fn reset(&mut self) {
        self.state.reset()
    }

    /// Get the maximum size of the virtio queue.
    pub fn max_size(&self) -> u16 {
        self.state.max_size()
    }

    /// Configure the queue size for the virtio queue.
    pub fn set_size(&mut self, size: u16) {
        self.state.set_size(size);
    }

    /// Check whether the queue is ready to be processed.
    pub fn ready(&self) -> bool {
        self.state.ready()
    }

    /// Configure the queue to `ready for processing` state.
    pub fn set_ready(&mut self, ready: bool) {
        self.state.set_ready(ready)
    }

    /// Set the descriptor table address for the queue.
    ///
    /// The descriptor table address is 64-bit, the corresponding part will be updated if 'low'
    /// and/or `high` is `Some` and valid.
    pub fn set_desc_table_address(&mut self, low: Option<u32>, high: Option<u32>) {
        self.state.set_desc_table_address(low, high);
    }

    /// Set the available ring address for the queue.
    ///
    /// The available ring address is 64-bit, the corresponding part will be updated if 'low'
    /// and/or `high` is `Some` and valid.
    pub fn set_avail_ring_address(&mut self, low: Option<u32>, high: Option<u32>) {
        self.state.set_avail_ring_address(low, high);
    }

    /// Set the used ring address for the queue.
    ///
    /// The used ring address is 64-bit, the corresponding part will be updated if 'low'
    /// and/or `high` is `Some` and valid.
    pub fn set_used_ring_address(&mut self, low: Option<u32>, high: Option<u32>) {
        self.state.set_used_ring_address(low, high);
    }

    /// Enable/disable the VIRTIO_F_RING_EVENT_IDX feature for interrupt coalescing.
    pub fn set_event_idx(&mut self, enabled: bool) {
        self.state.set_event_idx(enabled)
    }

    /// Read the `idx` field from the available ring.
    pub fn avail_idx(&self, order: Ordering) -> Result<Wrapping<u16>, Error> {
        self.state.avail_idx(self.mem.deref(), order)
    }

    /// Read the `idx` field from the used ring.
    pub fn used_idx(&self, order: Ordering) -> Result<Wrapping<u16>, Error> {
        self.state.used_idx(self.mem.deref(), order)
    }

    /// Put a used descriptor head into the used ring.
    pub fn add_used(&mut self, head_index: u16, len: u32) -> Result<(), Error> {
        self.state.add_used(self.mem.deref(), head_index, len)
    }

    /// Enable notification events from the guest driver.
    ///
    /// Return true if one or more descriptors can be consumed from the available ring after
    /// notifications were enabled (and thus it's possible there will be no corresponding
    /// notification).
    pub fn enable_notification(&mut self) -> Result<bool, Error> {
        self.state.enable_notification(self.mem.deref())
    }

    /// Disable notification events from the guest driver.
    pub fn disable_notification(&mut self) -> Result<(), Error> {
        self.state.disable_notification(self.mem.deref())
    }

    /// Check whether a notification to the guest is needed.
    ///
    /// Please note this method has side effects: once it returns `true`, it considers the
    /// driver will actually be notified, remember the associated index in the used ring, and
    /// won't return `true` again until the driver updates `used_event` and/or the notification
    /// conditions hold once more.
    pub fn needs_notification(&mut self) -> Result<bool, Error> {
        self.state.needs_notification(self.mem.deref())
    }

    /// Return the index of the next entry in the available ring.
    pub fn next_avail(&self) -> u16 {
        self.state.next_avail()
    }

    /// Return the index of the next entry in the used ring.
    pub fn next_used(&self) -> u16 {
        self.state.next_used()
    }

    /// Set the index of the next entry in the available ring.
    pub fn set_next_avail(&mut self, next_avail: u16) {
        self.state.set_next_avail(next_avail);
    }

    /// Set the index of the next entry in the used ring.
    pub fn set_next_used(&mut self, next_used: u16) {
        self.state.set_next_used(next_used);
    }

    /// Pop and return the next available descriptor chain, or `None` when there are no more
    /// descriptor chains available.
    pub fn pop_descriptor_chain(&mut self) -> Option<DescriptorChain<M>> {
        self.state.pop_descriptor_chain(self.mem.clone())
    }

    /// Get a consuming iterator over all available descriptor chain heads offered by the driver.
    pub fn iter(&mut self) -> Result<AvailIter<'_, M>, Error> {
        self.state.deref_mut().iter(self.mem.clone())
    }

    /// Decrement the value of the next available index by one position.
    pub fn go_to_previous_position(&mut self) {
        self.state.go_to_previous_position();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::defs::{VIRTQ_DESC_F_NEXT, VIRTQ_DESC_F_WRITE};
    use crate::mock::MockSplitQueue;
    use crate::Descriptor;

    use vm_memory::{GuestAddress, GuestMemoryMmap};

    #[test]
    fn test_queue_guard_object() {
        let m = &GuestMemoryMmap::<()>::from_ranges(&[(GuestAddress(0), 0x10000)]).unwrap();
        let vq = MockSplitQueue::new(m, 0x100);
        let mut q = vq.create_queue(m);
        let mut g = q.lock_with_memory();

        // g is currently valid.
        assert!(g.is_valid());
        assert!(g.ready());
        assert_eq!(g.max_size(), 0x100);
        g.set_size(16);

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
        assert_eq!(g.next_avail(), 0);
        assert_eq!(g.next_used(), 0);

        loop {
            g.disable_notification().unwrap();

            while let Some(chain) = g.iter().unwrap().next() {
                // Process the descriptor chain, and then add entries to the
                // used ring.
                let head_index = chain.head_index();
                let mut desc_len = 0;
                chain.for_each(|d| {
                    if d.flags() & VIRTQ_DESC_F_WRITE == VIRTQ_DESC_F_WRITE {
                        desc_len += d.len();
                    }
                });
                g.add_used(head_index, desc_len).unwrap();
            }
            if !g.enable_notification().unwrap() {
                break;
            }
        }

        {
            g.go_to_previous_position();
            let mut chain = g.pop_descriptor_chain().unwrap();
            // The descriptor index of the head descriptor from the last available chain
            // defined above is equal to 5. The chain has two descriptors, the second with
            // the index equal to 6.
            assert_eq!(chain.head_index(), 5);

            let desc = chain.next().unwrap();
            assert!(desc.has_next());
            assert_eq!(desc.next(), 6);

            let desc = chain.next().unwrap();
            assert!(!desc.has_next());

            assert!(chain.next().is_none());
        }

        // The next chain that can be consumed should have index 3.

        assert_eq!(g.next_avail(), 3);
        assert_eq!(g.avail_idx(Ordering::Acquire).unwrap(), Wrapping(3));
        assert_eq!(g.next_used(), 3);
        assert_eq!(g.used_idx(Ordering::Acquire).unwrap(), Wrapping(3));
        assert!(g.ready());

        // Decrement `idx` which should be forbidden. We don't enforce this thing, but we should
        // test that we don't panic in case the driver decrements it.
        vq.avail().idx().store(1);

        loop {
            g.disable_notification().unwrap();

            while let Some(_chain) = g.iter().unwrap().next() {
                // In a real use case, we would do something with the chain here.
            }

            if !g.enable_notification().unwrap() {
                break;
            }
        }
    }
}
