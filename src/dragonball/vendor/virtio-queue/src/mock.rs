// Copyright 2020 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0 OR BSD-3-Clause

//! Utilities used by unit tests and benchmarks for mocking the driver side
//! of the virtio protocol.

use std::marker::PhantomData;
use std::mem::size_of;

use vm_memory::{
    Address, ByteValued, Bytes, GuestAddress, GuestAddressSpace, GuestMemory, GuestUsize,
};

use crate::defs::{
    VIRTQ_AVAIL_ELEMENT_SIZE, VIRTQ_AVAIL_RING_HEADER_SIZE, VIRTQ_DESC_F_INDIRECT,
    VIRTQ_DESC_F_NEXT,
};
use crate::{Descriptor, DescriptorChain, Queue, QueueState, VirtqUsedElem};
use std::fmt::{self, Debug, Display};

/// Mock related errors.
#[derive(Debug)]
pub enum MockError {
    /// Invalid Ref index
    InvalidIndex,
    /// Invalid next avail
    InvalidNextAvail,
}

impl Display for MockError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::MockError::*;

        match self {
            InvalidIndex => write!(
                f,
                "invalid index for pointing to an address in a region when defining a Ref object"
            ),
            InvalidNextAvail => write!(
                f,
                "invalid next available descriptor chain head in the queue"
            ),
        }
    }
}

impl std::error::Error for MockError {}

/// Wrapper struct used for accessing a particular address of a GuestMemory area.
pub struct Ref<'a, M, T> {
    mem: &'a M,
    addr: GuestAddress,
    phantom: PhantomData<*const T>,
}

impl<'a, M: GuestMemory, T: ByteValued> Ref<'a, M, T> {
    fn new(mem: &'a M, addr: GuestAddress) -> Self {
        Ref {
            mem,
            addr,
            phantom: PhantomData,
        }
    }

    /// Read an object of type T from the underlying memory found at self.addr.
    pub fn load(&self) -> T {
        self.mem.read_obj(self.addr).unwrap()
    }

    /// Write an object of type T from the underlying memory found at self.addr.
    pub fn store(&self, val: T) {
        self.mem.write_obj(val, self.addr).unwrap()
    }
}

/// Wrapper struct used for accessing a subregion of a GuestMemory area.
pub struct ArrayRef<'a, M, T> {
    mem: &'a M,
    addr: GuestAddress,
    len: usize,
    phantom: PhantomData<*const T>,
}

impl<'a, M: GuestMemory, T: ByteValued> ArrayRef<'a, M, T> {
    fn new(mem: &'a M, addr: GuestAddress, len: usize) -> Self {
        ArrayRef {
            mem,
            addr,
            len,
            phantom: PhantomData,
        }
    }

    /// Return a `Ref` object pointing to an address defined by a particular
    /// index offset in the region.
    pub fn ref_at(&self, index: usize) -> Result<Ref<'a, M, T>, MockError> {
        if index >= self.len {
            return Err(MockError::InvalidIndex);
        }

        let addr = self
            .addr
            .checked_add((index * size_of::<T>()) as u64)
            .unwrap();

        Ok(Ref::new(self.mem, addr))
    }
}

/// Represents a virtio queue ring. The only difference between the used and available rings,
/// is the ring element type.
pub struct SplitQueueRing<'a, M, T: ByteValued> {
    flags: Ref<'a, M, u16>,
    // The value stored here should more precisely be a `Wrapping<u16>`, but that would require a
    // `ByteValued` impl for this type, which is not provided in vm-memory. Implementing the trait
    // here would require defining a wrapper for `Wrapping<u16>` and that would be too much for a
    // mock framework that is only used in tests.
    idx: Ref<'a, M, u16>,
    ring: ArrayRef<'a, M, T>,
    // `used_event` for `AvailRing`, `avail_event` for `UsedRing`.
    event: Ref<'a, M, u16>,
}

