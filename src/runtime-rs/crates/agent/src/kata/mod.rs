// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

mod agent;
mod trans;

use std::{
    os::unix::io::RawFd,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
};

use anyhow::{Context, Result};
use kata_types::config::Agent as AgentConfig;
use nix::sys::socket::{shutdown, Shutdown};
use protocols::{
    agent::{Interfaces, ListInterfacesRequest},
    agent_ttrpc_async as agent_ttrpc, health_ttrpc_async as health_ttrpc,
};
use tokio::sync::{Notify, RwLock};
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

impl std::fmt::Debug for KataAgentInner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KataAgentInner")
            .field("client_fd", &self.client_fd)
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
    reconnecting: AtomicBool,
    reconnect_generation: AtomicU64,
    reconnected: Notify,
}

impl KataAgent {
    pub fn new(config: AgentConfig) -> Self {
        KataAgent {
            inner: Arc::new(RwLock::new(KataAgentInner {
                client: None,
                client_fd: -1,
                socket_address: "".to_string(),
                config,
                log_forwarder: LogForwarder::new(),
            })),
            reconnecting: AtomicBool::new(false),
            reconnect_generation: AtomicU64::new(0),
            reconnected: Notify::new(),
        }
    }

    pub async fn get_health_client(&self) -> Option<(health_ttrpc::HealthClient, i64, RawFd)> {
        let inner = self.inner.read().await;
        inner.client.as_ref().map(|c| {
            (
                health_ttrpc::HealthClient::new(c.clone()),
                inner.config.health_check_request_timeout_ms as i64,
                inner.client_fd,
            )
        })
    }

    pub async fn get_agent_client(&self) -> Option<(agent_ttrpc::AgentServiceClient, i64, RawFd)> {
        let inner = self.inner.read().await;
        inner.client.as_ref().map(|c| {
            (
                agent_ttrpc::AgentServiceClient::new(c.clone()),
                inner.config.request_timeout_ms as i64,
                inner.client_fd,
            )
        })
    }

    pub(crate) async fn set_socket_address(&self, address: &str) -> Result<()> {
        let mut inner = self.inner.write().await;
        inner.socket_address = address.to_string();
        Ok(())
    }

    async fn reconnecting_client(&self) -> Result<(agent_ttrpc::AgentServiceClient, u64)> {
        loop {
            let notified = self.reconnected.notified();
            let inner = self.inner.read().await;
            if !self.reconnecting.load(Ordering::Acquire) {
                let client = inner.client.as_ref().context("get client")?;
                return Ok((
                    agent_ttrpc::AgentServiceClient::new(client.clone()),
                    self.reconnect_generation.load(Ordering::Acquire),
                ));
            }
            drop(inner);
            notified.await;
        }
    }

    pub(crate) async fn connect_agent_server(&self) -> Result<()> {
        let mut inner = self.inner.write().await;

        let config = sock::ConnectConfig::new(
            inner.config.dial_timeout_ms as u64,
            inner.config.reconnect_timeout_ms as u64,
            sock::HandshakeReplayPolicy::PeerClosedBeforeResponse,
        );
        let connection = async {
            let sock =
                sock::new(&inner.socket_address, inner.config.server_port).context("new sock")?;
            info!(sl!(), "try to connect agent server through {:?}", sock);
            sock.connect(&config).await.context("connect")
        }
        .await;
        let stream = match connection {
            Ok(stream) => stream,
            Err(err) => {
                // The attempt ended; pending RPCs must see the connection error.
                self.reconnecting.store(false, Ordering::Release);
                self.reconnected.notify_waiters();
                return Err(err);
            }
        };
        let client_fd = stream.raw_fd();
        info!(
            sl!(),
            "get stream raw fd {:?} with socket address: {:?} and server_port {:?}",
            client_fd,
            &inner.socket_address,
            inner.config.server_port
        );
        let c = Client::new(stream.into_ttrpc_socket());
        inner.client = Some(c);
        inner.client_fd = client_fd;
        self.reconnecting.store(false, Ordering::Release);
        self.reconnected.notify_waiters();
        Ok(())
    }

