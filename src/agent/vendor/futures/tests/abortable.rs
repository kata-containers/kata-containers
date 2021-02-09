#[test]
fn abortable_works() {
    use futures::channel::oneshot;
    use futures::future::{abortable, Aborted};
    use futures::executor::block_on;

    let (_tx, a_rx) = oneshot::channel::<()>();
    let (abortable_rx, abort_handle) = abortable(a_rx);

    abort_handle.abort();
    assert_eq!(Err(Aborted), block_on(abortable_rx));
}

#[test]
fn abortable_awakens() {
    use futures::channel::oneshot;
    use futures::future::{abortable, Aborted, FutureExt};
    use futures::task::{Context, Poll};
    use futures_test::task::new_count_waker;

    let (_tx, a_rx) = oneshot::channel::<()>();
    let (mut abortable_rx, abort_handle) = abortable(a_rx);

    let (waker, counter) = new_count_waker();
    let mut cx = Context::from_waker(&waker);
    assert_eq!(counter, 0);
    assert_eq!(Poll::Pending, abortable_rx.poll_unpin(&mut cx));
    assert_eq!(counter, 0);
    abort_handle.abort();
    assert_eq!(counter, 1);
    assert_eq!(Poll::Ready(Err(Aborted)), abortable_rx.poll_unpin(&mut cx));
}

#[test]
fn abortable_resolves() {
    use futures::channel::oneshot;
    use futures::future::abortable;
    use futures::executor::block_on;
    let (tx, a_rx) = oneshot::channel::<()>();
    let (abortable_rx, _abort_handle) = abortable(a_rx);

    tx.send(()).unwrap();

    assert_eq!(Ok(Ok(())), block_on(abortable_rx));
}