impl<'a, M: GuestMemory, T: ByteValued> SplitQueueRing<'a, M, T> {
    /// Create a new `SplitQueueRing` instance
    pub fn new(mem: &'a M, base: GuestAddress, len: u16) -> Self {
        let event_addr = base
            .checked_add(4)
            .and_then(|a| a.checked_add((size_of::<u16>() * len as usize) as u64))
            .unwrap();

        let split_queue_ring = SplitQueueRing {
            flags: Ref::new(mem, base),
            idx: Ref::new(mem, base.checked_add(2).unwrap()),
            ring: ArrayRef::new(mem, base.checked_add(4).unwrap(), len as usize),
            event: Ref::new(mem, event_addr),
        };

        split_queue_ring.flags.store(0);
        split_queue_ring.idx.store(0);
        split_queue_ring.event.store(0);

        split_queue_ring
    }

    /// Return the starting address of the `SplitQueueRing`.
    pub fn start(&self) -> GuestAddress {
        self.ring.addr
    }

    /// Return the end address of the `SplitQueueRing`.
    pub fn end(&self) -> GuestAddress {
        self.start()
            .checked_add(self.ring.len as GuestUsize)
            .unwrap()
    }

    /// Return a reference to the idx field.
    pub fn idx(&self) -> &Ref<'a, M, u16> {
        &self.idx
    }

    /// Return a reference to the ring field.
    pub fn ring(&self) -> &ArrayRef<'a, M, T> {
        &self.ring
    }
}

/// The available ring is used by the driver to offer buffers to the device.
pub type AvailRing<'a, M> = SplitQueueRing<'a, M, u16>;
/// The used ring is where the device returns buffers once it is done with them.
pub type UsedRing<'a, M> = SplitQueueRing<'a, M, VirtqUsedElem>;

/// Refers to the buffers the driver is using for the device.
pub struct DescriptorTable<'a, M> {
    table: ArrayRef<'a, M, Descriptor>,
    len: u16,
    free_descriptors: Vec<u16>,
}

impl<'a, M: GuestMemory> DescriptorTable<'a, M> {
    /// Create a new `DescriptorTable` instance
    pub fn new(mem: &'a M, addr: GuestAddress, len: u16) -> Self {
        let table = ArrayRef::new(mem, addr, len as usize);
        let free_descriptors = (0..len).rev().collect();

        DescriptorTable {
            table,
            len,
            free_descriptors,
        }
    }

    /// Read one descriptor from the specified index.
    pub fn load(&self, index: u16) -> Result<Descriptor, MockError> {
        self.table
            .ref_at(index as usize)
            .map(|load_ref| load_ref.load())
    }

    /// Write one descriptor at the specified index.
    pub fn store(&self, index: u16, value: Descriptor) -> Result<(), MockError> {
        self.table
            .ref_at(index as usize)
            .map(|store_ref| store_ref.store(value))
    }

    /// Return the total size of the DescriptorTable in bytes.
    pub fn total_size(&self) -> u64 {
        (self.len as usize * size_of::<Descriptor>()) as u64
    }

    /// Create a chain of descriptors.
    pub fn build_chain(&mut self, len: u16) -> Result<u16, MockError> {
        let indices = self
            .free_descriptors
            .iter()
            .copied()
            .rev()
            .take(usize::from(len))
            .collect::<Vec<_>>();

        assert_eq!(indices.len(), len as usize);

        for (pos, index_value) in indices.iter().copied().enumerate() {
            // Addresses and lens constant for now.
            let mut desc = Descriptor::new(0x1000, 0x1000, 0, 0);

            // It's not the last descriptor in the chain.
            if pos < indices.len() - 1 {
                desc.set_flags(VIRTQ_DESC_F_NEXT);
                desc.set_next(indices[pos + 1]);
            } else {
                desc.set_flags(0);
            }
            self.store(index_value, desc)?;
        }

        Ok(indices[0])
    }
}

trait GuestAddressExt {
    fn align_up(&self, x: GuestUsize) -> GuestAddress;
}

impl GuestAddressExt for GuestAddress {
    fn align_up(&self, x: GuestUsize) -> GuestAddress {
        Self((self.0 + (x - 1)) & !(x - 1))
    }
}

/// A mock version of the virtio queue implemented from the perspective of the driver.
pub struct MockSplitQueue<'a, M> {
    mem: &'a M,
    len: u16,
    desc_table_addr: GuestAddress,
    desc_table: DescriptorTable<'a, M>,
    avail_addr: GuestAddress,
    avail: AvailRing<'a, M>,
    used_addr: GuestAddress,
    used: UsedRing<'a, M>,
    indirect_addr: GuestAddress,
}

