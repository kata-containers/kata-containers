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
} /* end of impl CDHClient */

#[cfg(test)]
#[cfg(feature = "sealed-secret")]
mod tests {
    use crate::cdh::CDHClient;
    use crate::cdh::CDH_ADDR;
    use anyhow::anyhow;
    use async_trait::async_trait;
    use protocols::{sealed_secret, sealed_secret_ttrpc_async};
    use std::sync::Arc;
    use tokio::signal::unix::{signal, SignalKind};

    struct TestService;

    #[async_trait]
    impl sealed_secret_ttrpc_async::SealedSecretService for TestService {
        async fn unseal_secret(
            &self,
            _ctx: &::ttrpc::asynchronous::TtrpcContext,
            _req: sealed_secret::UnsealSecretInput,
        ) -> ttrpc::error::Result<sealed_secret::UnsealSecretOutput> {
            let mut output = sealed_secret::UnsealSecretOutput::new();
            output.set_plaintext("unsealed".into());
            Ok(output)
        }
    }

    fn remove_if_sock_exist(sock_addr: &str) -> std::io::Result<()> {
        let path = sock_addr
            .strip_prefix("unix://")
            .expect("socket address does not have the expected format.");

        if std::path::Path::new(path).exists() {
            std::fs::remove_file(path)?;
        }

        Ok(())
    }

    fn start_ttrpc_server() {
        tokio::spawn(async move {
            let ss = Box::new(TestService {})
                as Box<dyn sealed_secret_ttrpc_async::SealedSecretService + Send + Sync>;
            let ss = Arc::new(ss);
            let ss_service = sealed_secret_ttrpc_async::create_sealed_secret_service(ss);

            remove_if_sock_exist(CDH_ADDR).unwrap();

            let mut server = ttrpc::asynchronous::Server::new()
                .bind(CDH_ADDR)
                .unwrap()
                .register_service(ss_service);

            server.start().await.unwrap();

            let mut interrupt = signal(SignalKind::interrupt()).unwrap();
            tokio::select! {
                _ = interrupt.recv() => {
                    server.shutdown().await.unwrap();
                }
            };
        });
    }

    #[tokio::test]
    async fn test_unseal_env() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let _guard = rt.enter();
        start_ttrpc_server();
        std::thread::sleep(std::time::Duration::from_secs(2));

        let cc = Some(CDHClient::new().unwrap());
        let cdh_client = cc.as_ref().ok_or(anyhow!("get cdh_client failed")).unwrap();
        let sealed_env = String::from("key=sealed.testdata");
        let unsealed_env = cdh_client.unseal_env(&sealed_env).await.unwrap();
        assert_eq!(unsealed_env, String::from("key=unsealed"));
        let normal_env = String::from("key=testdata");
        let unchanged_env = cdh_client.unseal_env(&normal_env).await.unwrap();
        assert_eq!(unchanged_env, String::from("key=testdata"));

        rt.shutdown_background();
    }
}
