// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use agent::Agent;
use common::{
    error::Error,
    types::{
        ContainerConfig, ContainerID, ContainerProcess, ExecProcessRequest, KillRequest,
        ProcessExitStatus, ProcessStateInfo, ProcessStatus, ProcessType, ResizePTYRequest,
        ShutdownRequest, StatsInfo, UpdateRequest, PID,
    },
    ContainerManager,
};
use hypervisor::Hypervisor;
use oci::Process as OCIProcess;
use oci_spec::runtime as oci;
use resource::ResourceManager;
use runtime_spec as spec;
use tokio::sync::{watch, OnceCell, RwLock, RwLockWriteGuard};
use tracing::instrument;

use kata_sys_util::{hooks::HookStates, netns::NetnsGuard};

use crate::container_manager::is_termination_signal;
use crate::oom::CrioOomNotifier;

use super::{logger_with_process, Container};

pub struct VirtContainerManager {
    sid: String,
    pid: u32,
    containers: Arc<RwLock<HashMap<String, Container>>>,
    // Accessed while holding the containers lock. Create removes the old
    // record only after successfully creating a new task with the same ID.
    missing_container_exits: Mutex<HashMap<String, watch::Sender<Option<ProcessExitStatus>>>>,
    resource_manager: Arc<ResourceManager>,
    agent: Arc<dyn Agent>,
    hypervisor: Arc<dyn Hypervisor>,
    vmm_master_tid: OnceCell<u32>,
    oom_notifier: Arc<CrioOomNotifier>,
}

impl std::fmt::Debug for VirtContainerManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VirtContainerManager")
            .field("sid", &self.sid)
            .field("pid", &self.pid)
            .finish()
    }
}

fn from_hooks(hooks: &Option<Vec<oci::Hook>>) -> &[oci::Hook] {
    match hooks {
        Some(hooks_vec) => hooks_vec.as_slice(),
        None => &[],
    }
}

impl VirtContainerManager {
    pub fn new(
        sid: &str,
        pid: u32,
        agent: Arc<dyn Agent>,
        hypervisor: Arc<dyn Hypervisor>,
        resource_manager: Arc<ResourceManager>,
        oom_notifier: Arc<CrioOomNotifier>,
    ) -> Self {
        Self {
            sid: sid.to_string(),
            pid,
            containers: Default::default(),
            missing_container_exits: Default::default(),
            resource_manager,
            agent,
            hypervisor,
            vmm_master_tid: OnceCell::new(),
            oom_notifier,
        }
    }

    async fn get_vmm_master_tid(&self) -> Result<u32> {
        self.vmm_master_tid
            .get_or_try_init(|| self.hypervisor.get_vmm_master_tid())
            .await
            .copied()
    }

    fn missing_container_state(&self, process: &ContainerProcess) -> Option<ProcessStateInfo> {
        if process.process_type != ProcessType::Container || process.container_id() == self.sid {
            return None;
        }
        let exits = self.missing_container_exits.lock().unwrap();
        let exit = exits.get(process.container_id())?.borrow().clone()?;
        Some(ProcessStateInfo {
            container_id: process.container_id().to_string(),
            exec_id: String::new(),
            pid: PID { pid: 0 },
            bundle: String::new(),
            stdin: None,
            stdout: None,
            stderr: None,
            terminal: false,
            status: ProcessStatus::Stopped,
            exit_status: exit.exit_code,
            exited_at: exit.exit_time,
        })
    }

    fn subscribe_missing_container(&self, id: &str) -> watch::Receiver<Option<ProcessExitStatus>> {
        self.missing_container_exits
            .lock()
            .unwrap()
            .entry(id.to_string())
            .or_insert_with(|| watch::channel(None).0)
            .subscribe()
    }

    async fn begin_container_creation(&self) -> RwLockWriteGuard<'_, HashMap<String, Container>> {
        self.containers.write().await
    }

    fn finish_container_creation(
        &self,
        containers: &mut RwLockWriteGuard<'_, HashMap<String, Container>>,
        container: Container,
    ) {
        // Keep the accepted exit through failed or cancelled creation. Only a
        // successfully created replacement starts a new task generation.
        self.missing_container_exits
            .lock()
            .unwrap()
            .remove(&container.container_id.container_id);
        containers.insert(container.container_id.to_string(), container);
    }
}

