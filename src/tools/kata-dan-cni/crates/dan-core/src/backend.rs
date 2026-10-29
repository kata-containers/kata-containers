// Copyright (c) 2026 Kata Containers community
// SPDX-License-Identifier: Apache-2.0

use std::error::Error;
use std::fmt::{Display, Formatter};
use std::future::Future;
use std::pin::Pin;

use serde_json::Value;

use crate::{DanDevice, HostPortConfig, LogicalPort, NetworkAllocation, NetworkSelection};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NetworkContext {
    pub container_id: String,
    pub selection: NetworkSelection,
    pub allocation: NetworkAllocation,
    pub logical_port: LogicalPort,
}

/// The CNI fields that identify host resources without querying a Pod.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachmentIdentity {
    pub container_id: String,
    pub if_name: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BackendState {
    pub backend: String,
    pub data: Value,
}

#[derive(Clone, Copy, Debug)]
pub struct BackendCapabilities {
    pub ipv4: bool,
    pub ipv6: bool,
}

impl BackendCapabilities {
    pub fn validate(self, allocation: &NetworkAllocation) -> Result<(), BackendError> {
        for ip in &allocation.addresses {
            if (ip.address.is_ipv4() && !self.ipv4) || (ip.address.is_ipv6() && !self.ipv6) {
                return Err(BackendError::InvalidInput(format!(
                    "backend does not support address {}",
                    ip.address
                )));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BackendError {
    InvalidInput(String),
    OperationFailed(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct BackendAddOutcome {
    pub state: BackendState,
    pub device: DanDevice,
}

impl Display for BackendError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidInput(message) => write!(formatter, "invalid backend input: {message}"),
            Self::OperationFailed(message) => {
                write!(formatter, "backend operation failed: {message}")
            }
        }
    }
}

impl Error for BackendError {}

pub type BackendFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, BackendError>> + Send + 'a>>;

pub trait Backend: Send + Sync {
    fn name(&self) -> &'static str;

    fn capabilities(&self) -> BackendCapabilities;

    fn add<'a>(&'a self, context: &'a NetworkContext) -> BackendFuture<'a, BackendAddOutcome>;

    /// Create the endpoint with its initial host-side rate, netem and mirror settings.
    fn add_with_port_config<'a>(
        &'a self,
        context: &'a NetworkContext,
        config: &'a HostPortConfig,
    ) -> BackendFuture<'a, BackendAddOutcome> {
        Box::pin(async move {
            if !config.is_empty() {
                return Err(BackendError::InvalidInput(format!(
                    "backend `{}` does not support host port settings",
                    self.name()
                )));
            }
            self.add(context).await
        })
    }

    fn check<'a>(
        &'a self,
        identity: &'a AttachmentIdentity,
        device: &'a DanDevice,
    ) -> BackendFuture<'a, ()>;

    fn del<'a>(&'a self, identity: &'a AttachmentIdentity) -> BackendFuture<'a, ()>;

    /// Undo resources created by this ADD, not resources reused by a retry.
    fn rollback_add<'a>(&'a self, state: &'a BackendState) -> BackendFuture<'a, ()>;
}
