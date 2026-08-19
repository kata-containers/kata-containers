// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::collections::HashMap;
use std::sync::Arc;

use agent::Agent;
use anyhow::{anyhow, Context, Result};
use common::{
    error::{is_no_such_process_error, Error},
    types::{
        ContainerConfig, ContainerID, ContainerProcess, ProcessStateInfo, ProcessStatus,
        ProcessType,
    },
};
use kata_sys_util::k8s::update_ephemeral_storage_type;
use kata_types::{
    annotations::{BUNDLE_PATH_KEY, CONTAINER_TYPE_KEY, KATA_ANNO_CFG_HYPERVISOR_INIT_DATA},
    config::{hypervisor::HugePageType, TomlConfig},
    container::update_ocispec_annotations,
    k8s::{self, container_type},
};
use oci_spec::runtime as oci;

use oci::{LinuxResources, Process as OCIProcess};
use resource::{
    cdi_devices::container_device::annotate_container_devices, ResourceManager, ResourceUpdateOp,
};
use tokio::sync::RwLock;

use super::{
    process::{Process, ProcessWatcher},
    ContainerInner,
};
use crate::container_manager::{is_termination_signal, logger_with_process};

pub struct Exec {
    pub(crate) process: Process,
    pub(crate) oci_process: OCIProcess,
}

pub struct Container {
    pid: u32,
    pub container_id: ContainerID,
    config: ContainerConfig,
    spec: oci::Spec,
    inner: Arc<RwLock<ContainerInner>>,
    agent: Arc<dyn Agent>,
    resource_manager: Arc<ResourceManager>,
    logger: slog::Logger,
    pub(crate) passfd_listener_addr: Option<(String, u32)>,
}

fn process_uses_passfd_io(inner: &ContainerInner, process: &ContainerProcess) -> Result<bool> {
    match process.process_type {
        ProcessType::Container => Ok(inner.init_process.passfd_io.is_some()),
        ProcessType::Exec => Ok(inner
            .exec_processes
            .get(&process.exec_id)
            .ok_or_else(|| Error::ProcessNotFound(process.clone()))?
            .process
            .passfd_io
            .is_some()),
    }
}

impl Container {
    pub async fn new(
        pid: u32,
        config: ContainerConfig,
        spec: oci::Spec,
        agent: Arc<dyn Agent>,
        resource_manager: Arc<ResourceManager>,
        passfd_listener_addr: Option<(String, u32)>,
    ) -> Result<Self> {
        let container_id = ContainerID::new(&config.container_id).context("new container id")?;
        let logger = sl!().new(o!("container_id" => config.container_id.clone()));
        let process = ContainerProcess::new(&config.container_id, "")?;
        let init_process = Process::new(
            &process,
            pid,
            &config.bundle,
            config.stdin.clone(),
            config.stdout.clone(),
            config.stderr.clone(),
            config.terminal,
        );
        let linux_resources = spec
            .linux()
            .as_ref()
            .and_then(|linux| linux.resources().clone());

        Ok(Self {
            pid,
            container_id,
            config,
            spec,
            inner: Arc::new(RwLock::new(ContainerInner::new(
                agent.clone(),
                init_process,
                logger.clone(),
                linux_resources,
            ))),
            agent,
            resource_manager,
            logger,
            passfd_listener_addr,
        })
    }

