// Copyright (c) 2023 Intel Corporation
// Copyright (c) 2025 Alibaba Cloud
//
// SPDX-License-Identifier: Apache-2.0
//

// Confidential Data Hub client wrapper.
// Confidential Data Hub is a service running inside guest to provide resource related APIs.
// https://github.com/confidential-containers/guest-components/tree/main/confidential-data-hub

use crate::AGENT_CONFIG;
use anyhow::anyhow;
use anyhow::{bail, Context, Result};
use futures::{stream::FuturesUnordered, StreamExt};
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
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;
use std::{fmt, fs};
use std::{os::unix::fs::symlink, path::PathBuf};
use tokio::sync::OnceCell;
use tokio::time::timeout;
use ttrpc::asynchronous::Client as TtrpcClient;

pub mod image;

const SEALED_SECRET_PREFIX: &str = "sealed.";

// Convenience function to obtain the scope logger.
fn sl() -> slog::Logger {
    slog_scope::logger().new(o!("subsystem" => "cdh"))
}

pub static CDH_MULTI_CLIENTS: OnceCell<MultiCdhClients> = OnceCell::const_new();

pub async fn init_multi_cdh_clients() -> Result<()> {
    CDH_MULTI_CLIENTS
        .get_or_try_init(|| async {
            let mgr = MultiCdhClientsManager::new().await?;
            mgr.probe().await?;
            MultiCdhClients::new(&mgr)
        })
        .await?;
    Ok(())
}

/// Check if the CDH client is initialized
pub fn is_multi_cdh_clients_initialized() -> bool {
    CDH_MULTI_CLIENTS.get().is_some()
}

#[macro_export]
macro_rules! skip_if_cdh_client_uninitialized {
    // return Ok(())
    () => {{
        $crate::skip_if_cdh_client_uninitialized!(());
    }};

    // return Ok($ret)
    ($ret:expr) => {{
        if !$crate::confidential_data_hub::is_multi_cdh_clients_initialized() {
            eprintln!(
                "INFO: skipping {} because CDH_MULTI_CLIENTS is not initialized",
                module_path!()
            );
            return Ok($ret);
        }
    }};
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Service {
    ImagePull,
    SealedSecrets,
    SecureMount,
    GetResource,
}

impl Service {
    pub fn socket_path(&self) -> &'static str {
        match self {
            Service::ImagePull => "/run/guest-services/imagepull.socket",
            Service::SealedSecrets => "/run/guest-services/sealedsecrets.socket",
            Service::SecureMount => "/run/guest-services/securemount.socket",
            Service::GetResource => "/run/guest-services/getresource.socket",
        }
    }
}

impl fmt::Display for Service {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match self {
            Service::ImagePull => write!(f, "imagepull"),
            Service::SealedSecrets => write!(f, "sealedsecrets"),
            Service::SecureMount => write!(f, "securemount"),
            Service::GetResource => write!(f, "getresource"),
        }
    }
}

const REQUIRED_SERVICES: &[Service] = &[
    Service::GetResource,
    Service::ImagePull,
    Service::SealedSecrets,
    Service::SecureMount,
];

#[derive(Clone)]
pub struct MultiCdhClientsManager {
    connections: HashMap<Service, TtrpcClient>,
}

impl MultiCdhClientsManager {
    pub async fn new() -> Result<Self> {
        let mut connections = HashMap::new();

        for svc in [
            Service::ImagePull,
            Service::SealedSecrets,
            Service::SecureMount,
            Service::GetResource,
        ] {
            match Self::connect_to_service(svc).await {
                Ok(client) => {
                    info!(
                        sl(),
                        "Connected cdh client {} at {}",
                        svc,
                        svc.socket_path()
                    );
                    connections.insert(svc, client);
                }
                Err(e) => {
                    info!(
                        sl(),
                        "Failed to connect cdh client {} at {}: {:#}",
                        svc,
                        svc.socket_path(),
                        e
                    );
                }
            }
        }

        Ok(Self { connections })
    }

    async fn connect_to_service(service: Service) -> Result<TtrpcClient> {
        let socket_path = service.socket_path();

        if !Path::new(socket_path).exists() {
            return Err(anyhow!("socket path does not exist: {socket_path}"));
        }

        let sock_addr = format!("unix://{}", socket_path);
        let client = TtrpcClient::connect(&sock_addr).with_context(|| {
            format!(
                "failed to connect to socket '{sock_addr}' (service={})",
                service
            )
        })?;

        Ok(client)
    }

    pub fn get_client(&self, service: Service) -> Option<&TtrpcClient> {
        self.connections.get(&service)
    }

    pub fn has_required_clients_connected(&self) -> Result<()> {
        let missing: Vec<String> = REQUIRED_SERVICES
            .iter()
            .copied()
            .filter(|s| !self.connections.contains_key(s))
            .map(|s| s.to_string())
            .collect();

        if missing.is_empty() {
            Ok(())
        } else {
            Err(anyhow!("missing required cdh clients: {:?}", missing))
        }
    }

