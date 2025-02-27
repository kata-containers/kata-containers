// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

mod trans_from_agent;
mod trans_from_shim;
mod trans_into_agent;
mod trans_into_shim;
pub mod utils;

use std::{
    collections::{hash_map::RandomState, HashMap},
    fmt,
};

use crate::SandboxNetworkEnv;

use anyhow::{Context, Result};
use kata_sys_util::validate;
use kata_types::mount::Mount;
use oci_spec::runtime as oci;
use strum::Display;

/// TaskRequest: TaskRequest from shim
/// TaskRequest and TaskResponse messages need to be paired
#[derive(Debug, Clone, Display)]
pub enum TaskRequest {
    CreateContainer(ContainerConfig),
    CloseProcessIO(ContainerProcess),
    DeleteProcess(ContainerProcess),
    ExecProcess(ExecProcessRequest),
    KillProcess(KillRequest),
    WaitProcess(ContainerProcess),
    StartProcess(ContainerProcess),
    StateProcess(ContainerProcess),
    ShutdownContainer(ShutdownRequest),
    PauseContainer(ContainerID),
    ResumeContainer(ContainerID),
    ResizeProcessPTY(ResizePTYRequest),
    StatsContainer(ContainerID),
    UpdateContainer(UpdateRequest),
    Pid,
    ConnectContainer(ContainerID),
}

/// TaskResponse: TaskResponse to shim
/// TaskRequest and TaskResponse messages need to be paired
#[derive(Debug, Clone, Display)]
pub enum TaskResponse {
    CreateContainer(PID),
    CloseProcessIO,
    DeleteProcess(ProcessStateInfo),
    ExecProcess,
    KillProcess,
    WaitProcess(ProcessExitStatus),
    StartProcess(PID),
    StateProcess(ProcessStateInfo),
    ShutdownContainer,
    PauseContainer,
    ResumeContainer,
    ResizeProcessPTY,
    StatsContainer(StatsInfo),
    UpdateContainer,
    Pid(PID),
    ConnectContainer(PID),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ProcessType {
    Container,
    Exec,
}

#[derive(Clone, Debug)]
pub struct ContainerID {
    pub container_id: String,
}

impl std::fmt::Display for ContainerID {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.container_id)
    }
}

impl ContainerID {
    pub fn new(container_id: &str) -> Result<Self> {
        validate::verify_id(container_id).context("verify container id")?;
        Ok(Self {
            container_id: container_id.to_string(),
        })
    }
}

#[derive(Clone, Debug)]
pub struct ContainerProcess {
    pub container_id: ContainerID,
    pub exec_id: String,
    pub process_type: ProcessType,
}

impl fmt::Display for ContainerProcess {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", &self)
    }
}

impl ContainerProcess {
    pub fn new(container_id: &str, exec_id: &str) -> Result<Self> {
        let (exec_id, process_type) = if exec_id.is_empty() || container_id == exec_id {
            ("".to_string(), ProcessType::Container)
        } else {
            validate::verify_id(exec_id).context("verify exec id")?;
            (exec_id.to_string(), ProcessType::Exec)
        };
        Ok(Self {
            container_id: ContainerID::new(container_id)?,
            exec_id,
            process_type,
        })
    }

    pub fn container_id(&self) -> &str {
        &self.container_id.container_id
    }

    pub fn exec_id(&self) -> &str {
        &self.exec_id
    }
}
#[derive(Debug, Clone)]
pub struct ContainerConfig {
    pub container_id: String,
    pub bundle: String,
    pub rootfs_mounts: Vec<Mount>,
    pub terminal: bool,
    pub options: Option<Vec<u8>>,
    pub stdin: Option<String>,
    pub stdout: Option<String>,
    pub stderr: Option<String>,
}

#[derive(Debug, Clone, Display)]
pub enum SandboxRequest {
    CreateSandbox(Box<SandboxConfig>),
    StartSandbox(SandboxID),
    Platform(SandboxID),
    StopSandbox(StopSandboxRequest),
    WaitSandbox(SandboxID),
    SandboxStatus(SandboxStatusRequest),
    Ping(SandboxID),
    ShutdownSandbox(SandboxID),
}

