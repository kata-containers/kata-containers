// Copyright (c) 2026 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

//! External OpenVMM process wrapper using OpenVMM's TTRPC VM service.

use anyhow::{anyhow, Context, Result};
use std::os::fd::AsRawFd;
use std::path::Path;
use std::process::Stdio;
use std::time::{Duration, Instant};
use tokio::process::{Child, Command};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use super::empty::Empty;
use super::inner_hypervisor::blk_device_kind;
use super::vmservice;
use super::vmservice_ttrpc::VmClient;
use protobuf::MessageField;

const OPENVMM_READY_TIMEOUT: Duration = Duration::from_secs(20);
const OPENVMM_STOP_TIMEOUT: Duration = Duration::from_secs(5);
const OPENVMM_RPC_TIMEOUT: Duration = Duration::from_secs(30);

/// Wrapper around an external OpenVMM process, providing VM lifecycle control.
pub(crate) struct VmmInstance {
    pid: Option<u32>,
    ttrpc_socket_path: Option<String>,
    child: Option<Child>,
    wait_task: Option<JoinHandle<()>>,
    exit_notify: Option<mpsc::Sender<i32>>,
    /// Persistent ttrpc async client for the OpenVMM `vmservice.VM` service,
    /// established once the process is launched and reused for every RPC.
    client: Option<VmClient>,
}

impl std::fmt::Debug for VmmInstance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VmmInstance")
            .field("pid", &self.pid)
            .field("ttrpc_socket_path", &self.ttrpc_socket_path)
            .finish()
    }
}

impl VmmInstance {
    pub(crate) fn new(exit_notify: mpsc::Sender<i32>) -> Self {
        Self {
            pid: None,
            ttrpc_socket_path: None,
            child: None,
            wait_task: None,
            exit_notify: Some(exit_notify),
            client: None,
        }
    }

    pub(crate) async fn launch(
        &mut self,
        openvmm_path: &str,
        ttrpc_socket_path: String,
        request: vmservice::CreateVMRequest,
        netns: Option<String>,
        log_dir: Option<String>,
    ) -> Result<()> {
        self.launch_with_timeout(
            openvmm_path,
            ttrpc_socket_path,
            request,
            netns,
            log_dir,
            OPENVMM_READY_TIMEOUT,
        )
        .await
    }

    async fn launch_with_timeout(
        &mut self,
        openvmm_path: &str,
        ttrpc_socket_path: String,
        request: vmservice::CreateVMRequest,
        netns: Option<String>,
        log_dir: Option<String>,
        ready_timeout: Duration,
    ) -> Result<()> {
        if self.pid.is_some() || self.child.is_some() || self.wait_task.is_some() {
            anyhow::bail!("openvmm process is already running");
        }

        let _ = std::fs::remove_file(&ttrpc_socket_path);

        let mut command = Command::new(openvmm_path);
        command
            .arg("--ttrpc")
            .arg(&ttrpc_socket_path)
            .stdin(Stdio::null())
            .kill_on_drop(true);

        if let Some(log_dir) = &log_dir {
            std::fs::create_dir_all(log_dir)
                .with_context(|| format!("failed to create openvmm log dir {log_dir}"))?;
            let log_path = Path::new(log_dir).join("openvmm.log");
            let log_file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&log_path)
                .with_context(|| {
                    format!("failed to create openvmm log file {}", log_path.display())
                })?;
            command
                .stdout(Stdio::from(
                    log_file.try_clone().context("failed to clone log file")?,
                ))
                .stderr(Stdio::from(log_file));
        } else {
            command.stdout(Stdio::null()).stderr(Stdio::null());
        }

        if let Some(netns_path) = netns.filter(|path| !path.is_empty()) {
            // Open the namespace before fork. A pre_exec callback runs in the
            // forked child of this multithreaded process and therefore must not
            // allocate or acquire userspace locks.
            let netns = std::fs::File::open(&netns_path)
                .with_context(|| format!("failed to open network namespace {netns_path}"))?;
            unsafe {
                command.pre_exec(move || {
                    if libc::setns(netns.as_raw_fd(), libc::CLONE_NEWNET) == 0 {
                        Ok(())
                    } else {
                        Err(std::io::Error::last_os_error())
                    }
                });
            }
        }

        info!(
            sl!(),
            "openvmm: launching external process path={} socket={}",
            openvmm_path,
            ttrpc_socket_path
        );

        let mut child = command
            .spawn()
            .with_context(|| format!("failed to spawn openvmm at {openvmm_path}"))?;
        let pid = match child.id() {
            Some(pid) => pid,
            None => {
                terminate_child(&mut child, 0).await;
                anyhow::bail!("failed to get openvmm pid");
            }
        };
        info!(sl!(), "openvmm: spawned external process pid={}", pid);

