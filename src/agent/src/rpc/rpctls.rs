// copyright (c) 2019 ant financial
//
// SPDX-License-Identifier: Apache-2.0
//

use std::fs;
use std::io::{self};
use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use libc::{self, c_ushort, winsize, TIOCSWINSZ};
use nix::errno::Errno;
use protocols::agent;
use rustjail::container::{BaseContainer, Container};
use tokio::sync::Mutex;

use crate::linux_abi::*;
use crate::metrics::get_metrics as other_get_metrics;
use crate::random;
use crate::sandbox::Sandbox;
use crate::version::{AGENT_VERSION, API_VERSION};

use tonic::transport::{
    server::{TcpConnectInfo, TlsConnectInfo},
    Server, ServerTlsConfig,
};

use crate::rpc::rpctls::grpctls::{
    health_check_response, CheckRequest, CloseStdinRequest, ContainerInfoList, CopyFileRequest,
    CreateContainerRequest, ExecProcessRequest, GetMetricsRequest, GetOomEventRequest,
    GuestDetailsRequest, GuestDetailsResponse, HealthCheckResponse, Interfaces,
    ListContainersRequest, ListInterfacesRequest, ListRoutesRequest, Metrics, OnlineCpuMemRequest,
    OomEvent, PauseContainerRequest, ReadStreamRequest, ReadStreamResponse, RemoveContainerRequest,
    ReseedRandomDevRequest, ResumeContainerRequest, Routes, SetGuestDateTimeRequest,
    SignalProcessRequest, StartContainerRequest, StatsContainerRequest, StatsContainerResponse,
    TtyWinResizeRequest, UpdateContainerRequest, VersionCheckResponse, WaitProcessRequest,
    WaitProcessResponse, WriteStreamRequest, WriteStreamResponse,
};

use super::AgentService;
use super::HealthService;

pub mod grpctls {
    include!("../../../libs/protocols/src/grpctls/grpctls.rs");
}

pub mod types {
    include!("../../../libs/protocols/src/grpctls/types.rs");
}

// Convenience macro to obtain the scope logger
macro_rules! sl {
    () => {
        slog_scope::logger()
    };
}

pub const GRPC_TLS_SERVER_PORT: u16 = 50090;

#[tonic::async_trait]
impl grpctls::agent_service_server::AgentService for AgentService {
    async fn create_container(
        &self,
        req: tonic::Request<CreateContainerRequest>,
    ) -> Result<tonic::Response<()>, tonic::Status> {
        info!(sl!(), "grpctls: create_container, string req: {:#?}", req);
        let mut ttrpc_req = agent::CreateContainerRequest::new();
        let internal = req.into_inner();
        ttrpc_req.set_container_id(internal.container_id);
        ttrpc_req.set_exec_id(internal.exec_id);
        ttrpc_req.set_sandbox_pidns(internal.sandbox_pidns);

        let oci_obj = internal.oci.unwrap();
        let oci_str = match serde_json::to_string(&oci_obj) {
            Ok(j) => j,
            Err(e) => {
                return Err(tonic::Status::new(
                    tonic::Code::Internal,
                    format!("Unable to serialize{}", e),
                ))
            }
        };

        info!(
            sl!(),
            "grpctls: create_container, string oci_str {:?}", oci_str
        );
        let roci_spec: protocols::oci::Spec = match serde_json::from_str(&oci_str) {
            Ok(k) => k,
            Err(e) => {
                return Err(tonic::Status::new(
                    tonic::Code::Internal,
                    format!("Unable to deserialize{}", e),
                ))
            }
        };

        info!(
            sl!(),
            "grpctls: reate_container oci_spec, ttrpc oci obj: {:?}", roci_spec
        );
        ttrpc_req.set_OCI(roci_spec);

        // Convert to grpctls type to strint
        let vec_mount_str = match serde_json::to_string(&internal.shared_mounts) {
            Ok(j) => j,
            Err(e) => {
                return Err(tonic::Status::new(
                    tonic::Code::Internal,
                    format!("Unable to serialize{}", e),
                ))
            }
        };

        let r_shared_mounts: Vec<agent::SharedMount> = match convert_shared_mounts(vec_mount_str) {
            Ok(k) => k,
            Err(e) => {
                return Err(tonic::Status::new(
                    tonic::Code::Internal,
                    format!("Unable to deserialize shared_mounts {:?} ", e),
                ))
            }
        };

        //info!(sl!(), "grpctls: shared mounts, ttrpc_req: {:#?}", &r_shared_mounts);
        ttrpc_req.set_shared_mounts(r_shared_mounts);

        info!(
            sl!(),
            "grpctls: create_container, ttrpc_req: {:#?}", ttrpc_req
        );
        match self.do_create_container(ttrpc_req).await {
            Err(e) => Err(tonic::Status::new(tonic::Code::Internal, format!("{}", e))),
            Ok(_) => Ok(tonic::Response::new(())),
        }
    }

