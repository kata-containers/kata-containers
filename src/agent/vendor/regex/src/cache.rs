// This module defines a common API for caching internal runtime state.
// The `thread_local` crate provides an extremely optimized version of this.
// However, if the perf-cache feature is disabled, then we drop the
// thread_local dependency and instead use a pretty naive caching mechanism
// with a mutex.
//
// Strictly speaking, the CachedGuard isn't necessary for the much more
// flexible thread_local API, but implementing thread_local's API doesn't
// seem possible in purely safe code.

pub use self::imp::{Cached, CachedGuard};

#[cfg(feature = "perf-cache")]
mod imp {
    use thread_local::CachedThreadLocal;

    #[derive(Debug)]
    pub struct Cached<T: Send>(CachedThreadLocal<T>);

    #[derive(Debug)]
    pub struct CachedGuard<'a, T: 'a>(&'a T);

    impl<T: Send> Cached<T> {
        pub fn new() -> Cached<T> {
            Cached(CachedThreadLocal::new())
        }

        pub fn get_or(&self, create: impl FnOnce() -> T) -> CachedGuard<T> {
            CachedGuard(self.0.get_or(|| create()))
        }
    }

    impl<'a, T: Send> CachedGuard<'a, T> {
        pub fn value(&self) -> &T {
            self.0
        }
    }
}

#[cfg(not(feature = "perf-cache"))]
mod imp {
    use std::marker::PhantomData;
    use std::panic::UnwindSafe;
    use std::sync::Mutex;

    #[derive(Debug)]
    pub struct Cached<T: Send> {
        stack: Mutex<Vec<T>>,
        /// When perf-cache is enabled, the thread_local crate is used, and
        /// its CachedThreadLocal impls Send, Sync and UnwindSafe, but NOT
        /// RefUnwindSafe. However, a Mutex impls RefUnwindSafe. So in order
        /// to keep the APIs consistent regardless of whether perf-cache is
        /// enabled, we force this type to NOT impl RefUnwindSafe too.
        ///
        /// Ideally, we should always impl RefUnwindSafe, but it seems a little
        /// tricky to do that right now.
        ///
        /// See also: https://github.com/rust-lang/regex/issues/576
        _phantom: PhantomData<Box<dyn Send + Sync + UnwindSafe>>,
    }

    #[derive(Debug)]
    pub struct CachedGuard<'a, T: 'a + Send> {
        cache: &'a Cached<T>,
        value: Option<T>,
    }

    impl<T: Send> Cached<T> {
        pub fn new() -> Cached<T> {
            Cached { stack: Mutex::new(vec![]), _phantom: PhantomData }
        }

        pub fn get_or(&self, create: impl FnOnce() -> T) -> CachedGuard<T> {
            let mut stack = self.stack.lock().unwrap();
            match stack.pop() {
                None => CachedGuard { cache: self, value: Some(create()) },
                Some(value) => CachedGuard { cache: self, value: Some(value) },
            }
        }

        fn put(&self, value: T) {
            let mut stack = self.stack.lock().unwrap();
            stack.push(value);
        }
    }

    impl<'a, T: Send> CachedGuard<'a, T> {
        pub fn value(&self) -> &T {
            self.value.as_ref().unwrap()
        }
    }

    impl<'a, T: Send> Drop for CachedGuard<'a, T> {
        fn drop(&mut self) {
            if let Some(value) = self.value.take() {
                self.cache.put(value);
            }
        }
    }
}
