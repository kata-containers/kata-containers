// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

use super::inner::CloudHypervisorInner;
use crate::ch::utils::get_api_socket_path;
use crate::ch::utils::get_vsock_path;
use crate::kernel_param::KernelParams;
use crate::utils::{get_jailer_root, get_sandbox_path};
use crate::MemoryConfig;
use crate::VM_ROOTFS_DRIVER_BLK;
use crate::VM_ROOTFS_DRIVER_PMEM;
use crate::{VcpuThreadIds, VmmState};
use anyhow::{anyhow, Context, Result};
use ch_config::ch_api::{
    cloud_hypervisor_vm_create, cloud_hypervisor_vm_start, cloud_hypervisor_vmm_ping,
    cloud_hypervisor_vmm_shutdown,
};
use ch_config::{guest_protection_is_tdx, NamedHypervisorConfig, VmConfig};
use core::future::poll_fn;
use futures::future::join_all;
use kata_sys_util::protection::{available_guest_protection, GuestProtection};
use kata_types::capabilities::{Capabilities, CapabilityBits};
use kata_types::config::default::DEFAULT_CH_ROOTFS_TYPE;
use lazy_static::lazy_static;
use nix::sched::{setns, CloneFlags};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::fs;
use std::fs::create_dir_all;
use std::os::unix::io::AsRawFd;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::process::Stdio;
use std::sync::{Arc, RwLock};
use tokio::io::BufReader;
use tokio::process::{Child, Command};
use tokio::sync::watch::Receiver;
use tokio::task;
use tokio::task::JoinHandle;
use tokio::time::Duration;
use tokio::{io::AsyncBufReadExt, sync::mpsc};

const CH_NAME: &str = "cloud-hypervisor";

/// Number of milliseconds to wait before retrying a CH operation.
const CH_POLL_TIME_MS: u64 = 50;

// The name of the CH JSON key for the build-time features list.
const CH_FEATURES_KEY: &str = "features";

// The name of the CH build-time feature for Intel TDX.
const CH_FEATURE_TDX: &str = "tdx";

#[derive(Debug, PartialEq)]
enum CloudHypervisorLogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct VmmPingResponse {
    pub build_version: String,
    pub version: String,
    pub pid: i64,
    pub features: Vec<String>,
}

#[derive(thiserror::Error, Debug, PartialEq)]
pub enum GuestProtectionError {
    #[error("guest protection requested but no guest protection available")]
    NoProtectionAvailable,

    // LIMITATION: Current CH TDX limitation.
    //
    // When built to support TDX, if Cloud Hypervisor determines the host
    // system supports TDX, it can only create TD's (as opposed to VMs).
    // Hence, on a TDX capable system, confidential_guest *MUST* be set to
    // "true".
    #[error("TDX guest protection available and must be used with Cloud Hypervisor (set 'confidential_guest=true')")]
    TDXProtectionMustBeUsedWithCH,

