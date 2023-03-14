//! Keyed rate limiters (those that can hold one state per key).
//!
//! These are rate limiters that have one set of parameters (burst capacity per time period) but
//! apply those to several sets of actual rate-limiting states, e.g. to enforce one API call rate
//! limit per API key.
//!
//! Rate limiters based on these types are constructed with
//! [the `RateLimiter` constructors](../struct.RateLimiter.html#keyed-rate-limiters---default-constructors)

use std::hash::Hash;
use std::num::NonZeroU32;
use std::prelude::v1::*;

use crate::state::StateStore;
use crate::{
    clock::{self, Reference},
    middleware::RateLimitingMiddleware,
    nanos::Nanos,
    NegativeMultiDecision, Quota, RateLimiter,
};

/// A trait for state stores with one rate limiting state per key.
///
/// This is blanket-implemented by all [`StateStore`]s with hashable (`Eq + Hash + Clone`) key
/// associated types.
pub trait KeyedStateStore<K: Hash>: StateStore<Key = K> {}

impl<T, K: Hash> KeyedStateStore<K> for T
where
    T: StateStore<Key = K>,
    K: Eq + Clone + Hash,
{
}

/// # Keyed rate limiters - default constructors
impl<K> RateLimiter<K, DefaultKeyedStateStore<K>, clock::DefaultClock>
where
    K: Clone + Hash + Eq,
{
    /// Constructs a new keyed rate limiter backed by
    /// the [`DefaultKeyedStateStore`].
    pub fn keyed(quota: Quota) -> Self {
        let state = DefaultKeyedStateStore::default();
        let clock = clock::DefaultClock::default();
        RateLimiter::new(quota, state, &clock)
    }

    #[cfg(all(feature = "std", feature = "dashmap"))]
    /// Constructs a new keyed rate limiter explicitly backed by a [`DashMap`][::dashmap::DashMap].
    pub fn dashmap(quota: Quota) -> Self {
        let state = DashMapStateStore::default();
        let clock = clock::DefaultClock::default();
        RateLimiter::new(quota, state, &clock)
    }

    #[cfg(any(all(feature = "std", not(feature = "dashmap")), not(feature = "std")))]
    /// Constructs a new keyed rate limiter explicitly backed by a
    /// [`HashMap`][std::collections::HashMap].
    pub fn hashmap(quota: Quota) -> Self {
        let state = HashMapStateStore::default();
        let clock = clock::DefaultClock::default();
        RateLimiter::new(quota, state, &clock)
    }
}

#[cfg(all(feature = "std", feature = "dashmap"))]
impl<K> RateLimiter<K, HashMapStateStore<K>, clock::DefaultClock>
where
    K: Clone + Hash + Eq,
{
    /// Constructs a new keyed rate limiter explicitly backed by a
    /// [`HashMap`][std::collections::HashMap].
    pub fn hashmap(quota: Quota) -> Self {
        let state = HashMapStateStore::default();
        let clock = clock::DefaultClock::default();
        RateLimiter::new(quota, state, &clock)
    }
}

/// # Keyed rate limiters - Manually checking cells
impl<K, S, C, MW> RateLimiter<K, S, C, MW>
where
    S: KeyedStateStore<K>,
    K: Hash,
    C: clock::Clock,
    MW: RateLimitingMiddleware<C::Instant>,
{
    /// Allow a single cell through the rate limiter for the given key.
    ///
    /// If the rate limit is reached, `check_key` returns information about the earliest
    /// time that a cell might be allowed through again under that key.
    pub fn check_key(&self, key: &K) -> Result<MW::PositiveOutcome, MW::NegativeOutcome> {
        self.gcra.test_and_update::<K, C::Instant, S, MW>(
            self.start,
            key,
            &self.state,
            self.clock.now(),
        )
    }

    /// Allow *only all* `n` cells through the rate limiter for the given key.
    ///
    /// This method can succeed in only one way and fail in two ways:
    /// * Success: If all `n` cells can be accommodated, it returns `Ok(())`.
    /// * Failure (but ok): Not all cells can make it through at the current time.
    ///   The result is `Err(NegativeMultiDecision::BatchNonConforming(NotUntil))`, which can
    ///   be interrogated about when the batch might next conform.
    /// * Failure (the batch can never go through): The rate limit is too low for the given number
    ///   of cells.
    ///
    /// ### Performance
    /// This method diverges a little from the GCRA algorithm, using
    /// multiplication to determine the next theoretical arrival time, and so
    /// is not as fast as checking a single cell.
    pub fn check_key_n(
        &self,
        key: &K,
        n: NonZeroU32,
    ) -> Result<MW::PositiveOutcome, NegativeMultiDecision<MW::NegativeOutcome>> {
        self.gcra.test_n_all_and_update::<K, C::Instant, S, MW>(
            self.start,
            key,
            n,
            &self.state,
            self.clock.now(),
        )
    }
}