impl<'a, M: GuestMemory> MockSplitQueue<'a, M> {
    /// Create a new `MockSplitQueue` instance with 0 as the default guest
    /// physical starting address.
    pub fn new(mem: &'a M, len: u16) -> Self {
        Self::create(mem, GuestAddress(0), len)
    }

    /// Create a new `MockSplitQueue` instance.
    pub fn create(mem: &'a M, start: GuestAddress, len: u16) -> Self {
        const AVAIL_ALIGN: GuestUsize = 2;
        const USED_ALIGN: GuestUsize = 4;

        let desc_table_addr = start;
        let desc_table = DescriptorTable::new(mem, desc_table_addr, len);

        let avail_addr = start
            .checked_add(16 * len as GuestUsize)
            .unwrap()
            .align_up(AVAIL_ALIGN);
        let avail = AvailRing::new(mem, avail_addr, len);

        let used_addr = avail.end().align_up(USED_ALIGN);
        let used = UsedRing::new(mem, used_addr, len);

        let indirect_addr = GuestAddress(0x3000_0000);

        MockSplitQueue {
            mem,
            len,
            desc_table_addr,
            desc_table,
            avail_addr,
            avail,
            used_addr,
            used,
            indirect_addr,
        }
    }

    /// Return the starting address of the queue.
    pub fn start(&self) -> GuestAddress {
        self.desc_table_addr
    }

    /// Return the end address of the queue.
    pub fn end(&self) -> GuestAddress {
        self.used.end()
    }

