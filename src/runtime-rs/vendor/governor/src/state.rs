//! State stores for rate limiters

use std::{marker::PhantomData, prelude::v1::*};

pub mod direct;
mod in_memory;
pub mod keyed;

pub use self::in_memory::InMemoryState;

use crate::nanos::Nanos;
use crate::{clock, Quota};
use crate::{
    gcra::Gcra,
    middleware::{NoOpMiddleware, RateLimitingMiddleware},
};

pub use direct::*;

/// A way for rate limiters to keep state.
///
/// There are two important kinds of state stores: Direct and keyed. The direct kind have only
/// one state, and are useful for "global" rate limit enforcement (e.g. a process should never
/// do more than N tasks a day). The keyed kind allows one rate limit per key (e.g. an API
/// call budget per client API key).
///
/// A direct state store is expressed as [`StateStore::Key`] = [`NotKeyed`][direct::NotKeyed].
/// Keyed state stores have a
/// type parameter for the key and set their key to that.
pub trait StateStore {
    /// The type of key that the state store can represent.
    type Key;

    /// Updates a state store's rate limiting state for a given key, using the given closure.
    ///
    /// The closure parameter takes the old value (`None` if this is the first measurement) of the
    /// state store at the key's location, checks if the request an be accommodated and:
    ///
    /// * If the request is rate-limited, returns `Err(E)`.
    /// * If the request can make it through, returns `Ok(T)` (an arbitrary positive return
    ///   value) and the updated state.
    ///
    /// It is `measure_and_replace`'s job then to safely replace the value at the key - it must
    /// only update the value if the value hasn't changed. The implementations in this
    /// crate use `AtomicU64` operations for this.
    fn measure_and_replace<T, F, E>(&self, key: &Self::Key, f: F) -> Result<T, E>
    where
        F: Fn(Option<Nanos>) -> Result<(T, Nanos), E>;
}

/// A rate limiter.
///
/// This is the structure that ties together the parameters (how many cells to allow in what time
/// period) and the concrete state of rate limiting decisions. This crate ships in-memory state
/// stores, but it's possible (by implementing the [`StateStore`] trait) to make others.
#[derive(Debug)]
pub struct RateLimiter<K, S, C, MW = NoOpMiddleware>
where
    S: StateStore<Key = K>,
    C: clock::Clock,
    MW: RateLimitingMiddleware<C::Instant>,
{
    state: S,
    gcra: Gcra,
    clock: C,
    start: C::Instant,
    middleware: PhantomData<MW>,
}

impl<K, S, C, MW> RateLimiter<K, S, C, MW>
where
    S: StateStore<Key = K>,
    C: clock::Clock,
    MW: RateLimitingMiddleware<C::Instant>,
{
    /// Creates a new rate limiter from components.
    ///
    /// This is the most generic way to construct a rate-limiter; most users should prefer
    /// [`direct`] or other methods instead.
    pub fn new(quota: Quota, state: S, clock: &C) -> Self {
        let gcra = Gcra::new(quota);
        let start = clock.now();
        let clock = clock.clone();
        RateLimiter {
            state,
            clock,
            gcra,
            start,
            middleware: PhantomData,
        }
    }

    /// Consumes the `RateLimiter` and returns the state store.
    ///
    /// This is mostly useful for debugging and testing.
    pub fn into_state_store(self) -> S {
        self.state
    }
}

impl<K, S, C, MW> RateLimiter<K, S, C, MW>
where
    S: StateStore<Key = K>,
    C: clock::Clock,
    MW: RateLimitingMiddleware<C::Instant>,
{
    /// Convert the given rate limiter into one that uses a different middleware.
    pub fn with_middleware<Outer: RateLimitingMiddleware<C::Instant>>(
        self,
    ) -> RateLimiter<K, S, C, Outer> {
        RateLimiter {
            middleware: PhantomData,
            state: self.state,
            gcra: self.gcra,
            clock: self.clock,
            start: self.start,
        }
    }
}

#[cfg(feature = "std")]
impl<K, S, C, MW> RateLimiter<K, S, C, MW>
where
    S: StateStore<Key = K>,
    C: clock::ReasonablyRealtime,
    MW: RateLimitingMiddleware<C::Instant>,
{
    pub(crate) fn reference_reading(&self) -> C::Instant {
        self.clock.reference_point()
    }
}

#[cfg(all(feature = "std", test))]
mod test {
    use super::*;
    use crate::Quota;
    use nonzero_ext::nonzero;

    #[test]
    fn ratelimiter_impl_coverage() {
        let lim = RateLimiter::direct(Quota::per_second(nonzero!(3u32)));
        assert!(format!("{:?}", lim).len() > 0);
    }
}