    // TDX is the only tested CH protection currently.
    #[error("Expected TDX protection, found {0}")]
    ExpectedTDXProtection(GuestProtection),
}

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

        if guest_protection_is_tdx(self.guest_protection_to_use.clone()) {
            if let Some(features) = &self.ch_features {
                if !features.contains(&CH_FEATURE_TDX.to_string()) {
                    return Err(anyhow!("Cloud Hypervisor is not built with TDX support"));
                }
            }
        }

        Ok(())
    }

    async fn get_kernel_params(&self) -> Result<String> {
        let cfg = &self.config;

        let enable_debug = cfg.debug_info.enable_debug;

        let confidential_guest = cfg.security_info.confidential_guest;

        // Note that the configuration option hypervisor.block_device_driver is not used.
        let rootfs_driver = if confidential_guest {
            // PMEM is not available with TDX.
            VM_ROOTFS_DRIVER_BLK
        } else {
            VM_ROOTFS_DRIVER_PMEM
        };

        let rootfs_type = match cfg.boot_info.rootfs_type.is_empty() {
            true => DEFAULT_CH_ROOTFS_TYPE,
            false => &cfg.boot_info.rootfs_type,
        };

        // Start by adding the default set of kernel parameters.
        let mut params = KernelParams::new(enable_debug);

        #[cfg(target_arch = "x86_64")]
        let console_param_debug = KernelParams::from_string("console=ttyS0,115200n8");

        #[cfg(target_arch = "aarch64")]
        let console_param_debug = KernelParams::from_string("console=ttyAMA0,115200n8");

        let mut rootfs_param = KernelParams::new_rootfs_kernel_params(rootfs_driver, rootfs_type)?;

        let mut console_params = if enable_debug {
            if confidential_guest {
                KernelParams::from_string("console=hvc0")
            } else {
                console_param_debug
            }
        } else {
            KernelParams::from_string("quiet")
        };

        params.append(&mut console_params);

        // Add the rootfs device
        params.append(&mut rootfs_param);

        // Now add some additional options required for CH
        let extra_options = [
            "no_timer_check",             // Do not Check broken timer IRQ resources
            "noreplace-smp",              // Do not replace SMP instructions
            "systemd.log_target=console", // Send logging output to the console
        ];

        let mut extra_params = KernelParams::from_string(&extra_options.join(" "));
        params.append(&mut extra_params);

        // Finally, add the user-specified options at the end
        // (so they will take priority).
        params.append(&mut KernelParams::from_string(&cfg.boot_info.kernel_params));

        let kernel_params = params.to_string()?;

        Ok(kernel_params)
    }

    async fn boot_vm(&mut self) -> Result<()> {
        let (shared_fs_devices, network_devices) = self.get_shared_devices().await?;

        let socket = self
            .api_socket
            .as_ref()
            .ok_or("missing socket")
            .map_err(|e| anyhow!(e))?;

        let sandbox_path = get_sandbox_path(&self.id);

        std::fs::create_dir_all(sandbox_path.clone()).context("failed to create sandbox path")?;

        let vsock_socket_path = get_vsock_path(&self.id)?;

        debug!(
            sl!(),
            "generic Hypervisor configuration: {:?}",
            self.config.clone()
        );

        let kernel_params = self.get_kernel_params().await?;

        let named_cfg = NamedHypervisorConfig {
            kernel_params,
            sandbox_path,
            vsock_socket_path,
            cfg: self.config.clone(),
            guest_protection_to_use: self.guest_protection_to_use.clone(),
            shared_fs_devices,
            network_devices,
        };

        let cfg = VmConfig::try_from(named_cfg)?;

        let serialised = serde_json::to_string(&cfg)?;

        debug!(
            sl!(),
            "CH specific VmConfig configuration (JSON): {:?}", serialised
        );

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

        tokio::time::timeout(Duration::new(timeout_secs as u64, 0), join_handle)
            .await
            .context(timeout_msg)?
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

        let cfg = &self.config;

        let debug = cfg.debug_info.enable_debug;

        let disable_seccomp = cfg.security_info.disable_seccomp;

        let api_socket_path = get_api_socket_path(&self.id)?;

        let _ = std::fs::remove_file(api_socket_path.clone());

        let binary_path = cfg.path.to_string();

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
            // Note that with TDX enabled, this results in a lot of additional
            // CH output, particularly if the user adds "earlyprintk" to the
            // guest kernel command line (by modifying "kernel_params=").
            cmd.arg("-v");
        }

        if disable_seccomp {
            cmd.args(["--seccomp", "false"]);
        }

        let netns = self.netns.clone();
        if self.netns.is_some() {
            info!(
                sl!(),
                "set netns for vmm : {:?}",
                self.netns.as_ref().unwrap()
            );
        }

        unsafe {
            let _pre = cmd.pre_exec(move || {
                if let Some(netns_path) = &netns {
                    let netns_fd = std::fs::File::open(netns_path);
                    let _ = setns(netns_fd?.as_raw_fd(), CloneFlags::CLONE_NEWNET)
                        .context("set netns failed");
                }
                Ok(())
            });
        }

        debug!(sl!(), "launching {} as: {:?}", CH_NAME, cmd);

        let child = cmd.spawn().context(format!("{} spawn failed", CH_NAME))?;

        // Save process PID
        self.pid = child.id();

        let shutdown = self
            .shutdown_rx
            .as_ref()
            .ok_or("no receiver channel")
            .map_err(|e| anyhow!(e))?
            .clone();

        let exit_notify: mpsc::Sender<i32> = self
            .exit_notify
            .take()
            .ok_or_else(|| anyhow!("no exit notify"))?;

        let ch_outputlogger_task =
            tokio::spawn(cloud_hypervisor_log_output(child, shutdown, exit_notify));

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

    // Check the specified ping API response to see if it contains CH's
    // build-time features list. If so, save them.
    async fn handle_ch_build_features(&mut self, ping_response: &str) -> Result<()> {
        let v: Value = serde_json::from_str(ping_response)?;

        let got = &v[CH_FEATURES_KEY];

        if got.is_null() {
            return Ok(());
        }

        let features_list = got
            .as_array()
            .ok_or("expected CH to return array of features")
            .map_err(|e| anyhow!(e))?;

        let features: Vec<String> = features_list
            .iter()
            .map(Value::to_string)
            .map(|s| s.trim_start_matches('"').trim_end_matches('"').to_string())
            .collect();

        self.ch_features = Some(features);

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
                    // Check for a list of built-in features, returned by this
                    // API call in newer versions of CH.
                    debug!(sl!(), "ping response: {:?}", detail);

                    self.handle_ch_build_features(&detail).await?;
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

        self.handle_guest_protection().await?;

        self.netns = netns;

        Ok(())
    }

    // Check if guest protection is available and also check if the user
    // actually wants to use it.
    //
    // Note: This method must be called as early as possible since after this
    // call, if confidential_guest is set, a confidential
    // guest will be created.
    async fn handle_guest_protection(&mut self) -> Result<()> {
        let cfg = &self.config;

        let confidential_guest = cfg.security_info.confidential_guest;

        if confidential_guest {
            info!(sl!(), "confidential guest requested");
        }

        let protection =
            task::spawn_blocking(|| -> Result<GuestProtection> { get_guest_protection() })
                .await??;

        self.guest_protection_to_use = protection.clone();

        info!(sl!(), "guest protection {:?}", protection.to_string());

        if confidential_guest {
            if protection == GuestProtection::NoProtection {
                // User wants protection, but none available.
                return Err(anyhow!(GuestProtectionError::NoProtectionAvailable));
            } else if let GuestProtection::Tdx(_) = protection {
                info!(sl!(), "guest protection available and requested"; "guest-protection" => protection.to_string());
            } else {
                return Err(anyhow!(GuestProtectionError::ExpectedTDXProtection(
                    protection
                )));
            }
        } else if protection == GuestProtection::NoProtection {
            debug!(sl!(), "no guest protection available");
        } else if let GuestProtection::Tdx(_) = protection {
            // CH requires TDX protection to be used.
            return Err(anyhow!(GuestProtectionError::TDXProtectionMustBeUsedWithCH));
        } else {
            info!(sl!(), "guest protection available but not requested"; "guest-protection" => protection.to_string());
        }

        Ok(())
    }

    async fn setup_environment(&mut self) -> Result<()> {
        // run_dir and vm_path are the same (shared)
        self.run_dir = get_sandbox_path(&self.id);
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
        self.timeout_secs = timeout_secs;
        self.start_hypervisor(self.timeout_secs).await?;

        self.state = VmmState::VmmServerReady;

        self.boot_vm().await?;

        self.state = VmmState::VmRunning;

        Ok(())
    }

    pub(crate) async fn stop_vm(&mut self) -> Result<()> {
        // If the container workload exits, this method gets called. However,
        // the container manager always makes a ShutdownContainer request,
        // which results in this method being called potentially a second
        // time. Without this check, we'll return an error representing EPIPE
        // since the CH API socket is at that point invalid.
        if self.state != VmmState::VmRunning {
            return Ok(());
        }

        self.state = VmmState::NotReady;

        self.cloud_hypervisor_shutdown().await?;

        Ok(())
    }

    #[allow(dead_code)]
    pub(crate) async fn wait_vm(&self) -> Result<i32> {
        Ok(0)
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
        let thread_id = self.get_vmm_master_tid().await?;
        let proc_path = format!("/proc/{thread_id}");

        let vcpus = get_ch_vcpu_tids(&proc_path)?;
        let vcpu_thread_ids = VcpuThreadIds { vcpus };

        Ok(vcpu_thread_ids)
    }

    pub(crate) async fn cleanup(&self) -> Result<()> {
        Ok(())
    }

    pub(crate) async fn resize_vcpu(&self, old_vcpu: u32, new_vcpu: u32) -> Result<(u32, u32)> {
        Ok((old_vcpu, new_vcpu))
    }

    pub(crate) async fn get_pids(&self) -> Result<Vec<u32>> {
        let pid = self.get_vmm_master_tid().await?;

        Ok(vec![pid])
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
        let root_path = get_jailer_root(&self.id);

        std::fs::create_dir_all(&root_path)?;

        Ok(root_path)
    }

    pub(crate) async fn capabilities(&self) -> Result<Capabilities> {
        let mut caps = Capabilities::default();

        let flags = if guest_protection_is_tdx(self.guest_protection_to_use.clone()) {
            // TDX does not permit the use of virtio-fs.
            CapabilityBits::BlockDeviceSupport
                | CapabilityBits::BlockDeviceHotplugSupport
                | CapabilityBits::HybridVsockSupport
        } else {
            CapabilityBits::BlockDeviceSupport
                | CapabilityBits::BlockDeviceHotplugSupport
                | CapabilityBits::FsSharingSupport
                | CapabilityBits::HybridVsockSupport
        };

        caps.set(flags);

        Ok(caps)
    }

    pub(crate) async fn get_hypervisor_metrics(&self) -> Result<String> {
        Err(anyhow!("CH hypervisor metrics not implemented - see https://github.com/kata-containers/kata-containers/issues/8800"))
    }

    pub(crate) fn set_capabilities(&mut self, flag: CapabilityBits) {
        let mut caps = Capabilities::default();

        caps.set(flag)
    }

    pub(crate) fn set_guest_memory_block_size(&mut self, size: u32) {
        self._guest_memory_block_size_mb = size;
    }

    pub(crate) fn guest_memory_block_size_mb(&self) -> u32 {
        self._guest_memory_block_size_mb
    }

    pub(crate) fn resize_memory(&self, _new_mem_mb: u32) -> Result<(u32, MemoryConfig)> {
        warn!(sl!(), "CH memory resize not implemented - see https://github.com/kata-containers/kata-containers/issues/8801");

        Ok((0, MemoryConfig::default()))
    }
}

