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
use std::path::PathBuf;
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

/// Unseal the secret volume at `src` into `dst`, leaving `src` untouched.
///
/// Returns false if nothing was sealed, in which case `dst` is not created and
/// the caller should leave the mount pointing at `src`.
///
/// Unsealing in place would fail on a read-only volume, as it is once the
/// runtime ships it as an EROFS image, and it would fail quietly, leaving the
/// container with the ciphertext.
pub async fn unseal_files_into(src: &Path, dst: &Path) -> Result<bool> {
    if !src.exists() {
        bail!("sealed secret file {:?} does not exist", src);
    }

    // A hostPath file is a single file, and read_dir would fail. Sealed
    // secrets only exist in kubelet's atomic-writer directory layout.
    if !src.is_dir() {
        return Ok(false);
    }

    // Resolved up front so a volume with nothing sealed costs only the read.
    let mut entries = Vec::new();
    for entry in fs::read_dir(src)? {
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

        if !target_path.is_file() {
            debug!(sl(), "sealed source is not a file: {:?}", target_path);
            continue;
        }

        let sealed = content_starts_with_prefix(&target_path, SEALED_SECRET_PREFIX).await?;
        entries.push((entry.file_name(), target_path, sealed));
    }

    if !entries.iter().any(|(_, _, sealed)| *sealed) {
        return Ok(false);
    }

    // Only needed once there is something to unseal.
    let cdh_client = CDH_CLIENT
        .get()
        .expect("Confidential Data Hub not initialized");

    fs::create_dir_all(dst)?;

    for (name, target_path, sealed) in entries {
        let contents = if sealed {
            info!(sl(), "unsealing {:?}", target_path);
            let sealed_contents = fs::read_to_string(&target_path)?;
            cdh_client.unseal_secret_async(&sealed_contents).await?
        } else {
            // Copied, not linked: this directory replaces the volume, and a
            // link to the original guest path would not resolve in the
            // container's mount namespace.
            fs::read(&target_path)?
        };

        let path = dst.join(&name);
        fs::write(&path, contents)?;
        // Whatever defaultMode the pod asked for.
        fs::set_permissions(&path, fs::metadata(&target_path)?.permissions())?;
    }

    Ok(true)
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
    use std::io::Write;
    use std::os::unix::fs::symlink;
    use std::sync::Arc;
    use tempfile::{tempdir, NamedTempFile};
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
        assert!(
            is_cdh_client_initialized(),
            "CDH client should be initialized after init_cdh_client"
        );

        // Test sealed secret as env vars
        let sealed_env = String::from("key=sealed.testdata");
        let unsealed_env = unseal_env(&sealed_env).await.unwrap();
        assert_eq!(unsealed_env, String::from("key=unsealed"));
        let normal_env = String::from("key=testdata");
        let unchanged_env = unseal_env(&normal_env).await.unwrap();
        assert_eq!(unchanged_env, String::from("key=testdata"));

        // Test unsealing a mixed env var list (mirrors do_exec_process pattern)
        let mut exec_envs = [
            String::from("PATH=/usr/bin"),
            String::from("MY_SECRET=sealed.testdata"),
            String::from("HOME=/root"),
            String::from("ANOTHER_SECRET=sealed.otherdata"),
            String::from("PLAIN_VAR=normalvalue"),
        ];

        for env in exec_envs.iter_mut() {
            match unseal_env(env).await {
                Ok(unsealed) => *env = unsealed,
                Err(e) => panic!("unseal_env failed unexpectedly: {}", e),
            }
        }
        assert_eq!(exec_envs[0], "PATH=/usr/bin");
        assert_eq!(exec_envs[1], "MY_SECRET=unsealed");
        assert_eq!(exec_envs[2], "HOME=/root");
        assert_eq!(exec_envs[3], "ANOTHER_SECRET=unsealed");
        assert_eq!(exec_envs[4], "PLAIN_VAR=normalvalue");

        // Test edge cases for env var unsealing
        let empty_env = String::from("");
        assert_eq!(unseal_env(&empty_env).await.unwrap(), "");
        let no_value = String::from("KEY_ONLY");
        assert_eq!(unseal_env(&no_value).await.unwrap(), "KEY_ONLY");
        let empty_value = String::from("KEY=");
        assert_eq!(unseal_env(&empty_value).await.unwrap(), "KEY=");
        let sealed_prefix_only = String::from("KEY=sealed.");
        let result = unseal_env(&sealed_prefix_only).await.unwrap();
        assert_eq!(result, "KEY=unsealed");

        // Test sealed secret as files
        let volume = test_dir_path.join("volume");
        atomic_writer_volume(
            &volume,
            &[("secret", b"sealed.testdata"), ("plain", b"testdata")],
        );

        let unsealed = test_dir_path.join("unsealed");
        assert!(unseal_files_into(&volume, &unsealed).await.unwrap());

        // The unsealed copy replaces the volume, so entries that were not
        // sealed have to come through it too.
        assert_eq!(
            fs::read_to_string(unsealed.join("secret")).unwrap(),
            "unsealed"
        );
        assert_eq!(
            fs::read_to_string(unsealed.join("plain")).unwrap(),
            "testdata"
        );

        // The source is left alone: it may be read-only.
        assert_eq!(
            fs::read_to_string(volume.join("secret")).unwrap(),
            "sealed.testdata"
        );
        assert!(!volume.join("..data/secret.unsealed").exists());

        // Nothing sealed: no directory, and the caller keeps the original.
        fs::write(volume.join("..data/secret"), b"testdata").unwrap();
        let untouched = test_dir_path.join("untouched");
        assert!(!unseal_files_into(&volume, &untouched).await.unwrap());
        assert!(!untouched.exists());

        rt.shutdown_background();
        std::thread::sleep(std::time::Duration::from_secs(2));
    }

    /// The layout kubelet's atomic writer produces.
    fn atomic_writer_volume(root: &Path, contents: &[(&str, &[u8])]) {
        let data = root.join("..2024_09_30_02_55_58.2237819815");
        fs::create_dir_all(&data).unwrap();
        symlink("..2024_09_30_02_55_58.2237819815", root.join("..data")).unwrap();

        for (name, body) in contents {
            fs::write(data.join(name), body).unwrap();
            symlink(format!("..data/{name}"), root.join(name)).unwrap();
        }
    }

    /// Every volume in every pod comes through here and almost none hold a
    /// sealed secret, so this is the path that matters most.
    #[tokio::test]
    async fn test_nothing_sealed_leaves_the_volume_alone() {
        let tmp = tempdir().unwrap();

        let volume = tmp.path().join("volume");
        atomic_writer_volume(
            &volume,
            &[("key-a", b"value-a"), ("key-b", b"not-sealed.value-b")],
        );

        let dst = tmp.path().join("unsealed");
        assert!(!unseal_files_into(&volume, &dst).await.unwrap());

        // No directory to clean up, and the caller keeps the original mount.
        assert!(!dst.exists());
        assert_eq!(fs::read_to_string(volume.join("key-a")).unwrap(), "value-a");
    }

    #[tokio::test]
    async fn test_file_volume_is_left_alone() {
        let tmp = tempdir().unwrap();
        let file = tmp.path().join("foo.txt");
        fs::write(&file, "test\n").unwrap();
        let dst = tmp.path().join("unsealed");

        assert!(!unseal_files_into(&file, &dst).await.unwrap());
        assert!(!dst.exists());
        assert_eq!(fs::read_to_string(&file).unwrap(), "test\n");
    }

    #[tokio::test]
    async fn test_missing_source_is_an_error() {
        let tmp = tempdir().unwrap();

        assert!(
            unseal_files_into(&tmp.path().join("absent"), &tmp.path().join("out"))
                .await
                .is_err()
        );
    }

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
}
