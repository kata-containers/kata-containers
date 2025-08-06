// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::{
    any::type_name,
    convert::{Into, TryFrom},
};

use anyhow::{anyhow, Result};
use containerd_shim_protos::{api, sandbox_api};

use super::utils::option_system_time_into;
use super::{ProcessExitStatus, ProcessStateInfo, ProcessStatus, SandboxResponse, TaskResponse};
use crate::error::Error;

impl TryFrom<SandboxResponse> for sandbox_api::CreateSandboxResponse {
    type Error = anyhow::Error;
    fn try_from(from: SandboxResponse) -> Result<Self> {
        match from {
            SandboxResponse::CreateSandbox => Ok(Self::new()),
            _ => Err(anyhow!(Error::UnexpectedSandboxResponse(
                from,
                type_name::<Self>().to_string()
            ))),
        }
    }
}

impl TryFrom<SandboxResponse> for sandbox_api::StartSandboxResponse {
    type Error = anyhow::Error;
    fn try_from(from: SandboxResponse) -> Result<Self> {
        match from {
            SandboxResponse::StartSandbox(resp) => Ok(Self {
                pid: resp.pid,
                created_at: option_system_time_into(resp.create_time),
                ..Default::default()
            }),
            _ => Err(anyhow!(Error::UnexpectedSandboxResponse(
                from,
                type_name::<Self>().to_string()
            ))),
        }
    }
}

impl TryFrom<SandboxResponse> for sandbox_api::PlatformResponse {
    type Error = anyhow::Error;
    fn try_from(from: SandboxResponse) -> Result<Self> {
        match from {
            SandboxResponse::Platform(resp) => {
                let mut sandbox_resp = Self::new();
                sandbox_resp.mut_platform().set_os(resp.os);
                sandbox_resp
                    .mut_platform()
                    .set_architecture(resp.architecture);

                Ok(sandbox_resp)
            }
            _ => Err(anyhow!(Error::UnexpectedSandboxResponse(
                from,
                type_name::<Self>().to_string()
            ))),
        }
    }
}

impl TryFrom<SandboxResponse> for sandbox_api::StopSandboxResponse {
    type Error = anyhow::Error;
    fn try_from(from: SandboxResponse) -> Result<Self> {
        match from {
            SandboxResponse::StopSandbox => Ok(Self::new()),
            _ => Err(anyhow!(Error::UnexpectedSandboxResponse(
                from,
                type_name::<Self>().to_string()
            ))),
        }
    }
}

impl TryFrom<SandboxResponse> for sandbox_api::WaitSandboxResponse {
    type Error = anyhow::Error;
    fn try_from(from: SandboxResponse) -> Result<Self> {
        match from {
            SandboxResponse::WaitSandbox(resp) => Ok(Self {
                exit_status: resp.exit_status,
                exited_at: option_system_time_into(resp.exited_at),
                ..Default::default()
            }),
            _ => Err(anyhow!(Error::UnexpectedSandboxResponse(
                from,
                type_name::<Self>().to_string()
            ))),
        }
    }
}

impl TryFrom<SandboxResponse> for sandbox_api::SandboxStatusResponse {
    type Error = anyhow::Error;
    fn try_from(from: SandboxResponse) -> Result<Self> {
        match from {
            SandboxResponse::SandboxStatus(resp) => Ok(Self {
                sandbox_id: resp.sandbox_id,
                pid: resp.pid,
                state: resp.state,
                created_at: option_system_time_into(resp.created_at),
                exited_at: option_system_time_into(resp.exited_at),
                ..Default::default()
            }),
            _ => Err(anyhow!(Error::UnexpectedSandboxResponse(
                from,
                type_name::<Self>().to_string()
            ))),
        }
    }
}

impl TryFrom<SandboxResponse> for sandbox_api::PingResponse {
    type Error = anyhow::Error;
    fn try_from(from: SandboxResponse) -> Result<Self> {
        match from {
            SandboxResponse::Ping => Ok(Self::new()),
            _ => Err(anyhow!(Error::UnexpectedSandboxResponse(
                from,
                type_name::<Self>().to_string()
            ))),
        }
    }
}

impl TryFrom<SandboxResponse> for sandbox_api::ShutdownSandboxResponse {
    type Error = anyhow::Error;
    fn try_from(from: SandboxResponse) -> Result<Self> {
        match from {
            SandboxResponse::ShutdownSandbox => Ok(Self::new()),
            _ => Err(anyhow!(Error::UnexpectedSandboxResponse(
                from,
                type_name::<Self>().to_string()
            ))),
        }
    }
}

impl From<ProcessExitStatus> for api::WaitResponse {
    fn from(from: ProcessExitStatus) -> Self {
        Self {
            exit_status: from.exit_code as u32,
            exited_at: option_system_time_into(from.exit_time),
            ..Default::default()
        }
    }
}

impl From<ProcessStatus> for api::Status {
    fn from(from: ProcessStatus) -> Self {
        match from {
            ProcessStatus::Unknown => api::Status::UNKNOWN,
            ProcessStatus::Created => api::Status::CREATED,
            ProcessStatus::Running => api::Status::RUNNING,
            ProcessStatus::Stopped => api::Status::STOPPED,
            ProcessStatus::Paused => api::Status::PAUSED,
            ProcessStatus::Pausing => api::Status::PAUSING,
        }
    }
}

