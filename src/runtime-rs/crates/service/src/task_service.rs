// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::{
    convert::{TryFrom, TryInto},
    sync::Arc,
};

use async_trait::async_trait;
use common::types::{TaskRequest, TaskResponse};
use containerd_shim_protos::{api, shim_async};
use ttrpc::{self, r#async::TtrpcContext};

use runtimes::RuntimeHandlerManager;

pub(crate) struct TaskService {
    handler: Arc<RuntimeHandlerManager>,
}

impl TaskService {
    pub(crate) fn new(handler: Arc<RuntimeHandlerManager>) -> Self {
        Self { handler }
    }

    async fn handler_message<TtrpcReq, TtrpcResp>(
        &self,
        ctx: &TtrpcContext,
        req: TtrpcReq,
    ) -> ttrpc::Result<TtrpcResp>
    where
        TaskRequest: TryFrom<TtrpcReq>,
        <TaskRequest as TryFrom<TtrpcReq>>::Error: std::fmt::Debug,
        TtrpcResp: TryFrom<TaskResponse>,
        <TtrpcResp as TryFrom<TaskResponse>>::Error: std::fmt::Debug,
    {
        let r = req.try_into().map_err(|err| {
            ttrpc::Error::Others(format!("failed to translate from shim {:?}", err))
        })?;
        let logger = sl!().new(o!("stream id" =>  ctx.mh.stream_id));
        debug!(logger, "====> task service {:?}", &r);
        let resp =
            self.handler.handler_task_message(r).await.map_err(|err| {
                ttrpc::Error::Others(format!("failed to handle message {:?}", err))
            })?;
        debug!(logger, "<==== task service {:?}", &resp);
        resp.try_into()
            .map_err(|err| ttrpc::Error::Others(format!("failed to translate to shim {:?}", err)))
    }
}

macro_rules! impl_service {
    ($($name: tt | $req: ty | $resp: ty),*) => {
        #[async_trait]
        impl shim_async::Task for TaskService {
            $(async fn $name(&self, ctx: &TtrpcContext, req: $req) -> ttrpc::Result<$resp> {
                self.handler_message(ctx, req).await
            })*
        }
    };
}

impl_service!(
    state | api::StateRequest | api::StateResponse,
    create | api::CreateTaskRequest | api::CreateTaskResponse,
    start | api::StartRequest | api::StartResponse,
    delete | api::DeleteRequest | api::DeleteResponse,
    pids | api::PidsRequest | api::PidsResponse,
    pause | api::PauseRequest | api::Empty,
    resume | api::ResumeRequest | api::Empty,
    kill | api::KillRequest | api::Empty,
    exec | api::ExecProcessRequest | api::Empty,
    resize_pty | api::ResizePtyRequest | api::Empty,
    update | api::UpdateTaskRequest | api::Empty,
    wait | api::WaitRequest | api::WaitResponse,
    stats | api::StatsRequest | api::StatsResponse,
    connect | api::ConnectRequest | api::ConnectResponse,
    shutdown | api::ShutdownRequest | api::Empty,
    close_io | api::CloseIORequest | api::Empty
);
