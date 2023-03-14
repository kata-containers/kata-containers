use std::prelude::v1::*;

use crate::nanos::Nanos;
#[cfg(feature = "jitter")]
use rand::distributions::uniform::{SampleBorrow, SampleUniform, UniformInt, UniformSampler};
#[cfg(feature = "jitter")]
use rand::distributions::{Distribution, Uniform};
#[cfg(feature = "jitter")]
use rand::{thread_rng, Rng};
use std::ops::Add;
use std::time::Duration;

#[cfg(feature = "std")]
use std::time::Instant;

/// An interval specification for deviating from the nominal wait time.
///
/// Jitter can be added to wait time `Duration`s to ensure that multiple tasks waiting on the same
/// rate limit don't wake up at the same time and attempt to measure at the same time.
///
/// Methods on rate limiters that work asynchronously like
/// [`DirectRateLimiter.until_ready_with_jitter`](struct.DirectRateLimiter.html#method.until_ready_with_jitter)
/// exist to automatically apply jitter to wait periods, thereby reducing the chance of a
/// thundering herd problem.
///
/// # Examples
///
/// Jitter can be added manually to a `Duration`:
///
/// ```rust
/// # #[cfg(feature = "jitter")]
/// # fn main() {
/// # use governor::Jitter;
/// # use std::time::Duration;
/// let reference = Duration::from_secs(24);
/// let jitter = Jitter::new(Duration::from_secs(1), Duration::from_secs(1));
/// let result = jitter + reference;
/// assert!(result >= reference + Duration::from_secs(1));
/// assert!(result < reference + Duration::from_secs(2))
/// # }
/// # #[cfg(not(feature = "jitter"))]
/// # fn main() {}
/// ```
///
/// In a `std` build (the default), Jitter can also be added to an `Instant`:
///
/// ```rust
/// # #[cfg(all(feature = "jitter", feature = "std"))]
/// # fn main() {
/// # use governor::Jitter;
/// # use std::time::{Duration, Instant};
/// let reference = Instant::now();
/// let jitter = Jitter::new(Duration::from_secs(1), Duration::from_secs(1));
/// let result = jitter + reference;
/// assert!(result >= reference + Duration::from_secs(1));
/// assert!(result < reference + Duration::from_secs(2))
/// # }
/// # #[cfg(any(not(feature = "jitter"), not(feature = "std")))] fn main() {}
/// ```
#[derive(Debug, PartialEq, Default, Clone, Copy)]
#[cfg_attr(feature = "docs", doc(cfg(jitter)))]
pub struct Jitter {
    min: Nanos,
    max: Nanos,
}

impl Jitter {
    #[cfg(feature = "std")]
    /// The "empty" jitter interval - no jitter at all.
    pub(crate) const NONE: Jitter = Jitter {
        min: Nanos::new(0),
        max: Nanos::new(0),
    };

    /// Constructs a new Jitter interval, waiting at most a duration of `max`.
    ///
    /// ```rust
    /// # use std::time::Duration;
    /// # use governor::Jitter;
    /// let jitter = Jitter::up_to(Duration::from_secs(20));
    /// let now = Duration::from_secs(0);
    /// assert!(jitter + now <= Duration::from_secs(20)); // always.
    /// ```
    #[cfg(feature = "jitter")]
    pub fn up_to(max: Duration) -> Jitter {
        Jitter {
            min: Nanos::from(0),
            max: max.into(),
        }
    }

    /// Constructs a new Jitter interval, waiting at least `min` and at most `min+interval`.
    #[cfg(feature = "jitter")]
    pub fn new(min: Duration, interval: Duration) -> Jitter {
        let min: Nanos = min.into();
        let max: Nanos = min + Nanos::from(interval);
        Jitter { min, max }
    }

    /// Returns a random amount of jitter within the configured interval.
    #[cfg(feature = "jitter")]
    pub(crate) fn get(&self) -> Nanos {
        if self.min == self.max {
            return self.min;
        }
        let uniform = Uniform::new(self.min, self.max);
        uniform.sample(&mut thread_rng())
    }

    /// Returns a random amount of jitter within the configured interval.
    #[cfg(not(feature = "jitter"))]
    pub(crate) fn get(&self) -> Nanos {
        self.min
    }
}

/// A random distribution of nanoseconds
#[cfg(feature = "jitter")]
#[derive(Clone, Copy, Debug)]
pub struct UniformJitter(UniformInt<u64>);

#[cfg(feature = "jitter")]
impl UniformSampler for UniformJitter {
    type X = Nanos;

    fn new<B1, B2>(low: B1, high: B2) -> Self
    where
        B1: SampleBorrow<Self::X> + Sized,
        B2: SampleBorrow<Self::X> + Sized,
    {
        UniformJitter(UniformInt::new(
            low.borrow().as_u64(),
            high.borrow().as_u64(),
        ))
    }

    fn new_inclusive<B1, B2>(low: B1, high: B2) -> Self
    where
        B1: SampleBorrow<Self::X> + Sized,
        B2: SampleBorrow<Self::X> + Sized,
    {
        UniformJitter(UniformInt::new_inclusive(
            low.borrow().as_u64(),
            high.borrow().as_u64(),
        ))
    }

    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Self::X {
        Nanos::from(self.0.sample(rng))
    }
}

#[cfg(feature = "jitter")]
impl SampleUniform for Nanos {
    type Sampler = UniformJitter;
}

impl Add<Duration> for Jitter {
    type Output = Duration;

    fn add(self, rhs: Duration) -> Duration {
        let amount: Duration = self.get().into();
        rhs + amount
    }
}

impl Add<Nanos> for Jitter {
    type Output = Nanos;

    fn add(self, rhs: Nanos) -> Nanos {
        rhs + self.get()
    }
}

#[cfg(feature = "std")]
impl Add<Instant> for Jitter {
    type Output = Instant;

    fn add(self, rhs: Instant) -> Instant {
        let amount: Duration = self.get().into();
        rhs + amount
    }
}

#[cfg(all(feature = "jitter", test))]
mod test {
    use super::*;

    #[test]
    fn jitter_impl_coverage() {
        let basic = Jitter::up_to(Duration::from_secs(20));
        let verbose = Jitter::new(Duration::from_secs(0), Duration::from_secs(20));
        assert_eq!(basic, verbose);
    }

    #[test]
    fn uniform_sampler_coverage() {
        let low = Duration::from_secs(0);
        let high = Duration::from_secs(20);
        let sampler = UniformJitter::new_inclusive(Nanos::from(low), Nanos::from(high));
        assert!(format!("{:?}", sampler).len() > 0);
        assert!(format!("{:?}", sampler.clone()).len() > 0);
    }
}
