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
use common::error::Error as CommonError;
use common::types::{SandboxRequest, SandboxResponse};
use common::types::utils::sandbox_operation_timeout;
use containerd_shim_protos::{sandbox_api, sandbox_async};
use runtimes::RuntimeHandlerManager;
use ttrpc::{self, r#async::TtrpcContext};

/// map common runtime error to ttrpc CODE
fn map_runtime_error(err: anyhow::Error) -> ttrpc::Error {
    let code = match err.downcast_ref::<CommonError>() {
        Some(CommonError::InvalidSandboxOperation(_)) => ttrpc::Code::INVALID_ARGUMENT,
        Some(CommonError::SandboxOperationPrecondition(_)) => ttrpc::Code::FAILED_PRECONDITION,
        Some(CommonError::SandboxOperationUnsupported(_)) => ttrpc::Code::UNIMPLEMENTED,
        Some(CommonError::SandboxOperationDeadline(_)) => ttrpc::Code::DEADLINE_EXCEEDED,
        _ => ttrpc::Code::INTERNAL,
    };
    ttrpc::Error::RpcStatus(ttrpc::get_status(
        code,
        format!("failed to handle sandbox message: {err:#}"),
    ))
}

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
        let mut r = req.try_into().map_err(|err| {
            ttrpc::Error::RpcStatus(ttrpc::get_status(
                ttrpc::Code::INVALID_ARGUMENT,
                format!("failed to translate sandbox request: {err:?}"),
            ))
        })?;
        let is_transactional_operation = matches!(
            &r,
            SandboxRequest::CheckpointSandbox(_) | SandboxRequest::RestoreSandbox(_)
        );
        if let SandboxRequest::CheckpointSandbox(checkpoint) = &mut r {
            checkpoint.operation_timeout = sandbox_operation_timeout(ctx.timeout_nano);
        }
        if let SandboxRequest::RestoreSandbox(restore) = &mut r {
            restore.operation_timeout = sandbox_operation_timeout(ctx.timeout_nano);
        }
        let logger = sl!().new(o!("stream id" =>  ctx.mh.stream_id));
        debug!(logger, "====> sandbox service {:?}", &r);
        let response = if is_transactional_operation {
            // ttrpc drops the handler future when its transport deadline expires.
            // Detach checkpoint/restore execution so its resume/rollback path
            // still runs and no half-finished sandbox is left behind.
            let handler = self.handler.clone();
            tokio::spawn(async move { handler.handler_sandbox_message(r).await })
                .await
                .map_err(|err| {
                    ttrpc::Error::RpcStatus(ttrpc::get_status(
                        ttrpc::Code::INTERNAL,
                        format!("sandbox operation worker failed: {err}"),
                    ))
                })?
        } else {
            self.handler.handler_sandbox_message(r).await
        };
        let resp = response.map_err(map_runtime_error)?;
        debug!(logger, "<==== sandbox service {:?}", &resp);
        resp.try_into()
            .map_err(|err| ttrpc::Error::Others(format!("failed to translate to shim {err:?}")))
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
    shutdown_sandbox | sandbox_api::ShutdownSandboxRequest | sandbox_api::ShutdownSandboxResponse,
    checkpoint_sandbox | sandbox_api::CheckpointSandboxRequest | sandbox_api::CheckpointSandboxResponse,
    restore_sandbox | sandbox_api::RestoreSandboxRequest | sandbox_api::RestoreSandboxResponse
);
