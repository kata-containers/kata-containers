use std::{
    cell::RefCell,
    collections::HashMap,
    future::Future,
    io,
    pin::Pin,
    sync::atomic::{AtomicUsize, Ordering},
    task::{Context, Poll},
};

use futures_core::ready;
use tokio::sync::{mpsc, oneshot};

use crate::{arbiter::ArbiterHandle, Arbiter};

static SYSTEM_COUNT: AtomicUsize = AtomicUsize::new(0);

thread_local!(
    static CURRENT: RefCell<Option<System>> = RefCell::new(None);
);

/// A manager for a per-thread distributed async runtime.
#[derive(Clone, Debug)]
pub struct System {
    id: usize,
    sys_tx: mpsc::UnboundedSender<SystemCommand>,

    /// Handle to the first [Arbiter] that is created with the System.
    arbiter_handle: ArbiterHandle,
}

#[cfg(not(feature = "io-uring"))]
impl System {
    /// Create a new system.
    ///
    /// # Panics
    /// Panics if underlying Tokio runtime can not be created.
    #[allow(clippy::new_ret_no_self)]
    pub fn new() -> SystemRunner {
        Self::with_tokio_rt(|| {
            crate::runtime::default_tokio_runtime()
                .expect("Default Actix (Tokio) runtime could not be created.")
        })
    }

    /// Create a new System using the [Tokio Runtime](tokio-runtime) returned from a closure.
    ///
    /// [tokio-runtime]: tokio::runtime::Runtime
    pub fn with_tokio_rt<F>(runtime_factory: F) -> SystemRunner
    where
        F: Fn() -> tokio::runtime::Runtime,
    {
        let (stop_tx, stop_rx) = oneshot::channel();
        let (sys_tx, sys_rx) = mpsc::unbounded_channel();

        let rt = crate::runtime::Runtime::from(runtime_factory());
        let sys_arbiter = rt.block_on(async { Arbiter::in_new_system() });
        let system = System::construct(sys_tx, sys_arbiter.clone());

        system
            .tx()
            .send(SystemCommand::RegisterArbiter(usize::MAX, sys_arbiter))
            .unwrap();

        // init background system arbiter
        let sys_ctrl = SystemController::new(sys_rx, stop_tx);
        rt.spawn(sys_ctrl);

        SystemRunner { rt, stop_rx }
    }
}

#[cfg(feature = "io-uring")]
impl System {
    /// Create a new system.
    ///
    /// # Panics
    /// Panics if underlying Tokio runtime can not be created.
    #[allow(clippy::new_ret_no_self)]
    pub fn new() -> SystemRunner {
        SystemRunner
    }

    /// Create a new System using the [Tokio Runtime](tokio-runtime) returned from a closure.
    ///
    /// [tokio-runtime]: tokio::runtime::Runtime
    #[doc(hidden)]
    pub fn with_tokio_rt<F>(_: F) -> SystemRunner
    where
        F: Fn() -> tokio::runtime::Runtime,
    {
        unimplemented!("System::with_tokio_rt is not implemented for io-uring feature yet")
    }
}

impl System {
    /// Constructs new system and registers it on the current thread.
    pub(crate) fn construct(
        sys_tx: mpsc::UnboundedSender<SystemCommand>,
        arbiter_handle: ArbiterHandle,
    ) -> Self {
        let sys = System {
            sys_tx,
            arbiter_handle,
            id: SYSTEM_COUNT.fetch_add(1, Ordering::SeqCst),
        };

        System::set_current(sys.clone());

        sys
    }

    /// Get current running system.
    ///
    /// # Panics
    /// Panics if no system is registered on the current thread.
    pub fn current() -> System {
        CURRENT.with(|cell| match *cell.borrow() {
            Some(ref sys) => sys.clone(),
            None => panic!("System is not running"),
        })
    }

    /// Try to get current running system.
    ///
    /// Returns `None` if no System has been started.
    ///
    /// Unlike [`current`](Self::current), this never panics.
    pub fn try_current() -> Option<System> {
        CURRENT.with(|cell| cell.borrow().clone())
    }

    /// Get handle to a the System's initial [Arbiter].
    pub fn arbiter(&self) -> &ArbiterHandle {
        &self.arbiter_handle
    }

    /// Check if there is a System registered on the current thread.
    pub fn is_registered() -> bool {
        CURRENT.with(|sys| sys.borrow().is_some())
    }

    /// Register given system on current thread.
    #[doc(hidden)]
    pub fn set_current(sys: System) {
        CURRENT.with(|cell| {
            *cell.borrow_mut() = Some(sys);
        })
    }

    /// Numeric system identifier.
    ///
    /// Useful when using multiple Systems.
    pub fn id(&self) -> usize {
        self.id
    }

    /// Stop the system (with code 0).
    pub fn stop(&self) {
        self.stop_with_code(0)
    }

    /// Stop the system with a given exit code.
    pub fn stop_with_code(&self, code: i32) {
        let _ = self.sys_tx.send(SystemCommand::Exit(code));
    }