/// Keyed rate limiters that can be "cleaned up".
///
/// Any keyed state store implementing this trait allows users to evict elements that are
/// indistinguishable from fresh rate-limiting states (that is, if a key hasn't been used for
/// rate-limiting decisions for as long as the bucket capacity).
///
/// As this does not make sense for not all keyed state stores (e.g. stores that auto-expire like
/// memcache), this is an optional trait. All the keyed state stores in this crate implement
/// shrinking.
pub trait ShrinkableKeyedStateStore<K: Hash>: KeyedStateStore<K> {
    /// Remove those keys with state older than `drop_below`.
    fn retain_recent(&self, drop_below: Nanos);

    /// Shrinks the capacity of the state store, if possible.
    ///
    /// If the state store does not support shrinking, this method is a no-op.
    fn shrink_to_fit(&self) {}

    /// Returns the number of "live" keys stored in the state store.
    ///
    /// Depending on how the state store is implemented, this may
    /// return an estimate or an out-of-date result.
    fn len(&self) -> usize;

    /// Returns `true` if `self` has no keys stored in it.
    ///
    /// As with [`len`](#tymethod.len), this method may return
    /// imprecise results (indicating that the state store is empty
    /// while a concurrent rate-limiting operation is taking place).
    fn is_empty(&self) -> bool;
}

/// # Keyed rate limiters - Housekeeping
///
/// As the inputs to a keyed rate-limiter can be arbitrary keys, the set of retained keys retained
/// grows, while the number of active keys may stay smaller. To save on space, a keyed rate-limiter
/// allows removing those keys that are "stale", i.e., whose values are no different from keys' that
/// aren't present in the rate limiter state store.
impl<K, S, C, MW> RateLimiter<K, S, C, MW>
where
    S: ShrinkableKeyedStateStore<K>,
    K: Hash,
    C: clock::Clock,
    MW: RateLimitingMiddleware<C::Instant>,
{
    /// Retains all keys in the rate limiter that were used recently enough.
    ///
    /// Any key whose rate limiting state is indistinguishable from a "fresh" state (i.e., the
    /// theoretical arrival time lies in the past).
    pub fn retain_recent(&self) {
        // calculate the minimum retention parameter: Any key whose state store's theoretical
        // arrival time is larger than a starting state for the bucket gets to stay, everything
        // else (that's indistinguishable from a starting state) goes.
        let now = self.clock.now();
        let drop_below = now.duration_since(self.start);

        self.state.retain_recent(drop_below);
    }

    /// Shrinks the capacity of the rate limiter's state store, if possible.
    pub fn shrink_to_fit(&self) {
        self.state.shrink_to_fit();
    }

    /// Returns the number of "live" keys in the rate limiter's state store.
    ///
    /// Depending on how the state store is implemented, this may
    /// return an estimate or an out-of-date result.
    pub fn len(&self) -> usize {
        self.state.len()
    }

    /// Returns `true` if the rate limiter has no keys in it.
    ///
    /// As with [`len`](#method.len), this method may return
    /// imprecise results (indicating that the state store is empty
    /// while a concurrent rate-limiting operation is taking place).
    pub fn is_empty(&self) -> bool {
        self.state.is_empty()
    }
}

mod hashmap;

pub use hashmap::HashMapStateStore;

#[cfg(all(feature = "std", feature = "dashmap"))]
mod dashmap;

#[cfg(all(feature = "std", feature = "dashmap"))]
pub use self::dashmap::DashMapStateStore;

#[cfg(feature = "std")]
mod future;

#[cfg(any(all(feature = "std", not(feature = "dashmap")), not(feature = "std")))]
/// The default keyed rate limiter type: a mutex-wrapped [`HashMap`][std::collections::HashMap].
pub type DefaultKeyedStateStore<K> = HashMapStateStore<K>;

#[cfg(all(feature = "std", feature = "dashmap"))]
/// The default keyed rate limiter type: the concurrent [`DashMap`][::dashmap::DashMap].
pub type DefaultKeyedStateStore<K> = DashMapStateStore<K>;
