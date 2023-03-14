use std::prelude::v1::*;

use crate::{
    clock,
    middleware::RateLimitingMiddleware,
    state::{DirectStateStore, NotKeyed},
    Jitter, NotUntil, RateLimiter,
};
use futures::task::{Context, Poll};
use futures::{Future, Sink, Stream};
use futures_timer::Delay;
use std::marker::PhantomData;
use std::pin::Pin;

/// Allows converting a [`futures::Sink`] combinator into a rate-limited sink.
pub trait SinkRateLimitExt<Item, S>: Sink<Item>
where
    S: Sink<Item>,
{
    /// Limits the rate at which items can be put into the current sink.
    fn ratelimit_sink<
        D: DirectStateStore,
        C: clock::ReasonablyRealtime,
        MW: RateLimitingMiddleware<C::Instant, NegativeOutcome = NotUntil<C::Instant>>,
    >(
        self,
        limiter: &'_ RateLimiter<NotKeyed, D, C, MW>,
    ) -> RatelimitedSink<'_, Item, S, D, C, MW>
    where
        Self: Sized;

    /// Limits the rate at which items can be put into the current sink, with a randomized wait
    /// period.
    #[cfg(feature = "jitter")]
    fn ratelimit_sink_with_jitter<
        D: DirectStateStore,
        C: clock::ReasonablyRealtime,
        MW: RateLimitingMiddleware<C::Instant, NegativeOutcome = NotUntil<C::Instant>>,
    >(
        self,
        limiter: &'_ RateLimiter<NotKeyed, D, C, MW>,
        jitter: Jitter,
    ) -> RatelimitedSink<'_, Item, S, D, C, MW>
    where
        Self: Sized;
}

impl<Item, S: Sink<Item>> SinkRateLimitExt<Item, S> for S {
    fn ratelimit_sink<
        D: DirectStateStore,
        C: clock::ReasonablyRealtime,
        MW: RateLimitingMiddleware<C::Instant, NegativeOutcome = NotUntil<C::Instant>>,
    >(
        self,
        limiter: &RateLimiter<NotKeyed, D, C, MW>,
    ) -> RatelimitedSink<Item, S, D, C, MW>
    where
        Self: Sized,
    {
        RatelimitedSink::new(self, limiter, Jitter::NONE)
    }

    #[cfg(feature = "jitter")]
    fn ratelimit_sink_with_jitter<
        D: DirectStateStore,
        C: clock::ReasonablyRealtime,
        MW: RateLimitingMiddleware<C::Instant, NegativeOutcome = NotUntil<C::Instant>>,
    >(
        self,
        limiter: &RateLimiter<NotKeyed, D, C, MW>,
        jitter: Jitter,
    ) -> RatelimitedSink<Item, S, D, C, MW>
    where
        Self: Sized,
    {
        RatelimitedSink::new(self, limiter, jitter)
    }
}

#[derive(Debug)]
enum State {
    NotReady,
    Wait,
    Ready,
}

/// A [`Sink`][futures::Sink] combinator that only allows sending elements when the rate-limiter
/// allows it.
pub struct RatelimitedSink<
    'a,
    Item,
    S: Sink<Item>,
    D: DirectStateStore,
    C: clock::ReasonablyRealtime,
    MW: RateLimitingMiddleware<C::Instant, NegativeOutcome = NotUntil<C::Instant>>,
> {
    inner: S,
    state: State,
    limiter: &'a RateLimiter<NotKeyed, D, C, MW>,
    delay: Delay,
    jitter: Jitter,
    phantom: PhantomData<Item>,
}

