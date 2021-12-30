// Copyright (c) 2021 Alibaba Cloud
// Copyright (c) 2021 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::convert::TryFrom;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, RwLock};
use std::time::SystemTime;

use nix::sys::signal::Signal;
use protobuf::well_known_types::Timestamp;
use protobuf::Message;
use slog::{debug, error, info, trace, warn};
use ttrpc::error::get_rpc_status;
use ttrpc::{Code, Result, TtrpcContext};

use agent_client::{BlkioStatsEntry, MemoryData, StatsContainerResponse};
use shim_proto::task::{ProcessInfo, Status};
use shim_proto::{empty::Empty, metrics, shim, shim_ttrpc::Task};
use virtcontainers::container::State;
use virtcontainers::spec_info::ContainerType;
use virtcontainers::Sandbox;

use crate::run::{ServerAction, ServerMessage};

macro_rules! sl {
    () => {
        slog_scope::logger().new(slog::o!("source" => "task_service"))
    };
}

pub(crate) struct TaskService {
    pub pid: u32,
    pub sandbox: Arc<Mutex<Sandbox>>,
    pub shutdown_sender: Mutex<Sender<ServerMessage>>,
}

impl Task for TaskService {
    fn state(&self, _ctx: &TtrpcContext, req: shim::StateRequest) -> Result<shim::StateResponse> {
        debug!(sl!(), "====> state: {:?}", req);
        let rsp = self.do_state(req);
        debug!(sl!(), "<==== state: {:?}", rsp);
        rsp
    }

    fn create(
        &self,
        _ctx: &TtrpcContext,
        req: shim::CreateTaskRequest,
    ) -> Result<shim::CreateTaskResponse> {
        debug!(sl!(), "====> create: {:?}", req);
        let rsp = self.do_create(req);
        debug!(sl!(), "<==== create: {:?}", rsp);
        rsp
    }

    fn start(&self, _ctx: &TtrpcContext, req: shim::StartRequest) -> Result<shim::StartResponse> {
        debug!(sl!(), "====> start: {:?}", req);
        let rsp = self.do_start(req);
        debug!(sl!(), "<==== start: {:?}", rsp);
        rsp
    }

    fn delete(
        &self,
        _ctx: &TtrpcContext,
        req: shim::DeleteRequest,
    ) -> Result<shim::DeleteResponse> {
        debug!(sl!(), "====> delete: {:?}", req);
        let rsp = self.do_delete(req);
        debug!(sl!(), "<==== delete: {:?}", rsp);
        rsp
    }

    fn pids(&self, _ctx: &TtrpcContext, req: shim::PidsRequest) -> Result<shim::PidsResponse> {
        debug!(sl!(), "====> pids: {:?}", req);
        let rsp = self.do_pids(req);
        debug!(sl!(), "<==== pids: {:?}", rsp);
        rsp
    }

    fn pause(&self, _ctx: &TtrpcContext, req: shim::PauseRequest) -> Result<Empty> {
        debug!(sl!(), "====> pause: {:?}", req);
        let rsp = self.do_pause(req);
        debug!(sl!(), "<==== pause: {:?}", rsp);
        rsp
    }

    fn resume(&self, _ctx: &TtrpcContext, req: shim::ResumeRequest) -> Result<Empty> {
        debug!(sl!(), "====> resume: {:?}", req);
        let rsp = self.do_resume(req);
        debug!(sl!(), "<==== resume: {:?}", rsp);
        rsp
    }

    fn checkpoint(&self, _ctx: &TtrpcContext, _req: shim::CheckpointTaskRequest) -> Result<Empty> {
        Err(get_rpc_status(
            Code::NOT_FOUND,
            "/containerd.task.v2.Task/Checkpoint is not supported".to_string(),
        ))
    }

    fn kill(&self, _ctx: &TtrpcContext, req: shim::KillRequest) -> Result<Empty> {
        debug!(sl!(), "====> kill: {:?}", req);
        let rsp = self.do_kill(req);
        debug!(sl!(), "<==== kill: {:?}", rsp);
        rsp
    }

    fn exec(&self, _ctx: &TtrpcContext, req: shim::ExecProcessRequest) -> Result<Empty> {
        debug!(sl!(), "====> exec: {:?}", req);
        let rsp = self.do_exec(req);
        debug!(sl!(), "<==== exec: {:?}", rsp);
        rsp
    }

