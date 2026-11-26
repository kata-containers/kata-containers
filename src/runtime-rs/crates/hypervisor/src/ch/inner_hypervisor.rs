// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

use super::inner::CloudHypervisorInner;
use crate::ch::utils::get_api_socket_path;
use crate::ch::utils::get_vsock_path;
use crate::kernel_param::KernelParams;
use crate::selinux;
use crate::utils::create_dir_all_with_inherit_owner;
use crate::utils::exit_code_from_status;
use crate::utils::set_process_credentials;
use crate::utils::vm_cleanup;
use crate::utils::{bytes_to_megs, get_jailer_root, get_sandbox_path, megs_to_bytes};
use crate::MemoryConfig;
use crate::VM_ROOTFS_DRIVER_BLK;
use crate::{VcpuThreadIds, VmResourceTopology, VmRestoreOptions, VmmState};
use anyhow::{anyhow, Context, Result};
use ch_config::ch_api::cloud_hypervisor_vm_netdev_add_with_fds;
use ch_config::{
    ch_api::{
        cloud_hypervisor_vm_create, cloud_hypervisor_vm_info, cloud_hypervisor_vm_pause,
        cloud_hypervisor_vm_resize, cloud_hypervisor_vm_restore, cloud_hypervisor_vm_resume,
        cloud_hypervisor_vm_snapshot, cloud_hypervisor_vm_start, cloud_hypervisor_vmm_ping,
        cloud_hypervisor_vmm_shutdown, RestoreConfig, VmSnapshotConfig,
    },
    VmResize,
};
use ch_config::{NamedHypervisorConfig, State, VmConfig};
use futures::future::join_all;
use kata_types::capabilities::{Capabilities, CapabilityBits};
use kata_types::config::default::DEFAULT_CH_ROOTFS_TYPE;
use kata_types::config::hypervisor::RootlessUser;
use kata_types::rootless::is_rootless;
use nix::sched::{setns, CloneFlags};
use serde_json::Value;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::fs;
use std::os::fd::AsRawFd;
use std::os::fd::FromRawFd;
use std::os::fd::OwnedFd;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::io::BufReader;
use tokio::process::{Child, Command};
use tokio::sync::watch::Receiver;
use tokio::task;
use tokio::task::JoinHandle;
use tokio::time::Duration;
use tokio::{io::AsyncBufReadExt, sync::mpsc};

fn checkpoint_memory_mb(memory: &ch_config::MemoryConfig) -> Result<u32> {
    const MIB: u64 = 1 << 20;

    let mut bytes = memory
        .size
        .checked_add(memory.hotplugged_size.unwrap_or_default())
        .context("checkpoint memory topology overflow")?;
    for zone in memory.zones.as_deref().unwrap_or_default() {
        bytes = bytes
            .checked_add(zone.size)
            .and_then(|size| size.checked_add(zone.hotplugged_size.unwrap_or_default()))
            .context("checkpoint memory topology overflow")?;
    }
    if bytes == 0 {
        return Err(anyhow!("checkpoint memory topology is empty"));
    }
    if bytes % MIB != 0 {
        return Err(anyhow!(
            "checkpoint memory topology is not MiB-aligned: {bytes} bytes"
        ));
    }
    u32::try_from(bytes / MIB).context("checkpoint memory topology exceeds u32 MiB")
}

const CH_NAME: &str = "clh";

/// Number of milliseconds to wait before retrying a CH operation.
const CH_POLL_TIME_MS: u64 = 50;

const CLH_TEMPLATE_STATE_FILE: &str = "state.json";
const CLH_TEMPLATE_CONFIG_FILE: &str = "config.json";

#[derive(Debug, PartialEq)]
enum CloudHypervisorLogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

#[derive(thiserror::Error, Debug, PartialEq)]
pub enum GuestProtectionError {
    #[error("confidential guests are not supported with Cloud Hypervisor")]
    ConfidentialGuestNotSupported,
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

