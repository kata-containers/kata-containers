// Copyright 2021 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0 OR BSD-3-Clause

use std::ops::Deref;
use std::sync::Arc;

use crate::bitmap::{ArcSlice, AtomicBitmap, Bitmap, WithBitmapSlice};

#[cfg(feature = "backend-mmap")]
use crate::mmap::NewBitmap;

/// A `Bitmap` implementation that's based on an atomically reference counted handle to an
/// `AtomicBitmap` object.
pub struct AtomicBitmapArc {
    inner: Arc<AtomicBitmap>,
}

impl AtomicBitmapArc {
    pub fn new(inner: AtomicBitmap) -> Self {
        AtomicBitmapArc {
            inner: Arc::new(inner),
        }
    }
}

// The current clone implementation creates a deep clone of the inner bitmap, as opposed to
// simply cloning the `Arc`.
impl Clone for AtomicBitmapArc {
    fn clone(&self) -> Self {
        Self::new(self.inner.deref().clone())
    }
}

// Providing a `Deref` to `AtomicBitmap` implementation, so the methods of the inner object
// can be called in a transparent manner.
impl Deref for AtomicBitmapArc {
    type Target = AtomicBitmap;

    fn deref(&self) -> &Self::Target {
        self.inner.deref()
    }
}

impl WithBitmapSlice<'_> for AtomicBitmapArc {
    type S = ArcSlice<AtomicBitmap>;
}

impl Bitmap for AtomicBitmapArc {
    fn mark_dirty(&self, offset: usize, len: usize) {
        self.inner.set_addr_range(offset, len)
    }

    fn dirty_at(&self, offset: usize) -> bool {
        self.inner.is_addr_set(offset)
    }

    fn slice_at(&self, offset: usize) -> <Self as WithBitmapSlice>::S {
        ArcSlice::new(self.inner.clone(), offset)
    }
}

impl Default for AtomicBitmapArc {
    fn default() -> Self {
        Self::new(AtomicBitmap::default())
    }
}

#[cfg(feature = "backend-mmap")]
impl NewBitmap for AtomicBitmapArc {
    fn with_len(len: usize) -> Self {
        Self::new(AtomicBitmap::with_len(len))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::bitmap::tests::test_bitmap;

    #[test]
    fn test_bitmap_impl() {
        let b = AtomicBitmapArc::new(AtomicBitmap::new(0x2000, 128));
        test_bitmap(&b);
    }
}
