// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

use super::inner::CloudHypervisorInner;
use crate::ch::utils::get_api_socket_path;
use crate::ch::utils::{get_jailer_root, get_sandbox_path, get_vsock_path};
use crate::kernel_param::KernelParams;
use crate::Device;
use crate::VsockConfig;
use crate::VM_ROOTFS_DRIVER_PMEM;
use crate::{VcpuThreadIds, VmmState};
use anyhow::{anyhow, Context, Result};
use ch_config::ch_api::{
    cloud_hypervisor_vm_create, cloud_hypervisor_vm_start, cloud_hypervisor_vmm_ping,
    cloud_hypervisor_vmm_shutdown,
};
use ch_config::{NamedHypervisorConfig, VmConfig};
use core::future::poll_fn;
use futures::executor::block_on;
use futures::future::join_all;
use kata_types::capabilities::{Capabilities, CapabilityBits};
use kata_types::config::default::DEFAULT_CH_ROOTFS_TYPE;
use std::convert::TryFrom;
use std::fs::create_dir_all;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::process::Stdio;
use tokio::io::AsyncBufReadExt;
use tokio::io::BufReader;
use tokio::process::{Child, Command};
use tokio::sync::watch::Receiver;
use tokio::task;
use tokio::task::JoinHandle;
use tokio::time::Duration;

const CH_NAME: &str = "cloud-hypervisor";

/// Number of milliseconds to wait before retrying a CH operation.
const CH_POLL_TIME_MS: u64 = 50;

impl CloudHypervisorInner {
    async fn start_hypervisor(&mut self, timeout_secs: i32) -> Result<()> {
        self.cloud_hypervisor_launch(timeout_secs)
            .await
            .context("launch failed")?;

        self.cloud_hypervisor_setup_comms()
            .await
            .context("comms setup failed")?;

        self.cloud_hypervisor_check_running()
            .await
            .context("hypervisor running check failed")?;

        self.state = VmmState::VmmServerReady;

        Ok(())
    }

    async fn get_kernel_params(&self) -> Result<String> {
        let cfg = self
            .config
            .as_ref()
            .ok_or("no hypervisor config for CH")
            .map_err(|e| anyhow!(e))?;

        let enable_debug = cfg.debug_info.enable_debug;

        // Note that the configuration option hypervisor.block_device_driver is not used.
        let rootfs_driver = VM_ROOTFS_DRIVER_PMEM;

        let rootfs_type = match cfg.boot_info.rootfs_type.is_empty() {
            true => DEFAULT_CH_ROOTFS_TYPE,
            false => &cfg.boot_info.rootfs_type,
        };

        // Start by adding the default set of kernel parameters.
        let mut params = KernelParams::new(enable_debug);

        let mut rootfs_param = KernelParams::new_rootfs_kernel_params(rootfs_driver, rootfs_type)?;

        // Add the rootfs device
        params.append(&mut rootfs_param);

        // Finally, add the user-specified options at the end
        // (so they will take priority).
        params.append(&mut KernelParams::from_string(&cfg.boot_info.kernel_params));

        let kernel_params = params.to_string()?;

        Ok(kernel_params)
    }

    async fn boot_vm(&mut self) -> Result<()> {
        let shared_fs_devices = self.get_shared_fs_devices().await?;

        let socket = self
            .api_socket
            .as_ref()
            .ok_or("missing socket")
            .map_err(|e| anyhow!(e))?;

        let sandbox_path = get_sandbox_path(&self.id)?;

        std::fs::create_dir_all(sandbox_path.clone()).context("failed to create sandbox path")?;

        let vsock_socket_path = get_vsock_path(&self.id)?;

        let hypervisor_config = self
            .config
            .as_ref()
            .ok_or("no hypervisor config for CH")
            .map_err(|e| anyhow!(e))?;

        debug!(
            sl!(),
            "generic Hypervisor configuration: {:?}", hypervisor_config
        );

        let kernel_params = self.get_kernel_params().await?;

        let named_cfg = NamedHypervisorConfig {
            kernel_params,
            sandbox_path,
            vsock_socket_path,
            cfg: hypervisor_config.clone(),
            shared_fs_devices,
        };

        let cfg = VmConfig::try_from(named_cfg)?;

        debug!(sl!(), "CH specific VmConfig configuration: {:?}", cfg);

        let response =
            cloud_hypervisor_vm_create(socket.try_clone().context("failed to clone socket")?, cfg)
                .await?;

        if let Some(detail) = response {
            debug!(sl!(), "vm boot response: {:?}", detail);
        }

        let response =
            cloud_hypervisor_vm_start(socket.try_clone().context("failed to clone socket")?)
                .await?;

        if let Some(detail) = response {
            debug!(sl!(), "vm start response: {:?}", detail);
        }

        self.state = VmmState::VmRunning;

        Ok(())
    }

