#![doc(
    html_root_url = "https://docs.rs/arc-swap/0.4.7/arc-swap/",
    test(attr(deny(warnings)))
)]
#![deny(missing_docs, warnings)]
// We aim at older rust too, one without dyn
#![allow(unknown_lints, bare_trait_objects, renamed_and_removed_lints)]

//! Making [`Arc`][Arc] itself atomic
//!
//! The library provides a type that is somewhat similar to what `RwLock<Arc<T>>` is or
//! `Atomic<Arc<T>>` would be if it existed, optimized for read-mostly update-seldom scenarios,
//! with consistent performance characteristics.
//!
//! # Motivation
//!
//! There are many situations in which one might want to have some data structure that is often
//! read and seldom updated. Some examples might be a configuration of a service, routing tables,
//! snapshot of some data that is renewed every few minutes, etc.
//!
//! In all these cases one needs:
//! * Being able to read the current value of the data structure, *fast*.
//! * Using the same version of the data structure over longer period of time ‒ a query should be
//!   answered by a consistent version of data, a packet should be routed either by an old or by a
//!   new version of the routing table but not by a combination, etc.
//! * Perform an update without disrupting the processing.
//!
//! The first idea would be to use [`RwLock<T>`][RwLock] and keep a read-lock for the whole time of
//! processing. Update would, however, pause all processing until done.
//!
//! Better option would be to have [`RwLock<Arc<T>>`][RwLock]. Then one would lock, clone the [Arc]
//! and unlock. This suffers from CPU-level contention (on the lock and on the reference count of
//! the [Arc]) which makes it relatively slow. Depending on the implementation, an update may be
//! blocked for arbitrary long time by a steady inflow of readers.
//!
//! ```rust
//! # extern crate once_cell;
//! # use std::sync::{Arc, RwLock};
//! # use once_cell::sync::Lazy;
//! # struct RoutingTable; struct Packet; impl RoutingTable { fn route(&self, _: Packet) {} }
//! static ROUTING_TABLE: Lazy<RwLock<Arc<RoutingTable>>> = Lazy::new(|| {
//!     RwLock::new(Arc::new(RoutingTable))
//! });
//!
//! fn process_packet(packet: Packet) {
//!     let table = Arc::clone(&ROUTING_TABLE.read().unwrap());
//!     table.route(packet);
//! }
//! # fn main() { process_packet(Packet); }
//! ```
//!
//! The [ArcSwap] can be used instead, which solves the above problems and has better performance
//! characteristics than the [RwLock], both in contended and non-contended scenarios.
//!
//! ```rust
//! # extern crate arc_swap;
//! # extern crate once_cell;
//! # use arc_swap::ArcSwap;
//! # use once_cell::sync::Lazy;
//! # struct RoutingTable; struct Packet; impl RoutingTable { fn route(&self, _: Packet) {} }
//! static ROUTING_TABLE: Lazy<ArcSwap<RoutingTable>> = Lazy::new(|| {
//!     ArcSwap::from_pointee(RoutingTable)
//! });
//!
//! fn process_packet(packet: Packet) {
//!     let table = ROUTING_TABLE.load();
//!     table.route(packet);
//! }
//! # fn main() { process_packet(Packet); }
//! ```
//!
//! # Type aliases
//!
//! The most interesting types in the crate are the [ArcSwap] and [ArcSwapOption] (the latter
//! similar to `Atomic<Option<Arc<T>>>`). These are the types users will want to use.
//!
//! Note, however, that these are type aliases of the [ArcSwapAny]. While that type is the
//! low-level implementation and usually isn't referred to directly in the user code, all the
//! relevant methods (and therefore documentation) is on it.
//!
//! # Atomic orderings
//!
//! Each operation on the [ArcSwapAny] type callable concurrently (eg. [load], but not
//! [into_inner]) contains at least one SeqCst atomic read-write operation, therefore even
//! operations on different instances have a defined global order of operations.
//!
//! # Less usual needs
//!
//! There are some utilities that make the crate useful in more places than just the basics
//! described above.
//!
//! The [load_signal_safe] method can be safely used inside unix signal handlers (it is the only
//! one guaranteed to be safe there).
//!
//! The [Cache] allows further speed improvements over simply using [load] every time. The downside
//! is less comfortable API (the caller needs to keep the cache around). Also, a cache may keep the
//! older version of the value alive even when it is not in active use, until the cache is
//! re-validated.
//!
//! The [access] module (and similar traits in the [cache] module) allows shielding independent
//! parts of application from each other and from the exact structure of the *whole* configuration.
//! This helps structuring the application and giving it access only to its own parts of the
//! configuration.
//!
//! Finally, the [gen_lock] module allows further customization of low-level locking/concurrency
//! details.
//!
//! # Performance characteristics
//!
//! There are several performance advantages of [ArcSwap] over [RwLock].
//!
//! ## Lock-free readers
//!
//! All the read operations are always [lock-free]. Most of the time, they are actually
//! [wait-free], the notable exception is the first [load] access in each thread (across all the
//! instances of [ArcSwap]), as it sets up some thread-local data structures.
//!
//! Whenever the documentation talks about *contention* in the context of [ArcSwap], it talks about
//! contention on the CPU level ‒ multpile cores having to deal with accessing the same cache line.
//! This slows things down (compared to each one accessing its own cache line), but an eventual
//! progress is still guaranteed and the cost is significantly lower than parking threads as with
//! mutex-style contention.
//!
//! Unfortunately writers are *not* [lock-free]. A reader stuck (suspended/killed) in a critical
//! section (few instructions long in case of [load]) may block a writer from completion.
//! Nevertheless, a steady inflow of new readers nor other writers will not block the writer.
//!
//! ## Speeds
//!
//! The base line speed of read operations is similar to using an *uncontended* [`Mutex`][Mutex].
//! However, [load] suffers no contention from any other read operations and only slight
//! ones during updates. The [`load_full`][load_full] operation is additionally contended only on
//! the reference count of the [Arc] inside ‒ so, in general, while [Mutex] rapidly
//! loses its performance when being in active use by multiple threads at once and
//! [RwLock] is slow to start with, [ArcSwap] mostly keeps its performance even when read by many
//! threads in parallel.
//!
//! Write operations are considered expensive. A write operation is more expensive than access to
//! an *uncontended* [Mutex] and on some architectures even slower than uncontended
//! [RwLock]. However, it is faster than either under contention.
//!
//! There are some (very unscientific) [benchmarks] within the source code of the library.
//!
//! The exact numbers are highly dependant on the machine used (both absolute numbers and relative
//! between different data structures). Not only architectures have a huge impact (eg. x86 vs ARM),
//! but even AMD vs. Intel or two different Intel processors. Therefore, if what matters is more
//! the speed than the wait-free guarantees, you're advised to do your own measurements.
//!
//! Further speed improvements may be gained by the use of the [Cache].
//!
//! ## Consistency
//!
//! The combination of [wait-free] guarantees of readers and no contention between concurrent
//! [load]s provides *consistent* performance characteristics of the synchronization mechanism.
//! This might be important for soft-realtime applications (the CPU-level contention caused by a
//! recent update/write operation might be problematic for some hard-realtime cases, though).
//!
//! ## Choosing the right reading operation
//!
//! There are several load operations available. While the general go-to one should be
//! [load], there may be situations in which the others are a better match.
//!
//! The [load] usually only borrows the instance from the shared [ArcSwap]. This makes
//! it faster, because different threads don't contend on the reference count. There are two
//! situations when this borrow isn't possible. If the content gets changed, all existing
//! [`Guard`][Guard]s are promoted to contain an owned instance. The promotion is done by the
//! writer, but the readers still need to decrement the reference counts of the old instance when
//! they no longer use it, contending on the count.
//!
//! The other situation derives from internal implementation. The number of borrows each thread can
//! have at each time (across all [Guard]s) is limited. If this limit is exceeded, an onwed
//! instance is created instead.
//!
//! Therefore, if you intend to hold onto the loaded value for extended time span, you may prefer
//! [load_full]. It loads the pointer instance (`Arc`) without borrowing, which is
//! slower (because of the possible contention on the reference count), but doesn't consume one of
//! the borrow slots, which will make it more likely for following [load]s to have a slot
//! available. Similarly, if some API needs an owned `Arc`, [load_full] is more convenient.
//!
//! There's also [load_signal_safe]. This is the only method guaranteed to be
//! safely usable inside a unix signal handler. It has no advantages outside of them, so it makes
//! it kind of niche one.
//!
//! Additionally, it is possible to use a [`Cache`][Cache] to get further speed improvement at the
//! cost of less comfortable API and possibly keeping the older values alive for longer than
//! necessary.
//!
//! # Examples
//!
//! ```rust
//! extern crate arc_swap;
//! extern crate crossbeam_utils;
//!
//! use std::sync::Arc;
//!
//! use arc_swap::ArcSwap;
//! use crossbeam_utils::thread;
//!
//! fn main() {
//!     let config = ArcSwap::from(Arc::new(String::default()));
//!     thread::scope(|scope| {
//!         scope.spawn(|_| {
//!             let new_conf = Arc::new("New configuration".to_owned());
//!             config.store(new_conf);
//!         });
//!         for _ in 0..10 {
//!             scope.spawn(|_| {
//!                 loop {
//!                     let cfg = config.load();
//!                     if !cfg.is_empty() {
//!                         assert_eq!(**cfg, "New configuration");
//!                         return;
//!                     }
//!                 }
//!             });
//!         }
//!     }).unwrap();
//! }
//! ```
//!
//! # Features
//!
//! The `weak` feature adds the ability to use arc-swap with the [Weak] pointer too,
//! through the [ArcSwapWeak] type. The needed std support is stabilized in rust version 1.45 (as
//! of now in beta).
//!
//! # Internal details
//!
//! The crate uses a hybrid approach of stripped-down hazard pointers and something close to a
//! sharded spin lock with asymmetric read/write usage (called the generation lock).
//!
//! Further details are described in comments inside the source code and in two blog posts:
//!
//! * [Making `Arc` more atomic](https://vorner.github.io/2018/06/24/arc-more-atomic.html)
//! * [More tricks up in the ArcSwap's sleeve](https://vorner.github.io/2019/04/06/tricks-in-arc-swap.html)
//!
//! # Limitations
//!
//! This currently works only for `Sized` types. Unsized types have „fat pointers“, which are twice
//! as large as the normal ones. The [`AtomicPtr`] doesn't support them. One could use something
//! like `AtomicU128` for them. The catch is this doesn't exist and the difference would make it
//! really hard to implement the debt storage/stripped down hazard pointers.
//!
//! A workaround is to use double indirection:
//!
//! ```rust
//! # use arc_swap::ArcSwap;
//! // This doesn't work:
//! // let data: ArcSwap<[u8]> = ArcSwap::new(Arc::from([1, 2, 3]));
//!
//! // But this does:
//! let data: ArcSwap<Box<[u8]>> = ArcSwap::from_pointee(Box::new([1, 2, 3]));
//! # drop(data);
//! ```
//!
//! [Arc]: https://doc.rust-lang.org/std/sync/struct.Arc.html
//! [Weak]: https://doc.rust-lang.org/std/sync/struct.Arc.html
//! [RwLock]: https://doc.rust-lang.org/std/sync/struct.RwLock.html
//! [Mutex]: https://doc.rust-lang.org/std/sync/struct.Mutex.html
//! [lock-free]: https://en.wikipedia.org/wiki/Non-blocking_algorithm#Lock-freedom
//! [wait-free]: https://en.wikipedia.org/wiki/Non-blocking_algorithm#Wait-freedom
//! [load]: struct.ArcSwapAny.html#method.load
//! [into_inner]: struct.ArcSwapAny.html#method.into_inner
//! [load_full]: struct.ArcSwapAny.html#method.load_full
//! [load_signal_safe]: struct.ArcSwapAny.html#method.peek_signal_safe
//! [benchmarks]: https://github.com/vorner/arc-swap/tree/master/benchmarks
//! [ArcSwapWeak]: type.ArcSwapWeak.html

