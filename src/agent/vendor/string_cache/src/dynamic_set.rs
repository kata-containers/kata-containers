// Copyright 2014 The Servo Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use once_cell::sync::Lazy;
use parking_lot::Mutex;
use std::borrow::Cow;
use std::mem;
use std::ptr::NonNull;
use std::sync::atomic::AtomicIsize;
use std::sync::atomic::Ordering::SeqCst;

const NB_BUCKETS: usize = 1 << 12; // 4096
const BUCKET_MASK: u32 = (1 << 12) - 1;

pub(crate) struct Set {
    buckets: Box<[Option<Box<Entry>>; NB_BUCKETS]>,
}

pub(crate) struct Entry {
    pub(crate) string: Box<str>,
    pub(crate) hash: u32,
    pub(crate) ref_count: AtomicIsize,
    next_in_bucket: Option<Box<Entry>>,
}

// Addresses are a multiples of this,
// and therefore have have TAG_MASK bits unset, available for tagging.
pub(crate) const ENTRY_ALIGNMENT: usize = 4;

#[test]
fn entry_alignment_is_sufficient() {
    assert!(mem::align_of::<Entry>() >= ENTRY_ALIGNMENT);
}

pub(crate) static DYNAMIC_SET: Lazy<Mutex<Set>> = Lazy::new(|| {
    Mutex::new({
        type T = Option<Box<Entry>>;
        let _static_assert_size_eq = std::mem::transmute::<T, usize>;
        let vec = std::mem::ManuallyDrop::new(vec![0_usize; NB_BUCKETS]);
        Set {
            buckets: unsafe { Box::from_raw(vec.as_ptr() as *mut [T; NB_BUCKETS]) },
        }
    })
});

impl Set {
    pub(crate) fn insert(&mut self, string: Cow<str>, hash: u32) -> NonNull<Entry> {
        let bucket_index = (hash & BUCKET_MASK) as usize;
        {
            let mut ptr: Option<&mut Box<Entry>> = self.buckets[bucket_index].as_mut();

            while let Some(entry) = ptr.take() {
                if entry.hash == hash && *entry.string == *string {
                    if entry.ref_count.fetch_add(1, SeqCst) > 0 {
                        return NonNull::from(&mut **entry);
                    }
                    // Uh-oh. The pointer's reference count was zero, which means someone may try
                    // to free it. (Naive attempts to defend against this, for example having the
                    // destructor check to see whether the reference count is indeed zero, don't
                    // work due to ABA.) Thus we need to temporarily add a duplicate string to the
                    // list.
                    entry.ref_count.fetch_sub(1, SeqCst);
                    break;
                }
                ptr = entry.next_in_bucket.as_mut();
            }
        }
        debug_assert!(mem::align_of::<Entry>() >= ENTRY_ALIGNMENT);
        let string = string.into_owned();
        let mut entry = Box::new(Entry {
            next_in_bucket: self.buckets[bucket_index].take(),
            hash,
            ref_count: AtomicIsize::new(1),
            string: string.into_boxed_str(),
        });
        let ptr = NonNull::from(&mut *entry);
        self.buckets[bucket_index] = Some(entry);

        ptr
    }

    pub(crate) fn remove(&mut self, ptr: *mut Entry) {
        let bucket_index = {
            let value: &Entry = unsafe { &*ptr };
            debug_assert!(value.ref_count.load(SeqCst) == 0);
            (value.hash & BUCKET_MASK) as usize
        };

        let mut current: &mut Option<Box<Entry>> = &mut self.buckets[bucket_index];

        while let Some(entry_ptr) = current.as_mut() {
            let entry_ptr: *mut Entry = &mut **entry_ptr;
            if entry_ptr == ptr {
                mem::drop(mem::replace(current, unsafe {
                    (*entry_ptr).next_in_bucket.take()
                }));
                break;
            }
            current = unsafe { &mut (*entry_ptr).next_in_bucket };
        }
    }
}
