// Copyright (C) 2021 Alibaba Cloud. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause

use std::num::Wrapping;
use std::ops::Deref;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex, MutexGuard};

use vm_memory::GuestMemory;

use crate::{DescriptorChain, Error, QueueState, QueueStateGuard, QueueStateT};

/// Struct to maintain information and manipulate state of a virtio queue for multi-threaded
/// context.
///
/// # Example
///
/// ```rust
/// use virtio_queue::{Queue, QueueState, QueueStateSync, QueueStateT};
/// use vm_memory::{Bytes, GuestAddress, GuestAddressSpace, GuestMemoryMmap};
///
/// let m = &GuestMemoryMmap::<()>::from_ranges(&[(GuestAddress(0), 0x10000)]).unwrap();
/// let mut queue = QueueStateSync::new(1024);
///
/// // First, the driver sets up the queue; this set up is done via writes on the bus (PCI, MMIO).
/// queue.set_size(8);
/// queue.set_desc_table_address(Some(0x1000), None);
/// queue.set_avail_ring_address(Some(0x2000), None);
/// queue.set_used_ring_address(Some(0x3000), None);
/// queue.set_ready(true);
/// // The user should check if the queue is valid before starting to use it.
/// assert!(queue.is_valid(m.memory()));
///
/// // The memory object is not embedded in the `QueueStateSync`, so we have to pass it as a
/// // parameter to the methods that access the guest memory. Examples would be:
/// queue.add_used(m.memory(), 1, 0x100).unwrap();
/// queue.needs_notification(m.memory()).unwrap();
/// ```
#[derive(Clone, Debug)]
pub struct QueueStateSync {
    state: Arc<Mutex<QueueState>>,
}

impl QueueStateSync {
    fn lock_state(&self) -> MutexGuard<QueueState> {
        // Do not expect poisoned lock.
        self.state.lock().unwrap()
    }
}

impl<'a> QueueStateGuard<'a> for QueueStateSync {
    type G = MutexGuard<'a, QueueState>;
}

impl QueueStateT for QueueStateSync {
    fn new(max_size: u16) -> Self {
        QueueStateSync {
            state: Arc::new(Mutex::new(QueueState::new(max_size))),
        }
    }

    fn is_valid<M: GuestMemory>(&self, mem: &M) -> bool {
        self.lock_state().is_valid(mem)
    }

    fn reset(&mut self) {
        self.lock_state().reset();
    }

    fn lock(&mut self) -> <Self as QueueStateGuard>::G {
        self.lock_state()
    }

    fn max_size(&self) -> u16 {
        self.lock_state().max_size()
    }

    fn size(&self) -> u16 {
        self.lock_state().size()
    }

    fn set_size(&mut self, size: u16) {
        self.lock_state().set_size(size);
    }

    fn ready(&self) -> bool {
        self.lock_state().ready
    }

    fn set_ready(&mut self, ready: bool) {
        self.lock_state().set_ready(ready)
    }

    fn set_desc_table_address(&mut self, low: Option<u32>, high: Option<u32>) {
        self.lock_state().set_desc_table_address(low, high);
    }

    fn set_avail_ring_address(&mut self, low: Option<u32>, high: Option<u32>) {
        self.lock_state().set_avail_ring_address(low, high);
    }

    fn set_used_ring_address(&mut self, low: Option<u32>, high: Option<u32>) {
        self.lock_state().set_used_ring_address(low, high);
    }

    fn set_event_idx(&mut self, enabled: bool) {
        self.lock_state().set_event_idx(enabled);
    }

    fn avail_idx<M>(&self, mem: &M, order: Ordering) -> Result<Wrapping<u16>, Error>
    where
        M: GuestMemory + ?Sized,
    {
        self.lock_state().avail_idx(mem, order)
    }

    fn used_idx<M: GuestMemory>(&self, mem: &M, order: Ordering) -> Result<Wrapping<u16>, Error> {
        self.lock_state().used_idx(mem, order)
    }

    fn add_used<M: GuestMemory>(
        &mut self,
        mem: &M,
        head_index: u16,
        len: u32,
    ) -> Result<(), Error> {
        self.lock_state().add_used(mem, head_index, len)
    }