// Log all output from the CH process until a shutdown signal is received.
// When that happens, stop logging and wait for the child process to finish
// before returning.
async fn cloud_hypervisor_log_output(
    mut child: Child,
    mut shutdown: Receiver<bool>,
    exit_notify: mpsc::Sender<i32>,
) -> Result<()> {
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

                    match parse_ch_log_level(&line) {
                        CloudHypervisorLogLevel::Trace => trace!(sl!(), "{:?}", line; "stream" => "stderr"),
                        CloudHypervisorLogLevel::Debug => debug!(sl!(), "{:?}", line; "stream" => "stderr"),
                        CloudHypervisorLogLevel::Warn => warn!(sl!(), "{:?}", line; "stream" => "stderr"),
                        CloudHypervisorLogLevel::Error => error!(sl!(), "{:?}", line; "stream" => "stderr"),
                        _ => info!(sl!(), "{:?}", line; "stream" => "stderr"),
                    }
                }
            },
            stdout_line = poll_fn(|cx| Pin::new(&mut stdout_lines).poll_next_line(cx)) => {
                if let Ok(line) = stdout_line {
                    let line = line.ok_or("missing stdout line").map_err(|e| anyhow!(e))?;

                    match parse_ch_log_level(&line) {
                        CloudHypervisorLogLevel::Trace => trace!(sl!(), "{:?}", line; "stream" => "stdout"),
                        CloudHypervisorLogLevel::Debug => debug!(sl!(), "{:?}", line; "stream" => "stdout"),
                        CloudHypervisorLogLevel::Warn => warn!(sl!(), "{:?}", line; "stream" => "stdout"),
                        CloudHypervisorLogLevel::Error => error!(sl!(), "{:?}", line; "stream" => "stdout"),
                        _ => info!(sl!(), "{:?}", line; "stream" => "stdout"),
                    }
                }
            },
        };
    }

    // Note that this kills _and_ waits for the process!
    let _ = child.kill().await;
    if let Ok(status) = child.wait().await {
        let _ = exit_notify.try_send(status.code().unwrap_or(0));
    }

    Ok(())
}