        let setup_result = async {
            // Wait for the OpenVMM TTRPC server to start accepting connections.
            // `Client::connect` performs a one-shot `connect(2)`, so
            // `connect_client` retries until the child has created the socket and
            // begun listening (or the timeout elapses). This single readiness
            // loop intentionally replaces a separate "does the socket file exist
            // yet?" poll: a missing socket file and a not-yet-listening server are
            // both just transient connect errors that the retry already handles.
            let client = connect_client(&ttrpc_socket_path, ready_timeout)
                .await
                .with_context(|| {
                    format!("openvmm TTRPC socket did not become ready: {ttrpc_socket_path}")
                })?;
            info!(sl!(), "openvmm: TTRPC connected pid={}", pid);
            info!(sl!(), "openvmm: creating VM pid={}", pid);
            Self::create_vm(&client, request).await?;
            info!(sl!(), "openvmm: VM created pid={}", pid);
            Ok::<VmClient, anyhow::Error>(client)
        }
        .await;

        let client = match setup_result {
            Ok(client) => client,
            Err(err) => {
                terminate_child(&mut child, pid).await;
                let _ = std::fs::remove_file(&ttrpc_socket_path);
                return Err(err);
            }
        };

        self.pid = Some(pid);
        self.ttrpc_socket_path = Some(ttrpc_socket_path);
        self.child = Some(child);
        self.client = Some(client);

        Ok(())
    }

    pub(crate) fn start_wait_task(&mut self) -> Result<()> {
        if self.wait_task.is_some() {
            anyhow::bail!("openvmm wait task is already running");
        }

        let pid = self.pid.context("openvmm process has no pid")?;
        let Some(mut child) = self.child.take() else {
            anyhow::bail!("openvmm process is not awaiting startup commit");
        };
        let exit_notify = self.exit_notify.clone();
        self.wait_task = Some(tokio::spawn(async move {
            let exit_code = match child.wait().await {
                Ok(status) => status.code().unwrap_or(1),
                Err(err) => {
                    warn!(
                        sl!(),
                        "openvmm: failed waiting for process {}: {:?}", pid, err
                    );
                    1
                }
            };

            if let Some(exit_notify) = exit_notify {
                let _ = exit_notify.try_send(exit_code);
            }
        }));

        Ok(())
    }

    pub(crate) async fn resume(&self) -> Result<()> {
        self.client()?
            .resume_vm(rpc_ctx(), &Empty::new())
            .await
            .map(|_| ())
            .map_err(|e| anyhow!("openvmm resume_vm RPC failed: {:?}", e))
    }

    pub(crate) async fn pause(&self) -> Result<()> {
        self.client()?
            .pause_vm(rpc_ctx(), &Empty::new())
            .await
            .map(|_| ())
            .map_err(|e| anyhow!("openvmm pause_vm RPC failed: {:?}", e))
    }

    /// Hot-add a virtio-blk-pci device behind the named (pre-declared) PCIe
    /// hotplug port.
    pub(crate) async fn add_pcie_device(
        &self,
        port_name: &str,
        host_path: String,
        read_only: bool,
    ) -> Result<()> {
        let request = vmservice::AddPcieDeviceRequest {
            port_name: port_name.to_string(),
            device: MessageField::some(blk_device_kind(host_path, read_only)),
            ..Default::default()
        };

        self.client()?
            .add_pcie_device(rpc_ctx(), &request)
            .await
            .map(|_| ())
            .map_err(|e| anyhow!("openvmm add_pcie_device RPC failed: {:?}", e))
    }

    /// Hot-remove the device behind the named PCIe hotplug port.
    pub(crate) async fn remove_pcie_device(&self, port_name: &str) -> Result<()> {
        let request = vmservice::RemovePcieDeviceRequest {
            port_name: port_name.to_string(),
            ..Default::default()
        };

        self.client()?
            .remove_pcie_device(rpc_ctx(), &request)
            .await
            .map(|_| ())
            .map_err(|e| anyhow!("openvmm remove_pcie_device RPC failed: {:?}", e))
    }

    pub(crate) async fn stop(&mut self) -> Result<()> {
        let has_client = self.client.is_some();
        if has_client {
            if let Err(err) = self.teardown_vm().await {
                warn!(sl!(), "openvmm: teardown RPC failed: {:?}", err);
            }
            if let Err(err) = self.quit().await {
                warn!(sl!(), "openvmm: quit RPC failed: {:?}", err);
            }
        }

        if let Some(mut child) = self.child.take() {
            let pid = self.pid.unwrap_or_default();
            if has_client {
                wait_for_child_exit(&mut child, pid).await;
            } else {
                terminate_child(&mut child, pid).await;
            }
        }

        if let Some(mut wait_task) = self.wait_task.take() {
            if let Err(err) = tokio::time::timeout(OPENVMM_STOP_TIMEOUT, &mut wait_task).await {
                warn!(sl!(), "openvmm: process did not exit after quit: {:?}", err);
                if let Some(pid) = self.pid {
                    let _ = nix::sys::signal::kill(
                        nix::unistd::Pid::from_raw(pid as i32),
                        nix::sys::signal::Signal::SIGKILL,
                    );
                }
                if tokio::time::timeout(OPENVMM_STOP_TIMEOUT, &mut wait_task)
                    .await
                    .is_err()
                {
                    wait_task.abort();
                }
            }
        }

        if let Some(socket_path) = self.ttrpc_socket_path.take() {
            let _ = std::fs::remove_file(socket_path);
        }
        self.client = None;
        self.pid = None;

        Ok(())
    }

    pub(crate) fn pid(&self) -> Option<u32> {
        self.pid
    }

    async fn create_vm(client: &VmClient, request: vmservice::CreateVMRequest) -> Result<()> {
        client
            .create_vm(rpc_ctx(), &request)
            .await
            .map(|_| ())
            .map_err(|e| anyhow!("openvmm create_vm RPC failed: {:?}", e))
    }

    async fn teardown_vm(&self) -> Result<()> {
        self.client()?
            .teardown_vm(rpc_ctx(), &Empty::new())
            .await
            .map(|_| ())
            .map_err(|e| anyhow!("openvmm teardown_vm RPC failed: {:?}", e))
    }

    async fn quit(&self) -> Result<()> {
        self.client()?
            .quit(rpc_ctx(), &Empty::new())
            .await
            .map(|_| ())
            .map_err(|e| anyhow!("openvmm quit RPC failed: {:?}", e))
    }

    fn client(&self) -> Result<&VmClient> {
        self.client
            .as_ref()
            .context("openvmm TTRPC client not connected")
    }
}

