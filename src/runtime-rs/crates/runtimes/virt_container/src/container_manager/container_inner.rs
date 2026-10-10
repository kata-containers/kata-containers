// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use agent::Agent;
use anyhow::{anyhow, Context, Result};
use common::{
    error::{is_no_such_process_error, Error},
    types::{ContainerID, ContainerProcess, ProcessExitStatus, ProcessStatus, ProcessType},
};
use hypervisor::device::device_manager::DeviceManager;
use nix::sys::signal::Signal;
use oci::LinuxResources;
use oci_spec::runtime as oci;
use resource::{rootfs::Rootfs, volume::Volume};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::RwLock;

use crate::container_manager::{convert_agent_error, logger_with_process};

use super::{
    io::ContainerIo,
    process::{Process, ProcessWatcher},
    Exec,
};

pub struct ContainerInner {
    agent: Arc<dyn Agent>,
    logger: slog::Logger,
    pub(crate) init_process: Process,
    pub(crate) exec_processes: HashMap<String, Exec>,
    pub(crate) rootfs: Vec<Arc<dyn Rootfs>>,
    pub(crate) volumes: Vec<Arc<dyn Volume>>,
    pub(crate) linux_resources: Option<LinuxResources>,
    removed_from_agent: bool,
}

impl ContainerInner {
    pub(crate) fn new(
        agent: Arc<dyn Agent>,
        init_process: Process,
        logger: slog::Logger,
        linux_resources: Option<LinuxResources>,
    ) -> Self {
        Self {
            agent,
            logger,
            init_process,
            exec_processes: HashMap::new(),
            rootfs: vec![],
            volumes: vec![],
            linux_resources,
            removed_from_agent: false,
        }
    }

    pub(crate) fn has_pending_cleanup(&self) -> bool {
        !self.volumes.is_empty() || !self.rootfs.is_empty()
    }

    fn container_id(&self) -> &str {
        self.init_process.process.container_id()
    }

    pub(crate) async fn check_state(&self, states: Vec<ProcessStatus>) -> Result<()> {
        let state = self.init_process.get_status().await;
        if states.contains(&state) {
            return Ok(());
        }

        Err(anyhow!(
            "failed to check state {:?} for {:?}",
            state,
            states
        ))
    }

    pub(crate) async fn set_state(&mut self, state: ProcessStatus) {
        let mut status = self.init_process.status.write().await;
        *status = state;
    }

    pub(crate) async fn start_exec_process(&mut self, process: &ContainerProcess) -> Result<()> {
        let exec = self
            .exec_processes
            .get_mut(&process.exec_id)
            .ok_or_else(|| Error::ProcessNotFound(process.clone()))?;

        self.agent
            .exec_process(agent::ExecProcessRequest {
                process_id: process.clone().into(),
                string_user: None,
                process: Some(exec.oci_process.clone()),
                stdin_port: exec.process.passfd_io.as_ref().and_then(|io| io.stdin_port),
                stdout_port: exec
                    .process
                    .passfd_io
                    .as_ref()
                    .and_then(|io| io.stdout_port),
                stderr_port: exec
                    .process
                    .passfd_io
                    .as_ref()
                    .and_then(|io| io.stderr_port),
            })
            .await
            .context("exec process")?;
        exec.process.set_status(ProcessStatus::Running).await;
        Ok(())
    }

    pub(crate) async fn win_resize_process(
        &self,
        process: &ContainerProcess,
        height: u32,
        width: u32,
    ) -> Result<()> {
        self.check_state(vec![ProcessStatus::Created, ProcessStatus::Running])
            .await
            .context("check state")?;

        self.agent
            .tty_win_resize(agent::TtyWinResizeRequest {
                process_id: process.clone().into(),
                row: height,
                column: width,
            })
            .await?;
        Ok(())
    }

    pub fn fetch_exit_watcher(&self, process: &ContainerProcess) -> Result<ProcessWatcher> {
        match process.process_type {
            ProcessType::Container => self.init_process.fetch_exit_watcher(),
            ProcessType::Exec => {
                let exec = self
                    .exec_processes
                    .get(&process.exec_id)
                    .ok_or_else(|| Error::ProcessNotFound(process.clone()))?;
                exec.process.fetch_exit_watcher()
            }
        }
    }

