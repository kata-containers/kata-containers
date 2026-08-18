// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use super::{
    CheckpointSandboxRequest, ContainerConfig, ContainerID, ContainerProcess, ExecProcessRequest,
    KillRequest, ResizePTYRequest, RestoreSandboxRequest, SandboxCheckpointTask, SandboxConfig,
    SandboxID, SandboxNetworkEnv, SandboxRequest, SandboxRestoreTask, SandboxStatusRequest,
    ShutdownRequest, StopSandboxRequest, TaskRequest, UpdateRequest, DEFAULT_SHM_SIZE,
};

use kata_types::mount::Mount;
use std::{
    convert::{From, TryFrom},
    path::PathBuf,
};

use protobuf::Message;
use runtime_spec;

use protocols::api as cri_api_v1;

use anyhow::{anyhow, Context, Result};
use containerd_shim_protos::{api, sandbox_api};

pub const SANDBOX_API_V1: &str = "runtime.v1.PodSandboxConfig";

fn resolv_conf_lines(
    servers: Vec<String>,
    searches: Vec<String>,
    options: Vec<String>,
) -> Vec<String> {
    let mut lines = servers
        .into_iter()
        .filter(|server| !server.is_empty())
        .map(|server| format!("nameserver {server}"))
        .collect::<Vec<_>>();
    if !searches.is_empty() {
        lines.push(format!("search {}", searches.join(" ")));
    }
    if !options.is_empty() {
        lines.push(format!("options {}", options.join(" ")));
    }
    lines
}

fn trans_from_shim_mount(from: &api::Mount) -> Mount {
    let options = from.options.to_vec();
    let mut read_only = false;
    for o in &options {
        if o == "ro" {
            read_only = true;
            break;
        }
    }

    Mount {
        source: from.source.clone(),
        destination: PathBuf::from(&from.target),
        fs_type: from.type_.clone(),
        options,
        device_id: None,
        host_shared_fs_path: None,
        read_only,
    }
}

// There're a lot of information to create a sandbox from CreateSandboxRequest and the internal PodSandboxConfig.
// At present, we only take out part of it to build SandboxConfig.
impl TryFrom<sandbox_api::CreateSandboxRequest> for SandboxRequest {
    type Error = anyhow::Error;
    fn try_from(from: sandbox_api::CreateSandboxRequest) -> Result<Self> {
        let type_url = from.options.type_url.clone();
        if type_url != SANDBOX_API_V1 {
            return Err(anyhow!(format!("unsupported type url: {}", type_url)));
        };

        let config = cri_api_v1::PodSandboxConfig::parse_from_bytes(&from.options.value)?;

        let dns = config
            .dns_config
            .map(|dns_config| {
                resolv_conf_lines(dns_config.servers, dns_config.searches, dns_config.options)
            })
            .unwrap_or_default();

        Ok(SandboxRequest::CreateSandbox(Box::new(SandboxConfig {
            sandbox_id: from.sandbox_id.clone(),
            hostname: config.hostname,
            dns,
            network_env: SandboxNetworkEnv {
                netns: Some(from.netns_path),
                network_created: false,
            },
            annotations: config.annotations.clone(),
            hooks: None,
            state: runtime_spec::State {
                version: Default::default(),
                id: from.sandbox_id,
                status: runtime_spec::ContainerState::Creating,
                pid: 0,
                bundle: from.bundle_path,
                annotations: config.annotations,
            },
            shm_size: DEFAULT_SHM_SIZE,
        })))
    }
}

impl TryFrom<sandbox_api::StartSandboxRequest> for SandboxRequest {
    type Error = anyhow::Error;
    fn try_from(from: sandbox_api::StartSandboxRequest) -> Result<Self> {
        Ok(SandboxRequest::StartSandbox(SandboxID {
            sandbox_id: from.sandbox_id,
        }))
    }
}

impl TryFrom<sandbox_api::PlatformRequest> for SandboxRequest {
    type Error = anyhow::Error;
    fn try_from(from: sandbox_api::PlatformRequest) -> Result<Self> {
        Ok(SandboxRequest::Platform(SandboxID {
            sandbox_id: from.sandbox_id,
        }))
    }
}

impl TryFrom<sandbox_api::StopSandboxRequest> for SandboxRequest {
    type Error = anyhow::Error;
    fn try_from(from: sandbox_api::StopSandboxRequest) -> Result<Self> {
        Ok(SandboxRequest::StopSandbox(StopSandboxRequest {
            sandbox_id: from.sandbox_id,
            timeout_secs: from.timeout_secs,
        }))
    }
}

impl TryFrom<sandbox_api::WaitSandboxRequest> for SandboxRequest {
    type Error = anyhow::Error;
    fn try_from(from: sandbox_api::WaitSandboxRequest) -> Result<Self> {
        Ok(SandboxRequest::WaitSandbox(SandboxID {
            sandbox_id: from.sandbox_id,
        }))
    }
}