#[async_trait]
impl ContainerManager for VirtContainerManager {
    #[instrument]
    async fn create_container(&self, config: ContainerConfig, spec: oci::Spec) -> Result<PID> {
        let vmm_master_tid = self.get_vmm_master_tid().await?;

        let mut container = Container::new(
            vmm_master_tid,
            config.clone(),
            spec.clone(),
            self.agent.clone(),
            self.resource_manager.clone(),
            self.hypervisor.get_passfd_listener_addr().await.ok(),
        )
        .await
        .context("new container")?;

        // CreateContainer Hooks:
        // * should be run in vmm namespace (hook path in runtime namespace)
        // * should be run after the vm is started, before container is created, and after CreateRuntime Hooks
        // * spec details: https://github.com/opencontainers/runtime-spec/blob/c1662686cff159595277b79322d0272f5182941b/config.md#createcontainer-hooks
        let vmm_ns_path = self.hypervisor.get_ns_path().await?;
        let vmm_netns_path = format!("{}/{}", vmm_ns_path, "net");
        let state = spec::State {
            version: spec.version().clone(),
            id: config.container_id.clone(),
            status: spec::ContainerState::Creating,
            pid: vmm_master_tid as i32,
            bundle: config.bundle.clone(),
            annotations: spec.annotations().clone().unwrap_or_default(),
        };

        // new scope, CreateContainer hooks in which will execute in a new network namespace
        {
            let _netns_guard = NetnsGuard::new(&vmm_netns_path).context("vmm netns guard")?;
            if let Some(hooks) = spec.hooks().as_ref() {
                let mut create_container_hook_states = HookStates::new();
                create_container_hook_states
                    .execute_hooks(from_hooks(hooks.create_container()), Some(state))?;
            }
        }

        self.oom_notifier
            .track(&config.container_id, &config.bundle, &spec)
            .await;

        let mut containers = self.begin_container_creation().await;
        if let Err(e) = container.create(spec).await {
            if let Err(inner_e) = container.cleanup().await {
                warn!(sl!(), "failed to cleanup container {:?}", inner_e);
            }
            self.oom_notifier.forget(&config.container_id).await;

            return Err(e);
        }

        self.finish_container_creation(&mut containers, container);
        Ok(PID {
            pid: vmm_master_tid,
        })
    }

    #[instrument]
    async fn close_process_io(&self, process: &ContainerProcess) -> Result<()> {
        let containers = self.containers.read().await;
        let container_id = &process.container_id.to_string();
        let c = containers
            .get(container_id)
            .ok_or_else(|| Error::ContainerNotFound(container_id.clone()))?;

        c.close_io(process).await.context("close io")?;
        Ok(())
    }

    #[instrument]
    async fn delete_process(&self, process: &ContainerProcess) -> Result<ProcessStateInfo> {
        let container_id = &process.container_id.container_id;
        match process.process_type {
            ProcessType::Container => {
                let mut containers = self.containers.write().await;
                let mut c = match containers.remove(container_id) {
                    Some(c) => c,
                    None => {
                        return self.missing_container_state(process).ok_or_else(|| {
                            Error::ContainerNotFound(container_id.to_string()).into()
                        })
                    }
                };
                // Returning an error here blocks containerd from processing the exit
                // and reaching sandbox teardown, so report the cleanup failure and continue.
                if let Err(e) = c.finish_cleanup().await {
                    error!(
                        sl!(),
                        "failed to finish cleanup of container {}: {:?}", container_id, e
                    );
                }

                // A task that never started has no exit-driven cleanup to release its mounts.
                let status = c.state_process(process).await.context("state process")?;
                if status.status == ProcessStatus::Created {
                    if let Err(e) = c.cleanup().await {
                        warn!(
                            logger_with_process(process),
                            "failed to clean up container deleted before start: {:?}", e
                        );
                    }
                }

                // Poststop Hooks:
                // * should be run in runtime namespace
                // * should be run after the container is deleted but before delete operation returns
                // * spec details: https://github.com/opencontainers/runtime-spec/blob/c1662686cff159595277b79322d0272f5182941b/config.md#poststop
                let c_spec = c.spec().await;

                let vmm_pid = self.get_vmm_master_tid().await?;
                let state = spec::State {
                    version: c_spec.version().clone(),
                    id: c.container_id.to_string(),
                    status: spec::ContainerState::Stopped,
                    pid: vmm_pid as i32,
                    bundle: c.config().await.bundle,
                    annotations: c_spec.annotations().clone().unwrap_or_default(),
                };
                if let Some(hooks) = c_spec.hooks().as_ref() {
                    let mut poststop_hook_states = HookStates::new();
                    poststop_hook_states
                        .execute_hooks(from_hooks(hooks.poststop()), Some(state))?;
                }

                c.state_process(process).await.context("state process")
            }
            ProcessType::Exec => {
                let containers = self.containers.read().await;
                let c = containers
                    .get(container_id)
                    .ok_or_else(|| Error::ContainerNotFound(container_id.to_string()))?;
                let state = c.state_process(process).await.context("state process");
                c.delete_exec_process(process)
                    .await
                    .context("delete process")?;
                return state;
            }
        }
    }