    fn resize_pty(&self, _ctx: &TtrpcContext, req: shim::ResizePtyRequest) -> Result<Empty> {
        debug!(sl!(), "====> resize_pty: {:?}", req);
        let rsp = self.do_resize_pty(req);
        debug!(sl!(), "<==== resize_pty: {:?}", rsp);
        rsp
    }

    fn close_io(&self, _ctx: &TtrpcContext, req: shim::CloseIORequest) -> Result<Empty> {
        debug!(sl!(), "====> close_io: {:?}", req);
        let rsp = self.do_close_io(req);
        debug!(sl!(), "<==== close_io: {:?}", rsp);
        rsp
    }

    fn update(&self, _ctx: &TtrpcContext, req: shim::UpdateTaskRequest) -> Result<Empty> {
        debug!(sl!(), "====> update: {:?}", req);
        let rsp = self.do_update(req);
        debug!(sl!(), "<==== update: {:?}", rsp);
        rsp
    }

    fn wait(&self, _ctx: &TtrpcContext, req: shim::WaitRequest) -> Result<shim::WaitResponse> {
        debug!(sl!(), "====> wait: {:?}", req);
        let rsp = self.do_wait(req);
        debug!(sl!(), "<==== wait: {:?}", rsp);
        rsp
    }

    fn stats(&self, _ctx: &TtrpcContext, req: shim::StatsRequest) -> Result<shim::StatsResponse> {
        debug!(sl!(), "====> stats: {:?}", req);
        let rsp = self.do_stats(req);
        debug!(sl!(), "<==== stats: {:?}", rsp);
        rsp
    }

    fn connect(
        &self,
        _ctx: &TtrpcContext,
        req: shim::ConnectRequest,
    ) -> Result<shim::ConnectResponse> {
        debug!(sl!(), "====> connect: {:?}", req);
        let rsp = shim::ConnectResponse {
            shim_pid: self.pid,
            ..Default::default()
        };
        debug!(sl!(), "<==== connect: {:?}", rsp);
        Ok(rsp)
    }

    fn shutdown(&self, _ctx: &TtrpcContext, req: shim::ShutdownRequest) -> Result<Empty> {
        debug!(sl!(), "====> shutdown: {:?}", req);

        let mut sandbox = self.sandbox.lock().expect("poisoned sandbox lock");

        if sandbox.containers.is_empty() {
            sandbox.try_stop_and_delete();
            let msg = ServerMessage::new(ServerAction::ShutdownForce);
            self.shutdown_sender
                .lock()
                .expect("poisoned shutdown_sender lock")
                .send(msg)
                .map_err(|e| {
                    get_rpc_status(
                        Code::INTERNAL,
                        format!("Failed to shutdown with error: {:?}", e),
                    )
                })?;
        } else {
            debug!(
                sl!(),
                "<==== shutdown: cannot shutdown shim, containers not empty"
            );
        }

        Ok(Empty::new())
    }
}

impl TaskService {
    fn do_state(&self, req: shim::StateRequest) -> Result<shim::StateResponse> {
        let cid = req.get_id();
        let eid = req.get_exec_id();

        let mut sandbox = self.sandbox.lock().expect("poisoned sandbox lock");
        let c = sandbox.find_container(cid).map_err(|e| {
            get_rpc_status(
                Code::INTERNAL,
                format!("failed to find container {}  error {}", cid, e),
            )
        })?;
        // unlock the sandbox
        drop(sandbox);

        let container = c.lock().expect("poisoned container lock");
        let bundle = container.config.bundle_path.clone();
        if eid.is_empty() {
            let status = to_status(container.common_process.state);

            let (exit_code, exit_time) = exit_info(container.common_process.status.clone());

            let stdin = container.common_process.stdin.clone().unwrap_or_default();
            let stdout = container.common_process.stdout.clone().unwrap_or_default();
            let stderr = container.common_process.stderr.clone().unwrap_or_default();
            let terminal = container.common_process.terminal;
            Ok(shim::StateResponse {
                id: cid.into(),
                bundle,
                pid: self.pid,
                status,
                stdin,
                stdout,
                stderr,
                terminal,
                exit_status: exit_code,
                exited_at: exit_time.into(),
                ..Default::default()
            })
        } else if let Some(exec) = container.processes.get(eid) {
            let status = to_status(exec.state);
            let (exit_code, exit_time) = exit_info(exec.common_process.status.clone());
            let stdin = exec.common_process.stdin.clone().unwrap_or_default();
            let stdout = exec.common_process.stdout.clone().unwrap_or_default();
            let stderr = exec.common_process.stderr.clone().unwrap_or_default();
            let terminal = exec.common_process.terminal;
            Ok(shim::StateResponse {
                id: cid.into(),
                bundle,
                pid: self.pid,
                status,
                stdin,
                stdout,
                stderr,
                terminal,
                exit_status: exit_code,
                exited_at: exit_time.into(),
                ..Default::default()
            })
        } else {
            Err(get_rpc_status(
                Code::INTERNAL,
                format!("failed to find exec id {}", eid),
            ))
        }
    }

