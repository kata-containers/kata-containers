// Copyright (c) 2023 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

// Confidential Data Hub client wrapper.
// Confidential Data Hub is a service running inside guest to provide resource related APIs.
// https://github.com/confidential-containers/guest-components/tree/main/confidential-data-hub

use crate::AGENT_CONFIG;
use anyhow::{bail, Context, Result};
use derivative::Derivative;
use protocols::{
    confidential_data_hub, confidential_data_hub_ttrpc_async,
    confidential_data_hub_ttrpc_async::{SealedSecretServiceClient, SecureMountServiceClient},
};
use std::fs;
use std::os::unix::fs::symlink;
use std::path::Path;
use tokio::sync::OnceCell;

// Nanoseconds
lazy_static! {
    static ref CDH_API_TIMEOUT: i64 = AGENT_CONFIG.cdh_api_timeout.as_nanos() as i64;
    pub static ref CDH_CLIENT: OnceCell<CDHClient> = OnceCell::new();
}

const SEALED_SECRET_PREFIX: &str = "sealed.";

// Convenience function to obtain the scope logger.
fn sl() -> slog::Logger {
    slog_scope::logger().new(o!("subsystem" => "cdh"))
}

#[derive(Derivative)]
#[derivative(Clone, Debug)]
pub struct CDHClient {
    #[derivative(Debug = "ignore")]
    sealed_secret_client: SealedSecretServiceClient,
    #[derivative(Debug = "ignore")]
    secure_mount_client: SecureMountServiceClient,
}

impl CDHClient {
    pub fn new(cdh_socket_uri: &str) -> Result<Self> {
        let client = ttrpc::asynchronous::Client::connect(cdh_socket_uri)?;
        let sealed_secret_client =
            confidential_data_hub_ttrpc_async::SealedSecretServiceClient::new(client.clone());
        let secure_mount_client =
            confidential_data_hub_ttrpc_async::SecureMountServiceClient::new(client);
        Ok(CDHClient {
            sealed_secret_client,
            secure_mount_client,
        })
    }

    pub async fn unseal_secret_async(&self, sealed_secret: &str) -> Result<Vec<u8>> {
        let mut input = confidential_data_hub::UnsealSecretInput::new();
        input.set_secret(sealed_secret.into());

        let unsealed_secret = self
            .sealed_secret_client
            .unseal_secret(ttrpc::context::with_timeout(*CDH_API_TIMEOUT), &input)
            .await?;
        Ok(unsealed_secret.plaintext)
    }

    pub async fn secure_mount(
        &self,
        volume_type: &str,
        options: &std::collections::HashMap<String, String>,
        flags: Vec<String>,
        mount_point: &str,
    ) -> Result<()> {
        let req = confidential_data_hub::SecureMountRequest {
            volume_type: volume_type.to_string(),
            options: options.clone(),
            flags,
            mount_point: mount_point.to_string(),
            ..Default::default()
        };
        self.secure_mount_client
            .secure_mount(ttrpc::context::with_timeout(*CDH_API_TIMEOUT), &req)
            .await?;
        Ok(())
    }
}

pub async fn init_cdh_client(cdh_socket_uri: &str) -> Result<()> {
    CDH_CLIENT
        .get_or_try_init(|| async {
            CDHClient::new(cdh_socket_uri).context("Failed to create CDH Client")
        })
        .await?;
    Ok(())
}

/// Check if the CDH client is initialized
pub async fn is_cdh_client_initialized() -> bool {
    CDH_CLIENT.get().is_some() // Returns true if CDH_CLIENT is initialized, false otherwise
}

pub async fn unseal_env(env: &str) -> Result<String> {
    let cdh_client = CDH_CLIENT
        .get()
        .expect("Confidential Data Hub not initialized");

    if let Some((key, value)) = env.split_once('=') {
        if value.starts_with(SEALED_SECRET_PREFIX) {
            let unsealed_value = cdh_client.unseal_secret_async(value).await?;
            let unsealed_env = format!("{}={}", key, std::str::from_utf8(&unsealed_value)?);

            return Ok(unsealed_env);
        }
    }
    Ok((*env.to_owned()).to_string())
}