    async fn exec_process(
        &self,
        req: tonic::Request<ExecProcessRequest>,
    ) -> Result<tonic::Response<()>, tonic::Status> {
        info!(sl!(), "grpctls: do_exec_process, string req: {:#?}", req);
        let message = req.get_ref();
        let jstr = match serde_json::to_string(message) {
            Ok(j) => j,
            Err(e) => {
                return Err(tonic::Status::new(
                    tonic::Code::Internal,
                    format!("Unable to serialize{}", e),
                ))
            }
        };

        //info!(sl!(), "grpctls: exec_process, string req: {}", jstr);
        let ttrpc_req: agent::ExecProcessRequest = match serde_json::from_str(&jstr) {
            Ok(t) => t,
            Err(e) => {
                return Err(tonic::Status::new(
                    tonic::Code::Internal,
                    format!("Unable to deserialize{}", e),
                ))
            }
        };

        info!(
            sl!(),
            "grpctls: do_exec_process, string req: {:#?}", ttrpc_req
        );
        match self.do_exec_process(ttrpc_req).await {
            Err(e) => Err(tonic::Status::new(tonic::Code::Internal, format!("{}", e))),
            Ok(_) => Ok(tonic::Response::new(())),
        }
    }

    async fn pause_container(
        &self,
        req: tonic::Request<PauseContainerRequest>,
    ) -> Result<tonic::Response<()>, tonic::Status> {
        let _conn_info = req
            .extensions()
            .get::<TlsConnectInfo<TcpConnectInfo>>()
            .unwrap();

        let cid = req.into_inner().container_id;
        let s = Arc::clone(&self.sandbox);
        let mut sandbox = s.lock().await;

        let ctr = sandbox.get_container(&cid).ok_or_else(|| {
            tonic::Status::new(tonic::Code::Internal, "Ivalid container id".to_string())
        })?;

        ctr.pause().map_err(|e| {
            tonic::Status::new(
                tonic::Code::Internal,
                format!("Service was not ready: {:?}", e),
            )
        })?;

        Ok(tonic::Response::new(()))
    }

    async fn remove_container(
        &self,
        req: tonic::Request<RemoveContainerRequest>,
    ) -> Result<tonic::Response<()>, tonic::Status> {
        let message = req.get_ref();
        let jstr = match serde_json::to_string(message) {
            Ok(j) => j,
            Err(e) => {
                return Err(tonic::Status::new(
                    tonic::Code::Internal,
                    format!("Unable to serialize{}", e),
                ))
            }
        };

        //info!(sl!(), "grpctls: do_remove_container, string req: {}", jstr);
        let ttrpc_req: agent::RemoveContainerRequest = match serde_json::from_str(&jstr) {
            Ok(t) => t,
            Err(e) => {
                return Err(tonic::Status::new(
                    tonic::Code::Internal,
                    format!("Unable to deserialize{}", e),
                ))
            }
        };

        info!(
            sl!(),
            "grpctls: do_remove_container, string req: {:#?}", ttrpc_req
        );
        match self.do_remove_container(ttrpc_req).await {
            Err(e) => Err(tonic::Status::new(tonic::Code::Internal, format!("{}", e))),
            Ok(_) => Ok(tonic::Response::new(())),
        }
    }

