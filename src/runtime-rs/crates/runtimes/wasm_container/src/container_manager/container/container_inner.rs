// Copyright (c) 2022-2023 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//
use super::process::{ProcessWatcher, WasmProcess};
use super::rootfs::handle_rootfs;
use crate::sandbox::is_signal_handled;
use crate::CONTAINER_BASE;
use agent::types::{CgroupStats, StatsContainerResponse};
use common::{
    error::Error,
    types::{
        ContainerConfig, ContainerID, ContainerProcess, ExecProcessRequest, ProcessStateInfo,
        ProcessStatus, ProcessType, StatsInfo,
    },
};

use oci::{LinuxResources, Process as OCIProcess};
use rustjail::{
    container as InnerContainer, container::BaseContainer, container::SYSTEMD_CGROUP_PATH_FORMAT,
    process as InnerProcess, process::ProcessOperations,
};

use anyhow::{anyhow, Context, Result};
use cgroups::freezer::FreezerState;
use libc::{c_ushort, winsize, TIOCSWINSZ};
use nix::sys::signal::Signal;
use nix::{errno::Errno, libc::pid_t};
use slog::Logger;
use std::{collections::HashMap, path::PathBuf};

const ANNOTATIONS_WASM: &str = "io.katacontainers.platform.wasi/wasm32";

pub struct ContainerInner {
    config: ContainerConfig,
    runner: InnerContainer::HybridContainer,
    pub init_process: WasmProcess,
    pub exec_processes: HashMap<String, WasmProcess>,
    logger: Logger,
}

impl ContainerInner {
    pub fn new(
        mut config: ContainerConfig,
        mut spec: oci::Spec,
        logger: slog::Logger,
    ) -> Result<Self> {
        let container_id = config.container_id.clone();

        // ensure rootfs in bundle_path/rootfs
        // mount corresponding rootfs_mounts
        let bundle = PathBuf::from(config.bundle.clone()).canonicalize()?;
        config.bundle = bundle.as_path().display().to_string();
        handle_rootfs(
            &bundle,
            spec.root.as_mut().ok_or(anyhow!("no root"))?,
            &config.rootfs_mounts,
        )?;

        // determine which cgroup driver to take and then assign to use_systemd_cgroup
        // systemd: "[slice]:[prefix]:[name]"
        // fs: "/path_a/path_b"
        let cgroups_path = spec.linux.as_ref().map_or("", |linux| &linux.cgroups_path);
        let use_systemd_cgroup = SYSTEMD_CGROUP_PATH_FORMAT.is_match(cgroups_path);

        // determine whether to use wasm runtime to execute the process, which
        // requires the feature wasm-runtime to be enabled and corresponding
        // ANNOTATIONS_WASM to be provided.
        // We must support linux containers in wasm_container, because at least
        // infra container based on registry.k8s.io/pause:3.6 for a wasm pod is a linux container.
        let mut wasm_runtime = false;
        if let Some(value) = spec.annotations.get(ANNOTATIONS_WASM) {
            wasm_runtime = value.eq_ignore_ascii_case("yes") || value.eq_ignore_ascii_case("true");
        };

        let inner_config = InnerContainer::Config {
            cgroup_name: "".to_string(),
            use_systemd_cgroup,
            no_pivot_root: false,
            no_new_keyring: false,
            spec: Some(spec),
            rootless_euid: false,
            rootless_cgroup: false,
            wasm_runtime,
        };
        let runner = InnerContainer::HybridContainer::new(
            container_id.as_str(),
            CONTAINER_BASE.as_str(),
            inner_config,
            &sl!(),
        )?;

        let init_process = WasmProcess::new(
            ContainerProcess::new(container_id.as_str(), container_id.as_str())?,
            None,
            0,
            &config.bundle,
            config.stdin.clone(),
            config.stdout.clone(),
            config.stderr.clone(),
            config.terminal,
        );

        Ok(Self {
            config,
            runner,
            init_process,
            exec_processes: HashMap::new(),
            logger: logger.new(o!("subsystem" => "wasm_container_inner")),
        })
    }

    pub async fn create(&mut self) -> Result<()> {
        // chdir before start runner container
        let current_dir = std::env::current_dir()?;
        nix::unistd::chdir(&PathBuf::from(self.config.bundle.clone()))?;
        scopeguard::defer! {
            nix::unistd::chdir(&current_dir).unwrap();
        }

        // runner process start
        let oci_process = self
            .runner
            .config
            .spec
            .as_ref()
            .map(|spec| spec.process.as_ref().unwrap())
            .unwrap();
        let inner_process =
            InnerProcess::Process::new(&sl!(), oci_process, &self.runner.id, true, 0)?;
        self.runner.start(inner_process).await?;

        // update init_process pid
        self.init_process.pid = self.runner.init_process_pid;

        Ok(())
    }

