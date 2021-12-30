// Copyright (c) 2021 Alibaba Cloud
// Copyright (c) 2021 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use agent_client::{Agent, StatsContainerResponse};
use nix::sys::signal::Signal;
use oci_spec::runtime::{LinuxResources as ociLinuxResources, Process as ociProcess};
use std::collections::HashMap;
use std::path::Path;
use std::sync::{mpsc::SyncSender, Arc, Mutex, RwLock};

use crate::config::TomlConfig;
use crate::container;
use crate::{Container, Error, Result};

// TODO: just a placeholder here. Details will be released later.
pub struct Sandbox {
    pub containers: HashMap<String, Arc<Mutex<Container>>>,
    pub agent: Arc<dyn Agent>,
}

// TODO: just a placeholder here. Details will be released later.
impl Sandbox {
    pub fn new(_id: &str, _toml_config: TomlConfig, _bundle_path: &Path) -> Result<Sandbox> {
        Err(Error::NotImplemented)
    }

    pub fn start(&mut self) -> Result<()> {
        Err(Error::NotImplemented)
    }

    pub fn cleanup_container(_id: &str) -> Result<()> {
        Ok(())
    }

    pub fn get_state(&self) -> container::State {
        container::State::Ready
    }

    pub fn get_agent(&self) -> Arc<dyn Agent> {
        self.agent.clone()
    }

    pub fn find_container(&mut self, _id: &str) -> Result<Arc<Mutex<Container>>> {
        Err(Error::NotImplemented)
    }

    pub fn stats_container(&mut self, _id: &str) -> Result<StatsContainerResponse> {
        Err(Error::NotImplemented)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn create_container(
        &mut self,
        _container_id: &str,
        _stdin: Option<String>,
        _stdout: Option<String>,
        _stderr: Option<String>,
        _terminal: bool,
        _bundle_path: &str,
        _rootfs: Vec<container::Mount>,
    ) -> Result<()> {
        Err(Error::NotImplemented)
    }

    pub fn pause_container(&mut self, _id: &str) -> Result<()> {
        Err(Error::NotImplemented)
    }

    pub fn resume_container(&mut self, _id: &str) -> Result<()> {
        Err(Error::NotImplemented)
    }

    pub fn signal_process(
        &mut self,
        _container_id: &str,
        _process_id: &str,
        _signal: Signal,
        _all: bool,
    ) -> Result<()> {
        Err(Error::NotImplemented)
    }

    pub fn fetch_exit_channel(
        &mut self,
        _container_id: &str,
        _process_id: &str,
    ) -> Result<(SyncSender<()>, Arc<RwLock<container::ExitStatus>>)> {
        Err(Error::NotImplemented)
    }

    pub fn close_io(&mut self, _container_id: &str, _process_id: &str) -> Result<()> {
        Err(Error::NotImplemented)
    }

    pub fn winsize_process(
        &mut self,
        _container_id: &str,
        _process_id: &str,
        _height: u32,
        _width: u32,
    ) -> Result<()> {
        Err(Error::NotImplemented)
    }

    pub fn update_container(
        &mut self,
        _container_id: &str,
        _resources: &ociLinuxResources,
    ) -> Result<()> {
        Err(Error::NotImplemented)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn create_exec(
        &mut self,
        _container_id: &str,
        _exec_id: &str,
        _stdin: Option<String>,
        _stdout: Option<String>,
        _stderr: Option<String>,
        _terminal: bool,
        _oci_process: ociProcess,
    ) -> Result<()> {
        Err(Error::NotImplemented)
    }

    pub fn stop_container(
        &mut self,
        _container_id: &str,
        _exec_id: &str,
        _force: bool,
    ) -> Result<()> {
        Err(Error::NotImplemented)
    }

    pub fn remove_container(&mut self, _container_id: &str, _exec_id: &str) -> Result<()> {
        Err(Error::NotImplemented)
    }

    pub fn start_container(&mut self, _container_id: &str, _exec_id: &str) -> Result<()> {
        Err(Error::NotImplemented)
    }

    pub fn try_stop_and_delete(&mut self) {}
}