pub mod access;
mod as_raw;
pub mod cache;
mod compile_fail_tests;
mod debt;
pub mod gen_lock;
mod ref_cnt;
#[cfg(feature = "weak")]
mod weak;

use std::fmt::{Debug, Display, Formatter, Result as FmtResult};
use std::isize;
use std::marker::PhantomData;
use std::mem::{self, ManuallyDrop};
use std::ops::Deref;
use std::process;
use std::ptr;
use std::sync::atomic::{self, AtomicPtr, AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;

use access::{Access, Map};
use as_raw::AsRaw;
pub use cache::Cache;
use debt::Debt;
use gen_lock::{Global, LockStorage, PrivateUnsharded, GEN_CNT};
pub use ref_cnt::RefCnt;

// # Implementation details
//
// The first idea would be to just use AtomicPtr with whatever the Arc::into_raw returns. Then
// replacing it would be fine (there's no need to update ref counts). The load needs to increment
// the reference count ‒ one still stays inside and another is returned to the caller. This is done
// by re-creating the Arc from the raw pointer and then cloning it, throwing one instance away
// (without destroying it).
//
// This approach has a problem. There's a short time between we read the raw pointer and increment
// the count. If some other thread replaces the stored Arc and throws it away, the ref count could
// drop to 0, get destroyed and we would be trying to bump ref counts in a ghost, which would be
// totally broken.
//
// To prevent this, we actually use two approaches in a hybrid manner.
//
// The first one is based on hazard pointers idea, but slightly modified. There's a global
// repository of pointers that owe a reference. When someone swaps a pointer, it walks this list
// and pays all the debts (and takes them out of the repository).
//
// For simplicity and performance, storing into the repository is fallible. If storing into the
// repository fails (because the thread used up all its own slots, or because the pointer got
// replaced in just the wrong moment and it can't confirm the reservation), unlike the full
// hazard-pointers approach, we don't retry, but fall back onto secondary strategy.
//
// Each reader registers itself so it can be tracked, but only as a number. Each writer first
// switches the pointer. Then it takes a snapshot of all the current readers and waits until all of
// them confirm bumping their reference count. Only then the writer returns to the caller, handing
// it the ownership of the Arc and allowing possible bad things (like being destroyed) to happen to
// it. This has its own disadvantages, so it is only the second approach.
//
// # Unsafety
//
// All the uses of the unsafe keyword is just to turn the raw pointer back to Arc. It originated
// from an Arc in the first place, so the only thing to ensure is it is still valid. That means its
// ref count never dropped to 0.
//
// At the beginning, there's ref count of 1 stored in the raw pointer (and maybe some others
// elsewhere, but we can't rely on these). This 1 stays there for the whole time the pointer is
// stored there. When the arc is replaced, this 1 is returned to the caller, so we just have to
// make sure no more readers access it by that time.
//
// # Tracking of readers
//
// The simple way would be to have a count of all readers that could be in the dangerous area
// between reading the pointer and bumping the reference count. We could „lock“ the ref count by
// incrementing this atomic counter and „unlock“ it when done. The writer would just have to
// busy-wait for this number to drop to 0 ‒ then there are no readers at all. This is safe, but a
// steady inflow of readers could make a writer wait forever.
//
// Therefore, we separate readers into two groups, odd and even ones (see below how). When we see
// both groups to drop to 0 (not necessarily at the same time, though), we are sure all the
// previous readers were flushed ‒ each of them had to be either odd or even.
//
// To do that, we define a generation. A generation is a number, incremented at certain times and a
// reader decides by this number if it is odd or even.
//
// One of the writers may increment the generation when it sees a zero in the next-generation's
// group (if the writer sees 0 in the odd group and the current generation is even, all the current
// writers are even ‒ so it remembers it saw odd-zero and increments the generation, so new readers
// start to appear in the odd group and the even has a chance to drop to zero later on). Only one
// writer does this switch, but all that witness the zero can remember it.
//
// We also split the reader threads into shards ‒ we have multiple copies of the counters, which
// prevents some contention and sharing of the cache lines. The writer reads them all and sums them
// up.
//
// # Leases and debts
//
// Instead of incrementing the reference count, the pointer reference can be owed. In such case, it
// is recorded into a global storage. As each thread has its own storage (the global storage is
// composed of multiple thread storages), the readers don't contend. When the pointer is no longer
// in use, the debt is erased.
//
// The writer pays all the existing debts, therefore the reader have the full Arc with ref count at
// that time. The reader is made aware the debt was paid and decrements the reference count.
//
// # Memory orders
//
// ## Synchronizing the data pointed to by the pointer.
//
// We have AcqRel (well, SeqCst, but that's included) on the swap and Acquire on the loads. In case
// of the double read around the debt allocation, we do that on the *second*, because of ABA.
// That's also why that SeqCst on the allocation of debt itself is not enough.
//
// ## The generation lock
//
// Second, the dangerous area when we borrowed the pointer but haven't yet incremented its ref
// count needs to stay between incrementing and decrementing the reader count (in either group). To
// accomplish that, using Acquire on the increment and Release on the decrement would be enough.
// The loads in the writer use Acquire to complete the edge and make sure no part of the dangerous
// area leaks outside of it in the writers view. This Acquire, however, forms the edge only with
// the *latest* decrement. By making both the increment and decrement AcqRel, we effectively chain
// the edges together.
//
// Now the hard part :-). We need to ensure that whatever zero a writer sees is not stale in the
// sense that it happened before the switch of the pointer. In other words, we need to make sure
// that at the time we start to look for the zeroes, we already see all the current readers. To do
// that, we need to synchronize the time lines of the pointer itself and the corresponding group
// counters. As these are separate, unrelated, atomics, it calls for SeqCst ‒ on the swap and on
// the increment. This'll guarantee that they'll know which happened first (either increment or the
// swap), making a base line for the following operations (load of the pointer or looking for
// zeroes).
//
// # Memory orders around debts
//
// The linked list of debt nodes only grows. The shape of the list (existence of nodes) is
// synchronized through Release on creation and Acquire on load on the head pointer.
//
// The debts work similar to locks ‒ Acquire and Release make all the pointer manipulation at the
// interval where it is written down. However, we use the SeqCst on the allocation of the debt for
// the same reason we do so with the generation lock.
//
// In case the writer pays the debt, it sees the new enough data (for the same reasons the stale
// zeroes are not seen). The reference count on the Arc is AcqRel and makes sure it is not
// destroyed too soon. The writer traverses all the slots, therefore they don't need to synchronize
// with each other.
//
// # Orderings on the rest
//
// We don't really care much if we use a stale generation number ‒ it only works to route the
// readers into one or another bucket, but even if it was completely wrong, it would only slow the
// waiting for 0 down. So, the increments of it are just hints.
//
// All other operations can be Relaxed (they either only claim something, which doesn't need to
// synchronize with anything else, or they are failed attempts at something ‒ and another attempt
// will be made, the successful one will do the necessary synchronization).

const MAX_GUARDS: usize = (isize::MAX) as usize;

/// Generation lock, to abstract locking and unlocking readers.
struct GenLock<'a> {
    slot: &'a AtomicUsize,
}

