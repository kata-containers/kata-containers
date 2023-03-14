use super::{Clock, Reference};

use std::prelude::v1::*;

use crate::nanos::Nanos;
use std::ops::Add;
use std::time::{Duration, Instant, SystemTime};

/// The monotonic clock implemented by [`Instant`].
#[derive(Clone, Debug, Default)]
pub struct MonotonicClock;

impl Add<Nanos> for Instant {
    type Output = Instant;

    fn add(self, other: Nanos) -> Instant {
        let other: Duration = other.into();
        self + other
    }
}

impl Reference for Instant {
    fn duration_since(&self, earlier: Self) -> Nanos {
        if earlier < *self {
            (*self - earlier).into()
        } else {
            Nanos::from(Duration::new(0, 0))
        }
    }

    fn saturating_sub(&self, duration: Nanos) -> Self {
        self.checked_sub(duration.into()).unwrap_or(*self)
    }
}

impl Clock for MonotonicClock {
    type Instant = Instant;

    fn now(&self) -> Self::Instant {
        Instant::now()
    }
}

/// The non-monotonic clock implemented by [`SystemTime`].
#[derive(Clone, Debug, Default)]
pub struct SystemClock;

impl Reference for SystemTime {
    /// Returns the difference in times between the two
    /// SystemTimes. Due to the fallible nature of SystemTimes,
    /// returns the zero duration if a negative duration would
    /// result (e.g. due to system clock adjustments).
    fn duration_since(&self, earlier: Self) -> Nanos {
        self.duration_since(earlier)
            .unwrap_or_else(|_| Duration::new(0, 0))
            .into()
    }

    fn saturating_sub(&self, duration: Nanos) -> Self {
        self.checked_sub(duration.into()).unwrap_or(*self)
    }
}

impl Add<Nanos> for SystemTime {
    type Output = SystemTime;

    fn add(self, other: Nanos) -> SystemTime {
        let other: Duration = other.into();
        self + other
    }
}

impl Clock for SystemClock {
    type Instant = SystemTime;

    fn now(&self) -> Self::Instant {
        SystemTime::now()
    }
}

/// Identifies clocks that run similarly to the monotonic realtime clock.
///
/// Clocks implementing this trait can be used with rate-limiters functions that operate
/// asynchronously.
pub trait ReasonablyRealtime: Clock {
    /// Returns a reference point at the start of an operation.
    fn reference_point(&self) -> Self::Instant {
        self.now()
    }
}

impl ReasonablyRealtime for MonotonicClock {}

impl ReasonablyRealtime for SystemClock {}

/// Some tests to ensure that the code above gets exercised. We don't
/// rely on them in tests (being nastily tainted by realism), so we
/// have to get creative.
#[cfg(test)]
mod test {
    use super::*;
    use crate::clock::{Clock, MonotonicClock, Reference, SystemClock};
    use crate::nanos::Nanos;
    use std::time::Duration;

    #[test]
    fn instant_impls_coverage() {
        let one_ns = Nanos::new(1);
        let c = MonotonicClock::default();
        let now = c.now();
        assert_ne!(now + one_ns, now);
        assert_eq!(one_ns, Reference::duration_since(&(now + one_ns), now));
        assert_eq!(Nanos::new(0), Reference::duration_since(&now, now + one_ns));
        assert_eq!(
            Reference::saturating_sub(&(now + Duration::from_nanos(1)), one_ns),
            now
        );
    }

    #[test]
    fn system_clock_impls_coverage() {
        let one_ns = Nanos::new(1);
        let c = SystemClock::default();
        let now = c.now();
        assert_ne!(now + one_ns, now);
        // Thankfully, we're not comparing two system clock readings
        // here so that ought to be safe, I think:
        assert_eq!(one_ns, Reference::duration_since(&(now + one_ns), now));
        assert_eq!(
            Reference::saturating_sub(&(now + Duration::from_nanos(1)), one_ns),
            now
        );
    }
}