    pub(crate) async fn start_container(&mut self, cid: &ContainerID) -> Result<()> {
        self.check_state(vec![ProcessStatus::Created, ProcessStatus::Stopped])
            .await
            .context("check state")?;

        self.agent
            .start_container(agent::ContainerID {
                container_id: cid.container_id.clone(),
            })
            .await
            .context("start container")?;

        self.set_state(ProcessStatus::Running).await;

        Ok(())
    }

    async fn get_exit_status(&self) -> Arc<RwLock<ProcessExitStatus>> {
        self.init_process.exit_status.clone()
    }

    pub(crate) fn add_exec_process(&mut self, id: &str, exec: Exec) -> Option<Exec> {
        self.exec_processes.insert(id.to_string(), exec)
    }

    pub(crate) async fn delete_exec_process(&mut self, eid: &str) -> Result<()> {
        match self.exec_processes.remove(eid) {
            Some(_) => {
                debug!(self.logger, " delete process eid {}", eid);
                Ok(())
            }
            None => Err(anyhow!(
                "failed to find cid {} eid {}",
                self.container_id(),
                eid
            )),
        }
    }

    pub(crate) async fn cleanup_container(
        &mut self,
        cid: &str,
        force: bool,
        device_manager: &RwLock<DeviceManager>,
    ) -> Result<()> {
        // wait until the container process
        // terminated and the status write lock released.
        info!(self.logger, "wait on container terminated");
        let exit_status = self.get_exit_status().await;
        let _locked_exit_status = exit_status.read().await;
        info!(self.logger, "container terminated");
        if !self.removed_from_agent {
            let remove_request = agent::RemoveContainerRequest {
                container_id: cid.to_string(),
                ..Default::default()
            };
            self.agent
                .remove_container(remove_request)
                .await
                .or_else(|e| {
                    if force {
                        warn!(
                            self.logger,
                            "stop container: agent remove container failed: {}", e
                        );
                        Ok(agent::Empty::new())
                    } else {
                        Err(e)
                    }
                })?;
            self.removed_from_agent = true;
        }

        // close the exit channel to wakeup wait service
        // send to notify watchers who are waiting for the process exit
        self.init_process.stop().await;

        let volumes = self
            .clean_volumes(device_manager)
            .await
            .context("clean volumes");
        let rootfs = self
            .clean_rootfs(device_manager)
            .await
            .context("clean rootfs");
        volumes.and(rootfs)
    }

    pub(crate) async fn stop_process(
        &mut self,
        process: &ContainerProcess,
        force: bool,
        device_manager: &RwLock<DeviceManager>,
    ) -> Result<()> {
        let logger = logger_with_process(process);
        info!(logger, "begin to stop process");

        let state = self.init_process.get_status().await;
        if state == ProcessStatus::Stopped {
            if process.process_type == ProcessType::Container && self.has_pending_cleanup() {
                return self
                    .cleanup_container(&process.container_id.container_id, force, device_manager)
                    .await
                    .context("stop container");
            }
            return Ok(());
        }

        self.check_state(vec![ProcessStatus::Running])
            .await
            .context("check state")?;

        // Send kill signal to the container's init process.
        //
        // We must never abort teardown when signaling fails: the process may
        // already have exited (or the agent connection may already be gone
        // during sandbox shutdown), and in all of these cases we still have to
        // clean up the container's resources. Failing to do so leaves the
        // container's mounts in place, which later makes the sandbox-level
        // virtiofs cleanup fail with "Resource busy"/"Directory not empty".
        //
        // Errors indicating the process is already gone are logged at info
        // level; any other failure is logged as a warning, but cleanup always
        // proceeds.
        if let Err(e) = self
            .signal_process(process, Signal::SIGKILL as u32, false)
            .await
        {
            if is_no_such_process_error(&e) {
                info!(
                    logger,
                    "signal_process: init process is already gone, treat it as stopped"
                );
            } else {
                warn!(logger, "failed to send kill signal to process: {:?}", e);
            }
        }

        match process.process_type {
            ProcessType::Container => self
                .cleanup_container(&process.container_id.container_id, force, device_manager)
                .await
                .context("stop container")?,
            ProcessType::Exec => {
                let exec = self
                    .exec_processes
                    .get_mut(&process.exec_id)
                    .ok_or_else(|| anyhow!("failed to find exec"))?;
                exec.process.stop().await;
            }
        }

        Ok(())
    }

