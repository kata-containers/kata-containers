pub mod sync {
    pub use std::sync::{Arc, Mutex, Condvar};

    pub mod atomic {
        pub use std::sync::atomic::{AtomicPtr, AtomicBool, AtomicUsize, Ordering};
    }

    use std::cell::UnsafeCell;

    pub struct CausalCell<T>(UnsafeCell<T>);

    impl<T> CausalCell<T> {
        pub fn new(data: T) -> CausalCell<T> {
            CausalCell(UnsafeCell::new(data))
        }

        /*
        pub fn with<F, R>(&self, f: F) -> R
        where
            F: FnOnce(*const T) -> R,
        {
            f(self.0.get())
        }
        */

        /*
        pub fn with_unchecked<F, R>(&self, f: F) -> R
        where
            F: FnOnce(*const T) -> R,
        {
            f(self.0.get())
        }
        */

        pub fn with_mut<F, R>(&self, f: F) -> R
        where
            F: FnOnce(*mut T) -> R,
        {
            f(self.0.get())
        }
    }
}

pub mod thread {
    // Requires Rust 1.24+
    // pub use std::sync::atomic::spin_loop_hint as yield_now;
    pub fn yield_now() {}
}