// Search in the log line looking for the log level.
//
// For performance, the line is scanned exactly once and all log levels
// are search for.
fn parse_ch_log_level(line: &str) -> CloudHypervisorLogLevel {
    for (i, c) in line.chars().enumerate() {
        if c == 'I' && line[i..].starts_with("INFO:") {
            return CloudHypervisorLogLevel::Info;
        } else if c == 'D' && line[i..].starts_with("DEBG:") {
            return CloudHypervisorLogLevel::Debug;
        } else if c == 'W' && line[i..].starts_with("WARN:") {
            return CloudHypervisorLogLevel::Warn;
        } else if c == 'E' && line[i..].starts_with("ERRO:") {
            return CloudHypervisorLogLevel::Error;
        } else if c == 'T' && line[i..].starts_with("TRCE:") {
            return CloudHypervisorLogLevel::Trace;
        }
    }

    // Default - logging code cannot fail.
    CloudHypervisorLogLevel::Info
}

lazy_static! {
    // Store the fake guest protection value used by
    // get_fake_guest_protection() and set_fake_guest_protection().
    //
    // Note that if this variable is set to None, get_fake_guest_protection()
    // will fall back to checking the actual guest protection by calling
    // get_guest_protection().
    static ref FAKE_GUEST_PROTECTION: Arc<RwLock<Option<GuestProtection>>> =
        Arc::new(RwLock::new(Some(GuestProtection::NoProtection)));
}