    async fn resume_container(
        &self,
        req: tonic::Request<ResumeContainerRequest>,
    ) -> Result<tonic::Response<()>, tonic::Status> {
        let _conn_info = req
            .extensions()
            .get::<TlsConnectInfo<TcpConnectInfo>>()
            .unwrap();

        let cid = req.into_inner().container_id;
        let s = Arc::clone(&self.sandbox);
        let mut sandbox = s.lock().await;

        let ctr = sandbox.get_container(&cid).ok_or_else(|| {
            tonic::Status::new(tonic::Code::Internal, "Invalid container id".to_string())
        })?;

        ctr.resume().map_err(|e| {
            tonic::Status::new(
                tonic::Code::Internal,
                format!("Service was not ready: {:?}", e),
            )
        })?;

        Ok(tonic::Response::new(()))
    }

    async fn update_container(
        &self,
        req: tonic::Request<UpdateContainerRequest>,
    ) -> Result<tonic::Response<()>, tonic::Status> {
        let internal = req.into_inner();
        let cid = internal.container_id.clone();
        let res = internal.resources;

        let s = Arc::clone(&self.sandbox);
        let mut sandbox = s.lock().await;

        let ctr = sandbox.get_container(&cid).ok_or_else(|| {
            tonic::Status::new(
                tonic::Code::InvalidArgument,
                format!("invalid container id {}", cid),
            )
        })?;

        let resp = tonic::Response::new(());

        // Convert grpctls::LinuxResources to protocol::oci::LinuxResources
        let jstr = match serde_json::to_string(&res) {
            Ok(j) => j,
            Err(e) => {
                return Err(tonic::Status::new(
                    tonic::Code::Internal,
                    format!("Unable to serialize linuxresource: {}", e),
                ))
            }
        };

        let res_obj: protocols::oci::LinuxResources = match serde_json::from_str(&jstr) {
            Ok(k) => k,
            Err(e) => {
                return Err(tonic::Status::new(
                    tonic::Code::Internal,
                    format!("Unable to deserialize to linuxresource: {}", e),
                ))
            }
        };

        info!(
            sl!(),
            "grpctls: update_container linuxresource, res obj: {:?}", res_obj
        );
        let oci_res = rustjail::resources_grpc_to_oci(&res_obj);
        match ctr.set(oci_res) {
            Err(e) => {
                return Err(tonic::Status::new(tonic::Code::Internal, format!("{}", e)));
            }

            Ok(_) => return Ok(resp),
        }
    }

    async fn stats_container(
        &self,
        req: tonic::Request<StatsContainerRequest>,
    ) -> Result<tonic::Response<StatsContainerResponse>, tonic::Status> {
        let internal = req.into_inner();
        let cid = internal.container_id.clone();

        let mut sandbox = self.sandbox.lock().await;
        let ctr = sandbox.get_container(&cid).ok_or_else(|| {
            tonic::Status::new(tonic::Code::Internal, "Invalid container id".to_string())
        })?;

        let ctr_stats = ctr.stats().map_err(|e| {
            tonic::Status::new(
                tonic::Code::Internal,
                format!("fail to get stats info!{:?}", e),
            )
        })?;

        // let ctr_obj:  grpctls::StatsContainerResponse = convert_type_grcptls(&ctr_stats)?;

        let ctr_str = match serde_json::to_string(&ctr_stats) {
            Ok(j) => j,
            Err(e) => {
                return Err(tonic::Status::new(
                    tonic::Code::Internal,
                    format!("Unable to serialize{}", e),
                ))
            }
        };

        let ctr_obj: grpctls::StatsContainerResponse = match serde_json::from_str(&ctr_str) {
            Ok(k) => k,
            Err(e) => {
                return Err(tonic::Status::new(
                    tonic::Code::Internal,
                    format!("Unable to deserialize{}", e),
                ))
            }
        };

        Ok(tonic::Response::new(StatsContainerResponse {
            cgroup_stats: ctr_obj.cgroup_stats,
            network_stats: ctr_obj.network_stats,
        }))
    }