/// Conversion methods for the sink combinator.
impl<
        'a,
        Item,
        S: Sink<Item>,
        D: DirectStateStore,
        C: clock::ReasonablyRealtime,
        MW: RateLimitingMiddleware<C::Instant, NegativeOutcome = NotUntil<C::Instant>>,
    > RatelimitedSink<'a, Item, S, D, C, MW>
{
    fn new(inner: S, limiter: &'a RateLimiter<NotKeyed, D, C, MW>, jitter: Jitter) -> Self {
        RatelimitedSink {
            inner,
            limiter,
            delay: Delay::new(Default::default()),
            state: State::NotReady,
            jitter,
            phantom: PhantomData,
        }
    }

    /// Acquires a reference to the underlying sink that this combinator is sending into.
    pub fn get_ref(&self) -> &S {
        &self.inner
    }

    /// Acquires a mutable reference to the underlying sink that this combinator is sending into.
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// # use futures::sink::{self, SinkExt};
    /// # use nonzero_ext::nonzero;
    /// use governor::{prelude::*, RateLimiter, Quota};
    /// let drain = sink::drain();
    /// let lim = RateLimiter::direct(Quota::per_second(nonzero!(10u32)));
    /// let mut limited = drain.ratelimit_sink(&lim);
    /// limited.get_mut().send(5).await?;
    /// # Ok::<(), futures::never::Never>(()) }).unwrap();
    /// ```
    pub fn get_mut(&mut self) -> &mut S {
        &mut self.inner
    }

    /// Consumes this combinator, returning the underlying sink.
    pub fn into_inner(self) -> S {
        self.inner
    }
}

impl<
        'a,
        Item,
        S: Sink<Item>,
        D: DirectStateStore,
        C: clock::ReasonablyRealtime,
        MW: RateLimitingMiddleware<C::Instant, NegativeOutcome = NotUntil<C::Instant>>,
    > Sink<Item> for RatelimitedSink<'a, Item, S, D, C, MW>
where
    S: Unpin,
    Item: Unpin,
{
    type Error = S::Error;

    fn poll_ready(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        loop {
            match self.state {
                State::NotReady => {
                    let reference = self.limiter.reference_reading();
                    if let Err(negative) = self.limiter.check() {
                        let earliest = negative.wait_time_with_offset(reference, self.jitter);
                        self.delay.reset(earliest);
                        let future = Pin::new(&mut self.delay);
                        match future.poll(cx) {
                            Poll::Pending => {
                                self.state = State::Wait;
                                return Poll::Pending;
                            }
                            Poll::Ready(_) => {}
                        }
                    } else {
                        self.state = State::Ready;
                    }
                }
                State::Wait => {
                    let future = Pin::new(&mut self.delay);
                    match future.poll(cx) {
                        Poll::Pending => {
                            return Poll::Pending;
                        }
                        Poll::Ready(_) => {
                            self.state = State::NotReady;
                        }
                    }
                }
                State::Ready => {
                    let inner = Pin::new(&mut self.inner);
                    return inner.poll_ready(cx);
                }
            }
        }
    }

    fn start_send(mut self: Pin<&mut Self>, item: Item) -> Result<(), Self::Error> {
        match self.state {
            State::Wait | State::NotReady => {
                unreachable!("Must not start_send before we're ready"); // !no_rcov!
            }
            State::Ready => {
                self.state = State::NotReady;
                let inner = Pin::new(&mut self.inner);
                inner.start_send(item)
            }
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let inner = Pin::new(&mut self.inner);
        inner.poll_flush(cx)
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let inner = Pin::new(&mut self.inner);
        inner.poll_close(cx)
    }
}

/// Pass-through implementation for [`futures::Stream`] if the Sink also implements it.
impl<
        'a,
        Item,
        S: Stream + Sink<Item>,
        D: DirectStateStore,
        C: clock::ReasonablyRealtime,
        MW: RateLimitingMiddleware<C::Instant, NegativeOutcome = NotUntil<C::Instant>>,
    > Stream for RatelimitedSink<'a, Item, S, D, C, MW>
where
    S::Item: Unpin,
    S: Unpin,
    Item: Unpin,
{
    type Item = <S as Stream>::Item;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let inner = Pin::new(&mut self.inner);
        inner.poll_next(cx)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}