    async fn cloud_hypervisor_setup_comms(&mut self) -> Result<()> {
        let api_socket_path = get_api_socket_path(&self.id)?;

        // The hypervisor has just been spawned, but may not yet have created
        // the API socket, so repeatedly try to connect for up to
        // timeout_secs.
        let join_handle: JoinHandle<Result<UnixStream>> =
            task::spawn_blocking(move || -> Result<UnixStream> {
                let api_socket: UnixStream;

                loop {
                    let result = UnixStream::connect(api_socket_path.clone());

                    if let Ok(result) = result {
                        api_socket = result;
                        break;
                    }

                    std::thread::sleep(Duration::from_millis(CH_POLL_TIME_MS));
                }

                Ok(api_socket)
            });

        let timeout_msg = format!(
            "API socket connect timed out after {} seconds",
            self.timeout_secs
        );

        let result =
            tokio::time::timeout(Duration::from_secs(self.timeout_secs as u64), join_handle)
                .await
                .context(timeout_msg)?;

        let result = result?;

        let api_socket = result?;

        self.api_socket = Some(api_socket);

        Ok(())
    }

    async fn cloud_hypervisor_check_running(&mut self) -> Result<()> {
        let timeout_secs = self.timeout_secs;

        let timeout_msg = format!(
            "API socket connect timed out after {} seconds",
            timeout_secs
        );

        let join_handle = self.cloud_hypervisor_ping_until_ready(CH_POLL_TIME_MS);

        let result = tokio::time::timeout(Duration::new(timeout_secs as u64, 0), join_handle)
            .await
            .context(timeout_msg)?;

        result
    }

    async fn cloud_hypervisor_ensure_not_launched(&self) -> Result<()> {
        if let Some(child) = &self.process {
            return Err(anyhow!(
                "{} already running with PID {}",
                CH_NAME,
                child.id().unwrap_or(0)
            ));
        }

        Ok(())
    }

    async fn cloud_hypervisor_launch(&mut self, _timeout_secs: i32) -> Result<()> {
        self.cloud_hypervisor_ensure_not_launched().await?;

        let debug = false;

        let disable_seccomp = true;

        let api_socket_path = get_api_socket_path(&self.id)?;

        let _ = std::fs::remove_file(api_socket_path.clone());

        let binary_path = self
            .config
            .as_ref()
            .ok_or("no hypervisor config for CH")
            .map_err(|e| anyhow!(e))?
            .path
            .to_string();

        let path = Path::new(&binary_path).canonicalize()?;

        let mut cmd = Command::new(path);

        cmd.current_dir("/");

        cmd.stdin(Stdio::null());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        cmd.env("RUST_BACKTRACE", "full");

        cmd.args(["--api-socket", &api_socket_path]);

        if let Some(extra_args) = &self.extra_args {
            cmd.args(extra_args);
        }

        if debug {
            cmd.arg("-v");
        }

        if disable_seccomp {
            cmd.args(["--seccomp", "false"]);
        }

        let child = cmd.spawn().context(format!("{} spawn failed", CH_NAME))?;

        // Save process PID
        self.pid = child.id();

        let shutdown = self
            .shutdown_rx
            .as_ref()
            .ok_or("no receiver channel")
            .map_err(|e| anyhow!(e))?
            .clone();

        let ch_outputlogger_task = tokio::spawn(cloud_hypervisor_log_output(child, shutdown));

        let tasks = vec![ch_outputlogger_task];

        self.tasks = Some(tasks);

        Ok(())
    }