    pub(crate) async fn start_log_forwarder(&self) -> Result<()> {
        let mut inner = self.inner.write().await;
        let config = sock::ConnectConfig::new(
            inner.config.dial_timeout_ms as u64,
            inner.config.reconnect_timeout_ms as u64,
            sock::HandshakeReplayPolicy::PeerClosedBeforeResponse,
        );
        let address = inner.socket_address.clone();
        let port = inner.config.log_port;
        inner
            .log_forwarder
            .start(&address, port, config)
            .await
            .context("start log forwarder")
            .context("connect log forwarder")?;

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
        self.reconnect_generation.fetch_add(1, Ordering::AcqRel);
        self.reconnecting.store(true, Ordering::Release);
        inner.log_forwarder.stop();
        let mut shutdown_error = None;

        // Long-running WaitProcess calls retain ttrpc client clones. Shut down
        // the transport so the guest agent returns to accept before a VM
        // snapshot; WaitProcess retries after the planned reconnect.
        if inner.client_fd >= 0 {
            if let Err(err) = shutdown(inner.client_fd, Shutdown::Both) {
                if !matches!(err, nix::errno::Errno::ENOTCONN | nix::errno::Errno::EBADF) {
                    shutdown_error = Some(err);
                } else {
                    debug!(sl!(), "agent client socket is already closed: {err}");
                }
            }
        }

        // If there is a valid client, drop it (closes the connection).
        inner.client.take();
        inner.client_fd = -1;

        if let Some(err) = shutdown_error {
            // The planned reconnect did not start. Wake RPCs so they fail on
            // the missing client instead of waiting forever for a reconnect
            // notification that no caller is required to send.
            self.reconnecting.store(false, Ordering::Release);
            self.reconnected.notify_waiters();
            return Err(err).context("shutdown agent client socket");
        }

        Ok(())
    }
}

