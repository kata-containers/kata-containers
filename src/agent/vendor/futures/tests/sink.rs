mod sassert_next {
    use futures::stream::{Stream, StreamExt};
    use futures::task::Poll;
    use futures_test::task::panic_context;
    use std::fmt;

    pub fn sassert_next<S>(s: &mut S, item: S::Item)
    where
        S: Stream + Unpin,
        S::Item: Eq + fmt::Debug,
    {
        match s.poll_next_unpin(&mut panic_context()) {
            Poll::Ready(None) => panic!("stream is at its end"),
            Poll::Ready(Some(e)) => assert_eq!(e, item),
            Poll::Pending => panic!("stream wasn't ready"),
        }
    }
}

mod unwrap {
    use futures::task::Poll;
    use std::fmt;

    pub fn unwrap<T, E: fmt::Debug>(x: Poll<Result<T, E>>) -> T {
        match x {
            Poll::Ready(Ok(x)) => x,
            Poll::Ready(Err(_)) => panic!("Poll::Ready(Err(_))"),
            Poll::Pending => panic!("Poll::Pending"),
        }
    }
}

mod flag_cx {
    use futures::task::{self, ArcWake, Context};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    // An Unpark struct that records unpark events for inspection
    pub struct Flag(AtomicBool);

    impl Flag {
        pub fn new() -> Arc<Self> {
            Arc::new(Self(AtomicBool::new(false)))
        }

        pub fn take(&self) -> bool {
            self.0.swap(false, Ordering::SeqCst)
        }

        pub fn set(&self, v: bool) {
            self.0.store(v, Ordering::SeqCst)
        }
    }

    impl ArcWake for Flag {
        fn wake_by_ref(arc_self: &Arc<Self>) {
            arc_self.set(true)
        }
    }

    pub fn flag_cx<F, R>(f: F) -> R
    where
        F: FnOnce(Arc<Flag>, &mut Context<'_>) -> R,
    {
        let flag = Flag::new();
        let waker = task::waker_ref(&flag);
        let cx = &mut Context::from_waker(&waker);
        f(flag.clone(), cx)
    }
}

mod start_send_fut {
    use futures::future::Future;
    use futures::ready;
    use futures::sink::Sink;
    use futures::task::{Context, Poll};
    use std::pin::Pin;

    // Sends a value on an i32 channel sink
    pub struct StartSendFut<S: Sink<Item> + Unpin, Item: Unpin>(Option<S>, Option<Item>);

    impl<S: Sink<Item> + Unpin, Item: Unpin> StartSendFut<S, Item> {
        pub fn new(sink: S, item: Item) -> Self {
            Self(Some(sink), Some(item))
        }
    }

    impl<S: Sink<Item> + Unpin, Item: Unpin> Future for StartSendFut<S, Item> {
        type Output = Result<S, S::Error>;

        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            let Self(inner, item) = self.get_mut();
            {
                let mut inner = inner.as_mut().unwrap();
                ready!(Pin::new(&mut inner).poll_ready(cx))?;
                Pin::new(&mut inner).start_send(item.take().unwrap())?;
            }
            Poll::Ready(Ok(inner.take().unwrap()))
        }
    }
}

mod manual_flush {
    use futures::sink::Sink;
    use futures::task::{Context, Poll, Waker};
    use std::mem;
    use std::pin::Pin;

    // Immediately accepts all requests to start pushing, but completion is managed
    // by manually flushing
    pub struct ManualFlush<T: Unpin> {
        data: Vec<T>,
        waiting_tasks: Vec<Waker>,
    }

    impl<T: Unpin> Sink<Option<T>> for ManualFlush<T> {
        type Error = ();

        fn poll_ready(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn start_send(mut self: Pin<&mut Self>, item: Option<T>) -> Result<(), Self::Error> {
            if let Some(item) = item {
                self.data.push(item);
            } else {
                self.force_flush();
            }
            Ok(())
        }

        fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            if self.data.is_empty() {
                Poll::Ready(Ok(()))
            } else {
                self.waiting_tasks.push(cx.waker().clone());
                Poll::Pending
            }
        }

        fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            self.poll_flush(cx)
        }
    }

    impl<T: Unpin> ManualFlush<T> {
        pub fn new() -> Self {
            Self {
                data: Vec::new(),
                waiting_tasks: Vec::new(),
            }
        }

        pub fn force_flush(&mut self) -> Vec<T> {
            for task in self.waiting_tasks.drain(..) {
                task.wake()
            }
            mem::replace(&mut self.data, Vec::new())
        }
    }
}

mod allowance {
    use futures::sink::Sink;
    use futures::task::{Context, Poll, Waker};
    use std::cell::{Cell, RefCell};
    use std::pin::Pin;
    use std::rc::Rc;

    pub struct ManualAllow<T: Unpin> {
        pub data: Vec<T>,
        allow: Rc<Allow>,
    }

    pub struct Allow {
        flag: Cell<bool>,
        tasks: RefCell<Vec<Waker>>,
    }

    impl Allow {
        pub fn new() -> Self {
            Self {
                flag: Cell::new(false),
                tasks: RefCell::new(Vec::new()),
            }
        }

        pub fn check(&self, cx: &mut Context<'_>) -> bool {
            if self.flag.get() {
                true
            } else {
                self.tasks.borrow_mut().push(cx.waker().clone());
                false
            }
        }

        pub fn start(&self) {
            self.flag.set(true);
            let mut tasks = self.tasks.borrow_mut();
            for task in tasks.drain(..) {
                task.wake();
            }
        }
    }

    impl<T: Unpin> Sink<T> for ManualAllow<T> {
        type Error = ();

        fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            if self.allow.check(cx) {
                Poll::Ready(Ok(()))
            } else {
                Poll::Pending
            }
        }

        fn start_send(mut self: Pin<&mut Self>, item: T) -> Result<(), Self::Error> {
            self.data.push(item);
            Ok(())
        }

        fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn poll_close(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }
    }

    pub fn manual_allow<T: Unpin>() -> (ManualAllow<T>, Rc<Allow>) {
        let allow = Rc::new(Allow::new());
        let manual_allow = ManualAllow {
            data: Vec::new(),
            allow: allow.clone(),
        };
        (manual_allow, allow)
    }
}

#[test]
fn either_sink() {
    use futures::sink::{Sink, SinkExt};
    use std::collections::VecDeque;
    use std::pin::Pin;

    let mut s = if true {
        Vec::<i32>::new().left_sink()
    } else {
        VecDeque::<i32>::new().right_sink()
    };

    Pin::new(&mut s).start_send(0).unwrap();
}

#[test]
fn vec_sink() {
    use futures::executor::block_on;
    use futures::sink::{Sink, SinkExt};
    use std::pin::Pin;

    let mut v = Vec::new();
    Pin::new(&mut v).start_send(0).unwrap();
    Pin::new(&mut v).start_send(1).unwrap();
    assert_eq!(v, vec![0, 1]);
    block_on(v.flush()).unwrap();
    assert_eq!(v, vec![0, 1]);
}

#[test]
fn vecdeque_sink() {
    use futures::sink::Sink;
    use std::collections::VecDeque;
    use std::pin::Pin;

    let mut deque = VecDeque::new();
    Pin::new(&mut deque).start_send(2).unwrap();
    Pin::new(&mut deque).start_send(3).unwrap();

    assert_eq!(deque.pop_front(), Some(2));
    assert_eq!(deque.pop_front(), Some(3));
    assert_eq!(deque.pop_front(), None);
}

#[test]
fn send() {
    use futures::executor::block_on;
    use futures::sink::SinkExt;

    let mut v = Vec::new();

    block_on(v.send(0)).unwrap();
    assert_eq!(v, vec![0]);

    block_on(v.send(1)).unwrap();
    assert_eq!(v, vec![0, 1]);

    block_on(v.send(2)).unwrap();
    assert_eq!(v, vec![0, 1, 2]);
}