    async fn cloud_hypervisor_shutdown(&mut self) -> Result<()> {
        let socket = self
            .api_socket
            .as_ref()
            .ok_or("missing socket")
            .map_err(|e| anyhow!(e))?;

        let response =
            cloud_hypervisor_vmm_shutdown(socket.try_clone().context("shutdown failed")?).await?;

        if let Some(detail) = response {
            debug!(sl!(), "shutdown response: {:?}", detail);
        }

        // Trigger a controlled shutdown
        self.shutdown_tx
            .as_mut()
            .ok_or("no shutdown channel")
            .map_err(|e| anyhow!(e))?
            .send(true)
            .map_err(|e| anyhow!(e).context("failed to request shutdown"))?;

        let tasks = self
            .tasks
            .take()
            .ok_or("no tasks")
            .map_err(|e| anyhow!(e))?;

        let results = join_all(tasks).await;

        let mut wait_errors: Vec<tokio::task::JoinError> = vec![];

        for result in results {
            if let Err(e) = result {
                eprintln!("wait task error: {:#?}", e);

                wait_errors.push(e);
            }
        }

        if wait_errors.is_empty() {
            Ok(())
        } else {
            Err(anyhow!("wait all tasks failed: {:#?}", wait_errors))
        }
    }

    #[allow(dead_code)]
    async fn cloud_hypervisor_wait(&mut self) -> Result<()> {
        let mut child = self
            .process
            .take()
            .ok_or(format!("{} not running", CH_NAME))
            .map_err(|e| anyhow!(e))?;

        let _pid = child
            .id()
            .ok_or(format!("{} missing PID", CH_NAME))
            .map_err(|e| anyhow!(e))?;

        // Note that this kills _and_ waits for the process!
        child.kill().await?;

        Ok(())
    }

    async fn cloud_hypervisor_ping_until_ready(&mut self, _poll_time_ms: u64) -> Result<()> {
        let socket = self
            .api_socket
            .as_ref()
            .ok_or("missing socket")
            .map_err(|e| anyhow!(e))?;

        loop {
            let response =
                cloud_hypervisor_vmm_ping(socket.try_clone().context("failed to clone socket")?)
                    .await
                    .context("ping failed");

            if let Ok(response) = response {
                if let Some(detail) = response {
                    debug!(sl!(), "ping response: {:?}", detail);
                }
                break;
            }

            tokio::time::sleep(Duration::from_millis(CH_POLL_TIME_MS)).await;
        }

        Ok(())
    }

    pub(crate) async fn prepare_vm(&mut self, id: &str, netns: Option<String>) -> Result<()> {
        self.id = id.to_string();
        self.state = VmmState::NotReady;

        self.setup_environment().await?;

        self.netns = netns;

        let vsock_cfg = VsockConfig::new(self.id.clone()).await?;

        let dev = Device::Vsock(vsock_cfg);
        self.add_device(dev).await.context("add vsock device")?;

        self.start_hypervisor(self.timeout_secs).await?;

        Ok(())
    }

    async fn setup_environment(&mut self) -> Result<()> {
        // run_dir and vm_path are the same (shared)
        self.run_dir = get_sandbox_path(&self.id)?;
        self.vm_path = self.run_dir.to_string();

        create_dir_all(&self.run_dir)
            .with_context(|| anyhow!("failed to create sandbox directory {}", self.run_dir))?;

        if !self.jailer_root.is_empty() {
            create_dir_all(self.jailer_root.as_str())
                .map_err(|e| anyhow!("Failed to create dir {} err : {:?}", self.jailer_root, e))?;
        }

        Ok(())
    }

    pub(crate) async fn start_vm(&mut self, timeout_secs: i32) -> Result<()> {
        self.setup_environment().await?;

        self.timeout_secs = timeout_secs;

        self.boot_vm().await?;

        Ok(())
    }

