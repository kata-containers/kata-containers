use std::fmt::{self, Debug};

use self::thread_id::ThreadId;

/// ThreadBound is a Sync-maker and Send-maker that allows accessing a value
/// of type T only from the original thread on which the ThreadBound was
/// constructed.
pub struct ThreadBound<T> {
    value: T,
    thread_id: ThreadId,
}

unsafe impl<T> Sync for ThreadBound<T> {}

// Send bound requires Copy, as otherwise Drop could run in the wrong place.
unsafe impl<T: Copy> Send for ThreadBound<T> {}

impl<T> ThreadBound<T> {
    pub fn new(value: T) -> Self {
        ThreadBound {
            value: value,
            thread_id: thread_id::current(),
        }
    }

    pub fn get(&self) -> Option<&T> {
        if thread_id::current() == self.thread_id {
            Some(&self.value)
        } else {
            None
        }
    }
}

impl<T: Debug> Debug for ThreadBound<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        match self.get() {
            Some(value) => Debug::fmt(value, formatter),
            None => formatter.write_str("unknown"),
        }
    }
}

#[cfg(syn_can_use_thread_id)]
mod thread_id {
    use std::thread;

    pub use std::thread::ThreadId;

    pub fn current() -> ThreadId {
        thread::current().id()
    }
}

#[cfg(not(syn_can_use_thread_id))]
mod thread_id {
    #[allow(deprecated)]
    use std::sync::atomic::{AtomicUsize, Ordering, ATOMIC_USIZE_INIT};

    thread_local! {
        static THREAD_ID: usize = {
            #[allow(deprecated)]
            static NEXT_THREAD_ID: AtomicUsize = ATOMIC_USIZE_INIT;

            // Ordering::Relaxed because our only requirement for the ids is
            // that they are unique. It is okay for the compiler to rearrange
            // other memory reads around this fetch. It's still an atomic
            // fetch_add, so no two threads will be able to read the same value
            // from it.
            //
            // The main thing which these orderings affect is other memory reads
            // around the atomic read, which for our case are irrelevant as this
            // atomic guards nothing.
            NEXT_THREAD_ID.fetch_add(1, Ordering::Relaxed)
        };
    }

    pub type ThreadId = usize;

    pub fn current() -> ThreadId {
        THREAD_ID.with(|id| *id)
    }
}