    fn do_create(&self, req: shim::CreateTaskRequest) -> Result<shim::CreateTaskResponse> {
        let mut sandbox = self.sandbox.lock().expect("poisoned sandbox lock");

        let mounts = req
            .get_rootfs()
            .iter()
            .map(|m| virtcontainers::container::Mount {
                fs_type: m.get_field_type().into(),
                source: m.get_source().into(),
                destination: m.get_target().into(),
                options: m.get_options().into(),
                ..Default::default()
            })
            .collect();

        let stdin = (!req.stdin.is_empty()).then(|| req.stdin.clone());
        let stdout = (!req.stdout.is_empty()).then(|| req.stdout.clone());
        let stderr = (!req.stderr.is_empty()).then(|| req.stderr.clone());

        sandbox
            .create_container(
                req.get_id(),
                stdin,
                stdout,
                stderr,
                req.get_terminal(),
                req.get_bundle(),
                mounts,
            )
            .map_err(|e| {
                error!(sl!(), "create container err. {:?}", e);
                get_rpc_status(
                    Code::INTERNAL,
                    format!("Failed to Create Container: {:?}", e),
                )
            })?;

        let rsp = shim::CreateTaskResponse {
            pid: self.pid,
            ..Default::default()
        };
        Ok(rsp)
    }

    fn do_start(&self, req: shim::StartRequest) -> Result<shim::StartResponse> {
        let cid = req.get_id();
        let exec_id = req.get_exec_id();
        let mut sandbox = self.sandbox.lock().expect("poisoned sandbox lock");

        info!(
            sl!(),
            "try to start container {} process {} in shimv2", cid, exec_id
        );
        sandbox.start_container(cid, exec_id).map_err(|e| {
            get_rpc_status(
                Code::INTERNAL,
                format!(
                    "Failed to Start Container {} process {} with error: {:?}",
                    cid, exec_id, e
                ),
            )
        })?;

        let rsp = shim::StartResponse {
            pid: self.pid,
            ..Default::default()
        };

        info!(
            sl!(),
            "start container {} process {} successfully", cid, exec_id
        );
        Ok(rsp)
    }

    fn do_delete(&self, req: shim::DeleteRequest) -> Result<shim::DeleteResponse> {
        let cid = req.get_id();
        let exec_id = req.get_exec_id();
        let mut sandbox = self.sandbox.lock().expect("poisoned sandbox lock");

        sandbox.remove_container(cid, exec_id).map_err(|e| {
            get_rpc_status(
                Code::INTERNAL,
                format!(
                    "Failed to delete container {} process {} with error {:?}",
                    cid, exec_id, e
                ),
            )
        })?;

        let rsp = shim::DeleteResponse {
            pid: self.pid,
            ..Default::default()
        };

        Ok(rsp)
    }

    fn do_pids(&self, _req: shim::PidsRequest) -> Result<shim::PidsResponse> {
        let p_info = ProcessInfo {
            pid: self.pid,
            ..Default::default()
        };

        let rsp = shim::PidsResponse {
            processes: vec![p_info].into(),
            ..Default::default()
        };

        Ok(rsp)
    }