impl From<ProcessStateInfo> for api::StateResponse {
    fn from(from: ProcessStateInfo) -> Self {
        Self {
            id: from.container_id.clone(),
            bundle: from.bundle.clone(),
            pid: from.pid.pid,
            status: protobuf::EnumOrUnknown::new(from.status.into()),
            stdin: from.stdin.unwrap_or_default(),
            stdout: from.stdout.unwrap_or_default(),
            stderr: from.stderr.unwrap_or_default(),
            terminal: from.terminal,
            exit_status: from.exit_status as u32,
            exited_at: option_system_time_into(from.exited_at),
            exec_id: from.exec_id,
            ..Default::default()
        }
    }
}

impl From<ProcessStateInfo> for api::DeleteResponse {
    fn from(from: ProcessStateInfo) -> Self {
        Self {
            pid: from.pid.pid,
            exit_status: from.exit_status as u32,
            exited_at: option_system_time_into(from.exited_at),
            ..Default::default()
        }
    }
}

impl TryFrom<TaskResponse> for api::CreateTaskResponse {
    type Error = anyhow::Error;
    fn try_from(from: TaskResponse) -> Result<Self> {
        match from {
            TaskResponse::CreateContainer(resp) => Ok(Self {
                pid: resp.pid,
                ..Default::default()
            }),
            _ => Err(anyhow!(Error::UnexpectedResponse(
                from,
                type_name::<Self>().to_string()
            ))),
        }
    }
}

impl TryFrom<TaskResponse> for api::DeleteResponse {
    type Error = anyhow::Error;
    fn try_from(from: TaskResponse) -> Result<Self> {
        match from {
            TaskResponse::DeleteProcess(resp) => Ok(resp.into()),
            _ => Err(anyhow!(Error::UnexpectedResponse(
                from,
                type_name::<Self>().to_string()
            ))),
        }
    }
}

impl TryFrom<TaskResponse> for api::WaitResponse {
    type Error = anyhow::Error;
    fn try_from(from: TaskResponse) -> Result<Self> {
        match from {
            TaskResponse::WaitProcess(resp) => Ok(resp.into()),
            _ => Err(anyhow!(Error::UnexpectedResponse(
                from,
                type_name::<Self>().to_string()
            ))),
        }
    }
}

impl TryFrom<TaskResponse> for api::StartResponse {
    type Error = anyhow::Error;
    fn try_from(from: TaskResponse) -> Result<Self> {
        match from {
            TaskResponse::StartProcess(resp) => Ok(api::StartResponse {
                pid: resp.pid,
                ..Default::default()
            }),
            _ => Err(anyhow!(Error::UnexpectedResponse(
                from,
                type_name::<Self>().to_string()
            ))),
        }
    }
}

impl TryFrom<TaskResponse> for api::StateResponse {
    type Error = anyhow::Error;
    fn try_from(from: TaskResponse) -> Result<Self> {
        match from {
            TaskResponse::StateProcess(resp) => Ok(resp.into()),
            _ => Err(anyhow!(Error::UnexpectedResponse(
                from,
                type_name::<Self>().to_string()
            ))),
        }
    }
}

impl TryFrom<TaskResponse> for api::StatsResponse {
    type Error = anyhow::Error;
    fn try_from(from: TaskResponse) -> Result<Self> {
        let mut any = ::protobuf::well_known_types::any::Any::new();
        let mut response = api::StatsResponse::new();
        match from {
            TaskResponse::StatsContainer(resp) => {
                if let Some(value) = resp.value {
                    any.type_url = value.type_url;
                    any.value = value.value;
                    response.set_stats(any);
                }
                Ok(response)
            }
            _ => Err(anyhow!(Error::UnexpectedResponse(
                from,
                type_name::<Self>().to_string()
            ))),
        }
    }
}

impl TryFrom<TaskResponse> for api::PidsResponse {
    type Error = anyhow::Error;
    fn try_from(from: TaskResponse) -> Result<Self> {
        match from {
            TaskResponse::Pid(resp) => {
                let mut processes: Vec<api::ProcessInfo> = vec![];
                let mut p_info = api::ProcessInfo::new();
                let mut res = api::PidsResponse::new();
                p_info.set_pid(resp.pid);
                processes.push(p_info);
                res.set_processes(processes);
                Ok(res)
            }
            _ => Err(anyhow!(Error::UnexpectedResponse(
                from,
                type_name::<Self>().to_string()
            ))),
        }
    }
}

impl TryFrom<TaskResponse> for api::ConnectResponse {
    type Error = anyhow::Error;
    fn try_from(from: TaskResponse) -> Result<Self> {
        match from {
            TaskResponse::ConnectContainer(resp) => {
                let mut res = api::ConnectResponse::new();
                res.set_shim_pid(resp.pid);
                Ok(res)
            }
            _ => Err(anyhow!(Error::UnexpectedResponse(
                from,
                type_name::<Self>().to_string()
            ))),
        }
    }
}

impl TryFrom<TaskResponse> for api::Empty {
    type Error = anyhow::Error;
    fn try_from(from: TaskResponse) -> Result<Self> {
        match from {
            TaskResponse::CloseProcessIO => Ok(api::Empty::new()),
            TaskResponse::ExecProcess => Ok(api::Empty::new()),
            TaskResponse::KillProcess => Ok(api::Empty::new()),
            TaskResponse::ShutdownContainer => Ok(api::Empty::new()),
            TaskResponse::PauseContainer => Ok(api::Empty::new()),
            TaskResponse::ResumeContainer => Ok(api::Empty::new()),
            TaskResponse::ResizeProcessPTY => Ok(api::Empty::new()),
            TaskResponse::UpdateContainer => Ok(api::Empty::new()),
            _ => Err(anyhow!(Error::UnexpectedResponse(
                from,
                type_name::<Self>().to_string()
            ))),
        }
    }
}
