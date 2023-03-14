use crate::state::StateStore;
use crate::{clock, middleware::StateSnapshot, NegativeMultiDecision, Quota};
use crate::{middleware::RateLimitingMiddleware, nanos::Nanos};
use std::num::NonZeroU32;
use std::time::Duration;
use std::{cmp, fmt};

#[cfg(feature = "std")]
use crate::Jitter;

/// A negative rate-limiting outcome.
///
/// `NotUntil`'s methods indicate when a caller can expect the next positive
/// rate-limiting result.
#[derive(Debug, PartialEq)]
pub struct NotUntil<P: clock::Reference> {
    state: StateSnapshot,
    start: P,
}

impl<'a, P: clock::Reference> NotUntil<P> {
    /// Create a `NotUntil` as a negative rate-limiting result.
    #[inline]
    pub(crate) fn new(state: StateSnapshot, start: P) -> Self {
        Self { state, start }
    }

    /// Returns the earliest time at which a decision could be
    /// conforming (excluding conforming decisions made by the Decider
    /// that are made in the meantime).
    #[inline]
    pub fn earliest_possible(&self) -> P {
        let tat: Nanos = self.state.tat;
        self.start + tat
    }

    /// Returns the minimum amount of time from the time that the
    /// decision was made that must pass before a
    /// decision can be conforming.
    ///
    /// If the time of the next expected positive result is in the past,
    /// `wait_time_from` returns a zero `Duration`.
    #[inline]
    pub fn wait_time_from(&self, from: P) -> Duration {
        let earliest = self.earliest_possible();
        earliest.duration_since(earliest.min(from)).into()
    }

    /// Returns the rate limiting [`Quota`] used to reach the decision.
    #[inline]
    pub fn quota(&self) -> Quota {
        self.state.quota()
    }

    #[cfg(feature = "std")] // not used unless we use Instant-compatible clocks.
    #[inline]
    pub(crate) fn earliest_possible_with_offset(&self, jitter: Jitter) -> P {
        let tat = jitter + self.state.tat;
        self.start + tat
    }

    #[cfg(feature = "std")] // not used unless we use Instant-compatible clocks.
    #[inline]
    pub(crate) fn wait_time_with_offset(&self, from: P, jitter: Jitter) -> Duration {
        let earliest = self.earliest_possible_with_offset(jitter);
        earliest.duration_since(earliest.min(from)).into()
    }
}

impl<'a, P: clock::Reference> fmt::Display for NotUntil<P> {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(f, "rate-limited until {:?}", self.start + self.state.tat)
    }
}

#[derive(Debug, PartialEq)]
pub(crate) struct Gcra {
    /// The "weight" of a single packet in units of time.
    t: Nanos,

    /// The "burst capacity" of the bucket.
    tau: Nanos,
}

impl Gcra {
    pub(crate) fn new(quota: Quota) -> Self {
        let tau: Nanos = (quota.replenish_1_per * quota.max_burst.get()).into();
        let t: Nanos = quota.replenish_1_per.into();
        Gcra { t, tau }
    }

    /// Computes and returns a new ratelimiter state if none exists yet.
    fn starting_state(&self, t0: Nanos) -> Nanos {
        t0 + self.t
    }

    /// Tests a single cell against the rate limiter state and updates it at the given key.
    pub(crate) fn test_and_update<
        K,
        P: clock::Reference,
        S: StateStore<Key = K>,
        MW: RateLimitingMiddleware<P>,
    >(
        &self,
        start: P,
        key: &K,
        state: &S,
        t0: P,
    ) -> Result<MW::PositiveOutcome, MW::NegativeOutcome> {
        let t0 = t0.duration_since(start);
        let tau = self.tau;
        let t = self.t;
        state.measure_and_replace(key, |tat| {
            let tat = tat.unwrap_or_else(|| self.starting_state(t0));
            let earliest_time = tat.saturating_sub(tau);
            if t0 < earliest_time {
                Err(MW::disallow(key, (self, earliest_time), start))
            } else {
                let next = cmp::max(tat, t0) + t;
                Ok((
                    MW::allow(key, StateSnapshot::new(self.t, self.tau, next)),
                    next,
                ))
            }
        })
    }