impl TryFrom<sandbox_api::SandboxStatusRequest> for SandboxRequest {
    type Error = anyhow::Error;
    fn try_from(from: sandbox_api::SandboxStatusRequest) -> Result<Self> {
        Ok(SandboxRequest::SandboxStatus(SandboxStatusRequest {
            sandbox_id: from.sandbox_id,
            verbose: from.verbose,
        }))
    }
}

impl TryFrom<sandbox_api::PingRequest> for SandboxRequest {
    type Error = anyhow::Error;
    fn try_from(from: sandbox_api::PingRequest) -> Result<Self> {
        Ok(SandboxRequest::Ping(SandboxID {
            sandbox_id: from.sandbox_id,
        }))
    }
}

impl TryFrom<sandbox_api::ShutdownSandboxRequest> for SandboxRequest {
    type Error = anyhow::Error;
    fn try_from(from: sandbox_api::ShutdownSandboxRequest) -> Result<Self> {
        Ok(SandboxRequest::ShutdownSandbox(SandboxID {
            sandbox_id: from.sandbox_id,
        }))
    }
}

impl TryFrom<sandbox_api::CheckpointSandboxRequest> for SandboxRequest {
    type Error = anyhow::Error;

    fn try_from(from: sandbox_api::CheckpointSandboxRequest) -> Result<Self> {
        Ok(SandboxRequest::CheckpointSandbox(
            CheckpointSandboxRequest {
                sandbox_id: from.sandbox_id,
                output_path: from.output_path,
                tasks: from
                    .tasks
                    .into_iter()
                    .map(|task| SandboxCheckpointTask {
                        checkpoint_key: task.checkpoint_key,
                        task_id: task.task_id,
                        rootfs_device_id: None,
                    })
                    .collect(),
                options: from.options,
                operation_timeout: None,
            },
        ))
    }
}

impl TryFrom<sandbox_api::RestoreSandboxRequest> for SandboxRequest {
    type Error = anyhow::Error;

    fn try_from(from: sandbox_api::RestoreSandboxRequest) -> Result<Self> {
        let type_url = from.sandbox_options.type_url.clone();
        if type_url != SANDBOX_API_V1 {
            return Err(anyhow!("unsupported type url: {type_url}"));
        }
        let config = cri_api_v1::PodSandboxConfig::parse_from_bytes(&from.sandbox_options.value)?;
        let dns = config
            .dns_config
            .into_option()
            .map(|dns_config| {
                resolv_conf_lines(dns_config.servers, dns_config.searches, dns_config.options)
            })
            .unwrap_or_default();
        let sandbox_id = from.sandbox_id;
        let sandbox_config = SandboxConfig {
            sandbox_id: sandbox_id.clone(),
            hostname: config.hostname,
            dns,
            network_env: SandboxNetworkEnv {
                netns: (!from.netns_path.is_empty()).then_some(from.netns_path),
                network_created: false,
            },
            annotations: config.annotations.clone(),
            hooks: None,
            state: runtime_spec::State {
                version: Default::default(),
                id: sandbox_id,
                status: runtime_spec::ContainerState::Creating,
                pid: 0,
                bundle: from.bundle_path,
                annotations: config.annotations,
            },
            shm_size: DEFAULT_SHM_SIZE,
        };
        let tasks = from
            .tasks
            .into_iter()
            .map(|task| SandboxRestoreTask {
                checkpoint_key: task.checkpoint_key,
                task_id: task.task_id,
                bundle: task.bundle,
                terminal: task.terminal,
                stdin: (!task.stdin.is_empty()).then_some(task.stdin),
                stdout: (!task.stdout.is_empty()).then_some(task.stdout),
                stderr: (!task.stderr.is_empty()).then_some(task.stderr),
                options_type_url: task.options.type_url.clone(),
                options: task.options.value.to_vec(),
            })
            .collect();
        Ok(SandboxRequest::RestoreSandbox(Box::new(
            RestoreSandboxRequest {
                sandbox_config,
                checkpoint_path: from.checkpoint_path,
                options: from.options,
                tasks,
                operation_timeout: None,
            },
        )))
    }
}

impl TryFrom<api::CreateTaskRequest> for TaskRequest {
    type Error = anyhow::Error;
    fn try_from(from: api::CreateTaskRequest) -> Result<Self> {
        let options = if from.has_options() {
            Some(from.options().value.to_vec())
        } else {
            None
        };
        Ok(TaskRequest::CreateContainer(ContainerConfig {
            container_id: from.id.clone(),
            bundle: from.bundle.clone(),
            rootfs_mounts: from.rootfs.iter().map(trans_from_shim_mount).collect(),
            terminal: from.terminal,
            options,
            stdin: (!from.stdin.is_empty()).then(|| from.stdin.clone()),
            stdout: (!from.stdout.is_empty()).then(|| from.stdout.clone()),
            stderr: (!from.stderr.is_empty()).then(|| from.stderr.clone()),
        }))
    }
}

impl TryFrom<api::CloseIORequest> for TaskRequest {
    type Error = anyhow::Error;
    fn try_from(from: api::CloseIORequest) -> Result<Self> {
        Ok(TaskRequest::CloseProcessIO(
            ContainerProcess::new(&from.id, &from.exec_id).context("new process id")?,
        ))
    }
}

