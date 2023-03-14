// this is the only way to make the path attribute play nice with `in
// crate::support`...
#[allow(clippy::module_inception)]
#[path = "../../tracing/tests/support/mod.rs"]
mod support;
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
pub use support::*;
use tokio_test::task;

pub struct PollN<T, E> {
    and_return: Option<Result<T, E>>,
    finish_at: usize,
    polls: usize,
}

impl<T, E> std::future::Future for PollN<T, E>
where
    T: Unpin,
    E: Unpin,
{
    type Output = Result<T, E>;
    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        let this = self.get_mut();

        this.polls += 1;
        if this.polls == this.finish_at {
            let value = this.and_return.take().expect("polled after ready");

            Poll::Ready(value)
        } else {
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    }
}

impl PollN<(), ()> {
    pub fn new_ok(finish_at: usize) -> Self {
        Self {
            and_return: Some(Ok(())),
            finish_at,
            polls: 0,
        }
    }

    pub fn new_err(finish_at: usize) -> Self {
        Self {
            and_return: Some(Err(())),
            finish_at,
            polls: 0,
        }
    }
}

pub fn block_on_future<F>(future: F) -> F::Output
where
    F: Future,
{
    let mut task = task::spawn(future);
    loop {
        if let Poll::Ready(v) = task.poll() {
            break v;
        }
    }
}