#[test]
fn send_all() {
    use futures::executor::block_on;
    use futures::sink::SinkExt;
    use futures::stream::{self, StreamExt};

    let mut v = Vec::new();

    block_on(v.send_all(&mut stream::iter(vec![0, 1]).map(Ok))).unwrap();
    assert_eq!(v, vec![0, 1]);

    block_on(v.send_all(&mut stream::iter(vec![2, 3]).map(Ok))).unwrap();
    assert_eq!(v, vec![0, 1, 2, 3]);

    block_on(v.send_all(&mut stream::iter(vec![4, 5]).map(Ok))).unwrap();
    assert_eq!(v, vec![0, 1, 2, 3, 4, 5]);
}

// Test that `start_send` on an `mpsc` channel does indeed block when the
// channel is full
#[test]
fn mpsc_blocking_start_send() {
    use futures::channel::mpsc;
    use futures::executor::block_on;
    use futures::future::{self, FutureExt};

    use start_send_fut::StartSendFut;
    use flag_cx::flag_cx;
    use sassert_next::sassert_next;
    use unwrap::unwrap;

    let (mut tx, mut rx) = mpsc::channel::<i32>(0);

    block_on(future::lazy(|_| {
        tx.start_send(0).unwrap();

        flag_cx(|flag, cx| {
            let mut task = StartSendFut::new(tx, 1);

            assert!(task.poll_unpin(cx).is_pending());
            assert!(!flag.take());
            sassert_next(&mut rx, 0);
            assert!(flag.take());
            unwrap(task.poll_unpin(cx));
            assert!(!flag.take());
            sassert_next(&mut rx, 1);
        })
    }));
}

// test `flush` by using `with` to make the first insertion into a sink block
// until a oneshot is completed
#[test]
fn with_flush() {
    use futures::channel::oneshot;
    use futures::executor::block_on;
    use futures::future::{self, FutureExt, TryFutureExt};
    use futures::never::Never;
    use futures::sink::{Sink, SinkExt};
    use std::mem;
    use std::pin::Pin;

    use flag_cx::flag_cx;
    use unwrap::unwrap;

    let (tx, rx) = oneshot::channel();
    let mut block = rx.boxed();
    let mut sink = Vec::new().with(|elem| {
        mem::replace(&mut block, future::ok(()).boxed())
            .map_ok(move |()| elem + 1)
            .map_err(|_| -> Never { panic!() })
    });

    assert_eq!(Pin::new(&mut sink).start_send(0).ok(), Some(()));

    flag_cx(|flag, cx| {
        let mut task = sink.flush();
        assert!(task.poll_unpin(cx).is_pending());
        tx.send(()).unwrap();
        assert!(flag.take());

        unwrap(task.poll_unpin(cx));

        block_on(sink.send(1)).unwrap();
        assert_eq!(sink.get_ref(), &[1, 2]);
    })
}

// test simple use of with to change data
#[test]
fn with_as_map() {
    use futures::executor::block_on;
    use futures::future;
    use futures::never::Never;
    use futures::sink::SinkExt;

    let mut sink = Vec::new().with(|item| future::ok::<i32, Never>(item * 2));
    block_on(sink.send(0)).unwrap();
    block_on(sink.send(1)).unwrap();
    block_on(sink.send(2)).unwrap();
    assert_eq!(sink.get_ref(), &[0, 2, 4]);
}

// test simple use of with_flat_map
#[test]
fn with_flat_map() {
    use futures::executor::block_on;
    use futures::sink::SinkExt;
    use futures::stream::{self, StreamExt};

    let mut sink = Vec::new().with_flat_map(|item| stream::iter(vec![item; item]).map(Ok));
    block_on(sink.send(0)).unwrap();
    block_on(sink.send(1)).unwrap();
    block_on(sink.send(2)).unwrap();
    block_on(sink.send(3)).unwrap();
    assert_eq!(sink.get_ref(), &[1, 2, 2, 3, 3, 3]);
}

