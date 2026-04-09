// Copyright (c) 2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//
//

use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use kata_types::rootless::is_rootless;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::RwLock;

use crate::share_fs::nydus::{nydus_client::NydusClient, MountRequest};

/// passthrough_fs is a special filesystem type in nydus which simply passthroughs the source directory
/// to the guest without any caching or overlay.
pub const NYDUS_PASSTHROUGH_FS: &str = "passthrough_fs";
/// RAFS filesystem type for nydus. This is used to tell nydusd to mount a RAFS filesystem.
pub const NYDUS_RAFS: &str = "rafs";
/// The mountpoint for passthrough_fs inside the nydusd virtiofs namespace.
/// This is NOT a guest absolute path; it's a path within the virtiofs namespace.
/// When the guest mounts virtiofs at `/run/kata-containers/shared/`, this maps to
/// `/run/kata-containers/shared/containers/` in the guest.
pub const SHARED_PATH_IN_GUEST: &str = "/containers";

/// The number of attempts to check if nydusd API server is ready after starting nydusd.
const NYDUSD_WAIT_READY_ATTEMPTS: u32 = 100;
/// The delay in milliseconds between each attempt to check if nydusd API server is ready.
const NYDUSD_WAIT_READY_DELAY_MS: u64 = 100;

/// PathType is used to specify the expected type of a path for validation purposes.
/// - Socket: the path is expected to be a socket file and it is used for nydusd's API and data sockets.
/// - File: the path is expected to be a regular file and it is used for the nydusd binary path.
/// - Directory: the path is expected to be a directory and it is used for the source directory of the passthrough_fs.
enum PathType {
    Socket,
    File,
    Directory,
}

#[derive(Clone, Debug)]
pub struct NydusdConfig {
    pub path: PathBuf,
    pub sock_path: PathBuf,
    pub api_sock_path: PathBuf,
    pub source_path: PathBuf,
    pub debug: bool,
    pub extra_args: Vec<String>,
}

#[allow(dead_code)]
impl NydusdConfig {
    pub fn new(
        path: PathBuf,
        sock_path: PathBuf,
        api_sock_path: PathBuf,
        source_path: PathBuf,
        debug: bool,
        extra_args: Vec<String>,
    ) -> Self {
        Self {
            path,
            sock_path,
            api_sock_path,
            source_path,
            debug,
            extra_args,
        }
    }

    pub fn validate(&self) -> Result<Self> {
        validate_path(&self.path, PathType::File)?;
        validate_path(&self.sock_path, PathType::Socket)?;
        validate_path(&self.api_sock_path, PathType::Socket)?;
        validate_path(&self.source_path, PathType::Directory)?;

        Ok(self.clone())
    }
}

struct NydusdInner {
    pid: Option<u32>,
    child: Option<Child>,
}

pub struct Nydusd {
    config: NydusdConfig,
    inner: Arc<RwLock<NydusdInner>>,
}

