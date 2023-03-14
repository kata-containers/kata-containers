//! Futures compatibility for [`tracing`].
//!
//! # Overview
//!
//! [`tracing`] is a framework for instrumenting Rust programs to collect
//! structured, event-based diagnostic information. This crate provides utilities
//! for using `tracing` to instrument asynchronous code written using futures and
//! async/await.
//!
//! The crate provides the following traits:
//!
//! * [`Instrument`] allows a `tracing` [span] to be attached to a future, sink,
//!   stream, or executor.
//!
//! * [`WithSubscriber`] allows a `tracing` [`Subscriber`] to be attached to a
//!   future, sink, stream, or executor.
//!
//! *Compiler support: [requires `rustc` 1.42+][msrv]*
//!
//! [msrv]: #supported-rust-versions
//!
//! # Feature flags
//!
//! This crate provides a number of feature flags that enable compatibility
//! features with other crates in the asynchronous ecosystem:
//!
//! - `tokio`: Enables compatibility with the `tokio` crate, including
//!    [`Instrument`] and [`WithSubscriber`] implementations for
//!    `tokio::executor::Executor`, `tokio::runtime::Runtime`, and
//!    `tokio::runtime::current_thread`. Enabled by default.
//! - `tokio-executor`: Enables compatibility with the `tokio-executor`
//!    crate, including [`Instrument`] and [`WithSubscriber`]
//!    implementations for types implementing `tokio_executor::Executor`.
//!    This is intended primarily for use in crates which depend on
//!    `tokio-executor` rather than `tokio`; in general the `tokio` feature
//!    should be used instead.
//! - `std-future`: Enables compatibility with `std::future::Future`.
//! - `futures-01`: Enables compatibility with version 0.1.x of the [`futures`]
//!   crate.
//! - `futures-03`: Enables compatibility with version 0.3.x of the `futures`
//!   crate's `Spawn` and `LocalSpawn` traits.
//! - `tokio-alpha`: Enables compatibility with `tokio` 0.2's alpha releases,
//!   including the `tokio` 0.2 `Executor` and `TypedExecutor` traits.
//! - `std`: Depend on the Rust standard library.
//!
//!   `no_std` users may disable this feature with `default-features = false`:
//!
//!   ```toml
//!   [dependencies]
//!   tracing-futures = { version = "0.2.5", default-features = false }
//!   ```
//!
//! The `tokio`, `std-future` and `std` features are enabled by default.
//!
//! [`tracing`]: https://crates.io/crates/tracing
//! [span]: https://docs.rs/tracing/latest/tracing/span/index.html
//! [`Subscriber`]: https://docs.rs/tracing/latest/tracing/subscriber/index.html
//! [`Instrument`]: trait.Instrument.html
//! [`WithSubscriber`]: trait.WithSubscriber.html
//! [`futures`]: https://crates.io/crates/futures
//!
//! ## Supported Rust Versions
//!
//! Tracing is built against the latest stable release. The minimum supported
//! version is 1.42. The current Tracing version is not guaranteed to build on
//! Rust versions earlier than the minimum supported version.
//!
//! Tracing follows the same compiler support policies as the rest of the Tokio
//! project. The current stable Rust compiler and the three most recent minor
//! versions before it will always be supported. For example, if the current
//! stable compiler version is 1.45, the minimum supported version will not be
//! increased past 1.42, three minor versions prior. Increasing the minimum
//! supported compiler version is not considered a semver breaking change as
//! long as doing so complies with this policy.
//!
#![doc(html_root_url = "https://docs.rs/tracing-futures/0.2.5")]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/tokio-rs/tracing/master/assets/logo-type.png",
    issue_tracker_base_url = "https://github.com/tokio-rs/tracing/issues/"
)]
#![warn(
    missing_debug_implementations,
    missing_docs,
    rust_2018_idioms,
    unreachable_pub,
    bad_style,
    const_err,
    dead_code,
    improper_ctypes,
    non_shorthand_field_patterns,
    no_mangle_generic_items,
    overflowing_literals,
    path_statements,
    patterns_in_fns_without_body,
    private_in_public,
    unconditional_recursion,
    unused,
    unused_allocation,
    unused_comparisons,
    unused_parens,
    while_true
)]
#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(docsrs, feature(doc_cfg), deny(broken_intra_doc_links))]
#[cfg(feature = "std-future")]
use pin_project::pin_project;