// Return the _fake_ GuestProtection value set by set_guest_protection().
fn get_fake_guest_protection() -> Result<GuestProtection> {
    let existing_ref = FAKE_GUEST_PROTECTION.clone();

    let existing = existing_ref.read().unwrap();

    let real_protection = available_guest_protection()?;

    let protection = if let Some(ref protection) = *existing {
        protection
    } else {
        // XXX: If no fake value is set, fall back to the real function.
        &real_protection
    };

    Ok(protection.clone())
}

// Return available hardware protection, or GuestProtection::NoProtection
// if none available.
//
// XXX: Note that this function wraps the low-level function to determine
// guest protection. It does this to allow us to force a particular guest
// protection type in the unit tests.
fn get_guest_protection() -> Result<GuestProtection> {
    let guest_protection = if cfg!(test) {
        get_fake_guest_protection()
    } else {
        available_guest_protection().map_err(|e| anyhow!(e.to_string()))
    }?;

    Ok(guest_protection)
}

// Return a TID/VCPU map from a specified /proc/{pid} path.
fn get_ch_vcpu_tids(proc_path: &str) -> Result<HashMap<u32, u32>> {
    const VCPU_STR: &str = "vcpu";

    let src = std::fs::canonicalize(proc_path)
        .map_err(|e| anyhow!("Invalid proc path: {proc_path}: {e}"))?;

    let tid_path = src.join("task");

    let mut vcpus = HashMap::new();

    for entry in fs::read_dir(&tid_path)? {
        let entry = entry?;

        let tid_str = match entry.file_name().into_string() {
            Ok(id) => id,
            Err(_) => continue,
        };

        let tid = tid_str
            .parse::<u32>()
            .map_err(|e| anyhow!(e).context("invalid tid."))?;

        let comm_path = tid_path.join(tid_str.clone()).join("comm");

        if !comm_path.exists() {
            return Err(anyhow!("comm path was not found."));
        }

        let p_name = fs::read_to_string(comm_path)?;

        // The CH names it's threads with a vcpu${number} to identify them, where
        // the thread name is located at /proc/${ch_pid}/task/${thread_id}/comm.
        if !p_name.starts_with(VCPU_STR) {
            continue;
        }

        let vcpu_id = p_name
            .trim_start_matches(VCPU_STR)
            .trim()
            .parse::<u32>()
            .map_err(|e| anyhow!(e).context("Invalid vcpu id."))?;

        vcpus.insert(tid, vcpu_id);
    }

    if vcpus.is_empty() {
        return Err(anyhow!("The contents of proc path are not available."));
    }

    Ok(vcpus)
}

#[cfg(test)]
mod tests {
    use super::*;
    use kata_sys_util::protection::TDXDetails;

    #[cfg(target_arch = "x86_64")]
    use kata_sys_util::protection::TDX_SYS_FIRMWARE_DIR;

    use kata_types::config::hypervisor::{Hypervisor as HypervisorConfig, SecurityInfo};
    use serial_test::serial;
    #[cfg(target_arch = "x86_64")]
    use std::path::PathBuf;
    use test_utils::{assert_result, skip_if_not_root};

    use std::fs::File;
    use tempdir::TempDir;

