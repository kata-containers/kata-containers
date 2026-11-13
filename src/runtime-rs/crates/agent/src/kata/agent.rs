// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::sync::atomic::Ordering;

use anyhow::{Context, Result};
use async_trait::async_trait;
use tracing::instrument;
use ttrpc::context as ttrpc_ctx;

use kata_types::config::Agent as AgentConfig;

use crate::{kata::KataAgent, Agent, AgentManager, HealthService};

/// millisecond to nanosecond
const MILLISECOND_TO_NANOSECOND: i64 = 1_000_000;

/// new ttrpc context with timeout
fn new_ttrpc_ctx(timeout: i64) -> ttrpc_ctx::Context {
    ttrpc_ctx::with_timeout(timeout)
}

#[async_trait]
impl AgentManager for KataAgent {
    #[instrument]
    async fn start(&self, address: &str) -> Result<()> {
        info!(sl!(), "begin to connect agent {:?}", address);
        self.set_socket_address(address)
            .await
            .context("set socket")?;
        self.connect_agent_server()
            .await
            .context("connect agent server")?;
        self.start_log_forwarder().await
    }

    async fn stop(&self) {
        self.stop_log_forwarder().await;
    }

    async fn agent_sock(&self) -> Result<String> {
        self.agent_sock().await
    }

    async fn agent_config(&self) -> AgentConfig {
        self.agent_config().await
    }

    async fn disconnect(&self) -> Result<()> {
        self.disconnect().await.context("disconnect agent")
    }
}

// implement for health service
macro_rules! impl_health_service {
    ($($name: tt | $req: ty | $resp: ty),*) => {
        #[async_trait]
        impl HealthService for KataAgent {
            $(async fn $name(&self, req: $req) -> Result<$resp> {
                let r = req.into();
                let (client, timeout, _) = self.get_health_client().await.context("get health client")?;
                let resp = client.$name(new_ttrpc_ctx(timeout * MILLISECOND_TO_NANOSECOND), &r).await?;
                Ok(resp.into())
            })*
        }
    };
}

impl_health_service!(
    check | crate::CheckRequest | crate::HealthCheckResponse,
    version | crate::CheckRequest | crate::VersionCheckResponse
);

macro_rules! impl_agent {
    ($($name: tt | $req: ty | $resp: ty | $new_timeout: expr),*; $($retry:ident | $retry_req:ty | $retry_resp:ty),*) => {
        #[async_trait]
        impl Agent for KataAgent {
            #[instrument(skip(req))]
            $(async fn $name(&self, req: $req) -> Result<$resp> {
                let r = req.into();
                let (client, mut timeout, _) = self.get_agent_client().await.context("get client")?;

                // update new timeout
                if let Some(v) = $new_timeout {
                    timeout = v;
                }

                let resp = client.$name(new_ttrpc_ctx(timeout * MILLISECOND_TO_NANOSECOND), &r).await?;
                Ok(resp.into())
            })*

            $(async fn $retry(&self, req: $retry_req) -> Result<$retry_resp> {
                let req = req.into();
                loop {
                    let (client, generation) = self.reconnecting_client().await?;
                    match client.$retry(new_ttrpc_ctx(0), &req).await {
                        Ok(resp) => return Ok(resp.into()),
                        Err(err) => {
                            if self.reconnect_generation.load(Ordering::Acquire) == generation {
                                return Err(err.into());
                            }
                            // Only retry across an intentional transport reset.
                            // In particular, WriteStdin must never be replayed.
                        }
                    }
                }
            })*
        }
    };
}

