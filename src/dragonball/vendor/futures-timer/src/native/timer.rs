use std::fmt;
use std::mem;
use std::pin::Pin;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering::SeqCst;
use std::sync::{Arc, Mutex, Weak};
use std::task::{Context, Poll};
use std::time::Instant;

use std::future::Future;

use super::AtomicWaker;
use super::{global, ArcList, Heap, HeapTimer, Node, Slot};

/// A "timer heap" used to power separately owned instances of `Delay`.
///
/// This timer is implemented as a priority queued-based heap. Each `Timer`
/// contains a few primary methods which which to drive it:
///
/// * `next_wake` indicates how long the ambient system needs to sleep until it
///   invokes further processing on a `Timer`
/// * `advance_to` is what actually fires timers on the `Timer`, and should be
///   called essentially every iteration of the event loop, or when the time
///   specified by `next_wake` has elapsed.
/// * The `Future` implementation for `Timer` is used to process incoming timer
///   updates and requests. This is used to schedule new timeouts, update
///   existing ones, or delete existing timeouts. The `Future` implementation
///   will never resolve, but it'll schedule notifications of when to wake up
///   and process more messages.
///
/// Note that if you're using this crate you probably don't need to use a
/// `Timer` as there is a global one already available for you run on a helper
/// thread. If this isn't desirable, though, then the
/// `TimerHandle::set_fallback` method can be used instead!
pub struct Timer {
    inner: Arc<Inner>,
    timer_heap: Heap<HeapTimer>,
}

/// A handle to a `Timer` which is used to create instances of a `Delay`.
#[derive(Clone)]
pub struct TimerHandle {
    pub(crate) inner: Weak<Inner>,
}

pub(crate) struct Inner {
    /// List of updates the `Timer` needs to process
    pub(crate) list: ArcList<ScheduledTimer>,

    /// The blocked `Timer` task to receive notifications to the `list` above.
    pub(crate) waker: AtomicWaker,
}

/// Shared state between the `Timer` and a `Delay`.
pub(crate) struct ScheduledTimer {
    pub(crate) waker: AtomicWaker,

    // The lowest bit here is whether the timer has fired or not, the second
    // lowest bit is whether the timer has been invalidated, and all the other
    // bits are the "generation" of the timer which is reset during the `reset`
    // function. Only timers for a matching generation are fired.
    pub(crate) state: AtomicUsize,

    pub(crate) inner: Weak<Inner>,
    pub(crate) at: Mutex<Option<Instant>>,

    // TODO: this is only accessed by the timer thread, should have a more
    // lightweight protection than a `Mutex`
    pub(crate) slot: Mutex<Option<Slot>>,
}

impl Timer {
    /// Creates a new timer heap ready to create new timers.
    pub fn new() -> Timer {
        Timer {
            inner: Arc::new(Inner {
                list: ArcList::new(),
                waker: AtomicWaker::new(),
            }),
            timer_heap: Heap::new(),
        }
    }

    /// Returns a handle to this timer heap, used to create new timeouts.
    pub fn handle(&self) -> TimerHandle {
        TimerHandle {
            inner: Arc::downgrade(&self.inner),
        }
    }

    /// Returns the time at which this timer next needs to be invoked with
    /// `advance_to`.
    ///
    /// Event loops or threads typically want to sleep until the specified
    /// instant.
    pub fn next_event(&self) -> Option<Instant> {
        self.timer_heap.peek().map(|t| t.at)
    }

    /// Proces any timers which are supposed to fire at or before the current
    /// instant.
    ///
    /// This method is equivalent to `self.advance_to(Instant::now())`.
    pub fn advance(&mut self) {
        self.advance_to(Instant::now())
    }

    /// Proces any timers which are supposed to fire before `now` specified.
    ///
    /// This method should be called on `Timer` periodically to advance the
    /// internal state and process any pending timers which need to fire.
    pub fn advance_to(&mut self, now: Instant) {
        loop {
            match self.timer_heap.peek() {
                Some(head) if head.at <= now => {}
                Some(_) => break,
                None => break,
            };

            // Flag the timer as fired and then notify its task, if any, that's
            // blocked.
            let heap_timer = self.timer_heap.pop().unwrap();
            *heap_timer.node.slot.lock().unwrap() = None;
            let bits = heap_timer.gen << 2;
            match heap_timer
                .node
                .state
                .compare_exchange(bits, bits | 0b01, SeqCst, SeqCst)
            {
                Ok(_) => heap_timer.node.waker.wake(),
                Err(_b) => {}
            }
        }
    }

    /// Either updates the timer at slot `idx` to fire at `at`, or adds a new
    /// timer at `idx` and sets it to fire at `at`.
    fn update_or_add(&mut self, at: Instant, node: Arc<Node<ScheduledTimer>>) {
        // TODO: avoid remove + push and instead just do one sift of the heap?
        // In theory we could update it in place and then do the percolation
        // as necessary
        let gen = node.state.load(SeqCst) >> 2;
        let mut slot = node.slot.lock().unwrap();
        if let Some(heap_slot) = slot.take() {
            self.timer_heap.remove(heap_slot);
        }
        *slot = Some(self.timer_heap.push(HeapTimer {
            at,
            gen,
            node: node.clone(),
        }));
    }