    fn set_fake_guest_protection(protection: Option<GuestProtection>) {
        let existing_ref = FAKE_GUEST_PROTECTION.clone();

        let mut existing = existing_ref.write().unwrap();

        // Modify the lazy static global config structure
        *existing = protection;
    }

    #[serial]
    #[actix_rt::test]
    async fn test_get_guest_protection() {
        // available_guest_protection() requires super user privs.
        skip_if_not_root!();

        let tdx_details = TDXDetails {
            major_version: 1,
            minor_version: 0,
        };

        #[derive(Debug)]
        struct TestData {
            value: Option<GuestProtection>,
            result: Result<GuestProtection>,
        }

        let tests = &[
            TestData {
                value: Some(GuestProtection::NoProtection),
                result: Ok(GuestProtection::NoProtection),
            },
            TestData {
                value: Some(GuestProtection::Pef),
                result: Ok(GuestProtection::Pef),
            },
            TestData {
                value: Some(GuestProtection::Se),
                result: Ok(GuestProtection::Se),
            },
            TestData {
                value: Some(GuestProtection::Sev),
                result: Ok(GuestProtection::Sev),
            },
            TestData {
                value: Some(GuestProtection::Snp),
                result: Ok(GuestProtection::Snp),
            },
            TestData {
                value: Some(GuestProtection::Tdx(tdx_details.clone())),
                result: Ok(GuestProtection::Tdx(tdx_details.clone())),
            },
        ];

        for (i, d) in tests.iter().enumerate() {
            let msg = format!("test[{}]: {:?}", i, d);

            set_fake_guest_protection(d.value.clone());

            let result =
                task::spawn_blocking(|| -> Result<GuestProtection> { get_guest_protection() })
                    .await
                    .unwrap();

            let msg = format!("{}: actual result: {:?}", msg, result);

            if std::env::var("DEBUG").is_ok() {
                eprintln!("DEBUG: {}", msg);
            }

            assert_result!(d.result, result, msg);
        }

        // Reset
        set_fake_guest_protection(None);
    }

    #[cfg(target_arch = "x86_64")]
    #[serial]
    #[actix_rt::test]
    async fn test_get_guest_protection_tdx() {
        // available_guest_protection() requires super user privs.
        skip_if_not_root!();

        let tdx_details = TDXDetails {
            major_version: 1,
            minor_version: 0,
        };

        // Use the hosts protection, not a fake one.
        set_fake_guest_protection(None);

        let tdx_fw_path = PathBuf::from(TDX_SYS_FIRMWARE_DIR);

        // Simple test for Intel TDX
        let have_tdx = if tdx_fw_path.exists() {
            if let Ok(metadata) = std::fs::metadata(tdx_fw_path.clone()) {
                metadata.is_dir()
            } else {
                false
            }
        } else {
            false
        };

        let protection =
            task::spawn_blocking(|| -> Result<GuestProtection> { get_guest_protection() })
                .await
                .unwrap()
                .unwrap();

        if std::env::var("DEBUG").is_ok() {
            let msg = format!(
                "tdx_fw_path: {:?}, have_tdx: {:?}, protection: {:?}",
                tdx_fw_path, have_tdx, protection
            );

            eprintln!("DEBUG: {}", msg);
        }

        if have_tdx {
            assert_eq!(protection, GuestProtection::Tdx(tdx_details));
        } else {
            assert_eq!(protection, GuestProtection::NoProtection);
        }
    }