pub async fn unseal_file(path: &str) -> Result<()> {
    let cdh_client = CDH_CLIENT
        .get()
        .expect("Confidential Data Hub not initialized");

    if !Path::new(path).exists() {
        bail!("sealed secret file {:?} does not exist", path);
    }

    // Iterate over all entries to handle the sealed secret file.
    // For example, the directory is as follows:
    // The secret directory in the guest: /run/kata-containers/shared/containers/21bbf0d932b70263d65d7052ecfd72ee46de03f766650cb378e93852ddb30a54-5063be11b6800f96-sealed-secret-target/:
    // - ..2024_09_30_02_55_58.2237819815
    // - ..data -> ..2024_09_30_02_55_58.2237819815
    // - secret -> ..2024_09_30_02_55_58.2237819815/secret
    //
    // The directory "..2024_09_30_02_55_58.2237819815":
    // - secret
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let entry_type = entry.file_type()?;
        if !entry_type.is_symlink() && !entry_type.is_file() {
            debug!(
                sl(),
                "skipping sealed source entry {:?} because its file type is {:?}",
                entry,
                entry_type
            );
            continue;
        }

        let target_path = fs::canonicalize(entry.path())?;
        info!(sl(), "sealed source entry target path: {:?}", target_path);

        // Skip if the target path is not a file (e.g., it's a symlink pointing to the secret file).
        if !target_path.is_file() {
            debug!(sl(), "sealed source is not a file: {:?}", target_path);
            continue;
        }

        let secret_name = entry.file_name();
        let contents = fs::read_to_string(&target_path)?;
        if contents.starts_with(SEALED_SECRET_PREFIX) {
            // Get the directory name of the sealed secret file
            let dir_name = target_path
                .parent()
                .and_then(|p| p.file_name())
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_default();

            // Create the unsealed file name in the same directory, which will be written the unsealed data.
            let unsealed_filename = format!("{}.unsealed", target_path.to_string_lossy());
            // Create the unsealed file symlink, which is used for reading the unsealed data in the container.
            let unsealed_filename_symlink =
                format!("{}/{}.unsealed", dir_name, secret_name.to_string_lossy());

            // Unseal the secret and write it to the unsealed file
            let unsealed_value = cdh_client.unseal_secret_async(&contents).await?;
            fs::write(&unsealed_filename, unsealed_value)?;

            // Remove the original sealed symlink and create a symlink to the unsealed file
            fs::remove_file(entry.path())?;
            symlink(unsealed_filename_symlink, entry.path())?;
        }
    }
    Ok(())
}

pub async fn secure_mount(
    volume_type: &str,
    options: &std::collections::HashMap<String, String>,
    flags: Vec<String>,
    mount_point: &str,
) -> Result<()> {
    let cdh_client = CDH_CLIENT
        .get()
        .expect("Confidential Data Hub not initialized");

    cdh_client
        .secure_mount(volume_type, options, flags, mount_point)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use std::fs::File;
    use std::io::{Read, Write};
    use std::sync::Arc;
    use tempfile::tempdir;
    use test_utils::skip_if_not_root;
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

    fn start_ttrpc_server(cdh_socket_uri: String) {
        tokio::spawn(async move {
            let ss = Box::new(TestService {})
                as Box<dyn confidential_data_hub_ttrpc_async::SealedSecretService + Send + Sync>;
            let ss = Arc::new(ss);
            let ss_service = confidential_data_hub_ttrpc_async::create_sealed_secret_service(ss);

            remove_if_sock_exist(&cdh_socket_uri).unwrap();

            let mut server = ttrpc::asynchronous::Server::new()
                .bind(&cdh_socket_uri)
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
    async fn test_sealed_secret() {
        skip_if_not_root!();
        let test_dir = tempdir().expect("failed to create tmpdir");
        let test_dir_path = test_dir.path();
        let cdh_sock_uri = &format!(
            "unix://{}",
            test_dir_path.join("cdh.sock").to_str().unwrap()
        );

        let rt = tokio::runtime::Runtime::new().unwrap();
        let _guard = rt.enter();
        start_ttrpc_server(cdh_sock_uri.to_string());
        std::thread::sleep(std::time::Duration::from_secs(2));
        init_cdh_client(cdh_sock_uri).await.unwrap();

        // Test sealed secret as env vars
        let sealed_env = String::from("key=sealed.testdata");
        let unsealed_env = unseal_env(&sealed_env).await.unwrap();
        assert_eq!(unsealed_env, String::from("key=unsealed"));
        let normal_env = String::from("key=testdata");
        let unchanged_env = unseal_env(&normal_env).await.unwrap();
        assert_eq!(unchanged_env, String::from("key=testdata"));

        // Test sealed secret as files
        let sealed_dir = test_dir_path.join("..test");
        fs::create_dir(&sealed_dir).unwrap();
        let sealed_filename = sealed_dir.join("secret");
        let mut sealed_file = File::create(sealed_filename.clone()).unwrap();
        sealed_file.write_all(b"sealed.testdata").unwrap();
        let secret_symlink = test_dir_path.join("secret");
        symlink(&sealed_filename, &secret_symlink).unwrap();

        unseal_file(test_dir_path.to_str().unwrap()).await.unwrap();

        let unsealed_filename = test_dir_path.join("secret");
        let mut unsealed_file = File::open(unsealed_filename.clone()).unwrap();
        let mut contents = String::new();
        unsealed_file.read_to_string(&mut contents).unwrap();
        assert_eq!(contents, String::from("unsealed"));
        fs::remove_file(sealed_filename).unwrap();
        fs::remove_file(unsealed_filename).unwrap();

        let normal_filename = test_dir_path.join("secret");
        let mut normal_file = File::create(normal_filename.clone()).unwrap();
        normal_file.write_all(b"testdata").unwrap();
        unseal_file(test_dir_path.to_str().unwrap()).await.unwrap();
        let mut contents = String::new();
        let mut normal_file = File::open(normal_filename.clone()).unwrap();
        normal_file.read_to_string(&mut contents).unwrap();
        assert_eq!(contents, String::from("testdata"));
        fs::remove_file(normal_filename).unwrap();

        rt.shutdown_background();
        std::thread::sleep(std::time::Duration::from_secs(2));
    }
}
