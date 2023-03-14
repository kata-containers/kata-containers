// Copyright (C) 2022 Alibaba Cloud. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0

//! `Runtime` to wrap over tokio current-thread `Runtime` and tokio-uring `Runtime`.

use std::future::Future;

/// An adapter enum to support both tokio current-thread Runtime and tokio-uring Runtime.
pub enum Runtime {
    /// Tokio current thread Runtime.
    Tokio(tokio::runtime::Runtime),
    #[cfg(target_os = "linux")]
    /// Tokio-uring Runtime.
    Uring(std::sync::Mutex<crate::tokio_uring::Runtime>),
}

impl Runtime {
    /// Create a new instance of async Runtime.
    ///
    /// A `tokio-uring::Runtime` is create if io-uring is available, otherwise a tokio current
    /// thread Runtime will be created.
    ///
    /// # Panic
    /// Panic if failed to create the Runtime object.
    pub fn new() -> Self {
        // Check whether io-uring is available.
        #[cfg(target_os = "linux")]
        {
            // TODO: use io-uring probe to detect supported operations.
            if let Ok(rt) = crate::tokio_uring::Runtime::new() {
                return Runtime::Uring(std::sync::Mutex::new(rt));
            }
        }

        // Create tokio runtime if io-uring is not supported.
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("utils: failed to create tokio runtime for current thread");
        Runtime::Tokio(rt)
    }

    /// Run a future to completion.
    pub fn block_on<F: Future>(&self, f: F) -> F::Output {
        match self {
            Runtime::Tokio(rt) => rt.block_on(f),
            #[cfg(target_os = "linux")]
            Runtime::Uring(rt) => rt.lock().unwrap().block_on(f),
        }
    }
}

/// Start an async runtime.
pub fn start<F: Future>(future: F) -> F::Output {
    Runtime::new().block_on(future)
}

impl Default for Runtime {
    fn default() -> Self {
        Runtime::new()
    }
}

std::thread_local! {
    pub(crate) static CURRENT_RUNTIME: Runtime = Runtime::new();
}

/// Run a callback with the default `Runtime` object.
pub fn with_runtime<F, R>(f: F) -> R
where
    F: FnOnce(&Runtime) -> R,
{
    CURRENT_RUNTIME.with(f)
}

/// Run a future to completion with the default `Runtime` object.
pub fn block_on<F: Future>(f: F) -> F::Output {
    CURRENT_RUNTIME.with(|rt| rt.block_on(f))
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
pub fn spawn<T: std::future::Future + 'static>(task: T) -> tokio::task::JoinHandle<T::Output> {
    CURRENT_RUNTIME.with(|rt| match rt {
        Runtime::Tokio(_) => tokio::task::spawn_local(task),
        #[cfg(target_os = "linux")]
        Runtime::Uring(_) => crate::tokio_uring::spawn(task),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_with_runtime() {
        let res = with_runtime(|rt| rt.block_on(async { 1 }));
        assert_eq!(res, 1);

        let res = with_runtime(|rt| rt.block_on(async { 3 }));
        assert_eq!(res, 3);
    }

    #[test]
    fn test_block_on() {
        let res = block_on(async { 1 });
        assert_eq!(res, 1);

        let res = block_on(async { 3 });
        assert_eq!(res, 3);
    }
}