async fn wait_for_child_exit(child: &mut Child, pid: u32) {
    match tokio::time::timeout(OPENVMM_STOP_TIMEOUT, child.wait()).await {
        Ok(Ok(_)) => {}
        Ok(Err(err)) => {
            warn!(
                sl!(),
                "openvmm: failed waiting for process {}: {:?}", pid, err
            );
            terminate_child(child, pid).await;
        }
        Err(err) => {
            warn!(
                sl!(),
                "openvmm: process {} did not exit after quit: {:?}", pid, err
            );
            terminate_child(child, pid).await;
        }
    }
}

async fn terminate_child(child: &mut Child, pid: u32) {
    match child.try_wait() {
        Ok(Some(_)) => return,
        Ok(None) => {}
        Err(err) => warn!(
            sl!(),
            "openvmm: failed checking process {} status: {:?}", pid, err
        ),
    }

    if let Err(err) = child.start_kill() {
        warn!(sl!(), "openvmm: failed killing process {}: {:?}", pid, err);
    }
    if let Err(err) = child.wait().await {
        warn!(sl!(), "openvmm: failed reaping process {}: {:?}", pid, err);
    }
}

/// Build a per-call ttrpc context carrying the standard OpenVMM RPC timeout.
fn rpc_ctx() -> ttrpc::context::Context {
    ttrpc::context::with_timeout(OPENVMM_RPC_TIMEOUT.as_nanos() as i64)
}

