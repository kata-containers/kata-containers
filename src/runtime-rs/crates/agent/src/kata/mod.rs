// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

mod agent;
mod trans;

use std::os::unix::io::{IntoRawFd, RawFd};

use anyhow::{Context, Result};
use kata_types::config::Agent as AgentConfig;
use protocols::{agent_ttrpc_async as agent_ttrpc, health_ttrpc_async as health_ttrpc};
use tokio::sync::Mutex;
use ttrpc::asynchronous::Client;

use crate::{log_forwarder::LogForwarder, sock};

// https://github.com/firecracker-microvm/firecracker/blob/master/docs/vsock.md
#[derive(Debug, Default)]
pub struct Vsock {
    pub context_id: u64,
    pub port: u32,
}

pub(crate) struct KataAgentInner {
    /// TTRPC client
    pub client: Option<Client>,

    /// Client fd
    pub client_fd: RawFd,

    /// Unix domain socket address
    pub socket_address: String,

    /// Agent config
    config: AgentConfig,

    /// Log forwarder
    log_forwarder: LogForwarder,
}

unsafe impl Send for KataAgent {}
unsafe impl Sync for KataAgent {}
pub struct KataAgent {
    pub(crate) inner: Mutex<KataAgentInner>,
}

impl KataAgent {
    pub fn new(config: AgentConfig) -> Self {
        KataAgent {
            inner: Mutex::new(KataAgentInner {
                client: None,
                client_fd: -1,
                socket_address: "".to_string(),
                config,
                log_forwarder: LogForwarder::new(),
            }),
        }
    }

    pub async fn get_health_client(&self) -> Option<(health_ttrpc::HealthClient, i64, RawFd)> {
        let inner = self.inner.lock().await;
        inner.client.as_ref().map(|c| {
            (
                health_ttrpc::HealthClient::new(c.clone()),
                inner.config.health_check_request_timeout_ms as i64,
                inner.client_fd,
            )
        })
    }

    pub async fn get_agent_client(&self) -> Option<(agent_ttrpc::AgentServiceClient, i64, RawFd)> {
        let inner = self.inner.lock().await;
        inner.client.as_ref().map(|c| {
            (
                agent_ttrpc::AgentServiceClient::new(c.clone()),
                inner.config.request_timeout_ms as i64,
                inner.client_fd,
            )
        })
    }

    pub(crate) async fn set_socket_address(&self, address: &str) -> Result<()> {
        let mut inner = self.inner.lock().await;
        inner.socket_address = address.to_string();
        Ok(())
    }

    pub(crate) async fn connect_agent_server(&self) -> Result<()> {
        let mut inner = self.inner.lock().await;

        let config = sock::ConnectConfig::new(
            inner.config.dial_timeout_ms as u64,
            inner.config.reconnect_timeout_ms as u64,
        );
        let sock =
            sock::new(&inner.socket_address, inner.config.server_port).context("new sock")?;
        let stream = sock.connect(&config).await.context("connect")?;
        let fd = stream.into_raw_fd();
        info!(sl!(), "get stream raw fd {:?}", fd);
        let c = Client::new(fd);
        inner.client = Some(c);
        inner.client_fd = fd;
        Ok(())
    }

    pub(crate) async fn start_log_forwarder(&self) -> Result<()> {
        let mut inner = self.inner.lock().await;
        let config = sock::ConnectConfig::new(
            inner.config.dial_timeout_ms as u64,
            inner.config.reconnect_timeout_ms as u64,
        );
        let address = inner.socket_address.clone();
        let port = inner.config.log_port;
        inner
            .log_forwarder
            .start(&address, port, config)
            .await
            .context("start log forwarder")?;
        Ok(())
    }

    pub(crate) async fn stop_log_forwarder(&self) {
        let mut inner = self.inner.lock().await;
        inner.log_forwarder.stop();
    }

    pub(crate) async fn agent_config(&self) -> AgentConfig {
        let inner = self.inner.lock().await;
        inner.config.clone()
    }
}