pub(crate) mod stdlib;

#[cfg(feature = "std-future")]
use crate::stdlib::{pin::Pin, task::Context};

#[cfg(feature = "std")]
use tracing::{dispatcher, Dispatch};

use tracing::Span;

/// Implementations for `Instrument`ed future executors.
pub mod executor;

/// Extension trait allowing futures, streams, sinks, and executors to be
/// instrumented with a `tracing` [span].
///
/// [span]: https://docs.rs/tracing/latest/tracing/span/index.html
pub trait Instrument: Sized {
    /// Instruments this type with the provided `Span`, returning an
    /// `Instrumented` wrapper.
    ///
    /// If the instrumented type is a future, stream, or sink, the attached `Span`
    /// will be [entered] every time it is polled. If the instrumented type
    /// is a future executor, every future spawned on that executor will be
    /// instrumented by the attached `Span`.
    ///
    /// # Examples
    ///
    /// Instrumenting a future:
    ///
    // TODO: ignored until async-await is stable...
    /// ```rust,ignore
    /// use tracing_futures::Instrument;
    ///
    /// # async fn doc() {
    /// let my_future = async {
    ///     // ...
    /// };
    ///
    /// my_future
    ///     .instrument(tracing::info_span!("my_future"))
    ///     .await
    /// # }
    /// ```
    ///
    /// [entered]: https://docs.rs/tracing/latest/tracing/span/struct.Span.html#method.enter
    fn instrument(self, span: Span) -> Instrumented<Self> {
        Instrumented { inner: self, span }
    }

    /// Instruments this type with the [current] `Span`, returning an
    /// `Instrumented` wrapper.
    ///
    /// If the instrumented type is a future, stream, or sink, the attached `Span`
    /// will be [entered] every time it is polled. If the instrumented type
    /// is a future executor, every future spawned on that executor will be
    /// instrumented by the attached `Span`.
    ///
    /// This can be used to propagate the current span when spawning a new future.
    ///
    /// # Examples
    ///
    // TODO: ignored until async-await is stable...
    /// ```rust,ignore
    /// use tracing_futures::Instrument;
    ///
    /// # async fn doc() {
    /// let span = tracing::info_span!("my_span");
    /// let _enter = span.enter();
    ///
    /// // ...
    ///
    /// let future = async {
    ///     tracing::debug!("this event will occur inside `my_span`");
    ///     // ...
    /// };
    /// tokio::spawn(future.in_current_span());
    /// # }
    /// ```
    ///
    /// [current]: https://docs.rs/tracing/latest/tracing/span/struct.Span.html#method.current
    /// [entered]: https://docs.rs/tracing/latest/tracing/span/struct.Span.html#method.enter
    #[inline]
    fn in_current_span(self) -> Instrumented<Self> {
        self.instrument(Span::current())
    }
}

/// Extension trait allowing futures, streams, and sinks to be instrumented with
/// a `tracing` [`Subscriber`].
///
/// [`Subscriber`]: https://docs.rs/tracing/latest/tracing/subscriber/trait.Subscriber.html
#[cfg(feature = "std")]
#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
pub trait WithSubscriber: Sized {
    /// Attaches the provided [`Subscriber`] to this type, returning a
    /// `WithDispatch` wrapper.
    ///
    /// When the wrapped type is a future, stream, or sink, the attached
    /// subscriber will be set as the [default] while it is being polled.
    /// When the wrapped type is an executor, the subscriber will be set as the
    /// default for any futures spawned on that executor.
    ///
    /// [`Subscriber`]: https://docs.rs/tracing/latest/tracing/subscriber/trait.Subscriber.html
    /// [default]: https://docs.rs/tracing/latest/tracing/dispatcher/index.html#setting-the-default-subscriber
    fn with_subscriber<S>(self, subscriber: S) -> WithDispatch<Self>
    where
        S: Into<Dispatch>,
    {
        WithDispatch {
            inner: self,
            dispatch: subscriber.into(),
        }
    }