    #[instrument]
    async fn exec_process(&self, req: ExecProcessRequest) -> Result<()> {
        if req.spec_type_url.is_empty() {
            return Err(anyhow!("invalid type url"));
        }
        let mut oci_process: OCIProcess =
            serde_json::from_slice(&req.spec_value).context("serde from slice")?;

        oci_process.set_apparmor_profile(None);
        oci_process.set_capabilities(None);

        // CRI-O derives an exec's process from the container's, so a container
        // declaring tty: true asks for a terminal the exec never wanted. Only
        // req.terminal says how the streams are copied, so it decides here too.
        oci_process.set_terminal(Some(req.terminal));

        let containers = self.containers.read().await;
        let container_id = &req.process.container_id.container_id;
        let c = containers
            .get(container_id)
            .ok_or_else(|| Error::ContainerNotFound(container_id.clone()))?;
        c.exec_process(
            &req.process,
            req.stdin,
            req.stdout,
            req.stderr,
            req.terminal,
            oci_process,
        )
        .await
        .context("container exec")
    }

    #[instrument]
    async fn kill_process(&self, req: &KillRequest) -> Result<()> {
        let containers = self.containers.read().await;
        let container_id = &req.process.container_id.container_id;

        // According to CRI specs, kubelet will call StopPodSandbox()
        // at least once before calling RemovePodSandbox and this call
        // is idempotent. It must not return an error if all relevant
        // resources have already been reclaimed
        let c = match containers.get(container_id) {
            Some(c) => c,
            None => {
                // Container already removed - this is OK for SIGKILL/SIGTERM
                if is_termination_signal(req.signal) {
                    if req.process.process_type == ProcessType::Container
                        && container_id != &self.sid
                    {
                        let mut exits = self.missing_container_exits.lock().unwrap();
                        let exit = exits
                            .entry(container_id.clone())
                            .or_insert_with(|| watch::channel(None).0);
                        // Preserve the first accepted exit across duplicate
                        // Kill, Wait, State and Delete requests.
                        exit.send_if_modified(|status| {
                            if status.is_some() {
                                return false;
                            }
                            let mut stopped = ProcessExitStatus::new();
                            stopped.update_exit_code(128 + req.signal as i32);
                            *status = Some(stopped);
                            true
                        });
                    }
                    warn!(
                        sl!(),
                        "Signal {} ignored due to container not existing", req.signal;
                        "container" => container_id,
                        "signal" => req.signal
                    );
                    return Ok(());
                }
                return Err(Error::ContainerNotFound(container_id.clone()).into());
            }
        };

        // According to CRI specs, kubelet will call StopPodSandbox()
        // at least once before calling RemovePodSandbox, and this call
        // is idempotent, and must not return an error if all relevant
        // resources have already been reclaimed. And in that call it will
        // send a SIGKILL signal first to try to stop the container, thus
        // once the container has terminated, here should ignore this signal
        // and return directly.
        //
        // When the VM/agent is dead (e.g., QEMU killed externally), the ttrpc
        // connection will fail with AgentConnectionClosed error.
        // Additionally, if the container's init process is already gone, the
        // agent returns ProcessAlreadyTerminated error.
        // For SIGKILL/SIGTERM, we should treat these as success since the
        // container is effectively terminated.
        c.kill_process(self.containers.clone(), &req.process, req.signal, req.all)
            .await
            .or_else(|err| {
                let is_term_signal = is_termination_signal(req.signal);

                // Check for typed errors using downcast_ref
                let is_expected_error = matches!(
                    err.downcast_ref::<Error>(),
                    Some(Error::AgentConnectionClosed) | Some(Error::ProcessAlreadyTerminated)
                );

                if is_term_signal && is_expected_error {
                    warn!(
                        sl!(),
                        "Signal encounters expected error, VM/process already terminated";
                        "container" => container_id,
                        "process" => ?&req.process,
                        "signal" => req.signal,
                    );
                    Ok(())
                } else {
                    Err(err)
                }
            })
    }

