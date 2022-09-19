// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use super::{
    ContainerConfig, ContainerID, ContainerProcess, ExecProcessRequest, KillRequest, Request,
    ResizePTYRequest, ShutdownRequest, UpdateRequest,
};
use anyhow::{Context, Result};
use containerd_shim_protos::api;
use kata_types::mount::Mount;
use std::{
    convert::{From, TryFrom},
    path::PathBuf,
};

fn trans_from_shim_mount(from: api::Mount) -> Mount {
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
        fs_type: from.field_type,
        options,
        device_id: None,
        host_shared_fs_path: None,
        read_only,
    }
}

impl TryFrom<api::CreateTaskRequest> for Request {
    type Error = anyhow::Error;
    fn try_from(from: api::CreateTaskRequest) -> Result<Self> {
        let options = if from.has_options() {
            Some(from.get_options().get_value().to_vec())
        } else {
            None
        };
        Ok(Request::CreateContainer(ContainerConfig {
            container_id: from.id.clone(),
            bundle: from.bundle.clone(),
            rootfs_mounts: from
                .rootfs
                .to_vec()
                .into_iter()
                .map(trans_from_shim_mount)
                .collect(),
            terminal: from.terminal,
            options,
            stdin: (!from.stdin.is_empty()).then(|| from.stdin.clone()),
            stdout: (!from.stdout.is_empty()).then(|| from.stdout.clone()),
            stderr: (!from.stderr.is_empty()).then(|| from.stderr.clone()),
        }))
    }
}

impl TryFrom<api::CloseIORequest> for Request {
    type Error = anyhow::Error;
    fn try_from(from: api::CloseIORequest) -> Result<Self> {
        Ok(Request::CloseProcessIO(
            ContainerProcess::new(&from.id, &from.exec_id).context("new process id")?,
        ))
    }
}

impl TryFrom<api::DeleteRequest> for Request {
    type Error = anyhow::Error;
    fn try_from(from: api::DeleteRequest) -> Result<Self> {
        Ok(Request::DeleteProcess(
            ContainerProcess::new(&from.id, &from.exec_id).context("new process id")?,
        ))
    }
}

impl TryFrom<api::ExecProcessRequest> for Request {
    type Error = anyhow::Error;
    fn try_from(from: api::ExecProcessRequest) -> Result<Self> {
        let spec = from.get_spec();
        Ok(Request::ExecProcess(ExecProcessRequest {
            process: ContainerProcess::new(&from.id, &from.exec_id).context("new process id")?,
            terminal: from.terminal,
            stdin: (!from.stdin.is_empty()).then(|| from.stdin.clone()),
            stdout: (!from.stdout.is_empty()).then(|| from.stdout.clone()),
            stderr: (!from.stderr.is_empty()).then(|| from.stderr.clone()),
            spec_type_url: spec.get_type_url().to_string(),
            spec_value: spec.get_value().to_vec(),
        }))
    }
}

impl TryFrom<api::KillRequest> for Request {
    type Error = anyhow::Error;
    fn try_from(from: api::KillRequest) -> Result<Self> {
        Ok(Request::KillProcess(KillRequest {
            process: ContainerProcess::new(&from.id, &from.exec_id).context("new process id")?,
            signal: from.signal,
            all: from.all,
        }))
    }
}

impl TryFrom<api::WaitRequest> for Request {
    type Error = anyhow::Error;
    fn try_from(from: api::WaitRequest) -> Result<Self> {
        Ok(Request::WaitProcess(
            ContainerProcess::new(&from.id, &from.exec_id).context("new process id")?,
        ))
    }
}

impl TryFrom<api::StartRequest> for Request {
    type Error = anyhow::Error;
    fn try_from(from: api::StartRequest) -> Result<Self> {
        Ok(Request::StartProcess(
            ContainerProcess::new(&from.id, &from.exec_id).context("new process id")?,
        ))
    }
}

impl TryFrom<api::StateRequest> for Request {
    type Error = anyhow::Error;
    fn try_from(from: api::StateRequest) -> Result<Self> {
        Ok(Request::StateProcess(
            ContainerProcess::new(&from.id, &from.exec_id).context("new process id")?,
        ))
    }
}

impl TryFrom<api::ShutdownRequest> for Request {
    type Error = anyhow::Error;
    fn try_from(from: api::ShutdownRequest) -> Result<Self> {
        Ok(Request::ShutdownContainer(ShutdownRequest {
            container_id: from.id.to_string(),
            is_now: from.now,
        }))
    }
}

impl TryFrom<api::ResizePtyRequest> for Request {
    type Error = anyhow::Error;
    fn try_from(from: api::ResizePtyRequest) -> Result<Self> {
        Ok(Request::ResizeProcessPTY(ResizePTYRequest {
            process: ContainerProcess::new(&from.id, &from.exec_id).context("new process id")?,
            width: from.width,
            height: from.height,
        }))
    }
}

impl TryFrom<api::PauseRequest> for Request {
    type Error = anyhow::Error;
    fn try_from(from: api::PauseRequest) -> Result<Self> {
        Ok(Request::PauseContainer(ContainerID::new(&from.id)?))
    }
}

impl TryFrom<api::ResumeRequest> for Request {
    type Error = anyhow::Error;
    fn try_from(from: api::ResumeRequest) -> Result<Self> {
        Ok(Request::ResumeContainer(ContainerID::new(&from.id)?))
    }
}

impl TryFrom<api::StatsRequest> for Request {
    type Error = anyhow::Error;
    fn try_from(from: api::StatsRequest) -> Result<Self> {
        Ok(Request::StatsContainer(ContainerID::new(&from.id)?))
    }
}

impl TryFrom<api::UpdateTaskRequest> for Request {
    type Error = anyhow::Error;
    fn try_from(from: api::UpdateTaskRequest) -> Result<Self> {
        Ok(Request::UpdateContainer(UpdateRequest {
            container_id: from.id.to_string(),
            value: from.get_resources().get_value().to_vec(),
        }))
    }
}

impl TryFrom<api::PidsRequest> for Request {
    type Error = anyhow::Error;
    fn try_from(_from: api::PidsRequest) -> Result<Self> {
        Ok(Request::Pid)
    }
}

impl TryFrom<api::ConnectRequest> for Request {
    type Error = anyhow::Error;
    fn try_from(from: api::ConnectRequest) -> Result<Self> {
        Ok(Request::ConnectContainer(ContainerID::new(&from.id)?))
    }
}