    fn do_pause(&self, req: shim::PauseRequest) -> Result<Empty> {
        let cid = req.get_id();
        let mut sandbox = self.sandbox.lock().expect("poisoned sandbox lock");

        sandbox.pause_container(cid).map_err(|e| {
            get_rpc_status(
                Code::INTERNAL,
                format!("Failed to pause container {} with error {:?}", cid, e),
            )
        })?;
        Ok(Empty::new())
    }

    fn do_resume(&self, req: shim::ResumeRequest) -> Result<Empty> {
        let cid = req.get_id();
        let mut sandbox = self.sandbox.lock().expect("poisoned sandbox lock");

        sandbox.resume_container(cid).map_err(|e| {
            get_rpc_status(
                Code::INTERNAL,
                format!("Failed to resume container {} with error {:?}", cid, e),
            )
        })?;
        Ok(Empty::new())
    }

    fn do_kill(&self, req: shim::KillRequest) -> Result<Empty> {
        let cid = req.get_id();
        let all = req.get_all();
        let eid = req.get_exec_id();
        let signal: Signal = Signal::try_from(req.get_signal() as i32).map_err(|e| {
            get_rpc_status(
                Code::INTERNAL,
                format!(
                    "Failed to get kill signal {} with error: {:?}",
                    req.get_signal(),
                    e
                ),
            )
        })?;

        let mut sandbox = self.sandbox.lock().expect("poisoned sandbox lock");

        sandbox.signal_process(cid, eid, signal, all).map_err(|e| {
            get_rpc_status(
                Code::INTERNAL,
                format!(
                    "Failed to Kill Container {} process {} by Signal {} with error: {:?}",
                    cid, eid, signal, e
                ),
            )
        })?;

        Ok(Empty::new())
    }

    fn do_exec(&self, req: shim::ExecProcessRequest) -> Result<Empty> {
        let cid = req.get_id();
        let exec_id = req.get_exec_id();
        let stdin = (!req.stdin.is_empty()).then(|| req.stdin.clone());
        let stdout = (!req.stdout.is_empty()).then(|| req.stdout.clone());
        let stderr = (!req.stderr.is_empty()).then(|| req.stderr.clone());
        let terminal = req.get_terminal();

        //FIXME
        if req.get_spec().get_type_url().is_empty() {
            return Err(get_rpc_status(
                Code::INVALID_ARGUMENT,
                "Failed to exec Container: Invalid argument",
            ));
        }

        let p = serde_json::from_slice::<oci_spec::runtime::Process>(req.get_spec().get_value()).map_err(|e| {
                get_rpc_status(
                    Code::INTERNAL,
                    format!("Failed to deserialize process for container {} process {} with error: {:?}", cid, exec_id, e),
                )
            })?;

        let mut sandbox = self.sandbox.lock().expect("poisoned sandbox lock");
        sandbox
            .create_exec(cid, exec_id, stdin, stdout, stderr, terminal, p)
            .map_err(|e| {
                get_rpc_status(
                    Code::INTERNAL,
                    format!(
                        "Failed to exec container {} process {} with error: {:?}",
                        cid, exec_id, e
                    ),
                )
            })?;
        Ok(Empty::new())
    }

    fn do_resize_pty(&self, req: shim::ResizePtyRequest) -> Result<Empty> {
        let cid = req.get_id();
        let exec_id = req.get_exec_id();
        let height = req.get_height();
        let width = req.get_width();

        let mut sandbox = self.sandbox.lock().expect("poisoned sandbox lock");

        sandbox
            .winsize_process(cid, exec_id, height, width)
            .map_err(|e| {
                get_rpc_status(
                    Code::INTERNAL,
                    format!(
                        "Failed to resize pty for container {} process {} with error: {:?}",
                        cid, exec_id, e
                    ),
                )
            })?;
        Ok(Empty::new())
    }

    fn do_close_io(&self, req: shim::CloseIORequest) -> Result<Empty> {
        let cid = req.get_id();
        let exec_id = req.get_exec_id();
        let mut sandbox = self.sandbox.lock().expect("poisoned sandbox lock");
        sandbox.close_io(cid, exec_id).map_err(|e| {
            get_rpc_status(
                Code::INTERNAL,
                format!(
                    "Failed to close io for container {} process {} with error: {:?}",
                    cid, exec_id, e
                ),
            )
        })?;
        Ok(Empty::new())
    }

