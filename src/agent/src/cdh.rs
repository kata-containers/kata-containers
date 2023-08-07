// Copyright (c) 2023 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

// Confidential Data Hub client wrapper.
// Confidential Data Hub is a service running inside guest to provide resource related APIs.
// https://github.com/confidential-containers/guest-components/tree/main/confidential-data-hub

use anyhow::Result;
use derivative::Derivative;
use protocols::{
    sealed_secret, sealed_secret_ttrpc_async, sealed_secret_ttrpc_async::SealedSecretServiceClient,
};

use crate::CDH_SOCKET_URI;

// Nanoseconds
const CDH_UNSEAL_TIMEOUT: i64 = 50 * 1000 * 1000 * 1000;
const SEALED_SECRET_PREFIX: &str = "sealed.";

#[derive(Derivative)]
#[derivative(Clone, Debug)]
pub struct CDHClient {
    #[derivative(Debug = "ignore")]
    sealed_secret_client: SealedSecretServiceClient,
}

impl CDHClient {
    pub fn new() -> Result<Self> {
        let client = ttrpc::asynchronous::Client::connect(CDH_SOCKET_URI)?;
        let sealed_secret_client =
            sealed_secret_ttrpc_async::SealedSecretServiceClient::new(client);

        Ok(CDHClient {
            sealed_secret_client,
        })
    }

    pub async fn unseal_secret_async(&self, sealed_secret: &str) -> Result<Vec<u8>> {
        let mut input = sealed_secret::UnsealSecretInput::new();
        input.set_secret(sealed_secret.into());

        let unsealed_secret = self
            .sealed_secret_client
            .unseal_secret(ttrpc::context::with_timeout(CDH_UNSEAL_TIMEOUT), &input)
            .await?;
        Ok(unsealed_secret.plaintext)
    }

    pub async fn unseal_env(&self, env: &str) -> Result<String> {
        if let Some((key, value)) = env.split_once('=') {
            if value.starts_with(SEALED_SECRET_PREFIX) {
                let unsealed_value = self.unseal_secret_async(value).await?;
                let unsealed_env = format!("{}={}", key, std::str::from_utf8(&unsealed_value)?);

                return Ok(unsealed_env);
            }
        }
        Ok((*env.to_owned()).to_string())
    }
}
