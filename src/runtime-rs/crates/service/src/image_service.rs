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
use common::types::{Request, Response};
use ttrpc::{self, r#async::TtrpcContext};

use protocols::{image_runtime, image_runtime_ttrpc_async};

use runtimes::RuntimeHandlerManager;

pub(crate) struct ImageService {
    handler: Arc<RuntimeHandlerManager>,
}

impl ImageService {
    pub(crate) fn new(handler: Arc<RuntimeHandlerManager>) -> Self {
        Self { handler }
    }
}

async fn handler_message<TtrpcReq, TtrpcResp>(
    s: &RuntimeHandlerManager,
    ctx: &TtrpcContext,
    req: TtrpcReq,
) -> ttrpc::Result<TtrpcResp>
where
    Request: TryFrom<TtrpcReq>,
    <Request as TryFrom<TtrpcReq>>::Error: std::fmt::Debug,
    TtrpcResp: TryFrom<Response>,
    <TtrpcResp as TryFrom<Response>>::Error: std::fmt::Debug,
{
    let r = req
        .try_into()
        .map_err(|err| ttrpc::Error::Others(format!("failed to translate from shim {:?}", err)))?;
    let logger = sl!().new(o!("stream id" =>  ctx.mh.stream_id));
    debug!(logger, "====> image service {:?}", &r);
    let resp = s
        .handler_message(r)
        .await
        .map_err(|err| ttrpc::Error::Others(format!("failed to handler message {:?}", err)))?;
    debug!(logger, "<==== image service {:?}", &resp);
    resp.try_into()
        .map_err(|err| ttrpc::Error::Others(format!("failed to translate to shim {:?}", err)))
}

macro_rules! impl_service {
    ($($name: tt | $req: ty | $resp: ty),*) => {
        #[async_trait]
        impl image_runtime_ttrpc_async::Image for ImageService {
            $(async fn $name(&self, ctx: &TtrpcContext, req: $req) -> ttrpc::Result<$resp> {
                handler_message(&self.handler, ctx, req).await
            })*
        }
    };
}

impl_service!(pull_image | image_runtime::PullImageRequest | image_runtime::PullImageResponse);
