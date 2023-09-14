// Copyright (c) 2023 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

// Confidential Data Hub client wrapper.
// Confidential Data Hub is a service running inside guest to provide resource related APIs.
// https://github.com/confidential-containers/guest-components/tree/main/confidential-data-hub

use anyhow::{anyhow, Result};
use oci::{Mount, Spec};
use protocols::{
    confidential_data_hub, confidential_data_hub_ttrpc_async,
    confidential_data_hub_ttrpc_async::SealedSecretServiceClient,
    confidential_data_hub_ttrpc_async::SecureMountServiceClient,
};
use std::fs;
use std::os::unix::fs::symlink;
use std::path::Path;
const CDH_ADDR: &str = "unix:///run/confidential-containers/cdh.sock";
const SECRETS_DIR: &str = "/run/secrets/";
const SEALED_SECRET_TIMEOUT: i64 = 50 * 1000 * 1000 * 1000;

// Convenience function to obtain the scope logger.
fn sl() -> slog::Logger {
    slog_scope::logger()
}

#[derive(Clone)]
pub struct CDHClient {
    sealed_secret_client: Option<SealedSecretServiceClient>,
    secure_mount_client: Option<SecureMountServiceClient>,
}

impl CDHClient {
    pub fn new() -> Result<Self> {
        let c = ttrpc::asynchronous::Client::connect(CDH_ADDR);
        match c {
            Ok(v) => {
                let ssclient =
                    confidential_data_hub_ttrpc_async::SealedSecretServiceClient::new(v.clone());
                let smclient = confidential_data_hub_ttrpc_async::SecureMountServiceClient::new(v);
                Ok(CDHClient {
                    sealed_secret_client: Some(ssclient),
                    secure_mount_client: Some(smclient),
                })
            }
            Err(_) => Ok(CDHClient {
                sealed_secret_client: None,
                secure_mount_client: None,
            }),
        }
    }

    pub async fn secure_mount_async(
        &self,
        driver: String,
        driver_options: Vec<String>,
        source: String,
        fstype: String,
        options: Vec<String>,
        mount_point: String,
    ) -> Result<confidential_data_hub::SecureMountResponse> {
        let mut req = confidential_data_hub::SecureMountRequest::new();
        req.set_driver(driver);
        req.set_driver_options(driver_options);
        req.set_source(source);
        req.set_fstype(fstype);
        req.set_options(options);
        req.set_mount_point(mount_point);
        let mount_path = self
            .secure_mount_client
            .as_ref()
            .ok_or(anyhow!("unwrap secure_mount_client failed"))?
            .secure_mount(ttrpc::context::with_timeout(SEALED_SECRET_TIMEOUT), &req)
            .await?;
        Ok(mount_path)
    }

    pub async fn unseal_secret_async(
        &self,
        sealed: &str,
    ) -> Result<confidential_data_hub::UnsealSecretOutput> {
        let secret = sealed
            .strip_prefix("sealed.")
            .ok_or(anyhow!("strip_prefix \"sealed.\" failed"))?;
        let mut input = confidential_data_hub::UnsealSecretInput::new();
        input.set_secret(secret.into());
        let unseal = self
            .sealed_secret_client
            .as_ref()
            .ok_or(anyhow!("unwrap sealed_secret_client failed"))?
            .unseal_secret(ttrpc::context::with_timeout(SEALED_SECRET_TIMEOUT), &input)
            .await?;
        Ok(unseal)
    }

    pub async fn unseal_env(&self, env: &str) -> Result<String> {
        let (key, value) = env.split_once('=').unwrap_or(("", ""));
        if value.starts_with("sealed.") {
            let unsealed_value = self.unseal_secret_async(value).await;
            match unsealed_value {
                Ok(v) => {
                    let plain_env = format!("{}={}", key, std::str::from_utf8(&v.plaintext)?);
                    return Ok(plain_env);
                }
                Err(e) => {
                    return Err(e);
                }
            };
        }
        Ok((*env.to_owned()).to_string())
    }

    pub async fn unseal_file(&self, sealed_source_path: &String) -> Result<()> {
        if !Path::new(sealed_source_path).exists() {
            info!(
                sl(),
                "sealed source path {:?} does not exist", sealed_source_path
            );
            return Ok(());
        }

        for entry in fs::read_dir(sealed_source_path)? {
            let entry = entry?;

            if !entry.file_type()?.is_symlink()
                && !fs::metadata(entry.path())?.file_type().is_file()
            {
                info!(
                    sl(),
                    "skipping sealed source entry {:?} because its file type is {:?}",
                    entry,
                    entry.file_type()?
                );
                continue;
            }

            let target_path = fs::canonicalize(&entry.path())?;
            info!(sl(), "sealed source entry target path: {:?}", target_path);
            if !target_path.is_file() {
                info!(sl(), "sealed source is not a file: {:?}", target_path);
                continue;
            }

            let secret_name = entry.file_name();
            let contents = fs::read_to_string(&target_path)?;
            if contents.starts_with("sealed.") {
                info!(sl(), "sealed source entry found: {:?}", target_path);
                let unsealed_filename = SECRETS_DIR.to_string()
                    + secret_name
                        .as_os_str()
                        .to_str()
                        .ok_or(anyhow!("create unsealed_filename failed"))?;
                let unsealed_value = self.unseal_secret_async(&contents).await?;
                fs::write(&unsealed_filename, unsealed_value.plaintext)?;
                fs::remove_file(&entry.path())?;
                symlink(unsealed_filename, &entry.path())?;
            }
        }
        Ok(())
    }

