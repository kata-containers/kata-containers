// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use nix::libc;

use crate::types::{ContainerProcess, SandboxResponse, TaskResponse};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("failed to find container {0}")]
    ContainerNotFound(String),
    #[error("failed to find process {0}")]
    ProcessNotFound(ContainerProcess),
    #[error("unexpected task response {0} to shim {1}")]
    UnexpectedResponse(TaskResponse, String),
    #[error("unexpected sandbox response {0} to shim {1}")]
    UnexpectedSandboxResponse(SandboxResponse, String),
    #[error("agent connection closed")]
    AgentConnectionClosed,
    #[error("process already terminated")]
    ProcessAlreadyTerminated,
}

/// Common error messages indicating normal OOM shutdowns due to network issues.
const NORMAL_OOM_SHUTDOWN_MESSAGES: &[&str] = &[
    "Connection reset by peer",
    "Broken pipe",
    "transport endpoint is not connected",
];

/// Checks if an error indicates a normal oom shutdown due to network disconnections.
///
/// This function identifies errors that commonly occur when a connection is gracefully
/// or unexpectedly terminated by the peer, such as network interruptions or the remote
/// end closing the connection.
pub fn is_normal_oom_shutdown_error(err: &anyhow::Error) -> bool {
    // Check for common I/O error kinds that indicate connection issues.
    if let Some(io_err) = err.downcast_ref::<std::io::Error>() {
        match io_err.kind() {
            std::io::ErrorKind::ConnectionReset
            | std::io::ErrorKind::BrokenPipe
            | std::io::ErrorKind::NotConnected => return true,
            _ => {}
        }
    }

    // Additionally, check the error message for specific substrings.
    let error_string = err.to_string().to_lowercase();
    NORMAL_OOM_SHUTDOWN_MESSAGES
        .iter()
        .any(|pattern| error_string.contains(pattern))
}

/// List of common error message patterns indicating that a process or container is missing.
const NO_SUCH_PROCESS_MESSAGES: &[&str] = &[
    "no such process",
    "process not found",
    "init process not found",
    "cannot find init process",
];

/// Returns `true` if the error indicates that the target process/container no longer exists.
/// This is used to determine if an operation, like signaling a process, failed because the
/// target is no longer available.
/// The function checks for standard OS error codes (`ESRCH`, `ENOENT`) and common error message patterns.
pub fn is_no_such_process_error(err: &anyhow::Error) -> bool {
    // Check for standard OS error codes.
    if let Some(io_err) = err.downcast_ref::<std::io::Error>() {
        if let Some(raw_os_error) = io_err.raw_os_error() {
            // standard "no such process" error.
            if raw_os_error == libc::ESRCH || raw_os_error == libc::ENOENT {
                return true;
            }
        }
    }

    // Fallback to checking the error message for known patterns.
    let error_string = err.to_string().to_lowercase();
    NO_SUCH_PROCESS_MESSAGES
        .iter()
        .any(|pattern| error_string.contains(pattern))
}
