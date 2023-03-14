// Copyright (c) 2023 Alibaba Cloud
//
// SPDX-License-Identifier: Apache-2.0
//

//! Get Rserouce native AA client

use anyhow::*;
use async_trait::async_trait;
use attestation_agent::AttestationAPIs;
use attestation_agent::AttestationAgent;

use super::Client;

#[derive(Default)]
pub struct Native {
    inner: AttestationAgent,
}

#[async_trait]
impl Client for Native {
    async fn get_resource(
        &mut self,
        kbc_name: &str,
        kbs_uri: &str,
        resource_description: String,
    ) -> Result<Vec<u8>> {
        self.inner
            .download_confidential_resource(kbc_name, kbs_uri, &resource_description)
            .await
    }
}
