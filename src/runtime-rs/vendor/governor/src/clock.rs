//! Time sources for rate limiters.
//!
//! The time sources contained in this module allow the rate limiter
//! to be (optionally) independent of std, and additionally
//! allow mocking the passage of time.
//!
//! You can supply a custom time source by implementing both [`Reference`]
//! and [`Clock`] for your own types, and by implementing `Add<Nanos>` for
//! your [`Reference`] type:
//! ```rust
//! # use std::ops::Add;
//! use governor::clock::{Reference, Clock};
//! use governor::nanos::Nanos;
//!
//! #[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
//! struct MyInstant(u64);
//!
//! impl Add<Nanos> for MyInstant {
//!     type Output = Self;
//!
//!    fn add(self, other: Nanos) -> Self {
//!        Self(self.0 + other.as_u64())
//!    }
//! }
//!
//! impl Reference for MyInstant {
//!     fn duration_since(&self, earlier: Self) -> Nanos {
//!         self.0.checked_sub(earlier.0).unwrap_or(0).into()
//!     }
//!
//!     fn saturating_sub(&self, duration: Nanos) -> Self {
//!         Self(self.0.checked_sub(duration.into()).unwrap_or(self.0))
//!     }
//! }
//!
//! #[derive(Clone)]
//! struct MyCounter(u64);
//!
//! impl Clock for MyCounter {
//!     type Instant = MyInstant;
//!
//!     fn now(&self) -> Self::Instant {
//!         MyInstant(self.0)
//!     }
//! }
//! ```

use std::prelude::v1::*;

use std::convert::TryInto;
use std::fmt::Debug;
use std::ops::Add;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use crate::nanos::Nanos;

/// A measurement from a clock.
pub trait Reference:
    Sized + Add<Nanos, Output = Self> + PartialEq + Eq + Ord + Copy + Clone + Send + Sync + Debug
{
    /// Determines the time that separates two measurements of a
    /// clock. Implementations of this must perform a saturating
    /// subtraction - if the `earlier` timestamp should be later,
    /// `duration_since` must return the zero duration.
    fn duration_since(&self, earlier: Self) -> Nanos;

    /// Returns a reference point that lies at most `duration` in the
    /// past from the current reference. If an underflow should occur,
    /// returns the current reference.
    fn saturating_sub(&self, duration: Nanos) -> Self;
}

/// A time source used by rate limiters.
pub trait Clock: Clone {
    /// A measurement of a monotonically increasing clock.
    type Instant: Reference;

    /// Returns a measurement of the clock.
    fn now(&self) -> Self::Instant;
}

impl Reference for Duration {
    /// The internal duration between this point and another.
    /// ```rust
    /// # use std::time::Duration;
    /// # use governor::clock::Reference;
    /// let diff = Duration::from_secs(20).duration_since(Duration::from_secs(10));
    /// assert_eq!(diff, Duration::from_secs(10).into());
    /// ```
    fn duration_since(&self, earlier: Self) -> Nanos {
        self.checked_sub(earlier)
            .unwrap_or_else(|| Duration::new(0, 0))
            .into()
    }

    /// The internal duration between this point and another.
    /// ```rust
    /// # use std::time::Duration;
    /// # use governor::clock::Reference;
    /// let diff = Reference::saturating_sub(&Duration::from_secs(20), Duration::from_secs(10).into());
    /// assert_eq!(diff, Duration::from_secs(10));
    /// ```
    fn saturating_sub(&self, duration: Nanos) -> Self {
        self.checked_sub(duration.into()).unwrap_or(*self)
    }
}

impl Add<Nanos> for Duration {
    type Output = Self;

    fn add(self, other: Nanos) -> Self {
        let other: Duration = other.into();
        self + other
    }
}

/// A mock implementation of a clock. All it does is keep track of
/// what "now" is (relative to some point meaningful to the program),
/// and returns that.
///
/// # Thread safety
/// The mock time is represented as an atomic u64 count of nanoseconds, behind an [`Arc`].
/// Clones of this clock will all show the same time, even if the original advances.
#[derive(Debug, Clone, Default)]
pub struct FakeRelativeClock {
    now: Arc<AtomicU64>,
}

impl FakeRelativeClock {
    /// Advances the fake clock by the given amount.
    pub fn advance(&self, by: Duration) {
        let by: u64 = by
            .as_nanos()
            .try_into()
            .expect("Can not represent times past ~584 years");

        let mut prev = self.now.load(Ordering::Acquire);
        let mut next = prev + by;
        while let Err(next_prev) =
            self.now
                .compare_exchange_weak(prev, next, Ordering::Release, Ordering::Relaxed)
        {
            prev = next_prev;
            next = prev + by;
        }
    }
}

impl PartialEq for FakeRelativeClock {
    /// Compares two fake relative clocks' current state, snapshotted.
    ///
    /// ```rust
    /// # use std::time::Duration;
    /// # use governor::clock::FakeRelativeClock;
    /// let clock1 = FakeRelativeClock::default();
    /// let clock2 = FakeRelativeClock::default();
    /// assert_eq!(clock1, clock2);
    /// clock1.advance(Duration::from_secs(1));
    /// assert_ne!(clock1, clock2);
    /// ```
    fn eq(&self, other: &Self) -> bool {
        self.now.load(Ordering::Relaxed) == other.now.load(Ordering::Relaxed)
    }
}

impl Clock for FakeRelativeClock {
    type Instant = Nanos;

    fn now(&self) -> Self::Instant {
        self.now.load(Ordering::Relaxed).into()
    }
}

#[cfg(feature = "std")]
mod with_std;
#[cfg(feature = "std")]
pub use with_std::*;

#[cfg(all(feature = "std", feature = "quanta"))]
mod quanta;
#[cfg(all(feature = "std", feature = "quanta"))]
pub use self::quanta::*;

mod default;

pub use default::*;

#[cfg(all(feature = "std", test))]
mod test {
    use super::*;
    use crate::nanos::Nanos;
    use std::iter::repeat;
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn fake_clock_parallel_advances() {
        let clock = Arc::new(FakeRelativeClock::default());
        let threads = repeat(())
            .take(10)
            .map(move |_| {
                let clock = Arc::clone(&clock);
                thread::spawn(move || {
                    for _ in (0..1000000).into_iter() {
                        let now = clock.now();
                        clock.advance(Duration::from_nanos(1));
                        assert!(clock.now() > now);
                    }
                })
            })
            .collect::<Vec<_>>();
        for t in threads {
            t.join().unwrap();
        }
    }

    #[test]
    fn duration_addition_coverage() {
        let d = Duration::from_secs(1);
        let one_ns = Nanos::new(1);
        assert!(d + one_ns > d);
    }
}
