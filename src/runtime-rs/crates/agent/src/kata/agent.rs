// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

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
        self.start_log_forwarder()
            .await
            .context("connect log forwarder")?;
        Ok(())
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
    ($($name: tt | $req: ty | $resp: ty | $new_timeout: expr),*) => {
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
        }
    };
}

impl_agent!(
    create_container | crate::CreateContainerRequest | crate::Empty | None,
    start_container | crate::ContainerID | crate::Empty | None,
    remove_container | crate::RemoveContainerRequest | crate::Empty | None,
    exec_process | crate::ExecProcessRequest | crate::Empty | None,
    signal_process | crate::SignalProcessRequest | crate::Empty | None,
    wait_process | crate::WaitProcessRequest | crate::WaitProcessResponse | Some(0),
    update_container | crate::UpdateContainerRequest | crate::Empty | None,
    stats_container | crate::ContainerID | crate::StatsContainerResponse | None,
    pause_container | crate::ContainerID | crate::Empty | None,
    resume_container | crate::ContainerID | crate::Empty | None,
    write_stdin | crate::WriteStreamRequest | crate::WriteStreamResponse | Some(0),
    read_stdout | crate::ReadStreamRequest | crate::ReadStreamResponse | Some(0),
    read_stderr | crate::ReadStreamRequest | crate::ReadStreamResponse | Some(0),
    close_stdin | crate::CloseStdinRequest | crate::Empty | None,
    tty_win_resize | crate::TtyWinResizeRequest | crate::Empty | None,
    update_interface | crate::UpdateInterfaceRequest | crate::Interface | None,
    update_routes | crate::UpdateRoutesRequest | crate::Routes | None,
    add_arp_neighbors | crate::AddArpNeighborRequest | crate::Empty | None,
    list_interfaces | crate::Empty | crate::Interfaces | None,
    list_routes | crate::Empty | crate::Routes | None,
    create_sandbox | crate::CreateSandboxRequest | crate::Empty | None,
    destroy_sandbox | crate::Empty | crate::Empty | None,
    copy_file | crate::CopyFileRequest | crate::Empty | None,
    get_oom_event | crate::Empty | crate::OomEventResponse | Some(0),
    get_ip_tables | crate::GetIPTablesRequest | crate::GetIPTablesResponse | None,
    set_ip_tables | crate::SetIPTablesRequest | crate::SetIPTablesResponse | None,
    get_volume_stats | crate::VolumeStatsRequest | crate::VolumeStatsResponse | None,
    resize_volume | crate::ResizeVolumeRequest | crate::Empty | None,
    online_cpu_mem | crate::OnlineCPUMemRequest | crate::Empty | None,
    get_metrics | crate::Empty | crate::MetricsResponse | None,
    get_guest_details | crate::GetGuestDetailsRequest | crate::GuestDetailsResponse | None
);