    async fn start_container(
        &self,
        req: tonic::Request<StartContainerRequest>,
    ) -> Result<tonic::Response<()>, tonic::Status> {
        let message = req.get_ref();
        let jstr = match serde_json::to_string(message) {
            Ok(j) => j,
            Err(e) => {
                return Err(tonic::Status::new(
                    tonic::Code::Internal,
                    format!("Unable to serialize{}", e),
                ))
            }
        };

        //info!(sl!(), "grpctls: do_start_container, string req: {}", jstr);
        let ttrpc_req: agent::StartContainerRequest = match serde_json::from_str(&jstr) {
            Ok(t) => t,
            Err(e) => {
                return Err(tonic::Status::new(
                    tonic::Code::Internal,
                    format!("Unable to deserialize{}", e),
                ))
            }
        };

        info!(
            sl!(),
            "grpctls: do_start_container, string req: {:?}", ttrpc_req
        );
        match self.do_start_container(ttrpc_req).await {
            Err(e) => Err(tonic::Status::new(tonic::Code::Internal, format!("{}", e))),
            Ok(_) => Ok(tonic::Response::new(())),
        }
    }

    async fn list_containers(
        &self,
        _req: tonic::Request<ListContainersRequest>,
    ) -> Result<tonic::Response<ContainerInfoList>, tonic::Status> {
        let s = Arc::clone(&self.sandbox);
        let sandbox = s.lock().await;
        let list = sandbox.list_containers().map_err(|e| {
            tonic::Status::new(
                tonic::Code::Internal,
                format!("List Contianer Service was not ready: {:?}", e),
            )
        })?;

        Ok(tonic::Response::new(ContainerInfoList {
            container_info_list: list.clone(),
        }))
    }

    async fn signal_process(
        &self,
        req: tonic::Request<SignalProcessRequest>,
    ) -> Result<tonic::Response<()>, tonic::Status> {
        info!(sl!(), "grpctls: signal_process, string req: {:?}", req);
        let mut ttrpc_req = agent::SignalProcessRequest::new();
        let internal = req.into_inner();
        ttrpc_req.set_container_id(internal.container_id);
        ttrpc_req.set_exec_id(internal.exec_id);
        ttrpc_req.set_signal(internal.signal);

        match self.do_signal_process(ttrpc_req).await {
            Err(e) => Err(tonic::Status::new(tonic::Code::Internal, format!("{}", e))),
            Ok(_) => Ok(tonic::Response::new(())),
        }
    }

    async fn wait_process(
        &self,
        req: tonic::Request<WaitProcessRequest>,
    ) -> Result<tonic::Response<WaitProcessResponse>, tonic::Status> {
        info!(sl!(), "grpctls: wait_process, string req: {:?}", req);
        let internal = req.into_inner();
        let mut ttrpc_req = agent::WaitProcessRequest::new();
        ttrpc_req.set_container_id(internal.container_id);
        ttrpc_req.set_exec_id(internal.exec_id);

        let response = self
            .do_wait_process(ttrpc_req)
            .await
            .map_err(|e| tonic::Status::new(tonic::Code::Internal, format!("{:?}", e)))?;
        let status = response.status();

        Ok(tonic::Response::new(WaitProcessResponse { status }))
    }

