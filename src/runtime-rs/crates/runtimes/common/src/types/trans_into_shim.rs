// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::{
    any::type_name,
    convert::{Into, TryFrom, TryInto},
    time,
};

use anyhow::{anyhow, Result};
use containerd_shim_protos::api;

use super::{ProcessExitStatus, ProcessStateInfo, ProcessStatus, Response};
use crate::error::Error;

fn system_time_into(time: time::SystemTime) -> ::protobuf::well_known_types::Timestamp {
    let mut proto_time = ::protobuf::well_known_types::Timestamp::new();
    proto_time.set_seconds(
        time.duration_since(time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
            .try_into()
            .unwrap_or_default(),
    );
    proto_time
}

fn option_system_time_into(
    time: Option<time::SystemTime>,
) -> ::protobuf::SingularPtrField<::protobuf::well_known_types::Timestamp> {
    match time {
        Some(v) => ::protobuf::SingularPtrField::some(system_time_into(v)),
        None => ::protobuf::SingularPtrField::none(),
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
            status: from.status.into(),
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

impl TryFrom<Response> for api::CreateTaskResponse {
    type Error = anyhow::Error;
    fn try_from(from: Response) -> Result<Self> {
        match from {
            Response::CreateContainer(resp) => Ok(Self {
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

impl TryFrom<Response> for api::DeleteResponse {
    type Error = anyhow::Error;
    fn try_from(from: Response) -> Result<Self> {
        match from {
            Response::DeleteProcess(resp) => Ok(resp.into()),
            _ => Err(anyhow!(Error::UnexpectedResponse(
                from,
                type_name::<Self>().to_string()
            ))),
        }
    }
}

impl TryFrom<Response> for api::WaitResponse {
    type Error = anyhow::Error;
    fn try_from(from: Response) -> Result<Self> {
        match from {
            Response::WaitProcess(resp) => Ok(resp.into()),
            _ => Err(anyhow!(Error::UnexpectedResponse(
                from,
                type_name::<Self>().to_string()
            ))),
        }
    }
}

impl TryFrom<Response> for api::StartResponse {
    type Error = anyhow::Error;
    fn try_from(from: Response) -> Result<Self> {
        match from {
            Response::StartProcess(resp) => Ok(api::StartResponse {
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

impl TryFrom<Response> for api::StateResponse {
    type Error = anyhow::Error;
    fn try_from(from: Response) -> Result<Self> {
        match from {
            Response::StateProcess(resp) => Ok(resp.into()),
            _ => Err(anyhow!(Error::UnexpectedResponse(
                from,
                type_name::<Self>().to_string()
            ))),
        }
    }
}

impl TryFrom<Response> for api::StatsResponse {
    type Error = anyhow::Error;
    fn try_from(from: Response) -> Result<Self> {
        let mut any = ::protobuf::well_known_types::Any::new();
        let mut response = api::StatsResponse::new();
        match from {
            Response::StatsContainer(resp) => {
                if let Some(value) = resp.value {
                    any.set_type_url(value.type_url);
                    any.set_value(value.value);
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

impl TryFrom<Response> for api::PidsResponse {
    type Error = anyhow::Error;
    fn try_from(from: Response) -> Result<Self> {
        match from {
            Response::Pid(resp) => {
                let mut processes: Vec<api::ProcessInfo> = vec![];
                let mut p_info = api::ProcessInfo::new();
                let mut res = api::PidsResponse::new();
                p_info.set_pid(resp.pid);
                processes.push(p_info);
                let v = protobuf::RepeatedField::<api::ProcessInfo>::from_vec(processes);
                res.set_processes(v);
                Ok(res)
            }
            _ => Err(anyhow!(Error::UnexpectedResponse(
                from,
                type_name::<Self>().to_string()
            ))),
        }
    }
}

impl TryFrom<Response> for api::ConnectResponse {
    type Error = anyhow::Error;
    fn try_from(from: Response) -> Result<Self> {
        match from {
            Response::ConnectContainer(resp) => {
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

impl TryFrom<Response> for api::Empty {
    type Error = anyhow::Error;
    fn try_from(from: Response) -> Result<Self> {
        match from {
            Response::CloseProcessIO => Ok(api::Empty::new()),
            Response::ExecProcess => Ok(api::Empty::new()),
            Response::KillProcess => Ok(api::Empty::new()),
            Response::ShutdownContainer => Ok(api::Empty::new()),
            Response::PauseContainer => Ok(api::Empty::new()),
            Response::ResumeContainer => Ok(api::Empty::new()),
            Response::ResizeProcessPTY => Ok(api::Empty::new()),
            Response::UpdateContainer => Ok(api::Empty::new()),
            _ => Err(anyhow!(Error::UnexpectedResponse(
                from,
                type_name::<Self>().to_string()
            ))),
        }
    }
}
