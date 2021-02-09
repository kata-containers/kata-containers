pub use self::imp::{Backtrace, InternalBacktrace};

#[cfg(feature = "backtrace")]
mod imp {
    extern crate backtrace;

    use std::cell::UnsafeCell;
    use std::env;
    use std::fmt;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    /// Internal representation of a backtrace
    #[doc(hidden)]
    #[derive(Clone)]
    pub struct InternalBacktrace {
        backtrace: Option<Arc<MaybeResolved>>,
    }

    struct MaybeResolved {
        resolved: Mutex<bool>,
        backtrace: UnsafeCell<Backtrace>,
    }

    unsafe impl Send for MaybeResolved {}
    unsafe impl Sync for MaybeResolved {}

    pub use self::backtrace::Backtrace;

    impl InternalBacktrace {
        /// Returns a backtrace of the current call stack if `RUST_BACKTRACE`
        /// is set to anything but ``0``, and `None` otherwise.  This is used
        /// in the generated error implementations.
        #[doc(hidden)]
        pub fn new() -> InternalBacktrace {
            static ENABLED: AtomicUsize = AtomicUsize::new(0);

            match ENABLED.load(Ordering::SeqCst) {
                0 => {
                    let enabled = match env::var_os("RUST_BACKTRACE") {
                        Some(ref val) if val != "0" => true,
                        _ => false,
                    };
                    ENABLED.store(enabled as usize + 1, Ordering::SeqCst);
                    if !enabled {
                        return InternalBacktrace { backtrace: None };
                    }
                }
                1 => return InternalBacktrace { backtrace: None },
                _ => {}
            }

            InternalBacktrace {
                backtrace: Some(Arc::new(MaybeResolved {
                    resolved: Mutex::new(false),
                    backtrace: UnsafeCell::new(Backtrace::new_unresolved()),
                })),
            }
        }

        /// Acquire the internal backtrace
        #[doc(hidden)]
        pub fn as_backtrace(&self) -> Option<&Backtrace> {
            let bt = match self.backtrace {
                Some(ref bt) => bt,
                None => return None,
            };
            let mut resolved = bt.resolved.lock().unwrap();
            unsafe {
                if !*resolved {
                    (*bt.backtrace.get()).resolve();
                    *resolved = true;
                }
                Some(&*bt.backtrace.get())
            }
        }
    }

    impl fmt::Debug for InternalBacktrace {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.debug_struct("InternalBacktrace")
                .field("backtrace", &self.as_backtrace())
                .finish()
        }
    }
}

#[cfg(not(feature = "backtrace"))]
mod imp {
    /// Dummy type used when the `backtrace` feature is disabled.
    pub type Backtrace = ();

    /// Internal representation of a backtrace
    #[doc(hidden)]
    #[derive(Clone, Debug)]
    pub struct InternalBacktrace {}

    impl InternalBacktrace {
        /// Returns a new backtrace
        #[doc(hidden)]
        pub fn new() -> InternalBacktrace {
            InternalBacktrace {}
        }

        /// Returns the internal backtrace
        #[doc(hidden)]
        pub fn as_backtrace(&self) -> Option<&Backtrace> {
            None
        }
    }
}