    pub fn create_sealed_secret_mounts(&self, spec: &mut Spec) -> Result<Vec<String>> {
        let mut sealed_source_path: Vec<String> = vec![];
        for m in spec.mounts.iter_mut() {
            if let Some(unsealed_mount_point) = m.destination.strip_prefix("/sealed") {
                info!(
                    sl(),
                    "sealed mount destination: {:?} source: {:?}", m.destination, m.source
                );
                sealed_source_path.push(m.source.clone());
                m.destination = unsealed_mount_point.to_string();
            }
        }

        if !sealed_source_path.is_empty() {
            let sealed_mounts = Mount {
                destination: SECRETS_DIR.to_string(),
                r#type: "bind".to_string(),
                source: SECRETS_DIR.to_string(),
                options: vec!["bind".to_string()],
            };
            spec.mounts.push(sealed_mounts);
        }
        fs::create_dir_all(SECRETS_DIR)?;
        Ok(sealed_source_path)
    }
} /* end of impl CDHClient */

#[cfg(test)]
#[cfg(feature = "sealed-secret")]
mod tests {
    use crate::cdh::CDHClient;
    use crate::cdh::CDH_ADDR;
    use crate::cdh::SECRETS_DIR;
    use anyhow::anyhow;
    use async_trait::async_trait;
    use protocols::{confidential_data_hub, confidential_data_hub_ttrpc_async};
    use std::fs;
    use std::fs::File;
    use std::io::{Read, Write};
    use std::path::Path;
    use std::sync::Arc;
    use tokio::signal::unix::{signal, SignalKind};

    struct TestService;

    #[async_trait]
    impl confidential_data_hub_ttrpc_async::SealedSecretService for TestService {
        async fn unseal_secret(
            &self,
            _ctx: &::ttrpc::asynchronous::TtrpcContext,
            _req: confidential_data_hub::UnsealSecretInput,
        ) -> ttrpc::error::Result<confidential_data_hub::UnsealSecretOutput> {
            let mut output = confidential_data_hub::UnsealSecretOutput::new();
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
                as Box<dyn confidential_data_hub_ttrpc_async::SealedSecretService + Send + Sync>;
            let ss = Arc::new(ss);
            let ss_service = confidential_data_hub_ttrpc_async::create_sealed_secret_service(ss);

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
        let cdh_client = cc
            .as_ref()
            .ok_or(anyhow!("get confidential-data-hub client failed"))
            .unwrap();
        let sealed_env = String::from("key=sealed.testdata");
        let unsealed_env = cdh_client.unseal_env(&sealed_env).await.unwrap();
        assert_eq!(unsealed_env, String::from("key=unsealed"));
        let normal_env = String::from("key=testdata");
        let unchanged_env = cdh_client.unseal_env(&normal_env).await.unwrap();
        assert_eq!(unchanged_env, String::from("key=testdata"));

        rt.shutdown_background();
        std::thread::sleep(std::time::Duration::from_secs(2));
    }

    #[tokio::test]
    async fn test_unseal_file() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let _guard = rt.enter();
        start_ttrpc_server();
        std::thread::sleep(std::time::Duration::from_secs(2));

        let cc = Some(CDHClient::new().unwrap());
        let cdh_client = cc
            .as_ref()
            .ok_or(anyhow!("get confidential-data-hub client failed"))
            .unwrap();

        fs::create_dir_all(SECRETS_DIR).unwrap();

        let sealed_filename = "passwd";
        let mut sealed_file = File::create(sealed_filename).unwrap();
        let dir = String::from(".");
        sealed_file.write_all(b"sealed.passwd").unwrap();
        cdh_client.unseal_file(&dir).await.unwrap();
        let unsealed_filename = SECRETS_DIR.to_string() + "/passwd";
        let mut unsealed_file = fs::File::open(unsealed_filename.clone()).unwrap();
        let mut contents = String::new();
        unsealed_file.read_to_string(&mut contents).unwrap();
        assert_eq!(contents, String::from("unsealed"));
        fs::remove_file(sealed_filename).unwrap();
        fs::remove_file(unsealed_filename).unwrap();

        let normal_filename = "passwd";
        let mut normal_file = File::create(normal_filename).unwrap();
        normal_file.write_all(b"passwd").unwrap();
        cdh_client.unseal_file(&dir).await.unwrap();
        let filename = SECRETS_DIR.to_string() + "/passwd";
        assert!(!Path::new(&filename).exists());
        fs::remove_file(normal_filename).unwrap();

        rt.shutdown_background();
        std::thread::sleep(std::time::Duration::from_secs(2));
    }
}
