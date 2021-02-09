#[test]
fn ready() {
    use futures::{
        executor::block_on,
        future,
        task::Poll,
        ready,
    };

    block_on(future::poll_fn(|_| {
        ready!(Poll::Ready(()),);
        Poll::Ready(())
    }))
}

#[test]
fn poll() {
    use futures::{
        executor::block_on,
        future::FutureExt,
        poll,
    };

    block_on(async {
        let _ = poll!(async {}.boxed(),);
    })
}

#[test]
fn join() {
    use futures::{
        executor::block_on,
        join
    };

    block_on(async {
        let future1 = async { 1 };
        let future2 = async { 2 };
        join!(future1, future2,);
    })
}

#[test]
fn try_join() {
    use futures::{
        executor::block_on,
        future::FutureExt,
        try_join,
    };

    block_on(async {
        let future1 = async { 1 }.never_error();
        let future2 = async { 2 }.never_error();
        try_join!(future1, future2,)
    })
    .unwrap();
}
