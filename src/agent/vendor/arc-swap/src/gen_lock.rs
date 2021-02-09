//! Customization of where and how the generation lock works.
//!
//! By default, all the [`ArcSwapAny`](../struct.ArcSwapAny.html) instances share the same
//! generation lock. This is to save space in them (they have the same size as a single pointer),
//! because the default lock is quite a large data structure (it's sharded, to prevent too much
//! contention between different threads). This has the disadvantage that a lock on one instance
//! influences another instance.
//!
//! The things in this module allow customizing how the lock behaves. The default one is
//! [`Global`](struct.Global.html). If you want to use independent but unsharded lock, use the
//! [`PrivateUnsharded`](struct.PrivateUnsharded.html) (or the
//! [`IndependentArcSwap`](../type.IndependentArcSwap.html) type alias).
//!
//! Or you can implement your own lock, but you probably should study the internals of the library
//! first.
//!
//! # Not Implemented Yet
//!
//! These variants would probably make sense, but haven't been written yet:
//!
//! * A lock storage that is shared, but only between a certain group of pointers. It could be
//!   either as a reference (but then each `ArcSwap` would get a bit bigger), or a macro that could
//!   generate an independent but global storage.

use std::cell::Cell;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Number of shards (see [`Shard`]).
const SHARD_CNT: usize = 9;

/// How many generations we have in the lock.
pub(crate) const GEN_CNT: usize = 2;

/// A single shard.
///
/// This is one copy of place where the library keeps tracks of generation locks. It consists of a
/// pair of counters and allows double-buffering readers (therefore, even if there's a never-ending
/// stream of readers coming in, writer will get through eventually).
///
/// To avoid contention and sharing of the counters between readers, we don't have one pair of
/// generation counters, but several. The reader picks one shard and uses that, while the writer
/// looks through all of them. This is still not perfect (two threads may choose the same ID), but
/// it helps.
///
/// Each [`LockStorage`](trait.LockStorage.html) must provide a (non-empty) array of these.
#[repr(align(64))]
#[derive(Default)]
pub struct Shard(pub(crate) [AtomicUsize; GEN_CNT]);

impl Shard {
    /// Takes a snapshot of current values (with Acquire ordering)
    pub(crate) fn snapshot(&self) -> [usize; GEN_CNT] {
        [
            self.0[0].load(Ordering::Acquire),
            self.0[1].load(Ordering::Acquire),
        ]
    }
}

/// Abstraction of the place where generation locks are stored.
///
/// The trait is unsafe because if the trait messes up with the values stored in there in any way
/// (or makes the values available to something else that messes them up), this can cause UB and
/// daemons and discomfort to users and such. The library expects it is the only one storing values
/// there. In other words, it is expected the trait is only a dumb storage and doesn't actively do
/// anything.
pub unsafe trait LockStorage: Default {
    /// The type for keeping several shards.
    ///
    /// In general, it is expected to be a fixed-size array, but different implementations can have
    /// different sizes.
    type Shards: AsRef<[Shard]>;

    /// Access to the generation index.
    ///
    /// Must return the same instance of the `AtomicUsize` for the lifetime of the storage, must
    /// start at `0` and the trait itself must not modify it. Must be async-signal-safe.
    fn gen_idx(&self) -> &AtomicUsize;

    /// Access to the shards storage.
    ///
    /// Must return the same instance of the shards for the lifetime of the storage. Must start
    /// zeroed-out and the trait itself must not modify it.
    fn shards(&self) -> &Self::Shards;

    /// Pick one shard of the all selected.
    ///
    /// Returns the index of one of the shards. The choice can be arbitrary, but it should be fast
    /// and avoid collisions.
    fn choose_shard(&self) -> usize;
}

static GEN_IDX: AtomicUsize = AtomicUsize::new(0);

macro_rules! sh {
    () => {
        Shard([AtomicUsize::new(0), AtomicUsize::new(0)])
    };
}

type Shards = [Shard; SHARD_CNT];

/// The global shards.
static SHARDS: [Shard; SHARD_CNT] = [
    sh!(),
    sh!(),
    sh!(),
    sh!(),
    sh!(),
    sh!(),
    sh!(),
    sh!(),
    sh!(),
];

/// Global counter of threads.
///
/// We specifically don't use ThreadId here, because it is opaque and doesn't give us a number :-(.
static THREAD_ID_GEN: AtomicUsize = AtomicUsize::new(0);

thread_local! {
    /// A shard a thread has chosen.
    ///
    /// The default value is just a marker it hasn't been set.
    static THREAD_SHARD: Cell<usize> = Cell::new(SHARD_CNT);
}

/// The default, global lock.
///
/// The lock is stored out-of-band, globally. This means that one `ArcSwap` with this lock storage
/// is only one machine word large, but a lock on one instance blocks the other, independent ones.
///
/// It has several shards so threads are less likely to collide (HW-contend) on them.
#[derive(Default)]
pub struct Global;

unsafe impl LockStorage for Global {
    type Shards = Shards;

    #[inline]
    fn gen_idx(&self) -> &AtomicUsize {
        &GEN_IDX
    }