    fn remove(&mut self, node: Arc<Node<ScheduledTimer>>) {
        // If this `idx` is still around and it's still got a registered timer,
        // then we jettison it form the timer heap.
        let mut slot = node.slot.lock().unwrap();
        let heap_slot = match slot.take() {
            Some(slot) => slot,
            None => return,
        };
        self.timer_heap.remove(heap_slot);
    }

    fn invalidate(&mut self, node: Arc<Node<ScheduledTimer>>) {
        node.state.fetch_or(0b10, SeqCst);
        node.waker.wake();
    }
}

impl Future for Timer {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut self.inner).waker.register(cx.waker());
        let mut list = self.inner.list.take();
        while let Some(node) = list.pop() {
            let at = *node.at.lock().unwrap();
            match at {
                Some(at) => self.update_or_add(at, node),
                None => self.remove(node),
            }
        }
        Poll::Pending
    }
}

impl Drop for Timer {
    fn drop(&mut self) {
        // Seal off our list to prevent any more updates from getting pushed on.
        // Any timer which sees an error from the push will immediately become
        // inert.
        let mut list = self.inner.list.take_and_seal();

        // Now that we'll never receive another timer, drain the list of all
        // updates and also drain our heap of all active timers, invalidating
        // everything.
        while let Some(t) = list.pop() {
            self.invalidate(t);
        }
        while let Some(t) = self.timer_heap.pop() {
            self.invalidate(t.node);
        }
    }
}

impl fmt::Debug for Timer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        f.debug_struct("Timer").field("heap", &"...").finish()
    }
}

impl Default for Timer {
    fn default() -> Self {
        Self::new()
    }
}

static HANDLE_FALLBACK: AtomicUsize = AtomicUsize::new(0);

/// Error returned from `TimerHandle::set_fallback`.
#[derive(Clone, Debug)]
struct SetDefaultError(());

impl TimerHandle {
    /// Configures this timer handle to be the one returned by
    /// `TimerHandle::default`.
    ///
    /// By default a global thread is initialized on the first call to
    /// `TimerHandle::default`. This first call can happen transitively through
    /// `Delay::new`. If, however, that hasn't happened yet then the global
    /// default timer handle can be configured through this method.
    ///
    /// This method can be used to prevent the global helper thread from
    /// spawning. If this method is successful then the global helper thread
    /// will never get spun up.
    ///
    /// On success this timer handle will have installed itself globally to be
    /// used as the return value for `TimerHandle::default` unless otherwise
    /// specified.
    ///
    /// # Errors
    ///
    /// If another thread has already called `set_as_global_fallback` or this
    /// thread otherwise loses a race to call this method then it will fail
    /// returning an error. Once a call to `set_as_global_fallback` is
    /// successful then no future calls may succeed.
    fn set_as_global_fallback(self) -> Result<(), SetDefaultError> {
        unsafe {
            let val = self.into_usize();
            match HANDLE_FALLBACK.compare_exchange(0, val, SeqCst, SeqCst) {
                Ok(_) => Ok(()),
                Err(_) => {
                    drop(TimerHandle::from_usize(val));
                    Err(SetDefaultError(()))
                }
            }
        }
    }

    fn into_usize(self) -> usize {
        unsafe { mem::transmute::<Weak<Inner>, usize>(self.inner) }
    }

    unsafe fn from_usize(val: usize) -> TimerHandle {
        let inner = mem::transmute::<usize, Weak<Inner>>(val);
        TimerHandle { inner }
    }
}

impl Default for TimerHandle {
    fn default() -> TimerHandle {
        let mut fallback = HANDLE_FALLBACK.load(SeqCst);

        // If the fallback hasn't been previously initialized then let's spin
        // up a helper thread and try to initialize with that. If we can't
        // actually create a helper thread then we'll just return a "defunkt"
        // handle which will return errors when timer objects are attempted to
        // be associated.
        if fallback == 0 {
            let helper = match global::HelperThread::new() {
                Ok(helper) => helper,
                Err(_) => return TimerHandle { inner: Weak::new() },
            };

            // If we successfully set ourselves as the actual fallback then we
            // want to `forget` the helper thread to ensure that it persists
            // globally. If we fail to set ourselves as the fallback that means
            // that someone was racing with this call to
            // `TimerHandle::default`.  They ended up winning so we'll destroy
            // our helper thread (which shuts down the thread) and reload the
            // fallback.
            if helper.handle().set_as_global_fallback().is_ok() {
                let ret = helper.handle();
                helper.forget();
                return ret;
            }
            fallback = HANDLE_FALLBACK.load(SeqCst);
        }

        // At this point our fallback handle global was configured so we use
        // its value to reify a handle, clone it, and then forget our reified
        // handle as we don't actually have an owning reference to it.
        assert!(fallback != 0);
        unsafe {
            let handle = TimerHandle::from_usize(fallback);
            let ret = handle.clone();
            let _ = handle.into_usize();
            ret
        }
    }
}

impl fmt::Debug for TimerHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        f.debug_struct("TimerHandle")
            .field("inner", &"...")
            .finish()
    }
}