    #[instrument]
    async fn wait_process(&self, process: &ContainerProcess) -> Result<ProcessExitStatus> {
        let logger = logger_with_process(process);

        let containers = self.containers.read().await;
        let container_id = &process.container_id.container_id;
        if !containers.contains_key(container_id)
            && process.process_type == ProcessType::Container
            && container_id != &self.sid
        {
            let mut exit = self.subscribe_missing_container(container_id);
            drop(containers);
            // containerd starts Wait before sending Kill for an UNKNOWN task.
            // Do not invent an exit until a termination signal was accepted.
            loop {
                if let Some(status) = exit.borrow_and_update().clone() {
                    return Ok(status);
                }
                if exit.changed().await.is_err() {
                    // Create invalidated this task generation.
                    return Err(Error::ContainerNotFound(container_id.clone()).into());
                }
            }
        }
        let c = containers
            .get(container_id)
            .ok_or_else(|| Error::ContainerNotFound(container_id.clone()))?;
        let (watcher, status) = c.wait_process(process).await.context("wait")?;
        drop(containers);

        match watcher {
            Some(mut watcher) => {
                info!(logger, "begin wait exit");
                while watcher.changed().await.is_ok() {}
                info!(logger, "end wait exited");
            }
            None => {
                warn!(logger, "failed to find watcher for wait process");
            }
        }

        let status = status.read().await;

        info!(logger, "wait process exit status {:?}", status);

        Ok(status.clone())
    }

    #[instrument]
    async fn start_process(&self, process: &ContainerProcess) -> Result<PID> {
        let containers = self.containers.read().await;
        let container_id = &process.container_id.container_id;
        let c = containers
            .get(container_id)
            .ok_or_else(|| Error::ContainerNotFound(container_id.clone()))?;
        c.start(self.containers.clone(), process)
            .await
            .context("start")?;

        // Poststart Hooks:
        // * should be run in runtime namespace
        // * should be run after user-specific command is executed but before start operation returns
        // * spec details: https://github.com/opencontainers/runtime-spec/blob/c1662686cff159595277b79322d0272f5182941b/config.md#poststart
        let c_spec = c.spec().await;
        let vmm_master_tid = self.get_vmm_master_tid().await?;
        let state = spec::State {
            version: c_spec.version().clone(),
            id: c.container_id.to_string(),
            status: spec::ContainerState::Running,
            pid: vmm_master_tid as i32,
            bundle: c.config().await.bundle,
            annotations: c_spec.annotations().clone().unwrap_or_default(),
        };
        if let Some(hooks) = c_spec.hooks().as_ref() {
            let mut poststart_hook_states = HookStates::new();
            poststart_hook_states.execute_hooks(from_hooks(hooks.poststart()), Some(state))?;
        }

        Ok(PID {
            pid: vmm_master_tid,
        })
    }

