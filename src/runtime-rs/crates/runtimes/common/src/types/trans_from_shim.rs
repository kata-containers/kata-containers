// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use super::{
    ContainerConfig, ContainerID, ContainerProcess, ExecProcessRequest, KillRequest,
    ResizePTYRequest, SandboxConfig, SandboxID, SandboxNetworkEnv, SandboxRequest,
    SandboxStatusRequest, ShutdownRequest, StopSandboxRequest, TaskRequest, UpdateRequest,
    DEFAULT_SHM_SIZE,
};

use kata_types::{annotations::cri_containerd, mount::Mount};
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

fn update_resource_annotation(config: &mut cri_api_v1::PodSandboxConfig) {
    if let Some(resources) = config
        .linux
        .as_ref()
        .and_then(|linux| linux.resources.as_ref())
    {
        for (key, value) in [
            (cri_containerd::SANDBOX_CPU_QUOTA_KEY, resources.cpu_quota),
            (cri_containerd::SANDBOX_CPU_PERIOD_KEY, resources.cpu_period),
            (cri_containerd::SANDBOX_CPU_SHARE_KEY, resources.cpu_shares),
            (
                cri_containerd::SANDBOX_MEM_KEY,
                resources.memory_limit_in_bytes,
            ),
        ] {
            config
                .annotations
                .insert(key.to_string(), value.to_string());
        }
    }
}

fn get_cgroup_parent(config: &cri_api_v1::PodSandboxConfig) -> Option<String> {
    config
        .linux
        .as_ref()
        .map(|linux| linux.cgroup_parent.clone())
        .filter(|parent| !parent.is_empty())
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

        let mut config = cri_api_v1::PodSandboxConfig::parse_from_bytes(&from.options.value)?;
        update_resource_annotation(&mut config);
        let cgroup_parent = get_cgroup_parent(&config);

        let mut dns: Vec<String> = vec![];
        config.dns_config.map(|mut dns_config| {
            dns.append(&mut dns_config.servers);
            dns.append(&mut dns_config.servers);
            dns.append(&mut dns_config.options);
        });

        Ok(SandboxRequest::CreateSandbox(Box::new(SandboxConfig {
            sandbox_id: from.sandbox_id.clone(),
            hostname: config.hostname,
            dns,
            network_env: SandboxNetworkEnv {
                netns: Some(from.netns_path),
                network_created: false,
            },
            cgroup_parent,
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn convert_sandbox_config(config: cri_api_v1::PodSandboxConfig) -> SandboxConfig {
        let request = sandbox_api::CreateSandboxRequest {
            sandbox_id: "sandbox-id".to_string(),
            options: protobuf::MessageField::some(protobuf::well_known_types::any::Any {
                type_url: SANDBOX_API_V1.to_string(),
                value: config.write_to_bytes().unwrap(),
                ..Default::default()
            }),
            ..Default::default()
        };
        match SandboxRequest::try_from(request).unwrap() {
            SandboxRequest::CreateSandbox(config) => *config,
            _ => panic!("expected create sandbox request"),
        }
    }

    #[test]
    fn test_sandbox_linux_resources_override_annotations() {
        let config = cri_api_v1::PodSandboxConfig {
            linux: protobuf::MessageField::some(cri_api_v1::LinuxPodSandboxConfig {
                cgroup_parent: "/kubepods/pod-id".to_string(),
                resources: protobuf::MessageField::some(cri_api_v1::LinuxContainerResources {
                    cpu_quota: 200_000,
                    cpu_period: 100_000,
                    memory_limit_in_bytes: 536_870_912,
                    ..Default::default()
                }),
                ..Default::default()
            }),
            annotations: HashMap::from([
                (
                    cri_containerd::SANDBOX_CPU_QUOTA_KEY.to_string(),
                    "1".to_string(),
                ),
                (
                    cri_containerd::SANDBOX_CPU_SHARE_KEY.to_string(),
                    "1024".to_string(),
                ),
                ("custom".to_string(), "preserved".to_string()),
            ]),
            ..Default::default()
        };

        let sandbox = convert_sandbox_config(config);
        assert_eq!(sandbox.cgroup_parent.as_deref(), Some("/kubepods/pod-id"));
        assert_eq!(
            sandbox.annotations[cri_containerd::SANDBOX_CPU_QUOTA_KEY],
            "200000"
        );
        assert_eq!(
            sandbox.annotations[cri_containerd::SANDBOX_CPU_PERIOD_KEY],
            "100000"
        );
        assert_eq!(
            sandbox.annotations[cri_containerd::SANDBOX_CPU_SHARE_KEY],
            "0"
        );
        assert_eq!(
            sandbox.annotations[cri_containerd::SANDBOX_MEM_KEY],
            "536870912"
        );
        assert_eq!(sandbox.annotations["custom"], "preserved");
        assert_eq!(sandbox.state.annotations, sandbox.annotations);
    }

    #[test]
    fn test_sandbox_without_linux_resources_preserves_annotations() {
        for linux in [None, Some(cri_api_v1::LinuxPodSandboxConfig::default())] {
            let annotations = HashMap::from([
                (
                    cri_containerd::SANDBOX_CPU_QUOTA_KEY.to_string(),
                    "-1".to_string(),
                ),
                (
                    cri_containerd::SANDBOX_MEM_KEY.to_string(),
                    "536870912".to_string(),
                ),
            ]);
            let sandbox = convert_sandbox_config(cri_api_v1::PodSandboxConfig {
                linux: linux.into(),
                annotations: annotations.clone(),
                ..Default::default()
            });

            assert_eq!(sandbox.cgroup_parent, None);
            assert_eq!(sandbox.annotations, annotations);
            assert_eq!(sandbox.state.annotations, annotations);
        }
    }
}
