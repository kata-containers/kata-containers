// Copyright (c) 2023 Alibaba Cloud
//
// SPDX-License-Identifier: Apache-2.0
//

//! Get Rserouce gRPC client

use anyhow::*;
use async_trait::async_trait;
use tonic::transport::Channel;

use self::get_resource::{
    get_resource_service_client::GetResourceServiceClient, GetResourceRequest,
};

use super::Client;

mod get_resource {
    #![allow(unknown_lints)]
    #![allow(clippy::derive_partial_eq_without_eq)]
    tonic::include_proto!("getresource");
}

/// Attestation Agent's GetResource gRPC address.
/// It's given <https://github.com/confidential-containers/attestation-agent#run>
pub const AA_GETRESOURCE_ADDR: &str = "http://127.0.0.1:50001";

pub struct Grpc {
    inner: GetResourceServiceClient<Channel>,
}

impl Grpc {
    pub async fn new() -> Result<Self> {
        let inner = GetResourceServiceClient::connect(AA_GETRESOURCE_ADDR).await?;
        Ok(Self { inner })
    }
}

#[async_trait]
impl Client for Grpc {
    async fn get_resource(
        &mut self,
        kbc_name: &str,
        kbs_uri: &str,
        resource_description: String,
    ) -> Result<Vec<u8>> {
        let req = tonic::Request::new(GetResourceRequest {
            kbc_name: kbc_name.to_string(),
            kbs_uri: kbs_uri.to_string(),
            resource_description,
        });
        Ok(self.inner.get_resource(req).await?.into_inner().resource)
    }
}