impl TryFrom<api::DeleteRequest> for TaskRequest {
    type Error = anyhow::Error;
    fn try_from(from: api::DeleteRequest) -> Result<Self> {
        Ok(TaskRequest::DeleteProcess(
            ContainerProcess::new(&from.id, &from.exec_id).context("new process id")?,
        ))
    }
}

impl TryFrom<api::ExecProcessRequest> for TaskRequest {
    type Error = anyhow::Error;
    fn try_from(from: api::ExecProcessRequest) -> Result<Self> {
        let spec = from.spec();
        Ok(TaskRequest::ExecProcess(ExecProcessRequest {
            process: ContainerProcess::new(&from.id, &from.exec_id).context("new process id")?,
            terminal: from.terminal,
            stdin: (!from.stdin.is_empty()).then(|| from.stdin.clone()),
            stdout: (!from.stdout.is_empty()).then(|| from.stdout.clone()),
            stderr: (!from.stderr.is_empty()).then(|| from.stderr.clone()),
            spec_type_url: spec.type_url.to_string(),
            spec_value: spec.value.to_vec(),
        }))
    }
}

impl TryFrom<api::KillRequest> for TaskRequest {
    type Error = anyhow::Error;
    fn try_from(from: api::KillRequest) -> Result<Self> {
        Ok(TaskRequest::KillProcess(KillRequest {
            process: ContainerProcess::new(&from.id, &from.exec_id).context("new process id")?,
            signal: from.signal,
            all: from.all,
        }))
    }
}

impl TryFrom<api::WaitRequest> for TaskRequest {
    type Error = anyhow::Error;
    fn try_from(from: api::WaitRequest) -> Result<Self> {
        Ok(TaskRequest::WaitProcess(
            ContainerProcess::new(&from.id, &from.exec_id).context("new process id")?,
        ))
    }
}

impl TryFrom<api::StartRequest> for TaskRequest {
    type Error = anyhow::Error;
    fn try_from(from: api::StartRequest) -> Result<Self> {
        Ok(TaskRequest::StartProcess(
            ContainerProcess::new(&from.id, &from.exec_id).context("new process id")?,
        ))
    }
}

impl TryFrom<api::StateRequest> for TaskRequest {
    type Error = anyhow::Error;
    fn try_from(from: api::StateRequest) -> Result<Self> {
        Ok(TaskRequest::StateProcess(
            ContainerProcess::new(&from.id, &from.exec_id).context("new process id")?,
        ))
    }
}

impl TryFrom<api::ShutdownRequest> for TaskRequest {
    type Error = anyhow::Error;
    fn try_from(from: api::ShutdownRequest) -> Result<Self> {
        Ok(TaskRequest::ShutdownContainer(ShutdownRequest {
            container_id: from.id.to_string(),
            is_now: from.now,
        }))
    }
}

impl TryFrom<api::ResizePtyRequest> for TaskRequest {
    type Error = anyhow::Error;
    fn try_from(from: api::ResizePtyRequest) -> Result<Self> {
        Ok(TaskRequest::ResizeProcessPTY(ResizePTYRequest {
            process: ContainerProcess::new(&from.id, &from.exec_id).context("new process id")?,
            width: from.width,
            height: from.height,
        }))
    }
}

impl TryFrom<api::PauseRequest> for TaskRequest {
    type Error = anyhow::Error;
    fn try_from(from: api::PauseRequest) -> Result<Self> {
        Ok(TaskRequest::PauseContainer(ContainerID::new(&from.id)?))
    }
}

impl TryFrom<api::ResumeRequest> for TaskRequest {
    type Error = anyhow::Error;
    fn try_from(from: api::ResumeRequest) -> Result<Self> {
        Ok(TaskRequest::ResumeContainer(ContainerID::new(&from.id)?))
    }
}

impl TryFrom<api::StatsRequest> for TaskRequest {
    type Error = anyhow::Error;
    fn try_from(from: api::StatsRequest) -> Result<Self> {
        Ok(TaskRequest::StatsContainer(ContainerID::new(&from.id)?))
    }
}

impl TryFrom<api::UpdateTaskRequest> for TaskRequest {
    type Error = anyhow::Error;
    fn try_from(from: api::UpdateTaskRequest) -> Result<Self> {
        Ok(TaskRequest::UpdateContainer(UpdateRequest {
            container_id: from.id.to_string(),
            value: from.resources().value.to_vec(),
        }))
    }
}

impl TryFrom<api::PidsRequest> for TaskRequest {
    type Error = anyhow::Error;
    fn try_from(_from: api::PidsRequest) -> Result<Self> {
        Ok(TaskRequest::Pid)
    }
}

impl TryFrom<api::ConnectRequest> for TaskRequest {
    type Error = anyhow::Error;
    fn try_from(from: api::ConnectRequest) -> Result<Self> {
        Ok(TaskRequest::ConnectContainer(ContainerID::new(&from.id)?))
    }
}