    pub async fn probe(&self) -> Result<()> {
        self.has_required_clients_connected()?;

        // Concurrently probe the "RPC reachability" of each service
        let mut futs = FuturesUnordered::new();
        for svc in REQUIRED_SERVICES.iter().copied() {
            futs.push(async move { (svc, self.probe_one(svc).await) });
        }

        while let Some((svc, res)) = futs.next().await {
            if let Err(e) = res {
                return Err(anyhow!("probe {} failed: {:#}", svc, e));
            }
        }
        Ok(())
    }

    async fn probe_one(&self, svc: Service) -> Result<()> {
        let t = AGENT_CONFIG.cdh_api_timeout;

        match svc {
            Service::GetResource => {
                let client = self.get_client(Service::GetResource).unwrap().clone();
                let c = confidential_data_hub_ttrpc_async::GetResourceServiceClient::new(client);
                let req = GetResourceRequest {
                    ResourcePath: "kbs://__kata_probe__/non-existent".to_string(), // FIXME
                    ..Default::default()
                };

                // Regardless of the error details, we consider the service reachable if it responds within the timeout.
                // The service itself will return an error for the non-existent resource, but that still means the service is up and reachable.
                let r = timeout(
                    t,
                    c.get_resource(ttrpc::context::with_timeout(t.as_nanos() as i64), &req),
                )
                .await;
                match r {
                    Ok(Ok(_)) => Ok(()),
                    Ok(Err(_e)) => Ok(()),
                    Err(_) => Err(anyhow!("timeout after {:?}", t)),
                }
            }
            Service::ImagePull | Service::SealedSecrets | Service::SecureMount => Ok(()),
        }
    }
}

pub async fn unseal_env(env: &str) -> Result<String> {
    let cdh_client = CDH_MULTI_CLIENTS
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

    let cdh_client = CDH_MULTI_CLIENTS
        .get()
        .expect("Confidential Data Hub not initialized");

    cdh_client
        .pull_image(image, bundle_path.to_string_lossy().as_ref())
        .await?;

    let image_bundle_path = scoped_join(&bundle_path, "rootfs")?;
    Ok(image_bundle_path.as_path().display().to_string())
}

pub async fn unseal_file(path: &str) -> Result<()> {
    let cdh_client = CDH_MULTI_CLIENTS
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
    let cdh_client = CDH_MULTI_CLIENTS
        .get()
        .expect("Confidential Data Hub not initialized");

    cdh_client
        .secure_mount(volume_type, options, flags, mount_point)
        .await?;
    Ok(())
}

#[allow(dead_code)]
pub async fn get_cdh_resource(resource_path: &str) -> Result<Vec<u8>> {
    let cdh_client = CDH_MULTI_CLIENTS
        .get()
        .expect("Confidential Data Hub not initialized");

    cdh_client.get_resource(resource_path).await
}

#[derive(Clone)]
pub struct MultiCdhClients {
    sealed_secret_client: SealedSecretServiceClient,
    secure_mount_client: SecureMountServiceClient,
    get_resource_client: GetResourceServiceClient,
    image_pull_client: ImagePullServiceClient,
}

impl MultiCdhClients {
    pub fn new(mgr: &MultiCdhClientsManager) -> Result<Self> {
        // Ensure all required services are connected, otherwise return an error
        mgr.has_required_clients_connected()?;

        let sealed = mgr
            .get_client(Service::SealedSecrets)
            .ok_or_else(|| anyhow!("SealedSecrets service not connected"))?
            .clone();
        let secure = mgr
            .get_client(Service::SecureMount)
            .ok_or_else(|| anyhow!("SecureMount service not connected"))?
            .clone();
        let getres = mgr
            .get_client(Service::GetResource)
            .ok_or_else(|| anyhow!("GetResource service not connected"))?
            .clone();
        let image = mgr
            .get_client(Service::ImagePull)
            .ok_or_else(|| anyhow!("ImagePull service not connected"))?
            .clone();

        Ok(Self {
            sealed_secret_client: SealedSecretServiceClient::new(sealed),
            secure_mount_client: SecureMountServiceClient::new(secure),
            get_resource_client: GetResourceServiceClient::new(getres),
            image_pull_client: ImagePullServiceClient::new(image),
        })
    }

    pub async fn unseal_secret_async(&self, sealed_secret: &str) -> Result<Vec<u8>> {
        let mut input = confidential_data_hub::UnsealSecretInput::new();
        input.set_secret(sealed_secret.into());

        let res = self
            .sealed_secret_client
            .unseal_secret(
                ttrpc::context::with_timeout(AGENT_CONFIG.cdh_api_timeout.as_nanos() as i64),
                &input,
            )
            .await?;
        Ok(res.plaintext)
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
            ResourcePath: format!("kbs://{resource_path}"),
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

        self.image_pull_client
            .pull_image(
                ttrpc::context::with_timeout(AGENT_CONFIG.image_pull_timeout.as_nanos() as i64),
                &req,
            )
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    // TODO: add tests for MultiCdhClientsManager and MultiCdhClients
}
