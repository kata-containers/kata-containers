//! Support for creating futures that represent timeouts.
//!
//! This module contains the `Delay` type which is a future that will resolve
//! at a particular point in the future.

use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering::SeqCst;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use super::arc_list::Node;
use super::AtomicWaker;
use super::{ScheduledTimer, TimerHandle};

/// A future representing the notification that an elapsed duration has
/// occurred.
///
/// This is created through the `Delay::new` method indicating when the future should fire.
/// Note that these futures are not intended for high resolution timers, but rather they will
/// likely fire some granularity after the exact instant that they're otherwise indicated to fire
/// at.
pub struct Delay {
    state: Option<Arc<Node<ScheduledTimer>>>,
}

impl Delay {
    /// Creates a new future which will fire at `dur` time into the future.
    ///
    /// The returned object will be bound to the default timer for this thread.
    /// The default timer will be spun up in a helper thread on first use.
    #[inline]
    pub fn new(dur: Duration) -> Delay {
        Delay::new_handle(Instant::now() + dur, Default::default())
    }

    /// Creates a new future which will fire at the time specified by `at`.
    ///
    /// The returned instance of `Delay` will be bound to the timer specified by
    /// the `handle` argument.
    pub(crate) fn new_handle(at: Instant, handle: TimerHandle) -> Delay {
        let inner = match handle.inner.upgrade() {
            Some(i) => i,
            None => return Delay { state: None },
        };
        let state = Arc::new(Node::new(ScheduledTimer {
            at: Mutex::new(Some(at)),
            state: AtomicUsize::new(0),
            waker: AtomicWaker::new(),
            inner: handle.inner,
            slot: Mutex::new(None),
        }));

        // If we fail to actually push our node then we've become an inert
        // timer, meaning that we'll want to immediately return an error from
        // `poll`.
        if inner.list.push(&state).is_err() {
            return Delay { state: None };
        }

        inner.waker.wake();
        Delay { state: Some(state) }
    }

    /// Resets this timeout to an new timeout which will fire at the time
    /// specified by `at`.
    #[inline]
    pub fn reset(&mut self, dur: Duration) {
        if self._reset(dur).is_err() {
            self.state = None
        }
    }

    fn _reset(&mut self, dur: Duration) -> Result<(), ()> {
        let state = match self.state {
            Some(ref state) => state,
            None => return Err(()),
        };
        if let Some(timeouts) = state.inner.upgrade() {
            let mut bits = state.state.load(SeqCst);
            loop {
                // If we've been invalidated, cancel this reset
                if bits & 0b10 != 0 {
                    return Err(());
                }
                let new = bits.wrapping_add(0b100) & !0b11;
                match state.state.compare_exchange(bits, new, SeqCst, SeqCst) {
                    Ok(_) => break,
                    Err(s) => bits = s,
                }
            }
            *state.at.lock().unwrap() = Some(Instant::now() + dur);
            // If we fail to push our node then we've become an inert timer, so
            // we'll want to clear our `state` field accordingly
            timeouts.list.push(state)?;
            timeouts.waker.wake();
        }

        Ok(())
    }
}

impl Future for Delay {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let state = match self.state {
            Some(ref state) => state,
            None => panic!("timer has gone away"),
        };

        if state.state.load(SeqCst) & 1 != 0 {
            return Poll::Ready(());
        }

        state.waker.register(&cx.waker());

        // Now that we've registered, do the full check of our own internal
        // state. If we've fired the first bit is set, and if we've been
        // invalidated the second bit is set.
        match state.state.load(SeqCst) {
            n if n & 0b01 != 0 => Poll::Ready(()),
            n if n & 0b10 != 0 => panic!("timer has gone away"),
            _ => Poll::Pending,
        }
    }
}

impl Drop for Delay {
    fn drop(&mut self) {
        let state = match self.state {
            Some(ref s) => s,
            None => return,
        };
        if let Some(timeouts) = state.inner.upgrade() {
            *state.at.lock().unwrap() = None;
            if timeouts.list.push(state).is_ok() {
                timeouts.waker.wake();
            }
        }
    }
}

impl fmt::Debug for Delay {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        f.debug_struct("Delay").finish()
    }
}
