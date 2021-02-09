//! Channel that delivers a message after a certain amount of time.
//!
//! Messages cannot be sent into this kind of channel; they are materialized on demand.

use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use context::Context;
use err::{RecvTimeoutError, TryRecvError};
use select::{Operation, SelectHandle, Token};
use utils;

/// Result of a receive operation.
pub type AfterToken = Option<Instant>;

/// Channel that delivers a message after a certain amount of time.
pub struct Channel {
    /// The instant at which the message will be delivered.
    delivery_time: Instant,

    /// `true` if the message has been received.
    received: AtomicBool,
}

impl Channel {
    /// Creates a channel that delivers a message after a certain duration of time.
    #[inline]
    pub fn new(dur: Duration) -> Self {
        Channel {
            delivery_time: Instant::now() + dur,
            received: AtomicBool::new(false),
        }
    }

    /// Attempts to receive a message without blocking.
    #[inline]
    pub fn try_recv(&self) -> Result<Instant, TryRecvError> {
        // We use relaxed ordering because this is just an optional optimistic check.
        if self.received.load(Ordering::Relaxed) {
            // The message has already been received.
            return Err(TryRecvError::Empty);
        }

        if Instant::now() < self.delivery_time {
            // The message was not delivered yet.
            return Err(TryRecvError::Empty);
        }

        // Try receiving the message if it is still available.
        if !self.received.swap(true, Ordering::SeqCst) {
            // Success! Return delivery time as the message.
            Ok(self.delivery_time)
        } else {
            // The message was already received.
            Err(TryRecvError::Empty)
        }
    }

    /// Receives a message from the channel.
    #[inline]
    pub fn recv(&self, deadline: Option<Instant>) -> Result<Instant, RecvTimeoutError> {
        // We use relaxed ordering because this is just an optional optimistic check.
        if self.received.load(Ordering::Relaxed) {
            // The message has already been received.
            utils::sleep_until(deadline);
            return Err(RecvTimeoutError::Timeout);
        }

        // Wait until the message is received or the deadline is reached.
        loop {
            let now = Instant::now();

            // Check if we can receive the next message.
            if now >= self.delivery_time {
                break;
            }

            // Check if the deadline has been reached.
            if let Some(d) = deadline {
                if now >= d {
                    return Err(RecvTimeoutError::Timeout);
                }

                thread::sleep(self.delivery_time.min(d) - now);
            } else {
                thread::sleep(self.delivery_time - now);
            }
        }

        // Try receiving the message if it is still available.
        if !self.received.swap(true, Ordering::SeqCst) {
            // Success! Return the message, which is the instant at which it was delivered.
            Ok(self.delivery_time)
        } else {
            // The message was already received. Block forever.
            utils::sleep_until(None);
            unreachable!()
        }
    }

    /// Reads a message from the channel.
    #[inline]
    pub unsafe fn read(&self, token: &mut Token) -> Result<Instant, ()> {
        token.after.ok_or(())
    }

    /// Returns `true` if the channel is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        // We use relaxed ordering because this is just an optional optimistic check.
        if self.received.load(Ordering::Relaxed) {
            return true;
        }

        // If the delivery time hasn't been reached yet, the channel is empty.
        if Instant::now() < self.delivery_time {
            return true;
        }

        // The delivery time has been reached. The channel is empty only if the message has already
        // been received.
        self.received.load(Ordering::SeqCst)
    }

    /// Returns `true` if the channel is full.
    #[inline]
    pub fn is_full(&self) -> bool {
        !self.is_empty()
    }

    /// Returns the number of messages in the channel.
    #[inline]
    pub fn len(&self) -> usize {
        if self.is_empty() {
            0
        } else {
            1
        }
    }

    /// Returns the capacity of the channel.
    #[inline]
    pub fn capacity(&self) -> Option<usize> {
        Some(1)
    }
}

impl SelectHandle for Channel {
    #[inline]
    fn try_select(&self, token: &mut Token) -> bool {
        match self.try_recv() {
            Ok(msg) => {
                token.after = Some(msg);
                true
            }
            Err(TryRecvError::Disconnected) => {
                token.after = None;
                true
            }
            Err(TryRecvError::Empty) => false,
        }
    }

    #[inline]
    fn deadline(&self) -> Option<Instant> {
        // We use relaxed ordering because this is just an optional optimistic check.
        if self.received.load(Ordering::Relaxed) {
            None
        } else {
            Some(self.delivery_time)
        }
    }

    #[inline]
    fn register(&self, _oper: Operation, _cx: &Context) -> bool {
        self.is_ready()
    }

    #[inline]
    fn unregister(&self, _oper: Operation) {}

    #[inline]
    fn accept(&self, token: &mut Token, _cx: &Context) -> bool {
        self.try_select(token)
    }

    #[inline]
    fn is_ready(&self) -> bool {
        !self.is_empty()
    }

    #[inline]
    fn watch(&self, _oper: Operation, _cx: &Context) -> bool {
        self.is_ready()
    }

    #[inline]
    fn unwatch(&self, _oper: Operation) {}
}