/// Response: sandbox response to shim
/// Request and Response messages need to be paired
#[derive(Debug, Clone, Display)]
pub enum SandboxResponse {
    CreateSandbox,
    StartSandbox(StartSandboxInfo),
    Platform(PlatformInfo),
    StopSandbox,
    WaitSandbox(SandboxExitInfo),
    SandboxStatus(SandboxStatusInfo),
    Ping,
    ShutdownSandbox,
}

#[derive(Clone, Debug)]
pub struct SandboxConfig {
    pub sandbox_id: String,
    pub hostname: String,
    pub dns: Vec<String>,
    pub network_env: SandboxNetworkEnv,
    pub annotations: HashMap<String, String, RandomState>,
    pub hooks: Option<oci::Hooks>,
    pub state: runtime_spec::State,
}

#[derive(Clone, Debug)]
pub struct SandboxID {
    pub sandbox_id: String,
}

#[derive(Clone, Debug)]
pub struct StartSandboxInfo {
    pub pid: u32,
    pub create_time: Option<std::time::SystemTime>,
}

#[derive(Clone, Debug)]
pub struct PlatformInfo {
    pub os: String,
    pub architecture: String,
}

#[derive(Clone, Debug)]
pub struct StopSandboxRequest {
    pub sandbox_id: String,
    pub timeout_secs: u32,
}

#[derive(Clone, Debug, Default)]
pub struct SandboxExitInfo {
    pub exit_status: u32,
    pub exited_at: Option<std::time::SystemTime>,
}

#[derive(Clone, Debug)]
pub struct SandboxStatusRequest {
    pub sandbox_id: String,
    pub verbose: bool,
}

#[derive(Clone, Debug)]
pub struct SandboxStatusInfo {
    pub sandbox_id: String,
    pub pid: u32,
    pub state: String,
    pub created_at: Option<std::time::SystemTime>,
    pub exited_at: Option<std::time::SystemTime>,
}

#[derive(Default, Clone, Debug)]
pub struct SandboxStatus {
    pub sandbox_id: String,
    pub pid: u32,
    pub state: String,
    pub info: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct PID {
    pub pid: u32,
}

impl PID {
    pub fn new(pid: u32) -> Self {
        Self { pid }
    }
}

#[derive(Debug, Clone)]
pub struct KillRequest {
    pub process: ContainerProcess,
    pub signal: u32,
    pub all: bool,
}

#[derive(Debug, Clone)]
pub struct ShutdownRequest {
    pub container_id: String,
    pub is_now: bool,
}

#[derive(Debug, Clone)]
pub struct ResizePTYRequest {
    pub process: ContainerProcess,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone)]
pub struct ExecProcessRequest {
    pub process: ContainerProcess,
    pub terminal: bool,
    pub stdin: Option<String>,
    pub stdout: Option<String>,
    pub stderr: Option<String>,
    pub spec_type_url: String,
    pub spec_value: Vec<u8>,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ProcessStatus {
    Unknown = 0,
    Created = 1,
    Running = 2,
    Stopped = 3,
    Paused = 4,
    Pausing = 5,
}

#[derive(Debug, Clone)]
pub struct ProcessStateInfo {
    pub container_id: String,
    pub exec_id: String,
    pub pid: PID,
    pub bundle: String,
    pub stdin: Option<String>,
    pub stdout: Option<String>,
    pub stderr: Option<String>,
    pub terminal: bool,
    pub status: ProcessStatus,
    pub exit_status: i32,
    pub exited_at: Option<std::time::SystemTime>,
}

#[derive(Debug, Clone, Default)]
pub struct ProcessExitStatus {
    pub exit_code: i32,
    pub exit_time: Option<std::time::SystemTime>,
}

impl ProcessExitStatus {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update_exit_code(&mut self, exit_code: i32) {
        self.exit_code = exit_code;
        self.exit_time = Some(std::time::SystemTime::now());
    }
}

#[derive(Debug, Clone)]
pub struct StatsInfoValue {
    pub type_url: String,
    pub value: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct StatsInfo {
    pub value: Option<StatsInfoValue>,
}

#[derive(Debug, Clone)]
pub struct UpdateRequest {
    pub container_id: String,
    pub value: Vec<u8>,
}
