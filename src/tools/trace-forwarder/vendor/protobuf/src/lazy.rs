//! Lazily initialized data.
//! Used in generated code.

// Avoid deprecation warnings when compiling rust-protobuf
#![allow(deprecated)]

use std::mem;
use std::sync;

/// Lasily initialized data.
#[deprecated(
    since = "2.16",
    note = "Please regenerate .rs files from .proto files to use newer APIs"
)]
pub struct Lazy<T> {
    #[doc(hidden)]
    pub lock: sync::Once,
    #[doc(hidden)]
    pub ptr: *const T,
}

impl<T> Lazy<T> {
    /// Uninitialized `Lazy` object.
    ///
    /// The initializer is added in rust-protobuf 2.11, for compatibility with
    /// previously generated code, existing fields are kept public.
    pub const INIT: Lazy<T> = Lazy {
        lock: sync::Once::new(),
        ptr: 0 as *const T,
    };

    /// Get lazy field value, initialize it with given function if not yet.
    pub fn get<F>(&'static mut self, init: F) -> &'static T
    where
        F: FnOnce() -> T,
    {
        // ~ decouple the lifetimes of 'self' and 'self.lock' such we
        // can initialize self.ptr in the call_once closure (note: we
        // do have to initialize self.ptr in the closure to guarantee
        // the ptr is valid for all calling threads at any point in
        // time)
        let lock: &sync::Once = unsafe { mem::transmute(&self.lock) };
        lock.call_once(|| unsafe {
            self.ptr = mem::transmute(Box::new(init()));
        });
        unsafe { &*self.ptr }
    }
}

/// Used to initialize `lock` field in `Lazy` struct.
#[deprecated(
    since = "2.11",
    note = "Regenerate .proto files to use safer initializer"
)]
pub const ONCE_INIT: sync::Once = sync::Once::new();

#[cfg(test)]
mod test {
    use std::sync::atomic::AtomicIsize;
    use std::sync::atomic::Ordering;
    use std::sync::Arc;
    use std::sync::Barrier;
    use std::thread;

    use super::Lazy;

    #[test]
    fn many_threads_calling_get() {
        const N_THREADS: usize = 32;
        const N_ITERS_IN_THREAD: usize = 32;
        const N_ITERS: usize = 16;

        static mut LAZY: Lazy<String> = Lazy::INIT;
        static CALL_COUNT: AtomicIsize = AtomicIsize::new(0);

        let value = "Hello, world!".to_owned();

        for _ in 0..N_ITERS {
            // Reset mutable state.
            unsafe {
                LAZY = Lazy::INIT;
            }
            CALL_COUNT.store(0, Ordering::SeqCst);

            // Create a bunch of threads, all calling .get() at the same time.
            let mut threads = vec![];
            let barrier = Arc::new(Barrier::new(N_THREADS));

            for _ in 0..N_THREADS {
                let cloned_value_thread = value.clone();
                let cloned_barrier = barrier.clone();
                threads.push(thread::spawn(move || {
                    // Ensure all threads start at once to maximise contention.
                    cloned_barrier.wait();
                    for _ in 0..N_ITERS_IN_THREAD {
                        assert_eq!(&cloned_value_thread, unsafe {
                            LAZY.get(|| {
                                CALL_COUNT.fetch_add(1, Ordering::SeqCst);
                                cloned_value_thread.clone()
                            })
                        });
                    }
                }));
            }

            for thread in threads {
                thread.join().unwrap();
            }

            assert_eq!(CALL_COUNT.load(Ordering::SeqCst), 1);
        }
    }
}
