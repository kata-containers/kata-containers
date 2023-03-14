// Copyright (C) 2019 Alibaba Cloud Computing. All rights reserved.
// Copyright (C) 2020 Red Hat, Inc. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! A wrapper over an `ArcSwap<GuestMemory>` struct to support RCU-style mutability.
//!
//! With the `backend-atomic` feature enabled, simply replacing `GuestMemoryMmap`
//! with `GuestMemoryAtomic<GuestMemoryMmap>` will enable support for mutable memory maps.
//! To support mutable memory maps, devices will also need to use
//! `GuestAddressSpace::memory()` to gain temporary access to guest memory.

extern crate arc_swap;

use arc_swap::{ArcSwap, Guard};
use std::ops::Deref;
use std::sync::{Arc, LockResult, Mutex, MutexGuard, PoisonError};

use crate::{GuestAddressSpace, GuestMemory};

/// A fast implementation of a mutable collection of memory regions.
///
/// This implementation uses `ArcSwap` to provide RCU-like snapshotting of the memory map:
/// every update of the memory map creates a completely new `GuestMemory` object, and
/// readers will not be blocked because the copies they retrieved will be collected once
/// no one can access them anymore.  Under the assumption that updates to the memory map
/// are rare, this allows a very efficient implementation of the `memory()` method.
#[derive(Clone, Debug)]
pub struct GuestMemoryAtomic<M: GuestMemory> {
    // GuestAddressSpace<M>, which we want to implement, is basically a drop-in
    // replacement for &M.  Therefore, we need to pass to devices the `GuestMemoryAtomic`
    // rather than a reference to it.  To obtain this effect we wrap the actual fields
    // of GuestMemoryAtomic with an Arc, and derive the Clone trait.  See the
    // documentation for GuestAddressSpace for an example.
    inner: Arc<(ArcSwap<M>, Mutex<()>)>,
}

impl<M: GuestMemory> From<Arc<M>> for GuestMemoryAtomic<M> {
    /// create a new `GuestMemoryAtomic` object whose initial contents come from
    /// the `map` reference counted `GuestMemory`.
    fn from(map: Arc<M>) -> Self {
        let inner = (ArcSwap::new(map), Mutex::new(()));
        GuestMemoryAtomic {
            inner: Arc::new(inner),
        }
    }
}

impl<M: GuestMemory> GuestMemoryAtomic<M> {
    /// create a new `GuestMemoryAtomic` object whose initial contents come from
    /// the `map` `GuestMemory`.
    pub fn new(map: M) -> Self {
        Arc::new(map).into()
    }

    fn load(&self) -> Guard<Arc<M>> {
        self.inner.0.load()
    }

    /// Acquires the update mutex for the `GuestMemoryAtomic`, blocking the current
    /// thread until it is able to do so.  The returned RAII guard allows for
    /// scoped unlock of the mutex (that is, the mutex will be unlocked when
    /// the guard goes out of scope), and optionally also for replacing the
    /// contents of the `GuestMemoryAtomic` when the lock is dropped.
    pub fn lock(&self) -> LockResult<GuestMemoryExclusiveGuard<M>> {
        match self.inner.1.lock() {
            Ok(guard) => Ok(GuestMemoryExclusiveGuard {
                parent: self,
                _guard: guard,
            }),
            Err(err) => Err(PoisonError::new(GuestMemoryExclusiveGuard {
                parent: self,
                _guard: err.into_inner(),
            })),
        }
    }
}

impl<M: GuestMemory> GuestAddressSpace for GuestMemoryAtomic<M> {
    type T = GuestMemoryLoadGuard<M>;
    type M = M;

    fn memory(&self) -> Self::T {
        GuestMemoryLoadGuard { guard: self.load() }
    }
}

/// A guard that provides temporary access to a `GuestMemoryAtomic`.  This
/// object is returned from the `memory()` method.  It dereference to
/// a snapshot of the `GuestMemory`, so it can be used transparently to
/// access memory.
#[derive(Debug)]
pub struct GuestMemoryLoadGuard<M: GuestMemory> {
    guard: Guard<Arc<M>>,
}

impl<M: GuestMemory> GuestMemoryLoadGuard<M> {
    /// Make a clone of the held pointer and returns it.  This is more
    /// expensive than just using the snapshot, but it allows to hold on
    /// to the snapshot outside the scope of the guard.  It also allows
    /// writers to proceed, so it is recommended if the reference must
    /// be held for a long time (including for caching purposes).
    pub fn into_inner(self) -> Arc<M> {
        Guard::into_inner(self.guard)
    }
}

impl<M: GuestMemory> Clone for GuestMemoryLoadGuard<M> {
    fn clone(&self) -> Self {
        GuestMemoryLoadGuard {
            guard: Guard::from_inner(Arc::clone(&*self.guard)),
        }
    }
}

impl<M: GuestMemory> Deref for GuestMemoryLoadGuard<M> {
    type Target = M;

    fn deref(&self) -> &Self::Target {
        &*self.guard
    }
}

/// An RAII implementation of a "scoped lock" for `GuestMemoryAtomic`.  When
/// this structure is dropped (falls out of scope) the lock will be unlocked,
/// possibly after updating the memory map represented by the
/// `GuestMemoryAtomic` that created the guard.
pub struct GuestMemoryExclusiveGuard<'a, M: GuestMemory> {
    parent: &'a GuestMemoryAtomic<M>,
    _guard: MutexGuard<'a, ()>,
}

