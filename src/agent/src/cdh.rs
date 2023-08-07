// Copyright (c) 2023 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

// Confidential Data Hub client wrapper.
// Confidential Data Hub is a service running inside guest to provide resource related APIs.
// https://github.com/confidential-containers/guest-components/tree/main/confidential-data-hub

use anyhow::{anyhow, Result};
use protocols::{
    sealed_secret, sealed_secret_ttrpc_async, sealed_secret_ttrpc_async::SealedSecretServiceClient,
};
const CDH_ADDR: &str = "unix:///run/confidential-containers/cdh.sock";

#[derive(Clone)]
pub struct CDHClient {
    sealed_secret_client: SealedSecretServiceClient,
}

impl CDHClient {
    pub fn new() -> Result<Self> {
        let c = ttrpc::asynchronous::Client::connect(CDH_ADDR)?;
        let ssclient = sealed_secret_ttrpc_async::SealedSecretServiceClient::new(c);
        Ok(CDHClient {
            sealed_secret_client: ssclient,
        })
    }

    pub async fn unseal_secret_async(
        &self,
        sealed: &str,
    ) -> Result<sealed_secret::UnsealSecretOutput> {
        let secret = sealed
            .strip_prefix("sealed.")
            .ok_or(anyhow!("strip_prefix sealed. failed"))?;
        let mut input = sealed_secret::UnsealSecretInput::new();
        input.set_secret(secret.into());
        let unseal = self
            .sealed_secret_client
            .unseal_secret(
                ttrpc::context::with_timeout(50 * 1000 * 1000 * 1000),
                &input,
            )
            .await?;
        Ok(unseal)
    }

    pub async fn unseal_env(&self, env: &str) -> Result<String> {
        let (key, value) = env.split_once('=').unwrap();
        if value.starts_with("sealed.") {
            let unsealed_value = self.unseal_secret_async(value).await;
            match unsealed_value {
                Ok(v) => {
                    let plain_env =
                        format!("{}={}", key, std::str::from_utf8(&v.plaintext).unwrap());
                    return Ok(plain_env);
                }
                Err(e) => {
                    return Err(e);
                }
            };
        }
        Ok((*env.to_owned()).to_string())
    }
}
