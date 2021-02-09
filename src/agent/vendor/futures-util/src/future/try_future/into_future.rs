use core::pin::Pin;
use futures_core::future::{FusedFuture, Future, TryFuture};
use futures_core::task::{Context, Poll};
use pin_project::pin_project;

/// Future for the [`into_future`](super::TryFutureExt::into_future) method.
#[pin_project]
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct IntoFuture<Fut>(#[pin] Fut);

impl<Fut> IntoFuture<Fut> {
    #[inline]
    pub(crate) fn new(future: Fut) -> Self {
        Self(future)
    }
}

impl<Fut: TryFuture + FusedFuture> FusedFuture for IntoFuture<Fut> {
    fn is_terminated(&self) -> bool { self.0.is_terminated() }
}

impl<Fut: TryFuture> Future for IntoFuture<Fut> {
    type Output = Result<Fut::Ok, Fut::Error>;

    #[inline]
    fn poll(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Self::Output> {
        self.project().0.try_poll(cx)
    }
}
