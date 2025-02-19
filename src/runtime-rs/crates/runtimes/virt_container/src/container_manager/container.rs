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
    error::Error,
    types::{
        ContainerConfig, ContainerID, ContainerProcess, ProcessStateInfo, ProcessStatus,
        ProcessType,
    },
};
use kata_sys_util::k8s::update_ephemeral_storage_type;
use kata_types::k8s;
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
use crate::container_manager::logger_with_process;

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
        let disable_guest_selinux = match toml_config
            .hypervisor
            .get(&toml_config.runtime.hypervisor_name)
        {
            Some(hypervisor_config) => hypervisor_config.disable_guest_selinux,
            // This shouldn't happen due to how logic in the config crate works
            // but we need to handle it anyway so we stick with the default
            // value of disable_guest_selinux in configuration.toml which
            // is 'true'.
            None => true,
        };
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
        if let Some(storage) = rootfs.get_storage().await {
            storages.push(storage);
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
                if let Err(err) = inner.start_container(&process.container_id).await {
                    let device_manager = self.resource_manager.get_device_manager().await;
                    let _ = inner.stop_process(process, true, &device_manager).await;
                    return Err(err);
                }

                if self.passfd_listener_addr.is_some() {
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

                if self.passfd_listener_addr.is_some() {
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
        inner.signal_process(container_process, signal, all).await
    }

    pub async fn exec_process(
        &self,
        container_process: &ContainerProcess,
        stdin: Option<String>,
        stdout: Option<String>,
        stderr: Option<String>,
        terminal: bool,
        oci_process: OCIProcess,
    ) -> Result<()> {
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
        inner.linux_resources = Some(resources.clone());
        // update vcpus, mems and host cgroups
        let agent_resources = self
            .resource_manager
            .update_linux_resource(
                &self.config.container_id,
                Some(resources),
                ResourceUpdateOp::Update,
            )
            .await?;

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
        inner
            .cleanup_container(
                self.container_id.container_id.as_str(),
                true,
                &device_manager,
            )
            .await
    }
}

fn amend_spec(
    spec: &mut oci::Spec,
    disable_guest_seccomp: bool,
    disable_guest_selinux: bool,
) -> Result<()> {
    // Only the StartContainer hook needs to be reserved for execution in the guest
    let start_container_hooks = if let Some(hooks) = spec.hooks().as_ref() {
        hooks.start_container().clone()
    } else {
        None
    };

    let mut oci_hooks = oci::Hooks::default();
    oci_hooks.set_start_container(start_container_hooks);
    spec.set_hooks(Some(oci_hooks));

    // special process K8s ephemeral volumes.
    update_ephemeral_storage_type(spec);

    if let Some(linux) = spec.linux_mut() {
        if disable_guest_seccomp {
            linux.set_seccomp(None);
        }

        if let Some(_resource) = linux.resources_mut() {
            LinuxResources::default();
        }

        // Host pidns path does not make sense in kata. Let's just align it with
        // sandbox namespace whenever it is set.
        let ns: Vec<oci::LinuxNamespace> = linux
            .namespaces()
            .clone()
            .unwrap_or_default()
            .iter()
            .filter(|n| {
                n.typ() != oci::LinuxNamespaceType::Pid
                    && n.typ() != oci::LinuxNamespaceType::Network
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
}
