//! Miscellaneous tools for concurrent programming.
//!
//! ## Atomics
//!
//! * [`AtomicCell`], a thread-safe mutable memory location.
//! * [`AtomicConsume`], for reading from primitive atomic types with "consume" ordering.
//!
//! ## Thread synchronization
//!
//! * [`Parker`], a thread parking primitive.
//! * [`ShardedLock`], a sharded reader-writer lock with fast concurrent reads.
//! * [`WaitGroup`], for synchronizing the beginning or end of some computation.
//!
//! ## Utilities
//!
//! * [`Backoff`], for exponential backoff in spin loops.
//! * [`CachePadded`], for padding and aligning a value to the length of a cache line.
//! * [`scope`], for spawning threads that borrow local variables from the stack.
//!
//! [`AtomicCell`]: atomic/struct.AtomicCell.html
//! [`AtomicConsume`]: atomic/trait.AtomicConsume.html
//! [`Parker`]: sync/struct.Parker.html
//! [`ShardedLock`]: sync/struct.ShardedLock.html
//! [`WaitGroup`]: sync/struct.WaitGroup.html
//! [`Backoff`]: struct.Backoff.html
//! [`CachePadded`]: struct.CachePadded.html
//! [`scope`]: thread/fn.scope.html

#![warn(missing_docs)]
#![warn(missing_debug_implementations)]
#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(feature = "nightly", feature(cfg_target_has_atomic))]

#[macro_use]
extern crate cfg_if;
#[cfg(feature = "std")]
extern crate core;

cfg_if! {
    if #[cfg(feature = "alloc")] {
        extern crate alloc;
    } else if #[cfg(feature = "std")] {
        extern crate std as alloc;
    }
}

#[cfg_attr(feature = "nightly", cfg(target_has_atomic = "ptr"))]
pub mod atomic;

mod cache_padded;
pub use cache_padded::CachePadded;

mod backoff;
pub use backoff::Backoff;

cfg_if! {
    if #[cfg(feature = "std")] {
        #[macro_use]
        extern crate lazy_static;

        pub mod sync;
        pub mod thread;
    }
}
