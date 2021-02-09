use std::rc::Weak as RcWeak;
use std::sync::Weak;

use crate::RefCnt;

unsafe impl<T> RefCnt for Weak<T> {
    type Base = T;
    fn as_ptr(me: &Self) -> *mut T {
        Weak::as_ptr(me) as *mut T
    }
    fn into_ptr(me: Self) -> *mut T {
        Weak::into_raw(me) as *mut T
    }
    unsafe fn from_ptr(ptr: *const T) -> Self {
        Weak::from_raw(ptr)
    }
}

unsafe impl<T> RefCnt for RcWeak<T> {
    type Base = T;
    fn as_ptr(me: &Self) -> *mut T {
        RcWeak::as_ptr(me) as *mut T
    }
    fn into_ptr(me: Self) -> *mut T {
        RcWeak::into_raw(me) as *mut T
    }
    unsafe fn from_ptr(ptr: *const T) -> Self {
        RcWeak::from_raw(ptr)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Weak};

    use crate::ArcSwapWeak;

    // Convert to weak, push it through the shared and pull it out again.
    #[test]
    fn there_and_back() {
        let data = Arc::new("Hello");
        let shared = ArcSwapWeak::new(Arc::downgrade(&data));
        assert_eq!(1, Arc::strong_count(&data));
        assert_eq!(1, Arc::weak_count(&data));
        let weak = shared.load();
        assert_eq!("Hello", *weak.upgrade().unwrap());
        assert!(Arc::ptr_eq(&data, &weak.upgrade().unwrap()));
    }

    // Replace a weak pointer with a NULL one
    #[test]
    fn reset() {
        let data = Arc::new("Hello");
        let shared = ArcSwapWeak::new(Arc::downgrade(&data));
        assert_eq!(1, Arc::strong_count(&data));
        assert_eq!(1, Arc::weak_count(&data));

        // An empty weak (eg. NULL)
        shared.store(Weak::new());
        assert_eq!(1, Arc::strong_count(&data));
        assert_eq!(0, Arc::weak_count(&data));

        let weak = shared.load();
        assert!(weak.upgrade().is_none());
    }

    // Destroy the underlying data while the weak is still stored inside. Should make it go
    // NULL-ish
    #[test]
    fn destroy() {
        let data = Arc::new("Hello");
        let shared = ArcSwapWeak::new(Arc::downgrade(&data));

        drop(data);
        let weak = shared.load();
        assert!(weak.upgrade().is_none());
    }
}