impl<'a> GenLock<'a> {
    /// Creates a generation lock.
    fn new<S: LockStorage + 'a>(signal_safe: SignalSafety, lock_storage: &'a S) -> Self {
        let shard = match signal_safe {
            SignalSafety::Safe => 0,
            SignalSafety::Unsafe => lock_storage.choose_shard(),
        };
        let gen = lock_storage.gen_idx().load(Ordering::Relaxed) % GEN_CNT;
        // SeqCst: Acquire, so the dangerous section stays in. SeqCst to sync timelines with the
        // swap on the ptr in writer thread.
        let slot = &lock_storage.shards().as_ref()[shard].0[gen];
        let old = slot.fetch_add(1, Ordering::SeqCst);
        // The trick is taken from Arc.
        if old > MAX_GUARDS {
            process::abort();
        }
        GenLock { slot }
    }

    /// Removes a generation lock.
    fn unlock(self) {
        // Release, so the dangerous section stays in. Acquire to chain the operations.
        self.slot.fetch_sub(1, Ordering::AcqRel);
        // Disarm the drop-bomb
        mem::forget(self);
    }
}

/// A bomb so one doesn't forget to unlock generations.
#[cfg(debug_assertions)] // The bomb actually makes it ~20% slower, so don't put it into production
impl<'a> Drop for GenLock<'a> {
    fn drop(&mut self) {
        unreachable!("Forgot to unlock generation");
    }
}

/// How the [Guard] content is protected.
enum Protection<'l> {
    /// The [Guard] contains independent value and doesn't have to be protected in any way.
    Unprotected,

    /// One ref-count is owed in the given debt and needs to be paid on release of the [Guard].
    Debt(&'static Debt),

    /// It is locked by a generation lock, needs to be unlocked.
    Lock(GenLock<'l>),
}

impl<'l> From<Option<&'static Debt>> for Protection<'l> {
    fn from(debt: Option<&'static Debt>) -> Self {
        match debt {
            Some(d) => Protection::Debt(d),
            None => Protection::Unprotected,
        }
    }
}

/// A temporary storage of the pointer.
///
/// This guard object is returned from most loading methods (with the notable exception of
/// [`load_full`](struct.ArcSwapAny.html#method.load_full)). It dereferences to the smart pointer
/// loaded, so most operations are to be done using that.
pub struct Guard<'l, T: RefCnt> {
    inner: ManuallyDrop<T>,
    protection: Protection<'l>,
}

impl<'a, T: RefCnt> Guard<'a, T> {
    fn new(ptr: *const T::Base, protection: Protection<'a>) -> Guard<'a, T> {
        Guard {
            inner: ManuallyDrop::new(unsafe { T::from_ptr(ptr) }),
            protection,
        }
    }

    /// Converts it into the held value.
    ///
    /// This, on occasion, may be a tiny bit faster than cloning the Arc or whatever is being held
    /// inside.
    // Associated function on purpose, because of deref
    #[cfg_attr(feature = "cargo-clippy", allow(wrong_self_convention))]
    #[inline]
    pub fn into_inner(mut lease: Self) -> T {
        // Drop any debt and release any lock held by the given guard and return a
        // full-featured value that even can outlive the ArcSwap it originated from.
        match mem::replace(&mut lease.protection, Protection::Unprotected) {
            // Not protected, nothing to unprotect.
            Protection::Unprotected => (),
            // If we owe, we need to create a new copy of the Arc. But if it gets payed in the
            // meantime, then we have to release it again, because it is extra. We can't check
            // first because of races.
            Protection::Debt(debt) => {
                T::inc(&lease.inner);
                let ptr = T::as_ptr(&lease.inner);
                if !debt.pay::<T>(ptr) {
                    unsafe { T::dec(ptr) };
                }
            }
            // If we had a lock, we first need to create our own copy, then unlock.
            Protection::Lock(lock) => {
                T::inc(&lease.inner);
                lock.unlock();
            }
        }

        // The ptr::read & forget is something like a cheating move. We can't move it out, because
        // we have a destructor and Rust doesn't allow us to do that.
        let inner = unsafe { ptr::read(lease.inner.deref()) };
        mem::forget(lease);
        inner
    }

    /// Create a guard for a given value `inner`.
    ///
    /// This can be useful on occasion to pass a specific object to code that expects or
    /// wants to store a Guard.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use arc_swap::{ArcSwap, Guard};
    /// # use std::sync::Arc;
    /// # let p = ArcSwap::from_pointee(42);
    /// // Create two guards pointing to the same object
    /// let g1 = p.load();
    /// let g2 = Guard::from_inner(Arc::clone(&*g1));
    /// # drop(g2);
    /// ```
    pub fn from_inner(inner: T) -> Self {
        Guard {
            inner: ManuallyDrop::new(inner),
            protection: Protection::Unprotected,
        }
    }
}

impl<'a, T: RefCnt> Deref for Guard<'a, T> {
    type Target = T;
    #[inline]
    fn deref(&self) -> &T {
        self.inner.deref()
    }
}

impl<'a, T: Debug + RefCnt> Debug for Guard<'a, T> {
    fn fmt(&self, formatter: &mut Formatter) -> FmtResult {
        self.deref().fmt(formatter)
    }
}

impl<'a, T: Display + RefCnt> Display for Guard<'a, T> {
    fn fmt(&self, formatter: &mut Formatter) -> FmtResult {
        self.deref().fmt(formatter)
    }
}

impl<'a, T: RefCnt> Drop for Guard<'a, T> {
    #[inline]
    fn drop(&mut self) {
        match mem::replace(&mut self.protection, Protection::Unprotected) {
            // We have our own copy of Arc, so we don't need a protection. Do nothing (but release
            // the Arc below).
            Protection::Unprotected => (),
            // If we owed something, just return the debt. We don't have a pointer owned, so
            // nothing to release.
            Protection::Debt(debt) => {
                let ptr = T::as_ptr(&self.inner);
                if debt.pay::<T>(ptr) {
                    return;
                }
                // But if the debt was already paid for us, we need to release the pointer, as we
                // were effectively already in the Unprotected mode.
            }
            // Similarly, we don't have anything owned, we just unlock and be done with it.
            Protection::Lock(lock) => {
                lock.unlock();
                return;
            }
        }
        // Equivalent to T::dec(ptr)
        unsafe { ManuallyDrop::drop(&mut self.inner) };
    }
}

/// Comparison of two pointer-like things.
// A and B are likely to *be* references, or thin wrappers around that. Calling that with extra
// reference is just annoying.
#[cfg_attr(feature = "cargo-clippy", allow(needless_pass_by_value))]
fn ptr_eq<Base, A, B>(a: A, b: B) -> bool
where
    A: AsRaw<Base>,
    B: AsRaw<Base>,
{
    let a = a.as_raw();
    let b = b.as_raw();
    ptr::eq(a, b)
}

#[derive(Copy, Clone)]
enum SignalSafety {
    Safe,
    Unsafe,
}

/// When waiting to something, yield the thread every so many iterations so something else might
/// get a chance to run and release whatever is being held.
const YIELD_EVERY: usize = 16;

/// An atomic storage for a reference counted smart pointer like [`Arc`] or `Option<Arc>`.
///
/// This is a storage where a smart pointer may live. It can be read and written atomically from
/// several threads, but doesn't act like a pointer itself.
///
/// One can be created [`from`] an [`Arc`]. To get the pointer back, use the
/// [`load`](#method.load).
///
/// # Note
///
/// This is the common generic implementation. This allows sharing the same code for storing
/// both `Arc` and `Option<Arc>` (and possibly other similar types).
///
/// In your code, you most probably want to interact with it through the
/// [`ArcSwap`](type.ArcSwap.html) and [`ArcSwapOption`](type.ArcSwapOption.html) aliases. However,
/// the methods they share are described here and are applicable to both of them. That's why the
/// examples here use `ArcSwap` ‒ but they could as well be written with `ArcSwapOption` or
/// `ArcSwapAny`.
///
/// # Type parameters
///
/// * `T`: The smart pointer to be kept inside. This crate provides implementation for `Arc<_>` and
///   `Option<Arc<_>>` (`Rc` too, but that one is not practically useful). But third party could
///   provide implementations of the [`RefCnt`] trait and plug in others.
/// * `S`: This describes where the generation lock is stored and how it works (this allows tuning
///   some of the performance trade-offs). See the [`LockStorage`][LockStorage] trait.
///
/// # Examples
///
/// ```rust
/// # use std::sync::Arc;
/// # use arc_swap::ArcSwap;
/// let arc = Arc::new(42);
/// let arc_swap = ArcSwap::from(arc);
/// assert_eq!(42, **arc_swap.load());
/// // It can be read multiple times
/// assert_eq!(42, **arc_swap.load());
///
/// // Put a new one in there
/// let new_arc = Arc::new(0);
/// assert_eq!(42, *arc_swap.swap(new_arc));
/// assert_eq!(0, **arc_swap.load());
/// ```
///
/// [`Arc`]: https://doc.rust-lang.org/std/sync/struct.Arc.html
/// [`from`]: https://doc.rust-lang.org/nightly/std/convert/trait.From.html#tymethod.from
/// [`RefCnt`]: trait.RefCnt.html
pub struct ArcSwapAny<T: RefCnt, S: LockStorage = Global> {
    // Notes: AtomicPtr needs Sized
    /// The actual pointer, extracted from the Arc.
    ptr: AtomicPtr<T::Base>,