// Check that `with` propagates `poll_ready` to the inner sink.
// Regression test for the issue #1834.
#[test]
fn with_propagates_poll_ready() {
    use futures::channel::mpsc;
    use futures::executor::block_on;
    use futures::future;
    use futures::sink::{Sink, SinkExt};
    use futures::task::Poll;
    use std::pin::Pin;

    use flag_cx::flag_cx;
    use sassert_next::sassert_next;

    let (tx, mut rx) = mpsc::channel::<i32>(0);
    let mut tx = tx.with(|item: i32| future::ok::<i32, mpsc::SendError>(item + 10));

    block_on(future::lazy(|_| {
        flag_cx(|flag, cx| {
            let mut tx = Pin::new(&mut tx);

            // Should be ready for the first item.
            assert_eq!(tx.as_mut().poll_ready(cx), Poll::Ready(Ok(())));
            assert_eq!(tx.as_mut().start_send(0), Ok(()));

            // Should be ready for the second item only after the first one is received.
            assert_eq!(tx.as_mut().poll_ready(cx), Poll::Pending);
            assert!(!flag.take());
            sassert_next(&mut rx, 10);
            assert!(flag.take());
            assert_eq!(tx.as_mut().poll_ready(cx), Poll::Ready(Ok(())));
            assert_eq!(tx.as_mut().start_send(1), Ok(()));
        })
    }));
}

// test that the `with` sink doesn't require the underlying sink to flush,
// but doesn't claim to be flushed until the underlying sink is
#[test]
fn with_flush_propagate() {
    use futures::future::{self, FutureExt};
    use futures::sink::{Sink, SinkExt};
    use std::pin::Pin;

    use manual_flush::ManualFlush;
    use flag_cx::flag_cx;
    use unwrap::unwrap;

    let mut sink = ManualFlush::new().with(future::ok::<Option<i32>, ()>);
    flag_cx(|flag, cx| {
        unwrap(Pin::new(&mut sink).poll_ready(cx));
        Pin::new(&mut sink).start_send(Some(0)).unwrap();
        unwrap(Pin::new(&mut sink).poll_ready(cx));
        Pin::new(&mut sink).start_send(Some(1)).unwrap();

        {
            let mut task = sink.flush();
            assert!(task.poll_unpin(cx).is_pending());
            assert!(!flag.take());
        }
        assert_eq!(sink.get_mut().force_flush(), vec![0, 1]);
        assert!(flag.take());
        unwrap(sink.flush().poll_unpin(cx));
    })
}

// test that a buffer is a no-nop around a sink that always accepts sends
#[test]
fn buffer_noop() {
    use futures::executor::block_on;
    use futures::sink::SinkExt;

    let mut sink = Vec::new().buffer(0);
    block_on(sink.send(0)).unwrap();
    block_on(sink.send(1)).unwrap();
    assert_eq!(sink.get_ref(), &[0, 1]);

    let mut sink = Vec::new().buffer(1);
    block_on(sink.send(0)).unwrap();
    block_on(sink.send(1)).unwrap();
    assert_eq!(sink.get_ref(), &[0, 1]);
}

// test basic buffer functionality, including both filling up to capacity,
// and writing out when the underlying sink is ready
#[test]
fn buffer() {
    use futures::executor::block_on;
    use futures::future::FutureExt;
    use futures::sink::SinkExt;

    use start_send_fut::StartSendFut;
    use flag_cx::flag_cx;
    use unwrap::unwrap;
    use allowance::manual_allow;

    let (sink, allow) = manual_allow::<i32>();
    let sink = sink.buffer(2);

    let sink = block_on(StartSendFut::new(sink, 0)).unwrap();
    let mut sink = block_on(StartSendFut::new(sink, 1)).unwrap();

    flag_cx(|flag, cx| {
        let mut task = sink.send(2);
        assert!(task.poll_unpin(cx).is_pending());
        assert!(!flag.take());
        allow.start();
        assert!(flag.take());
        unwrap(task.poll_unpin(cx));
        assert_eq!(sink.get_ref().data, vec![0, 1, 2]);
    })
}

