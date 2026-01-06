// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

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