        Ok(())
    }

    async fn get_kernel_params(&self) -> Result<String> {
        let cfg = &self.config;

        let enable_debug = cfg.debug_info.enable_debug;

        // Note that the configuration option hypervisor.block_device_driver is not used.
        // NVDIMM is not supported for Cloud Hypervisor.
        let rootfs_driver = VM_ROOTFS_DRIVER_BLK;

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

        let mut console_params = if enable_debug {
            console_param_debug
        } else {
            KernelParams::from_string("quiet")
        };

        params.append(&mut console_params);

        if cfg.boot_info.initrd.is_empty() {
            let mut rootfs_params = KernelParams::new_image_rootfs_kernel_params(
                &cfg.boot_info.image,
                &cfg.boot_info.kernel_verity_params,
                rootfs_driver,
                rootfs_type,
                true,
            )?;
            params.append(&mut rootfs_params);
        }

        // Now add some additional options required for CH
        let extra_options = [
            "no_timer_check",             // Do not Check broken timer IRQ resources
            "noreplace-smp",              // Do not replace SMP instructions
            "systemd.log_target=console", // Send logging output to the console
        ];

        let mut extra_params = KernelParams::from_string(&extra_options.join(" "));
        params.append(&mut extra_params);

        // Emit an activation parameter even for extensions without dm-verity.
        // An empty value renders as a bare key, as it does for QEMU.
        for extra in &cfg.guest_extension_images {
            params.append(&mut KernelParams::from_string(&format!(
                "kata.extension.{}.verity_params={}",
                extra.name, extra.verity_params
            )));
        }

        // Finally, add the user-specified options at the end
        // (so they will take priority).
        params.append(&mut KernelParams::from_string(&cfg.boot_info.kernel_params));

        let kernel_params = params.to_string()?;

        Ok(kernel_params)
    }

    async fn boot_vm(&mut self) -> Result<()> {
        let vfio_devices: Vec<_> = self
            .pending_devices
            .iter()
            .filter_map(|dev| {
                if let crate::device::DeviceType::VfioModern(vfio) = dev {
                    Some(vfio.clone())
                } else {
                    None
                }
            })
            .collect();
        let (shared_fs_devices, network_devices, host_devices, protection_device, boot_disks) =
            self.get_shared_devices().await?;
        let network_devices = network_devices.unwrap_or_default();
        let network_fds: Vec<Vec<OwnedFd>> = network_devices
            .iter()
            .map(|net| {
                net.fds
                    .as_ref()
                    .into_iter()
                    .flatten()
                    .map(|fd| unsafe { OwnedFd::from_raw_fd(*fd) })
                    .collect()
            })
            .collect();

        let sandbox_path = get_sandbox_path(&self.id);

        create_dir_all_with_inherit_owner(sandbox_path.clone(), 0o750)
            .context("failed to create sandbox path")?;

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
            shared_fs_devices,
            host_devices,
            boot_disks,
            protection_device,
            ..Default::default()
        };

        let cfg = VmConfig::try_from(named_cfg)?;

        let serialised = serde_json::to_string(&cfg)?;

        debug!(
            sl!(),
            "CH specific VmConfig configuration (JSON): {:?}", serialised
        );

        let response = cloud_hypervisor_vm_create(&self.api_socket, cfg).await?;

        if let Some(detail) = response {
            debug!(sl!(), "vm boot response: {:?}", detail);
        }

        for (net, owned_fds) in network_devices.into_iter().zip(network_fds.iter()) {
            let vm_fds: Vec<_> = owned_fds.iter().map(AsRawFd::as_raw_fd).collect();
            let response = cloud_hypervisor_vm_netdev_add_with_fds(&self.api_socket, net, vm_fds)
                .await
                .context("failed to add vm netdev with fds")?;

            if let Some(detail) = response {
                debug!(sl!(), "vm netdev add response: {:?}", detail);
            }
        }

        let response = cloud_hypervisor_vm_start(&self.api_socket).await?;

        if let Some(detail) = response {
            debug!(sl!(), "vm start response: {:?}", detail);
        }

        if !vfio_devices.is_empty() {
            let info = cloud_hypervisor_vm_info(&self.api_socket).await?;
            let tree = info.device_tree.context("CH vm.info has no device tree")?;
            for device in vfio_devices {
                let mut vfio = device.lock().await;
                let bdf = tree
                    .get(&vfio.device_id)
                    .and_then(|node| node.pci_bdf.as_deref())
                    .with_context(|| format!("CH has no PCI mapping for {}", vfio.device_id))?;
                let path = Self::clh_pci_info_to_path(bdf)?;
                self.device_ids
                    .insert(vfio.device_id.clone(), vfio.device_id.clone());
                vfio.device_options = vec![format!("{}={path}", vfio.device.primary.addr)];
                vfio.config.guest_pci_path = Some(path);
            }
        }

        Ok(())
    }

    fn template_dir(&self) -> Option<PathBuf> {
        let memory_path = Path::new(&self.config.vm_template.memory_path);

        memory_path.parent().map(Path::to_path_buf)
    }

    /// Copy a CLH template artifact while preserving the source file permissions.
    fn copy_template_artifact(src: &Path, dst: &Path) -> Result<()> {
        let metadata = fs::metadata(src).with_context(|| format!("stat {}", src.display()))?;
        fs::copy(src, dst)
            .with_context(|| format!("copy {} to {}", src.display(), dst.display()))?;
        fs::set_permissions(dst, metadata.permissions())
            .with_context(|| format!("set permissions on {}", dst.display()))?;
        Ok(())
    }

    fn write_json_file(path: &Path, value: &Value) -> Result<()> {
        let data = serde_json::to_vec(value)?;
        fs::write(path, data).with_context(|| format!("write {}", path.display()))?;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))
            .with_context(|| format!("set permissions on {}", path.display()))?;
        Ok(())
    }

    fn update_vsock_socket_path(config_path: &Path, sandbox_id: &str) -> Result<()> {
        let data =
            fs::read(config_path).with_context(|| format!("read {}", config_path.display()))?;
        let mut config: Value = serde_json::from_slice(&data)
            .with_context(|| format!("parse {}", config_path.display()))?;

        if let Some(vsock) = config.get_mut("vsock").and_then(Value::as_object_mut) {
            let vsock_socket_path = get_vsock_path(sandbox_id)?;
            vsock.insert("socket".to_string(), Value::String(vsock_socket_path));
        }

        Self::write_json_file(config_path, &config)
    }

    fn patch_snapshot_memory_shared(config_path: &Path, shared: bool) -> Result<()> {
        let data =
            fs::read(config_path).with_context(|| format!("read {}", config_path.display()))?;
        let mut config: Value = serde_json::from_slice(&data)
            .with_context(|| format!("parse {}", config_path.display()))?;

        let memory = config
            .get_mut("memory")
            .and_then(Value::as_object_mut)
            .ok_or_else(|| anyhow!("snapshot config missing memory section"))?;
        memory.insert("shared".to_string(), Value::Bool(shared));

        let zones = memory
            .get_mut("zones")
            .and_then(Value::as_array_mut)
            .ok_or_else(|| anyhow!("snapshot config missing memory zones"))?;
        for zone in zones {
            let zone = zone
                .as_object_mut()
                .ok_or_else(|| anyhow!("snapshot config has invalid memory zone"))?;
            zone.insert("shared".to_string(), Value::Bool(shared));
        }

        Self::write_json_file(config_path, &config)
    }

    fn prepare_restore_files(&self) -> Result<()> {
        let template_dir = self
            .template_dir()
            .ok_or_else(|| anyhow!("template memory path has no parent directory"))?;
        fs::metadata(&self.config.vm_template.memory_path).with_context(|| {
            format!(
                "access template memory {}",
                self.config.vm_template.memory_path
            )
        })?;
        let vm_path = PathBuf::from(&self.vm_path);

        create_dir_all_with_inherit_owner(&vm_path, 0o750)
            .with_context(|| format!("failed to create VM path {}", vm_path.display()))?;

        let src_config = template_dir.join(CLH_TEMPLATE_CONFIG_FILE);
        let src_state = template_dir.join(CLH_TEMPLATE_STATE_FILE);
        let dst_config = vm_path.join(CLH_TEMPLATE_CONFIG_FILE);
        let dst_state = vm_path.join(CLH_TEMPLATE_STATE_FILE);

        Self::copy_template_artifact(&src_config, &dst_config).context("copy template config")?;
        Self::copy_template_artifact(&src_state, &dst_state).context("copy template state")?;
        Self::update_vsock_socket_path(&dst_config, &self.id)
            .context("update restore vsock socket path")?;

        Ok(())
    }

    async fn restore_vm_from(&self, state_dir: &Path) -> Result<()> {
        let state_file = state_dir.join(CLH_TEMPLATE_STATE_FILE);
        let config_file = state_dir.join(CLH_TEMPLATE_CONFIG_FILE);

        fs::metadata(&state_file)
            .with_context(|| format!("access state file {}", state_file.display()))?;
        fs::metadata(&config_file)
            .with_context(|| format!("access config file {}", config_file.display()))?;

        let source_url = format!("file://{}", state_dir.display());
        let response = cloud_hypervisor_vm_restore(
            &self.api_socket,
            RestoreConfig {
                source_url: source_url.clone(),
            },
        )
        .await?;
        if let Some(detail) = response {
            debug!(sl!(), "vm restore response: {:?}", detail);
        }

        let info = cloud_hypervisor_vm_info(&self.api_socket).await?;
        if !matches!(info.state, State::Paused) {
            warn!(sl!(), "restored VM is not paused"; "state" => format!("{:?}", info.state));
        }

        info!(sl!(), "Successfully restored VM from snapshot");

        Ok(())
    }

    async fn prepare_restore_state(
        &mut self,
        state_dir: &Path,
        network_macs: &[[u8; 6]],
    ) -> Result<()> {
        let config_path = state_dir.join(CLH_TEMPLATE_CONFIG_FILE);
        let data =
            fs::read(&config_path).with_context(|| format!("read {}", config_path.display()))?;
        let mut config: VmConfig = serde_json::from_slice(&data)
            .with_context(|| format!("parse {}", config_path.display()))?;

        let mut networks = Vec::new();
        let mut blocks = Vec::new();
        for device in std::mem::take(&mut self.pending_devices) {
            match device {
                crate::DeviceType::Network(network) => networks.push(network),
                crate::DeviceType::BlockModern(block) => {
                    let block = block.lock().await;
                    blocks.push((
                        block.config.index,
                        block.device_id.clone(),
                        PathBuf::from(&block.config.path_on_host),
                    ));
                }
                other => {
                    return Err(anyhow!(
                        "Cloud Hypervisor workload restore does not support pending device {other}"
                    ));
                }
            }
        }
        networks.sort_by_key(|network| network.config.index);
        blocks.sort_by_key(|(index, _, _)| *index);

        let snapshot_networks = config.net.as_deref_mut().unwrap_or_default();
        if snapshot_networks.len() != networks.len() || networks.len() != network_macs.len() {
            return Err(anyhow!(
                "restored network inventory has {} devices, snapshot has {}, checkpoint has {}",
                networks.len(),
                snapshot_networks.len(),
                network_macs.len()
            ));
        }
        for ((snapshot, network), mac) in snapshot_networks
            .iter_mut()
            .zip(networks.iter())
            .zip(network_macs)
        {
            snapshot.tap = Some(network.config.host_dev_name.clone());
            snapshot.fds = None;
            snapshot.mac = ch_config::net_util::MacAddr { bytes: *mac };
        }

        let boot_image = Path::new(&self.config.boot_info.image);
        let snapshot_disks = config.disks.as_deref_mut().unwrap_or_default();
        let mut workload_disks = snapshot_disks
            .iter_mut()
            .filter(|disk| disk.path.as_deref() != Some(boot_image))
            .collect::<Vec<_>>();
        if workload_disks.len() != blocks.len() {
            return Err(anyhow!(
                "restored block inventory has {} devices, snapshot has {} workload disks",
                blocks.len(),
                workload_disks.len()
            ));
        }
        let mut restored_ids = HashMap::with_capacity(blocks.len());
        for (disk, (_, device_id, path)) in workload_disks.iter_mut().zip(blocks) {
            let native_id = disk
                .id
                .clone()
                .filter(|id| !id.is_empty())
                .ok_or_else(|| anyhow!("restored block device {device_id} has no native id"))?;
            if self.device_ids.contains_key(&device_id)
                || restored_ids.insert(device_id.clone(), native_id).is_some()
            {
                return Err(anyhow!(
                    "restored block device id is duplicated: {device_id}"
                ));
            }
            disk.path = Some(path);
        }

        if let Some(vsock) = config.vsock.as_mut() {
            vsock.socket = PathBuf::from(get_vsock_path(&self.id)?);
        }
        if let Some(zones) = config.memory.zones.as_mut() {
            for zone in zones {
                if let Some(source) = zone.file.as_ref() {
                    let name = source.file_name().ok_or_else(|| {
                        anyhow!(
                            "snapshot memory path has no file name: {}",
                            source.display()
                        )
                    })?;
                    let private = state_dir.join(name);
                    fs::metadata(&private).with_context(|| {
                        format!("access private snapshot memory {}", private.display())
                    })?;
                    zone.file = Some(private);
                }
            }
        }
        Self::write_json_file(&config_path, &serde_json::to_value(config)?)?;
        self.device_ids.extend(restored_ids);
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

        *self.api_socket.lock().await = Some(api_socket);

        Ok(())
    }

    async fn cloud_hypervisor_check_running(&mut self) -> Result<()> {
        let timeout_secs = self.timeout_secs;

        let timeout_msg = format!("API socket connect timed out after {timeout_secs} seconds");

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
            cmd.arg("-v");
        }

        if disable_seccomp {
            cmd.args(["--seccomp", "false"]);
        }

        let netns = self.netns.clone();
        if let Some(netns_ref) = &self.netns {
            info!(sl!(), "set netns for vmm : {:?}", netns_ref);
        }

        let user: Option<RootlessUser> = if is_rootless() {
            Some(
                self.config
                    .security_info
                    .rootless_user
                    .clone()
                    .ok_or_else(|| {
                        anyhow!("rootless user must be specified for rootless cloud-hypervisor")
                    })?,
            )
        } else {
            None
        };

        selinux::apply_exec_label(
            &mut cmd,
            self.config.effective_selinux_label(),
            "Cloud Hypervisor",
        )?;

        unsafe {
            let _pre = cmd.pre_exec(move || {
                if let Some(netns_path) = &netns {
                    let netns_fd = std::fs::File::open(netns_path);
                    let _ = setns(&netns_fd?, CloneFlags::CLONE_NEWNET).context("set netns failed");
                }
                if let Some(user) = &user {
                    set_process_credentials(user)
                        .map_err(|err| std::io::Error::other(format!("{err:#}")))?;
                }

                Ok(())
            });
        }

        debug!(sl!(), "launching {} as: {:?}", CH_NAME, cmd);

        let child = cmd.spawn().context(format!("{CH_NAME} spawn failed"))?;

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
        let api_result = cloud_hypervisor_vmm_shutdown(&self.api_socket)
            .await
            .context("Cloud Hypervisor API shutdown failed");
        if let Ok(Some(detail)) = &api_result {
            debug!(sl!(), "shutdown response: {:?}", detail);
        }

        // Always kill and reap the child locally. In particular, an API I/O
        // timeout leaves the HTTP connection unusable, so cleanup must not
        // depend on another successful API exchange.
        let cleanup_result = self.cleanup_process().await;
        match (api_result, cleanup_result) {
            (Ok(_), Ok(())) => Ok(()),
            (Err(api_err), Ok(())) => Err(api_err),
            (Ok(_), Err(cleanup_err)) => Err(cleanup_err),
            (Err(api_err), Err(cleanup_err)) => Err(api_err.context(format!(
                "also failed to clean up Cloud Hypervisor process: {cleanup_err:#}"
            ))),
        }
    }

    async fn cleanup_process(&mut self) -> Result<()> {
        // Trigger the output task to kill and reap the child. This path does
        // not depend on the API socket and is therefore safe after API errors.
        if let Err(err) = self
            .shutdown_tx
            .as_mut()
            .ok_or("no shutdown channel")
            .map_err(|e| anyhow!(e))?
            .send(true)
        {
            debug!(sl!(), "Cloud Hypervisor output task already stopped: {err}");
        }

        let Some(tasks) = self.tasks.take() else {
            self.state = VmmState::NotReady;
            return Ok(());
        };

        let results = join_all(tasks).await;

        let mut wait_errors = vec![];

        for result in results {
            match result {
                Ok(Ok(())) => {}
                Ok(Err(err)) => wait_errors.push(format!("{err:#}")),
                Err(err) => wait_errors.push(format!("{err:#}")),
            }
        }

        self.state = VmmState::NotReady;
        if wait_errors.is_empty() {
            Ok(())
        } else {
            Err(anyhow!("wait all tasks failed: {wait_errors:#?}"))
        }
    }

    #[allow(dead_code)]
    async fn cloud_hypervisor_wait(&mut self) -> Result<()> {
        let mut child = self
            .process
            .take()
            .ok_or(format!("{CH_NAME} not running"))
            .map_err(|e| anyhow!(e))?;

        let _pid = child
            .id()
            .ok_or(format!("{CH_NAME} missing PID"))
            .map_err(|e| anyhow!(e))?;

        // Note that this kills _and_ waits for the process!
        child.kill().await?;

        Ok(())
    }

    async fn cloud_hypervisor_ping_until_ready(&mut self, _poll_time_ms: u64) -> Result<()> {
        loop {
            let response = cloud_hypervisor_vmm_ping(&self.api_socket)
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

    pub(crate) async fn prepare_vm(
        &mut self,
        id: &str,
        netns: Option<String>,
        selinux_label: Option<String>,
    ) -> Result<()> {
        self.id = id.to_string();
        self.state = VmmState::NotReady;

        self.setup_environment().await?;

        self.handle_guest_protection()?;

        self.netns = netns;

        self.config.set_selinux_label(selinux_label);

        Ok(())
    }

    // Cloud Hypervisor does not support any form of guest protection, so
    // refuse to create a confidential guest rather than silently booting a
    // non-confidential one.
    fn handle_guest_protection(&self) -> Result<()> {
        if self.config.security_info.confidential_guest {
            return Err(anyhow!(GuestProtectionError::ConfidentialGuestNotSupported));
        }

        Ok(())
    }

    async fn setup_environment(&mut self) -> Result<()> {
        // run_dir and vm_path are the same (shared)
        self.run_dir = get_sandbox_path(&self.id);
        self.vm_path = self.run_dir.to_string();

        create_dir_all_with_inherit_owner(&self.run_dir, 0o750)
            .with_context(|| anyhow!("failed to create sandbox directory {}", self.run_dir))?;

        if !self.jailer_root.is_empty() {
            create_dir_all_with_inherit_owner(self.jailer_root.as_str(), 0o750)
                .map_err(|e| anyhow!("Failed to create dir {} err : {:?}", self.jailer_root, e))?;
        }

        Ok(())
    }

    pub(crate) async fn start_vm(&mut self, timeout_secs: i32) -> Result<()> {
        self.timeout_secs = timeout_secs;
        self.start_hypervisor(self.timeout_secs).await?;

        self.state = VmmState::VmmServerReady;

        if let Some(restore) = self.restore.take() {
            self.prepare_restore_state(&restore.state_path, &restore.network_macs)
                .await?;
            self.restore_vm_from(&restore.state_path).await?;
            self.resume_vm().await?;
        } else if self.config.vm_template.boot_from_template {
            self.prepare_restore_files()?;
            self.restore_vm_from(Path::new(&self.vm_path)).await?;
            self.resume_vm().await?;
        } else {
            self.boot_vm().await?;
        }

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

    pub(crate) async fn pause_vm(&self) -> Result<()> {
        let response = cloud_hypervisor_vm_pause(&self.api_socket).await?;
        if let Some(detail) = response {
            debug!(sl!(), "vm pause response: {:?}", detail);
        }
        Ok(())
    }

    pub(crate) async fn resume_vm(&self) -> Result<()> {
        let response = cloud_hypervisor_vm_resume(&self.api_socket).await?;
        if let Some(detail) = response {
            debug!(sl!(), "vm resume response: {:?}", detail);
        }
        Ok(())
    }

    pub(crate) async fn save_vm(&self) -> Result<()> {
        let snapshot_dir = self
            .template_dir()
            .ok_or_else(|| anyhow!("template memory path has no parent directory"))?;
        let destination_url = format!("file://{}", snapshot_dir.display());
        let response =
            cloud_hypervisor_vm_snapshot(&self.api_socket, VmSnapshotConfig { destination_url })
                .await?;
        if let Some(detail) = response {
            debug!(sl!(), "vm snapshot response: {:?}", detail);
        }

        if self.config.vm_template.boot_to_be_template {
            Self::patch_snapshot_memory_shared(&snapshot_dir.join(CLH_TEMPLATE_CONFIG_FILE), false)
                .context("patch snapshot memory sharing")?;
        }

        Ok(())
    }

    pub(crate) async fn checkpoint_vm_state_to(
        &self,
        state_dir: &Path,
        _timeout: Option<Duration>,
    ) -> Result<VmResourceTopology> {
        let topology = self.current_resource_topology().await?;
        let destination_url = format!("file://{}", state_dir.display());
        cloud_hypervisor_vm_snapshot(&self.api_socket, VmSnapshotConfig { destination_url })
            .await
            .context("snapshot workload VM")?;
        fs::metadata(state_dir.join(CLH_TEMPLATE_CONFIG_FILE))
            .context("Cloud Hypervisor snapshot did not produce config.json")?;
        fs::metadata(state_dir.join(CLH_TEMPLATE_STATE_FILE))
            .context("Cloud Hypervisor snapshot did not produce state.json")?;
        Ok(topology)
    }

    pub(crate) async fn current_resource_topology(&self) -> Result<VmResourceTopology> {
        let info = cloud_hypervisor_vm_info(&self.api_socket)
            .await
            .context("query checkpoint VM resource topology")?;
        let vcpus = info.config.cpus.boot_vcpus;
        if vcpus == 0 {
            return Err(anyhow!("checkpoint CPU topology is empty"));
        }
        let memory_mb = checkpoint_memory_mb(&info.config.memory)?;
        Ok(VmResourceTopology { vcpus, memory_mb })
    }

    pub(crate) fn configure_restore(&mut self, options: Option<VmRestoreOptions>) -> Result<()> {
        let Some(options) = options else {
            self.restore = None;
            return Ok(());
        };
        if self.process.is_some() {
            return Err(anyhow!(
                "VM restore must be configured before Cloud Hypervisor starts"
            ));
        }
        self.restore = Some(options);
        Ok(())
    }

    pub(crate) async fn get_agent_socket(&self) -> Result<String> {
        const HYBRID_VSOCK_SCHEME: &str = "hvsock";

        let vsock_path = get_vsock_path(&self.id)?;

        let uri = format!("{HYBRID_VSOCK_SCHEME}://{vsock_path}");

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
        info!(sl!(), "CloudHypervisor::cleanup()");
        vm_cleanup(&self.config, &self.id, self.vm_path.as_str())
    }

    pub(crate) async fn resize_vcpu(
        &self,
        old_vcpus: u32,
        mut new_vcpus: u32,
    ) -> Result<(u32, u32)> {
        info!(
            sl!(),
            "cloud hypervisor resize_vcpu(): {} -> {}", old_vcpus, new_vcpus
        );

        if new_vcpus == 0 {
            return Err(anyhow!("resize to 0 vcpus requested"));
        }

        if new_vcpus > self.config.cpu_info.default_maxvcpus {
            warn!(
                sl!(),
                "Cannot allocate more vcpus than the max allowed number of vcpus. The maximum allowed amount of vcpus will be used instead.");
            new_vcpus = self.config.cpu_info.default_maxvcpus;
        }

        if new_vcpus == old_vcpus {
            return Ok((old_vcpus, new_vcpus));
        }

        let vmresize = VmResize {
            desired_vcpus: Some(new_vcpus),
            ..Default::default()
        };

        cloud_hypervisor_vm_resize(&self.api_socket, vmresize)
            .await
            .context("resize vcpus")?;

        Ok((old_vcpus, new_vcpus))
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
            let ns_path = format!("/proc/{pid}/ns");
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

        create_dir_all_with_inherit_owner(&root_path, 0o750)?;

        Ok(root_path)
    }

    pub(crate) async fn capabilities(&self) -> Result<Capabilities> {
        let mut caps = Capabilities::default();

        let flags = CapabilityBits::BlockDeviceSupport
            | CapabilityBits::BlockDeviceHotplugSupport
            | CapabilityBits::BlockDeviceDiscardSupport
            | CapabilityBits::FsSharingSupport
            | CapabilityBits::HybridVsockSupport
            | CapabilityBits::NetworkDeviceHotplugSupport;

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
        self.guest_memory_block_size_mb = bytes_to_megs(size as u64);
    }

    pub(crate) fn guest_memory_block_size_mb(&self) -> u32 {
        self.guest_memory_block_size_mb
    }

    pub(crate) async fn resize_memory(&self, new_mem_mb: u32) -> Result<(u32, MemoryConfig)> {
        let vminfo = cloud_hypervisor_vm_info(&self.api_socket)
            .await
            .context("get vminfo")?;

        let current_mem_size = vminfo.config.memory.size;
        let new_total_mem = megs_to_bytes(new_mem_mb);

        info!(
            sl!(),
            "cloud-hypervisor::resize_memory(): asked to resize memory to {} MB, current memory is {} MB", new_mem_mb, bytes_to_megs(current_mem_size)
        );

        // Early Check to verify if boot memory is the same as requested
        if current_mem_size == new_total_mem {
            info!(sl!(), "VM alreay has requested memory");
            return Ok((new_mem_mb, MemoryConfig::default()));
        }

        if current_mem_size > new_total_mem {
            info!(sl!(), "Remove memory is not supported, nothing to do");
            return Ok((new_mem_mb, MemoryConfig::default()));
        }

        let guest_mem_block_size = megs_to_bytes(self.guest_memory_block_size_mb);

        let mut new_hotplugged_mem = new_total_mem - current_mem_size;

        info!(
            sl!(),
            "new hotplugged mem before alignment: {} B ({} MB), guest_mem_block_size: {} MB",
            new_hotplugged_mem,
            bytes_to_megs(new_hotplugged_mem),
            bytes_to_megs(guest_mem_block_size)
        );

        let is_unaligned = !new_hotplugged_mem.is_multiple_of(guest_mem_block_size);
        if is_unaligned {
            new_hotplugged_mem = ch_config::convert::checked_next_multiple_of(
                new_hotplugged_mem,
                guest_mem_block_size,
            )
            .ok_or(anyhow!(format!(
                "alignment of {} B to the block size of {} B failed",
                new_hotplugged_mem, guest_mem_block_size
            )))?
        }

        let new_total_mem_aligned = new_hotplugged_mem + current_mem_size;

        let max_total_mem = megs_to_bytes(self.config.memory_info.default_maxmemory);
        if new_total_mem_aligned > max_total_mem {
            return Err(anyhow!(
                "requested memory ({} MB) is greater than maximum allowed ({} MB)",
                bytes_to_megs(new_total_mem_aligned),
                self.config.memory_info.default_maxmemory
            ));
        }

        info!(
            sl!(),
            "hotplugged mem from {} MB to {} MB)",
            bytes_to_megs(current_mem_size),
            bytes_to_megs(new_total_mem_aligned)
        );

        let vmresize = VmResize {
            desired_ram: Some(new_total_mem_aligned),
            ..Default::default()
        };

        cloud_hypervisor_vm_resize(&self.api_socket, vmresize)
            .await
            .context("resize memory")?;

        Ok((new_mem_mb, MemoryConfig::default()))
    }
}

// Reap CH and report its exit promptly, then finish forwarding both output
// streams so diagnostics buffered at process exit are preserved.
async fn cloud_hypervisor_log_output(
    mut child: Child,
    mut shutdown: Receiver<bool>,
    exit_notify: mpsc::Sender<i32>,
) -> Result<()> {
    let stdout = child
        .stdout
        .take()
        .ok_or("failed to get child stdout")
        .map_err(|e| anyhow!(e))?;

    let stdout_reader = BufReader::new(stdout);
    let mut stdout_lines = stdout_reader.split(b'\n');

    let stderr = child
        .stderr
        .take()
        .ok_or("failed to get child stderr")
        .map_err(|e| anyhow!(e))?;

    let stderr_reader = BufReader::new(stderr);
    let mut stderr_lines = stderr_reader.split(b'\n');

    let mut stdout_open = true;
    let mut stderr_open = true;
    let mut exited = false;
    while !exited || stdout_open || stderr_open {
        tokio::select! {
            status = child.wait(), if !exited => {
                let status = status.context("wait for Cloud Hypervisor exit")?;
                exited = true;
                let _ = exit_notify.try_send(exit_code_from_status(status));
            },
            _ = shutdown.changed(), if !exited => {
                info!(sl!(), "got shutdown request");
                child.kill().await.context("kill Cloud Hypervisor")?;
                let status = child.wait().await.context("wait for Cloud Hypervisor exit")?;
                exited = true;
                let _ = exit_notify.try_send(exit_code_from_status(status));
            },
            stderr_line = stderr_lines.next_segment(), if stderr_open => {
                if let Ok(line) = stderr_line {
                    let Some(line) = line else {
                        stderr_open = false;
                        continue;
                    };

                    // Guest serial output can contain arbitrary bytes. Decode only for
                    // logging so invalid UTF-8 never stops draining the pipe.
                    let line = String::from_utf8_lossy(line.strip_suffix(b"\r").unwrap_or(&line));
                    match parse_ch_log_level(&line) {
                        CloudHypervisorLogLevel::Trace => trace!(sl!(), "{:?}", line; "stream" => "stderr"),
                        CloudHypervisorLogLevel::Debug => debug!(sl!(), "{:?}", line; "stream" => "stderr"),
                        CloudHypervisorLogLevel::Warn => warn!(sl!(), "{:?}", line; "stream" => "stderr"),
                        CloudHypervisorLogLevel::Error => error!(sl!(), "{:?}", line; "stream" => "stderr"),
                        _ => info!(sl!(), "{:?}", line; "stream" => "stderr"),
                    }
                } else {
                    warn!(sl!(), "failed to read Cloud Hypervisor stderr: {:?}", stderr_line);
                    stderr_open = false;
                }
            },
            stdout_line = stdout_lines.next_segment(), if stdout_open => {
                if let Ok(line) = stdout_line {
                    let Some(line) = line else {
                        stdout_open = false;
                        continue;
                    };

                    // Guest serial output can contain arbitrary bytes. Decode only for
                    // logging so invalid UTF-8 never stops draining the pipe.
                    let line = String::from_utf8_lossy(line.strip_suffix(b"\r").unwrap_or(&line));
                    match parse_ch_log_level(&line) {
                        CloudHypervisorLogLevel::Trace => trace!(sl!(), "{:?}", line; "stream" => "stdout"),
                        CloudHypervisorLogLevel::Debug => debug!(sl!(), "{:?}", line; "stream" => "stdout"),
                        CloudHypervisorLogLevel::Warn => warn!(sl!(), "{:?}", line; "stream" => "stdout"),
                        CloudHypervisorLogLevel::Error => error!(sl!(), "{:?}", line; "stream" => "stdout"),
                        _ => info!(sl!(), "{:?}", line; "stream" => "stdout"),
                    }
                } else {
                    warn!(sl!(), "failed to read Cloud Hypervisor stdout: {:?}", stdout_line);
                    stdout_open = false;
                }
            },
        };
    }

    Ok(())
}

// Search in the log line looking for the log level.
//
// For performance, the line is scanned exactly once and all log levels
// are search for.
fn parse_ch_log_level(line: &str) -> CloudHypervisorLogLevel {
    for (i, c) in line.char_indices() {
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

// Return a VCPU/TID map from a specified /proc/{pid} path. Cloud Hypervisor
// names its vCPU backing threads "vcpu${number}"; the shared scanner in
// crate::utils reads those names from /proc/<pid>/task/<tid>/comm.
fn get_ch_vcpu_tids(proc_path: &str) -> Result<HashMap<u32, u32>> {
    let vcpus = crate::utils::get_vcpu_tids(proc_path, "vcpu")?;

    if vcpus.is_empty() {
        return Err(anyhow!("The contents of proc path are not available."));
    }

    Ok(vcpus)
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{BlockConfigModern, BlockDeviceModern, DeviceType, VIRTIO_BLOCK_PCI};
    use kata_types::config::hypervisor::{Hypervisor as HypervisorConfig, SecurityInfo};

    use std::fs::{self, File};
    use std::sync::Arc;
    use tempfile::Builder;
    use tokio::sync::Mutex;

    #[test]
    fn regular_start_clears_restore_config() {
        let mut inner = CloudHypervisorInner::new(None);
        inner.config.shared_fs.shared_fs = None;
        inner.config.blockdev_info.block_device_driver = VIRTIO_BLOCK_PCI.into();
        inner
            .configure_restore(Some(VmRestoreOptions {
                state_path: PathBuf::from("checkpoint"),
                network_macs: vec![],
                resource_topology: VmResourceTopology {
                    vcpus: 1,
                    memory_mb: 512,
                },
            }))
            .unwrap();
        assert!(inner.restore.is_some());
        // An ordinary start must not require the workload C/R profile.
        inner.config.shared_fs.shared_fs = Some("virtio-fs".into());
        inner.configure_restore(None).unwrap();
        assert!(inner.restore.is_none());
    }

    fn snapshot_config(boot: &Path, disks: Vec<ch_config::DiskConfig>) -> VmConfig {
        VmConfig {
            cpus: ch_config::CpusConfig {
                max_phys_bits: 46,
                ..Default::default()
            },
            disks: Some(
                std::iter::once(ch_config::DiskConfig {
                    path: Some(boot.to_path_buf()),
                    id: Some("boot-native".into()),
                    ..Default::default()
                })
                .chain(disks)
                .collect(),
            ),
            ..Default::default()
        }
    }

    fn push_restore_block(ch: &mut CloudHypervisorInner, id: &str, index: u64, path: &Path) {
        ch.pending_devices
            .push(DeviceType::BlockModern(Arc::new(Mutex::new(
                BlockDeviceModern {
                    device_id: id.into(),
                    config: BlockConfigModern {
                        index,
                        path_on_host: path.to_string_lossy().into_owned(),
                        ..Default::default()
                    },
                    ..Default::default()
                },
            ))));
    }

    #[actix_rt::test]
    async fn test_prepare_restore_maps_block_ids() {
        let state = Builder::new().prefix("clh-restore").tempdir().unwrap();
        let config_path = state.path().join(CLH_TEMPLATE_CONFIG_FILE);
        let boot = state.path().join("boot.img");
        let restored = state.path().join("restored.img");
        let config = snapshot_config(
            &boot,
            vec![ch_config::DiskConfig {
                path: Some(state.path().join("checkpoint.img")),
                id: Some("native-disk".into()),
                ..Default::default()
            }],
        );
        fs::write(&config_path, serde_json::to_vec(&config).unwrap()).unwrap();

        let mut ch = CloudHypervisorInner::default();
        ch.config.boot_info.image = boot.to_string_lossy().into_owned();
        ch.device_ids
            .insert("existing-device".into(), "existing-native".into());
        push_restore_block(&mut ch, "runtime-disk", 0, &restored);

        ch.prepare_restore_state(state.path(), &[]).await.unwrap();

        assert_eq!(
            ch.device_ids.get("runtime-disk").map(String::as_str),
            Some("native-disk")
        );
        assert_eq!(
            ch.device_ids.get("existing-device").map(String::as_str),
            Some("existing-native")
        );
        let config: Value = serde_json::from_slice(&fs::read(config_path).unwrap()).unwrap();
        assert_eq!(config["disks"][1]["path"].as_str(), restored.to_str());
        assert_eq!(config["disks"][1]["id"], "native-disk");
    }

    #[actix_rt::test]
    async fn test_prepare_restore_rejects_missing_block_id() {
        for invalid_id in [None, Some(String::new())] {
            let state = Builder::new().prefix("clh-restore").tempdir().unwrap();
            let config_path = state.path().join(CLH_TEMPLATE_CONFIG_FILE);
            let boot = state.path().join("boot.img");
            let config = snapshot_config(
                &boot,
                vec![
                    ch_config::DiskConfig {
                        path: Some(state.path().join("checkpoint-0.img")),
                        id: Some("native-0".into()),
                        ..Default::default()
                    },
                    ch_config::DiskConfig {
                        path: Some(state.path().join("checkpoint-1.img")),
                        id: invalid_id,
                        ..Default::default()
                    },
                ],
            );
            let original = serde_json::to_vec(&config).unwrap();
            fs::write(&config_path, &original).unwrap();

            let mut ch = CloudHypervisorInner::default();
            ch.config.boot_info.image = boot.to_string_lossy().into_owned();
            ch.device_ids
                .insert("existing-device".into(), "existing-native".into());
            push_restore_block(&mut ch, "runtime-0", 0, &state.path().join("new-0.img"));
            push_restore_block(&mut ch, "runtime-1", 1, &state.path().join("new-1.img"));

            assert!(ch.prepare_restore_state(state.path(), &[]).await.is_err());
            assert_eq!(
                ch.device_ids,
                HashMap::from([("existing-device".into(), "existing-native".into())])
            );
            assert_eq!(fs::read(config_path).unwrap(), original);
        }
    }

    #[test]
    fn test_checkpoint_memory_mb() {
        const MIB: u64 = 1 << 20;

        let plain = ch_config::MemoryConfig {
            size: 256 * MIB,
            hotplug_size: Some(1024 * MIB),
            ..Default::default()
        };
        assert_eq!(checkpoint_memory_mb(&plain).unwrap(), 256);

        let hotplugged = ch_config::MemoryConfig {
            size: 256 * MIB,
            hotplugged_size: Some(128 * MIB),
            ..Default::default()
        };
        assert_eq!(checkpoint_memory_mb(&hotplugged).unwrap(), 384);

        let zoned = ch_config::MemoryConfig {
            zones: Some(vec![
                ch_config::MemoryZoneConfig {
                    id: "mem0".to_string(),
                    size: 256 * MIB,
                    hotplug_size: Some(1024 * MIB),
                    ..Default::default()
                },
                ch_config::MemoryZoneConfig {
                    id: "mem1".to_string(),
                    size: 128 * MIB,
                    hotplugged_size: Some(64 * MIB),
                    ..Default::default()
                },
            ]),
            ..Default::default()
        };
        assert_eq!(checkpoint_memory_mb(&zoned).unwrap(), 448);
    }

    #[test]
    fn test_checkpoint_memory_mb_rejects_invalid_sizes() {
        const MIB: u64 = 1 << 20;

        assert!(checkpoint_memory_mb(&ch_config::MemoryConfig::default()).is_err());
        assert!(checkpoint_memory_mb(&ch_config::MemoryConfig {
            size: MIB + 1,
            ..Default::default()
        })
        .is_err());
        assert!(checkpoint_memory_mb(&ch_config::MemoryConfig {
            size: (u64::from(u32::MAX) + 1) * MIB,
            ..Default::default()
        })
        .is_err());
        assert!(checkpoint_memory_mb(&ch_config::MemoryConfig {
            size: u64::MAX,
            hotplugged_size: Some(1),
            ..Default::default()
        })
        .is_err());
        assert!(checkpoint_memory_mb(&ch_config::MemoryConfig {
            zones: Some(vec![ch_config::MemoryZoneConfig {
                id: "mem0".to_string(),
                size: u64::MAX,
                hotplugged_size: Some(1),
                ..Default::default()
            }]),
            ..Default::default()
        })
        .is_err());
    }

    fn output_test_child(script: &str) -> Child {
        Command::new("/bin/sh")
            .arg("-c")
            .arg(script)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .unwrap()
    }

    #[rstest::rstest]
    #[case(0)]
    #[case(23)]
    #[tokio::test]
    async fn spontaneous_exit_is_reaped_and_reported(#[case] code: i32) {
        let child = output_test_child(&format!("exit {code}"));
        let pid = nix::unistd::Pid::from_raw(child.id().unwrap() as i32);
        let (_shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
        let (exit_tx, mut exit_rx) = mpsc::channel(1);

        tokio::time::timeout(
            Duration::from_secs(5),
            cloud_hypervisor_log_output(child, shutdown_rx, exit_tx),
        )
        .await
        .expect("output task must observe spontaneous exit")
        .unwrap();

        assert_eq!(exit_rx.recv().await, Some(code));
        assert_eq!(exit_rx.recv().await, None);
        assert_eq!(
            nix::sys::signal::kill(pid, None),
            Err(nix::errno::Errno::ESRCH)
        );
    }

    #[test]
    fn process_exit_preserves_buffered_diagnostics() {
        #[derive(Clone)]
        struct CapturedOutput(std::sync::Arc<std::sync::Mutex<Vec<String>>>);

        impl slog::Drain for CapturedOutput {
            type Ok = ();
            type Err = slog::Never;

            fn log(&self, record: &slog::Record, _: &slog::OwnedKVList) -> Result<(), Self::Err> {
                self.0.lock().unwrap().push(record.msg().to_string());
                Ok(())
            }
        }

        let output = CapturedOutput(Default::default());
        let logger = slog::Logger::root(output.clone(), slog::o!());
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        slog_scope::scope(&logger, || {
            runtime.block_on(async {
                let mut child = output_test_child(
                    "i=0; while [ $i -lt 100 ]; do \
                     printf 'stdout line %s\\n' \"$i\"; \
                     printf 'stderr line %s\\n' \"$i\" >&2; \
                     i=$((i + 1)); done; \
                     printf 'stdout final'; printf 'stderr final' >&2; exit 23",
                );
                // Make the process-exit branch ready before reading any output.
                tokio::time::timeout(Duration::from_secs(5), child.wait())
                    .await
                    .unwrap()
                    .unwrap();
                let (_shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
                let (exit_tx, mut exit_rx) = mpsc::channel(1);

                tokio::time::timeout(
                    Duration::from_secs(5),
                    cloud_hypervisor_log_output(child, shutdown_rx, exit_tx),
                )
                .await
                .expect("output task must finish draining after process exit")
                .unwrap();

                assert_eq!(exit_rx.recv().await, Some(23));
                assert_eq!(exit_rx.recv().await, None);
            });
        });

        let mut expected = Vec::new();
        for stream in ["stdout", "stderr"] {
            for i in 0..100 {
                expected.push(format!("\"{stream} line {i}\""));
            }
            expected.push(format!("\"{stream} final\""));
        }
        expected.sort();
        let mut actual = output.0.lock().unwrap().clone();
        actual.sort();
        assert_eq!(actual, expected);
    }

    #[tokio::test]
    async fn exit_notification_does_not_wait_for_output_eof() {
        let child = output_test_child("exec sleep 60");
        let pid = child.id().unwrap();
        // Keep the write ends open after the process exits, as an inherited
        // descriptor could, to check that draining cannot hold up notification.
        let stdout_writer = File::options()
            .write(true)
            .open(format!("/proc/{pid}/fd/1"))
            .unwrap();
        let stderr_writer = File::options()
            .write(true)
            .open(format!("/proc/{pid}/fd/2"))
            .unwrap();
        let (_shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
        let (exit_tx, mut exit_rx) = mpsc::channel(1);
        nix::sys::signal::kill(
            nix::unistd::Pid::from_raw(pid as i32),
            nix::sys::signal::Signal::SIGKILL,
        )
        .unwrap();
        let monitor = tokio::spawn(cloud_hypervisor_log_output(child, shutdown_rx, exit_tx));

        assert_eq!(
            tokio::time::timeout(Duration::from_secs(5), exit_rx.recv())
                .await
                .expect("exit notification must not wait for output EOF"),
            Some(137)
        );
        assert!(!monitor.is_finished());
        drop(stdout_writer);
        drop(stderr_writer);
        tokio::time::timeout(Duration::from_secs(5), monitor)
            .await
            .expect("output task must finish once both pipes reach EOF")
            .unwrap()
            .unwrap();
        assert_eq!(exit_rx.recv().await, None);
    }

    #[rstest::rstest]
    #[case::stdout("")]
    #[case::stderr("exec 1>&2;")]
    #[tokio::test]
    async fn invalid_utf8_keeps_draining_until_exit(#[case] redirect: &str) {
        // More than a pipe can buffer: stopping after the invalid byte would
        // block the writer and prevent natural exit without a shutdown request.
        let child = output_test_child(&format!(
            "{redirect} printf '\\377\\n'; exec head -c 4194304 /dev/zero"
        ));
        let pid = nix::unistd::Pid::from_raw(child.id().unwrap() as i32);
        let (_shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
        let (exit_tx, mut exit_rx) = mpsc::channel(1);

        tokio::time::timeout(
            Duration::from_secs(5),
            cloud_hypervisor_log_output(child, shutdown_rx, exit_tx),
        )
        .await
        .expect("invalid UTF-8 must not prevent draining output and observing exit")
        .unwrap();

        assert_eq!(exit_rx.recv().await, Some(0));
        assert_eq!(exit_rx.recv().await, None);
        assert_eq!(
            nix::sys::signal::kill(pid, None),
            Err(nix::errno::Errno::ESRCH)
        );
    }

    #[rstest::rstest]
    #[case::stdout_closed("exec 1>&-")]
    #[case::stderr_closed("exec 2>&-")]
    #[case::both_closed("exec 1>&- 2>&-")]
    #[case::invalid_stdout("printf '\\377\\n'")]
    #[tokio::test]
    async fn output_failure_keeps_shutdown_available(#[case] output: &str) {
        let child = output_test_child(&format!("{output}; exec sleep 60"));
        let pid = nix::unistd::Pid::from_raw(child.id().unwrap() as i32);
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
        let (exit_tx, mut exit_rx) = mpsc::channel(1);
        let monitor = cloud_hypervisor_log_output(child, shutdown_rx, exit_tx);
        tokio::pin!(monitor);

        assert!(
            tokio::time::timeout(Duration::from_millis(100), &mut monitor)
                .await
                .is_err()
        );
        assert_eq!(nix::sys::signal::kill(pid, None), Ok(()));
        assert!(matches!(
            exit_rx.try_recv(),
            Err(mpsc::error::TryRecvError::Empty)
        ));

        shutdown_tx.send(true).unwrap();
        tokio::time::timeout(Duration::from_secs(5), &mut monitor)
            .await
            .expect("shutdown must remain available after output closes or fails")
            .unwrap();

        assert_eq!(exit_rx.recv().await, Some(137));
        assert_eq!(exit_rx.recv().await, None);
        assert_eq!(
            nix::sys::signal::kill(pid, None),
            Err(nix::errno::Errno::ESRCH)
        );
    }

    #[rstest::rstest]
    #[case::sigkill(nix::sys::signal::Signal::SIGKILL, 137)]
    #[case::sigterm(nix::sys::signal::Signal::SIGTERM, 143)]
    #[tokio::test]
    async fn externally_killed_process_is_reaped_and_reported(
        #[case] signal: nix::sys::signal::Signal,
        #[case] expected: i32,
    ) {
        let child = output_test_child("exec sleep 60");
        let pid = nix::unistd::Pid::from_raw(child.id().unwrap() as i32);
        let (_shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
        let (exit_tx, mut exit_rx) = mpsc::channel(1);
        nix::sys::signal::kill(pid, signal).unwrap();

        tokio::time::timeout(
            Duration::from_secs(5),
            cloud_hypervisor_log_output(child, shutdown_rx, exit_tx),
        )
        .await
        .expect("output task must report a killed VMM")
        .unwrap();

        assert_eq!(exit_rx.recv().await, Some(expected));
        assert_eq!(exit_rx.recv().await, None);
        assert_eq!(
            nix::sys::signal::kill(pid, None),
            Err(nix::errno::Errno::ESRCH)
        );
    }

    #[actix_rt::test]
    async fn test_network_device_hotplug_capability() {
        let ch = CloudHypervisorInner::default();

        assert!(ch
            .capabilities()
            .await
            .unwrap()
            .is_network_device_hotplug_supported());
    }

    #[actix_rt::test]
    async fn test_resize_vcpu_limits() {
        for max_vcpus in [256, 512] {
            let mut ch = CloudHypervisorInner::default();
            ch.config.cpu_info.default_maxvcpus = max_vcpus;
            for requested in [max_vcpus, max_vcpus + 1] {
                assert_eq!(
                    ch.resize_vcpu(max_vcpus, requested).await.unwrap(),
                    (max_vcpus, max_vcpus)
                );
            }
        }
    }

    #[test]
    fn test_handle_guest_protection() {
        for confidential_guest in [false, true] {
            let mut ch = CloudHypervisorInner::default();

            ch.set_hypervisor_config(HypervisorConfig {
                security_info: SecurityInfo {
                    confidential_guest,

                    ..Default::default()
                },

                ..Default::default()
            });

            let result = ch.handle_guest_protection();

            if confidential_guest {
                let err = result.unwrap_err();
                assert_eq!(
                    err.downcast_ref::<GuestProtectionError>(),
                    Some(&GuestProtectionError::ConfidentialGuestNotSupported),
                    "confidential_guest=true: {err:?}"
                );
            } else {
                assert!(result.is_ok(), "confidential_guest=false: {:?}", result);
            }
        }
    }

    #[actix_rt::test]
    async fn test_guest_extension_kernel_params() {
        use kata_types::config::hypervisor::GuestExtensionImage;

        let mut ch = CloudHypervisorInner::default();
        ch.config.guest_extension_images = vec![
            GuestExtensionImage {
                name: "coco".into(),
                path: "/coco.img".into(),
                verity_params: "root_hash=abc,salt=def,data_blocks=1234".into(),
            },
            GuestExtensionImage {
                name: "devkit".into(),
                path: "/devkit.img".into(),
                verity_params: String::new(),
            },
        ];

        let params = ch.get_kernel_params().await.unwrap();
        let tokens: Vec<&str> = params.split_whitespace().collect();
        assert!(tokens.contains(
            &"kata.extension.coco.verity_params=root_hash=abc,salt=def,data_blocks=1234"
        ));
        assert!(tokens.contains(&"kata.extension.devkit.verity_params"));

        ch.config.guest_extension_images.clear();
        let params = ch.get_kernel_params().await.unwrap();
        assert!(!params.contains("kata.extension."));
    }

    #[actix_rt::test]
    async fn test_rootfs_partition_identity() {
        let image = crate::kernel_param::tests::rootfs_image();
        let mut ch = CloudHypervisorInner::default();
        ch.config.boot_info.image = image.path().to_str().unwrap().into();
        // CLH always attaches the root image as virtio-blk, regardless of the
        // configured container block driver or a requested pmem rootfs driver.
        ch.config.boot_info.vm_rootfs_driver = crate::VM_ROOTFS_DRIVER_PMEM.into();
        ch.config.blockdev_info.block_device_driver = "virtio-scsi".into();

        for verity in [
            "",
            "root_hash=abc,salt=def,data_blocks=1,data_block_size=4096,hash_block_size=4096",
        ] {
            ch.config.boot_info.kernel_verity_params = verity.into();
            let params = ch.get_kernel_params().await.unwrap();
            assert!(!params.contains("/dev/vda"));
            if verity.is_empty() {
                assert!(params.contains("root=PARTUUID=603f58ab-01"));
                assert!(params.split_whitespace().any(|param| param == "rootwait"));
            } else {
                assert!(params.contains("root=/dev/dm-0"));
                assert!(params.contains("verity 1 PARTUUID=603f58ab-01 PARTUUID=603f58ab-02 "));
                assert!(params.contains("dm-mod.waitfor=PARTUUID=603f58ab-01,PARTUUID=603f58ab-02"));
            }
        }
    }

    #[actix_rt::test]
    async fn test_initrd_skips_rootfs_partition_identity() {
        let mut ch = CloudHypervisorInner::default();
        ch.config.boot_info.initrd = "/initrd.img".into();
        ch.config.boot_info.image = "/image-must-not-be-opened".into();
        let params = ch.get_kernel_params().await.unwrap();
        assert!(!params.contains("root="));
        assert!(!params.contains("PARTUUID"));
        assert!(!params.contains("dm-mod."));
    }

    #[actix_rt::test]
    async fn test_get_kernel_params() {
        #[derive(Debug)]
        struct TestData<'a> {
            cfg: Option<HypervisorConfig>,
            debug: bool,
            fails: bool,
            contains: Vec<&'a str>,
        }

        let tests = &[
            TestData {
                cfg: None,
                debug: false,
                fails: true, // No hypervisor config
                contains: vec![],
            },
            TestData {
                cfg: Some(HypervisorConfig::default()),
                debug: false,
                fails: false,
                contains: vec![],
            },
        ];

        for (i, d) in tests.iter().enumerate() {
            let msg = format!("test[{i}]: {d:?}");

            let mut ch = CloudHypervisorInner::default();

            if let Some(ref mut cfg) = d.cfg.clone() {
                if d.debug {
                    cfg.debug_info.enable_debug = true;
                }

                ch.set_hypervisor_config(cfg.clone());

                let result = ch.get_kernel_params().await;

                let msg = format!("{msg}: actual result: {result:?}");

                if std::env::var("DEBUG").is_ok() {
                    eprintln!("DEBUG: {msg}");
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
            let msg = format!("test[{i}]: {d:?}");

            let level = parse_ch_log_level(d.line);

            let msg = format!("{msg}: actual level: {level:?}");

            if std::env::var("DEBUG").is_ok() {
                eprintln!("DEBUG: {msg}");
            }

            assert_eq!(d.level, level, "{msg}");
        }
    }

    #[actix_rt::test]
    async fn test_get_thread_ids() {
        let path_dir = "/tmp/proc";
        let file_name = "1";

        let tmp_dir = Builder::new().prefix("proc").tempdir().unwrap();
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
            let msg = format!("test: [{i}]: {d:?}");

            if std::env::var("DEBUG").is_ok() {
                println!("DEBUG: {msg}");
            }

            let result = get_ch_vcpu_tids(d.proc_path);
            let msg = format!("{msg}, result: {result:?}");

            let expected_error = format!("{}", d.result.as_ref().unwrap_err());
            let actual_error = format!("{}", result.unwrap_err());

            assert!(actual_error == expected_error, "{}", msg);
        }
    }

    #[actix_rt::test]
    async fn test_get_ch_vcpu_tids_mapping() {
        let tmp_dir = Builder::new().prefix("fake-proc-pid").tempdir().unwrap();
        let task_dir = tmp_dir.path().join("task");
        fs::create_dir_all(&task_dir).unwrap();

        #[derive(Debug)]
        struct ThreadInfo<'a> {
            tid: &'a str,
            comm: &'a str,
        }

        let threads = &[
            // Non-vcpu thread, should be skipped.
            ThreadInfo {
                tid: "1000",
                comm: "main_thread\n",
            },
            ThreadInfo {
                tid: "2001",
                comm: "vcpu0\n",
            },
            ThreadInfo {
                tid: "2002",
                comm: "vcpu1\n",
            },
            ThreadInfo {
                tid: "2003",
                comm: "vcpu2\n",
            },
        ];

        for t in threads {
            let tid_dir = task_dir.join(t.tid);
            fs::create_dir_all(&tid_dir).unwrap();
            fs::write(tid_dir.join("comm"), t.comm).unwrap();
        }

        let proc_path = tmp_dir.path().to_str().unwrap();
        let result = get_ch_vcpu_tids(proc_path);

        let msg = format!("result: {result:?}");

        if std::env::var("DEBUG").is_ok() {
            println!("DEBUG: {msg}");
        }

        let vcpus = result.unwrap();

        // The mapping must be vcpu_id -> tid.
        assert_eq!(vcpus.len(), 3, "non-vcpu threads should be excluded");
        assert_eq!(vcpus[&0], 2001, "vcpu 0 should map to tid 2001");
        assert_eq!(vcpus[&1], 2002, "vcpu 1 should map to tid 2002");
        assert_eq!(vcpus[&2], 2003, "vcpu 2 should map to tid 2003");

        assert!(
            !vcpus.contains_key(&1000),
            "non-vcpu thread should not be in the map"
        );
    }
}