#[test]
fn fanout_smoke() {
    use futures::executor::block_on;
    use futures::sink::SinkExt;
    use futures::stream::{self, StreamExt};

    let sink1 = Vec::new();
    let sink2 = Vec::new();
    let mut sink = sink1.fanout(sink2);
    block_on(sink.send_all(&mut stream::iter(vec![1, 2, 3]).map(Ok))).unwrap();
    let (sink1, sink2) = sink.into_inner();
    assert_eq!(sink1, vec![1, 2, 3]);
    assert_eq!(sink2, vec![1, 2, 3]);
}

#[test]
fn fanout_backpressure() {
    use futures::channel::mpsc;
    use futures::executor::block_on;
    use futures::future::FutureExt;
    use futures::sink::SinkExt;
    use futures::stream::StreamExt;

    use start_send_fut::StartSendFut;
    use flag_cx::flag_cx;
    use unwrap::unwrap;

    let (left_send, mut left_recv) = mpsc::channel(0);
    let (right_send, mut right_recv) = mpsc::channel(0);
    let sink = left_send.fanout(right_send);

    let mut sink = block_on(StartSendFut::new(sink, 0)).unwrap();

    flag_cx(|flag, cx| {
        let mut task = sink.send(2);
        assert!(!flag.take());
        assert!(task.poll_unpin(cx).is_pending());
        assert_eq!(block_on(left_recv.next()), Some(0));
        assert!(flag.take());
        assert!(task.poll_unpin(cx).is_pending());
        assert_eq!(block_on(right_recv.next()), Some(0));
        assert!(flag.take());

        assert!(task.poll_unpin(cx).is_pending());
        assert_eq!(block_on(left_recv.next()), Some(2));
        assert!(flag.take());
        assert!(task.poll_unpin(cx).is_pending());
        assert_eq!(block_on(right_recv.next()), Some(2));
        assert!(flag.take());

        unwrap(task.poll_unpin(cx));
        // make sure receivers live until end of test to prevent send errors
        drop(left_recv);
        drop(right_recv);
    })
}

#[test]
fn sink_map_err() {
    use futures::channel::mpsc;
    use futures::sink::{Sink, SinkExt};
    use futures::task::Poll;
    use futures_test::task::panic_context;
    use std::pin::Pin;

    {
        let cx = &mut panic_context();
        let (tx, _rx) = mpsc::channel(1);
        let mut tx = tx.sink_map_err(|_| ());
        assert_eq!(Pin::new(&mut tx).start_send(()), Ok(()));
        assert_eq!(Pin::new(&mut tx).poll_flush(cx), Poll::Ready(Ok(())));
    }

    let tx = mpsc::channel(0).0;
    assert_eq!(
        Pin::new(&mut tx.sink_map_err(|_| ())).start_send(()),
        Err(())
    );
}

#[test]
fn err_into() {
    use futures::channel::mpsc;
    use futures::sink::{Sink, SinkErrInto, SinkExt};
    use futures::task::Poll;
    use futures_test::task::panic_context;
    use std::pin::Pin;

    #[derive(Copy, Clone, Debug, PartialEq, Eq)]
    pub struct ErrIntoTest;

    impl From<mpsc::SendError> for ErrIntoTest {
        fn from(_: mpsc::SendError) -> Self {
            Self
        }
    }

    {
        let cx = &mut panic_context();
        let (tx, _rx) = mpsc::channel(1);
        let mut tx: SinkErrInto<mpsc::Sender<()>, _, ErrIntoTest> = tx.sink_err_into();
        assert_eq!(Pin::new(&mut tx).start_send(()), Ok(()));
        assert_eq!(Pin::new(&mut tx).poll_flush(cx), Poll::Ready(Ok(())));
    }

    let tx = mpsc::channel(0).0;
    assert_eq!(
        Pin::new(&mut tx.sink_err_into()).start_send(()),
        Err(ErrIntoTest)
    );
}