    /// We are basically an Arc in disguise. Inherit parameters from Arc by pretending to contain
    /// it.
    _phantom_arc: PhantomData<T>,

    lock_storage: S,
}

impl<T: RefCnt, S: LockStorage> From<T> for ArcSwapAny<T, S> {
    fn from(val: T) -> Self {
        // The AtomicPtr requires *mut in its interface. We are more like *const, so we cast it.
        // However, we always go back to *const right away when we get the pointer on the other
        // side, so it should be fine.
        let ptr = T::into_ptr(val);
        Self {
            ptr: AtomicPtr::new(ptr),
            _phantom_arc: PhantomData,
            lock_storage: S::default(),
        }
    }
}

impl<T: RefCnt, S: LockStorage> Drop for ArcSwapAny<T, S> {
    fn drop(&mut self) {
        let ptr = *self.ptr.get_mut();
        // To pay any possible debts
        self.wait_for_readers(ptr);
        // We are getting rid of the one stored ref count
        unsafe { T::dec(ptr) };
    }
}

impl<T: RefCnt, S: LockStorage> Clone for ArcSwapAny<T, S> {
    fn clone(&self) -> Self {
        Self::from(self.load_full())
    }
}

impl<T, S: LockStorage> Debug for ArcSwapAny<T, S>
where
    T: Debug + RefCnt,
{
    fn fmt(&self, formatter: &mut Formatter) -> FmtResult {
        formatter
            .debug_tuple("ArcSwapAny")
            .field(&self.load())
            .finish()
    }
}

impl<T, S: LockStorage> Display for ArcSwapAny<T, S>
where
    T: Display + RefCnt,
{
    fn fmt(&self, formatter: &mut Formatter) -> FmtResult {
        self.load().fmt(formatter)
    }
}

impl<T: RefCnt + Default, S: LockStorage> Default for ArcSwapAny<T, S> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<T: RefCnt, S: LockStorage> ArcSwapAny<T, S> {
    /// Constructs a new value.
    pub fn new(val: T) -> Self {
        Self::from(val)
    }

    /// Extracts the value inside.
    pub fn into_inner(mut self) -> T {
        let ptr = *self.ptr.get_mut();
        // To pay all the debts
        self.wait_for_readers(ptr);
        mem::forget(self);
        unsafe { T::from_ptr(ptr) }
    }

    /// Loads the value.
    ///
    /// This makes another copy of the held pointer and returns it, atomically (it is
    /// safe even when other thread stores into the same instance at the same time).
    ///
    /// The method is lock-free and wait-free, but usually more expensive than
    /// [`load`](#method.load).
    pub fn load_full(&self) -> T {
        Guard::into_inner(self.load())
    }

    #[inline]
    fn lock_internal(&self, signal_safe: SignalSafety) -> Guard<'_, T> {
        let gen = GenLock::new(signal_safe, &self.lock_storage);
        let ptr = self.ptr.load(Ordering::Acquire);

        Guard::new(ptr, Protection::Lock(gen))
    }

