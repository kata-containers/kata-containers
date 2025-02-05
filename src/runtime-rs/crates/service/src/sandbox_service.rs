// Copyright (c) 2019-2025 Alibaba Cloud
// Copyright (c) 2019-2025 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::{
    convert::{TryFrom, TryInto},
    sync::Arc,
};

use async_trait::async_trait;
use common::types::{SandboxRequest, SandboxResponse};
use containerd_shim_protos::{sandbox_api, sandbox_async};
use runtimes::RuntimeHandlerManager;
use ttrpc::{self, r#async::TtrpcContext};

pub(crate) struct SandboxService {
    handler: Arc<RuntimeHandlerManager>,
}

impl SandboxService {
    pub(crate) fn new(handler: Arc<RuntimeHandlerManager>) -> Self {
        Self { handler }
    }

    async fn handler_message<TtrpcReq, TtrpcResp>(
        &self,
        ctx: &TtrpcContext,
        req: TtrpcReq,
    ) -> ttrpc::Result<TtrpcResp>
    where
        SandboxRequest: TryFrom<TtrpcReq>,
        <SandboxRequest as TryFrom<TtrpcReq>>::Error: std::fmt::Debug,
        TtrpcResp: TryFrom<SandboxResponse>,
        <TtrpcResp as TryFrom<SandboxResponse>>::Error: std::fmt::Debug,
    {
        let r = req.try_into().map_err(|err| {
            ttrpc::Error::Others(format!("failed to translate from shim {:?}", err))
        })?;
        let logger = sl!().new(o!("stream id" =>  ctx.mh.stream_id));
        debug!(logger, "====> sandbox service {:?}", &r);
        let resp = self
            .handler
            .handler_sandbox_message(r)
            .await
            .map_err(|err| {
                ttrpc::Error::Others(format!("failed to handle sandbox message {:?}", err))
            })?;
        debug!(logger, "<==== sandbox service {:?}", &resp);
        resp.try_into()
            .map_err(|err| ttrpc::Error::Others(format!("failed to translate to shim {:?}", err)))
    }
}

macro_rules! impl_service {
    ($($name: tt | $req: ty | $resp: ty),*) => {
        #[async_trait]
        impl sandbox_async::Sandbox for SandboxService {
            $(async fn $name(&self, ctx: &TtrpcContext, req: $req) -> ttrpc::Result<$resp> {
                self.handler_message(ctx, req).await
            })*
        }
    };
}

impl_service!(
    create_sandbox | sandbox_api::CreateSandboxRequest | sandbox_api::CreateSandboxResponse,
    start_sandbox | sandbox_api::StartSandboxRequest | sandbox_api::StartSandboxResponse,
    platform | sandbox_api::PlatformRequest | sandbox_api::PlatformResponse,
    stop_sandbox | sandbox_api::StopSandboxRequest | sandbox_api::StopSandboxResponse,
    wait_sandbox | sandbox_api::WaitSandboxRequest | sandbox_api::WaitSandboxResponse,
    sandbox_status | sandbox_api::SandboxStatusRequest | sandbox_api::SandboxStatusResponse,
    ping_sandbox | sandbox_api::PingRequest | sandbox_api::PingResponse,
    shutdown_sandbox | sandbox_api::ShutdownSandboxRequest | sandbox_api::ShutdownSandboxResponse
);
