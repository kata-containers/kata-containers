use core::pin::Pin;
use futures_core::future::Future;
use futures_core::task::{Context, Poll};
use futures_sink::Sink;

/// Future for the [`send`](super::SinkExt::send) method.
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct Send<'a, Si: ?Sized, Item> {
    sink: &'a mut Si,
    item: Option<Item>,
}

// Pinning is never projected to children
impl<Si: Unpin + ?Sized, Item> Unpin for Send<'_, Si, Item> {}

impl<'a, Si: Sink<Item> + Unpin + ?Sized, Item> Send<'a, Si, Item> {
    pub(super) fn new(sink: &'a mut Si, item: Item) -> Self {
        Self {
            sink,
            item: Some(item),
        }
    }
}

impl<Si: Sink<Item> + Unpin + ?Sized, Item> Future for Send<'_, Si, Item> {
    type Output = Result<(), Si::Error>;

    fn poll(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Self::Output> {
        let this = &mut *self;
        if let Some(item) = this.item.take() {
            let mut sink = Pin::new(&mut this.sink);
            match sink.as_mut().poll_ready(cx)? {
                Poll::Ready(()) => sink.as_mut().start_send(item)?,
                Poll::Pending => {
                    this.item = Some(item);
                    return Poll::Pending;
                }
            }
        }

        // we're done sending the item, but want to block on flushing the
        // sink
        ready!(Pin::new(&mut this.sink).poll_flush(cx))?;

        Poll::Ready(Ok(()))
    }
}