    /// An async-signal-safe version of [`load`](#method.load)
    ///
    /// This method uses only restricted set of primitives to be async-signal-safe, so it can be
    /// used inside unix signal handlers. It has no advantages outside of them and it has its own
    /// downsides, so there's no reason to use it outside of them.
    ///
    /// # Warning
    ///
    /// While the method itself is lock-free (it will not be blocked by anything other threads do),
    /// methods that write are blocked from completion until the returned
    /// [`Guard`](struct.Guard.html) is dropped. This includes [`store`](#method.store),
    /// [`compare_and_swap`](#method.compare_and_swap) and [`rcu`](#method.rcu) and destruction of
    /// the `ArcSwapAny` instance.
    ///
    /// By default, the locks are *shared* across all the instances in the program, therefore it
    /// blocks writes even to *other* `ArcSwapAny` instances. It is possible to use a private lock
    /// (which is recommended if you want to do use this method) by using the
    /// [`IndependentArcSwap`](type.IndependentArcSwap.html) type alias.
    pub fn load_signal_safe(&self) -> Guard<'_, T> {
        self.lock_internal(SignalSafety::Safe)
    }

    #[inline]
    fn load_fallible(&self) -> Option<Guard<'static, T>> {
        // Relaxed is good enough here, see the Acquire below
        let ptr = self.ptr.load(Ordering::Relaxed);
        // Try to get a debt slot. If not possible, fail.
        let debt = Debt::new(ptr as usize)?;

        let confirm = self.ptr.load(Ordering::Acquire);
        if ptr == confirm {
            // Successfully got a debt
            Some(Guard::new(ptr, Protection::Debt(debt)))
        } else if debt.pay::<T>(ptr) {
            // It changed in the meantime, we return the debt (that is on the outdated pointer,
            // possibly destroyed) and fail.
            None
        } else {
            // It changed in the meantime, but the debt for the previous pointer was already paid
            // for by someone else, so we are fine using it.
            Some(Guard::new(ptr, Protection::Unprotected))
        }
    }

    /// Provides a temporary borrow of the object inside.
    ///
    /// This returns a proxy object allowing access to the thing held inside.  However, there's
    /// only limited amount of possible cheap proxies in existence for each thread ‒ if more are
    /// created, it falls back to equivalent of [`load_full`](#method.load_full) internally.
    ///
    /// This is therefore a good choice to use for eg. searching a data structure or juggling the
    /// pointers around a bit, but not as something to store in larger amounts. The rule of thumb
    /// is this is suited for local variables on stack, but not in long-living data structures.
    ///
    /// # Consistency
    ///
    /// In case multiple related operations are to be done on the loaded value, it is generally
    /// recommended to call `load` just once and keep the result over calling it multiple times.
    /// First, keeping it is usually faster. But more importantly, the value can change between the
    /// calls to load, returning different objects, which could lead to logical inconsistency.
    /// Keeping the result makes sure the same object is used.
    ///
    /// ```rust
    /// # use arc_swap::ArcSwap;
    /// struct Point {
    ///     x: usize,
    ///     y: usize,
    /// }
    ///
    /// fn print_broken(p: &ArcSwap<Point>) {
    ///     // This is broken, because the x and y may come from different points,
    ///     // combining into an invalid point that never existed.
    ///     println!("X: {}", p.load().x);
    ///     // If someone changes the content now, between these two loads, we
    ///     // have a problem
    ///     println!("Y: {}", p.load().y);
    /// }
    ///
    /// fn print_correct(p: &ArcSwap<Point>) {
    ///     // Here we take a snapshot of one specific point so both x and y come
    ///     // from the same one.
    ///     let point = p.load();
    ///     println!("X: {}", point.x);
    ///     println!("Y: {}", point.y);
    /// }
    /// # let p = ArcSwap::from_pointee(Point { x: 10, y: 20 });
    /// # print_correct(&p);
    /// # print_broken(&p);
    /// ```
    #[inline]
    pub fn load(&self) -> Guard<'static, T> {
        self.load_fallible().unwrap_or_else(|| {
            let locked = self.lock_internal(SignalSafety::Unsafe);
            // Extracting the object into a full-featured value has the
            // side effect of dropping the lock.
            Guard::from_inner(Guard::into_inner(locked))
        })
    }

    /// Replaces the value inside this instance.
    ///
    /// Further loads will yield the new value. Uses [`swap`](#method.swap) internally.
    pub fn store(&self, val: T) {
        drop(self.swap(val));
    }

    /// Exchanges the value inside this instance.
    ///
    /// Note that this method is *not* lock-free. In particular, it is possible to block this
    /// method by using the [`load_signal_safe`](#method.load_signal_safe), but
    /// [`load`](#method.load) may also block it for very short time (several CPU instructions). If
    /// this happens, `swap` will busy-wait in the meantime.
    ///
    /// It is also possible to cause a deadlock (eg. this is an example of *broken* code):
    ///
    /// ```rust,no_run
    /// # use std::sync::Arc;
    /// # use arc_swap::ArcSwap;
    /// let shared = ArcSwap::from(Arc::new(42));
    /// let guard = shared.load_signal_safe();
    /// // This will deadlock, because the guard is still active here and swap
    /// // can't pull the value from under its feet.
    /// shared.swap(Arc::new(0));
    /// # drop(guard);
    /// ```
    pub fn swap(&self, new: T) -> T {
        let new = T::into_ptr(new);
        // AcqRel needed to publish the target of the new pointer and get the target of the old
        // one.
        //
        // SeqCst to synchronize the time lines with the group counters.
        let old = self.ptr.swap(new, Ordering::SeqCst);
        self.wait_for_readers(old);
        unsafe { T::from_ptr(old) }
    }

    /// Swaps the stored Arc if it equals to `current`.
    ///
    /// If the current value of the `ArcSwapAny` equals to `current`, the `new` is stored inside.
    /// If not, nothing happens.
    ///
    /// The previous value (no matter if the swap happened or not) is returned. Therefore, if the
    /// returned value is equal to `current`, the swap happened. You want to do a pointer-based
    /// comparison to determine it.
    ///
    /// In other words, if the caller „guesses“ the value of current correctly, it acts like
    /// [`swap`](#method.swap), otherwise it acts like [`load_full`](#method.load_full) (including
    /// the limitations).
    ///
    /// The `current` can be specified as `&Arc`, [`Guard`](struct.Guard.html),
    /// [`&Guards`](struct.Guards.html) or as a raw pointer.
    pub fn compare_and_swap<C: AsRaw<T::Base>>(&self, current: C, new: T) -> Guard<T> {
        let cur_ptr = current.as_raw();
        let new = T::into_ptr(new);

        // As noted above, this method has either semantics of load or of store. We don't know
        // which ones upfront, so we need to implement safety measures for both.
        let gen = GenLock::new(SignalSafety::Unsafe, &self.lock_storage);

        let previous_ptr = self.ptr.compare_and_swap(cur_ptr, new, Ordering::SeqCst);
        let swapped = ptr::eq(cur_ptr, previous_ptr);

        // Drop it here, because:
        // * We can't drop it before the compare_and_swap ‒ in such case, it could get recycled,
        //   put into the pointer by another thread with a different value and create a fake
        //   success (ABA).
        // * We drop it before waiting for readers, because it could have been a Guard with a
        //   generation lock. In such case, the caller doesn't have it any more and can't check if
        //   it succeeded, but that's OK.
        drop(current);

        let debt = if swapped {
            // New went in, previous out, but their ref counts are correct. So nothing to do here.
            None
        } else {
            // Previous is a new copy of what is inside (and it stays there as well), so bump its
            // ref count. New is thrown away so dec its ref count (but do it outside of the
            // gen-lock).
            //
            // We try to do that by registering a debt and only if that fails by actually bumping
            // the ref.
            let debt = Debt::new(previous_ptr as usize);
            if debt.is_none() {
                let previous = unsafe { T::from_ptr(previous_ptr) };
                T::inc(&previous);
                T::into_ptr(previous);
            }
            debt
        };

        gen.unlock();

        if swapped {
            // We swapped. Before releasing the (possibly only) ref count of previous to user, wait
            // for all readers to make sure there are no more untracked copies of it.
            self.wait_for_readers(previous_ptr);
        } else {
            // We didn't swap, so new is black-holed.
            unsafe { T::dec(new) };
        }

        Guard::new(previous_ptr, debt.into())
    }

    /// Wait until all readers go away.
    fn wait_for_readers(&self, old: *const T::Base) {
        let mut seen_group = [false; GEN_CNT];
        let mut iter = 0usize;

        loop {
            // Note that we don't need the snapshot to be consistent. We just need to see both
            // halves being zero, not necessarily at the same time.
            let gen = self.lock_storage.gen_idx().load(Ordering::Relaxed);
            let groups = self
                .lock_storage
                .shards()
                .as_ref()
                .iter()
                .fold([0, 0], |[a1, a2], s| {
                    let [v1, v2] = s.snapshot();
                    [a1 + v1, a2 + v2]
                });
            // Should we increment the generation? Is the next one empty?
            let next_gen = gen.wrapping_add(1);
            if groups[next_gen % GEN_CNT] == 0 {
                // Replace it only if someone else didn't do it in the meantime
                self.lock_storage
                    .gen_idx()
                    .compare_and_swap(gen, next_gen, Ordering::Relaxed);
            }
            for i in 0..GEN_CNT {
                seen_group[i] = seen_group[i] || (groups[i] == 0);
            }

            if seen_group.iter().all(|seen| *seen) {
                break;
            }

            iter = iter.wrapping_add(1);
            if cfg!(not(miri)) {
                if iter % YIELD_EVERY == 0 {
                    thread::yield_now();
                } else {
                    atomic::spin_loop_hint();
                }
            }
        }
        Debt::pay_all::<T>(old);
    }

    /// Read-Copy-Update of the pointer inside.
    ///
    /// This is useful in read-heavy situations with several threads that sometimes update the data
    /// pointed to. The readers can just repeatedly use [`load`](#method.load) without any locking.
    /// The writer uses this method to perform the update.
    ///
    /// In case there's only one thread that does updates or in case the next version is
    /// independent of the previous one, simple [`swap`](#method.swap) or [`store`](#method.store)
    /// is enough. Otherwise, it may be needed to retry the update operation if some other thread
    /// made an update in between. This is what this method does.
    ///
    /// # Examples
    ///
    /// This will *not* work as expected, because between loading and storing, some other thread
    /// might have updated the value.
    ///
    /// ```rust
    /// # extern crate arc_swap;
    /// # extern crate crossbeam_utils;
    /// #
    /// # use std::sync::Arc;
    /// #
    /// # use arc_swap::ArcSwap;
    /// # use crossbeam_utils::thread;
    /// #
    /// let cnt = ArcSwap::from_pointee(0);
    /// thread::scope(|scope| {
    ///     for _ in 0..10 {
    ///         scope.spawn(|_| {
    ///            let inner = cnt.load_full();
    ///             // Another thread might have stored some other number than what we have
    ///             // between the load and store.
    ///             cnt.store(Arc::new(*inner + 1));
    ///         });
    ///     }
    /// }).unwrap();
    /// // This will likely fail:
    /// // assert_eq!(10, *cnt.load_full());
    /// ```
    ///
    /// This will, but it can call the closure multiple times to retry:
    ///
    /// ```rust
    /// # extern crate arc_swap;
    /// # extern crate crossbeam_utils;
    /// #
    /// # use arc_swap::ArcSwap;
    /// # use crossbeam_utils::thread;
    /// #
    /// let cnt = ArcSwap::from_pointee(0);
    /// thread::scope(|scope| {
    ///     for _ in 0..10 {
    ///         scope.spawn(|_| cnt.rcu(|inner| **inner + 1));
    ///     }
    /// }).unwrap();
    /// assert_eq!(10, *cnt.load_full());
    /// ```
    ///
    /// Due to the retries, you might want to perform all the expensive operations *before* the
    /// rcu. As an example, if there's a cache of some computations as a map, and the map is cheap
    /// to clone but the computations are not, you could do something like this:
    ///
    /// ```rust
    /// # extern crate arc_swap;
    /// # extern crate crossbeam_utils;
    /// # extern crate once_cell;
    /// #
    /// # use std::collections::HashMap;
    /// #
    /// # use arc_swap::ArcSwap;
    /// # use once_cell::sync::Lazy;
    /// #
    /// fn expensive_computation(x: usize) -> usize {
    ///     x * 2 // Let's pretend multiplication is *really expensive expensive*
    /// }
    ///
    /// type Cache = HashMap<usize, usize>;
    ///
    /// static CACHE: Lazy<ArcSwap<Cache>> = Lazy::new(|| ArcSwap::default());
    ///
    /// fn cached_computation(x: usize) -> usize {
    ///     let cache = CACHE.load();
    ///     if let Some(result) = cache.get(&x) {
    ///         return *result;
    ///     }
    ///     // Not in cache. Compute and store.
    ///     // The expensive computation goes outside, so it is not retried.
    ///     let result = expensive_computation(x);
    ///     CACHE.rcu(|cache| {
    ///         // The cheaper clone of the cache can be retried if need be.
    ///         let mut cache = HashMap::clone(&cache);
    ///         cache.insert(x, result);
    ///         cache
    ///     });
    ///     result
    /// }
    ///
    /// assert_eq!(42, cached_computation(21));
    /// assert_eq!(42, cached_computation(21));
    /// ```
    ///
    /// # The cost of cloning
    ///
    /// Depending on the size of cache above, the cloning might not be as cheap. You can however
    /// use persistent data structures ‒ each modification creates a new data structure, but it
    /// shares most of the data with the old one (which is usually accomplished by using `Arc`s
    /// inside to share the unchanged values). Something like
    /// [`rpds`](https://crates.io/crates/rpds) or [`im`](https://crates.io/crates/im) might do
    /// what you need.
    pub fn rcu<R, F>(&self, mut f: F) -> T
    where
        F: FnMut(&T) -> R,
        R: Into<T>,
    {
        let mut cur = self.load();
        loop {
            let new = f(&cur).into();
            let prev = self.compare_and_swap(&cur, new);
            let swapped = ptr_eq(&cur, &prev);
            if swapped {
                return Guard::into_inner(prev);
            } else {
                cur = prev;
            }
        }
    }

    /// Provides an access to an up to date projection of the carried data.
    ///
    /// # Motivation
    ///
    /// Sometimes, an application consists of components. Each component has its own configuration
    /// structure. The whole configuration contains all the smaller config parts.
    ///
    /// For the sake of separation and abstraction, it is not desirable to pass the whole
    /// configuration to each of the components. This allows the component to take only access to
    /// its own part.
    ///
    /// # Lifetimes & flexibility
    ///
    /// This method is not the most flexible way, as the returned type borrows into the `ArcSwap`.
    /// To provide access into eg. `Arc<ArcSwap<T>>`, you can create the [`Map`] type directly.
    ///
    /// # Performance
    ///
    /// As the provided function is called on each load from the shared storage, it should
    /// generally be cheap. It is expected this will usually be just referencing of a field inside
    /// the structure.
    ///
    /// # Examples
    ///
    /// ```rust
    /// extern crate arc_swap;
    /// extern crate crossbeam_utils;
    ///
    /// use std::sync::Arc;
    ///
    /// use arc_swap::ArcSwap;
    /// use arc_swap::access::Access;
    ///
    /// struct Cfg {
    ///     value: usize,
    /// }
    ///
    /// fn print_many_times<V: Access<usize>>(value: V) {
    ///     for _ in 0..25 {
    ///         let value = value.load();
    ///         println!("{}", *value);
    ///     }
    /// }
    ///
    /// let shared = ArcSwap::from_pointee(Cfg { value: 0 });
    /// let mapped = shared.map(|c: &Cfg| &c.value);
    /// crossbeam_utils::thread::scope(|s| {
    ///     // Will print some zeroes and some twos
    ///     s.spawn(|_| print_many_times(mapped));
    ///     s.spawn(|_| shared.store(Arc::new(Cfg { value: 2 })));
    /// }).expect("Something panicked in a thread");
    /// ```
    pub fn map<I, R, F>(&self, f: F) -> Map<&Self, I, F>
    where
        F: Fn(&I) -> &R,
        Self: Access<I>,
    {
        Map::new(self, f)
    }
}