    pub async fn start(&mut self) -> Result<()> {
        self.check_state(vec![ProcessStatus::Created, ProcessStatus::Stopped])
            .await
            .context("check state")?;

        self.runner.exec().await?;

        self.set_state(ProcessStatus::Running).await;

        Ok(())
    }

    pub async fn stats(&self) -> Result<StatsInfo> {
        let cgroup_stats: CgroupStats = self.runner.cgroup_manager.get_stats()?.into();
        let stats = StatsContainerResponse {
            cgroup_stats: Some(cgroup_stats),
            ..Default::default()
        };

        Ok(StatsInfo::from(Some(stats)))
    }

    pub async fn pause(&mut self) -> Result<()> {
        let state = self.init_process.get_status().await;
        if state == ProcessStatus::Paused {
            return Ok(());
        }

        self.runner.cgroup_manager.freeze(FreezerState::Frozen)?;

        self.init_process.set_status(ProcessStatus::Paused).await;

        Ok(())
    }

    pub async fn resume(&mut self) -> Result<()> {
        let state = self.init_process.get_status().await;
        if state != ProcessStatus::Paused {
            return Ok(());
        }

        self.runner.cgroup_manager.freeze(FreezerState::Thawed)?;

        self.init_process.set_status(ProcessStatus::Running).await;

        Ok(())
    }

    pub async fn update(&mut self, resources: LinuxResources) -> Result<()> {
        self.runner.set(resources)
    }

    pub async fn create_exec_process(&mut self, req: ExecProcessRequest) -> Result<()> {
        let process = req.process;
        let exec_id = process.exec_id.clone();

        let oci_process: OCIProcess =
            serde_json::from_slice(&req.spec_value).context("serde from slice")?;

        let inner_process = InnerProcess::Process::new(&sl!(), &oci_process, &exec_id, false, 0)?;

        let exec_process = WasmProcess::new(
            process,
            Some(inner_process),
            0,
            &self.init_process.bundle,
            req.stdin,
            req.stdout,
            req.stderr,
            req.terminal,
        );

        self.exec_processes
            .insert(exec_id.to_string(), exec_process);

        Ok(())
    }

    pub async fn start_exec_process(&mut self, process: &ContainerProcess) -> Result<()> {
        let mut exec_process = self
            .exec_processes
            .get_mut(&process.exec_id)
            .ok_or_else(|| Error::ProcessNotFound(process.clone()))?;

        self.runner
            .start(
                exec_process
                    .inner_process
                    .take()
                    .ok_or(anyhow!("no inner process"))?,
            )
            .await?;

        let exec_id = process.exec_id.clone();
        for (pid, inner_p) in self.runner.processes.iter_mut() {
            if exec_id == inner_p.exec_id.as_str() {
                exec_process.pid = *pid;
            }
        }

        exec_process.set_status(ProcessStatus::Running).await;

        Ok(())
    }

    pub async fn delete_exec_process(&mut self, process: &ContainerProcess) -> Result<()> {
        self.exec_processes
            .remove(process.exec_id())
            .ok_or(anyhow!("no such process"))?;

        Ok(())
    }

    pub async fn stop_process(&mut self, process: &ContainerProcess) -> Result<()> {
        // do not stop again when state stopped, may cause multi cleanup resource
        let state = self.init_process.get_status().await;

        if state == ProcessStatus::Stopped {
            return Ok(());
        }

        self.check_state(vec![ProcessStatus::Running, ProcessStatus::Exited])
            .await
            .context("check state")?;

        if state == ProcessStatus::Running {
            self.signal_process(process, Signal::SIGKILL as u32, false)
                .await
                .map_err(|e| {
                    warn!(self.logger, "failed to signal kill. {:?}", e);
                })
                .ok();
        }

        match process.process_type {
            ProcessType::Container => {
                self.runner.destroy().await?;
            }
            ProcessType::Exec => {
                let exec_process = self
                    .exec_processes
                    .get_mut(&process.exec_id)
                    .ok_or_else(|| Error::ProcessNotFound(process.clone()))?;

                exec_process.stop().await;
            }
        }

        Ok(())
    }

    pub async fn close_io(&mut self, process: &ContainerProcess) -> Result<()> {
        match process.process_type {
            ProcessType::Container => {
                self.init_process.wg_input.wait().await;
            }
            ProcessType::Exec => {
                let exec_process = self
                    .exec_processes
                    .get_mut(&process.exec_id)
                    .ok_or_else(|| Error::ProcessNotFound(process.clone()))?;

                exec_process.wg_input.wait().await;
            }
        };

        let p = self.get_inner_process(process)?;
        p.close_stdin();

        Ok(())
    }