/// Connect a ttrpc async client to the OpenVMM `vmservice` Unix socket, retrying
/// until the server accepts connections or `timeout` elapses.
async fn connect_client(socket_path: &str, timeout: Duration) -> Result<VmClient> {
    let address = format!("unix://{socket_path}");
    let deadline = Instant::now() + timeout;

    loop {
        match ttrpc::asynchronous::Client::connect(&address).await {
            Ok(inner) => return Ok(VmClient::new(inner)),
            Err(err) => {
                if Instant::now() >= deadline {
                    return Err(anyhow!(
                        "failed to connect to openvmm TTRPC socket {socket_path}: {err:?}"
                    ));
                }
                // Back off between attempts so we don't busy-spin on
                // ECONNREFUSED/ENOENT while the child process is still starting.
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::TempDir;
    use tokio::sync::mpsc::error::TryRecvError;

    fn make_nonresponsive_openvmm(temp_dir: &TempDir) -> (String, std::path::PathBuf) {
        let script_path = temp_dir.path().join("openvmm");
        let pid_path = temp_dir.path().join("openvmm.pid");
        fs::write(
            &script_path,
            format!(
                "#!/bin/sh\necho $$ > {}\ntouch \"$2\"\nexec sleep 30\n",
                pid_path.display()
            ),
        )
        .unwrap();
        let mut permissions = fs::metadata(&script_path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&script_path, permissions).unwrap();

        (script_path.to_string_lossy().into_owned(), pid_path)
    }

    fn make_marker_openvmm(temp_dir: &TempDir) -> (String, std::path::PathBuf) {
        let script_path = temp_dir.path().join("openvmm");
        let marker_path = temp_dir.path().join("openvmm-ran");
        fs::write(
            &script_path,
            format!("#!/bin/sh\ntouch {}\n", marker_path.display()),
        )
        .unwrap();
        let mut permissions = fs::metadata(&script_path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&script_path, permissions).unwrap();

        (script_path.to_string_lossy().into_owned(), marker_path)
    }

    #[tokio::test]
    async fn missing_netns_fails_before_spawning_openvmm() {
        let temp_dir = TempDir::new().unwrap();
        let (openvmm_path, marker_path) = make_marker_openvmm(&temp_dir);
        let missing_netns = temp_dir.path().join("missing-netns");
        let (exit_notify, _exit_waiter) = mpsc::channel(1);
        let mut instance = VmmInstance::new(exit_notify);

        let result = instance
            .launch_with_timeout(
                &openvmm_path,
                temp_dir
                    .path()
                    .join("openvmm.sock")
                    .to_string_lossy()
                    .into_owned(),
                vmservice::CreateVMRequest::new(),
                Some(missing_netns.to_string_lossy().into_owned()),
                None,
                Duration::from_millis(100),
            )
            .await;

        assert!(result
            .unwrap_err()
            .to_string()
            .contains("failed to open network namespace"));
        assert!(!marker_path.exists());
        assert!(instance.pid.is_none());
        assert!(instance.child.is_none());
    }

    #[test]
    fn non_netns_fd_makes_spawn_fail_without_running_openvmm() {
        let temp_dir = TempDir::new().unwrap();
        let (openvmm_path, marker_path) = make_marker_openvmm(&temp_dir);
        let socket_path = temp_dir
            .path()
            .join("openvmm.sock")
            .to_string_lossy()
            .into_owned();
        let (done_tx, done_rx) = std::sync::mpsc::channel();

        std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            let (exit_notify, _exit_waiter) = mpsc::channel(1);
            let mut instance = VmmInstance::new(exit_notify);
            let result = runtime.block_on(instance.launch_with_timeout(
                &openvmm_path,
                socket_path,
                vmservice::CreateVMRequest::new(),
                Some("/dev/null".to_string()),
                None,
                Duration::from_millis(100),
            ));
            done_tx.send(result.is_err()).unwrap();
        });

        assert!(done_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("spawn did not return after setns failed"));
        assert!(!marker_path.exists());
    }

    #[tokio::test]
    async fn launch_failure_reaps_child_and_suppresses_exit_notification() {
        let temp_dir = TempDir::new().unwrap();
        let (openvmm_path, pid_path) = make_nonresponsive_openvmm(&temp_dir);
        let socket_path = temp_dir.path().join("openvmm.sock");
        let socket_path_string = socket_path.to_string_lossy().into_owned();
        let (exit_notify, mut exit_waiter) = mpsc::channel(1);
        let mut instance = VmmInstance::new(exit_notify);

        let result = instance
            .launch_with_timeout(
                &openvmm_path,
                socket_path_string,
                vmservice::CreateVMRequest::new(),
                None,
                None,
                Duration::from_millis(100),
            )
            .await;

        assert!(result.is_err());
        let pid: u32 = fs::read_to_string(pid_path)
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        assert!(!Path::new(&format!("/proc/{pid}")).exists());
        assert!(!socket_path.exists());
        assert!(instance.pid.is_none());
        assert!(instance.ttrpc_socket_path.is_none());
        assert!(instance.child.is_none());
        assert!(instance.wait_task.is_none());
        assert!(instance.client.is_none());
        assert!(matches!(exit_waiter.try_recv(), Err(TryRecvError::Empty)));
    }

    #[tokio::test]
    async fn committed_child_exit_sends_notification() {
        let (exit_notify, mut exit_waiter) = mpsc::channel(1);
        let mut instance = VmmInstance::new(exit_notify);
        let mut command = Command::new("/bin/sh");
        command.arg("-c").arg("exit 7").kill_on_drop(true);
        let child = command.spawn().unwrap();
        let pid = child.id().unwrap();
        instance.pid = Some(pid);
        instance.child = Some(child);

        instance.start_wait_task().unwrap();

        let exit_code = tokio::time::timeout(Duration::from_secs(1), exit_waiter.recv())
            .await
            .unwrap();
        assert_eq!(exit_code, Some(7));
        assert!(instance.child.is_none());
        assert!(instance.wait_task.is_some());
    }
}
