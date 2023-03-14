// Copyright 2021 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0 OR BSD-3-Clause

//! This module holds abstractions that enable tracking the areas dirtied by writes of a specified
//! length to a given offset. In particular, this is used to track write accesses within a
//! `GuestMemoryRegion` object, and the resulting bitmaps can then be aggregated to build the
//! global view for an entire `GuestMemory` object.

#[cfg(any(test, feature = "backend-bitmap"))]
mod backend;

use std::fmt::Debug;

use crate::{GuestMemory, GuestMemoryRegion};

#[cfg(any(test, feature = "backend-bitmap"))]
pub use backend::{ArcSlice, AtomicBitmap, RefSlice};

/// Trait implemented by types that support creating `BitmapSlice` objects.
pub trait WithBitmapSlice<'a> {
    /// Type of the bitmap slice.
    type S: BitmapSlice;
}

/// Trait used to represent that a `BitmapSlice` is a `Bitmap` itself, but also satisfies the
/// restriction that slices created from it have the same type as `Self`.
pub trait BitmapSlice: Bitmap + Clone + Debug + for<'a> WithBitmapSlice<'a, S = Self> {}

/// Common bitmap operations. Using Higher-Rank Trait Bounds (HRTBs) to effectively define
/// an associated type that has a lifetime parameter, without tagging the `Bitmap` trait with
/// a lifetime as well.
///
/// Using an associated type allows implementing the `Bitmap` and `BitmapSlice` functionality
/// as a zero-cost abstraction when providing trivial implementations such as the one
/// defined for `()`.
// These methods represent the core functionality that's required by `vm-memory` abstractions
// to implement generic tracking logic, as well as tests that can be reused by different backends.
pub trait Bitmap: for<'a> WithBitmapSlice<'a> {
    /// Mark the memory range specified by the given `offset` and `len` as dirtied.
    fn mark_dirty(&self, offset: usize, len: usize);

    /// Check whether the specified `offset` is marked as dirty.
    fn dirty_at(&self, offset: usize) -> bool;

    /// Return a `<Self as WithBitmapSlice>::S` slice of the current bitmap, starting at
    /// the specified `offset`.
    fn slice_at(&self, offset: usize) -> <Self as WithBitmapSlice>::S;
}

/// A no-op `Bitmap` implementation that can be provided for backends that do not actually
/// require the tracking functionality.

impl<'a> WithBitmapSlice<'a> for () {
    type S = Self;
}

impl BitmapSlice for () {}

impl Bitmap for () {
    fn mark_dirty(&self, _offset: usize, _len: usize) {}

    fn dirty_at(&self, _offset: usize) -> bool {
        false
    }

    fn slice_at(&self, _offset: usize) -> Self {}
}

/// A `Bitmap` and `BitmapSlice` implementation for `Option<B>`.

impl<'a, B> WithBitmapSlice<'a> for Option<B>
where
    B: WithBitmapSlice<'a>,
{
    type S = Option<B::S>;
}

impl<B: BitmapSlice> BitmapSlice for Option<B> {}

impl<B: Bitmap> Bitmap for Option<B> {
    fn mark_dirty(&self, offset: usize, len: usize) {
        if let Some(inner) = self {
            inner.mark_dirty(offset, len)
        }
    }

    fn dirty_at(&self, offset: usize) -> bool {
        if let Some(inner) = self {
            return inner.dirty_at(offset);
        }
        false
    }

    fn slice_at(&self, offset: usize) -> Option<<B as WithBitmapSlice>::S> {
        if let Some(inner) = self {
            return Some(inner.slice_at(offset));
        }
        None
    }
}

/// Helper type alias for referring to the `BitmapSlice` concrete type associated with
/// an object `B: WithBitmapSlice<'a>`.
pub type BS<'a, B> = <B as WithBitmapSlice<'a>>::S;