    #[inline]
    fn shards(&self) -> &Shards {
        &SHARDS
    }

    #[inline]
    fn choose_shard(&self) -> usize {
        THREAD_SHARD
            .try_with(|ts| {
                let mut val = ts.get();
                if val >= SHARD_CNT {
                    val = THREAD_ID_GEN.fetch_add(1, Ordering::Relaxed) % SHARD_CNT;
                    ts.set(val);
                }
                val
            })
            .unwrap_or(0)
    }
}

/// A single „shard“ that is stored inline, inside the corresponding `ArcSwap`. Therefore, locks on
/// each instance won't influence any other instances. On the other hand, the `ArcSwap` itself gets
/// bigger and doesn't have multiple shards, so concurrent uses might contend each other a bit.
///
/// ```rust
/// # use std::sync::Arc;
/// # use arc_swap::{ArcSwap, ArcSwapAny};
/// # use arc_swap::gen_lock::PrivateUnsharded;
/// // This one shares locks with others.
/// let shared = ArcSwap::from_pointee(42);
/// // But this one has an independent lock.
/// let independent = ArcSwapAny::<Arc<usize>, PrivateUnsharded>::from_pointee(42);
///
/// // This'll hold a lock so any writers there wouldn't complete
/// let l = independent.load_signal_safe();
/// // But the lock doesn't influence the shared one, so this goes through just fine
/// shared.store(Arc::new(43));
///
/// assert_eq!(42, **l);
/// ```
///
/// Note that there`s a type alias [`IndependentArcSwap`](../type.IndependentArcSwap.html) that can
/// be used instead.
#[derive(Default)]
pub struct PrivateUnsharded {
    gen_idx: AtomicUsize,
    shard: [Shard; 1],
}

unsafe impl LockStorage for PrivateUnsharded {
    type Shards = [Shard; 1];

    #[inline]
    fn gen_idx(&self) -> &AtomicUsize {
        &self.gen_idx
    }

    #[inline]
    fn shards(&self) -> &[Shard; 1] {
        &self.shard
    }

    #[inline]
    fn choose_shard(&self) -> usize {
        0
    }
}

/// An alternative to [`PrivateUnsharded`], but with configurable number of shards.
///
/// The [`PrivateUnsharded`] is almost identical to `PrivateSharded<[Shard; 1]>` (the
/// implementation takes advantage of some details to avoid a little bit of overhead). It allows
/// the user to choose the trade-of between contention during locking and size of the pointer and
/// speed during writes.
///
/// [`PrivateUnsharded`]: struct.PrivateUnsharded.html
///
/// # Note on `AsRef<[Shard]>`
///
/// Rust provides the `AsRef` trait (or, actually any trait) up to arrays of 32 elements. If you
/// need something bigger, you have to work around it with a newtype.
#[derive(Default)]
pub struct PrivateSharded<S> {
    gen_idx: AtomicUsize,
    shards: S,
}

/// Global counter of threads.
///
/// We specifically don't use ThreadId here, because it is opaque and doesn't give us a number :-(.
static PRIV_THREAD_ID_GEN: AtomicUsize = AtomicUsize::new(0);

thread_local! {
    /// A shard a thread has chosen.
    static PRIV_THREAD_ID: usize = PRIV_THREAD_ID_GEN.fetch_add(1, Ordering::Relaxed);
}

unsafe impl<S: AsRef<[Shard]> + Default> LockStorage for PrivateSharded<S> {
    type Shards = S;

    #[inline]
    fn gen_idx(&self) -> &AtomicUsize {
        &self.gen_idx
    }

    #[inline]
    fn shards(&self) -> &Self::Shards {
        &self.shards
    }

    #[inline]
    fn choose_shard(&self) -> usize {
        PRIV_THREAD_ID
            .try_with(|id| id % self.shards.as_ref().len())
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    extern crate crossbeam_utils;

    use std::sync::Arc;

    use self::crossbeam_utils::thread;

    use super::super::{ArcSwapAny, SignalSafety};
    use super::*;

    const ITERATIONS: usize = 100;

    // Does a kind of ping-pong between two threads, torturing the arc-swap somewhat.
    fn basic_check<S: LockStorage + Send + Sync>() {
        for _ in 0..ITERATIONS {
            let shared = ArcSwapAny::<_, S>::from(Arc::new(usize::max_value()));
            thread::scope(|scope| {
                for i in 0..2 {
                    let shared = &shared;
                    scope.spawn(move |_| {
                        for j in 0..50 {
                            if j % 2 == i {
                                while **shared.lock_internal(SignalSafety::Unsafe) != j {}
                            } else {
                                shared.store(Arc::new(j));
                            }
                        }
                    });
                }
            })
            .unwrap();
        }
    }

    #[test]
    fn basic_check_global() {
        basic_check::<Global>();
    }

    #[test]
    fn basic_check_private_unsharded() {
        basic_check::<PrivateUnsharded>();
    }

    #[test]
    fn basic_check_private_sharded_2() {
        basic_check::<PrivateSharded<[Shard; 2]>>();
    }

    #[test]
    fn basic_check_private_sharded_63() {
        basic_check::<PrivateSharded<[Shard; 31]>>();
    }
}