    pub(crate) async fn signal_process(
        &mut self,
        process: &ContainerProcess,
        signal: u32,
        all: bool,
    ) -> Result<()> {
        if self.check_state(vec![ProcessStatus::Stopped]).await.is_ok() {
            return Ok(());
        }

        let mut process_id: agent::ContainerProcessID = process.clone().into();
        if all {
            // force signal init process
            process_id.exec_id.clear();
        };

        self.agent
            .signal_process(agent::SignalProcessRequest { process_id, signal })
            .await
            .map_err(convert_agent_error)?;

        Ok(())
    }

    pub async fn new_container_io(&self, process: &ContainerProcess) -> Result<ContainerIo> {
        Ok(ContainerIo::new(self.agent.clone(), process.clone()))
    }

    pub async fn close_io(&mut self, process: &ContainerProcess) -> Result<()> {
        match process.process_type {
            ProcessType::Container => self.init_process.close_io(self.agent.clone()).await,
            ProcessType::Exec => {
                let exec = self
                    .exec_processes
                    .get_mut(&process.exec_id)
                    .ok_or_else(|| Error::ProcessNotFound(process.clone()))?;
                exec.process.close_io(self.agent.clone()).await;
            }
        };

        Ok(())
    }

    // Keep entries until cleanup succeeds so cancellation cannot lose unfinished resources.
    async fn clean_volumes(&mut self, device_manager: &RwLock<DeviceManager>) -> Result<()> {
        let mut failed = 0;
        let mut first_err = None;
        while failed < self.volumes.len() {
            let v = self.volumes[failed].clone();
            match v.cleanup(device_manager).await {
                Ok(()) => {
                    self.volumes.remove(failed);
                }
                Err(err) => {
                    warn!(
                        sl!(),
                        "Failed to clean the volume = {:?}, error = {:?}",
                        v.get_volume_mount(),
                        err
                    );
                    first_err.get_or_insert(err);
                    failed += 1;
                }
            }
        }
        match first_err {
            Some(err) => Err(err.context(format!("failed to clean {} volume(s)", failed))),
            None => Ok(()),
        }
    }

    async fn clean_rootfs(&mut self, device_manager: &RwLock<DeviceManager>) -> Result<()> {
        let mut failed = 0;
        let mut first_err = None;
        while failed < self.rootfs.len() {
            let rootfs = self.rootfs[failed].clone();
            match rootfs.cleanup(device_manager).await {
                Ok(()) => {
                    self.rootfs.remove(failed);
                }
                Err(err) => {
                    warn!(
                        sl!(),
                        "Failed to umount rootfs, cid = {:?}, error = {:?}",
                        self.container_id(),
                        err
                    );
                    first_err.get_or_insert(err);
                    failed += 1;
                }
            }
        }
        match first_err {
            Some(err) => Err(err.context(format!("failed to clean {} rootfs", failed))),
            None => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent::types::SetPolicyRequest;
    use async_trait::async_trait;
    use hypervisor::qemu::Qemu;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Default)]
    struct FakeAgent {
        removed: AtomicUsize,
    }

    #[async_trait]
    impl agent::AgentManager for FakeAgent {
        async fn start(&self, _address: &str) -> Result<()> {
            unimplemented!()
        }
        async fn stop(&self) {
            unimplemented!()
        }
        async fn disconnect(&self) -> Result<()> {
            unimplemented!()
        }
        async fn agent_sock(&self) -> Result<String> {
            unimplemented!()
        }
        async fn agent_config(&self) -> kata_types::config::Agent {
            unimplemented!()
        }
    }

    #[async_trait]
    impl agent::HealthService for FakeAgent {
        async fn check(&self, _: agent::CheckRequest) -> Result<agent::HealthCheckResponse> {
            unimplemented!()
        }
        async fn version(&self, _: agent::CheckRequest) -> Result<agent::VersionCheckResponse> {
            unimplemented!()
        }
    }

