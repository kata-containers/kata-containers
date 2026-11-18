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
    container::Container,
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
    pub(crate) devices: Vec<String>, // device ids of devices hotplugged to the container
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
            devices: vec![],
            linux_resources,
            removed_from_agent: false,
        }
    }

    pub(crate) fn has_pending_cleanup(&self) -> bool {
        !self.devices.is_empty() || !self.volumes.is_empty() || !self.rootfs.is_empty()
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

    pub(crate) async fn start_exec(
        &mut self,
        containers: Arc<RwLock<HashMap<String, Container>>>,
        process: &ContainerProcess,
        passfd_listener_addr: Option<&(String, u32)>,
    ) -> Result<()> {
        // A rejected replay must not reconnect IO or kill the existing process.
        self.exec_processes
            .get(&process.exec_id)
            .ok_or_else(|| Error::ProcessNotFound(process.clone()))?
            .process
            .check_start()
            .await?;

        let result: Result<()> = async {
            // In passfd io mode, we create vsock connections for io in advance
            // and pass port info to agent in `ExecProcessRequest`.
            // These vsock connections will be used as stdin/stdout/stderr of the exec process.
            // See agent/src/passfd_io.rs for more details.
            if let Some((hvsock_uds_path, passfd_port)) = passfd_listener_addr {
                let exec = self
                    .exec_processes
                    .get_mut(&process.exec_id)
                    .ok_or_else(|| Error::ProcessNotFound(process.clone()))?;
                exec.process
                    .passfd_io_init(hvsock_uds_path, *passfd_port)
                    .await?;
            }

            self.start_exec_process(process)
                .await
                .context("enter process")?;

            {
                let exec = self
                    .exec_processes
                    .get(&process.exec_id)
                    .ok_or_else(|| Error::ProcessNotFound(process.clone()))?;
                if exec.process.height != 0 && exec.process.width != 0 {
                    self.win_resize_process(process, exec.process.height, exec.process.width)
                        .await
                        .context("win resize")?;
                }
            }

            let exec = self
                .exec_processes
                .get_mut(&process.exec_id)
                .ok_or_else(|| Error::ProcessNotFound(process.clone()))?;
            if exec.process.passfd_io.is_some() {
                // The agent waits for passfd IO to finish before reporting exit.
                exec.process
                    .passfd_io_wait(containers, self.agent.clone())
                    .await?;
            } else {
                // Legacy IO must be drained by the shim before waiting on the agent.
                let container_io = ContainerIo::new(self.agent.clone(), process.clone());
                exec.process
                    .start_io_and_wait(containers, self.agent.clone(), container_io)
                    .await
                    .context("start io and wait")?;
            }
            Ok(())
        }
        .await;

        if result.is_err() {
            let cleanup_succeeded = match self
                .signal_process(process, Signal::SIGKILL as u32, false)
                .await
            {
                Ok(()) => true,
                Err(err) if is_no_such_process_error(&err) => true,
                Err(err) => {
                    warn!(
                        self.logger,
                        "failed to stop exec after start failure: {:?}", err
                    );
                    false
                }
            };
            let exec = self
                .exec_processes
                .get_mut(&process.exec_id)
                .ok_or_else(|| Error::ProcessNotFound(process.clone()))?;
            // Keep a started guest killable when cleanup could not terminate it.
            let status = if !cleanup_succeeded
                && exec.process.get_status().await == ProcessStatus::Running
            {
                ProcessStatus::Running
            } else {
                ProcessStatus::Stopped
            };
            exec.process.fail_start(status).await;
        }
        result
    }

    async fn start_exec_process(&mut self, process: &ContainerProcess) -> Result<()> {
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
        self.check_state(vec![ProcessStatus::Created])
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
            };
            match self.agent.remove_container(remove_request).await {
                Ok(_) => self.removed_from_agent = true,
                // Still pending, so a retry for pending host resources removes it again.
                Err(e) if force => warn!(
                    self.logger,
                    "stop container: agent remove container failed: {}", e
                ),
                Err(e) => return Err(e),
            }
        }

        // close the exit channel to wakeup wait service
        // send to notify watchers who are waiting for the process exit
        self.init_process.stop().await;

        let devices = self
            .clean_devices(device_manager)
            .await
            .context("clean devices");
        let volumes = self
            .clean_volumes(device_manager)
            .await
            .context("clean volumes");
        let rootfs = self
            .clean_rootfs(device_manager)
            .await
            .context("clean rootfs");

        let errors: Vec<String> = vec![devices, volumes, rootfs]
            .into_iter()
            .filter_map(|result| result.err().map(|err| format!("{err:#}")))
            .collect();
        if errors.is_empty() {
            Ok(())
        } else {
            Err(anyhow!(errors.join("; ")))
        }
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

        self.check_state(vec![ProcessStatus::Created, ProcessStatus::Running])
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

    // Remove an entry only after its cleanup succeeds, here and in clean_rootfs.
    // Moving the list out first (drain, take) would drop the entries not yet
    // cleaned if this future is cancelled at an await. The index advances only
    // past failed entries, so it also counts them.
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

    async fn clean_devices(&mut self, device_manager: &RwLock<DeviceManager>) -> Result<()> {
        // Remove each entry immediately after successful cleanup.
        let mut failed = 0;
        let mut first_err = None;
        while failed < self.devices.len() {
            let device_id = self.devices[failed].clone();
            match device_manager
                .write()
                .await
                .try_remove_device(&device_id)
                .await
            {
                Ok(()) => {
                    self.devices.remove(failed);
                }
                Err(err) => {
                    warn!(
                        self.logger,
                        "failed to hotunplug container device {}: {:?}", device_id, err
                    );
                    first_err.get_or_insert(err);
                    failed += 1;
                }
            }
        }

        match first_err {
            Some(err) => Err(err.context(format!(
                "failed to hotunplug {} container device(s)",
                failed
            ))),
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

    #[derive(Clone, Copy, Debug)]
    enum SignalFailure {
        ConnectionClosed,
        Io(i32),
        NotFound,
    }

    #[derive(Default)]
    struct FakeAgent {
        removed: AtomicUsize,
        remove_failures: usize,
        exec_calls: AtomicUsize,
        exec_failures: usize,
        reject_duplicate: bool,
        wait_calls: AtomicUsize,
        wait_blocked: bool,
        wait_release: tokio::sync::Notify,
        signals: std::sync::Mutex<Vec<(String, u32)>>,
        signal_failure: Option<SignalFailure>,
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
            if self.removed.fetch_add(1, Ordering::SeqCst) < self.remove_failures {
                return Err(anyhow!("injected remove container failure"));
            }
            Ok(agent::Empty::new())
        }
        async fn signal_process(&self, req: agent::SignalProcessRequest) -> Result<agent::Empty> {
            let mut signals = self.signals.lock().unwrap();
            signals.push((req.process_id.exec_id, req.signal));
            // Only the cleanup attempt fails; a subsequent Kill can succeed.
            if signals.len() == 1 {
                if let Some(failure) = self.signal_failure {
                    return Err(match failure {
                        SignalFailure::ConnectionClosed => Error::AgentConnectionClosed.into(),
                        SignalFailure::Io(errno) => std::io::Error::from_raw_os_error(errno).into(),
                        SignalFailure::NotFound => Error::ProcessAlreadyTerminated.into(),
                    });
                }
            }
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
            let attempt = self.exec_calls.fetch_add(1, Ordering::SeqCst);
            if attempt < self.exec_failures {
                return Err(anyhow!("missing executable"));
            }
            if self.reject_duplicate && attempt > 0 {
                return Err(anyhow!("exec_id 'test-exec' already exists"));
            }
            Ok(agent::Empty::new())
        }
        async fn wait_process(
            &self,
            _: agent::WaitProcessRequest,
        ) -> Result<agent::WaitProcessResponse> {
            self.wait_calls.fetch_add(1, Ordering::SeqCst);
            if self.wait_blocked {
                self.wait_release.notified().await;
            }
            Ok(agent::WaitProcessResponse { status: 42 })
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
            Err(anyhow!("injected resize failure"))
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
        async fn get_device_ids(&self) -> Result<Vec<String>> {
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

        let err = inner
            .stop_process(&process, true, &device_manager)
            .await
            .expect_err("resource cleanup failure was hidden");
        let err = format!("{err:#}");
        assert!(
            err.contains("clean volumes"),
            "volume error missing: {}",
            err
        );
        assert!(
            err.contains("clean rootfs"),
            "rootfs error missing: {}",
            err
        );
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
    async fn resource_cleanup_runs_when_device_cleanup_fails() {
        let (mut inner, _) = new_container(Arc::default()).await;
        let device_manager = new_device_manager().await;
        let volume = FakeResource::failing(0);
        let rootfs = FakeResource::failing(0);
        inner.devices = vec!["missing-device".to_string()];
        inner.volumes = vec![volume.clone()];
        inner.rootfs = vec![rootfs.clone()];

        let err = inner
            .cleanup_container(CID, true, &device_manager)
            .await
            .expect_err("missing device cleanup failure was hidden");

        assert!(format!("{err:#}").contains("clean devices"));
        assert_eq!(volume.cleanups(), 1);
        assert_eq!(rootfs.cleanups(), 1);
        assert_eq!(inner.devices, ["missing-device"]);
        assert!(inner.has_pending_cleanup());
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
    async fn forced_agent_removal_failure_is_retried() {
        let agent = Arc::new(FakeAgent {
            remove_failures: 1,
            ..Default::default()
        });
        let (mut inner, _) = new_container(agent.clone()).await;
        let device_manager = new_device_manager().await;
        let volume = FakeResource::failing(1);
        inner.volumes = vec![volume.clone()];

        assert!(inner
            .cleanup_container(CID, true, &device_manager)
            .await
            .is_err());
        assert_eq!(agent.removed.load(Ordering::SeqCst), 1);

        inner
            .cleanup_container(CID, true, &device_manager)
            .await
            .unwrap();
        assert_eq!(agent.removed.load(Ordering::SeqCst), 2);
        assert_eq!(volume.cleanups(), 2);

        inner
            .cleanup_container(CID, true, &device_manager)
            .await
            .unwrap();
        assert_eq!(agent.removed.load(Ordering::SeqCst), 2);
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

    fn add_test_exec(inner: &mut ContainerInner) -> ContainerProcess {
        let process = ContainerProcess::new(CID, "test-exec").unwrap();
        inner.add_exec_process(
            &process.exec_id,
            Exec {
                process: Process::new(&process, 0, "", None, None, None, false),
                oci_process: oci::Process::default(),
            },
        );
        process
    }

    async fn wait_for_exit(
        inner: &ContainerInner,
        process: &ContainerProcess,
    ) -> ProcessExitStatus {
        let (watcher, status) = inner.fetch_exit_watcher(process).unwrap();
        let mut watcher = watcher.unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(1), async {
            while watcher.changed().await.is_ok() {}
        })
        .await
        .expect("Wait did not finish");
        let exit = status.read().await.clone();
        exit
    }

    #[tokio::test]
    async fn repeated_exec_start_preserves_running_process() {
        for passfd in [false, true] {
            let agent = Arc::new(FakeAgent {
                reject_duplicate: true,
                wait_blocked: true,
                ..Default::default()
            });
            let (mut inner, _) = new_container(agent.clone()).await;
            let process = add_test_exec(&mut inner);
            let containers = Arc::new(RwLock::new(HashMap::new()));
            let addr = ("unused-without-stdio".to_string(), 1);
            let addr = passfd.then_some(&addr);
            inner
                .start_exec(containers.clone(), &process, addr)
                .await
                .unwrap();

            // If a replay reaches IO setup, this missing FIFO makes it fail.
            inner
                .exec_processes
                .get_mut(&process.exec_id)
                .unwrap()
                .process
                .stdout = Some("/nonexistent-kata-start-test/stdout".to_string());
            for _ in 0..2 {
                let err = inner
                    .start_exec(containers.clone(), &process, addr)
                    .await
                    .unwrap_err();
                assert!(err.to_string().contains("cannot start process"));
            }
            assert_eq!(agent.exec_calls.load(Ordering::SeqCst), 1);
            assert!(agent.signals.lock().unwrap().is_empty());
            assert_eq!(
                inner.exec_processes[&process.exec_id]
                    .process
                    .get_status()
                    .await,
                ProcessStatus::Running
            );
            let (watcher, status) = inner.fetch_exit_watcher(&process).unwrap();
            assert!(watcher.unwrap().has_changed().is_ok(), "replay closed Wait");
            assert!(status.read().await.exit_time.is_none());

            agent.wait_release.notify_one();
            assert_eq!(wait_for_exit(&inner, &process).await.exit_code, 42);
        }
    }

    #[tokio::test]
    async fn failed_exec_start_completes_wait_and_rejects_retry() {
        for passfd in [false, true] {
            // Cover both a persistent failure and a guest that would accept a retry.
            for failures in [1, usize::MAX] {
                let agent = Arc::new(FakeAgent {
                    exec_failures: failures,
                    ..Default::default()
                });
                let (mut inner, _) = new_container(agent.clone()).await;
                let process = add_test_exec(&mut inner);
                let containers = Arc::new(RwLock::new(HashMap::new()));
                let addr = ("unused-without-stdio".to_string(), 1);
                let addr = passfd.then_some(&addr);
                let (watcher, _) = inner.fetch_exit_watcher(&process).unwrap();
                let mut watcher = watcher.unwrap();
                let err = inner
                    .start_exec(containers.clone(), &process, addr)
                    .await
                    .unwrap_err();
                assert!(format!("{err:#}").contains("missing executable"));
                assert!(
                    tokio::time::timeout(std::time::Duration::from_secs(1), watcher.changed())
                        .await
                        .expect("existing Wait was not notified")
                        .is_err()
                );
                let exit = wait_for_exit(&inner, &process).await;
                assert_eq!(exit.exit_code, crate::UNKNOWN_EXIT_STATUS);
                assert!(exit.exit_time.is_some());

                for _ in 0..2 {
                    assert!(inner
                        .start_exec(containers.clone(), &process, addr)
                        .await
                        .is_err());
                }
                assert_eq!(agent.exec_calls.load(Ordering::SeqCst), 1);
                assert_eq!(agent.signals.lock().unwrap().len(), 1);
                assert_eq!(agent.wait_calls.load(Ordering::SeqCst), 0);
                assert_eq!(
                    inner.exec_processes[&process.exec_id]
                        .process
                        .get_status()
                        .await,
                    ProcessStatus::Stopped
                );
                assert_eq!(
                    wait_for_exit(&inner, &process).await.exit_time,
                    exit.exit_time
                );
            }
        }
    }

    #[tokio::test]
    async fn exec_io_start_failure_completes_wait() {
        for passfd in [false, true] {
            let agent = Arc::new(FakeAgent::default());
            let (mut inner, _) = new_container(agent.clone()).await;
            let process = add_test_exec(&mut inner);
            inner
                .exec_processes
                .get_mut(&process.exec_id)
                .unwrap()
                .process
                .stdout = Some("/nonexistent-kata-start-test/stdout".to_string());
            let containers = Arc::new(RwLock::new(HashMap::new()));
            let addr = ("unused".to_string(), 1);
            let addr = passfd.then_some(&addr);
            assert!(inner
                .start_exec(containers.clone(), &process, addr)
                .await
                .is_err());
            assert_eq!(
                wait_for_exit(&inner, &process).await.exit_code,
                crate::UNKNOWN_EXIT_STATUS
            );
            // Passfd setup fails before the agent call, legacy IO setup after it.
            assert_eq!(
                agent.exec_calls.load(Ordering::SeqCst),
                usize::from(!passfd)
            );
            assert_eq!(agent.signals.lock().unwrap().len(), 1);
            assert!(inner.start_exec(containers, &process, addr).await.is_err());
            assert_eq!(agent.signals.lock().unwrap().len(), 1);
        }
    }

    #[tokio::test]
    async fn failed_start_preserves_live_exec_when_cleanup_signal_fails() {
        for passfd in [false, true] {
            for guest_started in [false, true] {
                for (failure, process_missing) in [
                    (SignalFailure::ConnectionClosed, false),
                    (SignalFailure::Io(libc::EIO), false),
                    (SignalFailure::Io(libc::ESRCH), true),
                    (SignalFailure::NotFound, true),
                ] {
                    let agent = Arc::new(FakeAgent {
                        exec_failures: usize::from(!guest_started),
                        signal_failure: Some(failure),
                        ..Default::default()
                    });
                    let (mut inner, _) = new_container(agent.clone()).await;
                    let process = add_test_exec(&mut inner);
                    let exec = &mut inner
                        .exec_processes
                        .get_mut(&process.exec_id)
                        .unwrap()
                        .process;
                    if passfd {
                        // Fail after ExecProcess succeeds, without needing a real vsock.
                        exec.height = 1;
                        exec.width = 1;
                    } else {
                        exec.stdout = Some("/nonexistent-kata-start-test/stdout".to_string());
                    }
                    let addr = ("unused-without-stdio".to_string(), 1);
                    let addr = passfd.then_some(&addr);
                    let (watcher, _) = inner.fetch_exit_watcher(&process).unwrap();
                    let mut watcher = watcher.unwrap();
                    assert!(inner
                        .start_exec(Arc::default(), &process, addr)
                        .await
                        .is_err());
                    let state = inner.exec_processes[&process.exec_id]
                        .process
                        .state()
                        .await
                        .unwrap();
                    let expected = if guest_started && !process_missing {
                        ProcessStatus::Running
                    } else {
                        ProcessStatus::Stopped
                    };
                    assert_eq!(
                        state.status, expected,
                        "passfd={passfd}, guest_started={guest_started}, failure={failure:?}"
                    );
                    tokio::time::timeout(std::time::Duration::from_secs(1), watcher.changed())
                        .await
                        .expect("existing Wait did not finish")
                        .unwrap_err();
                    let exit = wait_for_exit(&inner, &process).await;
                    assert_eq!(exit.exit_code, crate::UNKNOWN_EXIT_STATUS);
                    assert!(exit.exit_time.is_some());

                    // Reject Start without repeating either the agent call or cleanup.
                    assert!(inner
                        .start_exec(Arc::default(), &process, addr)
                        .await
                        .is_err());
                    assert_eq!(agent.exec_calls.load(Ordering::SeqCst), 1);
                    assert_eq!(agent.signals.lock().unwrap().len(), 1);
                    assert_eq!(agent.wait_calls.load(Ordering::SeqCst), 0);

                    if expected == ProcessStatus::Running {
                        // Kill's STOPPED fast path must not suppress this retry.
                        inner
                            .signal_process(&process, Signal::SIGKILL as u32, false)
                            .await
                            .unwrap();
                        assert_eq!(agent.signals.lock().unwrap().len(), 2);
                    }
                    assert_eq!(
                        wait_for_exit(&inner, &process).await.exit_time,
                        exit.exit_time
                    );
                }
            }
        }
    }

    #[tokio::test]
    async fn exited_exec_cannot_restart_or_replace_exit_status() {
        for passfd in [false, true] {
            let agent = Arc::new(FakeAgent::default());
            let (mut inner, _) = new_container(agent.clone()).await;
            let process = add_test_exec(&mut inner);
            let containers = Arc::new(RwLock::new(HashMap::new()));
            let addr = ("unused-without-stdio".to_string(), 1);
            let addr = passfd.then_some(&addr);
            inner
                .start_exec(containers.clone(), &process, addr)
                .await
                .unwrap();
            let exit = wait_for_exit(&inner, &process).await;
            assert_eq!(exit.exit_code, 42);

            assert!(inner.start_exec(containers, &process, addr).await.is_err());
            assert_eq!(agent.exec_calls.load(Ordering::SeqCst), 1);
            assert!(agent.signals.lock().unwrap().is_empty());
            assert_eq!(
                inner.exec_processes[&process.exec_id]
                    .process
                    .get_status()
                    .await,
                ProcessStatus::Stopped
            );
            let after = wait_for_exit(&inner, &process).await;
            assert_eq!(after.exit_code, exit.exit_code);
            assert_eq!(after.exit_time, exit.exit_time);
        }
    }

    #[tokio::test]
    async fn exec_start_rejects_non_created_states() {
        for status in [
            ProcessStatus::Unknown,
            ProcessStatus::Running,
            ProcessStatus::Stopped,
            ProcessStatus::Paused,
            ProcessStatus::Pausing,
        ] {
            let agent = Arc::new(FakeAgent::default());
            let (mut inner, _) = new_container(agent.clone()).await;
            let process = add_test_exec(&mut inner);
            inner.exec_processes[&process.exec_id]
                .process
                .set_status(status)
                .await;
            assert!(inner
                .start_exec(Arc::default(), &process, None)
                .await
                .is_err());
            assert_eq!(agent.exec_calls.load(Ordering::SeqCst), 0);
            assert!(agent.signals.lock().unwrap().is_empty());
            assert_eq!(
                inner.exec_processes[&process.exec_id]
                    .process
                    .get_status()
                    .await,
                status
            );
        }
    }

    #[tokio::test]
    async fn killed_created_container_cannot_start_with_an_existing_waiter() {
        let agent = Arc::new(FakeAgent {
            wait_blocked: true,
            ..Default::default()
        });
        let (mut inner, process) = new_container(agent.clone()).await;
        inner.init_process.set_status(ProcessStatus::Created).await;
        // Killing a Created container installs this waiter before its exit is known.
        inner
            .init_process
            .passfd_io_wait(Arc::default(), agent.clone())
            .await
            .unwrap();
        assert!(inner.init_process.check_start().await.is_err());
        assert!(inner
            .init_process
            .exit_status
            .read()
            .await
            .exit_time
            .is_none());
        agent.wait_release.notify_one();
        assert_eq!(wait_for_exit(&inner, &process).await.exit_code, 42);
    }
}
