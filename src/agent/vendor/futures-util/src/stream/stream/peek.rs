use crate::stream::{Fuse, StreamExt};
use core::fmt;
use core::pin::Pin;
use futures_core::future::{FusedFuture, Future};
use futures_core::stream::{FusedStream, Stream};
use futures_core::task::{Context, Poll};
#[cfg(feature = "sink")]
use futures_sink::Sink;
use pin_project::pin_project;

/// A `Stream` that implements a `peek` method.
///
/// The `peek` method can be used to retrieve a reference
/// to the next `Stream::Item` if available. A subsequent
/// call to `poll` will return the owned item.
#[pin_project]
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct Peekable<St: Stream> {
    #[pin]
    stream: Fuse<St>,
    peeked: Option<St::Item>,
}

impl<St: Stream> Peekable<St> {
    pub(super) fn new(stream: St) -> Self {
        Self {
            stream: stream.fuse(),
            peeked: None,
        }
    }

    delegate_access_inner!(stream, St, (.));

    /// Produces a `Peek` future which retrieves a reference to the next item
    /// in the stream, or `None` if the underlying stream terminates.
    pub fn peek(self: Pin<&mut Self>) -> Peek<'_, St> {
        Peek { inner: Some(self) }
    }

    /// Peek retrieves a reference to the next item in the stream.
    ///
    /// This method polls the underlying stream and return either a reference
    /// to the next item if the stream is ready or passes through any errors.
    pub fn poll_peek(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<&St::Item>> {
        let mut this = self.project();

        Poll::Ready(loop {
            if this.peeked.is_some() {
                break this.peeked.as_ref();
            } else if let Some(item) = ready!(this.stream.as_mut().poll_next(cx)) {
                *this.peeked = Some(item);
            } else {
                break None;
            }
        })
    }
}

impl<St: Stream> FusedStream for Peekable<St> {
    fn is_terminated(&self) -> bool {
        self.peeked.is_none() && self.stream.is_terminated()
    }
}

impl<S: Stream> Stream for Peekable<S> {
    type Item = S::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.project();
        if let Some(item) = this.peeked.take() {
            return Poll::Ready(Some(item));
        }
        this.stream.poll_next(cx)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let peek_len = if self.peeked.is_some() { 1 } else { 0 };
        let (lower, upper) = self.stream.size_hint();
        let lower = lower.saturating_add(peek_len);
        let upper = match upper {
            Some(x) => x.checked_add(peek_len),
            None => None,
        };
        (lower, upper)
    }
}

// Forwarding impl of Sink from the underlying stream
#[cfg(feature = "sink")]
impl<S, Item> Sink<Item> for Peekable<S>
where
    S: Sink<Item> + Stream,
{
    type Error = S::Error;

    delegate_sink!(stream, Item);
}

/// Future for the [`Peekable::peek()`](self::Peekable::peek) function from [`Peekable`]
#[pin_project]
#[must_use = "futures do nothing unless polled"]
pub struct Peek<'a, St: Stream> {
    inner: Option<Pin<&'a mut Peekable<St>>>,
}

impl<St> fmt::Debug for Peek<'_, St>
where
    St: Stream + fmt::Debug,
    St::Item: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Peek")
            .field("inner", &self.inner)
            .finish()
    }
}

impl<St: Stream> FusedFuture for Peek<'_, St> {
    fn is_terminated(&self) -> bool {
        self.inner.is_none()
    }
}

impl<'a, St> Future for Peek<'a, St>
where
    St: Stream,
{
    type Output = Option<&'a St::Item>;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let inner = self.project().inner;
        if let Some(peekable) = inner {
            ready!(peekable.as_mut().poll_peek(cx));

            inner.take().unwrap().poll_peek(cx)
        } else {
            panic!("Peek polled after completion")
        }
    }
}
