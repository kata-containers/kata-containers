use std::{error::Error, fmt, num::NonZeroU32};

use super::RateLimiter;
use crate::{
    clock,
    middleware::RateLimitingMiddleware,
    state::{DirectStateStore, NotKeyed},
    Jitter, NegativeMultiDecision, NotUntil,
};
use futures_timer::Delay;

/// An error that occurs when the number of cells required in `check_n`
/// exceeds the maximum capacity of the limiter.
#[derive(Debug, Clone)]
pub struct InsufficientCapacity(pub u32);

impl fmt::Display for InsufficientCapacity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "required number of cell {} exceeds bucket's capacity",
            self.0
        )
    }
}

impl Error for InsufficientCapacity {}

#[cfg(feature = "std")]
/// # Direct rate limiters - `async`/`await`
impl<S, C, MW> RateLimiter<NotKeyed, S, C, MW>
where
    S: DirectStateStore,
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
    pub async fn until_ready(&self) -> MW::PositiveOutcome {
        self.until_ready_with_jitter(Jitter::NONE).await
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
    pub async fn until_ready_with_jitter(&self, jitter: Jitter) -> MW::PositiveOutcome {
        loop {
            match self.check() {
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

    /// Asynchronously resolves as soon as the rate limiter allows it.
    ///
    /// This is similar to `until_ready` except it waits for an abitrary number
    /// of `n` cells to be available.
    ///
    /// Returns `InsufficientCapacity` if the `n` provided exceeds the maximum
    /// capacity of the rate limiter.
    pub async fn until_n_ready(
        &self,
        n: NonZeroU32,
    ) -> Result<MW::PositiveOutcome, InsufficientCapacity> {
        self.until_n_ready_with_jitter(n, Jitter::NONE).await
    }

    /// Asynchronously resolves as soon as the rate limiter allows it, with a
    /// randomized wait period.
    ///
    /// This is similar to `until_ready_with_jitter` except it waits for an
    /// abitrary number of `n` cells to be available.
    ///
    /// Returns `InsufficientCapacity` if the `n` provided exceeds the maximum
    /// capacity of the rate limiter.
    pub async fn until_n_ready_with_jitter(
        &self,
        n: NonZeroU32,
        jitter: Jitter,
    ) -> Result<MW::PositiveOutcome, InsufficientCapacity> {
        loop {
            match self.check_n(n) {
                Ok(x) => {
                    return Ok(x);
                }
                Err(NegativeMultiDecision::BatchNonConforming(_, negative)) => {
                    let delay = Delay::new(jitter + negative.wait_time_from(self.clock.now()));
                    delay.await;
                }
                Err(NegativeMultiDecision::InsufficientCapacity(cap)) => {
                    return Err(InsufficientCapacity(cap))
                }
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn insufficient_capacity_impl_coverage() {
        let i = InsufficientCapacity(1);
        assert_eq!(i.0, i.clone().0);
        assert!(format!("{}", i).len() > 0);
    }
}
