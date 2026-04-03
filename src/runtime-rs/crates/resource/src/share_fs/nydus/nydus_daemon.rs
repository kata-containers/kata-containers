// Copyright (c) 2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::RwLock;

use super::nydus_client::{NydusClient, MountRequest, NYDUS_PASSTHROUGH_FS, NYDUS_RAFS, SHARED_PATH_IN_GUEST};

const NYDUSD_WAIT_READY_ATTEMPTS: u32 = 20;
const NYDUSD_WAIT_READY_DELAY_MS: u64 = 20;

#[derive(Debug)]
pub struct NydusdConfig {
    pub path: String,
    pub sock_path: String,
    pub api_sock_path: String,
    pub source_path: String,
    pub debug: bool,
    pub extra_args: Vec<String>,
}

struct NydusdInner {
    pid: Option<u32>,
    child: Option<Child>,
}

pub struct Nydusd {
    config: NydusdConfig,
    inner: Arc<RwLock<NydusdInner>>,
}

impl Nydusd {
    pub fn new(config: NydusdConfig) -> Self {
        Self {
            config,
            inner: Arc::new(RwLock::new(NydusdInner {
                pid: None,
                child: None,
            })),
        }
    }

    fn build_args(&self) -> Result<Vec<String>> {
        let log_level = if self.config.debug { "debug" } else { "info" };
        
        let mut args = vec![
            "virtiofs".to_string(),
            "--hybrid-mode".to_string(),
            "--log-level".to_string(),
            log_level.to_string(),
            "--apisock".to_string(),
            self.config.api_sock_path.clone(),
            "--sock".to_string(),
            self.config.sock_path.clone(),
        ];

        for extra_arg in &self.config.extra_args {
            args.push(extra_arg.clone());
        }

        Ok(args)
    }

    fn validate_path(path: &str, name: &str) -> Result<()> {
        if path.is_empty() {
            return Err(anyhow!("{} path is empty", name));
        }
        let abs_path = std::fs::canonicalize(Path::new(path).parent().unwrap_or(Path::new("/")))
            .context(format!("failed to canonicalize {} path", name))?;
        if !abs_path.exists() {
            return Err(anyhow!("{} path {} does not exist", name, path));
        }
        Ok(())
    }

    pub async fn start(&self) -> Result<u32> {
        Self::validate_path(&self.config.sock_path, "socket")?;
        Self::validate_path(&self.config.api_sock_path, "api socket")?;
        Self::validate_path(&self.config.path, "daemon")?;
        Self::validate_path(&self.config.source_path, "source")?;

        let args = self.build_args()?;
        info!(
            sl!(),
            "X starting nydusd with path: {} args: {:?}",
            self.config.path,
            args
        );

        let mut cmd = Command::new(&self.config.path);
        cmd.args(&args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = cmd.spawn().context("failed to spawn nydusd process")?;
        let pid = child.id().ok_or_else(|| anyhow!("failed to get nydusd pid"))?;

        let stdout = child.stdout.take().ok_or_else(|| anyhow!("failed to capture stdout"))?;
        let stderr = child.stderr.take().ok_or_else(|| anyhow!("failed to capture stderr"))?;

        tokio::spawn(async move {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                info!(sl!(), "nydusd start: {}", line);
            }
        });

        tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                info!(sl!(), "nydusd stdout: {}", line);
            }
        });

        {
            let mut inner = self.inner.write().await;
            inner.pid = Some(pid);
            inner.child = Some(child);
        }

        info!(sl!(), "nydusd started with pid {}, waiting for API server ready", pid);

        let client = NydusClient::new(&self.config.api_sock_path);
        client
            .wait_until_ready(NYDUSD_WAIT_READY_ATTEMPTS, NYDUSD_WAIT_READY_DELAY_MS)
            .await
            .context("nydusd API server not ready")?;

        info!(sl!(), "nydusd API server ready, setting up passthrough fs");
        self.setup_passthrough_fs().await?;

        info!(sl!(), "nydusd setup completed");
        Ok(pid)
    }

    async fn setup_passthrough_fs(&self) -> Result<()> {
        let client = NydusClient::new(&self.config.api_sock_path);
        let req = MountRequest::new(NYDUS_PASSTHROUGH_FS, &self.config.source_path, "");
        
        info!(
            sl!(),
            "mounting passthrough fs from {} to {}",
            self.config.source_path,
            SHARED_PATH_IN_GUEST
        );

        client
            .mount(SHARED_PATH_IN_GUEST, &req)
            .await
            .context("failed to mount passthrough fs")?;

        Ok(())
    }

    pub async fn mount_rafs(&self, mountpoint: &str, source: &str, config: &str) -> Result<()> {
        let client = NydusClient::new(&self.config.api_sock_path);
        let req = MountRequest::new(NYDUS_RAFS, source, config);

        info!(
            sl!(),
            "mounting rafs from {} to {}",
            source,
            mountpoint
        );

        client
            .mount(mountpoint, &req)
            .await
            .context("failed to mount rafs")?;

        info!(sl!(), "rafs mounted successfully at {}", mountpoint);
        Ok(())
    }

    /// Mount rafs with nydusd native overlay support
    /// This creates a writable overlay filesystem using nydusd's built-in overlay implementation
    /// The overlay config should contain upper_dir and work_dir for the overlay
    pub async fn mount_rafs_with_overlay(
        &self,
        mountpoint: &str,
        source: &str,
        config: &str,
        overlay_config: &str,
    ) -> Result<()> {
        let client = NydusClient::new(&self.config.api_sock_path);
        let req = MountRequest::new_with_overlay(NYDUS_RAFS, source, config, overlay_config);

        info!(
            sl!(),
            "mounting rafs with overlay from {} to {}, overlay_config: {}",
            source,
            mountpoint,
            overlay_config
        );

        client
            .mount(mountpoint, &req)
            .await
            .context("failed to mount rafs with overlay")?;

        info!(sl!(), "rafs with overlay mounted successfully at {}", mountpoint);
        Ok(())
    }

    pub async fn umount(&self, mountpoint: &str) -> Result<()> {
        let client = NydusClient::new(&self.config.api_sock_path);

        info!(sl!(), "unmounting {}", mountpoint);

        client
            .umount(mountpoint)
            .await
            .context("failed to umount")?;

        info!(sl!(), "unmounted {}", mountpoint);
        Ok(())
    }

    pub async fn stop(&self) -> Result<()> {
        let mut inner = self.inner.write().await;

        if let Some(pid) = inner.pid.take() {
            info!(sl!(), "stopping nydusd with pid {}", pid);

            if let Some(mut child) = inner.child.take() {
                let _ = child.kill().await;
                let _ = child.wait().await;
            }

            if let Err(e) = tokio::fs::remove_file(&self.config.sock_path).await {
                warn!(sl!(), "failed to remove socket {}: {}", self.config.sock_path, e);
            }
            if let Err(e) = tokio::fs::remove_file(&self.config.api_sock_path).await {
                warn!(sl!(), "failed to remove api socket {}: {}", self.config.api_sock_path, e);
            }

            info!(sl!(), "nydusd stopped");
        }

        Ok(())
    }

    pub async fn get_pid(&self) -> Option<u32> {
        let inner = self.inner.read().await;
        inner.pid
    }
}