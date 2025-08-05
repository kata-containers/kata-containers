use std::collections::HashMap;

use anyhow::{anyhow, Context, Result};
use tracing::{info, warn};
use ttrpc::asynchronous::Client as TtrpcClient;

use crate::AGENT_CONFIG;
use protocols::{
    confidential_data_hub, confidential_data_hub::GetResourceRequest,
    confidential_data_hub_ttrpc_async,
};

const DEFAULT_SOCKET_PATHS: &[(&str, &str)] = &[
    ("imagepull", "/run/guest-services/imagepull.socket"),
    ("sealedsecrets", "/run/guest-services/sealedsecrets.socket"),
    ("securemount", "/run/guest-services/securemount.socket"),
    ("getresource", "/run/guest-services/getresource.socket"),
];

#[derive(Clone)]
pub struct GuestServiceManager {
    connections: HashMap<String, TtrpcClient>,
}

impl GuestServiceManager {
    pub async fn new() -> Result<Self> {
        let mut connections = HashMap::new();

        // try to connect all the existing sockets
        for (service_name, socket_path) in DEFAULT_SOCKET_PATHS {
            match Self::connect_to_service(socket_path).await {
                Ok(client) => {
                    info!("Successfully connected to {service_name} service at {socket_path}");
                    connections.insert(service_name.to_string(), client);
                }
                Err(e) => {
                    warn!("Failed to connect to {service_name} service at {socket_path}: {e}");
                    // go on handling other services, don't quit.
                }
            }
        }

        Ok(Self { connections })
    }

    async fn connect_to_service(socket_path: &str) -> Result<TtrpcClient> {
        let sock_addr = format!("unix://{}", socket_path);
        println!("socket address: {:?}", &sock_addr);
        let ttrpc_client = TtrpcClient::connect(&sock_addr).context(format!(
            "Failed to connect to socket at '{sock_addr}'. Is the server running and listening ?"
        ))?;

        Ok(ttrpc_client)
    }

    pub fn get_service(&self, service_name: &str) -> Option<&TtrpcClient> {
        self.connections.get(service_name)
    }

    #[allow(dead_code)]
    pub fn is_service_available(&self, service_name: &str) -> bool {
        self.connections.contains_key(service_name)
    }

    #[allow(dead_code)]
    pub fn list_available_services(&self) -> Vec<String> {
        self.connections.keys().cloned().collect()
    }

    #[allow(dead_code)]
    pub async fn unseal_secret_async(&self, sealed_secret: &str) -> Result<Vec<u8>> {
        if let Some(client) = self.get_service("sealedsecrets") {
            info!("unseal_secret: {:?}", sealed_secret.len());
            let sealed_secret_client =
                confidential_data_hub_ttrpc_async::SealedSecretServiceClient::new(client.clone());
            let mut input = confidential_data_hub::UnsealSecretInput::new();
            input.set_secret(sealed_secret.into());

            let unsealed_secret = sealed_secret_client
                .unseal_secret(
                    ttrpc::context::with_timeout(AGENT_CONFIG.cdh_api_timeout.as_nanos() as i64),
                    &input,
                )
                .await?;
            Ok(unsealed_secret.plaintext)
        } else {
            Err(anyhow!("SealedSecrets service not available"))
        }
    }

    #[allow(dead_code)]
    pub async fn secure_mount(
        &self,
        volume_type: &str,
        options: &std::collections::HashMap<String, String>,
        flags: Vec<String>,
        mount_point: &str,
    ) -> Result<()> {
        if let Some(client) = self.get_service("securemount") {
            info!(
                "secure_mount: {:?}, {:?}, {:?}",
                volume_type, flags, mount_point
            );
            let secure_mount_client =
                confidential_data_hub_ttrpc_async::SecureMountServiceClient::new(client.clone());
            let req = confidential_data_hub::SecureMountRequest {
                volume_type: volume_type.to_string(),
                options: options.clone(),
                flags,
                mount_point: mount_point.to_string(),
                ..Default::default()
            };
            secure_mount_client
                .secure_mount(
                    ttrpc::context::with_timeout(AGENT_CONFIG.cdh_api_timeout.as_nanos() as i64),
                    &req,
                )
                .await?;
            Ok(())
        } else {
            Err(anyhow!("SecureMount service not available"))
        }
    }

    #[allow(dead_code)]
    pub async fn get_resource(&self, resource_path: &str) -> Result<Vec<u8>> {
        if let Some(client) = self.get_service("getresource") {
            info!(resource_path, "get_resource");
            let req = GetResourceRequest {
                ResourcePath: format!("kbs://{}", resource_path),
                ..Default::default()
            };
            let get_resource_client =
                confidential_data_hub_ttrpc_async::GetResourceServiceClient::new(client.clone());
            let res = get_resource_client
                .get_resource(
                    ttrpc::context::with_timeout(AGENT_CONFIG.cdh_api_timeout.as_nanos() as i64),
                    &req,
                )
                .await?;
            Ok(res.Resource)
        } else {
            Err(anyhow!("GetResource service not available"))
        }
    }

    #[allow(dead_code)]
    pub async fn pull_image(&self, image: &str, bundle_path: &str) -> Result<()> {
        if let Some(client) = self.get_service("imagepull") {
            info!("Image pulled successfully");
            let req = confidential_data_hub::ImagePullRequest {
                image_url: image.to_string(),
                bundle_path: bundle_path.to_string(),
                ..Default::default()
            };
            let image_pull_client =
                confidential_data_hub_ttrpc_async::ImagePullServiceClient::new(client.clone());
            let _ = image_pull_client
                .pull_image(
                    ttrpc::context::with_timeout(AGENT_CONFIG.image_pull_timeout.as_nanos() as i64),
                    &req,
                )
                .await?;
            Ok(())
        } else {
            Err(anyhow!("ImagePull service not available"))
        }
    }
}
