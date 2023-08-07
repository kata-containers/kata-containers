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

#[cfg(test)]
#[cfg(feature = "sealed-secret")]
mod tests {
    use crate::cdh::CDHClient;
    use crate::cdh::CDH_ADDR;
    use anyhow::anyhow;
    use async_trait::async_trait;
    use protocols::{sealed_secret, sealed_secret_ttrpc_async};
    use std::sync::Arc;
    use test_utils::skip_if_not_root;
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
        skip_if_not_root!();

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
        std::thread::sleep(std::time::Duration::from_secs(2));
    }
}
