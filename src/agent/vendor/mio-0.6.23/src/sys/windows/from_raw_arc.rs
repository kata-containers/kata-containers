//! A "Manual Arc" which allows manually frobbing the reference count
//!
//! This module contains a copy of the `Arc` found in the standard library,
//! stripped down to the bare bones of what we actually need. The reason this is
//! done is for the ability to concretely know the memory layout of the `Inner`
//! structure of the arc pointer itself (e.g. `ArcInner` in the standard
//! library).
//!
//! We do some unsafe casting from `*mut OVERLAPPED` to a `FromRawArc<T>` to
//! ensure that data lives for the length of an I/O operation, but this means
//! that we have to know the layouts of the structures involved. This
//! representation primarily guarantees that the data, `T` is at the front of
//! the inner pointer always.
//!
//! Note that we're missing out on some various optimizations implemented in the
//! standard library:
//!
//! * The size of `FromRawArc` is actually two words because of the drop flag
//! * The compiler doesn't understand that the pointer in `FromRawArc` is never
//!   null, so Option<FromRawArc<T>> is not a nullable pointer.

use std::ops::Deref;
use std::mem;
use std::sync::atomic::{self, AtomicUsize, Ordering};

pub struct FromRawArc<T> {
    _inner: *mut Inner<T>,
}

unsafe impl<T: Sync + Send> Send for FromRawArc<T> { }
unsafe impl<T: Sync + Send> Sync for FromRawArc<T> { }

#[repr(C)]
struct Inner<T> {
    data: T,
    cnt: AtomicUsize,
}

impl<T> FromRawArc<T> {
    pub fn new(data: T) -> FromRawArc<T> {
        let x = Box::new(Inner {
            data: data,
            cnt: AtomicUsize::new(1),
        });
        FromRawArc { _inner: unsafe { mem::transmute(x) } }
    }

    pub unsafe fn from_raw(ptr: *mut T) -> FromRawArc<T> {
        // Note that if we could use `mem::transmute` here to get a libstd Arc
        // (guaranteed) then we could just use std::sync::Arc, but this is the
        // crucial reason this currently exists.
        FromRawArc { _inner: ptr as *mut Inner<T> }
    }
}

impl<T> Clone for FromRawArc<T> {
    fn clone(&self) -> FromRawArc<T> {
        // Atomic ordering of Relaxed lifted from libstd, but the general idea
        // is that you need synchronization to communicate this increment to
        // another thread, so this itself doesn't need to be synchronized.
        unsafe {
            (*self._inner).cnt.fetch_add(1, Ordering::Relaxed);
        }
        FromRawArc { _inner: self._inner }
    }
}

impl<T> Deref for FromRawArc<T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &(*self._inner).data }
    }
}

impl<T> Drop for FromRawArc<T> {
    fn drop(&mut self) {
        unsafe {
            // Atomic orderings lifted from the standard library
            if (*self._inner).cnt.fetch_sub(1, Ordering::Release) != 1 {
                return
            }
            atomic::fence(Ordering::Acquire);
            drop(mem::transmute::<_, Box<T>>(self._inner));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::FromRawArc;

    #[test]
    fn smoke() {
        let a = FromRawArc::new(1);
        assert_eq!(*a, 1);
        assert_eq!(*a.clone(), 1);
    }

    #[test]
    fn drops() {
        struct A<'a>(&'a mut bool);
        impl<'a> Drop for A<'a> {
            fn drop(&mut self) {
                *self.0 = true;
            }
        }
        let mut a = false;
        {
            let a = FromRawArc::new(A(&mut a));
            let _ = a.clone();
            assert!(!*a.0);
        }
        assert!(a);
    }
}