    fn do_update(&self, req: shim::UpdateTaskRequest) -> Result<Empty> {
        let cid = req.get_id();

        //FIXME
        if req.get_resources().get_type_url().is_empty() {
            return Err(get_rpc_status(
                Code::INVALID_ARGUMENT,
                "Failed to update Container: Invalid argument",
            ));
        }
        let resource = serde_json::from_slice::<oci_spec::runtime::LinuxResources>(
            req.get_resources().get_value(),
        )
        .map_err(|e| {
            get_rpc_status(
                Code::INTERNAL,
                format!(
                    "Failed to deserialize resource for container {}  with error: {:?}",
                    cid, e
                ),
            )
        })?;

        let mut sandbox = self.sandbox.lock().expect("poisoned sandbox lock");
        sandbox.update_container(cid, &resource).map_err(|e| {
            get_rpc_status(
                Code::INTERNAL,
                format!("Failed to update container {} with error: {:?}", cid, e),
            )
        })?;
        Ok(Empty::new())
    }

    fn do_wait(&self, req: shim::WaitRequest) -> Result<shim::WaitResponse> {
        let cid = req.get_id();
        let exec_id = req.get_exec_id();

        let mut sandbox = self.sandbox.lock().expect("poisoned sandbox lock");

        // In the corner case, containerd may do wait after the sandbox stopped and lead to leakage.
        if sandbox.get_state() == State::Stopped {
            info!(
                sl!(),
                "wait container {} process {} return empty, sandbox stopped", cid, exec_id
            );
            return Ok(shim::WaitResponse::new());
        }

        let (exit_channel, exist_status) =
            sandbox.fetch_exit_channel(cid, exec_id).map_err(|e| {
                info!(
                    sl!(),
                    "failed to fetch exit channel for container {} process {} with error: {:?}",
                    cid,
                    exec_id,
                    e
                );
                get_rpc_status(
                    Code::INTERNAL,
                    format!(
                        "Failed to fetch exit channel for container {} process {} with error: {:?}",
                        cid, exec_id, e
                    ),
                )
            })?;

        // unlock the sandbox
        drop(sandbox);

        info!(sl!(), "wait on container {} process {} exit", cid, exec_id);

        // the wait thread would hang here until the container process exited.
        // ignore channel send error, which means the container process already exited.
        exit_channel
            .send(())
            .map_err(|e| {
                warn!(
                    sl!(),
                    "failed to send to exit channel for container {} process {} with error: {:?}",
                    cid,
                    exec_id,
                    e
                )
            })
            .ok();
        info!(sl!(), "container {} process {} exited", cid, exec_id);

        let mut sandbox = self.sandbox.lock().expect("poisoned sandbox lock");

        let container = sandbox.find_container(cid).map_err(|e| {
            get_rpc_status(
                Code::INTERNAL,
                format!("failed to find container {} with error: {:?}", cid, e),
            )
        })?;

        let container_type = container
            .lock()
            .expect("poisoned container lock")
            .container_type();

        if container_type == ContainerType::PodSandbox && (exec_id.is_empty() || cid == exec_id) {
            info!(sl!(), "try to stop sandbox {}", cid);
            sandbox.try_stop_and_delete();
        } else {
            info!(sl!(), "try to stop container {} process {}", cid, exec_id);
            sandbox.stop_container(cid, exec_id, true).map_err(|e| {
                get_rpc_status(
                    Code::INTERNAL,
                    format!(
                        "failed to stop container {} process {} with error: {:?}",
                        cid, exec_id, e
                    ),
                )
            })?;
        }

        // unlock the sandbox
        drop(sandbox);

        let (exit_code, exit_time) = exit_info(exist_status);
        let rsp = shim::WaitResponse {
            exit_status: exit_code,
            exited_at: exit_time.into(),
            ..Default::default()
        };

        Ok(rsp)
    }

