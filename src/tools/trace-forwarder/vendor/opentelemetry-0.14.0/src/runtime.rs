//! Provides an abstraction of several async runtimes
//!
//! This  allows OpenTelemetry to work with any current or future runtime. There are currently
//! builtin implementations for [Tokio] and [async-std].
//!
//! [Tokio]: https://crates.io/crates/tokio
//! [async-std]: https://crates.io/crates/async-std

use futures::{future::BoxFuture, Stream};
use std::{future::Future, time::Duration};

/// A runtime is an abstraction of an async runtime like [Tokio] or [async-std]. It allows
/// OpenTelemetry to work with any current and hopefully future runtime implementation.
///
/// [Tokio]: https://crates.io/crates/tokio
/// [async-std]: https://crates.io/crates/async-std
pub trait Runtime: Clone + Send + Sync + 'static {
    /// A future stream, which returns items in a previously specified interval. The item type is
    /// not important.
    type Interval: Stream + Send;

    /// A future, which resolves after a previously specified amount of time. The output type is
    /// not important.
    type Delay: Future + Send;

    /// Create a [Stream][futures::Stream], which returns a new item every
    /// [Duration][std::time::Duration].
    fn interval(&self, duration: Duration) -> Self::Interval;

    /// Spawn a new task or thread, which executes the given future.
    ///
    /// # Note
    ///
    /// This is mainly used to run batch span processing in the background. Note, that the function
    /// does not return a handle. OpenTelemetry will use a different way to wait for the future to
    /// finish when TracerProvider gets shutdown. At the moment this happens by blocking the
    /// current thread. This means runtime implementations need to make sure they can still execute
    /// the given future even if the main thread is blocked.
    fn spawn(&self, future: BoxFuture<'static, ()>);

    /// Return a new future, which resolves after the specified [Duration][std::time::Duration].
    fn delay(&self, duration: Duration) -> Self::Delay;
}

/// Runtime implementation, which works with Tokio's multi thread runtime.
#[cfg(feature = "rt-tokio")]
#[cfg_attr(docsrs, doc(cfg(feature = "rt-tokio")))]
#[derive(Debug, Clone)]
pub struct Tokio;

#[cfg(feature = "rt-tokio")]
#[cfg_attr(docsrs, doc(cfg(feature = "rt-tokio")))]
impl Runtime for Tokio {
    type Interval = tokio_stream::wrappers::IntervalStream;
    type Delay = tokio::time::Sleep;

    fn interval(&self, duration: Duration) -> Self::Interval {
        crate::util::tokio_interval_stream(duration)
    }

    fn spawn(&self, future: BoxFuture<'static, ()>) {
        let _ = tokio::spawn(future);
    }

    fn delay(&self, duration: Duration) -> Self::Delay {
        tokio::time::sleep(duration)
    }
}

/// Runtime implementation, which works with Tokio's current thread runtime.
#[cfg(feature = "rt-tokio-current-thread")]
#[cfg_attr(docsrs, doc(cfg(feature = "rt-tokio-current-thread")))]
#[derive(Debug, Clone)]
pub struct TokioCurrentThread;

#[cfg(feature = "rt-tokio-current-thread")]
#[cfg_attr(docsrs, doc(cfg(feature = "rt-tokio-current-thread")))]
impl Runtime for TokioCurrentThread {
    type Interval = tokio_stream::wrappers::IntervalStream;
    type Delay = tokio::time::Sleep;

    fn interval(&self, duration: Duration) -> Self::Interval {
        crate::util::tokio_interval_stream(duration)
    }

    fn spawn(&self, future: BoxFuture<'static, ()>) {
        // We cannot force push tracing in current thread tokio scheduler because we rely on
        // BatchSpanProcessor to export spans in a background task, meanwhile we need to block the
        // shutdown function so that the runtime will not finish the blocked task and kill any
        // remaining tasks. But there is only one thread to run task, so it's a deadlock
        //
        // Thus, we spawn the background task in a separate thread.
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("failed to create Tokio current thead runtime for OpenTelemetry batch processing");
            rt.block_on(future);
        });
    }

    fn delay(&self, duration: Duration) -> Self::Delay {
        tokio::time::sleep(duration)
    }
}

/// Runtime implementation, which works with async-std.
#[cfg(feature = "rt-async-std")]
#[cfg_attr(docsrs, doc(cfg(feature = "rt-async-std")))]
#[derive(Debug, Clone)]
pub struct AsyncStd;

#[cfg(feature = "rt-async-std")]
#[cfg_attr(docsrs, doc(cfg(feature = "rt-async-std")))]
impl Runtime for AsyncStd {
    type Interval = async_std::stream::Interval;
    type Delay = BoxFuture<'static, ()>;

    fn interval(&self, duration: Duration) -> Self::Interval {
        async_std::stream::interval(duration)
    }

    fn spawn(&self, future: BoxFuture<'static, ()>) {
        let _ = async_std::task::spawn(future);
    }

    fn delay(&self, duration: Duration) -> Self::Delay {
        Box::pin(async_std::task::sleep(duration))
    }
}