    /// Attaches the current [default] [`Subscriber`] to this type, returning a
    /// `WithDispatch` wrapper.
    ///
    /// When the wrapped type is a future, stream, or sink, the attached
    /// subscriber will be set as the [default] while it is being polled.
    /// When the wrapped type is an executor, the subscriber will be set as the
    /// default for any futures spawned on that executor.
    ///
    /// This can be used to propagate the current dispatcher context when
    /// spawning a new future.
    ///
    /// [`Subscriber`]: https://docs.rs/tracing/latest/tracing/subscriber/trait.Subscriber.html
    /// [default]: https://docs.rs/tracing/latest/tracing/dispatcher/index.html#setting-the-default-subscriber
    #[inline]
    fn with_current_subscriber(self) -> WithDispatch<Self> {
        WithDispatch {
            inner: self,
            dispatch: dispatcher::get_default(|default| default.clone()),
        }
    }
}

/// A future, stream, sink, or executor that has been instrumented with a `tracing` span.
#[cfg_attr(feature = "std-future", pin_project)]
#[derive(Debug, Clone)]
pub struct Instrumented<T> {
    #[cfg(feature = "std-future")]
    #[pin]
    inner: T,
    #[cfg(not(feature = "std-future"))]
    inner: T,
    span: Span,
}

/// A future, stream, sink, or executor that has been instrumented with a
/// `tracing` subscriber.
#[cfg(feature = "std")]
#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
#[cfg_attr(feature = "std-future", pin_project)]
#[derive(Clone, Debug)]
pub struct WithDispatch<T> {
    // cfg_attr doesn't work inside structs, apparently...
    #[cfg(feature = "std-future")]
    #[pin]
    inner: T,
    #[cfg(not(feature = "std-future"))]
    inner: T,
    dispatch: Dispatch,
}

impl<T: Sized> Instrument for T {}

#[cfg(feature = "std-future")]
#[cfg_attr(docsrs, doc(cfg(feature = "std-future")))]
impl<T: crate::stdlib::future::Future> crate::stdlib::future::Future for Instrumented<T> {
    type Output = T::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> crate::stdlib::task::Poll<Self::Output> {
        let this = self.project();
        let _enter = this.span.enter();
        this.inner.poll(cx)
    }
}

#[cfg(feature = "futures-01")]
#[cfg_attr(docsrs, doc(cfg(feature = "futures-01")))]
impl<T: futures_01::Future> futures_01::Future for Instrumented<T> {
    type Item = T::Item;
    type Error = T::Error;

    fn poll(&mut self) -> futures_01::Poll<Self::Item, Self::Error> {
        let _enter = self.span.enter();
        self.inner.poll()
    }
}

#[cfg(feature = "futures-01")]
#[cfg_attr(docsrs, doc(cfg(feature = "futures-01")))]
impl<T: futures_01::Stream> futures_01::Stream for Instrumented<T> {
    type Item = T::Item;
    type Error = T::Error;

    fn poll(&mut self) -> futures_01::Poll<Option<Self::Item>, Self::Error> {
        let _enter = self.span.enter();
        self.inner.poll()
    }
}

#[cfg(feature = "futures-01")]
#[cfg_attr(docsrs, doc(cfg(feature = "futures-01")))]
impl<T: futures_01::Sink> futures_01::Sink for Instrumented<T> {
    type SinkItem = T::SinkItem;
    type SinkError = T::SinkError;

    fn start_send(
        &mut self,
        item: Self::SinkItem,
    ) -> futures_01::StartSend<Self::SinkItem, Self::SinkError> {
        let _enter = self.span.enter();
        self.inner.start_send(item)
    }

