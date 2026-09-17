// Copyright (c) 2023 Intel Corporation
// Copyright (c) 2025 Alibaba Cloud
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
    confidential_data_hub,
    confidential_data_hub::GetResourceRequest,
    confidential_data_hub_ttrpc_async,
    confidential_data_hub_ttrpc_async::{
        GetResourceServiceClient, ImagePullServiceClient, SealedSecretServiceClient,
        SecureMountServiceClient,
    },
};
use safe_path::scoped_join;
use std::fs;
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;
use std::{os::unix::fs::symlink, path::PathBuf};
use tokio::sync::OnceCell;

pub mod image;

pub static CDH_CLIENT: OnceCell<CDHClient> = OnceCell::const_new();

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
    #[derivative(Debug = "ignore")]
    get_resource_client: GetResourceServiceClient,
    #[derivative(Debug = "ignore")]
    image_pull_client: ImagePullServiceClient,
}

impl CDHClient {
    pub async fn new(cdh_socket_uri: &str) -> Result<Self> {
        let client = ttrpc::asynchronous::Client::connect(cdh_socket_uri).await?;
        let sealed_secret_client =
            confidential_data_hub_ttrpc_async::SealedSecretServiceClient::new(client.clone());
        let image_pull_client =
            confidential_data_hub_ttrpc_async::ImagePullServiceClient::new(client.clone());
        let secure_mount_client =
            confidential_data_hub_ttrpc_async::SecureMountServiceClient::new(client.clone());
        let get_resource_client =
            confidential_data_hub_ttrpc_async::GetResourceServiceClient::new(client);
        Ok(CDHClient {
            sealed_secret_client,
            secure_mount_client,
            get_resource_client,
            image_pull_client,
        })
    }

    pub async fn unseal_secret_async(&self, sealed_secret: &str) -> Result<Vec<u8>> {
        let mut input = confidential_data_hub::UnsealSecretInput::new();
        input.set_secret(sealed_secret.into());

        let unsealed_secret = self
            .sealed_secret_client
            .unseal_secret(
                ttrpc::context::with_timeout(AGENT_CONFIG.cdh_api_timeout.as_nanos() as i64),
                &input,
            )
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
            .secure_mount(
                ttrpc::context::with_timeout(AGENT_CONFIG.cdh_api_timeout.as_nanos() as i64),
                &req,
            )
            .await?;
        Ok(())
    }

    pub async fn get_resource(&self, resource_path: &str) -> Result<Vec<u8>> {
        let req = GetResourceRequest {
            ResourcePath: resource_path.to_string(),
            ..Default::default()
        };
        let res = self
            .get_resource_client
            .get_resource(
                ttrpc::context::with_timeout(AGENT_CONFIG.cdh_api_timeout.as_nanos() as i64),
                &req,
            )
            .await?;
        Ok(res.Resource)
    }

    pub async fn pull_image(&self, image: &str, bundle_path: &str) -> Result<()> {
        let req = confidential_data_hub::ImagePullRequest {
            image_url: image.to_string(),
            bundle_path: bundle_path.to_string(),
            ..Default::default()
        };

        let _ = self
            .image_pull_client
            .pull_image(
                ttrpc::context::with_timeout(AGENT_CONFIG.image_pull_timeout.as_nanos() as i64),
                &req,
            )
            .await?;

        Ok(())
    }
}

pub async fn init_cdh_client(cdh_socket_uri: &str) -> Result<()> {
    CDH_CLIENT
        .get_or_try_init(|| async {
            CDHClient::new(cdh_socket_uri)
                .await
                .context("Failed to create CDH Client")
        })
        .await?;

    Ok(())
}