    #[instrument]
    async fn state_process(&self, process: &ContainerProcess) -> Result<ProcessStateInfo> {
        let containers = self.containers.read().await;
        let container_id = &process.container_id.container_id;

        // When using Sandbox API, the sandbox container (container_id == sandbox_id)
        // is not stored in the containers map. Return a synthetic state for it.
        if let Some(c) = containers.get(container_id) {
            c.state_process(process).await.context("state process")
        } else if container_id == &self.sid {
            let vmm_pid = self.get_vmm_master_tid().await?;
            Ok(ProcessStateInfo {
                container_id: self.sid.clone(),
                exec_id: String::new(),
                pid: PID { pid: vmm_pid },
                bundle: String::new(),
                stdin: None,
                stdout: None,
                stderr: None,
                terminal: false,
                status: ProcessStatus::Running,
                exit_status: 0,
                exited_at: None,
            })
        } else if let Some(state) = self.missing_container_state(process) {
            Ok(state)
        } else {
            Err(Error::ContainerNotFound(container_id.clone()).into())
        }
    }

    #[instrument]
    async fn pause_container(&self, id: &ContainerID) -> Result<()> {
        let containers = self.containers.read().await;
        let c = containers
            .get(&id.container_id)
            .ok_or_else(|| Error::ContainerNotFound(id.container_id.clone()))?;
        c.pause().await.context("pause")?;
        Ok(())
    }

    #[instrument]
    async fn resume_container(&self, id: &ContainerID) -> Result<()> {
        let containers = self.containers.read().await;
        let c = containers
            .get(&id.container_id)
            .ok_or_else(|| Error::ContainerNotFound(id.container_id.clone()))?;
        c.resume().await.context("resume")?;
        Ok(())
    }

    #[instrument]
    async fn resize_process_pty(&self, req: &ResizePTYRequest) -> Result<()> {
        let containers = self.containers.read().await;
        let c = containers
            .get(&req.process.container_id.container_id)
            .ok_or_else(|| {
                Error::ContainerNotFound(req.process.container_id.container_id.clone())
            })?;
        c.resize_pty(&req.process, req.width, req.height)
            .await
            .context("resize pty")?;
        Ok(())
    }

    #[instrument]
    async fn stats_container(&self, id: &ContainerID) -> Result<StatsInfo> {
        let containers = self.containers.read().await;
        let c = containers
            .get(&id.container_id)
            .ok_or_else(|| Error::ContainerNotFound(id.container_id.clone()))?;
        let stats = c.stats().await.context("stats")?;
        Ok(StatsInfo::from(stats))
    }

    #[instrument]
    async fn update_container(&self, req: UpdateRequest) -> Result<()> {
        let resource = serde_json::from_slice::<oci::LinuxResources>(&req.value)
            .context("deserialize LinuxResource")?;
        let containers = self.containers.read().await;
        let container_id = &req.container_id;
        let c = containers
            .get(container_id)
            .ok_or_else(|| Error::ContainerNotFound(container_id.to_string()))?;
        c.update(&resource).await.context("update_container")
    }

    #[instrument]
    async fn pid(&self) -> Result<PID> {
        let vmm_pid = self.get_vmm_master_tid().await?;
        Ok(PID { pid: vmm_pid })
    }

    #[instrument]
    async fn connect_container(&self, _id: &ContainerID) -> Result<PID> {
        let vmm_pid = self.get_vmm_master_tid().await?;
        Ok(PID { pid: vmm_pid })
    }

    #[instrument]
    async fn need_shutdown_sandbox(&self, req: &ShutdownRequest) -> bool {
        req.is_now || self.sid == req.container_id
    }

    #[instrument]
    async fn is_sandbox_container(&self, process: &ContainerProcess) -> bool {
        process.process_type == ProcessType::Container
            && process.container_id.container_id == self.sid
    }
}

#[cfg(test)]
mod missing_container_tests {
    use super::*;
    use hypervisor::qemu::Qemu;
    use kata_types::config::TomlConfig;
    use persist::sandbox_persist::Persist;
    use resource::{
        cgroups::cgroup_persist::CgroupState, manager::ManagerArgs, resource_persist::ResourceState,
    };

    async fn manager() -> VirtContainerManager {
        let agent = Arc::new(agent::kata::KataAgent::new(Default::default()));
        let hypervisor = Arc::new(Qemu::new());
        // Restore only opens managers for these nonexistent cgroups. It does
        // not create cgroups or move the test process into one.
        let resources = Arc::new(
            ResourceManager::restore(
                ManagerArgs {
                    sid: "missing-container-test".into(),
                    agent: agent.clone(),
                    hypervisor: hypervisor.clone(),
                    config: TomlConfig::default(),
                },
                ResourceState {
                    cgroup_state: Some(CgroupState {
                        path: Some("missing-container-test".into()),
                        overhead_path: Some("missing-container-test-overhead".into()),
                        sandbox_cgroup_only: true,
                    }),
                    ..Default::default()
                },
            )
            .await
            .unwrap(),
        );
        VirtContainerManager::new(
            "sandbox",
            0,
            agent,
            hypervisor,
            resources,
            Arc::new(CrioOomNotifier::default()),
        )
    }