    pub(crate) fn stop_vm(&mut self) -> Result<()> {
        block_on(self.cloud_hypervisor_shutdown())?;

        Ok(())
    }

    pub(crate) fn pause_vm(&self) -> Result<()> {
        Ok(())
    }

    pub(crate) fn resume_vm(&self) -> Result<()> {
        Ok(())
    }

    pub(crate) async fn save_vm(&self) -> Result<()> {
        Ok(())
    }

    pub(crate) async fn get_agent_socket(&self) -> Result<String> {
        const HYBRID_VSOCK_SCHEME: &str = "hvsock";

        let vsock_path = get_vsock_path(&self.id)?;

        let uri = format!("{}://{}", HYBRID_VSOCK_SCHEME, vsock_path);

        Ok(uri)
    }

    pub(crate) async fn disconnect(&mut self) {
        self.state = VmmState::NotReady;
    }

    pub(crate) async fn get_thread_ids(&self) -> Result<VcpuThreadIds> {
        Ok(VcpuThreadIds::default())
    }

    pub(crate) async fn cleanup(&self) -> Result<()> {
        Ok(())
    }

    pub(crate) async fn get_pids(&self) -> Result<Vec<u32>> {
        Ok(Vec::<u32>::new())
    }

    pub(crate) async fn get_vmm_master_tid(&self) -> Result<u32> {
        if let Some(pid) = self.pid {
            Ok(pid)
        } else {
            Err(anyhow!("could not get vmm master tid"))
        }
    }

    pub(crate) async fn get_ns_path(&self) -> Result<String> {
        if let Some(pid) = self.pid {
            let ns_path = format!("/proc/{}/ns", pid);
            Ok(ns_path)
        } else {
            Err(anyhow!("could not get ns path"))
        }
    }

    pub(crate) async fn check(&self) -> Result<()> {
        Ok(())
    }

    pub(crate) async fn get_jailer_root(&self) -> Result<String> {
        let root_path = get_jailer_root(&self.id)?;

        std::fs::create_dir_all(&root_path)?;

        Ok(root_path)
    }

    pub(crate) async fn capabilities(&self) -> Result<Capabilities> {
        let mut caps = Capabilities::default();
        caps.set(CapabilityBits::FsSharingSupport);
        Ok(caps)
    }
}

// Log all output from the CH process until a shutdown signal is received.
// When that happens, stop logging and wait for the child process to finish
// before returning.
async fn cloud_hypervisor_log_output(mut child: Child, mut shutdown: Receiver<bool>) -> Result<()> {
    let stdout = child
        .stdout
        .as_mut()
        .ok_or("failed to get child stdout")
        .map_err(|e| anyhow!(e))?;

    let stdout_reader = BufReader::new(stdout);
    let mut stdout_lines = stdout_reader.lines();

    let stderr = child
        .stderr
        .as_mut()
        .ok_or("failed to get child stderr")
        .map_err(|e| anyhow!(e))?;

    let stderr_reader = BufReader::new(stderr);
    let mut stderr_lines = stderr_reader.lines();

    loop {
        tokio::select! {
            _ = shutdown.changed() => {
                info!(sl!(), "got shutdown request");
                break;
            },
            stderr_line = poll_fn(|cx| Pin::new(&mut stderr_lines).poll_next_line(cx)) => {
                if let Ok(line) = stderr_line {
                    let line = line.ok_or("missing stderr line").map_err(|e| anyhow!(e))?;

                    info!(sl!(), "{:?}", line; "stream" => "stderr");
                }
            },
            stdout_line = poll_fn(|cx| Pin::new(&mut stdout_lines).poll_next_line(cx)) => {
                if let Ok(line) = stdout_line {
                    let line = line.ok_or("missing stdout line").map_err(|e| anyhow!(e))?;

                    info!(sl!(), "{:?}", line; "stream" => "stdout");
                }
            },
        };
    }

    // Note that this kills _and_ waits for the process!
    child.kill().await?;

    Ok(())
}
