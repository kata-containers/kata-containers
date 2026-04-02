// Copyright (c) 2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{anyhow, Context, Result};
use hyper::{body::to_bytes, Body, Client, Method, Request, StatusCode};
use hyperlocal::{UnixClientExt, Uri};
use serde::{Deserialize, Serialize};
use tokio::time::{timeout, Duration};

pub const NYDUS_PASSTHROUGH_FS: &str = "passthrough_fs";
pub const NYDUS_RAFS: &str = "rafs";
// The mountpoint for passthrough_fs inside the nydusd virtiofs namespace.
// This is NOT a guest absolute path; it's a path within the virtiofs namespace.
// When the guest mounts virtiofs at `/run/kata-containers/shared/`, this maps to
// `/run/kata-containers/shared/containers/` in the guest.
pub const SHARED_PATH_IN_GUEST: &str = "/containers";

const HTTP_CLIENT_TIMEOUT_SECS: u64 = 30;

const INFO_ENDPOINT: &str = "/api/v1/daemon";
const MOUNT_ENDPOINT: &str = "/api/v1/mount";

const NYDUSD_DAEMON_STATE_RUNNING: &str = "RUNNING";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildTimeInfo {
    pub package_ver: String,
    pub git_commit: String,
    pub build_time: String,
    pub profile: String,
    pub rustc: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonInfo {
    pub version: BuildTimeInfo,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub supervisor: Option<String>,
    pub state: String,
    #[serde(default)]
    pub backend_collection: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorMessage {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MountRequest {
    pub fs_type: String,
    pub source: String,
    pub config: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overlay: Option<String>,
}

impl MountRequest {
    pub fn new(fs_type: &str, source: &str, config: &str) -> Self {
        Self {
            fs_type: fs_type.to_string(),
            source: source.to_string(),
            config: config.to_string(),
            overlay: None,
        }
    }

    pub fn new_with_overlay(fs_type: &str, source: &str, config: &str, overlay_config: &str) -> Self {
        Self {
            fs_type: fs_type.to_string(),
            source: source.to_string(),
            config: config.to_string(),
            overlay: Some(overlay_config.to_string()),
        }
    }
}

pub struct NydusClient {
    sock_path: String,
    client: Client<hyperlocal::UnixConnector>,
}

impl NydusClient {
    pub fn new(sock_path: &str) -> Self {
        Self {
            sock_path: sock_path.to_string(),
            client: Client::unix(),
        }
    }

    async fn send_request(
        &self,
        method: Method,
        path: &str,
        body: Option<&str>,
    ) -> Result<(StatusCode, Vec<u8>)> {
        let uri: hyper::Uri = Uri::new(&self.sock_path, path).into();

        let request_builder = Request::builder()
            .method(method)
            .uri(uri)
            .header("Content-Type", "application/json");

        let req = match body {
            Some(b) => request_builder
                .body(Body::from(b.to_string()))
                .context("failed to build HTTP request with body")?,
            None => request_builder
                .body(Body::empty())
                .context("failed to build HTTP request")?,
        };

        let response = timeout(Duration::from_secs(HTTP_CLIENT_TIMEOUT_SECS), self.client.request(req))
            .await
            .context("timeout waiting for response")?
            .context("failed to send HTTP request")?;

        let status = response.status();
        let body_bytes = to_bytes(response.into_body())
            .await
            .context("failed to read response body")?;

        Ok((status, body_bytes.to_vec()))
    }

    pub async fn check_status(&self) -> Result<DaemonInfo> {
        let (status, body) = self.send_request(Method::GET, INFO_ENDPOINT, None).await?;

        if status != StatusCode::OK {
            return Err(anyhow!("nydusd check status failed with code {}", status));
        }

        let info: DaemonInfo =
            serde_json::from_slice(&body).context("failed to parse DaemonInfo")?;
        Ok(info)
    }

    pub async fn mount(&self, mountpoint: &str, req: &MountRequest) -> Result<()> {
        let path = format!("{}?mountpoint={}", MOUNT_ENDPOINT, mountpoint);
        let body = serde_json::to_string(req).context("failed to serialize MountRequest")?;
        let (status, resp_body) = self.send_request(Method::POST, &path, Some(&body)).await?;

        if status == StatusCode::NO_CONTENT {
            return Ok(());
        }

        let err: ErrorMessage =
            serde_json::from_slice(&resp_body).context("failed to parse error message")?;
        Err(anyhow!("nydusd mount failed: {}", err.message))
    }

    pub async fn umount(&self, mountpoint: &str) -> Result<()> {
        let path = format!("{}?mountpoint={}", MOUNT_ENDPOINT, mountpoint);
        let (status, resp_body) = self.send_request(Method::DELETE, &path, None).await?;

        if status == StatusCode::NO_CONTENT {
            return Ok(());
        }

        let err: ErrorMessage =
            serde_json::from_slice(&resp_body).context("failed to parse error message")?;
        Err(anyhow!("nydusd umount failed: {}", err.message))
    }

    pub async fn wait_until_ready(&self, max_attempts: u32, delay_ms: u64) -> Result<()> {
        for _ in 0..max_attempts {
            match self.check_status().await {
                Ok(info) if info.state == NYDUSD_DAEMON_STATE_RUNNING => {
                    return Ok(());
                }
                Ok(info) => {
                    debug!(sl!(), "nydusd state: {}, waiting...", info.state);
                }
                Err(e) => {
                    debug!(sl!(), "nydusd not ready: {}", e);
                }
            }
            tokio::time::sleep(Duration::from_millis(delay_ms)).await;
        }
        Err(anyhow!(
            "nydusd API server not ready after {} attempts",
            max_attempts
        ))
    }
}