/// Check if the CDH client is initialized
pub fn is_cdh_client_initialized() -> bool {
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

/// pull_image is used for call confidential data hub to pull image in the guest.
/// Image layers will store at [`image::KATA_IMAGE_WORK_DIR`]`,
/// rootfs and config.json will store under given `bundle_path`.
///
/// # Parameters
/// - `image`: Image name (exp: quay.io/prometheus/busybox:latest)
/// - `bundle_path`: The path to store the image bundle (exp. /run/kata-containers/cb0b47276ea66ee9f44cc53afa94d7980b57a52c3f306f68cb034e58d9fbd3c6/rootfs)
pub async fn pull_image(image: &str, bundle_path: PathBuf) -> Result<String> {
    fs::create_dir_all(&bundle_path)?;
    info!(sl(), "pull image {image:?}, bundle path {bundle_path:?}");

    let cdh_client = CDH_CLIENT
        .get()
        .expect("Confidential Data Hub not initialized");

    cdh_client
        .pull_image(image, bundle_path.to_string_lossy().as_ref())
        .await?;

    let image_bundle_path = scoped_join(&bundle_path, "rootfs")?;
    Ok(image_bundle_path.as_path().display().to_string())
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
        if content_starts_with_prefix(&target_path, SEALED_SECRET_PREFIX).await? {
            let contents = fs::read_to_string(&target_path)?;
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

pub async fn content_starts_with_prefix(path: &Path, prefix: &str) -> io::Result<bool> {
    let mut file = File::open(path)?;
    let mut buffer = vec![0u8; prefix.len()];

    match file.read_exact(&mut buffer) {
        Ok(()) => Ok(buffer == prefix.as_bytes()),
        Err(ref e) if e.kind() == io::ErrorKind::UnexpectedEof => Ok(false),
        Err(e) => Err(e),
    }
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

#[allow(dead_code)]
pub async fn get_cdh_resource(resource_path: &str) -> Result<Vec<u8>> {
    let cdh_client = CDH_CLIENT
        .get()
        .expect("Confidential Data Hub not initialized");

    cdh_client.get_resource(resource_path).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use rstest::{fixture, rstest};
    use std::fs::File;
    use std::io::{Read, Write};
    use std::sync::Arc;
    use std::time::{Duration, Instant};
    use tempfile::{tempdir, NamedTempFile};
    use test_utils::skip_if_not_root;
    use tokio::signal::unix::{signal, SignalKind};

    const TEST_RETRY_COUNT: usize = 5;
    const TEST_RETRY_DELAY_MS: u64 = 100;

    struct TestService;

    struct CdhTestEnv {
        _test_dir: tempfile::TempDir,
        client: CDHClient,
    }

    impl CdhTestEnv {
        fn test_dir_path(&self) -> &std::path::Path {
            self._test_dir.path()
        }
    }

    #[fixture]
    async fn cdh_env() -> CdhTestEnv {
        let test_dir = tempdir().expect("failed to create tmpdir");
        let cdh_sock_uri = format!(
            "unix://{}",
            test_dir.path().join("cdh.sock").to_str().unwrap()
        );

        start_ttrpc_server(cdh_sock_uri.clone());
        wait_for_server_ready(&cdh_sock_uri, Duration::from_secs(5))
            .await
            .unwrap();
        let client = CDHClient::new(&cdh_sock_uri).unwrap();

        CdhTestEnv {
            _test_dir: test_dir,
            client,
        }
    }

    async fn wait_for_server_ready(uri: &str, timeout: Duration) -> Result<()> {
        let start = Instant::now();

        loop {
            if start.elapsed() > timeout {
                bail!("Server did not become ready within timeout");
            }

            match ttrpc::asynchronous::Client::connect(uri) {
                Ok(_) => return Ok(()),
                Err(_) => tokio::time::sleep(Duration::from_millis(100)).await,
            }
        }
    }

    // Generic retry helper to reduce code duplication
    async fn retry_operation<F, Fut, T>(
        operation: F,
        retries: usize,
        delay_ms: u64,
    ) -> Result<T>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        let mut last_err = None;

        for attempt in 0..retries {
            match operation().await {
                Ok(result) => return Ok(result),
                Err(err) => {
                    last_err = Some(err);
                    if attempt + 1 < retries {
                        tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
                    }
                }
            }
        }

        Err(last_err.expect("retry_operation called with zero retries"))
    }

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

    #[async_trait]
    impl confidential_data_hub_ttrpc_async::ImagePullService for TestService {
        async fn pull_image(
            &self,
            _ctx: &::ttrpc::asynchronous::TtrpcContext,
            _req: confidential_data_hub::ImagePullRequest,
        ) -> ttrpc::error::Result<confidential_data_hub::ImagePullResponse> {
            let output = confidential_data_hub::ImagePullResponse::new();
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
            let ss = Box::new(TestService {});
            let ss = Arc::new(*ss);
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

    async fn retry_unseal_env(
        cdh_client: &CDHClient,
        env: &str,
    ) -> Result<String> {
        retry_operation(
            || async {
                if let Some((key, value)) = env.split_once('=') {
                    if value.starts_with(SEALED_SECRET_PREFIX) {
                        cdh_client
                            .unseal_secret_async(value)
                            .await
                            .map(|unsealed_value| {
                                format!("{}={}", key, std::str::from_utf8(&unsealed_value).unwrap())
                            })
                    } else {
                        Ok(env.to_string())
                    }
                } else {
                    Ok(env.to_string())
                }
            },
            TEST_RETRY_COUNT,
            TEST_RETRY_DELAY_MS,
        )
        .await
    }

    async fn retry_unseal_file(
        cdh_client: &CDHClient,
        path: &str,
    ) -> Result<()> {
        retry_operation(
            || unseal_file_with_client(cdh_client, path),
            TEST_RETRY_COUNT,
            TEST_RETRY_DELAY_MS,
        )
        .await
    }

    async fn unseal_file_with_client(cdh_client: &CDHClient, path: &str) -> Result<()> {
        let path = Path::new(path);
        if !path.exists() {
            bail!("file/path {} does not exist", path.to_string_lossy());
        }

        if path.is_file() {
            if content_starts_with_prefix(path, SEALED_SECRET_PREFIX).await? {
                let sealed_secret = fs::read_to_string(path)?;
                let unsealed_secret = cdh_client.unseal_secret_async(&sealed_secret).await?;
                fs::write(path, unsealed_secret)?;
            }
        } else if path.is_dir() {
            for entry in fs::read_dir(path)? {
                let entry = entry?;
                let file_path = entry.path();
                if file_path.is_file() {
                    let metadata = fs::symlink_metadata(&file_path)?;
                    if metadata.file_type().is_symlink()
                        || content_starts_with_prefix(&file_path, SEALED_SECRET_PREFIX).await?
                    {
                        let target_path = fs::canonicalize(&file_path)?;
                        let sealed_secret = fs::read_to_string(target_path.clone())?;
                        if sealed_secret.starts_with(SEALED_SECRET_PREFIX) {
                            let unsealed_secret = cdh_client.unseal_secret_async(&sealed_secret).await?;
                            fs::write(file_path, unsealed_secret)?;
                        }
                    }
                }
            }
        }

        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_unseal_env_sealed_secret(#[future] cdh_env: CdhTestEnv) {
        skip_if_not_root!();
        let cdh_env = cdh_env.await;

        let sealed_env = String::from("key=sealed.testdata");
        let unsealed_env = retry_unseal_env(&cdh_env.client, &sealed_env)
            .await
            .unwrap();
        assert_eq!(unsealed_env, String::from("key=unsealed"));
    }

    #[rstest]
    #[tokio::test]
    async fn test_unseal_file_sealed_secret(#[future] cdh_env: CdhTestEnv) {
        skip_if_not_root!();
        let cdh_env = cdh_env.await;
        let test_dir_path = cdh_env.test_dir_path();

        let sealed_dir = test_dir_path.join("..test");
        fs::create_dir(&sealed_dir).unwrap();
        let sealed_filename = sealed_dir.join("secret");
        let mut sealed_file = File::create(sealed_filename.clone()).unwrap();
        sealed_file.write_all(b"sealed.testdata").unwrap();
        let secret_symlink = test_dir_path.join("secret");
        symlink(&sealed_filename, &secret_symlink).unwrap();

        retry_unseal_file(&cdh_env.client, test_dir_path.to_str().unwrap())
            .await
            .unwrap();
        let unsealed_filename = test_dir_path.join("secret");
        let mut unsealed_file = File::open(unsealed_filename.clone()).unwrap();
        let mut contents = String::new();
        unsealed_file.read_to_string(&mut contents).unwrap();
        assert_eq!(contents, String::from("unsealed"));
        fs::remove_file(sealed_filename).unwrap();
        fs::remove_file(unsealed_filename).unwrap();
    }

    #[rstest]
    #[tokio::test]
    async fn test_unseal_file_normal_secret(#[future] cdh_env: CdhTestEnv) {
        skip_if_not_root!();
        let cdh_env = cdh_env.await;
        let test_dir_path = cdh_env.test_dir_path();

        let normal_filename = test_dir_path.join("secret");
        let mut normal_file = File::create(normal_filename.clone()).unwrap();
        normal_file.write_all(b"testdata").unwrap();

        retry_unseal_file(&cdh_env.client, test_dir_path.to_str().unwrap())
            .await
            .unwrap();
        let mut contents = String::new();
        let mut normal_file = File::open(normal_filename.clone()).unwrap();
        normal_file.read_to_string(&mut contents).unwrap();
        assert_eq!(contents, String::from("testdata"));
        fs::remove_file(normal_filename).unwrap();
    }

    #[rstest]
    #[tokio::test]
    async fn test_unseal_env_normal_env(#[future] cdh_env: CdhTestEnv) {
        skip_if_not_root!();
        let cdh_env = cdh_env.await;

        let normal_env = "PATH=/usr/bin:/bin";
        let result = retry_unseal_env(&cdh_env.client, normal_env)
            .await
            .unwrap();
        assert_eq!(result, normal_env, "Normal env should remain unchanged");
    }

    #[rstest]
    #[tokio::test]
    async fn test_unseal_env_no_equals(#[future] cdh_env: CdhTestEnv) {
        skip_if_not_root!();
        let cdh_env = cdh_env.await;

        let invalid_env = "INVALID_ENV_VAR";
        let result = retry_unseal_env(&cdh_env.client, invalid_env)
            .await
            .unwrap();
        assert_eq!(result, invalid_env, "Invalid format should remain unchanged");
    }

    #[rstest]
    #[tokio::test]
    async fn test_unseal_env_empty_value(#[future] cdh_env: CdhTestEnv) {
        skip_if_not_root!();
        let cdh_env = cdh_env.await;

        let empty_env = "KEY=";
        let result = retry_unseal_env(&cdh_env.client, empty_env)
            .await
            .unwrap();
        assert_eq!(result, empty_env, "Empty value should remain unchanged");
    }

    #[rstest]
    #[tokio::test]
    async fn test_unseal_file_nonexistent_path(#[future] cdh_env: CdhTestEnv) {
        skip_if_not_root!();
        let cdh_env = cdh_env.await;

        let nonexistent_path = "/nonexistent/path/to/file";
        let result = retry_unseal_file(&cdh_env.client, nonexistent_path).await;
        assert!(result.is_err(), "Should fail with nonexistent path");

        if let Err(e) = result {
            let error_msg = format!("{}", e);
            assert!(error_msg.contains("does not exist"),
                "Error should mention file doesn't exist: {}", error_msg);
        }
    }

    #[rstest]
    #[tokio::test]
    async fn test_unseal_file_with_directory(#[future] cdh_env: CdhTestEnv) {
        skip_if_not_root!();
        let cdh_env = cdh_env.await;
        let test_dir_path = cdh_env.test_dir_path();

        let subdir = test_dir_path.join("subdir");
        fs::create_dir(&subdir).unwrap();

        let result = retry_unseal_file(&cdh_env.client, test_dir_path.to_str().unwrap()).await;
        assert!(result.is_ok(), "Should succeed with empty directory");
    }

    // Group all content_starts_with_prefix tests together
    #[tokio::test]
    async fn test_content_starts_with_prefix() {
        // Normal case: content matches the prefix
        let mut f = NamedTempFile::new().unwrap();
        write!(f, "sealed.hello_world").unwrap();
        assert!(content_starts_with_prefix(f.path(), "sealed.")
            .await
            .unwrap());

        // Does not match the prefix
        let mut f2 = NamedTempFile::new().unwrap();
        write!(f2, "notsealed.hello_world").unwrap();
        assert!(!content_starts_with_prefix(f2.path(), "sealed.")
            .await
            .unwrap());

        // File length < prefix.len()
        let mut f3 = NamedTempFile::new().unwrap();
        write!(f3, "seal").unwrap();
        assert!(!content_starts_with_prefix(f3.path(), "sealed.")
            .await
            .unwrap());

        // Empty file
        let f4 = NamedTempFile::new().unwrap();
        assert!(!content_starts_with_prefix(f4.path(), "sealed.")
            .await
            .unwrap());
    }

    #[tokio::test]
    async fn test_content_starts_with_prefix_exact_match() {
        let mut f = NamedTempFile::new().unwrap();
        write!(f, "sealed.").unwrap();
        assert!(content_starts_with_prefix(f.path(), "sealed.")
            .await
            .unwrap());
    }

    #[tokio::test]
    async fn test_content_starts_with_prefix_longer_content() {
        let mut f = NamedTempFile::new().unwrap();
        write!(f, "sealed.this_is_a_very_long_secret_value").unwrap();
        assert!(content_starts_with_prefix(f.path(), "sealed.")
            .await
            .unwrap());
    }

    #[tokio::test]
    async fn test_content_starts_with_prefix_different_prefix() {
        let mut f = NamedTempFile::new().unwrap();
        write!(f, "sealed.hello").unwrap();
        assert!(!content_starts_with_prefix(f.path(), "encrypted.")
            .await
            .unwrap());
    }

    #[tokio::test]
    async fn test_content_starts_with_prefix_binary_content() {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(&[0xFF, 0xFE, 0xFD, 0xFC]).unwrap();
        assert!(!content_starts_with_prefix(f.path(), "sealed.")
            .await
            .unwrap());
    }

    #[tokio::test]
    async fn test_content_starts_with_prefix_nonexistent_file() {
        let result = content_starts_with_prefix(Path::new("/nonexistent/file"), "sealed.").await;
        assert!(result.is_err(), "Should fail with nonexistent file");
    }

    #[test]
    fn test_sealed_secret_prefix_constant() {
        assert_eq!(SEALED_SECRET_PREFIX, "sealed.");
    }

    #[tokio::test]
    async fn test_cdh_client_new_invalid_socket() {
        // Test CDHClient creation with invalid socket
        let result = CDHClient::new("invalid://socket/path");
        assert!(result.is_err(), "Should fail with invalid socket URI");
    }

    #[tokio::test]
    async fn test_init_cdh_client_invalid_uri() {
        // Test initialization with invalid URI
        let result = init_cdh_client("invalid://uri").await;
        assert!(result.is_err(), "Should fail with invalid URI");
    }
}