    fn poll_complete(&mut self) -> futures_01::Poll<(), Self::SinkError> {
        let _enter = self.span.enter();
        self.inner.poll_complete()
    }
}

#[cfg(all(feature = "futures-03", feature = "std-future"))]
#[cfg_attr(docsrs, doc(cfg(all(feature = "futures-03", feature = "std-future"))))]
impl<T: futures::Stream> futures::Stream for Instrumented<T> {
    type Item = T::Item;

    fn poll_next(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> futures::task::Poll<Option<Self::Item>> {
        let this = self.project();
        let _enter = this.span.enter();
        T::poll_next(this.inner, cx)
    }
}

#[cfg(all(feature = "futures-03", feature = "std-future"))]
#[cfg_attr(docsrs, doc(cfg(all(feature = "futures-03", feature = "std-future"))))]
impl<I, T: futures::Sink<I>> futures::Sink<I> for Instrumented<T>
where
    T: futures::Sink<I>,
{
    type Error = T::Error;

    fn poll_ready(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> futures::task::Poll<Result<(), Self::Error>> {
        let this = self.project();
        let _enter = this.span.enter();
        T::poll_ready(this.inner, cx)
    }

    fn start_send(self: Pin<&mut Self>, item: I) -> Result<(), Self::Error> {
        let this = self.project();
        let _enter = this.span.enter();
        T::start_send(this.inner, item)
    }

    fn poll_flush(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> futures::task::Poll<Result<(), Self::Error>> {
        let this = self.project();
        let _enter = this.span.enter();
        T::poll_flush(this.inner, cx)
    }

    fn poll_close(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> futures::task::Poll<Result<(), Self::Error>> {
        let this = self.project();
        let _enter = this.span.enter();
        T::poll_close(this.inner, cx)
    }
}

impl<T> Instrumented<T> {
    /// Borrows the `Span` that this type is instrumented by.
    pub fn span(&self) -> &Span {
        &self.span
    }

    /// Mutably borrows the `Span` that this type is instrumented by.
    pub fn span_mut(&mut self) -> &mut Span {
        &mut self.span
    }

    /// Borrows the wrapped type.
    pub fn inner(&self) -> &T {
        &self.inner
    }

    /// Mutably borrows the wrapped type.
    pub fn inner_mut(&mut self) -> &mut T {
        &mut self.inner
    }

    /// Get a pinned reference to the wrapped type.
    #[cfg(feature = "std-future")]
    #[cfg_attr(docsrs, doc(cfg(feature = "std-future")))]
    pub fn inner_pin_ref(self: Pin<&Self>) -> Pin<&T> {
        self.project_ref().inner
    }

    /// Get a pinned mutable reference to the wrapped type.
    #[cfg(feature = "std-future")]
    #[cfg_attr(docsrs, doc(cfg(feature = "std-future")))]
    pub fn inner_pin_mut(self: Pin<&mut Self>) -> Pin<&mut T> {
        self.project().inner
    }

    /// Consumes the `Instrumented`, returning the wrapped type.
    ///
    /// Note that this drops the span.
    pub fn into_inner(self) -> T {
        self.inner
    }
}

#[cfg(feature = "std")]
impl<T: Sized> WithSubscriber for T {}

#[cfg(all(feature = "futures-01", feature = "std"))]
#[cfg_attr(docsrs, doc(cfg(all(feature = "futures-01", feature = "std"))))]
impl<T: futures_01::Future> futures_01::Future for WithDispatch<T> {
    type Item = T::Item;
    type Error = T::Error;

    fn poll(&mut self) -> futures_01::Poll<Self::Item, Self::Error> {
        let inner = &mut self.inner;
        dispatcher::with_default(&self.dispatch, || inner.poll())
    }
}

#[cfg(all(feature = "std-future", feature = "std"))]
#[cfg_attr(docsrs, doc(cfg(all(feature = "std-future", feature = "std"))))]
impl<T: crate::stdlib::future::Future> crate::stdlib::future::Future for WithDispatch<T> {
    type Output = T::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> crate::stdlib::task::Poll<Self::Output> {
        let this = self.project();
        let dispatch = this.dispatch;
        let future = this.inner;
        dispatcher::with_default(dispatch, || future.poll(cx))
    }
}

#[cfg(feature = "std")]
impl<T> WithDispatch<T> {
    /// Wrap a future, stream, sink or executor with the same subscriber as this WithDispatch.
    pub fn with_dispatch<U>(&self, inner: U) -> WithDispatch<U> {
        WithDispatch {
            dispatch: self.dispatch.clone(),
            inner,
        }
    }

    /// Borrows the `Dispatch` that this type is instrumented by.
    pub fn dispatch(&self) -> &Dispatch {
        &self.dispatch
    }

    /// Get a pinned reference to the wrapped type.
    #[cfg(feature = "std-future")]
    #[cfg_attr(docsrs, doc(cfg(feature = "std-future")))]
    pub fn inner_pin_ref(self: Pin<&Self>) -> Pin<&T> {
        self.project_ref().inner
    }

    /// Get a pinned mutable reference to the wrapped type.
    #[cfg(feature = "std-future")]
    #[cfg_attr(docsrs, doc(cfg(feature = "std-future")))]
    pub fn inner_pin_mut(self: Pin<&mut Self>) -> Pin<&mut T> {
        self.project().inner
    }

    /// Borrows the wrapped type.
    pub fn inner(&self) -> &T {
        &self.inner
    }

    /// Mutably borrows the wrapped type.
    pub fn inner_mut(&mut self) -> &mut T {
        &mut self.inner
    }

    /// Consumes the `WithDispatch`, returning the wrapped type.
    pub fn into_inner(self) -> T {
        self.inner
    }
}

#[cfg(test)]
pub(crate) use self::support as test_support;
// This has to have the same name as the module in `tracing`.
#[path = "../../tracing/tests/support/mod.rs"]
#[cfg(test)]
#[allow(unreachable_pub)]
pub(crate) mod support;

#[cfg(test)]
mod tests {
    use super::{test_support::*, *};

    #[cfg(feature = "futures-01")]
    mod futures_01_tests {
        use futures_01::{future, stream, task, Async, Future, Stream};
        use tracing::subscriber::with_default;

        use super::*;

        struct PollN<T, E> {
            and_return: Option<Result<T, E>>,
            finish_at: usize,
            polls: usize,
        }

        impl PollN<(), ()> {
            fn new_ok(finish_at: usize) -> Self {
                Self {
                    and_return: Some(Ok(())),
                    finish_at,
                    polls: 0,
                }
            }

            fn new_err(finish_at: usize) -> Self {
                Self {
                    and_return: Some(Err(())),
                    finish_at,
                    polls: 0,
                }
            }
        }

        impl<T, E> futures_01::Future for PollN<T, E> {
            type Item = T;
            type Error = E;
            fn poll(&mut self) -> futures_01::Poll<Self::Item, Self::Error> {
                self.polls += 1;
                if self.polls == self.finish_at {
                    self.and_return
                        .take()
                        .expect("polled after ready")
                        .map(Async::Ready)
                } else {
                    task::current().notify();
                    Ok(Async::NotReady)
                }
            }
        }

        #[test]
        fn future_enter_exit_is_reasonable() {
            let (subscriber, handle) = subscriber::mock()
                .enter(span::mock().named("foo"))
                .exit(span::mock().named("foo"))
                .enter(span::mock().named("foo"))
                .exit(span::mock().named("foo"))
                .drop_span(span::mock().named("foo"))
                .done()
                .run_with_handle();
            with_default(subscriber, || {
                PollN::new_ok(2)
                    .instrument(tracing::trace_span!("foo"))
                    .wait()
                    .unwrap();
            });
            handle.assert_finished();
        }

        #[test]
        fn future_error_ends_span() {
            let (subscriber, handle) = subscriber::mock()
                .enter(span::mock().named("foo"))
                .exit(span::mock().named("foo"))
                .enter(span::mock().named("foo"))
                .exit(span::mock().named("foo"))
                .drop_span(span::mock().named("foo"))
                .done()
                .run_with_handle();
            with_default(subscriber, || {
                PollN::new_err(2)
                    .instrument(tracing::trace_span!("foo"))
                    .wait()
                    .unwrap_err();
            });

            handle.assert_finished();
        }

        #[test]
        fn stream_enter_exit_is_reasonable() {
            let (subscriber, handle) = subscriber::mock()
                .enter(span::mock().named("foo"))
                .exit(span::mock().named("foo"))
                .enter(span::mock().named("foo"))
                .exit(span::mock().named("foo"))
                .enter(span::mock().named("foo"))
                .exit(span::mock().named("foo"))
                .enter(span::mock().named("foo"))
                .exit(span::mock().named("foo"))
                .drop_span(span::mock().named("foo"))
                .run_with_handle();
            with_default(subscriber, || {
                stream::iter_ok::<_, ()>(&[1, 2, 3])
                    .instrument(tracing::trace_span!("foo"))
                    .for_each(|_| future::ok(()))
                    .wait()
                    .unwrap();
            });
            handle.assert_finished();
        }

        #[test]
        fn span_follows_future_onto_threadpool() {
            let (subscriber, handle) = subscriber::mock()
                .enter(span::mock().named("a"))
                .enter(span::mock().named("b"))
                .exit(span::mock().named("b"))
                .enter(span::mock().named("b"))
                .exit(span::mock().named("b"))
                .drop_span(span::mock().named("b"))
                .exit(span::mock().named("a"))
                .drop_span(span::mock().named("a"))
                .done()
                .run_with_handle();
            let mut runtime = tokio::runtime::Runtime::new().unwrap();
            with_default(subscriber, || {
                tracing::trace_span!("a").in_scope(|| {
                    let future = PollN::new_ok(2)
                        .instrument(tracing::trace_span!("b"))
                        .map(|_| {
                            tracing::trace_span!("c").in_scope(|| {
                                // "c" happens _outside_ of the instrumented future's
                                // span, so we don't expect it.
                            })
                        });
                    runtime.block_on(Box::new(future)).unwrap();
                })
            });
            handle.assert_finished();
        }
    }

    #[cfg(all(feature = "futures-03", feature = "std-future"))]
    mod futures_03_tests {
        use futures::{future, sink, stream, FutureExt, SinkExt, StreamExt};
        use tracing::subscriber::with_default;

        use super::*;

        #[test]
        fn stream_enter_exit_is_reasonable() {
            let (subscriber, handle) = subscriber::mock()
                .enter(span::mock().named("foo"))
                .exit(span::mock().named("foo"))
                .enter(span::mock().named("foo"))
                .exit(span::mock().named("foo"))
                .enter(span::mock().named("foo"))
                .exit(span::mock().named("foo"))
                .enter(span::mock().named("foo"))
                .exit(span::mock().named("foo"))
                .drop_span(span::mock().named("foo"))
                .run_with_handle();
            with_default(subscriber, || {
                Instrument::instrument(stream::iter(&[1, 2, 3]), tracing::trace_span!("foo"))
                    .for_each(|_| future::ready(()))
                    .now_or_never()
                    .unwrap();
            });
            handle.assert_finished();
        }

        #[test]
        fn sink_enter_exit_is_reasonable() {
            let (subscriber, handle) = subscriber::mock()
                .enter(span::mock().named("foo"))
                .exit(span::mock().named("foo"))
                .enter(span::mock().named("foo"))
                .exit(span::mock().named("foo"))
                .enter(span::mock().named("foo"))
                .exit(span::mock().named("foo"))
                .drop_span(span::mock().named("foo"))
                .run_with_handle();
            with_default(subscriber, || {
                Instrument::instrument(sink::drain(), tracing::trace_span!("foo"))
                    .send(1u8)
                    .now_or_never()
                    .unwrap()
                    .unwrap()
            });
            handle.assert_finished();
        }
    }
}
