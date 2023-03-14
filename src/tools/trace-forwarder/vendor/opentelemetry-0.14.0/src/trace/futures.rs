//! # OpenTelemetry Futures Compatibility
//!
//! This module provides utilities for instrumenting asynchronous code written
//! using [`futures`] and async/await.
//!
//! This main trait is [`FutureExt`], which allows a [`Context`],
//! to be attached to a future, sink, or stream.
//!
//! [`futures`]: std::future::Future
//! [`Context`]: crate::Context
use crate::Context as OpenTelemetryContext;
use pin_project::pin_project;
use std::{
    pin::Pin,
    task::{Context as TaskContext, Poll},
};

/// A future, stream, or sink that has an associated context.
#[pin_project]
#[derive(Clone, Debug)]
pub struct WithContext<T> {
    #[pin]
    inner: T,
    otel_cx: OpenTelemetryContext,
}

impl<T: Sized> FutureExt for T {}

impl<T: std::future::Future> std::future::Future for WithContext<T> {
    type Output = T::Output;

    fn poll(self: Pin<&mut Self>, task_cx: &mut TaskContext<'_>) -> Poll<Self::Output> {
        let this = self.project();
        let _guard = this.otel_cx.clone().attach();

        this.inner.poll(task_cx)
    }
}

impl<T: futures::Stream> futures::Stream for WithContext<T> {
    type Item = T::Item;

    fn poll_next(self: Pin<&mut Self>, task_cx: &mut TaskContext<'_>) -> Poll<Option<Self::Item>> {
        let this = self.project();
        let _guard = this.otel_cx.clone().attach();
        T::poll_next(this.inner, task_cx)
    }
}

impl<I, T: futures::Sink<I>> futures::Sink<I> for WithContext<T>
where
    T: futures::Sink<I>,
{
    type Error = T::Error;

    fn poll_ready(
        self: Pin<&mut Self>,
        task_cx: &mut TaskContext<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        let this = self.project();
        let _guard = this.otel_cx.clone().attach();
        T::poll_ready(this.inner, task_cx)
    }

    fn start_send(self: Pin<&mut Self>, item: I) -> Result<(), Self::Error> {
        let this = self.project();
        let _guard = this.otel_cx.clone().attach();
        T::start_send(this.inner, item)
    }

    fn poll_flush(
        self: Pin<&mut Self>,
        task_cx: &mut TaskContext<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        let this = self.project();
        let _guard = this.otel_cx.clone().attach();
        T::poll_flush(this.inner, task_cx)
    }

    fn poll_close(
        self: Pin<&mut Self>,
        task_cx: &mut TaskContext<'_>,
    ) -> futures::task::Poll<Result<(), Self::Error>> {
        let this = self.project();
        let _enter = this.otel_cx.clone().attach();
        T::poll_close(this.inner, task_cx)
    }
}

/// Extension trait allowing futures, streams, and sinks to be traced with a span.
pub trait FutureExt: Sized {
    /// Attaches the provided [`Context`] to this type, returning a `WithContext`
    /// wrapper.
    ///
    /// When the wrapped type is a future, stream, or sink, the attached context
    /// will be set as current while it is being polled.
    ///
    /// [`Context`]: crate::Context
    fn with_context(self, otel_cx: OpenTelemetryContext) -> WithContext<Self> {
        WithContext {
            inner: self,
            otel_cx,
        }
    }

    /// Attaches the current [`Context`] to this type, returning a `WithContext`
    /// wrapper.
    ///
    /// When the wrapped type is a future, stream, or sink, the attached context
    /// will be set as the default while it is being polled.
    ///
    /// [`Context`]: crate::Context
    fn with_current_context(self) -> WithContext<Self> {
        let otel_cx = OpenTelemetryContext::current();
        self.with_context(otel_cx)
    }
}