impl<M: GuestMemory> GuestMemoryExclusiveGuard<'_, M> {
    /// Replace the memory map in the `GuestMemoryAtomic` that created the guard
    /// with the new memory map, `map`.  The lock is then dropped since this
    /// method consumes the guard.
    pub fn replace(self, map: M) {
        self.parent.inner.0.store(Arc::new(map))
    }
}

#[cfg(test)]
#[cfg(feature = "backend-mmap")]
mod tests {
    use super::*;
    use crate::{GuestAddress, GuestMemory, GuestMemoryRegion, GuestUsize, MmapRegion};

    type GuestMemoryMmap = crate::GuestMemoryMmap<()>;
    type GuestRegionMmap = crate::GuestRegionMmap<()>;
    type GuestMemoryMmapAtomic = GuestMemoryAtomic<GuestMemoryMmap>;

    #[test]
    fn test_atomic_memory() {
        let region_size = 0x400;
        let regions = vec![
            (GuestAddress(0x0), region_size),
            (GuestAddress(0x1000), region_size),
        ];
        let mut iterated_regions = Vec::new();
        let gmm = GuestMemoryMmap::from_ranges(&regions).unwrap();
        let gm = GuestMemoryMmapAtomic::new(gmm);
        let mem = gm.memory();

        for region in mem.iter() {
            assert_eq!(region.len(), region_size as GuestUsize);
        }

        for region in mem.iter() {
            iterated_regions.push((region.start_addr(), region.len() as usize));
        }
        assert_eq!(regions, iterated_regions);
        assert_eq!(mem.num_regions(), 2);
        assert!(mem.find_region(GuestAddress(0x1000)).is_some());
        assert!(mem.find_region(GuestAddress(0x10000)).is_none());

        assert!(regions
            .iter()
            .map(|x| (x.0, x.1))
            .eq(iterated_regions.iter().copied()));

        let mem2 = mem.into_inner();
        for region in mem2.iter() {
            assert_eq!(region.len(), region_size as GuestUsize);
        }
        assert_eq!(mem2.num_regions(), 2);
        assert!(mem2.find_region(GuestAddress(0x1000)).is_some());
        assert!(mem2.find_region(GuestAddress(0x10000)).is_none());

        assert!(regions
            .iter()
            .map(|x| (x.0, x.1))
            .eq(iterated_regions.iter().copied()));

        let mem3 = mem2.memory();
        for region in mem3.iter() {
            assert_eq!(region.len(), region_size as GuestUsize);
        }
        assert_eq!(mem3.num_regions(), 2);
        assert!(mem3.find_region(GuestAddress(0x1000)).is_some());
        assert!(mem3.find_region(GuestAddress(0x10000)).is_none());
    }

    #[test]
    fn test_clone_guard() {
        let region_size = 0x400;
        let regions = vec![
            (GuestAddress(0x0), region_size),
            (GuestAddress(0x1000), region_size),
        ];
        let gmm = GuestMemoryMmap::from_ranges(&regions).unwrap();
        let gm = GuestMemoryMmapAtomic::new(gmm);
        let mem = {
            let guard1 = gm.memory();
            Clone::clone(&guard1)
        };
        assert_eq!(mem.num_regions(), 2);
    }

    #[test]
    fn test_atomic_hotplug() {
        let region_size = 0x1000;
        let regions = vec![
            (GuestAddress(0x0), region_size),
            (GuestAddress(0x10_0000), region_size),
        ];
        let mut gmm = Arc::new(GuestMemoryMmap::from_ranges(&regions).unwrap());
        let gm: GuestMemoryAtomic<_> = gmm.clone().into();
        let mem_orig = gm.memory();
        assert_eq!(mem_orig.num_regions(), 2);

        {
            let guard = gm.lock().unwrap();
            let new_gmm = Arc::make_mut(&mut gmm);
            let mmap = Arc::new(
                GuestRegionMmap::new(MmapRegion::new(0x1000).unwrap(), GuestAddress(0x8000))
                    .unwrap(),
            );
            let new_gmm = new_gmm.insert_region(mmap).unwrap();
            let mmap = Arc::new(
                GuestRegionMmap::new(MmapRegion::new(0x1000).unwrap(), GuestAddress(0x4000))
                    .unwrap(),
            );
            let new_gmm = new_gmm.insert_region(mmap).unwrap();
            let mmap = Arc::new(
                GuestRegionMmap::new(MmapRegion::new(0x1000).unwrap(), GuestAddress(0xc000))
                    .unwrap(),
            );
            let new_gmm = new_gmm.insert_region(mmap).unwrap();
            let mmap = Arc::new(
                GuestRegionMmap::new(MmapRegion::new(0x1000).unwrap(), GuestAddress(0xc000))
                    .unwrap(),
            );
            new_gmm.insert_region(mmap).unwrap_err();
            guard.replace(new_gmm);
        }

        assert_eq!(mem_orig.num_regions(), 2);
        let mem = gm.memory();
        assert_eq!(mem.num_regions(), 5);
    }
}