    #[serial]
    #[actix_rt::test]
    async fn test_handle_guest_protection() {
        // available_guest_protection() requires super user privs.
        skip_if_not_root!();

        #[derive(Debug)]
        struct TestData {
            confidential_guest: bool,
            available_protection: Option<GuestProtection>,

            result: Result<()>,

            // The expected result (internal state)
            guest_protection_to_use: GuestProtection,
        }

        let tdx_details = TDXDetails {
            major_version: 1,
            minor_version: 0,
        };

        let tests = &[
            TestData {
                confidential_guest: false,
                available_protection: Some(GuestProtection::NoProtection),
                result: Ok(()),
                guest_protection_to_use: GuestProtection::NoProtection,
            },
            TestData {
                confidential_guest: true,
                available_protection: Some(GuestProtection::NoProtection),
                result: Err(anyhow!(GuestProtectionError::NoProtectionAvailable)),
                guest_protection_to_use: GuestProtection::NoProtection,
            },
            TestData {
                confidential_guest: false,
                available_protection: Some(GuestProtection::Tdx(tdx_details.clone())),
                result: Err(anyhow!(GuestProtectionError::TDXProtectionMustBeUsedWithCH)),
                guest_protection_to_use: GuestProtection::Tdx(tdx_details.clone()),
            },
            TestData {
                confidential_guest: true,
                available_protection: Some(GuestProtection::Tdx(tdx_details.clone())),
                result: Ok(()),
                guest_protection_to_use: GuestProtection::Tdx(tdx_details),
            },
            TestData {
                confidential_guest: false,
                available_protection: Some(GuestProtection::Pef),
                result: Ok(()),
                guest_protection_to_use: GuestProtection::NoProtection,
            },
            TestData {
                confidential_guest: true,
                available_protection: Some(GuestProtection::Pef),
                result: Err(anyhow!(GuestProtectionError::ExpectedTDXProtection(
                    GuestProtection::Pef
                ))),
                guest_protection_to_use: GuestProtection::Pef,
            },
        ];

        for (i, d) in tests.iter().enumerate() {
            let msg = format!("test[{}]: {:?}", i, d);

            set_fake_guest_protection(d.available_protection.clone());

            let mut ch = CloudHypervisorInner::default();

            let cfg = HypervisorConfig {
                security_info: SecurityInfo {
                    confidential_guest: d.confidential_guest,

                    ..Default::default()
                },

                ..Default::default()
            };

            ch.set_hypervisor_config(cfg);

            let result = ch.handle_guest_protection().await;

            let msg = format!("{}: actual result: {:?}", msg, result);

            if std::env::var("DEBUG").is_ok() {
                eprintln!("DEBUG: {}", msg);
            }

            if d.result.is_ok() && result.is_ok() {
                continue;
            }

            assert_result!(d.result, result, msg);

            assert_eq!(
                ch.guest_protection_to_use, d.guest_protection_to_use,
                "{}",
                msg
            );
        }

        // Reset
        set_fake_guest_protection(None);
    }

    #[actix_rt::test]
    async fn test_get_kernel_params() {
        #[derive(Debug)]
        struct TestData<'a> {
            cfg: Option<HypervisorConfig>,
            confidential_guest: bool,
            debug: bool,
            fails: bool,
            contains: Vec<&'a str>,
        }

        let tests = &[
            TestData {
                cfg: None,
                confidential_guest: false,
                debug: false,
                fails: true, // No hypervisor config
                contains: vec![],
            },
            TestData {
                cfg: Some(HypervisorConfig::default()),
                confidential_guest: false,
                debug: false,
                fails: false,
                contains: vec![],
            },
        ];