    async fn container(manager: &VirtContainerManager, id: &str) -> Container {
        Container::new(
            0,
            ContainerConfig {
                container_id: id.into(),
                bundle: String::new(),
                rootfs_mounts: vec![],
                terminal: false,
                options: None,
                stdin: None,
                stdout: None,
                stderr: None,
            },
            oci::Spec::default(),
            manager.agent.clone(),
            manager.resource_manager.clone(),
            None,
        )
        .await
        .unwrap()
    }

    fn kill(id: &str, signal: u32) -> KillRequest {
        KillRequest {
            process: ContainerProcess::new(id, "").unwrap(),
            signal,
            all: false,
        }
    }

    #[rstest::rstest]
    #[case(9, 137)]
    #[case(15, 143)]
    #[tokio::test]
    async fn accepted_kill_completes_wait_state_and_delete(#[case] signal: u32, #[case] code: i32) {
        let manager = manager().await;
        let request = kill("ghost", signal);
        manager.kill_process(&request).await.unwrap();
        let exit = manager.wait_process(&request.process).await.unwrap();
        assert_eq!(exit.exit_code, code);
        assert!(exit.exit_time.is_some());
        for _ in 0..2 {
            let state = manager.state_process(&request.process).await.unwrap();
            assert_eq!(state.status, ProcessStatus::Stopped);
            assert_eq!(state.exit_status, code);
            let deleted = manager.delete_process(&request.process).await.unwrap();
            assert_eq!(deleted.exit_status, code);
            manager.kill_process(&request).await.unwrap();
            assert_eq!(
                manager
                    .wait_process(&request.process)
                    .await
                    .unwrap()
                    .exit_time,
                exit.exit_time
            );
        }
    }

    #[tokio::test]
    async fn wait_before_kill_does_not_report_an_exit_early() {
        let manager = manager().await;
        let request = kill("ghost", 9);
        let wait = manager.wait_process(&request.process);
        tokio::pin!(wait);
        tokio::select! {
            biased;
            result = &mut wait => panic!("Wait completed without Kill: {:?}", result),
            _ = tokio::task::yield_now() => {}
        }
        assert!(manager.kill_process(&kill("ghost", 1)).await.is_err());
        assert!(manager.state_process(&request.process).await.is_err());
        assert!(manager.delete_process(&request.process).await.is_err());
        tokio::select! {
            biased;
            result = &mut wait => panic!("rejected signal completed Wait: {:?}", result),
            _ = tokio::task::yield_now() => {}
        }
        manager.kill_process(&request).await.unwrap();
        assert_eq!(
            tokio::time::timeout(std::time::Duration::from_secs(1), wait)
                .await
                .unwrap()
                .unwrap()
                .exit_code,
            137
        );
    }

    #[tokio::test]
    async fn cancelled_wait_does_not_lose_a_later_exit() {
        let manager = manager().await;
        let request = kill("ghost", 9);
        {
            let wait = manager.wait_process(&request.process);
            tokio::pin!(wait);
            tokio::select! {
                biased;
                result = &mut wait => panic!("Wait completed without Kill: {:?}", result),
                _ = tokio::task::yield_now() => {}
            }
        }
        manager.kill_process(&request).await.unwrap();
        assert_eq!(
            manager
                .wait_process(&request.process)
                .await
                .unwrap()
                .exit_code,
            137
        );
    }

