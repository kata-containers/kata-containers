// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use agent::Agent;
use anyhow::{anyhow, Context, Result};
use common::{
    error::Error,
    types::{ContainerID, ContainerProcess, ProcessExitStatus, ProcessStatus, ProcessType},
};
use hypervisor::device::device_manager::DeviceManager;
use nix::sys::signal::Signal;
use oci::LinuxResources;
use oci_spec::runtime as oci;
use resource::{rootfs::Rootfs, volume::Volume};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::RwLock;

use crate::container_manager::logger_with_process;

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
        }
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

        // close the exit channel to wakeup wait service
        // send to notify watchers who are waiting for the process exit
        self.init_process.stop().await;

        self.clean_volumes(device_manager)
            .await
            .context("clean volumes")?;
        self.clean_rootfs(device_manager)
            .await
            .context("clean rootfs")?;

        Ok(())
    }

    pub(crate) async fn stop_process(
        &mut self,
        process: &ContainerProcess,
        force: bool,
        device_manager: &RwLock<DeviceManager>,
    ) -> Result<()> {
        let logger = logger_with_process(process);
        info!(logger, "begin to stop process");

        // do not stop again when state stopped, may cause multi cleanup resource
        let state = self.init_process.get_status().await;
        if state == ProcessStatus::Stopped {
            return Ok(());
        }

        self.check_state(vec![ProcessStatus::Running])
            .await
            .context("check state")?;

        // if use force mode to stop container, stop always successful
        // send kill signal to container
        // ignore the error of sending signal, since the process would
        // have been killed and exited yet.
        self.signal_process(process, Signal::SIGKILL as u32, false)
            .await
            .map_err(|e| {
                warn!(logger, "failed to signal kill. {:?}", e);
            })
            .ok();

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
            .await?;

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

    async fn clean_volumes(&mut self, device_manager: &RwLock<DeviceManager>) -> Result<()> {
        let mut unhandled = Vec::new();
        for v in self.volumes.iter() {
            if let Err(err) = v.cleanup(device_manager).await {
                unhandled.push(Arc::clone(v));
                warn!(
                    sl!(),
                    "Failed to clean the volume = {:?}, error = {:?}",
                    v.get_volume_mount(),
                    err
                );
            }
        }
        if !unhandled.is_empty() {
            self.volumes = unhandled;
        }
        Ok(())
    }

    async fn clean_rootfs(&mut self, device_manager: &RwLock<DeviceManager>) -> Result<()> {
        let mut unhandled = Vec::new();
        for rootfs in self.rootfs.iter() {
            if let Err(err) = rootfs.cleanup(device_manager).await {
                unhandled.push(Arc::clone(rootfs));
                warn!(
                    sl!(),
                    "Failed to umount rootfs, cid = {:?}, error = {:?}",
                    self.container_id(),
                    err
                );
            }
        }
        if !unhandled.is_empty() {
            self.rootfs = unhandled;
        }
        Ok(())
    }
}