        for (i, d) in tests.iter().enumerate() {
            let msg = format!("test[{}]: {:?}", i, d);

            let mut ch = CloudHypervisorInner::default();

            if let Some(ref mut cfg) = d.cfg.clone() {
                if d.debug {
                    cfg.debug_info.enable_debug = true;
                }

                if d.confidential_guest {
                    cfg.security_info.confidential_guest = true;
                }

                ch.set_hypervisor_config(cfg.clone());

                let result = ch.get_kernel_params().await;

                let msg = format!("{}: actual result: {:?}", msg, result);

                if std::env::var("DEBUG").is_ok() {
                    eprintln!("DEBUG: {}", msg);
                }

                if d.fails {
                    assert!(result.is_err(), "{}", msg);
                    continue;
                }

                let result = result.unwrap();

                for token in d.contains.clone() {
                    assert!(result.contains(token), "{}", msg);
                }
            }
        }
    }

    #[actix_rt::test]
    async fn test_parse_ch_log_level() {
        #[derive(Debug)]
        struct TestData<'a> {
            line: &'a str,
            level: CloudHypervisorLogLevel,
        }

        let tests = &[
            // Test default level with various values
            TestData {
                line: "",
                level: CloudHypervisorLogLevel::Info,
            },
            TestData {
                line: "foo",
                level: CloudHypervisorLogLevel::Info,
            },
            TestData {
                line: "info:",
                level: CloudHypervisorLogLevel::Info,
            },
            // Levels are case sensitive
            TestData {
                line: "foo trce: bar",
                level: CloudHypervisorLogLevel::Info,
            },
            TestData {
                line: "foo debg: bar",
                level: CloudHypervisorLogLevel::Info,
            },
            TestData {
                line: "foo info: bar",
                level: CloudHypervisorLogLevel::Info,
            },
            TestData {
                line: "foo warn: bar",
                level: CloudHypervisorLogLevel::Info,
            },
            TestData {
                line: "foo erro: bar",
                level: CloudHypervisorLogLevel::Info,
            },
            TestData {
                line: "foo INFO: bar",
                level: CloudHypervisorLogLevel::Info,
            },
            TestData {
                line: "foo DEBUG: bar",
                level: CloudHypervisorLogLevel::Info,
            },
            TestData {
                line: "foo DEBG: bar",
                level: CloudHypervisorLogLevel::Debug,
            },
            TestData {
                line: "foo WARN:bar",
                level: CloudHypervisorLogLevel::Warn,
            },
            TestData {
                line: "foo ERROR: bar",
                level: CloudHypervisorLogLevel::Info,
            },
            TestData {
                line: "foo ERRO: bar",
                level: CloudHypervisorLogLevel::Error,
            },
            TestData {
                line: "foo TRACE: bar",
                level: CloudHypervisorLogLevel::Info,
            },
            TestData {
                line: "foo TRCE: bar",
                level: CloudHypervisorLogLevel::Trace,
            },
            // First match wins
            TestData {
                line: "TRCE:ERRO:WARN:DEBG:INFO:",
                level: CloudHypervisorLogLevel::Trace,
            },
            TestData {
                line: "ERRO:WARN:DEBG:INFO:TRCE",
                level: CloudHypervisorLogLevel::Error,
            },
            TestData {
                line: "WARN:DEBG:INFO:TRCE:ERRO:",
                level: CloudHypervisorLogLevel::Warn,
            },
            TestData {
                line: "DEBG:INFO:TRCE:ERRO:WARN:",
                level: CloudHypervisorLogLevel::Debug,
            },
            TestData {
                line: "INFO:TRCE:ERRO:WARN:DEBG:",
                level: CloudHypervisorLogLevel::Info,
            },
        ];

        for (i, d) in tests.iter().enumerate() {
            let msg = format!("test[{}]: {:?}", i, d);

            let level = parse_ch_log_level(d.line);

            let msg = format!("{}: actual level: {:?}", msg, level);

            if std::env::var("DEBUG").is_ok() {
                eprintln!("DEBUG: {}", msg);
            }

            assert_eq!(d.level, level, "{}", msg);
        }
    }

    #[actix_rt::test]
    async fn test_get_thread_ids() {
        let path_dir = "/tmp/proc";
        let file_name = "1";

        let tmp_dir = TempDir::new(path_dir).unwrap();
        let file_path = tmp_dir.path().join(file_name);
        let _tmp_file = File::create(file_path.as_os_str()).unwrap();
        let file_path_name = file_path.as_path().to_str().map(|s| s.to_string());
        let file_path_name_str = file_path_name.as_ref().unwrap().to_string();

        #[derive(Debug)]
        struct TestData<'a> {
            proc_path: &'a str,
            result: Result<HashMap<u32, u32>>,
        }

        let tests = &[
            TestData {
                // Test on a non-existent directory.
                proc_path: path_dir,
                result: Err(anyhow!(
                    "Invalid proc path: {path_dir}: No such file or directory (os error 2)"
                )),
            },
            TestData {
                // Test on an existing path, however it is not valid because it does not point to a pid.
                proc_path: &file_path_name_str,
                result: Err(anyhow!("Not a directory (os error 20)")),
            },
            TestData {
                // Test on an existing proc/${pid} but that does not correspond to a CH pid.
                proc_path: "/proc/1",
                result: Err(anyhow!("The contents of proc path are not available.")),
            },
        ];

        for (i, d) in tests.iter().enumerate() {
            let msg = format!("test: [{}]: {:?}", i, d);

            if std::env::var("DEBUG").is_ok() {
                println!("DEBUG: {msg}");
            }

            let result = get_ch_vcpu_tids(d.proc_path);
            let msg = format!("{}, result: {:?}", msg, result);

            let expected_error = format!("{}", d.result.as_ref().unwrap_err());
            let actual_error = format!("{}", result.unwrap_err());

            assert!(actual_error == expected_error, "{}", msg);
        }
    }
}