/// An atomic storage for `Arc`.
///
/// This is a type alias only. Most of its methods are described on
/// [`ArcSwapAny`](struct.ArcSwapAny.html).
pub type ArcSwap<T> = ArcSwapAny<Arc<T>>;

impl<T, S: LockStorage> ArcSwapAny<Arc<T>, S> {
    /// A convenience constructor directly from the pointed-to value.
    ///
    /// Direct equivalent for `ArcSwap::new(Arc::new(val))`.
    pub fn from_pointee(val: T) -> Self {
        Self::from(Arc::new(val))
    }

    /// An [`rcu`](struct.ArcSwapAny.html#method.rcu) which waits to be the sole owner of the
    /// original value and unwraps it.
    ///
    /// This one works the same way as the [`rcu`](struct.ArcSwapAny.html#method.rcu) method, but
    /// works on the inner type instead of `Arc`. After replacing the original, it waits until
    /// there are no other owners of the arc and unwraps it.
    ///
    /// Possible use case might be an RCU with a structure that is rather slow to drop ‒ if it was
    /// left to random reader (the last one to hold the old value), it could cause a timeout or
    /// jitter in a query time. With this, the deallocation is done in the updater thread,
    /// therefore outside of the hot path.
    ///
    /// # Warning
    ///
    /// Note that if you store a copy of the `Arc` somewhere except the `ArcSwap` itself for
    /// extended period of time, this'll busy-wait the whole time. Unless you need the assurance
    /// the `Arc` is deconstructed here, prefer [`rcu`](#method.rcu).
    pub fn rcu_unwrap<R, F>(&self, mut f: F) -> T
    where
        F: FnMut(&T) -> R,
        R: Into<Arc<T>>,
    {
        let mut wrapped = self.rcu(|prev| f(&*prev));
        loop {
            match Arc::try_unwrap(wrapped) {
                Ok(val) => return val,
                Err(w) => {
                    wrapped = w;
                    thread::yield_now();
                }
            }
        }
    }
}

/// An atomic storage for `Option<Arc>`.
///
/// This is very similar to [`ArcSwap`](type.ArcSwap.html), but allows storing NULL values, which
/// is useful in some situations.
///
/// This is a type alias only. Most of the methods are described on
/// [`ArcSwapAny`](struct.ArcSwapAny.html). Even though the examples there often use `ArcSwap`,
/// they are applicable to `ArcSwapOption` with appropriate changes.
///
/// # Examples
///
/// ```
/// use std::sync::Arc;
/// use arc_swap::ArcSwapOption;
///
/// let shared = ArcSwapOption::from(None);
/// assert!(shared.load_full().is_none());
/// assert!(shared.swap(Some(Arc::new(42))).is_none());
/// assert_eq!(42, **shared.load_full().as_ref().unwrap());
/// ```
pub type ArcSwapOption<T> = ArcSwapAny<Option<Arc<T>>>;

impl<T, S: LockStorage> ArcSwapAny<Option<Arc<T>>, S> {
    /// A convenience constructor directly from a pointed-to value.
    ///
    /// This just allocates the `Arc` under the hood.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use arc_swap::ArcSwapOption;
    ///
    /// let empty: ArcSwapOption<usize> = ArcSwapOption::from_pointee(None);
    /// assert!(empty.load().is_none());
    /// let non_empty: ArcSwapOption<usize> = ArcSwapOption::from_pointee(42);
    /// assert_eq!(42, **non_empty.load().as_ref().unwrap());
    /// ```
    pub fn from_pointee<V: Into<Option<T>>>(val: V) -> Self {
        Self::new(val.into().map(Arc::new))
    }

    /// A convenience constructor for an empty value.
    ///
    /// This is equivalent to `ArcSwapOption::new(None)`.
    pub fn empty() -> Self {
        Self::new(None)
    }
}

/// An atomic storage that doesn't share the internal generation locks with others.
///
/// This makes it bigger and it also might suffer contention (on the HW level) if used from many
/// threads at once. But using [`load_signal_safe`](struct.ArcSwapAny.html#method.load_signal_safe)
/// will not block writes on other instances.
///
/// ```rust
/// # use std::sync::Arc;
/// # use arc_swap::{ArcSwap, IndependentArcSwap};
/// // This one shares locks with others.
/// let shared = ArcSwap::from_pointee(42);
/// // But this one has an independent lock.
/// let independent = IndependentArcSwap::from_pointee(42);
///
/// // This'll hold a lock so any writers there wouldn't complete
/// let l = independent.load_signal_safe();
/// // But the lock doesn't influence the shared one, so this goes through just fine
/// shared.store(Arc::new(43));
///
/// assert_eq!(42, **l);
/// ```
pub type IndependentArcSwap<T> = ArcSwapAny<Arc<T>, PrivateUnsharded>;

