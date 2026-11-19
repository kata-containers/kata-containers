// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

mod agent;
mod trans;

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use anyhow::{Context, Result};
use kata_types::config::Agent as AgentConfig;
use protocols::{agent_ttrpc_async as agent_ttrpc, health_ttrpc_async as health_ttrpc};
use tokio::sync::RwLock;
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

    /// Unix domain socket address
    pub socket_address: String,

    /// Agent config
    config: AgentConfig,

    /// Log forwarder
    log_forwarder: LogForwarder,
}

impl std::fmt::Debug for KataAgentInner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KataAgentInner")
            .field("socket_address", &self.socket_address)
            .field("config", &self.config)
            .finish()
    }
}

unsafe impl Send for KataAgent {}
unsafe impl Sync for KataAgent {}
#[derive(Debug)]
pub struct KataAgent {
    pub(crate) inner: Arc<RwLock<KataAgentInner>>,
}

impl KataAgent {
    pub fn new(config: AgentConfig) -> Self {
        KataAgent {
            inner: Arc::new(RwLock::new(KataAgentInner {
                client: None,
                socket_address: "".to_string(),
                config,
                log_forwarder: LogForwarder::new(),
            })),
        }
    }

    pub async fn get_health_client(&self) -> Option<(health_ttrpc::HealthClient, i64)> {
        let inner = self.inner.read().await;
        inner.client.as_ref().map(|c| {
            (
                health_ttrpc::HealthClient::new(c.clone()),
                inner.config.health_check_request_timeout_ms as i64,
            )
        })
    }

    pub async fn get_agent_client(&self) -> Option<(agent_ttrpc::AgentServiceClient, i64)> {
        let inner = self.inner.read().await;
        inner.client.as_ref().map(|c| {
            (
                agent_ttrpc::AgentServiceClient::new(c.clone()),
                inner.config.request_timeout_ms as i64,
            )
        })
    }

    pub(crate) async fn set_socket_address(&self, address: &str) -> Result<()> {
        let mut inner = self.inner.write().await;
        inner.socket_address = address.to_string();
        Ok(())
    }

    // Retain the transport setup so a later RPC can reconnect, including the
    // hvsock handshake. An RPC interrupted by a disconnect still returns its
    // error: replaying it could execute a state-changing operation twice.
    pub(crate) async fn connect_agent_server(&self) -> Result<()> {
        let mut inner = self.inner.write().await;

        let config = Arc::new(sock::ConnectConfig::new(
            inner.config.dial_timeout_ms as u64,
            inner.config.reconnect_timeout_ms as u64,
        ));
        let sock =
            sock::new(&inner.socket_address, inner.config.server_port).context("new sock")?;
        let first_connection = AtomicBool::new(true);
        let client = Client::connect_with(move || {
            let sock = sock.clone();
            let config = config.clone();
            let reconnecting = !first_connection.swap(false, Ordering::Relaxed);
            async move {
                info!(sl!(), "try to connect agent server through {:?}", sock);
                // Bound the complete attempt, including the hvsock handshake.
                let stream = tokio::time::timeout(
                    config.reconnect_timeout(),
                    sock.connect(&config),
                )
                .await
                .unwrap_or_else(|_| Err(anyhow::anyhow!("agent connection timed out")))
                .map_err(|error| {
                    if reconnecting {
                        // A failed redial leaves the session closed. The VM
                        // and its processes may still be running.
                        warn!(sl!(), "failed to reconnect agent: {error:#}"; "socket" => ?sock);
                        ttrpc::Error::LocalClosed
                    } else {
                        ttrpc::Error::Socket(format!("connect agent: {error:#}"))
                    }
                })?;
                info!(
                    sl!(),
                    "connected to agent through {:?}, fd {}",
                    sock,
                    stream.raw_fd()
                );
                Ok(stream.into_ttrpc_socket())
            }
        })
        .await
        .context("connect agent")?;
        inner.client = Some(client);
        Ok(())
    }

    pub(crate) async fn start_log_forwarder(&self) -> Result<()> {
        let mut inner = self.inner.write().await;
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
        let mut inner = self.inner.write().await;
        inner.log_forwarder.stop();
    }

    pub(crate) async fn agent_sock(&self) -> Result<String> {
        let inner = self.inner.read().await;
        Ok(format!(
            "{}:{}",
            inner.socket_address.clone(),
            inner.config.server_port
        ))
    }

