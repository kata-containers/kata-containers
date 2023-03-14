use std::prelude::v1::*;

use crate::{
    clock, middleware::RateLimitingMiddleware, state::keyed::KeyedStateStore, Jitter, NotUntil,
    RateLimiter,
};
use futures_timer::Delay;
use std::hash::Hash;

#[cfg(feature = "std")]
/// # Keyed rate limiters - `async`/`await`
impl<K, S, C, MW> RateLimiter<K, S, C, MW>
where
    K: Hash + Eq + Clone,
    S: KeyedStateStore<K>,
    C: clock::ReasonablyRealtime,
    MW: RateLimitingMiddleware<C::Instant, NegativeOutcome = NotUntil<C::Instant>>,
{
    /// Asynchronously resolves as soon as the rate limiter allows it.
    ///
    /// When polled, the returned future either resolves immediately (in the case where the rate
    /// limiter allows it), or else triggers an asynchronous delay, after which the rate limiter
    /// is polled again. This means that the future might resolve at some later time (depending
    /// on what other measurements are made on the rate limiter).
    ///
    /// If multiple futures are dispatched against the rate limiter, it is advisable to use
    /// [`until_ready_with_jitter`](#method.until_ready_with_jitter), to avoid thundering herds.
    pub async fn until_key_ready(&self, key: &K) -> MW::PositiveOutcome {
        self.until_key_ready_with_jitter(key, Jitter::NONE).await
    }

    /// Asynchronously resolves as soon as the rate limiter allows it, with a randomized wait
    /// period.
    ///
    /// When polled, the returned future either resolves immediately (in the case where the rate
    /// limiter allows it), or else triggers an asynchronous delay, after which the rate limiter
    /// is polled again. This means that the future might resolve at some later time (depending
    /// on what other measurements are made on the rate limiter).
    ///
    /// This method allows for a randomized additional delay between polls of the rate limiter,
    /// which can help reduce the likelihood of thundering herd effects if multiple tasks try to
    /// wait on the same rate limiter.
    pub async fn until_key_ready_with_jitter(
        &self,
        key: &K,
        jitter: Jitter,
    ) -> MW::PositiveOutcome {
        loop {
            match self.check_key(key) {
                Ok(x) => {
                    return x;
                }
                Err(negative) => {
                    let delay = Delay::new(jitter + negative.wait_time_from(self.clock.now()));
                    delay.await;
                }
            }
        }
    }
}
