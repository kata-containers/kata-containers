mod util {
    use futures::future::Future;

    pub fn run<F: Future + Unpin>(mut f: F) -> F::Output {
        use futures_test::task::noop_context;
        use futures::task::Poll;
        use futures::future::FutureExt;

        let mut cx = noop_context();
        loop {
            if let Poll::Ready(x) = f.poll_unpin(&mut cx) {
                return x;
            }
        }
    }
}

#[test]
fn lines() {
    use futures::executor::block_on;
    use futures::stream::StreamExt;
    use futures::io::{AsyncBufReadExt, Cursor};

    macro_rules! block_on_next {
        ($expr:expr) => {
            block_on($expr.next()).unwrap().unwrap()
        };
    }

    let buf = Cursor::new(&b"12\r"[..]);
    let mut s = buf.lines();
    assert_eq!(block_on_next!(s), "12\r".to_string());
    assert!(block_on(s.next()).is_none());

    let buf = Cursor::new(&b"12\r\n\n"[..]);
    let mut s = buf.lines();
    assert_eq!(block_on_next!(s), "12".to_string());
    assert_eq!(block_on_next!(s), "".to_string());
    assert!(block_on(s.next()).is_none());
}

#[test]
fn maybe_pending() {
    use futures::stream::{self, StreamExt, TryStreamExt};
    use futures::io::AsyncBufReadExt;
    use futures_test::io::AsyncReadTestExt;

    use util::run;

    macro_rules! run_next {
        ($expr:expr) => {
            run($expr.next()).unwrap().unwrap()
        };
    }

    let buf = stream::iter(vec![&b"12"[..], &b"\r"[..]])
        .map(Ok)
        .into_async_read()
        .interleave_pending();
    let mut s = buf.lines();
    assert_eq!(run_next!(s), "12\r".to_string());
    assert!(run(s.next()).is_none());

    let buf = stream::iter(vec![&b"12"[..], &b"\r\n"[..], &b"\n"[..]])
        .map(Ok)
        .into_async_read()
        .interleave_pending();
    let mut s = buf.lines();
    assert_eq!(run_next!(s), "12".to_string());
    assert_eq!(run_next!(s), "".to_string());
    assert!(run(s.next()).is_none());
}
