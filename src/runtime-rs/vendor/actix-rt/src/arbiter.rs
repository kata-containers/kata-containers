use std::{
    cell::RefCell,
    fmt,
    future::Future,
    pin::Pin,
    sync::atomic::{AtomicUsize, Ordering},
    task::{Context, Poll},
    thread,
};

use futures_core::ready;
use tokio::sync::mpsc;

use crate::system::{System, SystemCommand};

pub(crate) static COUNT: AtomicUsize = AtomicUsize::new(0);

thread_local!(
    static HANDLE: RefCell<Option<ArbiterHandle>> = RefCell::new(None);
);

pub(crate) enum ArbiterCommand {
    Stop,
    Execute(Pin<Box<dyn Future<Output = ()> + Send>>),
}

impl fmt::Debug for ArbiterCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ArbiterCommand::Stop => write!(f, "ArbiterCommand::Stop"),
            ArbiterCommand::Execute(_) => write!(f, "ArbiterCommand::Execute"),
        }
    }
}

/// A handle for sending spawn and stop messages to an [Arbiter].
#[derive(Debug, Clone)]
pub struct ArbiterHandle {
    tx: mpsc::UnboundedSender<ArbiterCommand>,
}

impl ArbiterHandle {
    pub(crate) fn new(tx: mpsc::UnboundedSender<ArbiterCommand>) -> Self {
        Self { tx }
    }

    /// Send a future to the [Arbiter]'s thread and spawn it.
    ///
    /// If you require a result, include a response channel in the future.
    ///
    /// Returns true if future was sent successfully and false if the [Arbiter] has died.
    pub fn spawn<Fut>(&self, future: Fut) -> bool
    where
        Fut: Future<Output = ()> + Send + 'static,
    {
        self.tx
            .send(ArbiterCommand::Execute(Box::pin(future)))
            .is_ok()
    }

    /// Send a function to the [Arbiter]'s thread and execute it.
    ///
    /// Any result from the function is discarded. If you require a result, include a response
    /// channel in the function.
    ///
    /// Returns true if function was sent successfully and false if the [Arbiter] has died.
    pub fn spawn_fn<F>(&self, f: F) -> bool
    where
        F: FnOnce() + Send + 'static,
    {
        self.spawn(async { f() })
    }

    /// Instruct [Arbiter] to stop processing it's event loop.
    ///
    /// Returns true if stop message was sent successfully and false if the [Arbiter] has
    /// been dropped.
    pub fn stop(&self) -> bool {
        self.tx.send(ArbiterCommand::Stop).is_ok()
    }
}

/// An Arbiter represents a thread that provides an asynchronous execution environment for futures
/// and functions.
///
/// When an arbiter is created, it spawns a new [OS thread](thread), and hosts an event loop.
#[derive(Debug)]
pub struct Arbiter {
    tx: mpsc::UnboundedSender<ArbiterCommand>,
    thread_handle: thread::JoinHandle<()>,
}

impl Arbiter {
    /// Spawn a new Arbiter thread and start its event loop.
    ///
    /// # Panics
    /// Panics if a [System] is not registered on the current thread.
    #[cfg(not(all(target_os = "linux", feature = "io-uring")))]
    #[allow(clippy::new_without_default)]
    pub fn new() -> Arbiter {
        Self::with_tokio_rt(|| {
            crate::runtime::default_tokio_runtime()
                .expect("Cannot create new Arbiter's Runtime.")
        })
    }

    /// Spawn a new Arbiter using the [Tokio Runtime](tokio-runtime) returned from a closure.
    ///
    /// [tokio-runtime]: tokio::runtime::Runtime
    #[cfg(not(all(target_os = "linux", feature = "io-uring")))]
    pub fn with_tokio_rt<F>(runtime_factory: F) -> Arbiter
    where
        F: Fn() -> tokio::runtime::Runtime + Send + 'static,
    {
        let sys = System::current();
        let system_id = sys.id();
        let arb_id = COUNT.fetch_add(1, Ordering::Relaxed);

        let name = format!("actix-rt|system:{}|arbiter:{}", system_id, arb_id);
        let (tx, rx) = mpsc::unbounded_channel();

        let (ready_tx, ready_rx) = std::sync::mpsc::channel::<()>();

        let thread_handle = thread::Builder::new()
            .name(name.clone())
            .spawn({
                let tx = tx.clone();
                move || {
                    let rt = crate::runtime::Runtime::from(runtime_factory());
                    let hnd = ArbiterHandle::new(tx);

                    System::set_current(sys);

                    HANDLE.with(|cell| *cell.borrow_mut() = Some(hnd.clone()));

                    // register arbiter
                    let _ = System::current()
                        .tx()
                        .send(SystemCommand::RegisterArbiter(arb_id, hnd));

                    ready_tx.send(()).unwrap();

                    // run arbiter event processing loop
                    rt.block_on(ArbiterRunner { rx });

                    // deregister arbiter
                    let _ = System::current()
                        .tx()
                        .send(SystemCommand::DeregisterArbiter(arb_id));
                }
            })
            .unwrap_or_else(|err| {
                panic!("Cannot spawn Arbiter's thread: {:?}. {:?}", &name, err)
            });

        ready_rx.recv().unwrap();

        Arbiter { tx, thread_handle }
    }

