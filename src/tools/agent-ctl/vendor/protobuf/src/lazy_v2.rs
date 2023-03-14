//! Lazily initialized data.
//! Used in generated code.

use std::cell::UnsafeCell;
use std::sync;

/// Lazily initialized data.
pub struct LazyV2<T: Sync> {
    lock: sync::Once,
    ptr: UnsafeCell<*const T>,
}

unsafe impl<T: Sync> Sync for LazyV2<T> {}

impl<T: Sync> LazyV2<T> {
    /// Uninitialized `Lazy` object.
    pub const INIT: LazyV2<T> = LazyV2 {
        lock: sync::Once::new(),
        ptr: UnsafeCell::new(0 as *const T),
    };

    /// Get lazy field value, initialize it with given function if not yet.
    pub fn get<F>(&'static self, init: F) -> &'static T
    where
        F: FnOnce() -> T,
    {
        self.lock.call_once(|| unsafe {
            *self.ptr.get() = Box::into_raw(Box::new(init()));
        });
        unsafe { &**self.ptr.get() }
    }
}

#[cfg(test)]
mod test {
    use std::sync::atomic::AtomicIsize;
    use std::sync::atomic::Ordering;
    use std::sync::Arc;
    use std::sync::Barrier;
    use std::thread;

    use super::LazyV2;

    #[test]
    fn many_threads_calling_get() {
        const N_THREADS: usize = 32;
        const N_ITERS_IN_THREAD: usize = 32;
        const N_ITERS: usize = 16;

        static mut LAZY: LazyV2<String> = LazyV2::INIT;
        static CALL_COUNT: AtomicIsize = AtomicIsize::new(0);

        let value = "Hello, world!".to_owned();

        for _ in 0..N_ITERS {
            // Reset mutable state.
            unsafe {
                LAZY = LazyV2::INIT;
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