    #[tokio::test]
    async fn missing_exec_and_sandbox_ids_do_not_get_synthetic_exits() {
        let manager = manager().await;
        for process in [
            ContainerProcess::new("ghost", "exec").unwrap(),
            ContainerProcess::new("sandbox", "").unwrap(),
        ] {
            manager
                .kill_process(&KillRequest {
                    process: process.clone(),
                    signal: 9,
                    all: false,
                })
                .await
                .unwrap();
            assert!(manager.wait_process(&process).await.is_err());
            assert!(manager.delete_process(&process).await.is_err());
        }
        assert!(manager.missing_container_exits.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn ghosts_do_not_require_an_empty_map_and_leave_other_containers_alone() {
        let manager = manager().await;
        for id in ["sandbox", "sibling"] {
            let mut containers = manager.begin_container_creation().await;
            manager.finish_container_creation(&mut containers, container(&manager, id).await);
        }
        let request = kill("ghost", 9);
        manager.kill_process(&request).await.unwrap();
        assert_eq!(
            manager
                .wait_process(&request.process)
                .await
                .unwrap()
                .exit_code,
            137
        );
        let containers = manager.containers.read().await;
        assert!(containers.contains_key("sandbox"));
        assert!(containers.contains_key("sibling"));
    }

    #[tokio::test]
    async fn successful_creation_invalidates_an_older_exit_and_pending_wait() {
        let manager = manager().await;
        let request = kill("reused", 9);
        manager.kill_process(&request).await.unwrap();
        let mut pending = manager.subscribe_missing_container("reused");
        let mut containers = manager.begin_container_creation().await;
        assert_eq!(
            manager
                .missing_container_state(&request.process)
                .unwrap()
                .exit_status,
            137
        );
        manager.finish_container_creation(&mut containers, container(&manager, "reused").await);
        drop(containers);
        assert!(manager.missing_container_state(&request.process).is_none());
        assert!(pending.changed().await.is_err());

        let process = ContainerProcess::new("pending", "").unwrap();
        let wait = manager.wait_process(&process);
        tokio::pin!(wait);
        tokio::select! {
            biased;
            result = &mut wait => panic!("Wait completed before Kill or Create: {:?}", result),
            _ = tokio::task::yield_now() => {}
        }
        let mut containers = manager.begin_container_creation().await;
        manager.finish_container_creation(&mut containers, container(&manager, "pending").await);
        drop(containers);
        assert!(wait.await.is_err());
    }

    #[rstest::rstest]
    #[case(true)]
    #[case(false)]
    #[tokio::test]
    async fn failed_creation_preserves_the_exit_and_waiters(#[case] accepted: bool) {
        let manager = manager().await;
        let request = kill("reused", 9);
        let mut pending = manager.subscribe_missing_container("reused");
        if accepted {
            manager.kill_process(&request).await.unwrap();
        }
        let replacement = container(&manager, "reused").await;
        let creating = manager.begin_container_creation().await;
        let mut invalid = oci::Spec::default();
        invalid.set_root(None);
        assert!(replacement.create(invalid).await.is_err());
        drop(creating);
        if !accepted {
            manager.kill_process(&request).await.unwrap();
        }
        tokio::time::timeout(std::time::Duration::from_secs(1), pending.changed())
            .await
            .unwrap()
            .unwrap();
        let state = manager.missing_container_state(&request.process).unwrap();
        assert_eq!(state.exit_status, 137);
        assert_eq!(
            manager
                .wait_process(&request.process)
                .await
                .unwrap()
                .exit_code,
            137
        );
        assert_eq!(
            manager
                .delete_process(&request.process)
                .await
                .unwrap()
                .exited_at,
            state.exited_at
        );
    }

    #[tokio::test]
    async fn a_kill_cannot_overtake_creation_and_leave_a_stale_exit() {
        let manager = manager().await;
        let created = container(&manager, "new").await;
        let mut creating = manager.begin_container_creation().await;
        let request = kill("new", 9);
        let kill = manager.kill_process(&request);
        tokio::pin!(kill);
        tokio::select! {
            biased;
            result = &mut kill => panic!("Kill bypassed creation lock: {:?}", result),
            _ = tokio::task::yield_now() => {}
        }
        manager.finish_container_creation(&mut creating, created);
        drop(creating);
        // The real task reaches the unconnected test agent; it must never be
        // mistaken for a missing task and acknowledged with a synthetic exit.
        assert!(kill.await.is_err());
        assert!(manager
            .missing_container_exits
            .lock()
            .unwrap()
            .get("new")
            .is_none());
    }
}