    #[async_trait]
    impl Agent for FakeAgent {
        async fn remove_container(&self, _: agent::RemoveContainerRequest) -> Result<agent::Empty> {
            self.removed.fetch_add(1, Ordering::SeqCst);
            Ok(agent::Empty::new())
        }
        async fn signal_process(&self, _: agent::SignalProcessRequest) -> Result<agent::Empty> {
            Ok(agent::Empty::new())
        }

        async fn create_sandbox(&self, _: agent::CreateSandboxRequest) -> Result<agent::Empty> {
            unimplemented!()
        }
        async fn destroy_sandbox(&self, _: agent::Empty) -> Result<agent::Empty> {
            unimplemented!()
        }
        async fn online_cpu_mem(&self, _: agent::OnlineCPUMemRequest) -> Result<agent::Empty> {
            unimplemented!()
        }
        async fn reseed_random_dev(
            &self,
            _: agent::ReseedRandomDevRequest,
        ) -> Result<agent::Empty> {
            unimplemented!()
        }
        async fn set_guest_date_time(
            &self,
            _: agent::SetGuestDateTimeRequest,
        ) -> Result<agent::Empty> {
            unimplemented!()
        }
        async fn add_arp_neighbors(&self, _: agent::AddArpNeighborRequest) -> Result<agent::Empty> {
            unimplemented!()
        }
        async fn list_interfaces(&self, _: agent::Empty) -> Result<agent::Interfaces> {
            unimplemented!()
        }
        async fn list_routes(&self, _: agent::Empty) -> Result<agent::Routes> {
            unimplemented!()
        }
        async fn update_interface(
            &self,
            _: agent::UpdateInterfaceRequest,
        ) -> Result<agent::Interface> {
            unimplemented!()
        }
        async fn update_routes(&self, _: agent::UpdateRoutesRequest) -> Result<agent::Routes> {
            unimplemented!()
        }
        async fn create_container(&self, _: agent::CreateContainerRequest) -> Result<agent::Empty> {
            unimplemented!()
        }
        async fn pause_container(&self, _: agent::ContainerID) -> Result<agent::Empty> {
            unimplemented!()
        }
        async fn resume_container(&self, _: agent::ContainerID) -> Result<agent::Empty> {
            unimplemented!()
        }
        async fn start_container(&self, _: agent::ContainerID) -> Result<agent::Empty> {
            unimplemented!()
        }
        async fn stats_container(
            &self,
            _: agent::ContainerID,
        ) -> Result<agent::StatsContainerResponse> {
            unimplemented!()
        }
        async fn update_container(&self, _: agent::UpdateContainerRequest) -> Result<agent::Empty> {
            unimplemented!()
        }
        async fn exec_process(&self, _: agent::ExecProcessRequest) -> Result<agent::Empty> {
            unimplemented!()
        }
        async fn wait_process(
            &self,
            _: agent::WaitProcessRequest,
        ) -> Result<agent::WaitProcessResponse> {
            unimplemented!()
        }
        async fn close_stdin(&self, _: agent::CloseStdinRequest) -> Result<agent::Empty> {
            unimplemented!()
        }
        async fn read_stderr(
            &self,
            _: agent::ReadStreamRequest,
        ) -> Result<agent::ReadStreamResponse> {
            unimplemented!()
        }
        async fn read_stdout(
            &self,
            _: agent::ReadStreamRequest,
        ) -> Result<agent::ReadStreamResponse> {
            unimplemented!()
        }
        async fn tty_win_resize(&self, _: agent::TtyWinResizeRequest) -> Result<agent::Empty> {
            unimplemented!()
        }
        async fn write_stdin(
            &self,
            _: agent::WriteStreamRequest,
        ) -> Result<agent::WriteStreamResponse> {
            unimplemented!()
        }
        async fn copy_file(&self, _: agent::CopyFileRequest) -> Result<agent::Empty> {
            unimplemented!()
        }
        async fn get_metrics(&self, _: agent::Empty) -> Result<agent::MetricsResponse> {
            unimplemented!()
        }
        async fn get_oom_event(&self, _: agent::Empty) -> Result<agent::OomEventResponse> {
            unimplemented!()
        }
        async fn get_ip_tables(
            &self,
            _: agent::GetIPTablesRequest,
        ) -> Result<agent::GetIPTablesResponse> {
            unimplemented!()
        }
        async fn set_ip_tables(
            &self,
            _: agent::SetIPTablesRequest,
        ) -> Result<agent::SetIPTablesResponse> {
            unimplemented!()
        }
        async fn get_volume_stats(
            &self,
            _: agent::VolumeStatsRequest,
        ) -> Result<agent::VolumeStatsResponse> {
            unimplemented!()
        }
        async fn resize_volume(&self, _: agent::ResizeVolumeRequest) -> Result<agent::Empty> {
            unimplemented!()
        }
        async fn get_guest_details(
            &self,
            _: agent::GetGuestDetailsRequest,
        ) -> Result<agent::GuestDetailsResponse> {
            unimplemented!()
        }
        async fn add_swap(&self, _: agent::AddSwapRequest) -> Result<agent::Empty> {
            unimplemented!()
        }
        async fn add_swap_path(&self, _: agent::AddSwapPathRequest) -> Result<agent::Empty> {
            unimplemented!()
        }
        async fn set_policy(&self, _: SetPolicyRequest) -> Result<agent::Empty> {
            unimplemented!()
        }
        async fn get_diagnostic_data(
            &self,
            _: agent::GetDiagnosticDataRequest,
        ) -> Result<agent::GetDiagnosticDataResponse> {
            unimplemented!()
        }
    }