    /// Tests whether all `n` cells could be accommodated and updates the rate limiter state, if so.
    pub(crate) fn test_n_all_and_update<
        K,
        P: clock::Reference,
        S: StateStore<Key = K>,
        MW: RateLimitingMiddleware<P>,
    >(
        &self,
        start: P,
        key: &K,
        n: NonZeroU32,
        state: &S,
        t0: P,
    ) -> Result<MW::PositiveOutcome, NegativeMultiDecision<MW::NegativeOutcome>> {
        let t0 = t0.duration_since(start);
        let tau = self.tau;
        let t = self.t;
        let additional_weight = t * (n.get() - 1) as u64;

        // check that we can allow enough cells through. Note that `additional_weight` is the
        // value of the cells *in addition* to the first cell - so add that first cell back.
        if additional_weight + t > tau {
            return Err(NegativeMultiDecision::InsufficientCapacity(
                (tau.as_u64() / t.as_u64()) as u32,
            ));
        }
        state.measure_and_replace(key, |tat| {
            let tat = tat.unwrap_or_else(|| self.starting_state(t0));
            let earliest_time = (tat + additional_weight).saturating_sub(tau);
            if t0 < earliest_time {
                Err(NegativeMultiDecision::BatchNonConforming(
                    n.get(),
                    MW::disallow(key, (self, earliest_time), start),
                ))
            } else {
                let next = cmp::max(tat, t0) + t + additional_weight;
                Ok((MW::allow(key, (self, next)), next))
            }
        })
    }
}

impl From<(&Gcra, Nanos)> for StateSnapshot {
    #[inline]
    fn from(pair: (&Gcra, Nanos)) -> Self {
        StateSnapshot::new(pair.0.t, pair.0.tau, pair.1)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::Quota;
    use std::num::NonZeroU32;

    use proptest::prelude::*;

    /// Exercise derives and convenience impls on Gcra to make coverage happy
    #[cfg(feature = "std")]
    #[test]
    fn gcra_derives() {
        use nonzero_ext::nonzero;

        let g = Gcra::new(Quota::per_second(nonzero!(1u32)));
        let g2 = Gcra::new(Quota::per_second(nonzero!(2u32)));
        assert_eq!(g, g);
        assert_ne!(g, g2);
        assert!(format!("{:?}", g).len() > 0);
    }

    /// Exercise derives and convenience impls on NotUntil to make coverage happy
    #[cfg(feature = "std")]
    #[test]
    fn notuntil_impls() {
        use crate::RateLimiter;
        use clock::FakeRelativeClock;
        use nonzero_ext::nonzero;

        let clock = FakeRelativeClock::default();
        let quota = Quota::per_second(nonzero!(1u32));
        let lb = RateLimiter::direct_with_clock(quota, &clock);
        assert!(lb.check().is_ok());
        assert!(lb
            .check()
            .map_err(|nu| {
                assert_eq!(nu, nu);
                assert!(format!("{:?}", nu).len() > 0);
                assert_eq!(format!("{}", nu), "rate-limited until Nanos(1s)");
                assert_eq!(nu.quota(), quota);
            })
            .is_err());
    }

    #[derive(Debug)]
    struct Count(NonZeroU32);
    impl Arbitrary for Count {
        type Parameters = ();
        fn arbitrary_with(_args: ()) -> Self::Strategy {
            (1..10000u32)
                .prop_map(|x| Count(NonZeroU32::new(x).unwrap()))
                .boxed()
        }

        type Strategy = BoxedStrategy<Count>;
    }

    #[cfg(feature = "std")]
    #[test]
    fn cover_count_derives() {
        assert_eq!(
            format!("{:?}", Count(nonzero_ext::nonzero!(1_u32))),
            "Count(1)"
        );
    }

    #[test]
    fn roundtrips_quota() {
        proptest!(ProptestConfig::default(), |(per_second: Count, burst: Count)| {
            let quota = Quota::per_second(per_second.0).allow_burst(burst.0);
            let gcra = Gcra::new(quota);
            let back = Quota::from_gcra_parameters(gcra.t, gcra.tau);
            assert_eq!(quota, back);
        })
    }
}