    async fn write_stdin(
        &self,
        req: tonic::Request<WriteStreamRequest>,
    ) -> Result<tonic::Response<WriteStreamResponse>, tonic::Status> {
        let internal = req.into_inner();
        let mut ttrpc_req = agent::WriteStreamRequest::new();

        ttrpc_req.set_container_id(internal.container_id);
        ttrpc_req.set_exec_id(internal.exec_id);
        ttrpc_req.set_data(internal.data);

        let response = self
            .do_write_stream(ttrpc_req)
            .await
            .map_err(|e| tonic::Status::new(tonic::Code::Internal, format!("{:?}", e)))?;

        let len = response.len();

        Ok(tonic::Response::new(WriteStreamResponse { len }))
    }

    async fn read_stdout(
        &self,
        req: tonic::Request<ReadStreamRequest>,
    ) -> Result<tonic::Response<ReadStreamResponse>, tonic::Status> {
        let internal = req.into_inner();
        let mut ttrpc_req = agent::ReadStreamRequest::new();

        ttrpc_req.set_container_id(internal.container_id);
        ttrpc_req.set_exec_id(internal.exec_id);
        ttrpc_req.set_len(internal.len);

        let response = self
            .do_read_stream(ttrpc_req, true)
            .await
            .map_err(|e| tonic::Status::new(tonic::Code::Internal, format!("{:?}", e)))?;

        let data = response.data();

        Ok(tonic::Response::new(ReadStreamResponse {
            data: data.to_vec(),
        }))
    }

    async fn close_stdin(
        &self,
        req: tonic::Request<CloseStdinRequest>,
    ) -> Result<tonic::Response<()>, tonic::Status> {
        let internal = req.into_inner();

        let cid = internal.container_id.clone();
        let eid = internal.exec_id;
        let s = Arc::clone(&self.sandbox);
        let mut sandbox = s.lock().await;

        let p = sandbox
            .find_container_process(cid.as_str(), eid.as_str())
            .map_err(|e| {
                tonic::Status::new(
                    tonic::Code::InvalidArgument,
                    format!("invalid argument: {:?}", e),
                )
            })?;

        p.close_stdin().await;

        Ok(tonic::Response::new(()))
    }

    async fn read_stderr(
        &self,
        req: tonic::Request<ReadStreamRequest>,
    ) -> Result<tonic::Response<ReadStreamResponse>, tonic::Status> {
        let internal = req.into_inner();
        let mut ttrpc_req = agent::ReadStreamRequest::new();

        ttrpc_req.set_container_id(internal.container_id);
        ttrpc_req.set_exec_id(internal.exec_id);
        ttrpc_req.set_len(internal.len);

        let response = self
            .do_read_stream(ttrpc_req, false)
            .await
            .map_err(|e| tonic::Status::new(tonic::Code::Internal, format!("{:?}", e)))?;

        let data = response.data();

        Ok(tonic::Response::new(ReadStreamResponse {
            data: data.to_vec(),
        }))
    }

    async fn tty_win_resize(
        &self,
        req: tonic::Request<TtyWinResizeRequest>,
    ) -> Result<tonic::Response<()>, tonic::Status> {
        info!(sl!(), "grpctls: tty_win_resize req: {:?}", req);
        let internal = req.into_inner();

        let cid = internal.container_id.clone();
        let eid = internal.exec_id.clone();
        let row = internal.row;
        let column = internal.column;

        let s = Arc::clone(&self.sandbox);
        let mut sandbox = s.lock().await;
        let p = sandbox
            .find_container_process(cid.as_str(), eid.as_str())
            .map_err(|e| {
                tonic::Status::new(
                    tonic::Code::Unavailable,
                    format!("invalid argument: {:?}", e),
                )
            })?;

        if let Some(fd) = p.term_master {
            unsafe {
                let win = winsize {
                    ws_row: row as c_ushort,
                    ws_col: column as c_ushort,
                    ws_xpixel: 0,
                    ws_ypixel: 0,
                };

                let err = libc::ioctl(fd, TIOCSWINSZ, &win);
                Errno::result(err).map(drop).map_err(|e| {
                    tonic::Status::new(tonic::Code::Internal, format!("ioctl error: {:?}", e))
                })?;
            }
        } else {
            // return Err(ttrpc_error!(ttrpc::Code::UNAVAILABLE, "no tty".to_string()));
            return Err(tonic::Status::new(
                tonic::Code::Unavailable,
                "no tty".to_string(),
            ));
        }

        Ok(tonic::Response::new(()))
    }

