// Copyright (c) 2022-2023 Alibaba Cloud
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use hyper::{Body, Client, Method, Request, StatusCode, Uri};
use hyperlocal::UnixConnector;
use kata_types::mount::Mount;
use nix::sys::signal::{kill, Signal};
use nix::unistd::Pid;
use oci_spec::runtime as oci;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

pub const NYDUS_ROOTFS_TYPE: &str = "fuse.nydus-overlayfs";
const NYDUS_ROOTFS_V5: &str = "v5";
const NYDUS_ROOTFS_V6: &str = "v6";
const SNAPSHOT_DIR: &str = "snapshotdir";
const KATA_OVERLAY_DEV_TYPE: &str = "overlayfs";

const NYDUS_PREFETCH_FILE_LIST: &str = "prefetch_file.list";

const NYDUS_RAFS_FS_TYPE: &str = "rafs";
const NYDUSD_DAEMON_STATE_RUNNING: &str = "RUNNING";
const NYDUSD_STOP_TIMEOUT_SECS: u64 = 5;
const NYDUS_PASSTHROUGH_FS_TYPE: &str = "passthrough_fs";
const SHARED_PATH_IN_GUEST: &str = "/containers";

// API Endpoints
const INFO_ENDPOINT: &str = "http://unix/api/v1/daemon";
const MOUNT_ENDPOINT: &str = "http://unix/api/v1/mount";

#[async_trait]
pub trait Nydusd: Send + Sync {
    async fn mount(&self, source: &str) -> Result<String>;
    async fn umount(&self, mountpoint: &str) -> Result<()>;
    async fn start(&self, _on_quit: Box<dyn FnOnce() + Send + Sync>) -> Result<u32>;
    async fn stop(&self) -> Result<()>;
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct NydusdImpl {
    path: PathBuf,
    sock_path: PathBuf,
    api_sock_path: PathBuf,
    source_path: PathBuf,
    bootstrap_path: Option<PathBuf>,
    extra_args: Vec<String>,
    pid: Arc<tokio::sync::RwLock<Option<u32>>>,
    debug: bool,
}

#[derive(Debug, Default, Clone)]
pub struct MountOption {
    pub source: String,
    pub config: String,
    pub mountpoint: String,
}

impl NydusdImpl {
    #[allow(dead_code)]
    pub fn new(
        path: &str,
        sock_path: &str,
        api_sock_path: &str,
        source_path: &str,
        bootstrap_path: Option<&str>,
        extra_args: Vec<String>,
        debug: bool,
    ) -> Self {
        NydusdImpl {
            path: PathBuf::from(path),
            sock_path: PathBuf::from(sock_path),
            api_sock_path: PathBuf::from(api_sock_path),
            source_path: PathBuf::from(source_path),
            bootstrap_path: bootstrap_path.map(PathBuf::from),
            extra_args,
            pid: Arc::new(tokio::sync::RwLock::new(None)),
            debug,
        }
    }

    async fn get_client(&self) -> Result<NydusClient> {
        let api_sock_str = self
            .api_sock_path
            .to_str()
            .ok_or(anyhow!("Invalid api sock path string"))?;
        NydusClient::new(api_sock_str).await
    }

    #[allow(dead_code)]
    async fn kill(&self) -> Result<()> {
        let pid_lock = self.pid.read().await;
        if let Some(pid) = *pid_lock {
            println!("Stopping nydusd daemon: pid={}", pid);
            let pid = Pid::from_raw(pid as i32);
            if let Err(e) = kill(pid, Signal::SIGTERM) {
                eprintln!("Failed to send SIGTERM to nydusd: pid={}, error={}", pid, e);
            }
            // Simple wait, a more robust implementation would check if the process is still alive.
            sleep(Duration::from_secs(NYDUSD_STOP_TIMEOUT_SECS)).await;
        }
        *self.pid.write().await = None;
        Ok(())
    }

