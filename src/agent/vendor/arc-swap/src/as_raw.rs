use super::{Guard, RefCnt};

/// A trait describing things that can be turned into a raw pointer.
///
/// This is just an abstraction of things that can be passed to the
/// [`compare_and_swap`](struct.ArcSwapAny.html#method.compare_and_swap).
///
/// # Examples
///
/// ```
/// use std::ptr;
/// use std::sync::Arc;
///
/// use arc_swap::ArcSwapOption;
///
/// let a = Arc::new(42);
/// let shared = ArcSwapOption::from(Some(Arc::clone(&a)));
///
/// shared.compare_and_swap(&a, Some(Arc::clone(&a)));
/// shared.compare_and_swap(&None::<Arc<_>>, Some(Arc::clone(&a)));
/// shared.compare_and_swap(shared.load(), Some(Arc::clone(&a)));
/// shared.compare_and_swap(&shared.load(), Some(Arc::clone(&a)));
/// shared.compare_and_swap(ptr::null(), Some(Arc::clone(&a)));
/// ```
pub trait AsRaw<T> {
    /// Converts the value into a raw pointer.
    fn as_raw(&self) -> *mut T;
}

impl<'a, T: RefCnt> AsRaw<T::Base> for &'a T {
    fn as_raw(&self) -> *mut T::Base {
        T::as_ptr(self)
    }
}

impl<'a, T: RefCnt> AsRaw<T::Base> for &'a Guard<'a, T> {
    fn as_raw(&self) -> *mut T::Base {
        T::as_ptr(&self.inner)
    }
}

impl<'a, T: RefCnt> AsRaw<T::Base> for Guard<'a, T> {
    fn as_raw(&self) -> *mut T::Base {
        T::as_ptr(&self.inner)
    }
}

impl<T> AsRaw<T> for *mut T {
    fn as_raw(&self) -> *mut T {
        *self
    }
}

impl<T> AsRaw<T> for *const T {
    fn as_raw(&self) -> *mut T {
        *self as *mut T
    }
}
