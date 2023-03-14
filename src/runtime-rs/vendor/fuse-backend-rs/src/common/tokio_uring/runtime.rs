use crate::driver::{Driver, CURRENT};
use std::cell::RefCell;

use std::future::Future;
use std::io;
use tokio::io::unix::AsyncFd;
use tokio::task::LocalSet;

/// The tokio-uring runtime based on the Tokio current thread runtime.
pub struct Runtime {
    /// io-uring driver
    driver: AsyncFd<Driver>,

    /// LocalSet for !Send tasks
    local: LocalSet,

    /// Tokio runtime, always current-thread
    rt: tokio::runtime::Runtime,
}

/// Spawns a new asynchronous task, returning a [`JoinHandle`] for it.
///
/// Spawning a task enables the task to execute concurrently to other tasks.
/// There is no guarantee that a spawned task will execute to completion. When a
/// runtime is shutdown, all outstanding tasks are dropped, regardless of the
/// lifecycle of that task.
///
/// This function must be called from the context of a `tokio-uring` runtime.
///
/// [`JoinHandle`]: tokio::task::JoinHandle
///
/// # Examples
///
/// In this example, a server is started and `spawn` is used to start a new task
/// that processes each received connection.
///
/// ```no_run
/// fn main() {
///     tokio_uring::start(async {
///         let handle = tokio_uring::spawn(async {
///             println!("hello from a background task");
///         });
///
///         // Let the task complete
///         handle.await.unwrap();
///     });
/// }
/// ```
pub fn spawn<T: std::future::Future + 'static>(task: T) -> tokio::task::JoinHandle<T::Output> {
    tokio::task::spawn_local(task)
}

impl Runtime {
    /// Create a new tokio-uring [Runtime] object.
    pub fn new() -> io::Result<Runtime> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .on_thread_park(|| {
                CURRENT.with(|x| {
                    let _ = RefCell::borrow_mut(x).uring.submit();
                });
            })
            .enable_all()
            .build()?;

        let local = LocalSet::new();

        let driver = {
            let _guard = rt.enter();
            AsyncFd::new(Driver::new()?)?
        };

        Ok(Runtime { driver, local, rt })
    }

    /// Runs a future to completion on the Tokio-uring runtime.
    ///
    /// This runs the given future on the current thread, blocking until it is
    /// complete, and yielding its resolved result. Any tasks or timers
    /// which the future spawns internally will be executed on the runtime.
    pub fn block_on<F>(&mut self, future: F) -> F::Output
    where
        F: Future,
    {
        self.driver.get_ref().with(|| {
            let drive = async {
                loop {
                    // Wait for read-readiness
                    let mut guard = self.driver.readable().await.unwrap();
                    self.driver.get_ref().tick();
                    guard.clear_ready();
                }
            };

            tokio::pin!(drive);
            tokio::pin!(future);

            self.rt
                .block_on(self.local.run_until(crate::future::poll_fn(|cx| {
                    assert!(drive.as_mut().poll(cx).is_pending());
                    future.as_mut().poll(cx)
                })))
        })
    }
}
