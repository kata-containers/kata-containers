// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

mod container;
use container::{Container, Exec};
mod container_inner;
mod io;
use container_inner::ContainerInner;
mod manager;
pub use manager::VirtContainerManager;
mod process;

use common::{error::Error, types::ContainerProcess};
use containerd_shim_protos::ttrpc;

/// Check if the signal is a termination signal (SIGKILL or SIGTERM).
///
/// These signals are used to terminate processes and containers, and
/// are treated specially for idempotent cleanup operations.
pub fn is_termination_signal(signal: u32) -> bool {
    signal == libc::SIGKILL as u32 || signal == libc::SIGTERM as u32
}

fn logger_with_process(container_process: &ContainerProcess) -> slog::Logger {
    sl!().new(o!("container_id" => container_process.container_id.container_id.clone(), "exec_id" => container_process.exec_id.clone()))
}

/// Convert agent/ttrpc errors into typed Error enum variants.
///
/// This function handles common error cases when communicating with the kata agent:
/// - Connection closed (VM died, agent crashed) -> Error::AgentConnectionClosed
/// - Process not found (already terminated) -> Error::ProcessAlreadyTerminated
pub fn convert_agent_error(err: anyhow::Error) -> anyhow::Error {
    if let Some(ttrpc_err) = err.downcast_ref::<ttrpc::error::Error>() {
        match ttrpc_err {
            // Handle connection issues (local/remote connection closed)
            ttrpc::error::Error::LocalClosed
            | ttrpc::error::Error::RemoteClosed
            | ttrpc::error::Error::Eof => {
                return Error::AgentConnectionClosed.into();
            }
            // Handle errors returned by the agent (RPC status errors)
            ttrpc::error::Error::RpcStatus(status) => {
                if status.code() == ttrpc::Code::NOT_FOUND {
                    return Error::ProcessAlreadyTerminated.into();
                }
            }
            _ => {}
        }
    }

    // Return original error if no known patterns matched
    err
}
