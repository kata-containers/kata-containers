//! Direct rate limiters (those that can only hold one state).
//!
//! Rate limiters based on these types are constructed with
//! [the `RateLimiter` constructors](../struct.RateLimiter.html#direct-in-memory-rate-limiters---constructors)

use std::prelude::v1::*;

use std::num::NonZeroU32;

use crate::{
    clock,
    middleware::{NoOpMiddleware, RateLimitingMiddleware},
    state::InMemoryState,
    NegativeMultiDecision, Quota,
};

/// The "this state store does not use keys" key type.
///
/// It's possible to use this to create a "direct" rate limiter. It explicitly does not implement
/// [`Hash`][std::hash::Hash] so that it is possible to tell apart from "hashable" key types.
#[derive(PartialEq, Debug, Eq)]
pub enum NotKeyed {
    /// The value given to state stores' methods.
    NonKey,
}

/// A trait for state stores that only keep one rate limiting state.
///
/// This is blanket-implemented by all [`StateStore`]s with [`NotKeyed`] key associated types.
pub trait DirectStateStore: StateStore<Key = NotKeyed> {}

impl<T> DirectStateStore for T where T: StateStore<Key = NotKeyed> {}

/// # Direct in-memory rate limiters - Constructors
///
/// Here we construct an in-memory rate limiter that makes direct (un-keyed)
/// rate-limiting decisions. Direct rate limiters can be used to
/// e.g. regulate the transmission of packets on a single connection,
/// or to ensure that an API client stays within a service's rate
/// limit.
#[cfg(feature = "std")]
impl RateLimiter<NotKeyed, InMemoryState, clock::DefaultClock, NoOpMiddleware> {
    /// Constructs a new in-memory direct rate limiter for a quota with the default real-time clock.
    pub fn direct(
        quota: Quota,
    ) -> RateLimiter<NotKeyed, InMemoryState, clock::DefaultClock, NoOpMiddleware> {
        let clock = clock::DefaultClock::default();
        Self::direct_with_clock(quota, &clock)
    }
}

impl<C> RateLimiter<NotKeyed, InMemoryState, C, NoOpMiddleware<C::Instant>>
where
    C: clock::Clock,
{
    /// Constructs a new direct rate limiter for a quota with a custom clock.
    pub fn direct_with_clock(quota: Quota, clock: &C) -> Self {
        let state: InMemoryState = Default::default();
        RateLimiter::new(quota, state, clock)
    }
}

/// # Direct rate limiters - Manually checking cells
impl<S, C, MW> RateLimiter<NotKeyed, S, C, MW>
where
    S: DirectStateStore,
    C: clock::Clock,
    MW: RateLimitingMiddleware<C::Instant>,
{
    /// Allow a single cell through the rate limiter.
    ///
    /// If the rate limit is reached, `check` returns information about the earliest
    /// time that a cell might be allowed through again.
    pub fn check(&self) -> Result<MW::PositiveOutcome, MW::NegativeOutcome> {
        self.gcra.test_and_update::<NotKeyed, C::Instant, S, MW>(
            self.start,
            &NotKeyed::NonKey,
            &self.state,
            self.clock.now(),
        )
    }

    /// Allow *only all* `n` cells through the rate limiter.
    ///
    /// This method can succeed in only one way and fail in two ways:
    /// * Success: If all `n` cells can be accommodated, it returns `Ok(())`.
    /// * Failure (but ok): Not all cells can make it through at the current time.
    ///   The result is `Err(NegativeMultiDecision::BatchNonConforming(NotUntil))`, which can
    ///   be interrogated about when the batch might next conform.
    /// * Failure (the batch can never go through): The rate limit quota's burst size is too low
    ///   for the given number of cells to ever be allowed through.
    ///
    /// ### Performance
    /// This method diverges a little from the GCRA algorithm, using
    /// multiplication to determine the next theoretical arrival time, and so
    /// is not as fast as checking a single cell.
    pub fn check_n(
        &self,
        n: NonZeroU32,
    ) -> Result<MW::PositiveOutcome, NegativeMultiDecision<MW::NegativeOutcome>> {
        self.gcra
            .test_n_all_and_update::<NotKeyed, C::Instant, S, MW>(
                self.start,
                &NotKeyed::NonKey,
                n,
                &self.state,
                self.clock.now(),
            )
    }
}

#[cfg(feature = "std")]
mod future;
#[cfg(feature = "std")]
pub use future::*;

#[cfg(feature = "std")]
mod sinks;
#[cfg(feature = "std")]
pub use sinks::*;

#[cfg(feature = "std")]
mod streams;

use crate::state::{RateLimiter, StateStore};
#[cfg(feature = "std")]
pub use streams::*;

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn not_keyed_impls_coverage() {
        assert_eq!(NotKeyed::NonKey, NotKeyed::NonKey);
    }
}