    async fn list_interfaces(
        &self,
        req: tonic::Request<ListInterfacesRequest>,
    ) -> Result<tonic::Response<Interfaces>, tonic::Status> {
        info!(sl!(), "grpctls: list_interfaces, string req: {:?}", req);
        let list = self
            .sandbox
            .lock()
            .await
            .rtnl
            .list_interfaces()
            .await
            .map_err(|e| {
                tonic::Status::new(
                    tonic::Code::Internal,
                    format!("Failed to list interfaces: {:?}", e),
                )
            })?;
        // Convert to grpctls type
        let vec_interface_str = match serde_json::to_string(&list) {
            Ok(j) => j,
            Err(e) => {
                return Err(tonic::Status::new(
                    tonic::Code::Internal,
                    format!("Unable to serialize{}", e),
                ))
            }
        };

        info!(
            sl!(),
            "grpctls: list interfaces, string vec_interface_str {}", vec_interface_str
        );
        let vec_interface: Vec<types::Interface> = match convert_interface(vec_interface_str) {
            Ok(k) => k,
            Err(e) => {
                return Err(tonic::Status::new(
                    tonic::Code::Internal,
                    format!("Unable to deserialize interface list {:?} ", e),
                ))
            }
        };

        Ok(tonic::Response::new(Interfaces {
            interfaces: vec_interface,
        }))
    }

    async fn list_routes(
        &self,
        _req: tonic::Request<ListRoutesRequest>,
    ) -> Result<tonic::Response<Routes>, tonic::Status> {
        let list = self
            .sandbox
            .lock()
            .await
            .rtnl
            .list_routes()
            .await
            .map_err(|e| {
                tonic::Status::new(tonic::Code::Internal, format!("list routes: {:?}", e))
            })?;

        // Convert  protocols::types::Route to rpctls::types::Route
        let vec_routes_str = match serde_json::to_string(&list) {
            Ok(j) => j,
            Err(e) => {
                return Err(tonic::Status::new(
                    tonic::Code::Internal,
                    format!("Unable to serialize route list {}", e),
                ))
            }
        };

        info!(sl!(), "grpctls: route list string {}", vec_routes_str);
        let vec_routes: Vec<types::Route> = match convert_routes(vec_routes_str) {
            Ok(k) => k,
            Err(e) => {
                return Err(tonic::Status::new(
                    tonic::Code::Internal,
                    format!("Unable to deserialize route list {:?} ", e),
                ))
            }
        };

        //info!(sl!(), "grpctls: interface  obj: {:?}", vec_routes);
        Ok(tonic::Response::new(Routes { routes: vec_routes }))
    }

    async fn get_metrics(
        &self,
        req: tonic::Request<GetMetricsRequest>,
    ) -> Result<tonic::Response<Metrics>, tonic::Status> {
        info!(sl!(), "grpctls: get_metrics, string req: {:?}", req);
        let ttrpc_req = protocols::agent::GetMetricsRequest::new();

        match other_get_metrics(&ttrpc_req) {
            Err(e) => Err(tonic::Status::new(tonic::Code::Internal, format!("{}", e))),
            Ok(s) => Ok(tonic::Response::new(Metrics { metrics: s })),
        }
    }

    async fn online_cpu_mem(
        &self,
        req: tonic::Request<OnlineCpuMemRequest>,
    ) -> Result<tonic::Response<()>, tonic::Status> {
        info!(sl!(), "grpctls: online_cpu_mem req: {:?}", req);
        let internal = req.into_inner();

        let mut ttrpc_req = protocols::agent::OnlineCPUMemRequest::new();
        ttrpc_req.set_wait(internal.wait);
        ttrpc_req.set_nb_cpus(internal.nb_cpus);
        ttrpc_req.set_cpu_only(internal.cpu_only);

        let sandbox = self.sandbox.lock().await;

        sandbox
            .online_cpu_memory(&ttrpc_req)
            .map_err(|e| tonic::Status::new(tonic::Code::Internal, format!("{:?}", e)))?;

        Ok(tonic::Response::new(()))
    }