    fn do_stats(&self, req: shim::StatsRequest) -> Result<shim::StatsResponse> {
        let cid = req.get_id();

        let mut sandbox = self.sandbox.lock().expect("poisoned sandbox lock");

        let stats = sandbox.stats_container(cid).map_err(|e| {
            info!(sl!(), "fail to stats container {}", e);

            get_rpc_status(
                Code::INTERNAL,
                format!("Failed to stats container with error: {:?} ", e),
            )
        })?;
        // unlock the sandbox
        drop(sandbox);

        let metric = to_metric(stats);
        trace!(sl!(), "stats to metric {:?}", metric);
        let metric_bytes = metric.write_to_bytes().map_err(|e| {
            get_rpc_status(
                Code::INTERNAL,
                format!("Failed to stats container with error: {:?} ", e),
            )
        })?;

        let rsp = shim::StatsResponse {
            stats: Some(::protobuf::well_known_types::Any {
                type_url: metric.descriptor().full_name().into(),
                value: metric_bytes,
                ..Default::default()
            })
            .into(),
            ..Default::default()
        };

        Ok(rsp)
    }
}

fn to_metric(stats: StatsContainerResponse) -> metrics::Metrics {
    let mut metric = metrics::Metrics::new();

    if let Some(cg_stats) = stats.cgroup_stats {
        if let Some(cpu) = cg_stats.cpu_stats {
            // set prototbuf cpu stat
            let mut p_cpu = metrics::CPUStat::new();
            if let Some(usage) = cpu.cpu_usage {
                let p_usage = metrics::CPUUsage {
                    total: usage.total_usage,
                    kernel: usage.usage_in_kernelmode,
                    user: usage.usage_in_usermode,
                    per_cpu: usage.percpu_usage,
                    ..Default::default()
                };
                // set protobuf cpu usage
                p_cpu.set_usage(p_usage);
            }

            if let Some(throttle) = cpu.throttling_data {
                let p_throttle = metrics::Throttle {
                    periods: throttle.periods,
                    throttled_periods: throttle.throttled_periods,
                    throttled_time: throttle.throttled_time,
                    ..Default::default()
                };
                // set protobuf cpu usage
                p_cpu.set_throttling(p_throttle);
            }

            metric.set_cpu(p_cpu);
        }

        if let Some(m_stats) = cg_stats.memory_stats {
            let mut p_m = metrics::MemoryStat::new();
            p_m.set_cache(m_stats.cache);
            // memory usage
            if let Some(m_data) = m_stats.usage {
                let p_m_entry = to_memory_entry(m_data);
                p_m.set_usage(p_m_entry);
            }
            // memory swap_usage
            if let Some(m_data) = m_stats.swap_usage {
                let p_m_entry = to_memory_entry(m_data);
                p_m.set_swap(p_m_entry);
            }
            // memory kernel_usage
            if let Some(m_data) = m_stats.kernel_usage {
                let p_m_entry = to_memory_entry(m_data);
                p_m.set_kernel(p_m_entry);
            }

            for (k, v) in m_stats.stats {
                match k.as_str() {
                    "dirty" => p_m.set_dirty(v),
                    "rss" => p_m.set_rss(v),
                    "rss_huge" => p_m.set_rss_huge(v),
                    "mapped_file" => p_m.set_mapped_file(v),
                    "writeback" => p_m.set_writeback(v),
                    "pg_pg_in" => p_m.set_pg_pg_in(v),
                    "pg_pg_out" => p_m.set_pg_pg_out(v),
                    "pg_fault" => p_m.set_pg_fault(v),
                    "pg_maj_fault" => p_m.set_pg_maj_fault(v),
                    "inactive_file" => p_m.set_inactive_file(v),
                    "inactive_anon" => p_m.set_inactive_anon(v),
                    "active_file" => p_m.set_active_file(v),
                    "unevictable" => p_m.set_unevictable(v),
                    "hierarchical_memory_limit" => p_m.set_hierarchical_memory_limit(v),
                    "hierarchical_swap_limit" => p_m.set_hierarchical_swap_limit(v),
                    "total_cache" => p_m.set_total_cache(v),
                    "total_rss" => p_m.set_total_rss(v),
                    "total_mapped_file" => p_m.set_total_mapped_file(v),
                    "total_dirty" => p_m.set_total_dirty(v),

                    "total_pg_pg_in" => p_m.set_total_pg_pg_in(v),
                    "total_pg_pg_out" => p_m.set_total_pg_pg_out(v),
                    "total_pg_fault" => p_m.set_total_pg_fault(v),
                    "total_pg_maj_fault" => p_m.set_total_pg_maj_fault(v),
                    "total_inactive_file" => p_m.set_total_inactive_file(v),
                    "total_inactive_anon" => p_m.set_total_inactive_anon(v),
                    "total_active_file" => p_m.set_total_active_file(v),
                    "total_unevictable" => p_m.set_total_unevictable(v),
                    _ => {
                        info!(sl!(), "unknown stats info {}:{}", k, v)
                    }
                }
            }
            metric.set_memory(p_m);
        }

        if let Some(pid_stats) = cg_stats.pids_stats {
            let p_pid = metrics::PidsStat {
                limit: pid_stats.limit,
                current: pid_stats.current,
                ..Default::default()
            };
            metric.set_pids(p_pid);
        }

        if let Some(blk_stats) = cg_stats.blkio_stats {
            let p_blk_stats = metrics::BlkIOStat {
                io_service_bytes_recursive: to_blkio_entries(blk_stats.io_service_bytes_recursive),
                io_serviced_recursive: to_blkio_entries(blk_stats.io_serviced_recursive),
                io_queued_recursive: to_blkio_entries(blk_stats.io_queued_recursive),
                io_service_time_recursive: to_blkio_entries(blk_stats.io_service_time_recursive),
                io_wait_time_recursive: to_blkio_entries(blk_stats.io_wait_time_recursive),
                io_merged_recursive: to_blkio_entries(blk_stats.io_merged_recursive),
                io_time_recursive: to_blkio_entries(blk_stats.io_time_recursive),
                sectors_recursive: to_blkio_entries(blk_stats.sectors_recursive),
                ..Default::default()
            };

            metric.set_blkio(p_blk_stats);
        }

        if !cg_stats.hugetlb_stats.is_empty() {
            let p_huge = cg_stats
                .hugetlb_stats
                .iter()
                .map(|(k, v)| metrics::HugetlbStat {
                    pagesize: k.into(),
                    max: v.max_usage,
                    usage: v.usage,
                    failcnt: v.failcnt,
                    ..Default::default()
                })
                .collect();
            metric.set_hugetlb(p_huge);
        }
    }

    let net_stats = stats.network_stats;
    if !net_stats.is_empty() {
        let p_net = net_stats
            .iter()
            .map(|v| metrics::NetworkStat {
                name: v.name.clone(),
                tx_bytes: v.tx_bytes,
                tx_packets: v.tx_packets,
                tx_errors: v.tx_errors,
                tx_dropped: v.tx_dropped,
                rx_bytes: v.rx_bytes,
                rx_packets: v.rx_packets,
                rx_errors: v.rx_errors,
                rx_dropped: v.rx_dropped,
                ..Default::default()
            })
            .collect();
        metric.set_network(p_net);
    }

    metric
}