    pub async fn create(&self, mut spec: oci::Spec) -> Result<()> {
        // process oci spec
        let mut inner = self.inner.write().await;
        let toml_config = self.resource_manager.config().await;
        let config = &self.config;
        let sandbox_pidns = is_pid_namespace_enabled(&spec);
        let disable_guest_selinux = get_disable_guest_selinux(&toml_config);
        let annotations = spec.annotations().clone().unwrap_or_default();
        // Tag the spec with the actual container type. Previously every
        // non-pod-container was forced to "pod_sandbox", which made standalone
        // engines (Docker/nerdctl/podman) look like a pod sandbox. The agent
        // skips CDI device injection for "pod_sandbox", so the NVIDIA CDI edits
        // carried in the "cdi.k8s.io/*" annotations were never applied and the
        // GPU userspace (e.g. nvidia-smi) was missing in the guest. Emitting
        // "single_container" matches the Go runtime and lets the agent inject.
        let container_typ = container_type(&spec);
        let pod_type_anno = (CONTAINER_TYPE_KEY.to_string(), container_typ.to_string());

        let bund_path_anno = (BUNDLE_PATH_KEY.to_string(), config.bundle.clone());
        let updated_annotations = update_ocispec_annotations(
            &annotations,
            &[KATA_ANNO_CFG_HYPERVISOR_INIT_DATA],
            &[pod_type_anno, bund_path_anno],
        );
        spec.set_annotations(Some(updated_annotations.clone()));

        amend_spec(
            &mut spec,
            toml_config.runtime.disable_guest_seccomp,
            disable_guest_selinux,
        )
        .context("amend spec")?;

        // get mutable root from oci spec
        let root = match spec.root_mut() {
            Some(root) => root,
            None => return Err(anyhow!("spec miss root field")),
        };

        // handler rootfs
        let rootfs = self
            .resource_manager
            .handler_rootfs(
                &config.container_id,
                root,
                &config.bundle,
                &config.rootfs_mounts,
                &updated_annotations,
            )
            .await
            .context("handler rootfs")?;

        // update rootfs
        root.set_path(
            rootfs
                .get_guest_rootfs_path()
                .await
                .context("get guest rootfs path")?
                .into(),
        );

        let mut storages = vec![];
        if let Some(mut storage_list) = rootfs.get_storage().await {
            storages.append(&mut storage_list);
        }
        inner.rootfs.push(rootfs);

        // handler volumes
        let volumes = self
            .resource_manager
            .handler_volumes(&config.container_id, &spec)
            .await
            .context("handler volumes")?;
        let mut oci_mounts = vec![];
        for v in volumes {
            let mut volume_mounts = v.get_volume_mount().context("get volume mount")?;
            if !volume_mounts.is_empty() {
                oci_mounts.append(&mut volume_mounts);
            }

            let mut s = v.get_storage().context("get storage")?;
            if !s.is_empty() {
                storages.append(&mut s);
            }
            inner.volumes.push(v);
        }
        spec.set_mounts(Some(oci_mounts));

        let linux = spec
            .linux()
            .as_ref()
            .context("OCI spec missing linux field")?;

        let container_devices = self
            .resource_manager
            .handler_devices(&config.container_id, linux)
            .await?;
        let devices_agent = annotate_container_devices(&mut spec, container_devices)
            .context("annotate container devices failed")?;

        // update vcpus, mems and host cgroups
        let resources = self
            .resource_manager
            .update_linux_resource(
                &config.container_id,
                inner.linux_resources.as_ref(),
                ResourceUpdateOp::Add,
            )
            .await?;
        if let Some(linux) = &mut spec.linux_mut() {
            linux.set_resources(resources);

            // Only CPU and Memory constraints are supported in the guest.
            // Clear unsupported resource fields to match the Go runtime
            // and satisfy the agent policy checks.
            if let Some(resource) = linux.resources_mut() {
                resource.set_devices(None);
                resource.set_pids(None);
                resource.set_block_io(None);
                resource.set_network(None);

                if is_hugetlb_backed(&toml_config) {
                    let huge_pages = huge_pages_total(Some(resource));
                    add_huge_pages_to_memory_limit(
                        resource,
                        huge_pages,
                        static_guest_memory_mb(&toml_config),
                    );
                }
            }

            // VFIO device filtering depends on vfio_mode configuration:
            //
            // - guest-kernel mode: Devices are managed by the guest kernel driver and
            //   are not presented to the container. Remove them from the OCI spec to
            //   match the Go runtime (kata_agent.go:1093-1105) and satisfy the agent
            //   policy's allow_linux_devices check.
            //   * vfio-pci-gk: PCI device passthrough with guest kernel driver
            //
            // - vfio mode: Devices appear as VFIO character devices
            //   (/dev/vfio/*) inside the container. Keep them in the OCI spec so the
            //   agent can validate and bind them properly. This is required for:
            //   * vfio-pci: PCI device passthrough with VFIO in container
            //   * vfio-ap: Adjunct Processor (AP) device passthrough with VFIO-AP
            let vfio_mode = toml_config.runtime.vfio_mode.as_str();
            filter_vfio_devices(linux, vfio_mode);
        }

        let container_name = k8s::container_name(&spec);
        let mut shared_mounts = Vec::new();
        for shared_mount in &toml_config.runtime.shared_mounts {
            if shared_mount.dst_ctr == container_name {
                let m = agent::types::SharedMount {
                    name: shared_mount.name.clone(),
                    src_ctr: shared_mount.src_ctr.clone(),
                    src_path: shared_mount.src_path.clone(),
                    dst_ctr: shared_mount.dst_ctr.clone(),
                    dst_path: shared_mount.dst_path.clone(),
                };
                shared_mounts.push(m);
            }
        }

        // In passfd io mode, we create vsock connections for io in advance
        // and pass port info to agent in `CreateContainerRequest`.
        // These vsock connections will be used as stdin/stdout/stderr of the container process.
        // See agent/src/passfd_io.rs for more details.
        if let Some((hvsock_uds_path, passfd_port)) = &self.passfd_listener_addr {
            inner
                .init_process
                .passfd_io_init(hvsock_uds_path, *passfd_port)
                .await?;
        }

        info!(
            sl!(),
            "OCI Spec {:?} within CreateContainerRequest.",
            spec.clone()
        );

        // create container
        let r = agent::CreateContainerRequest {
            process_id: agent::ContainerProcessID::new(&config.container_id, ""),
            storages,
            oci: Some(spec),
            sandbox_pidns,
            devices: devices_agent,
            shared_mounts,
            stdin_port: inner
                .init_process
                .passfd_io
                .as_ref()
                .and_then(|io| io.stdin_port),
            stdout_port: inner
                .init_process
                .passfd_io
                .as_ref()
                .and_then(|io| io.stdout_port),
            stderr_port: inner
                .init_process
                .passfd_io
                .as_ref()
                .and_then(|io| io.stderr_port),
            ..Default::default()
        };

        self.agent
            .create_container(r)
            .await
            .context("agent create container")?;
        self.resource_manager.dump().await;
        Ok(())
    }

    pub async fn start(
        &self,
        containers: Arc<RwLock<HashMap<String, Container>>>,
        process: &ContainerProcess,
    ) -> Result<()> {
        let mut inner = self.inner.write().await;
        match process.process_type {
            ProcessType::Container => {
                let res: Result<()> = async {
                    inner.start_container(&process.container_id).await?;

                    if process_uses_passfd_io(&inner, process)? {
                        inner
                            .init_process
                            .passfd_io_wait(containers, self.agent.clone())
                            .await?;
                    } else {
                        let container_io = inner.new_container_io(process).await?;
                        inner
                            .init_process
                            .start_io_and_wait(containers, self.agent.clone(), container_io)
                            .await?;
                    }
                    Ok(())
                }
                .await;

                if let Err(err) = res {
                    let device_manager = self.resource_manager.get_device_manager().await;
                    let _ = inner.stop_process(process, true, &device_manager).await;

                    if let Err(e) = self
                        .resource_manager
                        .update_linux_resource(
                            &self.config.container_id,
                            inner.linux_resources.as_ref(),
                            ResourceUpdateOp::Del,
                        )
                        .await
                    {
                        warn!(
                            self.logger,
                            "failed to release linux resources after start failure: {:?}", e
                        );
                    }
                    return Err(err);
                }
            }
            ProcessType::Exec => {
                // In passfd io mode, we create vsock connections for io in advance
                // and pass port info to agent in `ExecProcessRequest`.
                // These vsock connections will be used as stdin/stdout/stderr of the exec process.
                // See agent/src/passfd_io.rs for more details.
                if let Some((hvsock_uds_path, passfd_port)) = &self.passfd_listener_addr {
                    let exec = inner
                        .exec_processes
                        .get_mut(&process.exec_id)
                        .ok_or_else(|| Error::ProcessNotFound(process.clone()))?;
                    exec.process
                        .passfd_io_init(hvsock_uds_path, *passfd_port)
                        .await?;
                }

                if let Err(e) = inner.start_exec_process(process).await {
                    let device_manager = self.resource_manager.get_device_manager().await;
                    let _ = inner.stop_process(process, true, &device_manager).await;
                    return Err(e).context("enter process");
                }

                {
                    let exec = inner
                        .exec_processes
                        .get(&process.exec_id)
                        .ok_or_else(|| Error::ProcessNotFound(process.clone()))?;
                    if exec.process.height != 0 && exec.process.width != 0 {
                        inner
                            .win_resize_process(process, exec.process.height, exec.process.width)
                            .await
                            .context("win resize")?;
                    }
                }

                if process_uses_passfd_io(&inner, process)? {
                    // In passfd io mode, we don't bother with the IO.
                    // We send `WaitProcessRequest` immediately to the agent
                    // and wait for the response in a separate thread.
                    // The agent will only respond after IO is done.
                    let exec = inner
                        .exec_processes
                        .get_mut(&process.exec_id)
                        .ok_or_else(|| Error::ProcessNotFound(process.clone()))?;
                    exec.process
                        .passfd_io_wait(containers, self.agent.clone())
                        .await?;
                } else {
                    // In legacy io mode, we handle IO by polling the agent.
                    // When IO is done, we send `WaitProcessRequest` to agent
                    // to get the exit status.
                    let container_io =
                        inner.new_container_io(process).await.context("io stream")?;

                    let exec = inner
                        .exec_processes
                        .get_mut(&process.exec_id)
                        .ok_or_else(|| Error::ProcessNotFound(process.clone()))?;
                    exec.process
                        .start_io_and_wait(containers, self.agent.clone(), container_io)
                        .await
                        .context("start io and wait")?;
                }
            }
        }

        Ok(())
    }

