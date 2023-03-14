use std::cell::Cell;
use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::ptr;
use std::task::{Context, Poll};

#[derive(Debug)]
pub struct Sender<T> {
    _p: PhantomData<T>,
}

#[derive(Debug)]
pub struct Receiver<T> {
    _p: PhantomData<T>,
}

pub(crate) struct Enter<'a, T> {
    _rx: &'a mut Receiver<T>,
    prev: *mut (),
}

pub fn pair<T>() -> (Sender<T>, Receiver<T>) {
    let tx = Sender { _p: PhantomData };
    let rx = Receiver { _p: PhantomData };
    (tx, rx)
}

// Tracks the pointer to `Option<T>`.
//
// TODO: Ensure wakers match?
thread_local!(static STORE: Cell<*mut ()> = Cell::new(ptr::null_mut()));

// ===== impl Sender =====

impl<T> Sender<T> {
    pub fn send(&mut self, value: T) -> impl Future<Output = ()> {
        Send { value: Some(value) }
    }
}

struct Send<T> {
    value: Option<T>,
}

impl<T> Unpin for Send<T> {}

impl<T> Future for Send<T> {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<()> {
        if self.value.is_none() {
            return Poll::Ready(());
        }

        STORE.with(|cell| unsafe {
            let ptr = cell.get() as *mut Option<T>;
            let option_ref = ptr.as_mut().expect("invalid usage");

            if option_ref.is_none() {
                *option_ref = self.value.take();
            }

            Poll::Pending
        })
    }
}

// ===== impl Receiver =====

impl<T> Receiver<T> {
    pub(crate) fn enter<'a>(&'a mut self, dst: &'a mut Option<T>) -> Enter<'a, T> {
        let prev = STORE.with(|cell| {
            let prev = cell.get();
            cell.set(dst as *mut _ as *mut ());
            prev
        });

        Enter { _rx: self, prev }
    }
}

// ===== impl Enter =====

impl<'a, T> Drop for Enter<'a, T> {
    fn drop(&mut self) {
        STORE.with(|cell| cell.set(self.prev));
    }
}