    async fn reseed_random_dev(
        &self,
        req: tonic::Request<ReseedRandomDevRequest>,
    ) -> Result<tonic::Response<()>, tonic::Status> {
        info!(sl!(), "grpctls: reseed_random_dev req: {:?}", req);
        let data = req.into_inner().data;

        random::reseed_rng(data.as_slice())
            .map_err(|e| tonic::Status::new(tonic::Code::Internal, format!("{:?}", e)))?;

        Ok(tonic::Response::new(()))
    }

    async fn get_guest_details(
        &self,
        req: tonic::Request<GuestDetailsRequest>,
    ) -> Result<tonic::Response<GuestDetailsResponse>, tonic::Status> {
        info!(sl!(), "grpctls: get guest details {:?}", req);
        let internal = req.into_inner();

        //let mut resp = GuestDetailsResponse::new();
        // to get memory block size
        let (u, v) = super::get_memory_info(
            internal.mem_block_size,
            internal.mem_hotplug_probe,
            SYSFS_MEMORY_BLOCK_SIZE_PATH,
            SYSFS_MEMORY_HOTPLUG_PROBE_PATH,
        )
        .map_err(|e| {
            tonic::Status::new(
                tonic::Code::Internal,
                format!("fail to get memory info!{:?}", e),
            )
        })?;

        let detail = super::get_agent_details();
        let detail_str = match serde_json::to_string(&detail) {
            Ok(j) => j,
            Err(e) => {
                return Err(tonic::Status::new(
                    tonic::Code::Internal,
                    format!("Unable to serialize{}", e),
                ))
            }
        };

        let detail_obj: grpctls::AgentDetails = match serde_json::from_str(&detail_str) {
            Ok(t) => t,
            Err(e) => {
                return Err(tonic::Status::new(
                    tonic::Code::Internal,
                    format!("Unable to deserialize{}", e),
                ))
            }
        };
        let message = Some(detail_obj);

        Ok(tonic::Response::new(GuestDetailsResponse {
            mem_block_size_bytes: u,
            support_mem_hotplug_probe: v,
            agent_details: message,
        }))
    }

    async fn set_guest_date_time(
        &self,
        req: tonic::Request<SetGuestDateTimeRequest>,
    ) -> Result<tonic::Response<()>, tonic::Status> {
        let internal = req.into_inner();

        super::do_set_guest_date_time(internal.sec, internal.usec)
            .map_err(|e| tonic::Status::new(tonic::Code::Internal, format!("{:?}", e)))?;

        Ok(tonic::Response::new(()))
    }

    async fn copy_file(
        &self,
        req: tonic::Request<CopyFileRequest>,
    ) -> Result<tonic::Response<()>, tonic::Status> {
        info!(sl!(), "grpctls: reseed_random_dev req: {:?}", req);
        let internal = req.into_inner();
        let mut ttrpc_req = agent::CopyFileRequest::new();

        ttrpc_req.set_path(internal.path);
        ttrpc_req.set_file_size(internal.file_size);
        ttrpc_req.set_file_mode(internal.file_mode);
        ttrpc_req.set_dir_mode(internal.dir_mode);
        ttrpc_req.set_uid(internal.uid);
        ttrpc_req.set_gid(internal.gid);
        ttrpc_req.set_offset(internal.offset);
        ttrpc_req.set_data(internal.data);

        super::do_copy_file(&ttrpc_req)
            .map_err(|e| tonic::Status::new(tonic::Code::Internal, format!("{:?}", e)))?;

        Ok(tonic::Response::new(()))
    }

