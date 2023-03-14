// Copyright (C) 2022 Alibaba Cloud. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0
// Async implementation of Multi-Producer-Multi-Consumer channel.

//! Asynchronous Multi-Producer Multi-Consumer channel.
//!
//! This module provides an asynchronous multi-producer multi-consumer channel based on [tokio::sync::Notify].

use std::collections::VecDeque;
use std::io::{Error, ErrorKind, Result};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard};
use tokio::sync::Notify;

/// An asynchronous multi-producer multi-consumer channel based on [tokio::sync::Notify].
pub struct Channel<T> {
    closed: AtomicBool,
    notifier: Notify,
    requests: Mutex<VecDeque<T>>,
}

impl<T> Default for Channel<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Channel<T> {
    /// Create a new instance of [`Channel`].
    pub fn new() -> Self {
        Channel {
            closed: AtomicBool::new(false),
            notifier: Notify::new(),
            requests: Mutex::new(VecDeque::new()),
        }
    }

    /// Close the channel.
    pub fn close(&self) {
        self.closed.store(true, Ordering::Release);
        self.notifier.notify_waiters();
    }

    /// Send a message to the channel.
    ///
    /// The message object will be returned on error, to ease the lifecycle management.
    pub fn send(&self, msg: T) -> std::result::Result<(), T> {
        if self.closed.load(Ordering::Acquire) {
            Err(msg)
        } else {
            self.requests.lock().unwrap().push_back(msg);
            self.notifier.notify_one();
            Ok(())
        }
    }

    /// Try to receive a message from the channel.
    pub fn try_recv(&self) -> Option<T> {
        self.requests.lock().unwrap().pop_front()
    }

    /// Receive message from the channel in asynchronous mode.
    pub async fn recv(&self) -> Result<T> {
        let future = self.notifier.notified();
        tokio::pin!(future);

        loop {
            // Make sure that no wakeup is lost if we get `None` from `try_recv`.
            future.as_mut().enable();

            if let Some(msg) = self.try_recv() {
                return Ok(msg);
            } else if self.closed.load(Ordering::Acquire) {
                return Err(Error::new(ErrorKind::BrokenPipe, "channel has been closed"));
            }

            // Wait for a call to `notify_one`.
            //
            // This uses `.as_mut()` to avoid consuming the future,
            // which lets us call `Pin::set` below.
            future.as_mut().await;

            // Reset the future in case another call to `try_recv` got the message before us.
            future.set(self.notifier.notified());
        }
    }

    /// Flush all pending requests specified by the predicator.
    ///
    pub fn flush_pending_prefetch_requests<F>(&self, mut f: F)
    where
        F: FnMut(&T) -> bool,
    {
        self.requests.lock().unwrap().retain(|t| !f(t));
    }

    /// Lock the channel to block all queue operations.
    pub fn lock_channel(&self) -> MutexGuard<VecDeque<T>> {
        self.requests.lock().unwrap()
    }

    /// Notify all waiters.
    pub fn notify_waiters(&self) {
        self.notifier.notify_waiters();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn test_new_channel() {
        let channel = Channel::new();

        channel.send(1u32).unwrap();
        channel.send(2u32).unwrap();
        assert_eq!(channel.try_recv().unwrap(), 1);
        assert_eq!(channel.try_recv().unwrap(), 2);

        channel.close();
        channel.send(2u32).unwrap_err();
    }

    #[test]
    fn test_flush_channel() {
        let channel = Channel::new();

        channel.send(1u32).unwrap();
        channel.send(2u32).unwrap();
        channel.flush_pending_prefetch_requests(|_| true);
        assert!(channel.try_recv().is_none());

        channel.notify_waiters();
        let _guard = channel.lock_channel();
    }

    #[test]
    fn test_async_recv() {
        let channel = Arc::new(Channel::new());
        let channel2 = channel.clone();

        let t = std::thread::spawn(move || {
            channel2.send(1u32).unwrap();
        });

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            let msg = channel.recv().await.unwrap();
            assert_eq!(msg, 1);
        });

        t.join().unwrap();
    }
}