    #[allow(dead_code)]
    async fn wait_until_api_server_ready(&self) -> Result<()> {
        let api_sock_str = self
            .api_sock_path
            .to_str()
            .ok_or(anyhow!("Invalid api sock path string"))?;

        println!("Checking nydusd API server readiness at: {}", api_sock_str);
        
        for i in 0..20 {
            println!("API readiness check attempt {}/20", i + 1);
            match NydusClient::new(api_sock_str).await {
                Ok(client) => {
                    println!("Successfully created NydusClient, checking status...");
                    match client.check_status().await {
                        Ok(info) => {
                            println!("API status response: state={}", info.state);
                            if info.state == NYDUSD_DAEMON_STATE_RUNNING {
                                println!("Nydusd API server is ready!");
                                return Ok(());
                            }
                        }
                        Err(e) => {
                            println!("Failed to check status: {}", e);
                        }
                    }
                }
                Err(e) => {
                    println!("Failed to create NydusClient: {}", e);
                }
            }
            sleep(Duration::from_millis(100)).await;
        }
        Err(anyhow!("Failed to wait for nydusd API server to be ready after 20 attempts"))
    }

    #[allow(dead_code)]
    async fn setup_passthrough_fs(&self) -> Result<()> {
        let client = self.get_client().await?;
        let source_str = self
            .source_path
            .to_str()
            .ok_or(anyhow!("Invalid source path string"))?;
        let mount_req = MountRequest::new(NYDUS_PASSTHROUGH_FS_TYPE, source_str, "");

        client.mount(SHARED_PATH_IN_GUEST, &mount_req).await
    }

    // Corresponds to nydusd.valid()
    #[allow(dead_code)]
    fn valid(&self) -> Result<()> {
        check_path_valid_is_dir(
            &self
                .api_sock_path
                .parent()
                .ok_or(anyhow!("Invalid api sock path"))?,
        )
        .context("api sock path's parent directory does not exist")?;
        check_path_valid_is_dir(&self.source_path)
            .context("source path directory does not exist")?;
        Ok(())
    }

    // Corresponds to nydusd.args()
    #[allow(dead_code)]
    fn args(&self) -> Result<Vec<String>> {
        let log_level = if self.debug { "debug" } else { "info" };
        let mut args = vec![
            "virtiofs".to_string(),  // Back to virtiofs mode like runtime-go
            "--log-level".to_string(),
            log_level.to_string(),
            "--apisock".to_string(),
            self.api_sock_path
                .to_str()
                .ok_or(anyhow!("Invalid api sock path string"))?
                .to_string(),
            "--sock".to_string(),  // virtiofs mode uses --sock parameter
            self.sock_path
                .to_str()
                .ok_or(anyhow!("Invalid sock path string"))?
                .to_string(),
        ];
        
        // Note: In virtiofs mode, bootstrap is provided via API calls, not command line
        args.extend_from_slice(&self.extra_args);
        Ok(args)
    }
}

#[async_trait]
impl Nydusd for NydusdImpl {
    async fn start(&self, _on_quit: Box<dyn FnOnce() + Send + Sync>) -> Result<u32> {
        // Create necessary directories before validation with enhanced error handling
        // For fuse mode, we only need API socket parent directory and mountpoint (source_path)
        if let Some(parent) = self.api_sock_path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                return Err(anyhow::anyhow!("failed to create API socket path parent directory {:?}: {}", parent, e));
            }
            // Set directory permissions to allow socket creation
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(parent)?.permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(parent, perms)?;
        }
        if let Err(e) = std::fs::create_dir_all(&self.source_path) {
            return Err(anyhow::anyhow!("failed to create source path directory {:?}: {}", self.source_path, e));
        }

        // Verify directories exist before proceeding
        if !self.source_path.exists() {
            return Err(anyhow::anyhow!("source path directory does not exist after creation: {:?}", self.source_path));
        }

        // Validate paths and configuration
        self.valid().context("nydusd validation failed")?;

        // Build arguments for nydusd
        let args = self.args().context("failed to build nydusd arguments")?;
        
        // Write debug info to a file
        let debug_file = format!("/tmp/nydusd-debug-{}.log", std::process::id());
        let mut debug_info = format!(
            "Starting nydusd daemon (virtiofs mode):\nPath: {:?}\nArgs: {:?}\nSock path: {:?}\nAPI sock path: {:?}\nSource path: {:?}\nSource path exists: {}\n",
            self.path, args, self.sock_path, self.api_sock_path, self.source_path, self.source_path.exists()
        );
        std::fs::write(&debug_file, &debug_info).unwrap_or_default();
        