/// Prove that the guest has drained earlier sandbox RPC handlers before a
/// snapshot. This temporary client is not published as the active agent
/// connection, so background runtime requests remain blocked until reconnect.
pub async fn wait_snapshot_ready(address: &str, config: &AgentConfig) -> Result<()> {
    let connect_config = sock::ConnectConfig::new(
        config.dial_timeout_ms as u64,
        config.reconnect_timeout_ms as u64,
        sock::HandshakeReplayPolicy::Never,
    );
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);

    loop {
        let result = tokio::time::timeout_at(deadline, async {
            let sock =
                sock::new(address, config.server_port).context("new snapshot readiness sock")?;
            let stream = sock
                .connect(&connect_config)
                .await
                .context("connect snapshot readiness client")?;

            let hvsock_local_port = stream.hvsock_local_port();
            let client_fd = stream.raw_fd();
            let client = Client::new(stream.into_ttrpc_socket());
            let agent = agent_ttrpc::AgentServiceClient::new(client.clone());

            let timeout =
                ttrpc::context::with_timeout(config.request_timeout_ms as i64 * 1_000_000);
            let probe_result: Result<Interfaces> = agent
                .list_interfaces(timeout, &ListInterfacesRequest::new())
                .await
                .context("probe guest sandbox lock with ListInterfaces");

            if let Err(err) = shutdown(client_fd, Shutdown::Both) {
                if !matches!(err, nix::errno::Errno::ENOTCONN | nix::errno::Errno::EBADF) {
                    return Err(err).context("shutdown snapshot readiness client socket");
                }
            }
            probe_result?;
            Ok::<_, anyhow::Error>((sock, hvsock_local_port))
        })
        .await
        .context("snapshot readiness deadline expired")?;

        match result {
            Ok((sock, hvsock_local_port)) => {
                if let Some(local_port) = hvsock_local_port {
                    sock.wait_guest_rst(local_port, config.server_port, deadline)
                        .await
                        .context("wait for snapshot readiness probe guest RST")?;
                }
                return Ok(());
            }
            Err(err) if tokio::time::Instant::now() < deadline => {
                debug!(sl!(), "snapshot readiness probe not ready: {err:#}");
                tokio::time::sleep_until(std::cmp::min(
                    deadline,
                    tokio::time::Instant::now() + std::time::Duration::from_millis(25),
                ))
                .await;
            }
            Err(err) => return Err(err).context("snapshot readiness deadline expired"),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::os::fd::AsRawFd;
    use std::time::Duration;

    use super::*;

    #[tokio::test]
    async fn failed_reconnect_releases_pending_rpcs() {
        use std::{future::Future, task::Poll};
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
        use tokio::net::UnixListener;

        use crate::Agent;

        for invalid_address in [true, false] {
            let path = std::env::temp_dir().join(format!(
                "kata-reconnect-failure-{}-{invalid_address}.sock",
                std::process::id()
            ));
            let server = if invalid_address {
                None
            } else {
                let listener = UnixListener::bind(&path).unwrap();
                Some(tokio::spawn(async move {
                    let (mut stream, _) = listener.accept().await.unwrap();
                    let mut command = String::new();
                    BufReader::new(&mut stream)
                        .read_line(&mut command)
                        .await
                        .unwrap();
                    assert_eq!(command, "connect 1024\n");
                    stream.write_all(b"NO\n").await.unwrap();
                }))
            };
            let agent = KataAgent::new(AgentConfig {
                server_port: 1024,
                ..Default::default()
            });
            agent.disconnect().await.unwrap();
            let stdout = agent.read_stdout(crate::ReadStreamRequest::default());
            let stderr = agent.read_stderr(crate::ReadStreamRequest::default());
            let wait = agent.wait_process(crate::WaitProcessRequest::default());
            tokio::pin!(stdout, stderr, wait);
            // Register all waiters before completing the failed attempt.
            std::future::poll_fn(|cx| {
                assert!(stdout.as_mut().poll(cx).is_pending());
                assert!(stderr.as_mut().poll(cx).is_pending());
                assert!(wait.as_mut().poll(cx).is_pending());
                Poll::Ready(())
            })
            .await;

            let address = if invalid_address {
                "invalid://agent".to_string()
            } else {
                format!("hvsock://{}", path.display())
            };
            agent.set_socket_address(&address).await.unwrap();
            let result = tokio::time::timeout(Duration::from_secs(2), agent.connect_agent_server())
                .await
                .expect("failed handshake must terminate");
            assert!(result.is_err());
            assert!(!agent.reconnecting.load(Ordering::Acquire));
            let inner = agent.inner.read().await;
            assert!(inner.client.is_none());
            assert_eq!(inner.client_fd, -1);
            drop(inner);
            tokio::time::timeout(Duration::from_millis(100), async {
                assert!(stdout.await.is_err());
                assert!(stderr.await.is_err());
                assert!(wait.await.is_err());
            })
            .await
            .expect("pending RPCs must finish after reconnect failure");

            if let Some(server) = server {
                server.await.unwrap();
                std::fs::remove_file(path).unwrap();
            }
        }
    }

    #[tokio::test]
    async fn disconnect_shutdown_error_does_not_leave_reconnect_waiters_blocked() {
        let file = std::fs::File::open("/dev/null").expect("open /dev/null");
        let agent = KataAgent::new(AgentConfig::default());
        agent.inner.write().await.client_fd = file.as_raw_fd();

        let err = agent.disconnect().await.expect_err("shutdown must fail");
        assert!(err.to_string().contains("shutdown agent client socket"));
        assert!(!agent.reconnecting.load(Ordering::Acquire));
        assert_eq!(agent.inner.read().await.client_fd, -1);

        let client_result =
            tokio::time::timeout(Duration::from_millis(100), agent.reconnecting_client())
                .await
                .expect("client lookup must not wait for a reconnect notification");
        assert!(client_result.is_err());
    }
}
