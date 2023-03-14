#[cfg(all(feature = "std", not(feature = "quanta")))]
/// The default clock that reports [`Instant`][std::time::Instant]s.
pub type DefaultClock = crate::clock::MonotonicClock;

#[cfg(all(feature = "std", feature = "quanta"))]
/// The default clock using [`quanta`] for extremely fast timekeeping (at a 100ns resolution).
pub type DefaultClock = crate::clock::QuantaClock;

#[cfg(not(feature = "std"))]
/// The default `no_std` clock that reports [`Durations`][core::time::Duration] must be advanced by the
/// program.
pub type DefaultClock = crate::clock::FakeRelativeClock;