        // Start nydusd process
        let mut cmd = tokio::process::Command::new(&self.path);
        cmd.args(&args);
        
        let mut child = cmd.spawn().context("failed to spawn nydusd process")?;
        let pid = child.id().ok_or_else(|| anyhow!("failed to get nydusd PID"))?;
        
        debug_info.push_str(&format!("Nydusd process spawned with PID: {}\n", pid));
        debug_info.push_str(&format!("Source path exists after spawn: {}\n", self.source_path.exists()));
        std::fs::write(&debug_file, &debug_info).unwrap_or_default();
        
        // Store the PID
        *self.pid.write().await = Some(pid);
        
        // Wait for API server to be ready
        debug_info.push_str("Waiting for nydusd API server to be ready...\n");
        std::fs::write(&debug_file, &debug_info).unwrap_or_default();
        self.wait_until_api_server_ready().await
            .context("nydusd API server failed to become ready")?;
        
        // Setup passthrough filesystem
        self.setup_passthrough_fs().await
            .context("failed to setup passthrough filesystem")?;
        
        println!("Nydusd daemon started successfully with PID: {}", pid);
        
        // Spawn a task to monitor the process
        let pid_ref = self.pid.clone();
        tokio::spawn(async move {
            let _result = child.wait().await;
            println!("Nydusd daemon has quit");
            *pid_ref.write().await = None;
            // Note: In a real implementation, we'd call the on_quit callback here
        });
        
        Ok(pid)
    }

    async fn stop(&self) -> Result<()> {
        let pid_lock = self.pid.read().await;
        if let Some(pid) = *pid_lock {
            drop(pid_lock); // Release the read lock before calling kill
            self.kill().await?;
        }
        
        // Clean up socket files
        if self.sock_path.exists() {
            if let Err(e) = std::fs::remove_file(&self.sock_path) {
                eprintln!("Failed to remove socket file {:?}: {}", self.sock_path, e);
            }
        }
        
        if self.api_sock_path.exists() {
            if let Err(e) = std::fs::remove_file(&self.api_sock_path) {
                eprintln!("Failed to remove API socket file {:?}: {}", self.api_sock_path, e);
            }
        }
        
        println!("Nydusd daemon stopped");
        Ok(())
    }

    async fn mount(&self, source: &str) -> Result<String> {
        let client = self.get_client().await?;
        let mountpoint = self.source_path.join(source);
        let mountpoint_str = mountpoint.to_str().ok_or(anyhow!("Invalid mountpoint string"))?;
        println!("Mounting rafs: source={}, mountpoint={}", source, mountpoint_str);
        let req = MountRequest::new(NYDUS_RAFS_FS_TYPE, source, "");
        client.mount(mountpoint_str, &req).await?;
        Ok(mountpoint_str.to_string())
    }

    async fn umount(&self, mountpoint: &str) -> Result<()> {
        let client = self.get_client().await?;
        println!("Unmounting rafs: mountpoint={}", mountpoint);
        client.umount(mountpoint).await
    }
}

// Utility functions
pub fn is_nydus_rootfs(m: &Mount) -> bool {
    m.fs_type == NYDUS_ROOTFS_TYPE
}

pub fn is_kata_overlayfs(m: &Mount) -> bool {
    m.fs_type == KATA_OVERLAY_DEV_TYPE
}

pub fn is_nydus_snapshot_v5(m: &Mount) -> bool {
    for o in &m.options {
        if o.contains(SNAPSHOT_DIR) && o.contains(NYDUS_ROOTFS_V5) {
            return true;
        }
    }
    false
}

pub fn is_nydus_snapshot_v6(m: &Mount) -> bool {
    for o in &m.options {
        if o.contains(SNAPSHOT_DIR) && o.contains(NYDUS_ROOTFS_V6) {
            return true;
        }
    }
    false
}

