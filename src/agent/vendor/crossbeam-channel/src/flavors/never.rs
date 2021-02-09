//! Channel that never delivers messages.
//!
//! Messages cannot be sent into this kind of channel.

use std::marker::PhantomData;
use std::time::Instant;

use context::Context;
use err::{RecvTimeoutError, TryRecvError};
use select::{Operation, SelectHandle, Token};
use utils;

/// This flavor doesn't need a token.
pub type NeverToken = ();

/// Channel that never delivers messages.
pub struct Channel<T> {
    _marker: PhantomData<T>,
}

impl<T> Channel<T> {
    /// Creates a channel that never delivers messages.
    #[inline]
    pub fn new() -> Self {
        Channel {
            _marker: PhantomData,
        }
    }

    /// Attempts to receive a message without blocking.
    #[inline]
    pub fn try_recv(&self) -> Result<T, TryRecvError> {
        Err(TryRecvError::Empty)
    }

    /// Receives a message from the channel.
    #[inline]
    pub fn recv(&self, deadline: Option<Instant>) -> Result<T, RecvTimeoutError> {
        utils::sleep_until(deadline);
        Err(RecvTimeoutError::Timeout)
    }

    /// Reads a message from the channel.
    #[inline]
    pub unsafe fn read(&self, _token: &mut Token) -> Result<T, ()> {
        Err(())
    }

    /// Returns `true` if the channel is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        true
    }

    /// Returns `true` if the channel is full.
    #[inline]
    pub fn is_full(&self) -> bool {
        true
    }

    /// Returns the number of messages in the channel.
    #[inline]
    pub fn len(&self) -> usize {
        0
    }

    /// Returns the capacity of the channel.
    #[inline]
    pub fn capacity(&self) -> Option<usize> {
        Some(0)
    }
}

impl<T> SelectHandle for Channel<T> {
    #[inline]
    fn try_select(&self, _token: &mut Token) -> bool {
        false
    }

    #[inline]
    fn deadline(&self) -> Option<Instant> {
        None
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
        false
    }

    #[inline]
    fn watch(&self, _oper: Operation, _cx: &Context) -> bool {
        self.is_ready()
    }

    #[inline]
    fn unwatch(&self, _oper: Operation) {}
}