/// Helper type alias for referring to the `BitmapSlice` concrete type associated with
/// the memory regions of an object `M: GuestMemory`.
pub type MS<'a, M> = BS<'a, <<M as GuestMemory>::R as GuestMemoryRegion>::B>;

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    use std::io::Cursor;
    use std::marker::PhantomData;
    use std::mem::size_of_val;
    use std::result::Result;
    use std::sync::atomic::Ordering;

    use crate::{Bytes, VolatileMemory};
    #[cfg(feature = "backend-mmap")]
    use crate::{GuestAddress, MemoryRegionAddress};

    // Helper method to check whether a specified range is clean.
    pub fn range_is_clean<B: Bitmap>(b: &B, start: usize, len: usize) -> bool {
        (start..start + len).all(|offset| !b.dirty_at(offset))
    }

    // Helper method to check whether a specified range is dirty.
    pub fn range_is_dirty<B: Bitmap>(b: &B, start: usize, len: usize) -> bool {
        (start..start + len).all(|offset| b.dirty_at(offset))
    }

    pub fn check_range<B: Bitmap>(b: &B, start: usize, len: usize, clean: bool) -> bool {
        if clean {
            range_is_clean(b, start, len)
        } else {
            range_is_dirty(b, start, len)
        }
    }

    // Helper method that tests a generic `B: Bitmap` implementation. It assumes `b` covers
    // an area of length at least 0x2000.
    pub fn test_bitmap<B: Bitmap>(b: &B) {
        let len = 0x2000;
        let dirty_offset = 0x1000;
        let dirty_len = 0x100;

        // Some basic checks.
        let s = b.slice_at(dirty_offset);

        assert!(range_is_clean(b, 0, len));
        assert!(range_is_clean(&s, 0, dirty_len));

        b.mark_dirty(dirty_offset, dirty_len);
        assert!(range_is_dirty(b, dirty_offset, dirty_len));
        assert!(range_is_dirty(&s, 0, dirty_len));
    }

    #[derive(Debug)]
    pub enum TestAccessError {
        RangeCleanCheck,
        RangeDirtyCheck,
    }

    // A helper object that implements auxiliary operations for testing `Bytes` implementations
    // in the context of dirty bitmap tracking.
    struct BytesHelper<F, G, M> {
        check_range_fn: F,
        address_fn: G,
        phantom: PhantomData<*const M>,
    }

    // `F` represents a closure the checks whether a specified range associated with the `Bytes`
    // object that's being tested is marked as dirty or not (depending on the value of the last
    // parameter). It has the following parameters:
    // - A reference to a `Bytes` implementations that's subject to testing.
    // - The offset of the range.
    // - The length of the range.
    // - Whether we are checking if the range is clean (when `true`) or marked as dirty.
    //
    // `G` represents a closure that translates an offset into an address value that's
    // relevant for the `Bytes` implementation being tested.
    impl<F, G, M, A> BytesHelper<F, G, M>
    where
        F: Fn(&M, usize, usize, bool) -> bool,
        G: Fn(usize) -> A,
        M: Bytes<A>,
    {
        fn check_range(&self, m: &M, start: usize, len: usize, clean: bool) -> bool {
            (self.check_range_fn)(m, start, len, clean)
        }

        fn address(&self, offset: usize) -> A {
            (self.address_fn)(offset)
        }

        fn test_access<Op>(
            &self,
            bytes: &M,
            dirty_offset: usize,
            dirty_len: usize,
            op: Op,
        ) -> Result<(), TestAccessError>
        where
            Op: Fn(&M, A),
        {
            if !self.check_range(bytes, dirty_offset, dirty_len, true) {
                return Err(TestAccessError::RangeCleanCheck);
            }

            op(bytes, self.address(dirty_offset));

            if !self.check_range(bytes, dirty_offset, dirty_len, false) {
                return Err(TestAccessError::RangeDirtyCheck);
            }

            Ok(())
        }
    }

    // `F` and `G` stand for the same closure types as described in the `BytesHelper` comment.
    // The `step` parameter represents the offset that's added the the current address after
    // performing each access. It provides finer grained control when testing tracking
    // implementations that aggregate entire ranges for accounting purposes (for example, doing
    // tracking at the page level).
    pub fn test_bytes<F, G, M, A>(bytes: &M, check_range_fn: F, address_fn: G, step: usize)
    where
        F: Fn(&M, usize, usize, bool) -> bool,
        G: Fn(usize) -> A,
        A: Copy,
        M: Bytes<A>,
        <M as Bytes<A>>::E: Debug,
    {
        const BUF_SIZE: usize = 1024;
        let buf = vec![1u8; 1024];

        let val = 1u64;

        let h = BytesHelper {
            check_range_fn,
            address_fn,
            phantom: PhantomData,
        };

        let mut dirty_offset = 0x1000;

        // Test `write`.
        h.test_access(bytes, dirty_offset, BUF_SIZE, |m, addr| {
            assert_eq!(m.write(buf.as_slice(), addr).unwrap(), BUF_SIZE)
        })
        .unwrap();
        dirty_offset += step;

        // Test `write_slice`.
        h.test_access(bytes, dirty_offset, BUF_SIZE, |m, addr| {
            m.write_slice(buf.as_slice(), addr).unwrap()
        })
        .unwrap();
        dirty_offset += step;

        // Test `write_obj`.
        h.test_access(bytes, dirty_offset, size_of_val(&val), |m, addr| {
            m.write_obj(val, addr).unwrap()
        })
        .unwrap();
        dirty_offset += step;

        // Test `read_from`.
        h.test_access(bytes, dirty_offset, BUF_SIZE, |m, addr| {
            assert_eq!(
                m.read_from(addr, &mut Cursor::new(&buf), BUF_SIZE).unwrap(),
                BUF_SIZE
            )
        })
        .unwrap();
        dirty_offset += step;

        // Test `read_exact_from`.
        h.test_access(bytes, dirty_offset, BUF_SIZE, |m, addr| {
            m.read_exact_from(addr, &mut Cursor::new(&buf), BUF_SIZE)
                .unwrap()
        })
        .unwrap();
        dirty_offset += step;

        // Test `store`.
        h.test_access(bytes, dirty_offset, size_of_val(&val), |m, addr| {
            m.store(val, addr, Ordering::Relaxed).unwrap()
        })
        .unwrap();
    }

    // This function and the next are currently conditionally compiled because we only use
    // them to test the mmap-based backend implementations for now. Going forward, the generic
    // test functions defined here can be placed in a separate module (i.e. `test_utilities`)
    // which is gated by a feature and can be used for testing purposes by other crates as well.
    #[cfg(feature = "backend-mmap")]
    fn test_guest_memory_region<R: GuestMemoryRegion>(region: &R) {
        let dirty_addr = MemoryRegionAddress(0x0);
        let val = 123u64;
        let dirty_len = size_of_val(&val);

        let slice = region.get_slice(dirty_addr, dirty_len).unwrap();

        assert!(range_is_clean(region.bitmap(), 0, region.len() as usize));
        assert!(range_is_clean(slice.bitmap(), 0, dirty_len));

        region.write_obj(val, dirty_addr).unwrap();

        assert!(range_is_dirty(
            region.bitmap(),
            dirty_addr.0 as usize,
            dirty_len
        ));

        assert!(range_is_dirty(slice.bitmap(), 0, dirty_len));

        // Finally, let's invoke the generic tests for `R: Bytes`. It's ok to pass the same
        // `region` handle because `test_bytes` starts performing writes after the range that's
        // been already dirtied in the first part of this test.
        test_bytes(
            region,
            |r: &R, start: usize, len: usize, clean: bool| {
                check_range(r.bitmap(), start, len, clean)
            },
            |offset| MemoryRegionAddress(offset as u64),
            0x1000,
        );
    }

    #[cfg(feature = "backend-mmap")]
    // Assumptions about M generated by f ...
    pub fn test_guest_memory_and_region<M, F>(f: F)
    where
        M: GuestMemory,
        F: Fn() -> M,
    {
        let m = f();
        let dirty_addr = GuestAddress(0x1000);
        let val = 123u64;
        let dirty_len = size_of_val(&val);

        let (region, region_addr) = m.to_region_addr(dirty_addr).unwrap();
        let slice = m.get_slice(dirty_addr, dirty_len).unwrap();

        assert!(range_is_clean(region.bitmap(), 0, region.len() as usize));
        assert!(range_is_clean(slice.bitmap(), 0, dirty_len));

        m.write_obj(val, dirty_addr).unwrap();

        assert!(range_is_dirty(
            region.bitmap(),
            region_addr.0 as usize,
            dirty_len
        ));

        assert!(range_is_dirty(slice.bitmap(), 0, dirty_len));

        // Now let's invoke the tests for the inner `GuestMemoryRegion` type.
        test_guest_memory_region(f().find_region(GuestAddress(0)).unwrap());

        // Finally, let's invoke the generic tests for `Bytes`.
        let check_range_closure = |m: &M, start: usize, len: usize, clean: bool| -> bool {
            let mut check_result = true;
            m.try_access(len, GuestAddress(start as u64), |_, size, reg_addr, reg| {
                if !check_range(reg.bitmap(), reg_addr.0 as usize, size, clean) {
                    check_result = false;
                }
                Ok(size)
            })
            .unwrap();

            check_result
        };

        test_bytes(
            &f(),
            check_range_closure,
            |offset| GuestAddress(offset as u64),
            0x1000,
        );
    }

    pub fn test_volatile_memory<M: VolatileMemory>(m: &M) {
        assert!(m.len() >= 0x8000);

        let dirty_offset = 0x1000;
        let val = 123u64;
        let dirty_len = size_of_val(&val);

        let get_ref_offset = 0x2000;
        let array_ref_offset = 0x3000;

        let s1 = m.as_volatile_slice();
        let s2 = m.get_slice(dirty_offset, dirty_len).unwrap();

        assert!(range_is_clean(s1.bitmap(), 0, s1.len()));
        assert!(range_is_clean(s2.bitmap(), 0, s2.len()));

        s1.write_obj(val, dirty_offset).unwrap();

        assert!(range_is_dirty(s1.bitmap(), dirty_offset, dirty_len));
        assert!(range_is_dirty(s2.bitmap(), 0, dirty_len));

        let v_ref = m.get_ref::<u64>(get_ref_offset).unwrap();
        assert!(range_is_clean(s1.bitmap(), get_ref_offset, dirty_len));
        v_ref.store(val);
        assert!(range_is_dirty(s1.bitmap(), get_ref_offset, dirty_len));

        let arr_ref = m.get_array_ref::<u64>(array_ref_offset, 1).unwrap();
        assert!(range_is_clean(s1.bitmap(), array_ref_offset, dirty_len));
        arr_ref.store(0, val);
        assert!(range_is_dirty(s1.bitmap(), array_ref_offset, dirty_len));
    }
}