    /// Descriptor table accessor.
    pub fn desc_table(&self) -> &DescriptorTable<'a, M> {
        &self.desc_table
    }

    /// Available ring accessor.
    pub fn avail(&self) -> &AvailRing<M> {
        &self.avail
    }

    /// Used ring accessor.
    pub fn used(&self) -> &UsedRing<M> {
        &self.used
    }

    /// Return the starting address of the descriptor table.
    pub fn desc_table_addr(&self) -> GuestAddress {
        self.desc_table_addr
    }

    /// Return the starting address of the available ring.
    pub fn avail_addr(&self) -> GuestAddress {
        self.avail_addr
    }

    /// Return the starting address of the used ring.
    pub fn used_addr(&self) -> GuestAddress {
        self.used_addr
    }

    fn update_avail_idx(&mut self, value: u16) -> Result<(), MockError> {
        let avail_idx = self.avail.idx.load();
        self.avail.ring.ref_at(avail_idx as usize)?.store(value);
        self.avail.idx.store(avail_idx.wrapping_add(1));
        Ok(())
    }

    fn alloc_indirect_chain(&mut self, len: u16) -> Result<GuestAddress, MockError> {
        // To simplify things for now, we round up the table len as a multiple of 16. When this is
        // no longer the case, we should make sure the starting address of the descriptor table
        // we're  creating below is properly aligned.

        let table_len = if len % 16 == 0 {
            len
        } else {
            16 * (len / 16 + 1)
        };

        let mut table = DescriptorTable::new(self.mem, self.indirect_addr, table_len);
        let head_decriptor_index = table.build_chain(len)?;
        // When building indirect descriptor tables, the descriptor at index 0 is supposed to be
        // first in the resulting chain. Just making sure our logic actually makes that happen.
        assert_eq!(head_decriptor_index, 0);

        let table_addr = self.indirect_addr;
        self.indirect_addr = self.indirect_addr.checked_add(table.total_size()).unwrap();
        Ok(table_addr)
    }

    /// Add a descriptor chain to the table.
    pub fn add_chain(&mut self, len: u16) -> Result<(), MockError> {
        self.desc_table
            .build_chain(len)
            .and_then(|head_idx| self.update_avail_idx(head_idx))
    }

    /// Add an indirect descriptor chain to the table.
    pub fn add_indirect_chain(&mut self, len: u16) -> Result<(), MockError> {
        let head_idx = self.desc_table.build_chain(1)?;

        // We just allocate the indirect table and forget about it for now.
        let indirect_addr = self.alloc_indirect_chain(len)?;

        let mut desc = self.desc_table.load(head_idx)?;
        desc.set_flags(VIRTQ_DESC_F_INDIRECT);
        desc.set_addr(indirect_addr.raw_value());
        desc.set_len(u32::from(len) * size_of::<Descriptor>() as u32);

        self.desc_table.store(head_idx, desc)?;
        self.update_avail_idx(head_idx)
    }

    /// Creates a new `Queue`, using the underlying memory regions represented
    /// by the `MockSplitQueue`.
    pub fn create_queue<A: GuestAddressSpace>(&self, a: A) -> Queue<A, QueueState> {
        let mut q = Queue::<A, QueueState>::new(a, self.len);

        q.state.size = self.len;
        q.state.ready = true;
        q.state.desc_table = self.desc_table_addr;
        q.state.avail_ring = self.avail_addr;
        q.state.used_ring = self.used_addr;
        q
    }

    /// Writes multiple descriptor chains to the memory object of the queue, at the beginning of
    /// the descriptor table, and returns the first `DescriptorChain` available.
    pub fn build_multiple_desc_chains(
        &self,
        descs: &[Descriptor],
    ) -> Result<DescriptorChain<&M>, MockError> {
        self.add_desc_chains(descs, 0)?;
        self.create_queue(self.mem)
            .iter()
            .unwrap()
            .next()
            .ok_or(MockError::InvalidNextAvail)
    }

    /// Writes a single descriptor chain to the memory object of the queue, at the beginning of the
    /// descriptor table, and returns the associated `DescriptorChain` object.
    // This method ensures the next flags and values are set properly for the desired chain, but
    // keeps the other characteristics of the input descriptors (`addr`, `len`, other flags).
    pub fn build_desc_chain(&self, descs: &[Descriptor]) -> Result<DescriptorChain<&M>, MockError> {
        let mut modified_descs: Vec<Descriptor> = Vec::with_capacity(descs.len());
        for (idx, desc) in descs.iter().enumerate() {
            let (flags, next) = if idx == descs.len() - 1 {
                // Clear the NEXT flag if it was set. The value of the next field of the
                // Descriptor doesn't matter at this point.
                (desc.flags() & !VIRTQ_DESC_F_NEXT, 0)
            } else {
                // Ensure that the next flag is set and that we are referring the following
                // descriptor. This ignores any value is actually present in `desc.next`.
                (desc.flags() | VIRTQ_DESC_F_NEXT, idx as u16 + 1)
            };
            modified_descs.push(Descriptor::new(desc.addr().0, desc.len(), flags, next));
        }
        self.build_multiple_desc_chains(&modified_descs[..])
    }

    /// Adds descriptor chains to the memory object of the queue.
    // `descs` represents a slice of `Descriptor` objects which are used to populate the chains, and
    // `offset` is the index in the descriptor table where the chains should be added.
    // The descriptor chain related information is written in memory starting with address 0.
    // The `addr` fields of the input descriptors should start at a sufficiently
    // greater location (i.e. 1MiB, or `0x10_0000`).
    pub fn add_desc_chains(&self, descs: &[Descriptor], offset: u16) -> Result<(), MockError> {
        let mut new_entries = 0;
        let avail_idx: u16 = self
            .mem
            .read_obj(self.avail_addr().unchecked_add(2))
            .unwrap();

        for (idx, desc) in descs.iter().enumerate() {
            let i = idx as u16 + offset;
            self.desc_table().store(i, *desc)?;

            if idx == 0 || descs[idx - 1].flags() & VIRTQ_DESC_F_NEXT != 1 {
                // Update the available ring position.
                self.mem
                    .write_obj(
                        i,
                        self.avail_addr().unchecked_add(
                            VIRTQ_AVAIL_RING_HEADER_SIZE
                                + (avail_idx + new_entries) as u64 * VIRTQ_AVAIL_ELEMENT_SIZE,
                        ),
                    )
                    .unwrap();
                new_entries += 1;
            }
        }

        // Increment `avail_idx`.
        self.mem
            .write_obj(avail_idx + new_entries, self.avail_addr().unchecked_add(2))
            .unwrap();

        Ok(())
    }
}