    /// Spawn a new Arbiter thread and start its event loop with `tokio-uring` runtime.
    ///
    /// # Panics
    /// Panics if a [System] is not registered on the current thread.
    #[cfg(all(target_os = "linux", feature = "io-uring"))]
    #[allow(clippy::new_without_default)]
    pub fn new() -> Arbiter {
        let sys = System::current();
        let system_id = sys.id();
        let arb_id = COUNT.fetch_add(1, Ordering::Relaxed);

        let name = format!("actix-rt|system:{}|arbiter:{}", system_id, arb_id);
        let (tx, rx) = mpsc::unbounded_channel();

        let (ready_tx, ready_rx) = std::sync::mpsc::channel::<()>();

        let thread_handle = thread::Builder::new()
            .name(name.clone())
            .spawn({
                let tx = tx.clone();
                move || {
                    let hnd = ArbiterHandle::new(tx);

                    System::set_current(sys);

                    HANDLE.with(|cell| *cell.borrow_mut() = Some(hnd.clone()));

                    // register arbiter
                    let _ = System::current()
                        .tx()
                        .send(SystemCommand::RegisterArbiter(arb_id, hnd));

                    ready_tx.send(()).unwrap();

                    // run arbiter event processing loop
                    tokio_uring::start(ArbiterRunner { rx });

                    // deregister arbiter
                    let _ = System::current()
                        .tx()
                        .send(SystemCommand::DeregisterArbiter(arb_id));
                }
            })
            .unwrap_or_else(|err| {
                panic!("Cannot spawn Arbiter's thread: {:?}. {:?}", &name, err)
            });

        ready_rx.recv().unwrap();

        Arbiter { tx, thread_handle }
    }

    /// Sets up an Arbiter runner in a new System using the environment's local set.
    pub(crate) fn in_new_system() -> ArbiterHandle {
        let (tx, rx) = mpsc::unbounded_channel();

        let hnd = ArbiterHandle::new(tx);

        HANDLE.with(|cell| *cell.borrow_mut() = Some(hnd.clone()));

        crate::spawn(ArbiterRunner { rx });

        hnd
    }

    /// Return a handle to the this Arbiter's message sender.
    pub fn handle(&self) -> ArbiterHandle {
        ArbiterHandle::new(self.tx.clone())
    }

    /// Return a handle to the current thread's Arbiter's message sender.
    ///
    /// # Panics
    /// Panics if no Arbiter is running on the current thread.
    pub fn current() -> ArbiterHandle {
        HANDLE.with(|cell| match *cell.borrow() {
            Some(ref hnd) => hnd.clone(),
            None => panic!("Arbiter is not running."),
        })
    }

    /// Try to get current running arbiter handle.
    ///
    /// Returns `None` if no Arbiter has been started.
    ///
    /// Unlike [`current`](Self::current), this never panics.
    pub fn try_current() -> Option<ArbiterHandle> {
        HANDLE.with(|cell| cell.borrow().clone())
    }

    /// Stop Arbiter from continuing it's event loop.
    ///
    /// Returns true if stop message was sent successfully and false if the Arbiter has been dropped.
    pub fn stop(&self) -> bool {
        self.tx.send(ArbiterCommand::Stop).is_ok()
    }

    /// Send a future to the Arbiter's thread and spawn it.
    ///
    /// If you require a result, include a response channel in the future.
    ///
    /// Returns true if future was sent successfully and false if the Arbiter has died.
    pub fn spawn<Fut>(&self, future: Fut) -> bool
    where
        Fut: Future<Output = ()> + Send + 'static,
    {
        self.tx
            .send(ArbiterCommand::Execute(Box::pin(future)))
            .is_ok()
    }

    /// Send a function to the Arbiter's thread and execute it.
    ///
    /// Any result from the function is discarded. If you require a result, include a response
    /// channel in the function.
    ///
    /// Returns true if function was sent successfully and false if the Arbiter has died.
    pub fn spawn_fn<F>(&self, f: F) -> bool
    where
        F: FnOnce() + Send + 'static,
    {
        self.spawn(async { f() })
    }

    /// Wait for Arbiter's event loop to complete.
    ///
    /// Joins the underlying OS thread handle. See [`JoinHandle::join`](thread::JoinHandle::join).
    pub fn join(self) -> thread::Result<()> {
        self.thread_handle.join()
    }
}

/// A persistent future that processes [Arbiter] commands.
struct ArbiterRunner {
    rx: mpsc::UnboundedReceiver<ArbiterCommand>,
}

impl Future for ArbiterRunner {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // process all items currently buffered in channel
        loop {
            match ready!(Pin::new(&mut self.rx).poll_recv(cx)) {
                // channel closed; no more messages can be received
                None => return Poll::Ready(()),

                // process arbiter command
                Some(item) => match item {
                    ArbiterCommand::Stop => {
                        return Poll::Ready(());
                    }
                    ArbiterCommand::Execute(task_fut) => {
                        tokio::task::spawn_local(task_fut);
                    }
                },
            }
        }
    }
}
