use std::prelude::v1::*;

use crate::clock::{Clock, ReasonablyRealtime, Reference};
use crate::nanos::Nanos;
use std::ops::Add;
use std::sync::Arc;
use std::time::Duration;

/// A clock using the default [`quanta::Clock`] structure.
///
/// This clock uses [`quanta::Clock.now`], which does retrieve the time synchronously. To use a
/// clock that uses a quanta background upkeep thread (which allows retrieving the time with an
/// atomic read, but requires a background thread that wakes up continually),
/// see [`QuantaUpkeepClock`].
#[derive(Debug, Clone, Default)]
pub struct QuantaClock(quanta::Clock);

impl From<quanta::Instant> for Nanos {
    fn from(instant: quanta::Instant) -> Self {
        instant.as_u64().into()
    }
}

impl Clock for QuantaClock {
    type Instant = QuantaInstant;

    fn now(&self) -> Self::Instant {
        QuantaInstant(Nanos::from(self.0.now()))
    }
}

/// A nanosecond-scale opaque instant (already scaled to reference time) returned from a
/// [`QuantaClock`].
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct QuantaInstant(Nanos);

impl Add<Nanos> for QuantaInstant {
    type Output = QuantaInstant;

    fn add(self, other: Nanos) -> QuantaInstant {
        QuantaInstant(self.0 + other)
    }
}

impl Reference for QuantaInstant {
    fn duration_since(&self, earlier: Self) -> Nanos {
        self.0.duration_since(earlier.0)
    }

    fn saturating_sub(&self, duration: Nanos) -> Self {
        QuantaInstant(self.0.saturating_sub(duration))
    }
}

/// A clock using the default [`quanta::Clock`] structure and an upkeep thread.
///
/// This clock relies on an upkeep thread that wakes up in regular (user defined) intervals to
/// retrieve the current time and update an atomic U64; the clock then can retrieve that time
/// (and is as behind as, at most, that interval).
///
/// The background thread is stopped as soon as the last clone of the clock is
/// dropped.
///
/// Whether this is faster than a [`QuantaClock`] depends on the utilization of the rate limiter
/// and the upkeep interval that you pick; you should measure and compare performance before
/// picking one or the other.
#[derive(Debug, Clone)]
pub struct QuantaUpkeepClock(quanta::Clock, Arc<quanta::Handle>);

impl QuantaUpkeepClock {
    /// Returns a new `QuantaUpkeepClock` with an upkeep thread that wakes up once in `interval`.
    pub fn from_interval(interval: Duration) -> Result<QuantaUpkeepClock, quanta::Error> {
        let builder = quanta::Upkeep::new(interval);
        Self::from_builder(builder)
    }

    /// Returns a new `QuantaUpkeepClock` with an upkeep thread as specified by the given builder.
    pub fn from_builder(builder: quanta::Upkeep) -> Result<QuantaUpkeepClock, quanta::Error> {
        let handle = builder.start()?;
        Ok(QuantaUpkeepClock(
            quanta::Clock::default(),
            Arc::new(handle),
        ))
    }
}

impl Clock for QuantaUpkeepClock {
    type Instant = QuantaInstant;

    fn now(&self) -> Self::Instant {
        QuantaInstant(Nanos::from(self.0.recent()))
    }
}

impl ReasonablyRealtime for QuantaClock {}

/// Some tests to ensure that the code above gets exercised. We don't
/// rely on them in tests (being nastily tainted by realism), so we
/// have to get creative.
#[cfg(test)]
mod test {
    use super::*;
    use crate::clock::{Clock, QuantaClock, QuantaUpkeepClock, Reference};
    use crate::nanos::Nanos;
    use std::time::Duration;

    #[test]
    fn quanta_impls_coverage() {
        let one_ns = Nanos::new(1);
        let c = QuantaClock::default();
        let now = c.now();
        assert_ne!(now + one_ns, now);
        assert_eq!(one_ns, Reference::duration_since(&(now + one_ns), now));
        assert_eq!(Nanos::new(0), Reference::duration_since(&now, now + one_ns));
        assert_eq!(
            Reference::saturating_sub(&(now + Duration::from_nanos(1).into()), one_ns),
            now
        );
    }

    #[test]
    fn quanta_upkeep_impls_coverage() {
        let one_ns = Nanos::new(1);
        // let _c1 =
        //     QuantaUpkeepClock::from_builder(quanta::Upkeep::new(Duration::from_secs(1))).unwrap();
        let c = QuantaUpkeepClock::from_interval(Duration::from_secs(1))
            .unwrap()
            .clone();
        let now = c.now();
        assert_ne!(now + one_ns, now);
        assert_eq!(one_ns, Reference::duration_since(&(now + one_ns), now));
        assert_eq!(Nanos::new(0), Reference::duration_since(&now, now + one_ns));
        assert_eq!(
            Reference::saturating_sub(&(now + Duration::from_nanos(1).into()), one_ns),
            now
        );
    }
}
