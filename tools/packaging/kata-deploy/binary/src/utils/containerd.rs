// Copyright 2026 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0

use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result};
use containerd_client::services::v1::{Plugin, PluginsRequest};

const PROBE_TIMEOUT: Duration = Duration::from_secs(3);
const SANDBOX_CONTROLLER_TYPE: &str = "io.containerd.sandbox.controller.v1";

fn shim_controller_ready(plugins: &[Plugin]) -> bool {
    plugins.iter().any(|plugin| {
        plugin.r#type == SANDBOX_CONTROLLER_TYPE && plugin.id == "shim" && plugin.init_err.is_none()
    })
}

/// Confirm that the running daemon has a usable SandboxAPI shim controller.
/// Callers should fall back to podsandbox on false or on a probe error.
pub async fn sandbox_api_available(socket: &Path) -> Result<bool> {
    probe_with_timeout(socket, PROBE_TIMEOUT).await
}

async fn probe_with_timeout(socket: &Path, deadline: Duration) -> Result<bool> {
    tokio::time::timeout(deadline, async {
        let client = containerd_client::Client::from_path(socket)
            .await
            .context("connect to containerd for SandboxAPI availability")?;
        let response = client
            .introspection()
            .plugins(tonic::Request::new(PluginsRequest {
                filters: Vec::new(),
            }))
            .await
            .context("query containerd plugins for SandboxAPI availability")?;
        Ok(shim_controller_ready(&response.into_inner().plugins))
    })
    .await
    .context("containerd SandboxAPI availability probe timed out")?
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shim_controller() -> Plugin {
        Plugin {
            r#type: SANDBOX_CONTROLLER_TYPE.into(),
            id: "shim".into(),
            ..Default::default()
        }
    }

    #[test]
    fn sandbox_api_requires_exact_initialized_shim_controller() {
        assert!(!shim_controller_ready(&[]));
        let mut plugin = shim_controller();
        assert!(shim_controller_ready(&[plugin.clone()]));
        plugin.id = "podsandbox".into();
        assert!(!shim_controller_ready(&[plugin]));
        let mut plugin = shim_controller();
        plugin.r#type = "io.containerd.podsandbox.controller.v1".into();
        assert!(!shim_controller_ready(&[plugin]));
        let mut plugin = shim_controller();
        plugin.init_err = Some(containerd_client::google::rpc::Status {
            code: 13,
            message: "controller initialization failed".into(),
            ..Default::default()
        });
        assert!(!shim_controller_ready(&[plugin]));
    }

    #[tokio::test]
    async fn sandbox_api_connection_failure_is_reported() {
        let directory = tempfile::tempdir().unwrap();
        let result = sandbox_api_available(&directory.path().join("missing.sock")).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn sandbox_api_stalled_peer_is_bounded() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("stalled.sock");
        let listener = tokio::net::UnixListener::bind(&path).unwrap();
        let peer = tokio::spawn(async move {
            let (_stream, _) = listener.accept().await.unwrap();
            std::future::pending::<()>().await;
        });
        let result = tokio::time::timeout(
            Duration::from_secs(1),
            probe_with_timeout(&path, Duration::from_millis(50)),
        )
        .await
        .expect("the probe must enforce its deadline");
        assert!(result.unwrap_err().to_string().contains("timed out"));
        peer.abort();
    }
}
