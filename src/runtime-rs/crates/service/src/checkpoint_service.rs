// Copyright (c) 2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::{convert::TryInto, sync::Arc};

use async_trait::async_trait;
use common::error::map_runtime_error;
use common::types::utils::sandbox_operation_timeout;
use common::types::SandboxRequest;
use containerd_shim_protos::{sandbox_api, sandbox_async};
use runtimes::RuntimeHandlerManager;
use ttrpc::{self, r#async::TtrpcContext};

pub(crate) struct CheckpointService {
    handler: Arc<RuntimeHandlerManager>,
}

impl CheckpointService {
    pub(crate) fn new(handler: Arc<RuntimeHandlerManager>) -> Self {
        Self { handler }
    }
}

macro_rules! impl_checkpoint_service {
    ($($name: tt | $req: ty | $resp: ty),*) => {
        #[async_trait]
        impl sandbox_async::Checkpoint for CheckpointService {
            $(async fn $name(&self, ctx: &TtrpcContext, req: $req) -> ttrpc::Result<$resp> {
                let result: ttrpc::Result<$resp> = async {
                    let mut r: SandboxRequest = req.try_into().map_err(|err| {
                        ttrpc::Error::RpcStatus(ttrpc::get_status(
                            ttrpc::Code::INVALID_ARGUMENT,
                            format!("failed to translate checkpoint request: {err:?}"),
                        ))
                    })?;
                    if let SandboxRequest::CheckpointSandbox(checkpoint) = &mut r {
                        checkpoint.operation_timeout = sandbox_operation_timeout(ctx.timeout_nano);
                    }
                    if let SandboxRequest::RestoreSandbox(restore) = &mut r {
                        restore.operation_timeout = sandbox_operation_timeout(ctx.timeout_nano);
                    }
                    let handler = self.handler.clone();
                    let response = tokio::spawn(async move { handler.handler_sandbox_message(r).await })
                        .await
                        .map_err(|err| {
                            ttrpc::Error::RpcStatus(ttrpc::get_status(
                                ttrpc::Code::INTERNAL,
                                format!("checkpoint operation worker failed: {err}"),
                            ))
                        })?;
                    let resp = response.map_err(map_runtime_error)?;
                    resp.try_into()
                        .map_err(|err| ttrpc::Error::Others(format!("failed to translate to checkpoint response {err:?}")))
                }.await;
                result
            })*
        }
    };
}

impl_checkpoint_service!(
    checkpoint | sandbox_api::CheckpointRequest | sandbox_api::CheckpointResponse,
    restore | sandbox_api::RestoreRequest | sandbox_api::RestoreResponse
);

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn checkpoint_and_restore_rpc_reach_runtime_handler() {
        let directory = tempfile::tempdir().unwrap();
        let address = format!("unix://{}", directory.path().join("rpc.sock").display());
        let (sender, _receiver) = tokio::sync::mpsc::channel(1);
        let handler = Arc::new(RuntimeHandlerManager::new("rpc-test", sender).unwrap());
        let service = Arc::new(CheckpointService::new(handler));
        let mut server = ttrpc::asynchronous::Server::new()
            .bind(&address)
            .unwrap()
            .register_service(sandbox_async::create_checkpoint(service));
        server.start().await.unwrap();
        let connection = ttrpc::asynchronous::Client::connect(&address)
            .await
            .unwrap();
        let client = sandbox_async::CheckpointClient::new(connection);
        let context = ttrpc::context::with_timeout(5_000_000_000);

        let checkpoint_error = client
            .checkpoint(context.clone(), &sandbox_api::CheckpointRequest::new())
            .await
            .expect_err("checkpoint requires a running sandbox");
        let mut restore = sandbox_api::RestoreRequest::new();
        restore.sandbox_id = "other-sandbox".into();
        let restore_error = client
            .restore(context, &restore)
            .await
            .expect_err("restore requires the matching sandbox id");
        server.shutdown().await.unwrap();

        for (error, expected_code, expected_message) in [
            (checkpoint_error, ttrpc::Code::INTERNAL, "runtime not ready"),
            (
                restore_error,
                ttrpc::Code::INVALID_ARGUMENT,
                "does not match shim id",
            ),
        ] {
            let ttrpc::Error::RpcStatus(status) = error else {
                panic!("expected an RPC status");
            };
            assert_eq!(status.code(), expected_code);
            assert!(status.message().contains(expected_message));
        }
    }
}
