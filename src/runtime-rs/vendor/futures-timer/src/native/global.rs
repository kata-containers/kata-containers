use std::future::Future;
use std::io;
use std::mem::{self, ManuallyDrop};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::task::{Context, RawWaker, RawWakerVTable, Waker};
use std::thread;
use std::thread::Thread;
use std::time::Instant;

use super::{Timer, TimerHandle};

pub struct HelperThread {
    thread: Option<thread::JoinHandle<()>>,
    timer: TimerHandle,
    done: Arc<AtomicBool>,
}

impl HelperThread {
    pub fn new() -> io::Result<HelperThread> {
        let timer = Timer::new();
        let timer_handle = timer.handle();
        let done = Arc::new(AtomicBool::new(false));
        let done2 = done.clone();
        let thread = thread::Builder::new()
            .name("futures-timer".to_owned())
            .spawn(move || run(timer, done2))?;

        Ok(HelperThread {
            thread: Some(thread),
            done,
            timer: timer_handle,
        })
    }

    pub fn handle(&self) -> TimerHandle {
        self.timer.clone()
    }

    pub fn forget(mut self) {
        self.thread.take();
    }
}

impl Drop for HelperThread {
    fn drop(&mut self) {
        let thread = match self.thread.take() {
            Some(thread) => thread,
            None => return,
        };
        self.done.store(true, Ordering::SeqCst);
        thread.thread().unpark();
        drop(thread.join());
    }
}

fn run(mut timer: Timer, done: Arc<AtomicBool>) {
    let waker = current_thread_waker();
    let mut cx = Context::from_waker(&waker);

    while !done.load(Ordering::SeqCst) {
        let _ = Pin::new(&mut timer).poll(&mut cx);

        timer.advance();
        match timer.next_event() {
            // Ok, block for the specified time
            Some(when) => {
                let now = Instant::now();
                if now < when {
                    thread::park_timeout(when - now)
                } else {
                    // .. continue...
                }
            }

            // Just wait for one of our futures to wake up
            None => thread::park(),
        }
    }
}

static VTABLE: RawWakerVTable = RawWakerVTable::new(raw_clone, raw_wake, raw_wake_by_ref, raw_drop);

fn raw_clone(ptr: *const ()) -> RawWaker {
    let me = ManuallyDrop::new(unsafe { Arc::from_raw(ptr as *const Thread) });
    mem::forget(me.clone());
    RawWaker::new(ptr, &VTABLE)
}

fn raw_wake(ptr: *const ()) {
    unsafe { Arc::from_raw(ptr as *const Thread) }.unpark()
}

fn raw_wake_by_ref(ptr: *const ()) {
    ManuallyDrop::new(unsafe { Arc::from_raw(ptr as *const Thread) }).unpark()
}

fn raw_drop(ptr: *const ()) {
    unsafe { Arc::from_raw(ptr as *const Thread) };
}

fn current_thread_waker() -> Waker {
    let thread = Arc::new(thread::current());
    unsafe { Waker::from_raw(RawWaker::new(Arc::into_raw(thread) as *const (), &VTABLE)) }
}