    #[derive(Default)]
    struct FakeResource {
        failures: usize,
        blocks_first: bool,
        cleanups: AtomicUsize,
    }

    impl FakeResource {
        fn failing(failures: usize) -> Arc<Self> {
            Arc::new(Self {
                failures,
                ..Default::default()
            })
        }

        fn blocking_first() -> Arc<Self> {
            Arc::new(Self {
                blocks_first: true,
                ..Default::default()
            })
        }

        async fn cleanup(&self) -> Result<()> {
            let attempt = self.cleanups.fetch_add(1, Ordering::SeqCst);
            if self.blocks_first && attempt == 0 {
                std::future::pending::<()>().await;
            }
            if attempt < self.failures {
                return Err(anyhow!("injected cleanup failure"));
            }
            Ok(())
        }

        fn cleanups(&self) -> usize {
            self.cleanups.load(Ordering::SeqCst)
        }
    }

    #[async_trait]
    impl Rootfs for FakeResource {
        async fn get_guest_rootfs_path(&self) -> Result<String> {
            unimplemented!()
        }
        async fn get_rootfs_mount(&self) -> Result<Vec<oci::Mount>> {
            unimplemented!()
        }
        async fn get_storage(&self) -> Option<Vec<agent::Storage>> {
            unimplemented!()
        }
        async fn cleanup(&self, _: &RwLock<DeviceManager>) -> Result<()> {
            FakeResource::cleanup(self).await
        }
        async fn get_device_id(&self) -> Result<Option<String>> {
            unimplemented!()
        }
    }

    #[async_trait]
    impl Volume for FakeResource {
        fn get_volume_mount(&self) -> Result<Vec<oci::Mount>> {
            Ok(vec![])
        }
        fn get_storage(&self) -> Result<Vec<agent::Storage>> {
            unimplemented!()
        }
        fn get_device_id(&self) -> Result<Option<String>> {
            unimplemented!()
        }
        async fn cleanup(&self, _: &RwLock<DeviceManager>) -> Result<()> {
            FakeResource::cleanup(self).await
        }
    }

    const CID: &str = "container";

    async fn new_container(agent: Arc<FakeAgent>) -> (ContainerInner, ContainerProcess) {
        let process = ContainerProcess::new(CID, "").unwrap();
        let init_process = Process::new(&process, 0, "", None, None, None, false);
        init_process.set_status(ProcessStatus::Running).await;
        let logger = slog::Logger::root(slog::Discard, slog::o!());
        (
            ContainerInner::new(agent, init_process, logger, None),
            process,
        )
    }

    async fn new_device_manager() -> RwLock<DeviceManager> {
        RwLock::new(
            DeviceManager::new(Arc::new(Qemu::new()), None)
                .await
                .unwrap(),
        )
    }

