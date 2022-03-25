// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{Context, Result};
use async_trait::async_trait;
use ttrpc::context as ttrpc_ctx;

use crate::{kata::KataAgent, Agent, AgentManager, HealthService};

/// millisecond to nanosecond
const MILLISECOND_TO_NANOSECOND: i64 = 1_000_000;

/// new ttrpc context with timeout
fn new_ttrpc_ctx(timeout: i64) -> ttrpc_ctx::Context {
    ttrpc_ctx::with_timeout(timeout)
}

#[async_trait]
impl AgentManager for KataAgent {
    async fn set_socket_address(&self, address: &str) -> Result<()> {
        let mut inner = self.inner.lock().await;
        inner.socket_address = address.to_string();
        Ok(())
    }

    async fn start(&self) -> Result<()> {
        info!(sl!(), "begin to connect agent");
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
}

// implement for health service
macro_rules! impl_health_service {
    ($(impl $name:ident ($req:ident, $resp:ident)),*) => {
        #[async_trait]
        impl HealthService for KataAgent {
            $(async fn $name(&self, req: crate::$req) -> Result<crate::$resp> {
                let r = req.into();
                let (mut client, timeout, _) = self.get_health_client().await.context("get health client")?;
                let resp = client.$name(new_ttrpc_ctx(timeout * MILLISECOND_TO_NANOSECOND), &r).await?;
                Ok(resp.into())
            })*
        }
    };
}

impl_health_service!(
    impl check (CheckRequest, HealthCheckResponse),
    impl version (CheckRequest, VersionCheckResponse)
);

macro_rules! impl_agent {
    ($(impl $name:ident ($req:ident, $resp:ident) await $new_timeout:expr),*) => {
        #[async_trait]
        impl Agent for KataAgent {
            $(async fn $name(&self, req: crate::$req) -> Result<crate::$resp> {
                let r = reeq.into();
                let (mut client, mut timeout, _) = self.get_agent_client().await.context("get client")?;

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
    impl create_container (CreateContainerRequest, Empty) await None,
    impl start_container (ContainerID, Empty) await None,
    impl remove_container (RemoveContainerRequest, Empty) await None,
    impl exec_process (ExecProcessRequest, Empty) await None,
    impl signal_process (SignalProcessRequest, Empty) await None,
    impl wait_process (WaitProcessRequest, WaitProcessResponse) await Some(0),
    impl update_container (UpdateContainerRequest, Empty) await None,
    impl stats_container (ContainerID, StatsContainerResponse) await None,
    impl pause_container (ContainerID, Empty) await None,
    impl resume_container (ContainerID, Empty) await None,
    impl write_stdin (WriteStreamRequest, WriteStreamResponse) await None,
    impl read_stdout (ReadStreamRequest, ReadStreamResponse) await None,
    impl read_stderr (ReadStreamRequest, ReadStreamResponse) await None,
    impl close_stdin (CloseStdinRequest, Empty) await None,
    impl tty_win_resize (TtyWinResizeRequest, Empty) await None,
    impl update_interface (UpdateInterfaceRequest, Interface) await None,
    impl update_routes (UpdateRoutesRequest, Routes) await None,
    impl add_arp_neighbors (AddArpNeighborRequest, Empty) await None,
    impl list_interfaces (Empty, Interfaces) await None,
    impl list_routes (Empty, Routes) await None,
    impl create_sandbox (CreateSandboxRequest, Empty) await None,
    impl destroy_sandbox (Empty, Empty) await None,
    impl copy_file (CopyFileRequest, Empty) await None,
    impl get_oom_event (Empty, OomEventResponse) await Some(0)
);