    pub async fn signal_process(
        &mut self,
        process: &ContainerProcess,
        signal: u32,
        all: bool,
    ) -> Result<()> {
        let mut sig: libc::c_int = signal as libc::c_int;
        {
            let p = self.get_inner_process(process)?;
            // For container initProcess, if it hasn't installed handler for "SIGTERM" signal,
            // it will ignore the "SIGTERM" signal sent to it, thus send it "SIGKILL" signal
            // instead of "SIGTERM" to terminate it.
            let proc_status_file = format!("/proc/{}/status", p.pid);
            if process.process_type == ProcessType::Container
                && sig == libc::SIGTERM
                && !is_signal_handled(&proc_status_file, sig as u32)
            {
                sig = libc::SIGKILL;
            }

            match p.signal(sig) {
                Err(Errno::ESRCH) => {
                    info!(
                        self.logger,
                        "signal encounter ESRCH, continue";
                        "signal" => sig,
                    );
                }
                Err(err) => return Err(anyhow!(err)),
                Ok(()) => (),
            }
        };

        if process.exec_id.is_empty() || all {
            // eid is empty, signal all the remaining processes in the container cgroup
            info!(
                self.logger,
                "signal all the remaining processes";
            );

            if let Err(err) = self.runner.cgroup_manager.freeze(FreezerState::Frozen) {
                warn!(
                    self.logger,
                    "freeze cgroup failed";
                    "error" => format!("{:?}", err),
                );
            }

            let pids = self.runner.cgroup_manager.get_pids()?;
            for pid in pids.iter() {
                let res = unsafe { libc::kill(*pid, sig) };
                if let Err(err) = Errno::result(res).map(drop) {
                    warn!(
                        self.logger,
                        "signal failed";
                        "error" => format!("{:?}", err),
                    );
                }
            }

            if let Err(err) = self.runner.cgroup_manager.freeze(FreezerState::Thawed) {
                warn!(
                    self.logger,
                    "unfreeze cgroup failed";
                    "error" => format!("{:?}", err),
                );
            }
        }

        Ok(())
    }

    pub async fn win_resize_process(
        &mut self,
        process: &ContainerProcess,
        height: u32,
        width: u32,
    ) -> Result<()> {
        self.check_state(vec![ProcessStatus::Created, ProcessStatus::Running])
            .await
            .context("check state")?;

        let p = self.get_inner_process(process)?;

        if let Some(fd) = p.term_master {
            unsafe {
                let win = winsize {
                    ws_row: height as c_ushort,
                    ws_col: width as c_ushort,
                    ws_xpixel: 0,
                    ws_ypixel: 0,
                };

                let err = libc::ioctl(fd, TIOCSWINSZ, &win);
                Errno::result(err)
                    .map(drop)
                    .map_err(|e| anyhow!(format!("ioctl error: {:?}", e)))?;
            }
        } else {
            return Err(anyhow!(format!("no tty")));
        }

        Ok(())
    }

    pub fn find_process_by_pid(
        &self,
        container_id: &ContainerID,
        pid: pid_t,
    ) -> Option<ContainerProcess> {
        if self.runner.init_process_pid == pid {
            Some(ContainerProcess {
                container_id: container_id.clone(),
                exec_id: container_id.container_id.clone(),
                process_type: ProcessType::Container,
            })
        } else {
            self.runner.processes.get(&pid).map(|p| ContainerProcess {
                container_id: container_id.clone(),
                exec_id: p.exec_id.clone(),
                process_type: ProcessType::Exec,
            })
        }
    }

    pub fn get_inner_process(
        &mut self,
        process: &ContainerProcess,
    ) -> Result<&mut InnerProcess::Process> {
        let exec_id = match process.process_type {
            ProcessType::Container => self.runner.id.clone(),
            ProcessType::Exec => process.exec_id.clone(),
        };

        self.runner.get_process(&exec_id)
    }

    pub async fn update_exited_process(
        &mut self,
        process: &ContainerProcess,
        exit_code: i32,
    ) -> Result<()> {
        let wasm_process = match process.process_type {
            ProcessType::Container => &mut self.init_process,
            ProcessType::Exec => self
                .exec_processes
                .get_mut(process.exec_id())
                .ok_or(anyhow!("no such process"))?,
        };

        wasm_process.update_exited_status(exit_code).await
    }

    pub async fn state_process(&self, process: &ContainerProcess) -> Result<ProcessStateInfo> {
        let wasm_process = match process.process_type {
            ProcessType::Container => &self.init_process,
            ProcessType::Exec => self
                .exec_processes
                .get(process.exec_id())
                .ok_or(anyhow!("no such process"))?,
        };

        wasm_process.state().await
    }

    pub fn fetch_exit_watcher(&self, process: &ContainerProcess) -> Result<ProcessWatcher> {
        let wasm_process = match process.process_type {
            ProcessType::Container => &self.init_process,
            ProcessType::Exec => self
                .exec_processes
                .get(process.exec_id())
                .ok_or(anyhow!("no such process"))?,
        };

        wasm_process.fetch_exit_watcher()
    }

    async fn check_state(&self, states: Vec<ProcessStatus>) -> Result<()> {
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

    async fn set_state(&mut self, state: ProcessStatus) {
        let mut status = self.init_process.status.write().await;
        *status = state;
    }
}