    async fn get_oom_event(
        &self,
        _req: tonic::Request<GetOomEventRequest>,
    ) -> Result<tonic::Response<OomEvent>, tonic::Status> {
        let sandbox = self.sandbox.clone();
        let s = sandbox.lock().await;
        let event_rx = &s.event_rx.clone();
        let mut event_rx = event_rx.lock().await;
        drop(s);
        drop(sandbox);

        if let Some(container_id) = event_rx.recv().await {
            info!(sl!(), "get_oom_event return {}", &container_id);

            return Ok(tonic::Response::new(OomEvent { container_id }));
        }

        Err(tonic::Status::new(tonic::Code::Internal, String::new()))
    }
}

#[tonic::async_trait]
impl grpctls::health_server::Health for HealthService {
    async fn check(
        &self,
        _req: tonic::Request<CheckRequest>,
    ) -> Result<tonic::Response<HealthCheckResponse>, tonic::Status> {
        let resp = HealthCheckResponse {
            status: health_check_response::ServingStatus::Serving.into(),
        };

        Ok(tonic::Response::new(resp))
    }

    async fn version(
        &self,
        req: tonic::Request<CheckRequest>,
    ) -> Result<tonic::Response<VersionCheckResponse>, tonic::Status> {
        info!(sl!(), "version {:?}", req);
        Ok(tonic::Response::new(VersionCheckResponse {
            agent_version: AGENT_VERSION.to_string(),
            grpc_version: API_VERSION.to_string(),
        }))
    }
}

fn from_file(file_path: &str) -> Result<String> {
    let file_content = fs::read_to_string(file_path)
        .map_err(|e| anyhow!("Read {:?} file failed: {:?}", file_path, e))?;

    Ok(file_content)
}

fn convert_routes(route_str: String) -> Result<Vec<types::Route>, io::Error> {
    let g_route: Vec<types::Route> = serde_json::from_str(&route_str).unwrap();
    Ok(g_route)
}

fn convert_interface(interface_str: String) -> Result<Vec<types::Interface>, io::Error> {
    let g_interface: Vec<types::Interface> = serde_json::from_str(&interface_str).unwrap();
    Ok(g_interface)
}

fn convert_shared_mounts(vec_mount_str: String) -> Result<Vec<agent::SharedMount>, io::Error> {
    let shared_mounts: Vec<agent::SharedMount> = serde_json::from_str(&vec_mount_str).unwrap();
    Ok(shared_mounts)
}

pub async fn grpcstart(
    s: Arc<Mutex<Sandbox>>,
    server_address: &str,
    init_mode: bool,
) -> Result<impl futures::Future<Output = Result<(), tonic::transport::Error>>> {
    let sec_agent = AgentService {
        sandbox: s.clone(),
        init_mode,
    };
    let sec_svc = grpctls::agent_service_server::AgentServiceServer::new(sec_agent);

    let health_service = HealthService {};
    let hservice = grpctls::health_server::HealthServer::new(health_service);

    let addr = SocketAddr::from(([0, 0, 0, 0], GRPC_TLS_SERVER_PORT));

    // Config TLS
    let cert = from_file("/run/tls-keys/server.pem")?;
    let key = from_file("/run/tls-keys/server.key")?;

    // create identity from cert and key
    let id = tonic::transport::Identity::from_pem(cert.as_bytes(), key.as_bytes());

    // Reading ca root from disk
    let pem = from_file("/run/tls-keys/ca.pem")?;

    // Create certificate
    let ca = tonic::transport::Certificate::from_pem(pem.as_bytes());

    // Create tls config
    let tls = ServerTlsConfig::new().identity(id).client_ca_root(ca);

    // Create server
    let grpc_tls = Server::builder()
        .tls_config(tls)?
        .add_service(sec_svc)
        .add_service(hservice)
        .serve(addr);

    info!(sl!(), "gRPC TLS server started"; "address" => server_address);
    Ok(grpc_tls)
}