#[allow(dead_code)]
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

        // In rootless mode the jailer prefix can make absolute socket paths exceed
        // the unix socket path length limit (typically 108 bytes), which would make
        // nydusd fail to bind its data/API sockets. Mirror the virtiofsd workaround:
        // pass short, relative socket file names and rely on the process working
        // directory being set to the socket parent directory (see `start()`).
        let (sock_arg, api_sock_arg) = if is_rootless() {
            (
                socket_file_name(&self.config.sock_path, "sock")?,
                socket_file_name(&self.config.api_sock_path, "api sock")?,
            )
        } else {
            (
                self.config.sock_path.to_string_lossy().to_string(),
                self.config.api_sock_path.to_string_lossy().to_string(),
            )
        };

        let mut args = vec![
            "virtiofs".to_string(),
            "--hybrid-mode".to_string(),
            "--log-level".to_string(),
            log_level.to_string(),
            "--apisock".to_string(),
            api_sock_arg,
            "--sock".to_string(),
            sock_arg,
        ];

        for extra_arg in &self.config.extra_args {
            args.push(extra_arg.clone());
        }

        Ok(args)
    }

    pub async fn start(&self) -> Result<u32> {
        // Before starting nydusd, we need to clean up any stale socket files
        // that might exist from a previous run.
        cleanup_socket(&self.config.sock_path).await?;
        cleanup_socket(&self.config.api_sock_path).await?;

        let args = self.build_args()?;
        info!(
            sl!(),
            "starting nydusd with path: {:?} args: {:?}", self.config.path, args
        );

        let mut cmd = Command::new(&self.config.path);
        cmd.args(&args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        if is_rootless() {
            // `build_args()` uses relative socket file names in rootless mode; run
            // nydusd from the socket parent directory so the short names resolve and
            // the bound socket files still land at the configured absolute paths.
            let work_dir = self
                .config
                .sock_path
                .parent()
                .ok_or_else(|| anyhow!("failed to get parent dir of {:?}", self.config.sock_path))?;
            cmd.current_dir(work_dir);
        }

        let mut child = cmd.spawn().context("failed to spawn nydusd process")?;
        let pid = child
            .id()
            .ok_or_else(|| anyhow!("failed to get nydusd pid"))?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("failed to capture stdout"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| anyhow!("failed to capture stderr"))?;

        tokio::spawn(async move {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                // It's not error here.
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

        info!(
            sl!(),
            "nydusd started with pid {}, waiting for API server ready", pid
        );

        let startup_result: Result<()> = async {
            let client = NydusClient::new(&self.config.api_sock_path);
            client
                .wait_until_ready(NYDUSD_WAIT_READY_ATTEMPTS, NYDUSD_WAIT_READY_DELAY_MS)
                .await
                .context("nydusd API server not ready")?;

            info!(sl!(), "nydusd API server ready, setting up passthrough fs");
            self.setup_passthrough_fs().await
        }
        .await;

        // As `wait_until_ready()` or `setup_passthrough_fs()` can fail after nydusd
        // has already been spawned and stored in `self.inner`, so clean it up here
        // to avoid leaking the process and stale socket files on startup failure.
        if let Err(err) = startup_result {
            if let Err(stop_err) = self.stop().await {
                warn!(
                    sl!(),
                    "failed to clean up nydusd after startup error: {}", stop_err
                );
            }

            return Err(err);
        }

        info!(sl!(), "nydusd setup completed");

        Ok(pid)
    }

    async fn setup_passthrough_fs(&self) -> Result<()> {
        let client = NydusClient::new(&self.config.api_sock_path);
        let req = MountRequest::new(NYDUS_PASSTHROUGH_FS, &self.config.source_path, "");

        info!(
            sl!(),
            "mounting passthrough fs from {:?} to {}",
            self.config.source_path,
            SHARED_PATH_IN_GUEST
        );

        client
            .mount(SHARED_PATH_IN_GUEST, &req)
            .await
            .context("failed to mount passthrough fs")?;

        Ok(())
    }

    pub async fn mount_rafs(&self, mountpoint: &str, source: &PathBuf, config: &str) -> Result<()> {
        let client = NydusClient::new(&self.config.api_sock_path);
        let req = MountRequest::new(NYDUS_RAFS, source, config);

        info!(sl!(), "mounting rafs from {:?} to {}", source, mountpoint);

        client
            .mount(mountpoint, &req)
            .await
            .context("failed to mount rafs")?;

        info!(sl!(), "rafs mounted successfully at {}", mountpoint);
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
        let (pid, child) = {
            let mut inner = self.inner.write().await;
            (inner.pid.take(), inner.child.take())
        };

        if let Some(pid) = pid {
            info!(sl!(), "stopping nydusd with pid {}", pid);

            if let Some(mut child) = child {
                let _ = child.kill().await;
                let _ = child.wait().await;
            }

            // Clean up the socket files created by nydusd
            cleanup_socket(&self.config.sock_path).await?;
            cleanup_socket(&self.config.api_sock_path).await?;

            info!(sl!(), "nydusd stopped");
        }

        Ok(())
    }

    pub async fn get_pid(&self) -> Option<u32> {
        let inner = self.inner.read().await;
        inner.pid
    }
}

/// Extract the file name component of a socket path as a string, used to build a
/// short relative socket path in rootless mode.
fn socket_file_name(path: &Path, name: &str) -> Result<String> {
    Ok(path
        .file_name()
        .ok_or_else(|| anyhow!("failed to get {} file name of {:?}", name, path))?
        .to_string_lossy()
        .to_string())
}

async fn cleanup_socket(path: &Path) -> Result<()> {
    match tokio::fs::remove_file(path).await {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err).context(format!("failed to remove socket {:?}", path)),
    }
}

/// validate that the path exists and is of the expected type
fn validate_path(path: &PathBuf, path_type: PathType) -> Result<()> {
    if path.as_os_str().is_empty() {
        return Err(anyhow!("path is empty"));
    }

    let parent = path.parent().unwrap_or(Path::new("/"));
    std::fs::canonicalize(parent)
        .context(format!("failed to canonicalize parent path {:?}", parent))?;

    match path_type {
        PathType::Socket => Ok(()),
        PathType::File => {
            if !path.exists() {
                return Err(anyhow!("path {:?} does not exist", path));
            }

            if !path.is_file() {
                return Err(anyhow!("path {:?} is not a file", path));
            }

            Ok(())
        }
        PathType::Directory => {
            if !path.exists() {
                return Err(anyhow!("path {:?} does not exist", path));
            }

            if !path.is_dir() {
                return Err(anyhow!("path {:?} is not a directory", path));
            }

            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_nydusd_config_validate_checks_all_paths() {
        let dir = tempdir().unwrap();
        let daemon_path = dir.path().join("nydusd");
        let source_path = dir.path().join("source");
        let sock_path = dir.path().join("nydusd.sock");
        let api_sock_path = dir.path().join("nydusd-api.sock");

        fs::write(&daemon_path, b"binary").unwrap();
        fs::create_dir(&source_path).unwrap();

        let config = NydusdConfig::new(
            daemon_path,
            sock_path,
            api_sock_path,
            source_path,
            false,
            vec![],
        );

        assert!(config.validate().is_ok());
    }
}