    pub(crate) async fn agent_config(&self) -> AgentConfig {
        let inner = self.inner.read().await;
        inner.config.clone()
    }

    /// Disconnect from the agent gRPC server and clean up related resources.
    pub(crate) async fn disconnect(&self) -> Result<()> {
        let mut inner = self.inner.write().await;
        inner.log_forwarder.stop();

        // If there is a valid client, drop it (closes the connection).
        inner.client.take();

        Ok(())
    }
}

#[cfg(test)]
mod reconnect_tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};
    use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
    use tokio::net::UnixListener;
    use ttrpc::proto::{Codec, GenMessage, MessageHeader};

    struct SocketPath(PathBuf);

    impl Drop for SocketPath {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }

    fn socket_path() -> SocketPath {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        SocketPath(std::env::temp_dir().join(format!(
            "kata-reconnect-{}-{suffix}.sock",
            std::process::id()
        )))
    }

    #[tokio::test]
    async fn reconnect_repeats_hvsock_handshake_without_replaying_failed_rpc() {
        let path = socket_path();
        let listener = UnixListener::bind(&path.0).unwrap();
        let config = AgentConfig::default();
        let port = config.server_port;
        let server = tokio::spawn(async move {
            for attempt in 0..2 {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut handshake = String::new();
                BufReader::new(&mut socket)
                    .read_line(&mut handshake)
                    .await
                    .unwrap();
                assert_eq!(handshake, format!("connect {port}\n"));
                socket.write_all(b"OK 1\n").await.unwrap();

                let message = GenMessage::read_from(&mut socket).await.unwrap();
                let request = ttrpc::Request::decode(message.payload).unwrap();
                assert_eq!(request.method, "Check");
                if attempt == 0 {
                    // The request executed, but its response was interrupted.
                    socket.write_all(&[0; 5]).await.unwrap();
                } else {
                    let payload = ttrpc::Response::default().encode().unwrap();
                    GenMessage {
                        header: MessageHeader::new_response(
                            message.header.stream_id,
                            payload.len() as u32,
                        ),
                        payload,
                    }
                    .write_to(&mut socket)
                    .await
                    .unwrap();
                }
            }
        });

        tokio::time::timeout(Duration::from_secs(10), async {
            let agent = KataAgent::new(config);
            agent
                .set_socket_address(&format!("hvsock://{}", path.0.display()))
                .await
                .unwrap();
            agent.connect_agent_server().await.unwrap();
            let (client, _) = agent.get_health_client().await.unwrap();
            let request = protocols::health::CheckRequest::default();
            let error = client
                .check(ttrpc::context::with_timeout(0), &request)
                .await
                .unwrap_err();
            assert!(matches!(error, ttrpc::Error::Socket(_)));
            // Reuse the same generated client; only this new call may reconnect.
            client
                .check(ttrpc::context::with_timeout(2_000_000_000), &request)
                .await
                .unwrap();
            server.await.unwrap();
            agent.disconnect().await.unwrap();
        })
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn initial_connection_bounds_a_stalled_hvsock_handshake() {
        let path = socket_path();
        let listener = UnixListener::bind(&path.0).unwrap();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut handshake = String::new();
            BufReader::new(&mut socket)
                .read_line(&mut handshake)
                .await
                .unwrap();
            assert!(handshake.starts_with("connect "));

            // Keep the connection open without replying to the handshake.
            // The client's connection deadline must close the socket.
            assert_eq!(socket.read(&mut [0; 1]).await.unwrap(), 0);
        });
        let agent = KataAgent::new(AgentConfig::default());
        agent
            .set_socket_address(&format!("hvsock://{}", path.0.display()))
            .await
            .unwrap();
        let error = tokio::time::timeout(Duration::from_secs(15), agent.connect_agent_server())
            .await
            .expect("agent connection attempt did not finish")
            .unwrap_err();
        assert!(format!("{error:#}").contains("agent connection timed out"));
        assert!(matches!(
            error.downcast_ref::<ttrpc::Error>(),
            Some(ttrpc::Error::Socket(_))
        ));
        assert!(agent.get_agent_client().await.is_none());
        tokio::time::timeout(Duration::from_secs(2), server)
            .await
            .unwrap()
            .unwrap();
    }
}
