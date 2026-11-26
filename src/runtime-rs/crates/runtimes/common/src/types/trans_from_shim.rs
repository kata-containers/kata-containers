// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use super::{
    ContainerConfig, ContainerID, ContainerProcess,
    CreateSandboxRequest as InternalCreateSandboxRequest, ExecProcessRequest, KillRequest,
    ResizePTYRequest, SandboxConfig, SandboxID, SandboxNetworkEnv, SandboxRequest,
    SandboxStatusRequest, ShutdownRequest, StopSandboxRequest, TaskRequest, UpdateRequest,
    DEFAULT_SHM_SIZE,
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

fn dns_config_to_resolv_conf(dns_config: cri_api_v1::DNSConfig) -> Vec<String> {
    let mut dns = dns_config
        .servers
        .into_iter()
        .map(|server| format!("nameserver {server}"))
        .collect::<Vec<_>>();

    if !dns_config.searches.is_empty() {
        dns.push(format!("search {}", dns_config.searches.join(" ")));
    }

    if !dns_config.options.is_empty() {
        dns.push(format!("options {}", dns_config.options.join(" ")));
    }

    dns
}

// Build the sandbox from PodSandboxConfig when options are provided. Direct
// Sandbox API callers may omit options and use the request fields instead.
impl TryFrom<sandbox_api::CreateSandboxRequest> for SandboxRequest {
    type Error = anyhow::Error;
    fn try_from(from: sandbox_api::CreateSandboxRequest) -> Result<Self> {
        let (hostname, dns, annotations) = if from.has_options() {
            let type_url = from.options.type_url.clone();
            if type_url != SANDBOX_API_V1 {
                return Err(anyhow!(format!("unsupported type url: {}", type_url)));
            }

            let config = cri_api_v1::PodSandboxConfig::parse_from_bytes(&from.options.value)?;
            let dns = config
                .dns_config
                .into_option()
                .map(dns_config_to_resolv_conf)
                .unwrap_or_default();
            (config.hostname, dns, config.annotations)
        } else {
            (
                from.sandbox_id.clone(),
                Vec::new(),
                from.annotations.clone(),
            )
        };

        let prepare_restore = None;
        let sandbox_config = SandboxConfig {
            sandbox_id: from.sandbox_id.clone(),
            hostname,
            dns,
            network_env: SandboxNetworkEnv {
                netns: Some(from.netns_path),
                network_created: false,
            },
            annotations: annotations.clone(),
            hooks: None,
            state: runtime_spec::State {
                version: Default::default(),
                id: from.sandbox_id,
                status: runtime_spec::ContainerState::Creating,
                pid: 0,
                bundle: from.bundle_path,
                annotations,
            },
            shm_size: DEFAULT_SHM_SIZE,
            config_path: None,
        };

        Ok(SandboxRequest::CreateSandbox(Box::new(
            InternalCreateSandboxRequest {
                sandbox_config,
                prepare_restore,
            },
        )))
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

    fn convert_create_sandbox_dns(dns_config: Option<cri_api_v1::DNSConfig>) -> Vec<String> {
        let mut config = cri_api_v1::PodSandboxConfig::new();
        config.dns_config = dns_config.into();

        let mut options = protobuf::well_known_types::any::Any::new();
        options.type_url = SANDBOX_API_V1.into();
        options.value = config.write_to_bytes().unwrap();

        let mut request = sandbox_api::CreateSandboxRequest::new();
        request.options = protobuf::MessageField::some(options);

        let request = SandboxRequest::try_from(request).unwrap();
        let SandboxRequest::CreateSandbox(config) = request else {
            panic!("expected create sandbox request");
        };

        config.sandbox_config.dns
    }

    #[test]
    fn create_sandbox_formats_dns_config_as_resolv_conf() {
        let mut dns_config = cri_api_v1::DNSConfig::new();
        dns_config.servers = vec!["192.0.2.1".into(), "2001:db8::53".into()];
        dns_config.searches = vec!["example.test".into(), "svc.example.test".into()];
        dns_config.options = vec!["ndots:2".into(), "single-request-reopen".into()];

        assert_eq!(
            convert_create_sandbox_dns(Some(dns_config)),
            [
                "nameserver 192.0.2.1",
                "nameserver 2001:db8::53",
                "search example.test svc.example.test",
                "options ndots:2 single-request-reopen",
            ]
        );
    }

    #[test]
    fn create_sandbox_without_dns_config_has_empty_dns() {
        assert!(convert_create_sandbox_dns(None).is_empty());
    }

    #[test]
    fn create_sandbox_without_options_uses_sandbox_id_as_hostname() {
        let mut request = sandbox_api::CreateSandboxRequest::new();
        request.sandbox_id = "sandbox-id".into();
        request.bundle_path = "/run/kata/sandbox-id".into();
        request.netns_path = "/run/netns/sandbox-id".into();
        request.annotations.insert(
            "io.katacontainers.config.hypervisor.default_vcpus".into(),
            "2".into(),
        );

        let request = SandboxRequest::try_from(request).unwrap();
        let SandboxRequest::CreateSandbox(config) = request else {
            panic!("expected create sandbox request");
        };

        let config = config.sandbox_config;
        assert_eq!(config.sandbox_id, "sandbox-id");
        assert_eq!(config.hostname, "sandbox-id");
        assert!(config.dns.is_empty());
        assert_eq!(
            config.network_env.netns.as_deref(),
            Some("/run/netns/sandbox-id")
        );
        assert_eq!(config.state.bundle, "/run/kata/sandbox-id");
        assert_eq!(
            config
                .annotations
                .get("io.katacontainers.config.hypervisor.default_vcpus")
                .map(String::as_str),
            Some("2")
        );
        assert_eq!(config.annotations, config.state.annotations);
    }

    #[test]
    fn create_sandbox_rejects_unknown_options_type() {
        let mut options = protobuf::well_known_types::any::Any::new();
        options.type_url = "example.test.UnknownSandboxConfig".into();

        let mut request = sandbox_api::CreateSandboxRequest::new();
        request.options = protobuf::MessageField::some(options);

        let error = SandboxRequest::try_from(request).unwrap_err();
        assert!(error
            .to_string()
            .contains("unsupported type url: example.test.UnknownSandboxConfig"));
    }

    #[test]
    fn create_sandbox_with_cri_options_preserves_cri_config_precedence() {
        let mut config = cri_api_v1::PodSandboxConfig::new();
        config.hostname = "cri-hostname".into();
        config
            .annotations
            .insert("annotation-source".into(), "cri".into());
        config
            .annotations
            .insert("example.test.annotation".into(), "value".into());

        let mut options = protobuf::well_known_types::any::Any::new();
        options.type_url = SANDBOX_API_V1.into();
        options.value = config.write_to_bytes().unwrap();

        let mut request = sandbox_api::CreateSandboxRequest::new();
        request.sandbox_id = "sandbox-id".into();
        request
            .annotations
            .insert("annotation-source".into(), "request".into());
        request.options = protobuf::MessageField::some(options);

        let request = SandboxRequest::try_from(request).unwrap();
        let SandboxRequest::CreateSandbox(config) = request else {
            panic!("expected create sandbox request");
        };

        assert_eq!(config.sandbox_config.hostname, "cri-hostname");
        assert_eq!(
            config
                .sandbox_config
                .annotations
                .get("example.test.annotation")
                .map(String::as_str),
            Some("value")
        );
        assert_eq!(
            config
                .sandbox_config
                .annotations
                .get("annotation-source")
                .map(String::as_str),
            Some("cri")
        );
    }
}