/// Arc swap for the [Weak] pointer.
///
/// This is similar to [ArcSwap], but it doesn't store [Arc], it stores [Weak]. It doesn't keep the
/// data alive when pointed to.
///
/// This is a type alias only. Most of the methods are described on the
/// [`ArcSwapAny`](struct.ArcSwapAny.html).
///
/// [Weak]: std::sync::Weak
#[cfg(feature = "weak")]
pub type ArcSwapWeak<T> = ArcSwapAny<std::sync::Weak<T>>;

#[cfg(test)]
mod tests {
    extern crate crossbeam_utils;

    use std::panic;
    use std::sync::atomic::AtomicUsize;
    use std::sync::Barrier;

    use self::crossbeam_utils::thread;

    use super::*;

    /// Similar to the one in doc tests of the lib, but more times and more intensive (we want to
    /// torture it a bit).
    ///
    /// Takes some time, presumably because this starts 21 000 threads during its lifetime and 20
    /// 000 of them just wait in a tight loop for the other thread to happen.
    #[test]
    fn publish() {
        for _ in 0..100 {
            let config = ArcSwap::<String>::default();
            let ended = AtomicUsize::new(0);
            thread::scope(|scope| {
                for _ in 0..20 {
                    scope.spawn(|_| loop {
                        let cfg = config.load_full();
                        if !cfg.is_empty() {
                            assert_eq!(*cfg, "New configuration");
                            ended.fetch_add(1, Ordering::Relaxed);
                            return;
                        }
                        atomic::spin_loop_hint();
                    });
                }
                scope.spawn(|_| {
                    let new_conf = Arc::new("New configuration".to_owned());
                    config.store(new_conf);
                });
            })
            .unwrap();
            assert_eq!(20, ended.load(Ordering::Relaxed));
            let arc = config.load_full();
            assert_eq!(2, Arc::strong_count(&arc));
            assert_eq!(0, Arc::weak_count(&arc));
        }
    }

    /// Similar to the doc tests of ArcSwap, but happens more times.
    #[test]
    fn swap_load() {
        for _ in 0..100 {
            let arc = Arc::new(42);
            let arc_swap = ArcSwap::from(Arc::clone(&arc));
            assert_eq!(42, **arc_swap.load());
            // It can be read multiple times
            assert_eq!(42, **arc_swap.load());

            // Put a new one in there
            let new_arc = Arc::new(0);
            assert_eq!(42, *arc_swap.swap(Arc::clone(&new_arc)));
            assert_eq!(0, **arc_swap.load());
            // One loaded here, one in the arc_swap, one in new_arc
            let loaded = arc_swap.load_full();
            assert_eq!(3, Arc::strong_count(&loaded));
            assert_eq!(0, Arc::weak_count(&loaded));
            // The original got released from the arc_swap
            assert_eq!(1, Arc::strong_count(&arc));
            assert_eq!(0, Arc::weak_count(&arc));
        }
    }

    /// Two different writers publish two series of values. The readers check that it is always
    /// increasing in each serie.
    ///
    /// For performance, we try to reuse the threads here.
    #[test]
    fn multi_writers() {
        let first_value = Arc::new((0, 0));
        let shared = ArcSwap::from(Arc::clone(&first_value));
        const WRITER_CNT: usize = 2;
        const READER_CNT: usize = 3;
        const ITERATIONS: usize = 100;
        const SEQ: usize = 50;
        let barrier = Barrier::new(READER_CNT + WRITER_CNT);
        thread::scope(|scope| {
            for w in 0..WRITER_CNT {
                // We need to move w into the closure. But we want to just reference the other
                // things.
                let barrier = &barrier;
                let shared = &shared;
                let first_value = &first_value;
                scope.spawn(move |_| {
                    for _ in 0..ITERATIONS {
                        barrier.wait();
                        shared.store(Arc::clone(&first_value));
                        barrier.wait();
                        for i in 0..SEQ {
                            shared.store(Arc::new((w, i + 1)));
                        }
                    }
                });
            }
            for _ in 0..READER_CNT {
                scope.spawn(|_| {
                    for _ in 0..ITERATIONS {
                        barrier.wait();
                        barrier.wait();
                        let mut previous = [0; 2];
                        let mut last = Arc::clone(&first_value);
                        loop {
                            let cur = shared.load();
                            if Arc::ptr_eq(&last, &cur) {
                                atomic::spin_loop_hint();
                                continue;
                            }
                            let (w, s) = **cur;
                            assert!(previous[w] < s);
                            previous[w] = s;
                            last = Guard::into_inner(cur);
                            if s == SEQ {
                                break;
                            }
                        }
                    }
                });
            }
        })
        .unwrap();
    }

    #[test]
    /// Make sure the reference count and compare_and_swap works as expected.
    fn cas_ref_cnt() {
        const ITERATIONS: usize = 50;
        let shared = ArcSwap::from(Arc::new(0));
        for i in 0..ITERATIONS {
            let orig = shared.load_full();
            assert_eq!(i, *orig);
            if i % 2 == 1 {
                // One for orig, one for shared
                assert_eq!(2, Arc::strong_count(&orig));
            }
            let n1 = Arc::new(i + 1);
            // Fill up the slots sometimes
            let fillup = || {
                if i % 2 == 0 {
                    Some((0..50).map(|_| shared.load()).collect::<Vec<_>>())
                } else {
                    None
                }
            };
            let guards = fillup();
            // Success
            let prev = shared.compare_and_swap(&orig, Arc::clone(&n1));
            assert!(ptr_eq(&orig, &prev));
            drop(guards);
            // One for orig, one for prev
            assert_eq!(2, Arc::strong_count(&orig));
            // One for n1, one for shared
            assert_eq!(2, Arc::strong_count(&n1));
            assert_eq!(i + 1, **shared.load());
            let n2 = Arc::new(i);
            drop(prev);
            let guards = fillup();
            // Failure
            let prev = Guard::into_inner(shared.compare_and_swap(&orig, Arc::clone(&n2)));
            drop(guards);
            assert!(ptr_eq(&n1, &prev));
            // One for orig
            assert_eq!(1, Arc::strong_count(&orig));
            // One for n1, one for shared, one for prev
            assert_eq!(3, Arc::strong_count(&n1));
            // n2 didn't get increased
            assert_eq!(1, Arc::strong_count(&n2));
            assert_eq!(i + 1, **shared.load());
        }

        let a = shared.load_full();
        // One inside shared, one for a
        assert_eq!(2, Arc::strong_count(&a));
        drop(shared);
        // Only a now
        assert_eq!(1, Arc::strong_count(&a));
    }

    #[test]
    /// Multiple RCUs interacting.
    fn rcu() {
        const ITERATIONS: usize = 50;
        const THREADS: usize = 10;
        let shared = ArcSwap::from(Arc::new(0));
        thread::scope(|scope| {
            for _ in 0..THREADS {
                scope.spawn(|_| {
                    for _ in 0..ITERATIONS {
                        shared.rcu(|old| **old + 1);
                    }
                });
            }
        })
        .unwrap();
        assert_eq!(THREADS * ITERATIONS, **shared.load());
    }

    #[test]
    /// Multiple RCUs interacting, with unwrapping.
    fn rcu_unwrap() {
        const ITERATIONS: usize = 50;
        const THREADS: usize = 10;
        let shared = ArcSwap::from(Arc::new(0));
        thread::scope(|scope| {
            for _ in 0..THREADS {
                scope.spawn(|_| {
                    for _ in 0..ITERATIONS {
                        shared.rcu_unwrap(|old| *old + 1);
                    }
                });
            }
        })
        .unwrap();
        assert_eq!(THREADS * ITERATIONS, **shared.load());
    }

    /// Handling null/none values
    #[test]
    fn nulls() {
        let shared = ArcSwapOption::from(Some(Arc::new(0)));
        let orig = shared.swap(None);
        assert_eq!(1, Arc::strong_count(&orig.unwrap()));
        let null = shared.load();
        assert!(null.is_none());
        let a = Arc::new(42);
        let orig = shared.compare_and_swap(ptr::null(), Some(Arc::clone(&a)));
        assert!(orig.is_none());
        assert_eq!(2, Arc::strong_count(&a));
        let orig = Guard::into_inner(shared.compare_and_swap(&None::<Arc<_>>, None));
        assert_eq!(3, Arc::strong_count(&a));
        assert!(ptr_eq(&a, &orig));
    }

    /// We have a callback in RCU. Check what happens if we access the value from within.
    #[test]
    fn recursive() {
        let shared = ArcSwap::from(Arc::new(0));

        shared.rcu(|i| {
            if **i < 10 {
                shared.rcu(|i| **i + 1);
            }
            **i
        });
        assert_eq!(10, **shared.load());
        assert_eq!(2, Arc::strong_count(&shared.load_full()));
    }