impl_agent!(
    create_container | crate::CreateContainerRequest | crate::Empty | None,
    start_container | crate::ContainerID | crate::Empty | None,
    remove_container | crate::RemoveContainerRequest | crate::Empty | None,
    exec_process | crate::ExecProcessRequest | crate::Empty | None,
    signal_process | crate::SignalProcessRequest | crate::Empty | None,
    update_container | crate::UpdateContainerRequest | crate::Empty | None,
    stats_container | crate::ContainerID | crate::StatsContainerResponse | None,
    pause_container | crate::ContainerID | crate::Empty | None,
    resume_container | crate::ContainerID | crate::Empty | None,
    write_stdin | crate::WriteStreamRequest | crate::WriteStreamResponse | Some(0),
    close_stdin | crate::CloseStdinRequest | crate::Empty | None,
    tty_win_resize | crate::TtyWinResizeRequest | crate::Empty | None,
    update_interface | crate::UpdateInterfaceRequest | crate::Interface | None,
    update_routes | crate::UpdateRoutesRequest | crate::Routes | None,
    add_arp_neighbors | crate::AddArpNeighborRequest | crate::Empty | None,
    list_interfaces | crate::Empty | crate::Interfaces | None,
    list_routes | crate::Empty | crate::Routes | None,
    create_sandbox | crate::CreateSandboxRequest | crate::Empty | None,
    rebind_sandbox | crate::RebindSandboxRequest | crate::Empty | None,
    destroy_sandbox | crate::Empty | crate::Empty | None,
    reseed_random_dev | crate::ReseedRandomDevRequest | crate::Empty | None,
    set_guest_date_time | crate::SetGuestDateTimeRequest | crate::Empty | None,
    copy_file | crate::CopyFileRequest | crate::Empty | None,
    get_oom_event | crate::Empty | crate::OomEventResponse | Some(0),
    get_ip_tables | crate::GetIPTablesRequest | crate::GetIPTablesResponse | None,
    set_ip_tables | crate::SetIPTablesRequest | crate::SetIPTablesResponse | None,
    get_volume_stats | crate::VolumeStatsRequest | crate::VolumeStatsResponse | None,
    resize_volume | crate::ResizeVolumeRequest | crate::Empty | None,
    online_cpu_mem | crate::OnlineCPUMemRequest | crate::Empty | None,
    get_metrics | crate::Empty | crate::MetricsResponse | None,
    get_guest_details | crate::GetGuestDetailsRequest | crate::GuestDetailsResponse | None,
    add_swap | crate::AddSwapRequest | crate::Empty | None,
    add_swap_path | crate::AddSwapPathRequest | crate::Empty | None,
    set_policy | crate::SetPolicyRequest | crate::Empty | None,
    get_diagnostic_data | crate::GetDiagnosticDataRequest | crate::GetDiagnosticDataResponse | None;
    wait_process | crate::WaitProcessRequest | crate::WaitProcessResponse,
    read_stdout | crate::ReadStreamRequest | crate::ReadStreamResponse,
    read_stderr | crate::ReadStreamRequest | crate::ReadStreamResponse
);

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::{
        io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
        net::UnixListener,
        time::timeout,
    };

    #[tokio::test]
    async fn start_connects_rpc_and_log() {
        for (empty_eof, reply, succeeds) in [
            (false, "OK 42\n", true),
            (true, "OK 42\n", true),
            (false, "NO\n", false),
        ] {
            let path = std::env::temp_dir().join(format!(
                "kata-agent-start-{}-{empty_eof}-{succeeds}.sock",
                std::process::id()
            ));
            let listener = UnixListener::bind(&path).unwrap();
            let server = tokio::spawn(async move {
                let attempts = if empty_eof { 2 } else { 1 };
                let mut connected = None;
                for attempt in 0..attempts {
                    let (mut stream, _) = timeout(Duration::from_secs(2), listener.accept())
                        .await
                        .unwrap()
                        .unwrap();
                    let mut command = String::new();
                    BufReader::new(&mut stream)
                        .read_line(&mut command)
                        .await
                        .unwrap();
                    assert_eq!(command, "connect 1024\n");
                    if !empty_eof || attempt == 1 {
                        stream.write_all(reply.as_bytes()).await.unwrap();
                        connected = Some(stream);
                    }
                }
                let mut log_stream = None;
                if succeeds {
                    let (mut stream, _) = timeout(Duration::from_secs(2), listener.accept())
                        .await
                        .unwrap()
                        .unwrap();
                    let mut command = String::new();
                    BufReader::new(&mut stream)
                        .read_line(&mut command)
                        .await
                        .unwrap();
                    assert_eq!(command, "connect 1025\n");
                    stream.write_all(b"OK 43\n").await.unwrap();
                    log_stream = Some(stream);
                }
                // Failed RPC connections must not start logging; successful
                // connections start exactly one forwarder.
                assert!(timeout(Duration::from_millis(100), listener.accept())
                    .await
                    .is_err());
                drop(connected);
                drop(log_stream);
            });
            let agent: std::sync::Arc<dyn Agent> =
                std::sync::Arc::new(KataAgent::new(AgentConfig {
                    server_port: 1024,
                    log_port: 1025,
                    dial_timeout_ms: 1,
                    ..Default::default()
                }));
            let address = format!("hvsock://{}", path.display());
            let result = timeout(Duration::from_secs(2), agent.start(&address))
                .await
                .unwrap();
            assert_eq!(result.is_ok(), succeeds);
            server.await.unwrap();
            agent.disconnect().await.unwrap();
            std::fs::remove_file(path).unwrap();
        }
    }
}