fn to_blkio_entries(entry: Vec<BlkioStatsEntry>) -> ::protobuf::RepeatedField<metrics::BlkIOEntry> {
    entry
        .iter()
        .map(|b| metrics::BlkIOEntry {
            op: b.op.clone(),
            value: b.value,
            major: b.major,
            minor: b.minor,
            ..Default::default()
        })
        .collect()
}

fn to_status(state: virtcontainers::container::State) -> shim_proto::task::Status {
    match state {
        State::Ready => Status::CREATED,
        State::Running => Status::RUNNING,
        State::Stopped => Status::STOPPED,
        State::Paused => Status::PAUSED,
    }
}

fn to_timestamp(time: SystemTime) -> Option<Timestamp> {
    match time.duration_since(SystemTime::UNIX_EPOCH) {
        Err(_) => None,
        Ok(n) => Some(Timestamp {
            seconds: n.as_secs() as i64,
            nanos: n.subsec_nanos() as i32,
            ..Default::default()
        }),
    }
}

fn to_memory_entry(memory_data: MemoryData) -> metrics::MemoryEntry {
    metrics::MemoryEntry {
        usage: memory_data.usage,
        limit: memory_data.limit,
        failcnt: memory_data.failcnt,
        max: memory_data.max_usage,
        ..Default::default()
    }
}

fn exit_info(
    exist_status: Arc<RwLock<virtcontainers::container::ExitStatus>>,
) -> (u32, Option<Timestamp>) {
    let status = exist_status.read().expect("poisoned exit status lock");
    (status.exit_code as u32, to_timestamp(status.exit_time))
}