    pub(crate) fn tx(&self) -> &mpsc::UnboundedSender<SystemCommand> {
        &self.sys_tx
    }
}

/// Runner that keeps a [System]'s event loop alive until stop message is received.
#[cfg(not(feature = "io-uring"))]
#[must_use = "A SystemRunner does nothing unless `run` is called."]
#[derive(Debug)]
pub struct SystemRunner {
    rt: crate::runtime::Runtime,
    stop_rx: oneshot::Receiver<i32>,
}

#[cfg(not(feature = "io-uring"))]
impl SystemRunner {
    /// Starts event loop and will return once [System] is [stopped](System::stop).
    pub fn run(self) -> io::Result<()> {
        let exit_code = self.run_with_code()?;

        match exit_code {
            0 => Ok(()),
            nonzero => Err(io::Error::new(
                io::ErrorKind::Other,
                format!("Non-zero exit code: {}", nonzero),
            )),
        }
    }

    /// Runs the event loop until [stopped](System::stop_with_code), returning the exit code.
    pub fn run_with_code(self) -> io::Result<i32> {
        let SystemRunner { rt, stop_rx, .. } = self;

        // run loop
        rt.block_on(stop_rx)
            .map_err(|err| io::Error::new(io::ErrorKind::Other, err))
    }

    /// Runs the provided future, blocking the current thread until the future completes.
    #[inline]
    pub fn block_on<F: Future>(&self, fut: F) -> F::Output {
        self.rt.block_on(fut)
    }
}

/// Runner that keeps a [System]'s event loop alive until stop message is received.
#[cfg(feature = "io-uring")]
#[must_use = "A SystemRunner does nothing unless `run` is called."]
#[derive(Debug)]
pub struct SystemRunner;

#[cfg(feature = "io-uring")]
impl SystemRunner {
    /// Starts event loop and will return once [System] is [stopped](System::stop).
    pub fn run(self) -> io::Result<()> {
        unimplemented!("SystemRunner::run is not implemented for io-uring feature yet");
    }

    /// Runs the event loop until [stopped](System::stop_with_code), returning the exit code.
    pub fn run_with_code(self) -> io::Result<i32> {
        unimplemented!(
            "SystemRunner::run_with_code is not implemented for io-uring feature yet"
        );
    }

    /// Runs the provided future, blocking the current thread until the future completes.
    #[inline]
    pub fn block_on<F: Future>(&self, fut: F) -> F::Output {
        tokio_uring::start(async move {
            let (stop_tx, stop_rx) = oneshot::channel();
            let (sys_tx, sys_rx) = mpsc::unbounded_channel();

            let sys_arbiter = Arbiter::in_new_system();
            let system = System::construct(sys_tx, sys_arbiter.clone());

            system
                .tx()
                .send(SystemCommand::RegisterArbiter(usize::MAX, sys_arbiter))
                .unwrap();

            // init background system arbiter
            let sys_ctrl = SystemController::new(sys_rx, stop_tx);
            tokio_uring::spawn(sys_ctrl);

            let res = fut.await;
            drop(stop_rx);
            res
        })
    }
}

#[derive(Debug)]
pub(crate) enum SystemCommand {
    Exit(i32),
    RegisterArbiter(usize, ArbiterHandle),
    DeregisterArbiter(usize),
}

/// There is one `SystemController` per [System]. It runs in the background, keeping track of
/// [Arbiter]s and is able to distribute a system-wide stop command.
#[derive(Debug)]
pub(crate) struct SystemController {
    stop_tx: Option<oneshot::Sender<i32>>,
    cmd_rx: mpsc::UnboundedReceiver<SystemCommand>,
    arbiters: HashMap<usize, ArbiterHandle>,
}

impl SystemController {
    pub(crate) fn new(
        cmd_rx: mpsc::UnboundedReceiver<SystemCommand>,
        stop_tx: oneshot::Sender<i32>,
    ) -> Self {
        SystemController {
            cmd_rx,
            stop_tx: Some(stop_tx),
            arbiters: HashMap::with_capacity(4),
        }
    }
}

impl Future for SystemController {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // process all items currently buffered in channel
        loop {
            match ready!(Pin::new(&mut self.cmd_rx).poll_recv(cx)) {
                // channel closed; no more messages can be received
                None => return Poll::Ready(()),

                // process system command
                Some(cmd) => match cmd {
                    SystemCommand::Exit(code) => {
                        // stop all arbiters
                        for arb in self.arbiters.values() {
                            arb.stop();
                        }

                        // stop event loop
                        // will only fire once
                        if let Some(stop_tx) = self.stop_tx.take() {
                            let _ = stop_tx.send(code);
                        }
                    }

                    SystemCommand::RegisterArbiter(id, arb) => {
                        self.arbiters.insert(id, arb);
                    }

                    SystemCommand::DeregisterArbiter(id) => {
                        self.arbiters.remove(&id);
                    }
                },
            }
        }
    }
}