pub fn get_nydus_prefetch_files(spec: &oci::Spec) -> Option<Vec<String>> {
    if let Some(annotations) = spec.annotations() {
        if let Some(prefetch_files) = annotations.get(NYDUS_PREFETCH_FILE_LIST) {
            let prefetch_files: Vec<String> = prefetch_files
                .split(',')
                .map(|s| s.trim().to_string())
                .collect();
            return Some(prefetch_files);
        }
    }
    None
}

// Corresponds to checkPathValid() but simplified
#[allow(dead_code)]
fn check_path_valid_is_dir(path: &Path) -> Result<()> {
    if path.as_os_str().is_empty() {
        return Err(anyhow!("path is empty"));
    }
    if !path.is_dir() {
        return Err(anyhow!("path is not a valid directory: {:?}", path));
    }
    Ok(())
}

// --- API Client ---

#[derive(Serialize, Deserialize, Debug)]
struct BuildTimeInfo {
    package_ver: String,
    git_commit: String,
    build_time: String,
    profile: String,
    rustc: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct DaemonInfo {
    id: String,
    version: BuildTimeInfo,
    pub state: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct ErrorMessage {
    code: String,
    message: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct MountRequest {
    fs_type: String,
    source: String,
    config: String,
}

impl MountRequest {
    pub fn new(fs_type: &str, source: &str, config: &str) -> Self {
        MountRequest {
            fs_type: fs_type.to_string(),
            source: source.to_string(),
            config: config.to_string(),
        }
    }
}

#[derive(Debug)]
pub struct NydusClient {
    client: Client<UnixConnector>,
}

impl NydusClient {
    pub async fn new(sock_path: &str) -> Result<Self> {
        wait_until_socket_ready(sock_path, 20, Duration::from_millis(100)).await?;
        let connector = UnixConnector;
        let client = Client::builder().build(connector);
        Ok(NydusClient { client })
    }

    pub async fn check_status(&self) -> Result<DaemonInfo> {
        let uri: Uri = INFO_ENDPOINT.parse()?;
        let resp = self.client.get(uri).await?;
        let body_bytes = hyper::body::to_bytes(resp.into_body()).await?;
        let info: DaemonInfo = serde_json::from_slice(&body_bytes)?;
        Ok(info)
    }

    pub async fn mount(&self, mount_point: &str, req: &MountRequest) -> Result<()> {
        let uri_str = format!("{}?mountpoint={}", MOUNT_ENDPOINT, mount_point);
        let uri: Uri = uri_str.parse()?;
        let body_str = serde_json::to_string(req)?;

        let http_req = Request::builder()
            .method(Method::POST)
            .uri(uri)
            .header("Content-Type", "application/json")
            .body(Body::from(body_str))?;

        let resp = self.client.request(http_req).await?;

        if resp.status() == StatusCode::NO_CONTENT {
            Ok(())
        } else {
            handle_mount_error(resp.into_body()).await
        }
    }

    pub async fn umount(&self, mount_point: &str) -> Result<()> {
        let uri_str = format!("{}?mountpoint={}", MOUNT_ENDPOINT, mount_point);
        let uri: Uri = uri_str.parse()?;

        let http_req = Request::builder()
            .method(Method::DELETE)
            .uri(uri)
            .body(Body::empty())?;

        let resp = self.client.request(http_req).await?;

        if resp.status() == StatusCode::NO_CONTENT {
            Ok(())
        } else {
            handle_mount_error(resp.into_body()).await
        }
    }
}

async fn wait_until_socket_ready(sock: &str, attempts: u32, delay: Duration) -> Result<()> {
    println!("Waiting for socket {} to be ready, attempts: {}", sock, attempts);
    for i in 0..attempts {
        if Path::new(sock).exists() {
            println!("Socket {} is ready after {} attempts", sock, i + 1);
            return Ok(());
        }
        println!("Socket {} not ready, attempt {}/{}", sock, i + 1, attempts);
        sleep(delay).await;
    }
    Err(anyhow!("Nydus socket not ready after {} attempts", attempts))
}

async fn handle_mount_error(body: Body) -> Result<()> {
    let body_bytes = hyper::body::to_bytes(body).await?;
    let err_msg: ErrorMessage = serde_json::from_slice(&body_bytes)?;
    Err(anyhow!(
        "Nydus API error: Code={}, Message={}",
        err_msg.code,
        err_msg.message
    ))
}