    #[tokio::test]
    async fn failed_resource_cleanup_is_reported_and_retried() {
        let agent = Arc::new(FakeAgent::default());
        let (mut inner, process) = new_container(agent.clone()).await;
        let device_manager = new_device_manager().await;
        let volume = FakeResource::failing(0);
        let failing_volume = FakeResource::failing(1);
        let failing_rootfs = FakeResource::failing(1);
        inner.volumes = vec![volume.clone(), failing_volume.clone()];
        inner.rootfs = vec![failing_rootfs.clone()];

        let result = inner.stop_process(&process, true, &device_manager).await;
        assert!(result.is_err(), "resource cleanup failure was hidden");
        assert_eq!(
            inner.init_process.get_status().await,
            ProcessStatus::Stopped
        );

        inner
            .stop_process(&process, true, &device_manager)
            .await
            .unwrap();
        assert_eq!(failing_volume.cleanups(), 2);
        assert_eq!(failing_rootfs.cleanups(), 2);
        assert_eq!(volume.cleanups(), 1, "successful cleanup was repeated");
        assert_eq!(agent.removed.load(Ordering::SeqCst), 1);

        inner
            .stop_process(&process, true, &device_manager)
            .await
            .unwrap();
        assert_eq!(failing_volume.cleanups(), 2);
        assert_eq!(failing_rootfs.cleanups(), 2);
        assert_eq!(volume.cleanups(), 1);
    }

    #[tokio::test]
    async fn rootfs_cleanup_runs_when_volume_cleanup_fails() {
        let (mut inner, _) = new_container(Arc::default()).await;
        let device_manager = new_device_manager().await;
        let failing_volume = FakeResource::failing(usize::MAX);
        let rootfs = FakeResource::failing(0);
        inner.volumes = vec![failing_volume.clone()];
        inner.rootfs = vec![rootfs.clone()];

        assert!(inner
            .cleanup_container(CID, true, &device_manager)
            .await
            .is_err());
        assert_eq!(rootfs.cleanups(), 1);
    }

    #[tokio::test]
    async fn successful_cleanup_is_not_repeated() {
        let agent = Arc::new(FakeAgent::default());
        let (mut inner, _) = new_container(agent.clone()).await;
        let device_manager = new_device_manager().await;
        let volume = FakeResource::failing(0);
        let rootfs = FakeResource::failing(0);
        inner.volumes = vec![volume.clone()];
        inner.rootfs = vec![rootfs.clone()];

        for _ in 0..2 {
            inner
                .cleanup_container(CID, true, &device_manager)
                .await
                .unwrap();
        }
        assert_eq!(volume.cleanups(), 1);
        assert_eq!(rootfs.cleanups(), 1);
        assert_eq!(agent.removed.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn cancelled_cleanup_keeps_unfinished_resources() {
        let (mut inner, _) = new_container(Arc::default()).await;
        let device_manager = new_device_manager().await;
        let blocked_volume = FakeResource::blocking_first();
        let volume = FakeResource::failing(0);
        let blocked_rootfs = FakeResource::blocking_first();
        let rootfs = FakeResource::failing(0);
        inner.volumes = vec![blocked_volume.clone(), volume.clone()];
        inner.rootfs = vec![blocked_rootfs.clone(), rootfs.clone()];

        let cancel_after = std::time::Duration::from_millis(50);
        assert!(
            tokio::time::timeout(cancel_after, inner.clean_volumes(&device_manager))
                .await
                .is_err()
        );
        assert!(
            tokio::time::timeout(cancel_after, inner.clean_rootfs(&device_manager))
                .await
                .is_err()
        );
        assert_eq!(inner.volumes.len(), 2, "cancellation dropped volumes");
        assert_eq!(inner.rootfs.len(), 2, "cancellation dropped rootfs");

        inner
            .cleanup_container(CID, true, &device_manager)
            .await
            .unwrap();
        assert!(inner.volumes.is_empty());
        assert!(inner.rootfs.is_empty());
        assert_eq!(blocked_volume.cleanups(), 2);
        assert_eq!(volume.cleanups(), 1);
        assert_eq!(blocked_rootfs.cleanups(), 2);
        assert_eq!(rootfs.cleanups(), 1);
    }
}