    pub async fn delete_exec_process(&self, container_process: &ContainerProcess) -> Result<()> {
        let mut inner = self.inner.write().await;
        inner
            .delete_exec_process(&container_process.exec_id)
            .await
            .context("delete process")
    }

    pub async fn state_process(
        &self,
        container_process: &ContainerProcess,
    ) -> Result<ProcessStateInfo> {
        let inner = self.inner.read().await;
        match container_process.process_type {
            ProcessType::Container => inner.init_process.state().await,
            ProcessType::Exec => {
                let exec = inner
                    .exec_processes
                    .get(&container_process.exec_id)
                    .ok_or_else(|| Error::ProcessNotFound(container_process.clone()))?;
                exec.process.state().await
            }
        }
    }

    pub async fn wait_process(
        &self,
        container_process: &ContainerProcess,
    ) -> Result<ProcessWatcher> {
        let logger = logger_with_process(container_process);
        info!(logger, "start wait process");

        let inner = self.inner.read().await;
        inner
            .fetch_exit_watcher(container_process)
            .context("fetch exit watcher")
    }

    pub async fn kill_process(
        &self,
        container_process: &ContainerProcess,
        signal: u32,
        all: bool,
    ) -> Result<()> {
        let mut inner = self.inner.write().await;

        // Check if process is already stopped before signaling.
        // For SIGKILL/SIGTERM, if the process is already stopped, return success immediately.
        // This is critical for proper cleanup when VM dies - the wait thread sets status to
        // Stopped even on error, so subsequent Kill() calls will see it as already stopped.
        let is_term_signal = is_termination_signal(signal);
        let process_status = if container_process.exec_id.is_empty() {
            inner.init_process.get_status().await
        } else if let Some(exec) = inner.exec_processes.get(&container_process.exec_id) {
            exec.process.get_status().await
        } else {
            ProcessStatus::Unknown
        };

        if is_term_signal && process_status == ProcessStatus::Stopped {
            info!(
                self.logger,
                "process has already stopped, skipping signal";
                "container" => &self.container_id.container_id,
                "process" => ?container_process,
                "signal" => signal
            );
            return Ok(());
        }

        match inner.signal_process(container_process, signal, all).await {
            Ok(()) => Ok(()),
            Err(e) if is_term_signal && is_no_such_process_error(&e) => {
                info!(
                    self.logger,
                    "process already gone during kill, treating as success";
                    "container" => &self.container_id.container_id,
                    "process" => ?container_process,
                    "signal" => signal
                );
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    pub async fn exec_process(
        &self,
        container_process: &ContainerProcess,
        stdin: Option<String>,
        stdout: Option<String>,
        stderr: Option<String>,
        terminal: bool,
        mut oci_process: OCIProcess,
    ) -> Result<()> {
        let toml_config = self.resource_manager.config().await;
        if get_disable_guest_selinux(&toml_config) {
            oci_process.set_selinux_label(None);
        }

        let process = Process::new(
            container_process,
            self.pid,
            &self.config.bundle,
            stdin,
            stdout,
            stderr,
            terminal,
        );
        let exec = Exec {
            process,
            oci_process,
        };
        let mut inner = self.inner.write().await;
        inner.add_exec_process(&container_process.exec_id, exec);
        Ok(())
    }

    pub async fn close_io(&self, container_process: &ContainerProcess) -> Result<()> {
        let mut inner = self.inner.write().await;
        inner.close_io(container_process).await
    }

    pub async fn stop_process(&self, container_process: &ContainerProcess) -> Result<()> {
        if container_process.process_type == ProcessType::Container {
            self.copy_termination_log().await;
        }

        let mut inner = self.inner.write().await;
        let device_manager = self.resource_manager.get_device_manager().await;
        inner
            .stop_process(container_process, true, &device_manager)
            .await
            .context("stop process")?;

        // update vcpus, mems and host cgroups
        if container_process.process_type == ProcessType::Container {
            self.resource_manager
                .update_linux_resource(
                    &self.config.container_id,
                    inner.linux_resources.as_ref(),
                    ResourceUpdateOp::Del,
                )
                .await?;
        }

        Ok(())
    }

    async fn copy_termination_log(&self) {
        let toml_config = self.resource_manager.config().await;
        let shared_fs = toml_config
            .hypervisor
            .get(&toml_config.runtime.hypervisor_name)
            .and_then(|h| h.shared_fs.shared_fs.as_deref());

        // When a shared filesystem is configured the host can read the
        // termination log directly.  shared_fs == None means no shared
        // filesystem (the "none" config value is normalised to None by
        // SharedFsInfo::adjust_config).
        if shared_fs.is_some() {
            return;
        }

        let annotations = self.spec.annotations().clone().unwrap_or_default();
        let policy = annotations.get("io.kubernetes.container.terminationMessagePolicy");
        if policy.map(|p| p.as_str()) != Some("File") {
            return;
        }

        let termination_path =
            match annotations.get("io.kubernetes.container.terminationMessagePath") {
                Some(p) if !p.is_empty() => p.clone(),
                _ => return,
            };

        let req = agent::GetDiagnosticDataRequest {
            log_type: "termination_log".to_string(),
            container_id: self.container_id.container_id.clone(),
        };

        // The kubelet bind-mounts a host file into the container at
        // terminationMessagePath, then reads back from that host file.
        // With shared_fs=none the guest cannot write through that mount,
        // so we locate the host-side source path from the OCI mounts and
        // write the data there directly.
        let host_path = self.spec.mounts().as_ref().and_then(|mounts| {
            mounts
                .iter()
                .find(|m| m.destination() == std::path::Path::new(&termination_path))
                .and_then(|m| m.source().clone())
        });

        let host_path = match host_path {
            Some(p) => p,
            None => {
                warn!(
                    self.logger,
                    "No host mount found for termination message path"
                );
                return;
            }
        };

        match self.agent.get_diagnostic_data(req).await {
            Ok(resp) if !resp.data.is_empty() => {
                if let Err(e) = tokio::fs::write(&host_path, resp.data.as_bytes()).await {
                    warn!(self.logger, "Failed to write termination message: {}", e);
                }
            }
            Ok(_) => {}
            Err(e) => {
                warn!(
                    self.logger,
                    "Failed to get termination message from guest: {}", e
                );
            }
        }
    }

    pub async fn pause(&self) -> Result<()> {
        let mut inner = self.inner.write().await;
        let status = inner.init_process.get_status().await;
        if status != ProcessStatus::Running {
            warn!(
                self.logger,
                "container is in {:?} state, will not pause", status
            );
            return Ok(());
        }

        self.agent
            .pause_container(self.container_id.clone().into())
            .await
            .context("agent pause container")?;
        inner.set_state(ProcessStatus::Paused).await;

        Ok(())
    }

    pub async fn resume(&self) -> Result<()> {
        let mut inner = self.inner.write().await;
        let status = inner.init_process.get_status().await;
        if status != ProcessStatus::Paused {
            warn!(
                self.logger,
                "container is in {:?} state, will not resume", status
            );
            return Ok(());
        }

        self.agent
            .resume_container(self.container_id.clone().into())
            .await
            .context("agent pause container")?;
        inner.set_state(ProcessStatus::Running).await;

        Ok(())
    }

    pub async fn resize_pty(
        &self,
        process: &ContainerProcess,
        width: u32,
        height: u32,
    ) -> Result<()> {
        let logger = logger_with_process(process);
        let mut inner = self.inner.write().await;
        if inner.init_process.get_status().await != ProcessStatus::Running {
            warn!(logger, "container is not running");
            return Ok(());
        }

        if process.exec_id.is_empty() {
            inner.init_process.height = height;
            inner.init_process.width = width;
        } else if let Some(exec) = inner.exec_processes.get_mut(&process.exec_id) {
            exec.process.height = height;
            exec.process.width = width;

            // for some case, resize_pty request should be handled while the process has not been started in agent
            // just return here, and truly resize_pty will happen in start_process
            if exec.process.get_status().await != ProcessStatus::Running {
                return Ok(());
            }
        } else {
            return Err(anyhow!(
                "could not find process {} in container {}",
                process.exec_id(),
                process.container_id()
            ));
        }

        inner.win_resize_process(process, height, width).await
    }

    pub async fn stats(&self) -> Result<Option<agent::StatsContainerResponse>> {
        let stats_resp = self
            .agent
            .stats_container(self.container_id.clone().into())
            .await
            .context("agent stats container")?;
        Ok(Some(stats_resp))
    }

    pub async fn update(&self, resources: &LinuxResources) -> Result<()> {
        let mut inner = self.inner.write().await;

        // Follow the huge pages the container holds, so that a later update
        // carrying none of them is still told the reservation this one asked
        // for rather than the one the container was created with.
        let mut current = resources.clone();
        if huge_pages_total(Some(&current)) == 0 {
            if let Some(held) = inner
                .linux_resources
                .as_ref()
                .and_then(|held| held.hugepage_limits().clone())
            {
                current.set_hugepage_limits(Some(held));
            }
        }
        inner.linux_resources = Some(current);

        // update vcpus, mems and host cgroups
        let mut agent_resources = self
            .resource_manager
            .update_linux_resource(
                &self.config.container_id,
                Some(resources),
                ResourceUpdateOp::Update,
            )
            .await?;

        // An update carries the limits the host is to apply now, and a resize
        // of the memory limit alone does not repeat the huge page reservation
        // the container was created with. Without folding that reservation in
        // again the update would undo the ceiling the container was started
        // with.
        let toml_config = self.resource_manager.config().await;
        if is_hugetlb_backed(&toml_config) {
            if let Some(agent_resources) = agent_resources.as_mut() {
                let mut huge_pages = huge_pages_total(Some(agent_resources));
                if huge_pages == 0 {
                    huge_pages = huge_pages_total(inner.linux_resources.as_ref());
                }
                add_huge_pages_to_memory_limit(
                    agent_resources,
                    huge_pages,
                    static_guest_memory_mb(&toml_config),
                );
            }
        }

        let req = agent::UpdateContainerRequest {
            container_id: self.container_id.container_id.clone(),
            resources: agent_resources,
            mounts: Vec::new(),
        };
        self.agent
            .update_container(req)
            .await
            .context("agent update container")?;
        Ok(())
    }

    pub async fn config(&self) -> ContainerConfig {
        self.config.clone()
    }

    pub async fn spec(&self) -> oci::Spec {
        self.spec.clone()
    }

    pub async fn cleanup(&mut self) -> Result<()> {
        let mut inner = self.inner.write().await;
        let device_manager = self.resource_manager.get_device_manager().await;
        let res = inner
            .cleanup_container(
                self.container_id.container_id.as_str(),
                true,
                &device_manager,
            )
            .await;

        if let Err(e) = self
            .resource_manager
            .update_linux_resource(
                &self.config.container_id,
                inner.linux_resources.as_ref(),
                ResourceUpdateOp::Del,
            )
            .await
        {
            warn!(self.logger, "failed to cleanup linux resources: {:?}", e);
        }

        res
    }
}

fn amend_spec(
    spec: &mut oci::Spec,
    disable_guest_seccomp: bool,
    disable_guest_selinux: bool,
) -> Result<()> {
    // Only the StartContainer hook needs to be reserved for execution in the guest
    if let Some(hooks) = spec.hooks().as_ref() {
        let mut oci_hooks = oci::Hooks::default();
        oci_hooks.set_start_container(hooks.start_container().clone());
        spec.set_hooks(Some(oci_hooks));
    }

    // special process K8s ephemeral volumes.
    update_ephemeral_storage_type(spec);

    if let Some(linux) = &mut spec.linux_mut() {
        if disable_guest_seccomp {
            linux.set_seccomp(None);
        }

        // Host pid/net/time namespace paths do not make sense in kata.
        // Pid is aligned with the sandbox namespace whenever it is set.
        let ns: Vec<oci::LinuxNamespace> = linux
            .namespaces()
            .clone()
            .unwrap_or_default()
            .iter()
            .filter(|n| {
                n.typ() != oci::LinuxNamespaceType::Pid
                    && n.typ() != oci::LinuxNamespaceType::Network
                    && n.typ() != oci::LinuxNamespaceType::Time
            })
            .map(|n| {
                let mut ns = oci::LinuxNamespace::default();
                ns.set_typ(n.typ());
                ns
            })
            .collect();

        linux.set_namespaces(if ns.is_empty() { None } else { Some(ns) });
    }

    if disable_guest_selinux {
        if let Some(ref mut process) = spec.process_mut() {
            process.set_selinux_label(None);
        }
        if let Some(ref mut linux) = spec.linux_mut() {
            linux.set_mount_label(None);
        }
    }

    Ok(())
}

fn get_disable_guest_selinux(toml_config: &TomlConfig) -> bool {
    match toml_config
        .hypervisor
        .get(&toml_config.runtime.hypervisor_name)
    {
        Some(hypervisor_config) => hypervisor_config.disable_guest_selinux,
        // This shouldn't happen due to how logic in the config crate works
        // but we need to handle it anyway so we stick with the default
        // value of disable_guest_selinux in configuration.toml which
        // is 'true'.
        None => true,
    }
}

// is_hugetlb_backed tells whether the guest runs on memory taken from the
// host's hugetlb pool, which is the memory a pod reserves as a
// hugepages-<size> resource. Transparent huge pages come from ordinary memory
// and are charged to the memory limit instead.
fn is_hugetlb_backed(toml_config: &TomlConfig) -> bool {
    match toml_config
        .hypervisor
        .get(&toml_config.runtime.hypervisor_name)
    {
        Some(hypervisor_config) => {
            hypervisor_config.memory_info.enable_hugepages
                && matches!(
                    hypervisor_config.memory_info.hugepage_type,
                    HugePageType::Hugetlbfs
                )
        }
        None => false,
    }
}

// static_guest_memory_mb returns the size of a guest that has all the memory it
// will ever have, and 0 for one that grows on demand, as the memory a limit asks
// for is hotplugged to it after the container is created.
fn static_guest_memory_mb(toml_config: &TomlConfig) -> u32 {
    if !toml_config.runtime.static_sandbox_resource_mgmt {
        return 0;
    }

    toml_config
        .hypervisor
        .get(&toml_config.runtime.hypervisor_name)
        .map(|hypervisor_config| hypervisor_config.memory_info.default_memory)
        .unwrap_or(0)
}

/// add_huge_pages_to_memory_limit folds a container's huge page reservations
/// into the memory limits the agent applies to it inside the guest.
///
/// The memory limit of a huge page backed container accounts for what the
/// sandbox uses outside the guest: the memory the guest itself runs on comes
/// from the hugetlb pool, and the pod reserves it as a hugepages-<size>
/// resource instead. Inside the guest that reservation is ordinary RAM, charged
/// to the container's memory cgroup like any other page, so applying the host
/// limit verbatim caps the container far below the memory the pod reserved for
/// it. Adding the reservation keeps the ceiling the pod described, and keeps it
/// per container: a sidecar that reserves no huge pages stays bounded by its
/// own limit.
///
/// Swap follows the limit, as it carries memory plus swap and a total below the
/// memory limit is rejected. The room it left above the limit is kept, so
/// holding the limit down does not take a configured swap allowance with it.
///
/// A guest whose size is fixed at boot passes that size as `guest_mem_mb`, and
/// the sum is held below the memory such a guest can hold. A ceiling above it
/// never stops the container: the guest's own OOM killer does, and it chooses
/// among every process in the guest, where the agent and the guest's init sit at
/// a lower oom_score_adj than a Kubernetes workload and are killed first, with
/// the pod reporting neither a restart nor an OOM. Container aware runtimes read
/// the ceiling too, and size themselves out of memory from one the guest cannot
/// meet. A guest that grows on demand passes 0, as the memory to cover the sum
/// is hotplugged to it later.
fn add_huge_pages_to_memory_limit(
    resources: &mut LinuxResources,
    huge_pages: i64,
    guest_mem_mb: u32,
) {
    if huge_pages <= 0 {
        return;
    }

    let holdable = holdable_guest_memory_bytes(guest_mem_mb);
    if let Some(memory) = resources.memory_mut() {
        // Memory plus swap minus memory is the swap alone, and it survives
        // both the addition and the holding below.
        let swap_room = match (memory.limit(), memory.swap()) {
            (Some(limit), Some(swap)) if limit > 0 && swap > limit => swap - limit,
            _ => 0,
        };

        memory.set_limit(add_huge_pages_to_limit(memory.limit(), huge_pages));
        memory.set_reservation(add_huge_pages_to_limit(memory.reservation(), huge_pages));
        memory.set_swap(add_huge_pages_to_limit(memory.swap(), huge_pages));

        if let Some(holdable) = holdable {
            if memory.limit().unwrap_or(0) > holdable {
                info!(
                    sl!(),
                    "the container's huge page reservation reaches past the guest: holding its memory limit to the {} bytes a {} MiB guest can hold",
                    holdable,
                    guest_mem_mb
                );

                memory.set_limit(Some(holdable));
                if memory.swap().unwrap_or(0) > 0 {
                    memory.set_swap(Some(holdable.saturating_add(swap_room)));
                }
            }
            memory.set_reservation(hold_limit_to_guest(memory.reservation(), holdable));
        }
    }
}

/// huge_pages_total adds up the huge pages a container reserved, of whatever
/// page sizes, saturating rather than wrapping on a spec that asks for more
/// than there could ever be.
fn huge_pages_total(resources: Option<&LinuxResources>) -> i64 {
    resources
        .and_then(|resources| {
            resources.hugepage_limits().as_ref().map(|limits| {
                limits
                    .iter()
                    .fold(0i64, |total, l| total.saturating_add(l.limit()))
            })
        })
        .unwrap_or(0)
}

// holdable_guest_memory_bytes returns the memory a container in a guest of
// guest_mem_mb can be held to, or None for a guest whose size is not known
// upfront.
//
// A guest reports less memory than the VM was given: its kernel spends a 64
// byte struct page on every 4 KiB page, a sixty-fourth of the whole, before
// MemTotal is counted. What is left still has to carry the guest's kernel, the
// agent and the page cache, so leave a sixty-fourth of it for them as well --
// never less than 128 MiB, which is more than a small guest's share of either.
fn holdable_guest_memory_bytes(guest_mem_mb: u32) -> Option<i64> {
    if guest_mem_mb == 0 {
        return None;
    }

    const MIN_RESERVE_MB: u32 = 128;
    let reserve_mb = (guest_mem_mb / 32).max(MIN_RESERVE_MB);
    if reserve_mb >= guest_mem_mb {
        return None;
    }

    Some(i64::from(guest_mem_mb - reserve_mb) * 1024 * 1024)
}

// hold_limit_to_guest keeps a memory limit at or below what the guest can hold,
// leaving an unset (or unlimited) limit alone.
fn hold_limit_to_guest(limit: Option<i64>, holdable: i64) -> Option<i64> {
    match limit {
        Some(limit) if limit > holdable => Some(holdable),
        other => other,
    }
}

// add_huge_pages_to_limit grows a memory limit by a huge page reservation,
// leaving an unset (or unlimited) limit alone and saturating rather than
// wrapping.
fn add_huge_pages_to_limit(limit: Option<i64>, huge_pages: i64) -> Option<i64> {
    match limit {
        Some(limit) if limit > 0 => Some(limit.saturating_add(huge_pages)),
        other => other,
    }
}

// is_pid_namespace_enabled checks if Pid namespace for a container needs to be shared with its sandbox
// pid namespace.
fn is_pid_namespace_enabled(spec: &oci::Spec) -> bool {
    if let Some(linux) = spec.linux().as_ref() {
        let namespaces = linux.namespaces().clone().unwrap_or_default();
        for n in namespaces.iter() {
            if n.typ() == oci::LinuxNamespaceType::Pid {
                return !n.path().is_none();
            }
        }
    }

    false
}

/// Filter VFIO devices from the Linux device list based on vfio_mode configuration.
/// - vfio mode: Keeps all devices including /dev/vfio/*
/// - guest-kernel mode: Removes /dev/vfio/* devices as they're managed by guest kernel
///   Note that the guest-kernel mode is assumed if vfio_mode is unset/empty.
fn filter_vfio_devices(linux: &mut oci::Linux, vfio_mode: &str) {
    if vfio_mode == "vfio" {
        return;
    }

    const VFIO_PATH: &str = "/dev/vfio/";
    let filtered = linux.devices().as_ref().map(|devices| {
        devices
            .iter()
            .filter(|d| {
                !(d.typ() == oci::LinuxDeviceType::C
                    && d.path().to_str().is_some_and(|p| p.starts_with(VFIO_PATH)))
            })
            .cloned()
            .collect::<Vec<_>>()
    });
    linux.set_devices(match filtered {
        Some(v) if v.is_empty() => None,
        other => other,
    });
}

#[cfg(test)]
mod tests {
    use super::amend_spec;
    use super::is_pid_namespace_enabled;
    use super::*;
    use oci_spec::runtime::LinuxNamespaceType;
    use oci_spec::runtime::{LinuxBuilder, LinuxNamespaceBuilder};

    #[test]
    fn test_amend_spec_disable_guest_seccomp() {
        let mut spec = oci::Spec::default();
        let mut linux = oci::Linux::default();
        linux.set_seccomp(Some(oci::LinuxSeccomp::default()));
        spec.set_linux(Some(linux));

        assert!(spec.linux().as_ref().unwrap().seccomp().is_some());

        // disable_guest_seccomp = false
        amend_spec(&mut spec, false, false).unwrap();
        assert!(spec.linux().as_ref().unwrap().seccomp().is_some());

        // disable_guest_seccomp = true
        amend_spec(&mut spec, true, false).unwrap();
        assert!(spec.linux().as_ref().unwrap().seccomp().is_none());
    }

    #[test]
    fn test_amend_spec_strips_host_time_namespace() {
        let mut spec = oci::Spec::default();
        let linux = LinuxBuilder::default()
            .namespaces(vec![
                LinuxNamespaceBuilder::default()
                    .typ(LinuxNamespaceType::Time)
                    .path("/proc/1/ns/time")
                    .build()
                    .unwrap(),
                LinuxNamespaceBuilder::default()
                    .typ(LinuxNamespaceType::Uts)
                    .build()
                    .unwrap(),
            ])
            .build()
            .unwrap();
        spec.set_linux(Some(linux));

        amend_spec(&mut spec, false, false).unwrap();

        let namespaces = spec
            .linux()
            .as_ref()
            .unwrap()
            .namespaces()
            .as_ref()
            .unwrap();
        assert_eq!(namespaces.len(), 1);
        assert_eq!(namespaces[0].typ(), LinuxNamespaceType::Uts);
    }

    #[test]
    fn test_amend_spec_disable_guest_selinux() {
        let mut spec = oci::SpecBuilder::default()
            .process(
                oci::ProcessBuilder::default()
                    .selinux_label("xxx".to_owned())
                    .build()
                    .unwrap(),
            )
            .linux(
                oci::LinuxBuilder::default()
                    .mount_label("yyy".to_owned())
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap();

        // disable_guest_selinux = false, selinux labels are left alone
        amend_spec(&mut spec, false, false).unwrap();
        assert!(spec.process().as_ref().unwrap().selinux_label() == &Some("xxx".to_owned()));
        assert!(spec.linux().as_ref().unwrap().mount_label() == &Some("yyy".to_owned()));

        // disable_guest_selinux = true, selinux labels are reset
        amend_spec(&mut spec, false, true).unwrap();
        assert!(spec.process().as_ref().unwrap().selinux_label().is_none());
        assert!(spec.linux().as_ref().unwrap().mount_label().is_none());
    }

    #[test]
    fn test_add_huge_pages_to_memory_limit() {
        const GIB: i64 = 1024 * 1024 * 1024;
        const HUGE_PAGES: i64 = 64 * GIB;

        struct TestData<'a> {
            desc: &'a str,
            memory: oci::LinuxMemory,
            huge_pages: Option<Vec<oci::LinuxHugepageLimit>>,
            guest_mem_mb: u32,
            expected: oci::LinuxMemory,
        }

        let huge_page_limits = || {
            Some(vec![oci::LinuxHugepageLimitBuilder::default()
                .page_size("1GB".to_owned())
                .limit(HUGE_PAGES)
                .build()
                .unwrap()])
        };

        let memory = |limit: Option<i64>, reservation: Option<i64>, swap: Option<i64>| {
            let mut memory = oci::LinuxMemory::default();
            memory.set_limit(limit);
            memory.set_reservation(reservation);
            memory.set_swap(swap);
            memory
        };

        let tests = &[
            TestData {
                desc: "the huge page reservation joins the limits the guest applies",
                memory: memory(Some(2 * GIB), Some(2 * GIB), Some(2 * GIB)),
                huge_pages: huge_page_limits(),
                guest_mem_mb: 0,
                expected: memory(
                    Some(2 * GIB + HUGE_PAGES),
                    Some(2 * GIB + HUGE_PAGES),
                    Some(2 * GIB + HUGE_PAGES),
                ),
            },
            TestData {
                desc: "a container reserving no huge pages keeps its limit",
                memory: memory(Some(256 * GIB / 1024), None, None),
                huge_pages: None,
                guest_mem_mb: 0,
                expected: memory(Some(256 * GIB / 1024), None, None),
            },
            TestData {
                desc: "an unset limit stays unlimited",
                memory: memory(None, Some(0), Some(-1)),
                huge_pages: huge_page_limits(),
                guest_mem_mb: 0,
                expected: memory(None, Some(0), Some(-1)),
            },
            TestData {
                desc: "a reservation that cannot be added saturates",
                memory: memory(Some(2 * GIB), None, None),
                huge_pages: Some(vec![oci::LinuxHugepageLimitBuilder::default()
                    .page_size("1GB".to_owned())
                    .limit(i64::MAX)
                    .build()
                    .unwrap()]),
                guest_mem_mb: 0,
                expected: memory(Some(i64::MAX), None, None),
            },
            TestData {
                desc: "a sum reaching past a fixed size guest is held to what it can hold",
                memory: memory(Some(2 * GIB), Some(2 * GIB), Some(2 * GIB)),
                huge_pages: Some(vec![oci::LinuxHugepageLimitBuilder::default()
                    .page_size("1GB".to_owned())
                    .limit(192 * GIB)
                    .build()
                    .unwrap()]),
                // A 192Gi guest reports about 189Gi, so hold the container to
                // 186Gi: 192Gi less a thirty-second of it.
                guest_mem_mb: 192 * 1024,
                expected: memory(Some(186 * GIB), Some(186 * GIB), Some(186 * GIB)),
            },
            TestData {
                desc: "a small guest keeps the whole reserve",
                memory: memory(Some(256 * GIB / 1024), None, None),
                huge_pages: Some(vec![oci::LinuxHugepageLimitBuilder::default()
                    .page_size("2MB".to_owned())
                    .limit(2 * GIB)
                    .build()
                    .unwrap()]),
                guest_mem_mb: 2 * 1024,
                expected: memory(Some((2 * 1024 - 128) * GIB / 1024), None, None),
            },
            TestData {
                desc: "a sum a fixed size guest can hold is left alone",
                memory: memory(Some(GIB / 4), None, None),
                huge_pages: Some(vec![oci::LinuxHugepageLimitBuilder::default()
                    .page_size("1GB".to_owned())
                    .limit(4 * GIB)
                    .build()
                    .unwrap()]),
                guest_mem_mb: 192 * 1024,
                expected: memory(Some(GIB / 4 + 4 * GIB), None, None),
            },
            TestData {
                desc: "holding the limit down leaves the swap the container was given",
                memory: memory(Some(2 * GIB), None, Some(4 * GIB)),
                huge_pages: huge_page_limits(),
                // The 2Gi of swap asked for above the memory limit stays
                // above the 62Gi a 64Gi guest can hold.
                guest_mem_mb: 64 * 1024,
                expected: memory(Some(62 * GIB), None, Some(64 * GIB)),
            },
            TestData {
                desc: "reservations of several page sizes saturate together",
                memory: memory(Some(2 * GIB), None, None),
                huge_pages: Some(vec![
                    oci::LinuxHugepageLimitBuilder::default()
                        .page_size("1GB".to_owned())
                        .limit(i64::MAX)
                        .build()
                        .unwrap(),
                    oci::LinuxHugepageLimitBuilder::default()
                        .page_size("2MB".to_owned())
                        .limit(2 * GIB)
                        .build()
                        .unwrap(),
                ]),
                guest_mem_mb: 0,
                expected: memory(Some(i64::MAX), None, None),
            },
        ];

        for d in tests.iter() {
            let mut resources = LinuxResources::default();
            resources.set_memory(Some(d.memory));
            resources.set_hugepage_limits(d.huge_pages.clone());

            let huge_pages = huge_pages_total(Some(&resources));
            add_huge_pages_to_memory_limit(&mut resources, huge_pages, d.guest_mem_mb);

            assert_eq!(
                resources.memory().unwrap(),
                d.expected,
                "test case: {}",
                d.desc
            );
        }
    }

    #[test]
    fn test_is_pid_namespace_enabled() {
        struct TestData<'a> {
            desc: &'a str,
            namespaces: Vec<oci::LinuxNamespace>,
            result: bool,
        }

        let tests = &[
            TestData {
                desc: "no pid namespace",
                namespaces: vec![LinuxNamespaceBuilder::default()
                    .typ(LinuxNamespaceType::Network)
                    .path("/dev/null")
                    .build()
                    .unwrap()],
                result: false,
            },
            TestData {
                desc: "empty pid namespace path",
                namespaces: vec![
                    LinuxNamespaceBuilder::default()
                        .typ(LinuxNamespaceType::Network)
                        .build()
                        .unwrap(),
                    LinuxNamespaceBuilder::default()
                        .typ(LinuxNamespaceType::Pid)
                        .build()
                        .unwrap(),
                ],
                result: false,
            },
            TestData {
                desc: "pid namespace is set",
                namespaces: vec![
                    LinuxNamespaceBuilder::default()
                        .typ(LinuxNamespaceType::Network)
                        .path("/some/path")
                        .build()
                        .unwrap(),
                    LinuxNamespaceBuilder::default()
                        .typ(LinuxNamespaceType::Pid)
                        .path("/dev/null")
                        .build()
                        .unwrap(),
                ],
                result: true,
            },
        ];

        let mut spec = oci::Spec::default();

        for (i, d) in tests.iter().enumerate() {
            spec.set_linux(Some(
                LinuxBuilder::default()
                    .namespaces(d.namespaces.clone())
                    .build()
                    .unwrap(),
            ));
            // spec.linux = Some(oci::Linux {
            //     namespaces: d.namespaces.clone(),
            //     ..Default::default()
            // });

            assert_eq!(
                d.result,
                is_pid_namespace_enabled(&spec),
                "test[{}]: {:?}",
                i,
                d.desc
            );
        }
    }

    #[test]
    fn test_filter_vfio_devices_guest_kernel_mode() {
        // Test that VFIO devices are filtered out in guest-kernel mode
        let vfio_device = oci::LinuxDeviceBuilder::default()
            .path("/dev/vfio/1")
            .typ(oci::LinuxDeviceType::C)
            .major(10)
            .minor(196)
            .build()
            .unwrap();

        let non_vfio_device = oci::LinuxDeviceBuilder::default()
            .path("/dev/null")
            .typ(oci::LinuxDeviceType::C)
            .major(1)
            .minor(3)
            .build()
            .unwrap();

        let mut linux = oci::LinuxBuilder::default()
            .devices(vec![vfio_device, non_vfio_device.clone()])
            .build()
            .unwrap();

        filter_vfio_devices(&mut linux, "guest-kernel");

        let devices = linux.devices().as_ref().unwrap();
        assert_eq!(
            devices.len(),
            1,
            "Should have only 1 device after filtering"
        );
        assert_eq!(
            devices[0].path(),
            non_vfio_device.path(),
            "Non-VFIO device should be preserved"
        );
    }

    #[test]
    fn test_filter_vfio_devices_vfio_mode() {
        // Test that VFIO devices are preserved in vfio mode
        let vfio_device = oci::LinuxDeviceBuilder::default()
            .path("/dev/vfio/1")
            .typ(oci::LinuxDeviceType::C)
            .major(10)
            .minor(196)
            .build()
            .unwrap();

        let non_vfio_device = oci::LinuxDeviceBuilder::default()
            .path("/dev/null")
            .typ(oci::LinuxDeviceType::C)
            .major(1)
            .minor(3)
            .build()
            .unwrap();

        let mut linux = oci::LinuxBuilder::default()
            .devices(vec![vfio_device, non_vfio_device])
            .build()
            .unwrap();

        filter_vfio_devices(&mut linux, "vfio");

        let devices = linux.devices().as_ref().unwrap();
        assert_eq!(devices.len(), 2, "Should have both devices in vfio mode");
    }

    #[test]
    fn test_filter_vfio_devices_only_vfio_filtered() {
        // Test that only /dev/vfio/* devices are filtered in guest-kernel mode
        let vfio_device = oci::LinuxDeviceBuilder::default()
            .path("/dev/vfio/1")
            .typ(oci::LinuxDeviceType::C)
            .major(10)
            .minor(196)
            .build()
            .unwrap();

        let vfio_container = oci::LinuxDeviceBuilder::default()
            .path("/dev/vfio/vfio")
            .typ(oci::LinuxDeviceType::C)
            .major(10)
            .minor(196)
            .build()
            .unwrap();

        let similar_path = oci::LinuxDeviceBuilder::default()
            .path("/dev/vfio-test")
            .typ(oci::LinuxDeviceType::C)
            .major(1)
            .minor(1)
            .build()
            .unwrap();

        let mut linux = oci::LinuxBuilder::default()
            .devices(vec![vfio_device, vfio_container, similar_path.clone()])
            .build()
            .unwrap();

        filter_vfio_devices(&mut linux, "guest-kernel");

        let devices = linux.devices().as_ref().unwrap();
        assert_eq!(
            devices.len(),
            1,
            "Should only filter devices starting with /dev/vfio/"
        );
        assert_eq!(
            devices[0].path(),
            similar_path.path(),
            "Device with similar but different path should be preserved"
        );
    }

    #[test]
    fn test_filter_vfio_devices_empty_mode() {
        // Test default/empty mode behavior (should filter /dev/vfio/* like guest-kernel mode)
        let vfio_device = oci::LinuxDeviceBuilder::default()
            .path("/dev/vfio/1")
            .typ(oci::LinuxDeviceType::C)
            .major(10)
            .minor(196)
            .build()
            .unwrap();

        let mut linux = oci::LinuxBuilder::default()
            .devices(vec![vfio_device])
            .build()
            .unwrap();

        filter_vfio_devices(&mut linux, "");

        assert!(
            linux.devices().is_none(),
            "Should filter out VFIO device with empty mode"
        );
    }

    #[test]
    fn test_filter_vfio_devices_no_devices() {
        // Test that filtering works when there are no devices
        let mut linux = oci::LinuxBuilder::default().build().unwrap();

        filter_vfio_devices(&mut linux, "guest-kernel");

        assert!(
            linux.devices().is_none(),
            "Should remain None when no devices"
        );
    }
}
