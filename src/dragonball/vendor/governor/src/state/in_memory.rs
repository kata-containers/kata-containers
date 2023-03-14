use std::prelude::v1::*;

use crate::nanos::Nanos;
use crate::state::{NotKeyed, StateStore};
use std::fmt;
use std::fmt::Debug;
use std::num::NonZeroU64;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::time::Duration;

/// An in-memory representation of a GCRA's rate-limiting state.
///
/// Implemented using [`AtomicU64`] operations, this state representation can be used to
/// construct rate limiting states for other in-memory states: e.g., this crate uses
/// `InMemoryState` as the states it tracks in the keyed rate limiters it implements.
///
/// Internally, the number tracked here is the theoretical arrival time (a GCRA term) in number of
/// nanoseconds since the rate limiter was created.
#[derive(Default)]
pub struct InMemoryState(AtomicU64);

impl InMemoryState {
    pub(crate) fn measure_and_replace_one<T, F, E>(&self, mut f: F) -> Result<T, E>
    where
        F: FnMut(Option<Nanos>) -> Result<(T, Nanos), E>,
    {
        let mut prev = self.0.load(Ordering::Acquire);
        let mut decision = f(NonZeroU64::new(prev).map(|n| n.get().into()));
        while let Ok((result, new_data)) = decision {
            match self.0.compare_exchange_weak(
                prev,
                new_data.into(),
                Ordering::Release,
                Ordering::Relaxed,
            ) {
                Ok(_) => return Ok(result),
                Err(next_prev) => prev = next_prev,
            }
            decision = f(NonZeroU64::new(prev).map(|n| n.get().into()));
        }
        // This map shouldn't be needed, as we only get here in the error case, but the compiler
        // can't see it.
        decision.map(|(result, _)| result)
    }

    pub(crate) fn is_older_than(&self, nanos: Nanos) -> bool {
        self.0.load(Ordering::Relaxed) <= nanos.into()
    }
}

/// The InMemoryState is the canonical "direct" state store.
impl StateStore for InMemoryState {
    type Key = NotKeyed;

    fn measure_and_replace<T, F, E>(&self, _key: &Self::Key, f: F) -> Result<T, E>
    where
        F: Fn(Option<Nanos>) -> Result<(T, Nanos), E>,
    {
        self.measure_and_replace_one(f)
    }
}

impl Debug for InMemoryState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        let d = Duration::from_nanos(self.0.load(Ordering::Relaxed));
        write!(f, "InMemoryState({:?})", d)
    }
}

#[cfg(test)]
mod test {

    use super::*;

    #[cfg(feature = "std")]
    fn try_triggering_collisions(n_threads: u64, tries_per_thread: u64) -> (u64, u64) {
        use std::sync::Arc;
        use std::thread;

        let mut state = Arc::new(InMemoryState(AtomicU64::new(0)));
        let threads: Vec<thread::JoinHandle<_>> = (0..n_threads)
            .map(|_| {
                thread::spawn({
                    let state = Arc::clone(&state);
                    move || {
                        let mut hits = 0;
                        for _ in 0..tries_per_thread {
                            assert!(state
                                .measure_and_replace_one(|old| {
                                    hits += 1;
                                    Ok::<((), Nanos), ()>((
                                        (),
                                        Nanos::from(old.map(Nanos::as_u64).unwrap_or(0) + 1),
                                    ))
                                })
                                .is_ok());
                        }
                        hits
                    }
                })
            })
            .collect();
        let hits: u64 = threads.into_iter().map(|t| t.join().unwrap()).sum();
        let value = Arc::get_mut(&mut state).unwrap().0.get_mut();
        (*value, hits)
    }

    #[cfg(feature = "std")]
    #[test]
    /// Checks that many threads running simultaneously will collide,
    /// but result in the correct number being recorded in the state.
    fn stresstest_collisions() {
        use all_asserts::assert_gt;

        const THREADS: u64 = 8;
        const MAX_TRIES: u64 = 20_000_000;
        let (mut value, mut hits) = (0, 0);
        for tries in (0..MAX_TRIES).step_by((MAX_TRIES / 100) as usize) {
            let attempt = try_triggering_collisions(THREADS, tries);
            value = attempt.0;
            hits = attempt.1;
            assert_eq!(value, tries * THREADS);
            if hits > value {
                break;
            }
            println!("Didn't trigger a collision in {} iterations", tries);
        }
        assert_gt!(hits, value);
    }

    #[test]
    fn in_memory_state_impls() {
        let state = InMemoryState(AtomicU64::new(0));
        assert!(format!("{:?}", state).len() > 0);
    }
}