    /// A panic from within the rcu callback should not change anything.
    #[test]
    fn rcu_panic() {
        let shared = ArcSwap::from(Arc::new(0));
        assert!(panic::catch_unwind(|| shared.rcu(|_| -> usize { panic!() })).is_err());
        assert_eq!(1, Arc::strong_count(&shared.swap(Arc::new(42))));
    }

    /// Accessing the value inside ArcSwap with Guards (and checks for the reference counts).
    #[test]
    fn load_cnt() {
        let a = Arc::new(0);
        let shared = ArcSwap::from(Arc::clone(&a));
        // One in shared, one in a
        assert_eq!(2, Arc::strong_count(&a));
        let guard = shared.load();
        assert_eq!(0, **guard);
        // The guard doesn't have its own ref count now
        assert_eq!(2, Arc::strong_count(&a));
        let guard_2 = shared.load();
        // Unlike with guard, this does not deadlock
        shared.store(Arc::new(1));
        // But now, each guard got a full Arc inside it
        assert_eq!(3, Arc::strong_count(&a));
        // And when we get rid of them, they disappear
        drop(guard_2);
        assert_eq!(2, Arc::strong_count(&a));
        let _b = Arc::clone(&guard);
        assert_eq!(3, Arc::strong_count(&a));
        // We can drop the guard it came from
        drop(guard);
        assert_eq!(2, Arc::strong_count(&a));
        let guard = shared.load();
        assert_eq!(1, **guard);
        drop(shared);
        // We can still use the guard after the shared disappears
        assert_eq!(1, **guard);
        let ptr = Arc::clone(&guard);
        // One in shared, one in guard
        assert_eq!(2, Arc::strong_count(&ptr));
        drop(guard);
        assert_eq!(1, Arc::strong_count(&ptr));
    }

    /// There can be only limited amount of leases on one thread. Following ones are created, but
    /// contain full Arcs.
    #[test]
    fn lease_overflow() {
        let a = Arc::new(0);
        let shared = ArcSwap::from(Arc::clone(&a));
        assert_eq!(2, Arc::strong_count(&a));
        let mut guards = (0..1000).map(|_| shared.load()).collect::<Vec<_>>();
        let count = Arc::strong_count(&a);
        assert!(count > 2);
        let guard = shared.load();
        assert_eq!(count + 1, Arc::strong_count(&a));
        drop(guard);
        assert_eq!(count, Arc::strong_count(&a));
        // When we delete the first one, it didn't have an Arc in it, so the ref count doesn't drop
        guards.swap_remove(0);
        // But new one reuses now vacant the slot and doesn't create a new Arc
        let _guard = shared.load();
        assert_eq!(count, Arc::strong_count(&a));
    }

    #[test]
    fn load_null() {
        let shared = ArcSwapOption::<usize>::default();
        let guard = shared.load();
        assert!(guard.is_none());
        shared.store(Some(Arc::new(42)));
        assert_eq!(42, **shared.load().as_ref().unwrap());
    }

    #[test]
    fn from_into() {
        let a = Arc::new(42);
        let shared = ArcSwap::new(a);
        let guard = shared.load();
        let a = shared.into_inner();
        assert_eq!(42, *a);
        assert_eq!(2, Arc::strong_count(&a));
        drop(guard);
        assert_eq!(1, Arc::strong_count(&a));
    }

    // Note on the Relaxed order here. This should be enough, because there's that barrier.wait
    // in between that should do the synchronization of happens-before for us. And using SeqCst
    // would probably not help either, as there's nothing else with SeqCst here in this test to
    // relate it to.
    #[derive(Default)]
    struct ReportDrop(Arc<AtomicUsize>);
    impl Drop for ReportDrop {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::Relaxed);
        }
    }

    const ITERATIONS: usize = 50;

    /// Interaction of two threads about a guard and dropping it.
    ///
    /// We make sure everything works in timely manner (eg. dropping of stuff) even if multiple
    /// threads interact.
    ///
    /// The idea is:
    /// * Thread 1 loads a value.
    /// * Thread 2 replaces the shared value. The original value is not destroyed.
    /// * Thread 1 drops the guard. The value is destroyed and this is observable in both threads.
    #[test]
    fn guard_drop_in_thread() {
        for _ in 0..ITERATIONS {
            let cnt = Arc::new(AtomicUsize::new(0));

            let shared = ArcSwap::from_pointee(ReportDrop(cnt.clone()));
            assert_eq!(cnt.load(Ordering::Relaxed), 0, "Dropped prematurely");
            // We need the threads to wait for each other at places.
            let sync = Barrier::new(2);

            thread::scope(|scope| {
                scope.spawn(|_| {
                    let guard = shared.load();
                    sync.wait();
                    // Thread 2 replaces the shared value. We wait for it to confirm.
                    sync.wait();
                    drop(guard);
                    assert_eq!(cnt.load(Ordering::Relaxed), 1, "Value not dropped");
                    // Let thread 2 know we already dropped it.
                    sync.wait();
                });

                scope.spawn(|_| {
                    // Thread 1 loads, we wait for that
                    sync.wait();
                    shared.store(Default::default());
                    assert_eq!(cnt.load(Ordering::Relaxed), 0, "Dropped while still in use");
                    // Let thread 2 know we replaced it
                    sync.wait();
                    // Thread 1 drops its guard. We wait for it to confirm.
                    sync.wait();
                    assert_eq!(cnt.load(Ordering::Relaxed), 1, "Value not dropped");
                });
            })
            .unwrap();
        }
    }

    /// Check dropping a lease in a different thread than it was created doesn't cause any
    /// problems.
    #[test]
    fn guard_drop_in_another_thread() {
        for _ in 0..ITERATIONS {
            let cnt = Arc::new(AtomicUsize::new(0));
            let shared = ArcSwap::from_pointee(ReportDrop(cnt.clone()));
            assert_eq!(cnt.load(Ordering::Relaxed), 0, "Dropped prematurely");
            let guard = shared.load();

            drop(shared);
            assert_eq!(cnt.load(Ordering::Relaxed), 0, "Dropped prematurely");

            thread::scope(|scope| {
                scope.spawn(|_| {
                    drop(guard);
                });
            })
            .unwrap();

            assert_eq!(cnt.load(Ordering::Relaxed), 1, "Not dropped");
        }
    }

    /// Similar, but for peek guard.
    #[test]
    fn signal_drop_in_another_thread() {
        for _ in 0..ITERATIONS {
            let cnt = Arc::new(AtomicUsize::new(0));
            let shared = ArcSwap::from_pointee(ReportDrop(cnt.clone()));
            assert_eq!(cnt.load(Ordering::Relaxed), 0, "Dropped prematurely");
            let guard = shared.load_signal_safe();

            // We can't drop here, sorry. Or, not even replace, as that would deadlock.

            thread::scope(|scope| {
                scope.spawn(|_| {
                    drop(guard);
                });

                assert_eq!(cnt.load(Ordering::Relaxed), 0, "Dropped prematurely");
                shared.swap(Default::default());
                assert_eq!(cnt.load(Ordering::Relaxed), 1, "Not dropped");
            })
            .unwrap();
        }
    }

    #[test]
    fn load_option() {
        let shared = ArcSwapOption::from_pointee(42);
        // The type here is not needed in real code, it's just addition test the type matches.
        let opt: Option<_> = Guard::into_inner(shared.load());
        assert_eq!(42, *opt.unwrap());

        shared.store(None);
        assert!(shared.load().is_none());
    }

    // Check stuff can get formatted
    #[test]
    fn debug_impl() {
        let shared = ArcSwap::from_pointee(42);
        assert_eq!("ArcSwapAny(42)", &format!("{:?}", shared));
        assert_eq!("42", &format!("{:?}", shared.load()));
    }

    #[test]
    fn display_impl() {
        let shared = ArcSwap::from_pointee(42);
        assert_eq!("42", &format!("{}", shared));
        assert_eq!("42", &format!("{}", shared.load()));
    }

    // The following "tests" are not run, only compiled. They check that things that should be
    // Send/Sync actually are.
    fn _check_stuff_is_send_sync() {
        let shared = ArcSwap::from_pointee(42);
        let moved = ArcSwap::from_pointee(42);
        let shared_ref = &shared;
        let lease = shared.load();
        let lease_ref = &lease;
        let lease = shared.load();
        let guard = shared.load_signal_safe();
        let guard_ref = &guard;
        let guard = shared.load_signal_safe();
        thread::scope(|s| {
            s.spawn(move |_| {
                let _ = guard;
                let _ = guard_ref;
                let _ = lease;
                let _ = lease_ref;
                let _ = shared_ref;
                let _ = moved;
            });
        })
        .unwrap();
    }
}