    fn enable_notification<M: GuestMemory>(&mut self, mem: &M) -> Result<bool, Error> {
        self.lock_state().enable_notification(mem)
    }

    fn disable_notification<M: GuestMemory>(&mut self, mem: &M) -> Result<(), Error> {
        self.lock_state().disable_notification(mem)
    }

    fn needs_notification<M: GuestMemory>(&mut self, mem: &M) -> Result<bool, Error> {
        self.lock_state().needs_notification(mem)
    }

    fn next_avail(&self) -> u16 {
        self.lock_state().next_avail()
    }

    fn set_next_avail(&mut self, next_avail: u16) {
        self.lock_state().set_next_avail(next_avail);
    }

    fn next_used(&self) -> u16 {
        self.lock_state().next_used()
    }

    fn set_next_used(&mut self, next_used: u16) {
        self.lock_state().set_next_used(next_used);
    }

    fn pop_descriptor_chain<M>(&mut self, mem: M) -> Option<DescriptorChain<M>>
    where
        M: Clone + Deref,
        M::Target: GuestMemory,
    {
        self.lock_state().pop_descriptor_chain(mem)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::defs::{
        DEFAULT_AVAIL_RING_ADDR, DEFAULT_DESC_TABLE_ADDR, DEFAULT_USED_RING_ADDR,
        VIRTQ_USED_F_NO_NOTIFY,
    };
    use std::sync::Barrier;
    use vm_memory::{Address, Bytes, GuestAddress, GuestAddressSpace, GuestMemoryMmap};

    #[test]
    fn test_queue_state_sync() {
        let mut q = QueueStateSync::new(0x1000);
        let mut q2 = q.clone();
        let q3 = q.clone();
        let barrier = Arc::new(Barrier::new(3));
        let b2 = barrier.clone();
        let b3 = barrier.clone();

        let t1 = std::thread::spawn(move || {
            {
                let guard = q2.lock();
                assert!(!guard.ready());
            }
            b2.wait();
            b2.wait();
            {
                let guard = q2.lock();
                assert!(guard.ready());
            }
        });

        let t2 = std::thread::spawn(move || {
            assert!(!q3.ready());
            b3.wait();
            b3.wait();
            assert!(q3.ready());
        });

        barrier.wait();
        q.set_ready(true);
        barrier.wait();

        t1.join().unwrap();
        t2.join().unwrap();
    }

    #[test]
    fn test_state_sync_add_used() {
        let m = &GuestMemoryMmap::<()>::from_ranges(&[(GuestAddress(0), 0x10000)]).unwrap();
        let mut q = QueueStateSync::new(0x100);

        q.set_desc_table_address(Some(0x1000), None);
        q.set_avail_ring_address(Some(0x2000), None);
        q.set_used_ring_address(Some(0x3000), None);
        q.set_event_idx(true);
        q.set_ready(true);
        assert!(q.is_valid(m.memory()));
        assert_eq!(q.lock().size, 0x100);

        assert_eq!(q.max_size(), 0x100);
        assert_eq!(q.size(), 0x100);
        q.set_size(0x80);
        assert_eq!(q.size(), 0x80);
        assert_eq!(q.max_size(), 0x100);
        q.set_next_avail(5);
        assert_eq!(q.next_avail(), 5);
        q.set_next_used(3);
        assert_eq!(q.next_used(), 3);
        assert_eq!(
            q.avail_idx(m.memory(), Ordering::Acquire).unwrap(),
            Wrapping(0)
        );
        assert_eq!(
            q.used_idx(m.memory(), Ordering::Acquire).unwrap(),
            Wrapping(0)
        );

        assert_eq!(q.next_used(), 3);

        // index too large
        assert!(q.add_used(m.memory(), 0x200, 0x1000).is_err());
        assert_eq!(q.next_used(), 3);

        // should be ok
        q.add_used(m.memory(), 1, 0x1000).unwrap();
        assert_eq!(q.next_used(), 4);
        assert_eq!(
            q.used_idx(m.memory(), Ordering::Acquire).unwrap(),
            Wrapping(4)
        );
    }

    #[test]
    fn test_sync_state_reset_queue() {
        let m = &GuestMemoryMmap::<()>::from_ranges(&[(GuestAddress(0), 0x10000)]).unwrap();
        let mut q = QueueStateSync::new(0x100);

        q.set_desc_table_address(Some(0x1000), None);
        q.set_avail_ring_address(Some(0x2000), None);
        q.set_used_ring_address(Some(0x3000), None);
        q.set_event_idx(true);
        q.set_next_avail(2);
        q.set_next_used(2);
        q.set_size(0x8);
        q.set_ready(true);
        assert!(q.is_valid(m.memory()));

        q.needs_notification(m.memory()).unwrap();

        assert_eq!(q.lock_state().size, 0x8);
        assert!(q.lock_state().ready);
        assert_ne!(
            q.lock_state().desc_table,
            GuestAddress(DEFAULT_DESC_TABLE_ADDR)
        );
        assert_ne!(
            q.lock_state().avail_ring,
            GuestAddress(DEFAULT_AVAIL_RING_ADDR)
        );
        assert_ne!(
            q.lock_state().used_ring,
            GuestAddress(DEFAULT_USED_RING_ADDR)
        );
        assert_ne!(q.lock_state().next_avail, Wrapping(0));
        assert_ne!(q.lock_state().next_used, Wrapping(0));
        assert_eq!(q.lock_state().num_added, Wrapping(0));
        assert!(q.lock_state().event_idx_enabled);

        q.reset();
        assert_eq!(q.lock_state().size, 0x100);
        assert!(!q.lock_state().ready);
        assert_eq!(
            q.lock_state().desc_table,
            GuestAddress(DEFAULT_DESC_TABLE_ADDR)
        );
        assert_eq!(
            q.lock_state().avail_ring,
            GuestAddress(DEFAULT_AVAIL_RING_ADDR)
        );
        assert_eq!(
            q.lock_state().used_ring,
            GuestAddress(DEFAULT_USED_RING_ADDR)
        );
        assert_eq!(q.lock_state().next_avail, Wrapping(0));
        assert_eq!(q.lock_state().next_used, Wrapping(0));
        assert_eq!(q.lock_state().num_added, Wrapping(0));
        assert!(!q.lock_state().event_idx_enabled);
    }

    #[test]
    fn test_enable_disable_notification() {
        let m = &GuestMemoryMmap::<()>::from_ranges(&[(GuestAddress(0), 0x10000)]).unwrap();
        let mem = m.memory();
        let mut q = QueueStateSync::new(0x100);

        q.set_desc_table_address(Some(0x1000), None);
        q.set_avail_ring_address(Some(0x2000), None);
        q.set_used_ring_address(Some(0x3000), None);
        q.set_ready(true);
        assert!(q.is_valid(mem));

        let used_addr = q.lock_state().used_ring;

        assert!(!q.lock_state().event_idx_enabled);
        q.enable_notification(mem).unwrap();
        let v = m.read_obj::<u16>(used_addr).map(u16::from_le).unwrap();
        assert_eq!(v, 0);

        q.disable_notification(m.memory()).unwrap();
        let v = m.read_obj::<u16>(used_addr).map(u16::from_le).unwrap();
        assert_eq!(v, VIRTQ_USED_F_NO_NOTIFY);

        q.enable_notification(mem).unwrap();
        let v = m.read_obj::<u16>(used_addr).map(u16::from_le).unwrap();
        assert_eq!(v, 0);

        q.set_event_idx(true);
        let avail_addr = q.lock_state().avail_ring;
        m.write_obj::<u16>(u16::to_le(2), avail_addr.unchecked_add(2))
            .unwrap();

        assert!(q.enable_notification(mem).unwrap());
        q.lock_state().next_avail = Wrapping(2);
        assert!(!q.enable_notification(mem).unwrap());

        m.write_obj::<u16>(u16::to_le(8), avail_addr.unchecked_add(2))
            .unwrap();

        assert!(q.enable_notification(mem).unwrap());
        q.lock_state().next_avail = Wrapping(8);
        assert!(!q.enable_notification(mem).unwrap());
    }
}
