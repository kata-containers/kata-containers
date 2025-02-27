// Copyright (c) 2019 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

use async_trait::async_trait;
use rustjail::{pipestream::PipeStream, process::StreamType};
use tokio::io::{AsyncReadExt, AsyncWriteExt, ReadHalf};
use tokio::sync::Mutex;

use std::convert::TryFrom;
use std::ffi::{CString, OsStr};
use std::fmt::Debug;
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;
use ttrpc::{
    self,
    error::get_rpc_status,
    r#async::{Server as TtrpcServer, TtrpcContext},
};

use anyhow::{anyhow, Context, Result};
use cgroups::freezer::FreezerState;
use oci::{Hooks, LinuxNamespace, Spec};
use oci_spec::runtime as oci;
use protobuf::MessageField;
use protocols::agent::{
    AddSwapRequest, AgentDetails, CopyFileRequest, GetIPTablesRequest, GetIPTablesResponse,
    GuestDetailsResponse, Interfaces, Metrics, OOMEvent, ReadStreamResponse, Routes,
    SetIPTablesRequest, SetIPTablesResponse, StatsContainerResponse, VolumeStatsRequest,
    WaitProcessResponse, WriteStreamResponse,
};
use protocols::csi::{
    volume_usage::Unit as VolumeUsage_Unit, VolumeCondition, VolumeStatsResponse, VolumeUsage,
};
use protocols::empty::Empty;
use protocols::health::{
    health_check_response::ServingStatus as HealthCheckResponse_ServingStatus, HealthCheckResponse,
    VersionCheckResponse,
};
use protocols::types::Interface;
use protocols::{agent_ttrpc_async as agent_ttrpc, health_ttrpc_async as health_ttrpc};
use rustjail::cgroups::notifier;
use rustjail::container::{BaseContainer, Container, LinuxContainer, SYSTEMD_CGROUP_PATH_FORMAT};
use rustjail::mount::parse_mount_table;
use rustjail::process::Process;
use rustjail::specconv::CreateOpts;

use nix::errno::Errno;
use nix::mount::MsFlags;
use nix::sys::{stat, statfs};
use nix::unistd::{self, Pid};
use rustjail::process::ProcessOperations;

use crate::cdh;
use crate::device::block_device_handler::get_virtio_blk_pci_device_name;
use crate::device::network_device_handler::wait_for_net_interface;
use crate::device::{add_devices, handle_cdi_devices, update_env_pci};
use crate::features::get_build_features;
use crate::image::KATA_IMAGE_WORK_DIR;
use crate::linux_abi::*;
use crate::metrics::get_metrics;
use crate::mount::baremount;
use crate::namespace::{NSTYPEIPC, NSTYPEPID, NSTYPEUTS};
use crate::network::setup_guest_dns;
use crate::passfd_io;
use crate::pci;
use crate::random;
use crate::sandbox::Sandbox;
use crate::storage::{add_storages, update_ephemeral_mounts, STORAGE_HANDLERS};
use crate::util;
use crate::version::{AGENT_VERSION, API_VERSION};
use crate::AGENT_CONFIG;

use crate::trace_rpc_call;
use crate::tracer::extract_carrier_from_ttrpc;

#[cfg(feature = "agent-policy")]
use crate::policy::{do_set_policy, is_allowed};

#[cfg(feature = "guest-pull")]
use crate::image;

use opentelemetry::global;
use tracing::span;
use tracing_opentelemetry::OpenTelemetrySpanExt;

use tracing::instrument;

use libc::{self, c_char, c_ushort, pid_t, winsize, TIOCSWINSZ};
use std::fs;
use std::os::unix::prelude::PermissionsExt;
use std::process::{Command, Stdio};
use std::time::Duration;

use nix::unistd::{Gid, Uid};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::FileExt;
use std::path::PathBuf;

use kata_types::k8s;

pub const CONTAINER_BASE: &str = "/run/kata-containers";
const MODPROBE_PATH: &str = "/sbin/modprobe";
const TRUSTED_IMAGE_STORAGE_DEVICE: &str = "/dev/trusted_store";
/// the iptables seriers binaries could appear either in /sbin
/// or /usr/sbin, we need to check both of them
const USR_IPTABLES_SAVE: &str = "/usr/sbin/iptables-save";
const IPTABLES_SAVE: &str = "/sbin/iptables-save";
const USR_IPTABLES_RESTORE: &str = "/usr/sbin/iptables-store";
const IPTABLES_RESTORE: &str = "/sbin/iptables-restore";
const USR_IP6TABLES_SAVE: &str = "/usr/sbin/ip6tables-save";
const IP6TABLES_SAVE: &str = "/sbin/ip6tables-save";
const USR_IP6TABLES_RESTORE: &str = "/usr/sbin/ip6tables-save";
const IP6TABLES_RESTORE: &str = "/sbin/ip6tables-restore";
const KATA_GUEST_SHARE_DIR: &str = "/run/kata-containers/shared/containers/";

const ERR_CANNOT_GET_WRITER: &str = "Cannot get writer";
const ERR_INVALID_BLOCK_SIZE: &str = "Invalid block size";
const ERR_NO_LINUX_FIELD: &str = "Spec does not contain linux field";
const ERR_NO_SANDBOX_PIDNS: &str = "Sandbox does not have sandbox_pidns";

// IPTABLES_RESTORE_WAIT_SEC is the timeout value provided to iptables-restore --wait. Since we
// don't expect other writers to iptables, we don't expect contention for grabbing the iptables
// filesystem lock. Based on this, 5 seconds seems a resonable timeout period in case the lock is
// not available.
const IPTABLES_RESTORE_WAIT_SEC: u64 = 5;

// Convenience function to obtain the scope logger.
fn sl() -> slog::Logger {
    slog_scope::logger()
}

// Convenience function to wrap an error and response to ttrpc client
pub fn ttrpc_error(code: ttrpc::Code, err: impl Debug) -> ttrpc::Error {
    get_rpc_status(code, format!("{:?}", err))
}

#[cfg(not(feature = "agent-policy"))]
async fn is_allowed(_req: &impl serde::Serialize) -> ttrpc::Result<()> {
    Ok(())
}

fn same<E>(e: E) -> E {
    e
}

trait ResultToTtrpcResult<T, E: Debug>: Sized {
    fn map_ttrpc_err<R: Debug>(self, msg_builder: impl FnOnce(E) -> R) -> ttrpc::Result<T>;
    fn map_ttrpc_err_do(self, doer: impl FnOnce(&E)) -> ttrpc::Result<T> {
        self.map_ttrpc_err(|e| {
            doer(&e);
            e
        })
    }
}

impl<T, E: Debug> ResultToTtrpcResult<T, E> for Result<T, E> {
    fn map_ttrpc_err<R: Debug>(self, msg_builder: impl FnOnce(E) -> R) -> ttrpc::Result<T> {
        self.map_err(|e| ttrpc_error(ttrpc::Code::INTERNAL, msg_builder(e)))
    }
}

trait OptionToTtrpcResult<T>: Sized {
    fn map_ttrpc_err(self, code: ttrpc::Code, msg: &str) -> ttrpc::Result<T>;
}

impl<T> OptionToTtrpcResult<T> for Option<T> {
    fn map_ttrpc_err(self, code: ttrpc::Code, msg: &str) -> ttrpc::Result<T> {
        self.ok_or_else(|| ttrpc_error(code, msg))
    }
}

#[derive(Clone, Debug)]
pub struct AgentService {
    sandbox: Arc<Mutex<Sandbox>>,
    init_mode: bool,
    oma: Option<mem_agent::agent::MemAgent>,
}

impl AgentService {
    #[instrument]
    async fn do_create_container(
        &self,
        req: protocols::agent::CreateContainerRequest,
    ) -> Result<()> {
        // create the proc_io first, in case there's some error occur below, thus we can make sure
        // the io stream closed when error occur.
        let proc_io = if AGENT_CONFIG.passfd_listener_port != 0 {
            Some(passfd_io::take_io_streams(req.stdin_port, req.stdout_port, req.stderr_port).await)
        } else {
            None
        };

        let cid = req.container_id.clone();

        kata_sys_util::validate::verify_id(&cid)?;

        let use_sandbox_pidns = req.sandbox_pidns();

        let mut oci = match req.OCI.into_option() {
            Some(spec) => spec.into(),
            None => {
                error!(sl(), "no oci spec in the create container request!");
                return Err(anyhow!(nix::Error::EINVAL));
            }
        };

        let container_name = k8s::container_name(&oci);

        info!(sl(), "receive createcontainer, spec: {:?}", &oci);
        info!(
            sl(),
            "receive createcontainer, storages: {:?}", &req.storages
        );

        // Some devices need some extra processing (the ones invoked with
        // --device for instance), and that's what this call is doing. It
        // updates the devices listed in the OCI spec, so that they actually
        // match real devices inside the VM. This step is necessary since we
        // cannot predict everything from the caller.
        add_devices(&sl(), &req.devices, &mut oci, &self.sandbox).await?;

        // In guest-kernel mode some devices need extra handling. Taking the
        // GPU as an example the shim will inject CDI annotations that will
        // be used by the kata-agent to do containerEdits according to the
        // CDI spec coming from a registry that is created on the fly by UDEV
        // or other entities for a specifc device.
        // In Kata we only consider the directory "/var/run/cdi", "/etc" may be
        // readonly
        handle_cdi_devices(&sl(), &mut oci, "/var/run/cdi", AGENT_CONFIG.cdi_timeout).await?;

        cdh_handler(&mut oci).await?;

        // Both rootfs and volumes (invoked with --volume for instance) will
        // be processed the same way. The idea is to always mount any provided
        // storage to the specified MountPoint, so that it will match what's
        // inside oci.Mounts.
        // After all those storages have been processed, no matter the order
        // here, the agent will rely on rustjail (using the oci.Mounts
        // list) to bind mount all of them inside the container.
        let m = add_storages(
            sl(),
            req.storages.clone(),
            &self.sandbox,
            Some(req.container_id),
        )
        .await?;

        let mut s = self.sandbox.lock().await;
        s.container_mounts.insert(cid.clone(), m);

        update_container_namespaces(&s, &mut oci, use_sandbox_pidns)?;

        // Append guest hooks
        append_guest_hooks(&s, &mut oci)?;

        // write spec to bundle path, hooks might
        // read ocispec
        let olddir = setup_bundle(&cid, &mut oci)?;
        // restore the cwd for kata-agent process.
        defer!(unistd::chdir(&olddir).unwrap());

        // determine which cgroup driver to take and then assign to use_systemd_cgroup
        // systemd: "[slice]:[prefix]:[name]"
        // fs: "/path_a/path_b"
        // If agent is init we can't use systemd cgroup mode, no matter what the host tells us
        let cgroups_path = &oci
            .linux()
            .as_ref()
            .and_then(|linux| linux.cgroups_path().as_ref())
            .map(|cgrps_path| cgrps_path.display().to_string())
            .unwrap_or_default();

        let use_systemd_cgroup = if self.init_mode {
            false
        } else {
            SYSTEMD_CGROUP_PATH_FORMAT.is_match(cgroups_path)
        };

        let opts = CreateOpts {
            cgroup_name: "".to_string(),
            use_systemd_cgroup,
            no_pivot_root: s.no_pivot_root,
            no_new_keyring: false,
            spec: Some(oci.clone()),
            rootless_euid: false,
            rootless_cgroup: false,
            container_name,
        };

        let mut ctr: LinuxContainer = LinuxContainer::new(
            cid.as_str(),
            CONTAINER_BASE,
            Some(s.devcg_info.clone()),
            opts,
            &sl(),
        )?;

        let pipe_size = AGENT_CONFIG.container_pipe_size;

        let p = if let Some(p) = oci.process() {
            #[cfg(feature = "guest-pull")]
            {
                let new_p = image::get_process(p, &oci, req.storages.clone())?;
                Process::new(&sl(), &new_p, cid.as_str(), true, pipe_size, proc_io)?
            }

            #[cfg(not(feature = "guest-pull"))]
            Process::new(&sl(), p, cid.as_str(), true, pipe_size, proc_io)?
        } else {
            info!(sl(), "no process configurations!");
            return Err(anyhow!(nix::Error::EINVAL));
        };

        // if starting container failed, we will do some rollback work
        // to ensure no resources are leaked.
        if let Err(err) = ctr.start(p).await {
            error!(sl(), "failed to start container: {:?}", err);
            if let Err(e) = ctr.destroy().await {
                error!(sl(), "failed to destroy container: {:?}", e);
            }
            if let Err(e) = remove_container_resources(&mut s, &cid).await {
                error!(sl(), "failed to remove container resources: {:?}", e);
            }
            return Err(err);
        }

        s.update_shared_pidns(&ctr)?;
        s.setup_shared_mounts(&ctr, &req.shared_mounts)?;
        s.add_container(ctr);
        info!(sl(), "created container!");

        Ok(())
    }

    #[instrument]
    async fn do_start_container(&self, req: protocols::agent::StartContainerRequest) -> Result<()> {
        let mut s = self.sandbox.lock().await;
        let sid = s.id.clone();
        let cid = req.container_id;

        let ctr = s
            .get_container(&cid)
            .ok_or_else(|| anyhow!("Invalid container id"))?;
        ctr.exec().await?;

        if sid == cid {
            return Ok(());
        }

        // start oom event loop
        if let Ok(cg_path) = ctr.cgroup_manager.as_ref().get_cgroup_path("memory") {
            let rx = notifier::notify_oom(cid.as_str(), cg_path.to_string()).await?;
            s.run_oom_event_monitor(rx, cid).await;
        }

        Ok(())
    }

    #[instrument]
    async fn do_remove_container(
        &self,
        req: protocols::agent::RemoveContainerRequest,
    ) -> Result<()> {
        let cid = req.container_id;

        if req.timeout == 0 {
            let mut sandbox = self.sandbox.lock().await;
            sandbox.bind_watcher.remove_container(&cid).await;
            sandbox
                .get_container(&cid)
                .ok_or_else(|| anyhow!("Invalid container id"))?
                .destroy()
                .await?;
            remove_container_resources(&mut sandbox, &cid).await?;
            return Ok(());
        }

        // timeout != 0
        let s = self.sandbox.clone();
        let cid2 = cid.clone();
        let handle = tokio::spawn(async move {
            let mut sandbox = s.lock().await;
            sandbox.bind_watcher.remove_container(&cid2).await;
            sandbox
                .get_container(&cid2)
                .ok_or_else(|| anyhow!("Invalid container id"))?
                .destroy()
                .await
        });

        let to = Duration::from_secs(req.timeout.into());
        tokio::time::timeout(to, handle)
            .await
            .map_err(|_| anyhow!(nix::Error::ETIME))???;

        remove_container_resources(&mut *self.sandbox.lock().await, &cid).await
    }

    #[instrument]
    async fn do_exec_process(&self, req: protocols::agent::ExecProcessRequest) -> Result<()> {
        let cid = req.container_id;
        let exec_id = req.exec_id;

        info!(sl(), "do_exec_process cid: {} eid: {}", cid, exec_id);

        // create the proc_io first, in case there's some error occur below, thus we can make sure
        // the io stream closed when error occur.
        let proc_io = if AGENT_CONFIG.passfd_listener_port != 0 {
            Some(passfd_io::take_io_streams(req.stdin_port, req.stdout_port, req.stderr_port).await)
        } else {
            None
        };

        let mut sandbox = self.sandbox.lock().await;
        let mut process = req
            .process
            .into_option()
            .ok_or_else(|| anyhow!("Unable to parse process from ExecProcessRequest"))?;

        // Apply any necessary corrections for PCI addresses
        update_env_pci(&mut process.Env, &sandbox.pcimap)?;

        let pipe_size = AGENT_CONFIG.container_pipe_size;
        let ocip = process.into();
        let p = Process::new(&sl(), &ocip, exec_id.as_str(), false, pipe_size, proc_io)?;

        let ctr = sandbox
            .get_container(&cid)
            .ok_or_else(|| anyhow!("Invalid container id"))?;

        ctr.run(p).await
    }

    #[instrument]
    async fn do_signal_process(&self, req: protocols::agent::SignalProcessRequest) -> Result<()> {
        let cid = req.container_id;
        let eid = req.exec_id;

        info!(
            sl(),
            "signal process";
            "container-id" => &cid,
            "exec-id" => &eid,
            "signal" => req.signal,
        );

        let mut sig: libc::c_int = req.signal as libc::c_int;
        {
            let mut sandbox = self.sandbox.lock().await;
            let p = sandbox.find_container_process(cid.as_str(), eid.as_str())?;
            // For container initProcess, if it hasn't installed handler for "SIGTERM" signal,
            // it will ignore the "SIGTERM" signal sent to it, thus send it "SIGKILL" signal
            // instead of "SIGTERM" to terminate it.
            let proc_status_file = format!("/proc/{}/status", p.pid);
            if p.init && sig == libc::SIGTERM && !is_signal_handled(&proc_status_file, sig as u32) {
                sig = libc::SIGKILL;
            }

            match p.signal(sig) {
                Err(Errno::ESRCH) => {
                    info!(
                        sl(),
                        "signal encounter ESRCH, continue";
                        "container-id" => &cid,
                        "exec-id" => &eid,
                        "pid" => p.pid,
                        "signal" => sig,
                    );
                }
                Err(err) => return Err(anyhow!(err)),
                Ok(()) => (),
            }
        };

        if eid.is_empty() {
            // eid is empty, signal all the remaining processes in the container cgroup
            info!(
                sl(),
                "signal all the remaining processes";
                "container-id" => &cid,
                "exec-id" => &eid,
            );

            if let Err(err) = self.freeze_cgroup(&cid, FreezerState::Frozen).await {
                warn!(
                    sl(),
                    "freeze cgroup failed";
                    "container-id" => &cid,
                    "exec-id" => &eid,
                    "error" => format!("{:?}", err),
                );
            }

            let pids = self.get_pids(&cid).await?;
            for pid in pids.iter() {
                let res = unsafe { libc::kill(*pid, sig) };
                if let Err(err) = Errno::result(res).map(drop) {
                    warn!(
                        sl(),
                        "signal failed";
                        "container-id" => &cid,
                        "exec-id" => &eid,
                        "pid" => pid,
                        "error" => format!("{:?}", err),
                    );
                }
            }
            if let Err(err) = self.freeze_cgroup(&cid, FreezerState::Thawed).await {
                warn!(
                    sl(),
                    "unfreeze cgroup failed";
                    "container-id" => &cid,
                    "exec-id" => &eid,
                    "error" => format!("{:?}", err),
                );
            }
        }

        Ok(())
    }

    async fn freeze_cgroup(&self, cid: &str, state: FreezerState) -> Result<()> {
        let mut sandbox = self.sandbox.lock().await;
        let ctr = sandbox
            .get_container(cid)
            .ok_or_else(|| anyhow!("Invalid container id {}", cid))?;
        ctr.cgroup_manager.as_ref().freeze(state)
    }

    async fn get_pids(&self, cid: &str) -> Result<Vec<i32>> {
        let mut sandbox = self.sandbox.lock().await;
        let ctr = sandbox
            .get_container(cid)
            .ok_or_else(|| anyhow!("Invalid container id {}", cid))?;
        ctr.cgroup_manager.as_ref().get_pids()
    }

    #[instrument]
    async fn do_wait_process(
        &self,
        req: protocols::agent::WaitProcessRequest,
    ) -> Result<protocols::agent::WaitProcessResponse> {
        let cid = req.container_id;
        let eid = req.exec_id;
        let mut resp = WaitProcessResponse::new();

        info!(
            sl(),
            "wait process";
            "container-id" => &cid,
            "exec-id" => &eid
        );

        let pid: pid_t;
        let (exit_send, mut exit_recv) = tokio::sync::mpsc::channel(100);
        let exit_rx = {
            let mut sandbox = self.sandbox.lock().await;
            let p = sandbox.find_container_process(cid.as_str(), eid.as_str())?;

            p.exit_watchers.push(exit_send);
            pid = p.pid;

            p.exit_rx.clone()
        };

        if let Some(mut exit_rx) = exit_rx {
            info!(sl(), "cid {} eid {} waiting for exit signal", &cid, &eid);
            while exit_rx.changed().await.is_ok() {}
            info!(sl(), "cid {} eid {} received exit signal", &cid, &eid);
        }

        let mut sandbox = self.sandbox.lock().await;
        let ctr = sandbox
            .get_container(&cid)
            .ok_or_else(|| anyhow!("Invalid container id"))?;

        let p = match ctr.processes.get_mut(&pid) {
            Some(p) => p,
            None => {
                // Lost race, pick up exit code from channel
                resp.status = exit_recv
                    .recv()
                    .await
                    .ok_or_else(|| anyhow!("Failed to receive exit code"))?;

                return Ok(resp);
            }
        };

        // need to close all fd
        // ignore errors for some fd might be closed by stream
        p.cleanup_process_stream();

        resp.status = p.exit_code;
        // broadcast exit code to all parallel watchers
        for s in p.exit_watchers.iter_mut() {
            // Just ignore errors in case any watcher quits unexpectedly
            let _ = s.send(p.exit_code).await;
        }

        ctr.processes.remove(&pid);

        Ok(resp)
    }

    async fn do_write_stream(
        &self,
        req: protocols::agent::WriteStreamRequest,
    ) -> Result<protocols::agent::WriteStreamResponse> {
        let cid = req.container_id;
        let eid = req.exec_id;

        let mut resp = WriteStreamResponse::new();
        resp.set_len(req.data.len() as u32);

        // EOF of stdin
        if req.data.is_empty() {
            let mut sandbox = self.sandbox.lock().await;
            let p = sandbox.find_container_process(cid.as_str(), eid.as_str())?;
            p.close_stdin().await;
        } else {
            let writer = {
                let mut sandbox = self.sandbox.lock().await;
                let p = sandbox.find_container_process(cid.as_str(), eid.as_str())?;

                // use ptmx io
                if p.term_master.is_some() {
                    p.get_writer(StreamType::TermMaster)
                } else {
                    // use piped io
                    p.get_writer(StreamType::ParentStdin)
                }
            };

            let writer = writer.ok_or_else(|| anyhow!(ERR_CANNOT_GET_WRITER))?;
            writer.lock().await.write_all(req.data.as_slice()).await?;
        }

        Ok(resp)
    }

    async fn do_read_stream(
        &self,
        req: protocols::agent::ReadStreamRequest,
        stdout: bool,
    ) -> Result<protocols::agent::ReadStreamResponse> {
        let cid = req.container_id;
        let eid = req.exec_id;

        let term_exit_notifier;
        let reader = {
            let mut sandbox = self.sandbox.lock().await;
            let p = sandbox.find_container_process(cid.as_str(), eid.as_str())?;

            term_exit_notifier = p.term_exit_notifier.clone();

            if p.term_master.is_some() {
                p.get_reader(StreamType::TermMaster)
            } else if stdout {
                if p.parent_stdout.is_some() {
                    p.get_reader(StreamType::ParentStdout)
                } else {
                    None
                }
            } else {
                p.get_reader(StreamType::ParentStderr)
            }
        };

        let reader = reader.ok_or_else(|| anyhow!("cannot get stream reader"))?;

        tokio::select! {
            // Poll the futures in the order they appear from top to bottom
            // it is very important to avoid data loss. If there is still
            // data in the buffer and read_stream branch will return
            // Poll::Ready so that the term_exit_notifier will never polled
            // before all data were read.
            biased;
            v = read_stream(&reader, req.len as usize)  => {
                let vector = v?;

                let mut resp = ReadStreamResponse::new();
                resp.set_data(vector);

                Ok(resp)
            }
            _ = term_exit_notifier.notified() => {
                Err(anyhow!("eof"))
            }
        }
    }
}

fn mem_agent_memcgconfig_to_memcg_optionconfig(
    mc: &protocols::agent::MemAgentMemcgConfig,
) -> mem_agent::memcg::OptionConfig {
    mem_agent::memcg::OptionConfig {
        disabled: mc.disabled,
        swap: mc.swap,
        swappiness_max: mc.swappiness_max.map(|x| x as u8),
        period_secs: mc.period_secs,
        period_psi_percent_limit: mc.period_psi_percent_limit.map(|x| x as u8),
        eviction_psi_percent_limit: mc.eviction_psi_percent_limit.map(|x| x as u8),
        eviction_run_aging_count_min: mc.eviction_run_aging_count_min,
        ..Default::default()
    }
}

fn mem_agent_compactconfig_to_compact_optionconfig(
    cc: &protocols::agent::MemAgentCompactConfig,
) -> mem_agent::compact::OptionConfig {
    mem_agent::compact::OptionConfig {
        disabled: cc.disabled,
        period_secs: cc.period_secs,
        period_psi_percent_limit: cc.period_psi_percent_limit.map(|x| x as u8),
        compact_psi_percent_limit: cc.compact_psi_percent_limit.map(|x| x as u8),
        compact_sec_max: cc.compact_sec_max,
        compact_order: cc.compact_order.map(|x| x as u8),
        compact_threshold: cc.compact_threshold,
        compact_force_times: cc.compact_force_times,
        ..Default::default()
    }
}

#[async_trait]
impl agent_ttrpc::AgentService for AgentService {
    async fn create_container(
        &self,
        ctx: &TtrpcContext,
        req: protocols::agent::CreateContainerRequest,
    ) -> ttrpc::Result<Empty> {
        trace_rpc_call!(ctx, "create_container", req);
        is_allowed(&req).await?;
        self.do_create_container(req).await.map_ttrpc_err(same)?;
        Ok(Empty::new())
    }

    async fn start_container(
        &self,
        ctx: &TtrpcContext,
        req: protocols::agent::StartContainerRequest,
    ) -> ttrpc::Result<Empty> {
        trace_rpc_call!(ctx, "start_container", req);
        is_allowed(&req).await?;
        self.do_start_container(req).await.map_ttrpc_err(same)?;
        Ok(Empty::new())
    }

    async fn remove_container(
        &self,
        ctx: &TtrpcContext,
        req: protocols::agent::RemoveContainerRequest,
    ) -> ttrpc::Result<Empty> {
        trace_rpc_call!(ctx, "remove_container", req);
        is_allowed(&req).await?;
        self.do_remove_container(req).await.map_ttrpc_err(same)?;
        Ok(Empty::new())
    }

    async fn exec_process(
        &self,
        ctx: &TtrpcContext,
        req: protocols::agent::ExecProcessRequest,
    ) -> ttrpc::Result<Empty> {
        trace_rpc_call!(ctx, "exec_process", req);
        is_allowed(&req).await?;
        self.do_exec_process(req).await.map_ttrpc_err(same)?;
        Ok(Empty::new())
    }

    async fn signal_process(
        &self,
        ctx: &TtrpcContext,
        req: protocols::agent::SignalProcessRequest,
    ) -> ttrpc::Result<Empty> {
        trace_rpc_call!(ctx, "signal_process", req);
        is_allowed(&req).await?;
        self.do_signal_process(req).await.map_ttrpc_err(same)?;
        Ok(Empty::new())
    }

    async fn wait_process(
        &self,
        ctx: &TtrpcContext,
        req: protocols::agent::WaitProcessRequest,
    ) -> ttrpc::Result<WaitProcessResponse> {
        trace_rpc_call!(ctx, "wait_process", req);
        is_allowed(&req).await?;
        self.do_wait_process(req).await.map_ttrpc_err(same)
    }

    async fn update_container(
        &self,
        ctx: &TtrpcContext,
        req: protocols::agent::UpdateContainerRequest,
    ) -> ttrpc::Result<Empty> {
        trace_rpc_call!(ctx, "update_container", req);
        is_allowed(&req).await?;

        let mut sandbox = self.sandbox.lock().await;
        let ctr = sandbox
            .get_container(&req.container_id)
            .map_ttrpc_err(ttrpc::Code::INVALID_ARGUMENT, "invalid container id")?;
        if let Some(res) = req.resources.as_ref() {
            let oci_res = res.clone().into();
            ctr.set(oci_res).map_ttrpc_err(same)?;
        }

        Ok(Empty::new())
    }

    async fn stats_container(
        &self,
        ctx: &TtrpcContext,
        req: protocols::agent::StatsContainerRequest,
    ) -> ttrpc::Result<StatsContainerResponse> {
        trace_rpc_call!(ctx, "stats_container", req);
        is_allowed(&req).await?;

        let mut sandbox = self.sandbox.lock().await;
        let ctr = sandbox
            .get_container(&req.container_id)
            .map_ttrpc_err(ttrpc::Code::INVALID_ARGUMENT, "invalid container id")?;
        ctr.stats().map_ttrpc_err(same)
    }

    async fn pause_container(
        &self,
        ctx: &TtrpcContext,
        req: protocols::agent::PauseContainerRequest,
    ) -> ttrpc::Result<protocols::empty::Empty> {
        trace_rpc_call!(ctx, "pause_container", req);
        is_allowed(&req).await?;

        let mut sandbox = self.sandbox.lock().await;
        let ctr = sandbox
            .get_container(&req.container_id)
            .map_ttrpc_err(ttrpc::Code::INVALID_ARGUMENT, "invalid container id")?;
        ctr.pause().map_ttrpc_err(same)?;
        Ok(Empty::new())
    }

    async fn resume_container(
        &self,
        ctx: &TtrpcContext,
        req: protocols::agent::ResumeContainerRequest,
    ) -> ttrpc::Result<protocols::empty::Empty> {
        trace_rpc_call!(ctx, "resume_container", req);
        is_allowed(&req).await?;

        let mut sandbox = self.sandbox.lock().await;
        let ctr = sandbox
            .get_container(&req.container_id)
            .map_ttrpc_err(ttrpc::Code::INVALID_ARGUMENT, "invalid container id")?;
        ctr.resume().map_ttrpc_err(same)?;
        Ok(Empty::new())
    }

    async fn remove_stale_virtiofs_share_mounts(
        &self,
        ctx: &TtrpcContext,
        req: protocols::agent::RemoveStaleVirtiofsShareMountsRequest,
    ) -> ttrpc::Result<Empty> {
        trace_rpc_call!(ctx, "remove_stale_virtiofs_share_mounts", req);
        is_allowed(&req).await?;
        let mount_infos = parse_mount_table("/proc/self/mountinfo").map_ttrpc_err(same)?;
        for m in &mount_infos {
            if m.mount_point.starts_with(KATA_GUEST_SHARE_DIR) {
                // stat the mount point, virtiofs daemon will remove the stale cache and release the fds if the mount point doesn't exist any more.
                // More details in https://github.com/kata-containers/kata-containers/issues/6455#issuecomment-1477137277
                match stat::stat(Path::new(&m.mount_point)) {
                    Ok(_) => info!(sl(), "stat {} success", m.mount_point),
                    Err(e) => info!(sl(), "stat {} failed: {}", m.mount_point, e),
                }
            }
        }

        Ok(Empty::new())
    }

    async fn write_stdin(
        &self,
        _ctx: &TtrpcContext,
        req: protocols::agent::WriteStreamRequest,
    ) -> ttrpc::Result<WriteStreamResponse> {
        is_allowed(&req).await?;
        self.do_write_stream(req).await.map_ttrpc_err(same)
    }

    async fn read_stdout(
        &self,
        _ctx: &TtrpcContext,
        req: protocols::agent::ReadStreamRequest,
    ) -> ttrpc::Result<ReadStreamResponse> {
        is_allowed(&req).await?;
        self.do_read_stream(req, true).await.map_ttrpc_err(same)
    }

    async fn read_stderr(
        &self,
        _ctx: &TtrpcContext,
        req: protocols::agent::ReadStreamRequest,
    ) -> ttrpc::Result<ReadStreamResponse> {
        is_allowed(&req).await?;
        self.do_read_stream(req, false).await.map_ttrpc_err(same)
    }

    async fn close_stdin(
        &self,
        ctx: &TtrpcContext,
        req: protocols::agent::CloseStdinRequest,
    ) -> ttrpc::Result<Empty> {
        // The stdin will be closed when EOF is got in rpc `write_stdin`[runtime-rs]
        // so this rpc will not be called anymore by runtime-rs.

        trace_rpc_call!(ctx, "close_stdin", req);
        is_allowed(&req).await?;

        let cid = req.container_id;
        let eid = req.exec_id;
        let mut sandbox = self.sandbox.lock().await;

        let p = sandbox
            .find_container_process(cid.as_str(), eid.as_str())
            .map_err(|e| {
                ttrpc_error(
                    ttrpc::Code::INVALID_ARGUMENT,
                    format!("invalid argument: {:?}", e),
                )
            })?;

        p.close_stdin().await;

        Ok(Empty::new())
    }

    async fn tty_win_resize(
        &self,
        ctx: &TtrpcContext,
        req: protocols::agent::TtyWinResizeRequest,
    ) -> ttrpc::Result<Empty> {
        trace_rpc_call!(ctx, "tty_win_resize", req);
        is_allowed(&req).await?;

        let mut sandbox = self.sandbox.lock().await;
        let p = sandbox
            .find_container_process(req.container_id(), req.exec_id())
            .map_err(|e| {
                ttrpc_error(
                    ttrpc::Code::UNAVAILABLE,
                    format!("invalid argument: {:?}", e),
                )
            })?;

        let fd = p
            .term_master
            .map_ttrpc_err(ttrpc::Code::UNAVAILABLE, "no tty")?;
        let win = winsize {
            ws_row: req.row as c_ushort,
            ws_col: req.column as c_ushort,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };

        let err = unsafe { libc::ioctl(fd, TIOCSWINSZ, &win) };
        Errno::result(err)
            .map(drop)
            .map_ttrpc_err(|e| format!("ioctl error: {:?}", e))?;

        Ok(Empty::new())
    }

    async fn update_interface(
        &self,
        ctx: &TtrpcContext,
        req: protocols::agent::UpdateInterfaceRequest,
    ) -> ttrpc::Result<Interface> {
        trace_rpc_call!(ctx, "update_interface", req);
        is_allowed(&req).await?;

        let interface = req.interface.into_option().map_ttrpc_err(
            ttrpc::Code::INVALID_ARGUMENT,
            "empty update interface request",
        )?;

        // For network devices passed on the pci bus, check for the network interface
        // to be available first.
        if !interface.pciPath.is_empty() {
            let pcipath = pci::Path::from_str(&interface.pciPath)
                .map_ttrpc_err(|e| format!("Unexpected pci-path for network interface: {:?}", e))?;

            wait_for_net_interface(&self.sandbox, &pcipath)
                .await
                .map_ttrpc_err(|e| format!("interface not available: {:?}", e))?;
        }

        self.sandbox
            .lock()
            .await
            .rtnl
            .update_interface(&interface)
            .await
            .map_ttrpc_err(|e| format!("update interface: {:?}", e))?;

        Ok(interface)
    }

    async fn update_routes(
        &self,
        ctx: &TtrpcContext,
        req: protocols::agent::UpdateRoutesRequest,
    ) -> ttrpc::Result<Routes> {
        trace_rpc_call!(ctx, "update_routes", req);
        is_allowed(&req).await?;

        let new_routes = req
            .routes
            .into_option()
            .map(|r| r.Routes)
            .map_ttrpc_err(ttrpc::Code::INVALID_ARGUMENT, "empty update routes request")?;

        let mut sandbox = self.sandbox.lock().await;

        sandbox
            .rtnl
            .update_routes(new_routes)
            .await
            .map_ttrpc_err(|e| format!("Failed to update routes: {:?}", e))?;

        let list = sandbox
            .rtnl
            .list_routes()
            .await
            .map_ttrpc_err(|e| format!("Failed to list routes after update: {:?}", e))?;

        Ok(protocols::agent::Routes {
            Routes: list,
            ..Default::default()
        })
    }

    async fn update_ephemeral_mounts(
        &self,
        ctx: &TtrpcContext,
        req: protocols::agent::UpdateEphemeralMountsRequest,
    ) -> ttrpc::Result<Empty> {
        trace_rpc_call!(ctx, "update_mounts", req);
        is_allowed(&req).await?;

        update_ephemeral_mounts(sl(), &req.storages, &self.sandbox)
            .await
            .map_ttrpc_err(|e| format!("Failed to update mounts: {:?}", e))?;
        Ok(Empty::new())
    }

    async fn get_ip_tables(
        &self,
        ctx: &TtrpcContext,
        req: GetIPTablesRequest,
    ) -> ttrpc::Result<GetIPTablesResponse> {
        trace_rpc_call!(ctx, "get_iptables", req);
        is_allowed(&req).await?;

        info!(sl(), "get_ip_tables: request received");

        // the binary could exists in either /usr/sbin or /sbin
        // here check both of the places and return the one exists
        // if none exists, return the /sbin one, and the rpc will
        // returns an internal error
        let cmd = if req.is_ipv6 {
            if Path::new(USR_IP6TABLES_SAVE).exists() {
                USR_IP6TABLES_SAVE
            } else {
                IP6TABLES_SAVE
            }
        } else if Path::new(USR_IPTABLES_SAVE).exists() {
            USR_IPTABLES_SAVE
        } else {
            IPTABLES_SAVE
        }
        .to_string();

        let output = Command::new(cmd.clone())
            .output()
            .map_ttrpc_err_do(|e| warn!(sl(), "failed to run {}: {:?}", cmd, e.kind()))?;
        Ok(GetIPTablesResponse {
            data: output.stdout,
            ..Default::default()
        })
    }

    async fn set_ip_tables(
        &self,
        ctx: &TtrpcContext,
        req: SetIPTablesRequest,
    ) -> ttrpc::Result<SetIPTablesResponse> {
        trace_rpc_call!(ctx, "set_iptables", req);
        is_allowed(&req).await?;

        info!(sl(), "set_ip_tables request received");

        // the binary could exists in both /usr/sbin and /sbin
        // here check both of the places and return the one exists
        // if none exists, return the /sbin one, and the rpc will
        // returns an internal error
        let cmd = if req.is_ipv6 {
            if Path::new(USR_IP6TABLES_RESTORE).exists() {
                USR_IP6TABLES_RESTORE
            } else {
                IP6TABLES_RESTORE
            }
        } else if Path::new(USR_IPTABLES_RESTORE).exists() {
            USR_IPTABLES_RESTORE
        } else {
            IPTABLES_RESTORE
        }
        .to_string();

        let mut child = Command::new(cmd.clone())
            .arg("--wait")
            .arg(IPTABLES_RESTORE_WAIT_SEC.to_string())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_ttrpc_err_do(|e| warn!(sl(), "failure to spawn {}: {:?}", cmd, e.kind()))?;

        let mut stdin = match child.stdin.take() {
            Some(si) => si,
            None => {
                println!("failed to get stdin from child");
                return Err(ttrpc_error(
                    ttrpc::Code::INTERNAL,
                    "failed to take stdin from child",
                ));
            }
        };

        let (tx, rx) = tokio::sync::oneshot::channel::<i32>();
        let handle = tokio::spawn(async move {
            let _ = match stdin.write_all(&req.data) {
                Ok(o) => o,
                Err(e) => {
                    warn!(sl(), "error writing stdin: {:?}", e.kind());
                    return;
                }
            };
            if tx.send(1).is_err() {
                warn!(sl(), "stdin writer thread receiver dropped");
            };
        });

        let _ = tokio::time::timeout(Duration::from_secs(IPTABLES_RESTORE_WAIT_SEC), rx)
            .await
            .map_ttrpc_err(|_| "timeout waiting for stdin writer to complete")?;

        handle
            .await
            .map_ttrpc_err(|_| "stdin writer thread failure")?;

        let output = child.wait_with_output().map_ttrpc_err_do(|e| {
            warn!(
                sl(),
                "failure waiting for spawned {} to complete: {:?}",
                cmd,
                e.kind()
            )
        })?;

        if !output.status.success() {
            warn!(sl(), "{} failed: {:?}", cmd, output.stderr);
            return Err(ttrpc_error(
                ttrpc::Code::INTERNAL,
                format!(
                    "{} failed: {:?}",
                    cmd,
                    String::from_utf8_lossy(&output.stderr)
                ),
            ));
        }

        Ok(SetIPTablesResponse {
            data: output.stdout,
            ..Default::default()
        })
    }

    async fn list_interfaces(
        &self,
        ctx: &TtrpcContext,
        req: protocols::agent::ListInterfacesRequest,
    ) -> ttrpc::Result<Interfaces> {
        trace_rpc_call!(ctx, "list_interfaces", req);
        is_allowed(&req).await?;

        let list = self
            .sandbox
            .lock()
            .await
            .rtnl
            .list_interfaces()
            .await
            .map_ttrpc_err(|e| format!("Failed to list interfaces: {:?}", e))?;

        Ok(protocols::agent::Interfaces {
            Interfaces: list,
            ..Default::default()
        })
    }

    async fn list_routes(
        &self,
        ctx: &TtrpcContext,
        req: protocols::agent::ListRoutesRequest,
    ) -> ttrpc::Result<Routes> {
        trace_rpc_call!(ctx, "list_routes", req);
        is_allowed(&req).await?;

        let list = self
            .sandbox
            .lock()
            .await
            .rtnl
            .list_routes()
            .await
            .map_ttrpc_err(|e| format!("list routes: {:?}", e))?;

        Ok(protocols::agent::Routes {
            Routes: list,
            ..Default::default()
        })
    }

    async fn create_sandbox(
        &self,
        ctx: &TtrpcContext,
        req: protocols::agent::CreateSandboxRequest,
    ) -> ttrpc::Result<Empty> {
        trace_rpc_call!(ctx, "create_sandbox", req);
        is_allowed(&req).await?;

        {
            let mut s = self.sandbox.lock().await;

            let _ = fs::remove_dir_all(CONTAINER_BASE);
            let _ = fs::create_dir_all(CONTAINER_BASE);

            s.hostname = req.hostname.clone();
            s.running = true;

            if !req.sandbox_id.is_empty() {
                s.id = req.sandbox_id.clone();
            }

            for m in req.kernel_modules.iter() {
                load_kernel_module(m).map_ttrpc_err(same)?;
            }

            s.setup_shared_namespaces().await.map_ttrpc_err(same)?;
        }

        let m = add_storages(sl(), req.storages.clone(), &self.sandbox, None)
            .await
            .map_ttrpc_err(same)?;
        self.sandbox.lock().await.mounts = m;

        // Scan guest hooks upon creating new sandbox and append
        // them to guest OCI spec before running containers.
        {
            let mut s = self.sandbox.lock().await;
            if !req.guest_hook_path.is_empty() {
                let _ = s.add_hooks(&req.guest_hook_path).map_err(|e| {
                    error!(
                        sl(),
                        "add guest hook {} failed: {:?}", req.guest_hook_path, e
                    );
                });
            }
        }

        setup_guest_dns(sl(), &req.dns).map_ttrpc_err(same)?;
        {
            let mut s = self.sandbox.lock().await;
            for dns in req.dns {
                s.network.set_dns(dns);
            }
        }

        #[cfg(feature = "guest-pull")]
        image::init_image_service().await.map_ttrpc_err(same)?;

        Ok(Empty::new())
    }

    async fn destroy_sandbox(
        &self,
        ctx: &TtrpcContext,
        req: protocols::agent::DestroySandboxRequest,
    ) -> ttrpc::Result<Empty> {
        trace_rpc_call!(ctx, "destroy_sandbox", req);
        is_allowed(&req).await?;

        let mut sandbox = self.sandbox.lock().await;
        // destroy all containers, clean up, notify agent to exit etc.
        sandbox.destroy().await.map_ttrpc_err(same)?;
        // Close get_oom_event connection,
        // otherwise it will block the shutdown of ttrpc.
        drop(sandbox.event_tx.take());

        sandbox
            .sender
            .take()
            .map_ttrpc_err(
                ttrpc::Code::INTERNAL,
                "failed to get sandbox sender channel",
            )?
            .send(1)
            .map_ttrpc_err(same)?;

        Ok(Empty::new())
    }

    async fn add_arp_neighbors(
        &self,
        ctx: &TtrpcContext,
        req: protocols::agent::AddARPNeighborsRequest,
    ) -> ttrpc::Result<Empty> {
        trace_rpc_call!(ctx, "add_arp_neighbors", req);
        is_allowed(&req).await?;

        let neighs = req
            .neighbors
            .into_option()
            .map(|n| n.ARPNeighbors)
            .map_ttrpc_err(
                ttrpc::Code::INVALID_ARGUMENT,
                "empty add arp neighbours request",
            )?;

        self.sandbox
            .lock()
            .await
            .rtnl
            .add_arp_neighbors(neighs)
            .await
            .map_ttrpc_err(|e| format!("Failed to add ARP neighbours: {:?}", e))?;

        Ok(Empty::new())
    }

    async fn online_cpu_mem(
        &self,
        ctx: &TtrpcContext,
        req: protocols::agent::OnlineCPUMemRequest,
    ) -> ttrpc::Result<Empty> {
        trace_rpc_call!(ctx, "online_cpu_mem", req);
        is_allowed(&req).await?;
        let sandbox = self.sandbox.lock().await;

        sandbox.online_cpu_memory(&req).map_ttrpc_err(same)?;

        Ok(Empty::new())
    }

    async fn reseed_random_dev(
        &self,
        ctx: &TtrpcContext,
        req: protocols::agent::ReseedRandomDevRequest,
    ) -> ttrpc::Result<Empty> {
        trace_rpc_call!(ctx, "reseed_random_dev", req);
        is_allowed(&req).await?;

        random::reseed_rng(req.data.as_slice()).map_ttrpc_err(same)?;

        Ok(Empty::new())
    }

    async fn get_guest_details(
        &self,
        ctx: &TtrpcContext,
        req: protocols::agent::GuestDetailsRequest,
    ) -> ttrpc::Result<GuestDetailsResponse> {
        trace_rpc_call!(ctx, "get_guest_details", req);
        is_allowed(&req).await?;

        info!(sl(), "get guest details!");
        let mut resp = GuestDetailsResponse::new();
        // to get memory block size
        let (u, v) = get_memory_info(
            req.mem_block_size,
            req.mem_hotplug_probe,
            SYSFS_MEMORY_BLOCK_SIZE_PATH,
            SYSFS_MEMORY_HOTPLUG_PROBE_PATH,
        )
        .map_ttrpc_err_do(|_| info!(sl(), "fail to get memory info!"))?;

        resp.mem_block_size_bytes = u;
        resp.support_mem_hotplug_probe = v;

        // to get agent details
        let detail = get_agent_details();
        resp.agent_details = MessageField::some(detail);

        Ok(resp)
    }

    async fn mem_hotplug_by_probe(
        &self,
        ctx: &TtrpcContext,
        req: protocols::agent::MemHotplugByProbeRequest,
    ) -> ttrpc::Result<Empty> {
        trace_rpc_call!(ctx, "mem_hotplug_by_probe", req);
        is_allowed(&req).await?;

        do_mem_hotplug_by_probe(&req.memHotplugProbeAddr).map_ttrpc_err(same)?;

        Ok(Empty::new())
    }

    async fn set_guest_date_time(
        &self,
        ctx: &TtrpcContext,
        req: protocols::agent::SetGuestDateTimeRequest,
    ) -> ttrpc::Result<Empty> {
        trace_rpc_call!(ctx, "set_guest_date_time", req);
        is_allowed(&req).await?;

        do_set_guest_date_time(req.Sec, req.Usec).map_ttrpc_err(same)?;

        Ok(Empty::new())
    }

    async fn copy_file(
        &self,
        ctx: &TtrpcContext,
        req: protocols::agent::CopyFileRequest,
    ) -> ttrpc::Result<Empty> {
        trace_rpc_call!(ctx, "copy_file", req);
        is_allowed(&req).await?;

        do_copy_file(&req).map_ttrpc_err(same)?;

        Ok(Empty::new())
    }

    async fn get_metrics(
        &self,
        ctx: &TtrpcContext,
        req: protocols::agent::GetMetricsRequest,
    ) -> ttrpc::Result<Metrics> {
        trace_rpc_call!(ctx, "get_metrics", req);
        is_allowed(&req).await?;

        let s = get_metrics(&req).map_ttrpc_err(same)?;
        let mut metrics = Metrics::new();
        metrics.set_metrics(s);
        Ok(metrics)
    }

    async fn get_oom_event(
        &self,
        _ctx: &TtrpcContext,
        req: protocols::agent::GetOOMEventRequest,
    ) -> ttrpc::Result<OOMEvent> {
        is_allowed(&req).await?;
        let s = self.sandbox.lock().await;
        let event_rx = &s.event_rx.clone();
        let mut event_rx = event_rx.lock().await;
        drop(s);

        let container_id = event_rx
            .recv()
            .await
            .map_ttrpc_err(ttrpc::Code::INTERNAL, "")?;

        info!(sl(), "get_oom_event return {}", &container_id);

        let mut resp = OOMEvent::new();
        resp.container_id = container_id;
        Ok(resp)
    }

    async fn get_volume_stats(
        &self,
        ctx: &TtrpcContext,
        req: VolumeStatsRequest,
    ) -> ttrpc::Result<VolumeStatsResponse> {
        trace_rpc_call!(ctx, "get_volume_stats", req);
        is_allowed(&req).await?;

        info!(sl(), "get volume stats!");
        let mut resp = VolumeStatsResponse::new();
        let mut condition = VolumeCondition::new();

        File::open(&req.volume_guest_path)
            .map_ttrpc_err_do(|_| info!(sl(), "failed to open the volume"))?;

        condition.abnormal = false;
        condition.message = String::from("OK");

        let mut usage_vec = Vec::new();

        // to get volume capacity stats
        get_volume_capacity_stats(&req.volume_guest_path)
            .map(|u| usage_vec.push(u))
            .map_ttrpc_err(same)?;

        // to get volume inode stats
        get_volume_inode_stats(&req.volume_guest_path)
            .map(|u| usage_vec.push(u))
            .map_ttrpc_err(same)?;

        resp.usage = usage_vec;
        resp.volume_condition = MessageField::some(condition);
        Ok(resp)
    }

    async fn add_swap(
        &self,
        ctx: &TtrpcContext,
        req: protocols::agent::AddSwapRequest,
    ) -> ttrpc::Result<Empty> {
        trace_rpc_call!(ctx, "add_swap", req);
        is_allowed(&req).await?;

        do_add_swap(&self.sandbox, &req).await.map_ttrpc_err(same)?;

        Ok(Empty::new())
    }

    #[cfg(feature = "agent-policy")]
    async fn set_policy(
        &self,
        ctx: &TtrpcContext,
        req: protocols::agent::SetPolicyRequest,
    ) -> ttrpc::Result<Empty> {
        trace_rpc_call!(ctx, "set_policy", req);

        do_set_policy(&req).await?;

        Ok(Empty::new())
    }

    async fn mem_agent_memcg_set(
        &self,
        _ctx: &::ttrpc::r#async::TtrpcContext,
        config: protocols::agent::MemAgentMemcgConfig,
    ) -> ::ttrpc::Result<Empty> {
        if let Some(ma) = &self.oma {
            ma.memcg_set_config_async(mem_agent_memcgconfig_to_memcg_optionconfig(&config))
                .await
                .map_err(|e| {
                    let estr = format!("ma.memcg_set_config_async fail: {}", e);
                    error!(sl(), "{}", estr);
                    ttrpc::Error::RpcStatus(ttrpc::get_status(ttrpc::Code::INTERNAL, estr))
                })?;
        } else {
            let estr = "mem-agent is disabled";
            error!(sl(), "{}", estr);
            return Err(ttrpc::Error::RpcStatus(ttrpc::get_status(
                ttrpc::Code::INTERNAL,
                estr,
            )));
        }
        Ok(Empty::new())
    }

    async fn mem_agent_compact_set(
        &self,
        _ctx: &::ttrpc::r#async::TtrpcContext,
        config: protocols::agent::MemAgentCompactConfig,
    ) -> ::ttrpc::Result<Empty> {
        if let Some(ma) = &self.oma {
            ma.compact_set_config_async(mem_agent_compactconfig_to_compact_optionconfig(&config))
                .await
                .map_err(|e| {
                    let estr = format!("ma.compact_set_config_async fail: {}", e);
                    error!(sl(), "{}", estr);
                    ttrpc::Error::RpcStatus(ttrpc::get_status(ttrpc::Code::INTERNAL, estr))
                })?;
        } else {
            let estr = "mem-agent is disabled";
            error!(sl(), "{}", estr);
            return Err(ttrpc::Error::RpcStatus(ttrpc::get_status(
                ttrpc::Code::INTERNAL,
                estr,
            )));
        }
        Ok(Empty::new())
    }
}

#[derive(Clone)]
struct HealthService;

#[async_trait]
impl health_ttrpc::Health for HealthService {
    async fn check(
        &self,
        _ctx: &TtrpcContext,
        _req: protocols::health::CheckRequest,
    ) -> ttrpc::Result<HealthCheckResponse> {
        let mut resp = HealthCheckResponse::new();
        resp.set_status(HealthCheckResponse_ServingStatus::SERVING);

        Ok(resp)
    }

    async fn version(
        &self,
        _ctx: &TtrpcContext,
        req: protocols::health::CheckRequest,
    ) -> ttrpc::Result<VersionCheckResponse> {
        info!(sl(), "version {:?}", req);
        let mut rep = protocols::health::VersionCheckResponse::new();
        rep.agent_version = AGENT_VERSION.to_string();
        rep.grpc_version = API_VERSION.to_string();

        Ok(rep)
    }
}

fn get_memory_info(
    block_size: bool,
    hotplug: bool,
    block_size_path: &str,
    hotplug_probe_path: &str,
) -> Result<(u64, bool)> {
    let mut size: u64 = 0;
    let mut plug: bool = false;
    if block_size {
        match fs::read_to_string(block_size_path) {
            Ok(v) => {
                if v.is_empty() {
                    warn!(sl(), "file {} is empty", block_size_path);
                    return Err(anyhow!(ERR_INVALID_BLOCK_SIZE));
                }

                size = u64::from_str_radix(v.trim(), 16).map_err(|_| {
                    warn!(sl(), "failed to parse the str {} to hex", size);
                    anyhow!(ERR_INVALID_BLOCK_SIZE)
                })?;
            }
            Err(e) => {
                warn!(sl(), "memory block size error: {:?}", e.kind());
                if e.kind() != std::io::ErrorKind::NotFound {
                    return Err(anyhow!(e));
                }
            }
        }
    }

    if hotplug {
        match stat::stat(hotplug_probe_path) {
            Ok(_) => plug = true,
            Err(e) => {
                warn!(sl(), "hotplug memory error: {:?}", e);
                match e {
                    nix::Error::ENOENT => plug = false,
                    _ => return Err(anyhow!(e)),
                }
            }
        }
    }

    Ok((size, plug))
}

fn get_volume_capacity_stats(path: &str) -> Result<VolumeUsage> {
    let mut usage = VolumeUsage::new();

    let stat = statfs::statfs(path)?;
    let block_size = stat.block_size() as u64;
    usage.total = stat.blocks() * block_size;
    usage.available = stat.blocks_free() * block_size;
    usage.used = usage.total - usage.available;
    usage.unit = VolumeUsage_Unit::BYTES.into();

    Ok(usage)
}

fn get_volume_inode_stats(path: &str) -> Result<VolumeUsage> {
    let mut usage = VolumeUsage::new();

    let stat = statfs::statfs(path)?;
    usage.total = stat.files();
    usage.available = stat.files_free();
    usage.used = usage.total - usage.available;
    usage.unit = VolumeUsage_Unit::INODES.into();

    Ok(usage)
}

pub fn have_seccomp() -> bool {
    if cfg!(feature = "seccomp") {
        return true;
    }

    false
}

fn get_agent_details() -> AgentDetails {
    let mut detail = AgentDetails::new();

    detail.set_version(AGENT_VERSION.to_string());
    detail.set_supports_seccomp(have_seccomp());
    detail.init_daemon = unistd::getpid() == Pid::from_raw(1);

    detail.device_handlers = Vec::new();
    detail.storage_handlers = STORAGE_HANDLERS.get_handlers();
    detail.extra_features = get_build_features();

    detail
}

async fn read_stream(reader: &Mutex<ReadHalf<PipeStream>>, l: usize) -> Result<Vec<u8>> {
    let mut content = vec![0u8; l];

    let mut reader = reader.lock().await;
    let len = reader.read(&mut content).await?;
    content.resize(len, 0);

    if len == 0 {
        return Err(anyhow!("read meet eof"));
    }

    Ok(content)
}

pub async fn start(
    s: Arc<Mutex<Sandbox>>,
    server_address: &str,
    init_mode: bool,
    oma: Option<mem_agent::agent::MemAgent>,
) -> Result<TtrpcServer> {
    let agent_service = Box::new(AgentService {
        sandbox: s,
        init_mode,
        oma,
    }) as Box<dyn agent_ttrpc::AgentService + Send + Sync>;
    let aservice = agent_ttrpc::create_agent_service(Arc::new(agent_service));

    let health_service = Box::new(HealthService {}) as Box<dyn health_ttrpc::Health + Send + Sync>;
    let hservice = health_ttrpc::create_health(Arc::new(health_service));

    let server = TtrpcServer::new()
        .bind(server_address)?
        .register_service(aservice)
        .register_service(hservice);

    info!(sl(), "ttRPC server started"; "address" => server_address);

    Ok(server)
}

// This function updates the container namespaces configuration based on the
// sandbox information. When the sandbox is created, it can be setup in a way
// that all containers will share some specific namespaces. This is the agent
// responsibility to create those namespaces so that they can be shared across
// several containers.
// If the sandbox has not been setup to share namespaces, then we assume all
// containers will be started in their own new namespace.
// The value of a.sandbox.sharedPidNs.path will always override the namespace
// path set by the spec, since we will always ignore it. Indeed, it makes no
// sense to rely on the namespace path provided by the host since namespaces
// are different inside the guest.
fn update_container_namespaces(
    sandbox: &Sandbox,
    spec: &mut Spec,
    sandbox_pidns: bool,
) -> Result<()> {
    let linux = spec
        .linux_mut()
        .as_mut()
        .ok_or_else(|| anyhow!(ERR_NO_LINUX_FIELD))?;

    if let Some(namespaces) = linux.namespaces_mut() {
        for namespace in namespaces.iter_mut() {
            if namespace.typ().to_string() == NSTYPEIPC {
                namespace.set_path(if !sandbox.shared_ipcns.path.is_empty() {
                    Some(PathBuf::from(&sandbox.shared_ipcns.path))
                } else {
                    None
                });
                continue;
            }
            if namespace.typ().to_string() == NSTYPEUTS {
                namespace.set_path(if !sandbox.shared_utsns.path.is_empty() {
                    Some(PathBuf::from(&sandbox.shared_utsns.path))
                } else {
                    None
                });
                continue;
            }
        }

        // update pid namespace
        let mut pid_ns = LinuxNamespace::default();
        pid_ns.set_typ(oci::LinuxNamespaceType::try_from(NSTYPEPID).unwrap());

        // Use shared pid ns if useSandboxPidns has been set in either
        // the create_sandbox request or create_container request.
        // Else set this to empty string so that a new pid namespace is
        // created for the container.
        if sandbox_pidns {
            if let Some(ref pidns) = &sandbox.sandbox_pidns {
                if !pidns.path.is_empty() {
                    pid_ns.set_path(Some(PathBuf::from(&pidns.path)));
                }
            } else if !sandbox.containers.is_empty() {
                return Err(anyhow!(ERR_NO_SANDBOX_PIDNS));
            }
        }

        namespaces.push(pid_ns);
    }

    Ok(())
}

async fn remove_container_resources(sandbox: &mut Sandbox, cid: &str) -> Result<()> {
    let mut cmounts: Vec<String> = vec![];

    // Find the sandbox storage used by this container
    let mounts = sandbox.container_mounts.get(cid);
    if let Some(mounts) = mounts {
        for m in mounts.iter() {
            if sandbox.storages.contains_key(m) {
                cmounts.push(m.to_string());
            }
        }
    }

    for m in cmounts.iter() {
        if let Err(err) = sandbox.remove_sandbox_storage(m).await {
            error!(
                sl(),
                "failed to unset_and_remove_sandbox_storage for container {}, error: {:?}",
                cid,
                err
            );
        }
    }

    sandbox.container_mounts.remove(cid);
    sandbox.containers.remove(cid);
    Ok(())
}

fn append_guest_hooks(s: &Sandbox, oci: &mut Spec) -> Result<()> {
    if let Some(ref guest_hooks) = s.hooks {
        if let Some(hooks) = oci.hooks_mut() {
            util::merge(hooks.poststart_mut(), guest_hooks.prestart());
            util::merge(hooks.poststart_mut(), guest_hooks.poststart());
            util::merge(hooks.poststop_mut(), guest_hooks.poststop());
        } else {
            let _oci_hooks = oci.set_hooks(Some(Hooks::default()));
            if let Some(hooks) = oci.hooks_mut() {
                hooks.set_prestart(guest_hooks.prestart().clone());
                hooks.set_poststart(guest_hooks.poststart().clone());
                hooks.set_poststop(guest_hooks.poststop().clone());
            }
        }
    }

    Ok(())
}

// Check if the container process installed the
// handler for specific signal.
fn is_signal_handled(proc_status_file: &str, signum: u32) -> bool {
    let shift_count: u64 = if signum == 0 {
        // signum 0 is used to check for process liveness.
        // Since that signal is not part of the mask in the file, we only need
        // to know if the file (and therefore) process exists to handle
        // that signal.
        return fs::metadata(proc_status_file).is_ok();
    } else if signum > 64 {
        // Ensure invalid signum won't break bit shift logic
        warn!(sl(), "received invalid signum {}", signum);
        return false;
    } else {
        (signum - 1).into()
    };

    // Open the file in read-only mode (ignoring errors).
    let file = match File::open(proc_status_file) {
        Ok(f) => f,
        Err(_) => {
            warn!(sl(), "failed to open file {}", proc_status_file);
            return false;
        }
    };

    let sig_mask: u64 = 1 << shift_count;
    let reader = BufReader::new(file);

    // read lines start with SigBlk/SigIgn/SigCgt and check any match the signal mask
    reader
        .lines()
        .map_while(Result::ok)
        .filter(|line| {
            line.starts_with("SigBlk:")
                || line.starts_with("SigIgn:")
                || line.starts_with("SigCgt:")
        })
        .any(|line| {
            let mask_vec: Vec<&str> = line.split(':').collect();
            if mask_vec.len() == 2 {
                let sig_str = mask_vec[1].trim();
                if let Ok(sig) = u64::from_str_radix(sig_str, 16) {
                    return sig & sig_mask == sig_mask;
                }
            }
            false
        })
}

fn do_mem_hotplug_by_probe(addrs: &[u64]) -> Result<()> {
    for addr in addrs.iter() {
        fs::write(SYSFS_MEMORY_HOTPLUG_PROBE_PATH, format!("{:#X}", *addr))?;
    }
    Ok(())
}

fn do_set_guest_date_time(sec: i64, usec: i64) -> Result<()> {
    let tv = libc::timeval {
        tv_sec: sec,
        tv_usec: usec,
    };

    let ret = unsafe {
        libc::settimeofday(
            &tv as *const libc::timeval,
            std::ptr::null::<libc::timezone>(),
        )
    };

    Errno::result(ret).map(drop)?;

    Ok(())
}

fn do_copy_file(req: &CopyFileRequest) -> Result<()> {
    let path = PathBuf::from(req.path.as_str());

    if !path.starts_with(CONTAINER_BASE) {
        return Err(anyhow!(
            "Path {:?} does not start with {}",
            path,
            CONTAINER_BASE
        ));
    }

    if let Some(parent) = path.parent() {
        if !parent.exists() {
            let dir = parent.to_path_buf();
            if let Err(e) = fs::create_dir_all(&dir) {
                if e.kind() != std::io::ErrorKind::AlreadyExists {
                    return Err(e.into());
                }
            } else {
                std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(req.dir_mode))?;
            }
        }
    }

    let sflag = stat::SFlag::from_bits_truncate(req.file_mode);

    if sflag.contains(stat::SFlag::S_IFDIR) {
        fs::create_dir(&path).or_else(|e| {
            if e.kind() != std::io::ErrorKind::AlreadyExists {
                return Err(e);
            }
            Ok(())
        })?;

        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(req.file_mode))?;

        unistd::chown(
            &path,
            Some(Uid::from_raw(req.uid as u32)),
            Some(Gid::from_raw(req.gid as u32)),
        )?;

        return Ok(());
    }

    if sflag.contains(stat::SFlag::S_IFLNK) {
        // After kubernetes secret's volume update, the '..data' symlink should point to
        // the new timestamped directory.
        // TODO:The old and deleted timestamped dir still exists due to missing DELETE api in agent.
        // Hence, Unlink the existing symlink.
        if path.is_symlink() && path.exists() {
            unistd::unlink(&path)?;
        }
        let src = PathBuf::from(OsStr::from_bytes(&req.data));
        unistd::symlinkat(&src, None, &path)?;
        let path_str = CString::new(path.as_os_str().as_bytes())?;

        let ret = unsafe { libc::lchown(path_str.as_ptr(), req.uid as u32, req.gid as u32) };
        Errno::result(ret).map(drop)?;

        return Ok(());
    }

    let mut tmpfile = path.clone();
    tmpfile.set_extension("tmp");

    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .open(&tmpfile)?;

    file.write_all_at(req.data.as_slice(), req.offset as u64)?;
    let st = stat::stat(&tmpfile)?;

    if st.st_size != req.file_size {
        return Ok(());
    }

    file.set_permissions(std::fs::Permissions::from_mode(req.file_mode))?;

    unistd::chown(
        &tmpfile,
        Some(Uid::from_raw(req.uid as u32)),
        Some(Gid::from_raw(req.gid as u32)),
    )?;

    fs::rename(tmpfile, path)?;

    Ok(())
}

async fn do_add_swap(sandbox: &Arc<Mutex<Sandbox>>, req: &AddSwapRequest) -> Result<()> {
    let mut slots = Vec::new();
    for slot in &req.PCIPath {
        slots.push(pci::SlotFn::new(*slot, 0)?);
    }
    let pcipath = pci::Path::new(slots)?;
    let dev_name = get_virtio_blk_pci_device_name(sandbox, &pcipath).await?;

    let c_str = CString::new(dev_name)?;
    let ret = unsafe { libc::swapon(c_str.as_ptr() as *const c_char, 0) };
    if ret != 0 {
        return Err(anyhow!(
            "libc::swapon get error {}",
            io::Error::last_os_error()
        ));
    }

    Ok(())
}

// Setup container bundle under CONTAINER_BASE, which is cleaned up
// before removing a container.
// - bundle path is /<CONTAINER_BASE>/<cid>/
// - config.json at /<CONTAINER_BASE>/<cid>/config.json
// - container rootfs bind mounted at /<CONTAINER_BASE>/<cid>/rootfs
// - modify container spec root to point to /<CONTAINER_BASE>/<cid>/rootfs
pub fn setup_bundle(cid: &str, spec: &mut Spec) -> Result<PathBuf> {
    let spec_root = if let Some(sr) = &spec.root() {
        sr
    } else {
        return Err(anyhow!(nix::Error::EINVAL));
    };

    let bundle_path = Path::new(CONTAINER_BASE).join(cid);
    let config_path = bundle_path.join("config.json");
    let rootfs_path = bundle_path.join("rootfs");
    let spec_root_path = spec_root.path();

    let rootfs_exists = Path::new(&rootfs_path).exists();
    info!(
        sl(),
        "The rootfs_path is {:?} and exists: {}", rootfs_path, rootfs_exists
    );

    if !rootfs_exists {
        fs::create_dir_all(&rootfs_path)?;
        baremount(
            spec_root_path,
            &rootfs_path,
            "bind",
            MsFlags::MS_BIND,
            "",
            &sl(),
        )?;
    }

    let mut oci_root = oci::Root::default();
    oci_root.set_path(rootfs_path);
    oci_root.set_readonly(spec_root.readonly());
    spec.set_root(Some(oci_root));

    let _ = spec.save(
        config_path
            .to_str()
            .ok_or_else(|| anyhow!("cannot convert path to unicode"))?,
    );

    let olddir = unistd::getcwd().context("cannot getcwd")?;
    unistd::chdir(
        bundle_path
            .to_str()
            .ok_or_else(|| anyhow!("cannot convert bundle path to unicode"))?,
    )?;

    Ok(olddir)
}

fn load_kernel_module(module: &protocols::agent::KernelModule) -> Result<()> {
    if module.name.is_empty() {
        return Err(anyhow!("Kernel module name is empty"));
    }

    info!(
        sl(),
        "load_kernel_module {}: {:?}", module.name, module.parameters
    );

    let mut args = vec!["-v", &module.name];

    if !module.parameters.is_empty() {
        args.extend(module.parameters.iter().map(String::as_str));
    }

    let output = Command::new(MODPROBE_PATH)
        .args(args.as_slice())
        .stdout(Stdio::piped())
        .output()?;

    let status = output.status;
    if status.success() {
        return Ok(());
    }

    match status.code() {
        Some(code) => {
            let std_out = String::from_utf8_lossy(&output.stdout);
            let std_err = String::from_utf8_lossy(&output.stderr);
            let msg = format!(
                "load_kernel_module return code: {} stdout:{} stderr:{}",
                code, std_out, std_err
            );
            Err(anyhow!(msg))
        }
        None => Err(anyhow!("Process terminated by signal")),
    }
}

fn is_sealed_secret_path(source_path: &str) -> bool {
    // Base path to check
    let base_path = "/run/kata-containers/shared/containers";
    // Paths to exclude
    let excluded_suffixes = [
        "resolv.conf",
        "termination-log",
        "hostname",
        "hosts",
        "serviceaccount",
    ];

    // Ensure the path starts with the base path and does not end with any excluded suffix
    source_path.starts_with(base_path)
        && !excluded_suffixes
            .iter()
            .any(|suffix| source_path.ends_with(suffix))
}

async fn cdh_handler(oci: &mut Spec) -> Result<()> {
    if !cdh::is_cdh_client_initialized().await {
        return Ok(());
    }
    let process = oci
        .process_mut()
        .as_mut()
        .ok_or_else(|| anyhow!("Spec didn't contain process field"))?;
    if let Some(envs) = process.env_mut().as_mut() {
        for env in envs.iter_mut() {
            match cdh::unseal_env(env).await {
                Ok(unsealed_env) => *env = unsealed_env.to_string(),
                Err(e) => {
                    warn!(sl(), "Failed to unseal secret: {}", e)
                }
            }
        }
    }

    let mounts = oci
        .mounts_mut()
        .as_mut()
        .ok_or_else(|| anyhow!("Spec didn't contain mounts field"))?;

    for m in mounts.iter_mut() {
        let Some(source_path) = m.source().as_ref().and_then(|p| p.to_str()) else {
            warn!(sl(), "Mount source is None or invalid");
            continue;
        };

        // Check if source_path starts with "/run/kata-containers/shared/containers"
        // For a volume mount path /mydir,
        // the secret file path will be like this under the /run/kata-containers/shared/containers dir
        // a128482812bad768f404e063f225decd425fc94a673aec4add45a9caa1122ccb-75490e32e51da3ff-mydir
        // We can ignore few paths like: resolv.conf, termination-log, hostname,hosts,serviceaccount
        if is_sealed_secret_path(source_path) {
            debug!(
                sl(),
                "Calling unseal_file for - source: {:?} destination: {:?}",
                source_path,
                m.destination()
            );
            // Call unseal_file. This function checks the files under the source_path
            // for the sealed secret header and unseal it if the header is present.
            // This is suboptimal as we are going through every file under the source_path.
            // But currently there is no quick way to determine which volume-mount is referring
            // to a sealed secret without reading the file.
            // And relying on file naming heuristic is inflexible. So we are going with this approach.
            if let Err(e) = cdh::unseal_file(source_path).await {
                warn!(
                    sl(),
                    "Failed to unseal file: {:?}, Error: {:?}", source_path, e
                );
            }
        }
    }

    let linux = oci
        .linux()
        .as_ref()
        .ok_or_else(|| anyhow!("Spec didn't contain linux field"))?;

    if let Some(devices) = linux.devices() {
        for specdev in devices.iter() {
            if specdev.path().as_path().to_str() == Some(TRUSTED_IMAGE_STORAGE_DEVICE) {
                let dev_major_minor = format!("{}:{}", specdev.major(), specdev.minor());
                let secure_storage_integrity = AGENT_CONFIG.secure_storage_integrity.to_string();
                info!(
                    sl(),
                    "trusted_store device major:min {}, enable data integrity {}",
                    dev_major_minor,
                    secure_storage_integrity
                );

                let options = std::collections::HashMap::from([
                    ("deviceId".to_string(), dev_major_minor),
                    ("encryptType".to_string(), "LUKS".to_string()),
                    ("dataIntegrity".to_string(), secure_storage_integrity),
                ]);
                cdh::secure_mount("BlockDevice", &options, vec![], KATA_IMAGE_WORK_DIR).await?;
                break;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
#[allow(dead_code)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;
    use crate::{namespace::Namespace, protocols::agent_ttrpc_async::AgentService as _};
    use nix::mount;
    use nix::sched::{unshare, CloneFlags};
    use oci::{
        HookBuilder, HooksBuilder, Linux, LinuxBuilder, LinuxDeviceCgroupBuilder, LinuxNamespace,
        LinuxNamespaceBuilder, LinuxResourcesBuilder, SpecBuilder,
    };
    use oci_spec::runtime::{LinuxNamespaceType, Root};
    use tempfile::{tempdir, TempDir};
    use test_utils::{assert_result, skip_if_not_root};
    use ttrpc::{r#async::TtrpcContext, MessageHeader};
    use which::which;

    const CGROUP_PARENT: &str = "kata.agent.test.k8s.io";

    fn check_command(cmd: &str) -> bool {
        which(cmd).is_ok()
    }

    fn mk_ttrpc_context() -> TtrpcContext {
        TtrpcContext {
            fd: -1,
            mh: MessageHeader::default(),
            metadata: std::collections::HashMap::new(),
            timeout_nano: 0,
        }
    }

    fn create_dummy_opts() -> CreateOpts {
        let start = SystemTime::now();
        let since_the_epoch = start
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards");

        let mut root = Root::default();
        root.set_path(PathBuf::from("/"));

        let linux_resources = LinuxResourcesBuilder::default()
            .devices(vec![LinuxDeviceCgroupBuilder::default()
                .allow(true)
                .access("rwm")
                .build()
                .unwrap()])
            .build()
            .unwrap();

        let cgroups_path = format!(
            "/{}/dummycontainer{}",
            CGROUP_PARENT,
            since_the_epoch.as_millis()
        );

        let spec = SpecBuilder::default()
            .linux(
                LinuxBuilder::default()
                    .cgroups_path(cgroups_path)
                    .resources(linux_resources)
                    .build()
                    .unwrap(),
            )
            .root(root)
            .build()
            .unwrap();

        CreateOpts {
            cgroup_name: "".to_string(),
            use_systemd_cgroup: false,
            no_pivot_root: false,
            no_new_keyring: false,
            spec: Some(spec),
            rootless_euid: false,
            rootless_cgroup: false,
            container_name: "".to_string(),
        }
    }

    fn create_linuxcontainer() -> (LinuxContainer, TempDir) {
        let dir = tempdir().expect("failed to make tempdir");

        (
            LinuxContainer::new(
                "some_id",
                dir.path().join("rootfs").to_str().unwrap(),
                None,
                create_dummy_opts(),
                &slog_scope::logger(),
            )
            .unwrap(),
            dir,
        )
    }

    #[test]
    fn test_load_kernel_module() {
        let mut m = protocols::agent::KernelModule {
            name: "module_not_exists".to_string(),
            ..Default::default()
        };

        // case 1: module not exists
        let result = load_kernel_module(&m);
        assert!(result.is_err(), "load module should failed");

        // case 2: module name is empty
        m.name = "".to_string();
        let result = load_kernel_module(&m);
        assert!(result.is_err(), "load module should failed");

        skip_if_not_root!();
        // case 3: normal module.
        // normally this module should eixsts...
        m.name = "bridge".to_string();
        let result = load_kernel_module(&m);
        assert!(result.is_ok(), "load module should success");
    }

    #[tokio::test]
    async fn test_append_guest_hooks() {
        let logger = slog::Logger::root(slog::Discard, o!());
        let mut s = Sandbox::new(&logger).unwrap();
        let hooks = HooksBuilder::default()
            .prestart(vec![HookBuilder::default()
                .path(PathBuf::from("foo"))
                .build()
                .unwrap()])
            .build()
            .unwrap();
        s.hooks = Some(hooks);

        let mut oci = Spec::default();
        append_guest_hooks(&s, &mut oci).unwrap();
        assert_eq!(s.hooks, oci.hooks().clone());
    }

    #[tokio::test]
    async fn test_update_interface() {
        let logger = slog::Logger::root(slog::Discard, o!());
        let sandbox = Sandbox::new(&logger).unwrap();

        let agent_service = Box::new(AgentService {
            sandbox: Arc::new(Mutex::new(sandbox)),
            init_mode: true,
            oma: None,
        });

        let req = protocols::agent::UpdateInterfaceRequest::default();
        let ctx = mk_ttrpc_context();

        let result = agent_service.update_interface(&ctx, req).await;

        assert!(result.is_err(), "expected update interface to fail");
    }

    #[tokio::test]
    async fn test_update_routes() {
        let logger = slog::Logger::root(slog::Discard, o!());
        let sandbox = Sandbox::new(&logger).unwrap();
        let agent_service = Box::new(AgentService {
            sandbox: Arc::new(Mutex::new(sandbox)),
            init_mode: true,
            oma: None,
        });

        let req = protocols::agent::UpdateRoutesRequest::default();
        let ctx = mk_ttrpc_context();

        let result = agent_service.update_routes(&ctx, req).await;

        assert!(result.is_err(), "expected update routes to fail");
    }

    #[tokio::test]
    async fn test_add_arp_neighbors() {
        let logger = slog::Logger::root(slog::Discard, o!());
        let sandbox = Sandbox::new(&logger).unwrap();
        let agent_service = Box::new(AgentService {
            sandbox: Arc::new(Mutex::new(sandbox)),
            init_mode: true,
            oma: None,
        });

        let req = protocols::agent::AddARPNeighborsRequest::default();
        let ctx = mk_ttrpc_context();

        let result = agent_service.add_arp_neighbors(&ctx, req).await;

        assert!(result.is_err(), "expected add arp neighbors to fail");
    }

    #[tokio::test]
    #[cfg(not(target_arch = "powerpc64"))]
    async fn test_do_write_stream() {
        skip_if_not_root!();

        #[derive(Debug)]
        struct TestData<'a> {
            create_container: bool,
            has_fd: bool,
            has_tty: bool,
            break_pipe: bool,

            container_id: &'a str,
            exec_id: &'a str,
            data: Vec<u8>,
            result: Result<protocols::agent::WriteStreamResponse>,
        }

        impl Default for TestData<'_> {
            fn default() -> Self {
                TestData {
                    create_container: true,
                    has_fd: true,
                    has_tty: true,
                    break_pipe: false,

                    container_id: "1",
                    exec_id: "2",
                    data: vec![1, 2, 3],
                    result: Ok(WriteStreamResponse {
                        len: 3,
                        ..WriteStreamResponse::default()
                    }),
                }
            }
        }

        let tests = &[
            TestData {
                ..Default::default()
            },
            TestData {
                has_tty: false,
                ..Default::default()
            },
            TestData {
                break_pipe: true,
                result: Err(anyhow!(std::io::Error::from_raw_os_error(libc::EPIPE))),
                ..Default::default()
            },
            TestData {
                create_container: false,
                result: Err(anyhow!(crate::sandbox::ERR_INVALID_CONTAINER_ID)),
                ..Default::default()
            },
            TestData {
                container_id: "8181",
                result: Err(anyhow!(crate::sandbox::ERR_INVALID_CONTAINER_ID)),
                ..Default::default()
            },
            TestData {
                data: vec![],
                result: Ok(WriteStreamResponse {
                    len: 0,
                    ..WriteStreamResponse::default()
                }),
                ..Default::default()
            },
            TestData {
                has_fd: false,
                result: Err(anyhow!(ERR_CANNOT_GET_WRITER)),
                ..Default::default()
            },
        ];

        for (i, d) in tests.iter().enumerate() {
            let msg = format!("test[{}]: {:?}", i, d);

            let logger = slog::Logger::root(slog::Discard, o!());
            let mut sandbox = Sandbox::new(&logger).unwrap();

            let (rfd, wfd) = unistd::pipe().unwrap();
            if d.break_pipe {
                unistd::close(rfd).unwrap();
            }

            if d.create_container {
                let (mut linux_container, _root) = create_linuxcontainer();
                let exec_process_id = 2;

                linux_container.id = "1".to_string();

                let mut exec_process = Process::new(
                    &logger,
                    &oci::Process::default(),
                    &exec_process_id.to_string(),
                    false,
                    1,
                    None,
                )
                .unwrap();

                let fd = {
                    if d.has_fd {
                        Some(wfd)
                    } else {
                        unistd::close(wfd).unwrap();
                        None
                    }
                };

                if d.has_tty {
                    exec_process.parent_stdin = None;
                    exec_process.term_master = fd;
                } else {
                    exec_process.parent_stdin = fd;
                    exec_process.term_master = None;
                }
                linux_container
                    .processes
                    .insert(exec_process_id, exec_process);

                sandbox.add_container(linux_container);
            }

            let agent_service = Box::new(AgentService {
                sandbox: Arc::new(Mutex::new(sandbox)),
                init_mode: true,
                oma: None,
            });

            let result = agent_service
                .do_write_stream(protocols::agent::WriteStreamRequest {
                    container_id: d.container_id.to_string(),
                    exec_id: d.exec_id.to_string(),
                    data: d.data.clone(),
                    ..Default::default()
                })
                .await;

            if !d.break_pipe {
                unistd::close(rfd).unwrap();
            }
            // XXX: Do not close wfd.
            // the fd will be closed on Process's dropping.
            // unistd::close(wfd).unwrap();

            let msg = format!("{}, result: {:?}", msg, result);
            assert_result!(d.result, result, msg);
        }
    }
    #[tokio::test]
    async fn test_update_container_namespaces() {
        #[derive(Debug)]
        struct TestData<'a> {
            has_linux_in_spec: bool,
            sandbox_pidns_path: Option<&'a str>,

            namespaces: Vec<LinuxNamespace>,
            use_sandbox_pidns: bool,
            result: Result<()>,
            expected_namespaces: Vec<LinuxNamespace>,
        }

        impl Default for TestData<'_> {
            fn default() -> Self {
                TestData {
                    has_linux_in_spec: true,
                    sandbox_pidns_path: Some("sharedpidns"),
                    namespaces: vec![
                        LinuxNamespaceBuilder::default()
                            .typ(LinuxNamespaceType::Ipc)
                            .path("ipcpath")
                            .build()
                            .unwrap(),
                        LinuxNamespaceBuilder::default()
                            .typ(LinuxNamespaceType::Uts)
                            .path("utspath")
                            .build()
                            .unwrap(),
                    ],
                    use_sandbox_pidns: false,
                    result: Ok(()),
                    expected_namespaces: vec![
                        LinuxNamespaceBuilder::default()
                            .typ(LinuxNamespaceType::Ipc)
                            .build()
                            .unwrap(),
                        LinuxNamespaceBuilder::default()
                            .typ(LinuxNamespaceType::Uts)
                            .build()
                            .unwrap(),
                        LinuxNamespaceBuilder::default()
                            .typ(LinuxNamespaceType::Pid)
                            .build()
                            .unwrap(),
                    ],
                }
            }
        }

        let tests = &[
            TestData {
                ..Default::default()
            },
            TestData {
                use_sandbox_pidns: true,
                expected_namespaces: vec![
                    LinuxNamespaceBuilder::default()
                        .typ(LinuxNamespaceType::Ipc)
                        .build()
                        .unwrap(),
                    LinuxNamespaceBuilder::default()
                        .typ(LinuxNamespaceType::Uts)
                        .build()
                        .unwrap(),
                    LinuxNamespaceBuilder::default()
                        .typ(LinuxNamespaceType::Pid)
                        .path("sharedpidns")
                        .build()
                        .unwrap(),
                ],
                ..Default::default()
            },
            TestData {
                namespaces: vec![],
                use_sandbox_pidns: true,
                expected_namespaces: vec![LinuxNamespaceBuilder::default()
                    .typ(LinuxNamespaceType::Pid)
                    .path("sharedpidns")
                    .build()
                    .unwrap()],
                ..Default::default()
            },
            TestData {
                namespaces: vec![],
                use_sandbox_pidns: false,
                expected_namespaces: vec![LinuxNamespaceBuilder::default()
                    .typ(LinuxNamespaceType::Pid)
                    .build()
                    .unwrap()],
                ..Default::default()
            },
            TestData {
                has_linux_in_spec: false,
                result: Err(anyhow!(ERR_NO_LINUX_FIELD)),
                ..Default::default()
            },
        ];

        for (i, d) in tests.iter().enumerate() {
            let msg = format!("test[{}]: {:?}", i, d);

            let logger = slog::Logger::root(slog::Discard, o!());
            let mut sandbox = Sandbox::new(&logger).unwrap();
            if let Some(pidns_path) = d.sandbox_pidns_path {
                let mut sandbox_pidns = Namespace::new(&logger);
                sandbox_pidns.path = pidns_path.to_string();
                sandbox.sandbox_pidns = Some(sandbox_pidns);
            }

            let mut oci = Spec::default();
            oci.set_linux(None);
            if d.has_linux_in_spec {
                let mut linux = Linux::default();
                linux.set_namespaces(Some(d.namespaces.clone()));
                oci.set_linux(Some(linux));
            }

            let result = update_container_namespaces(&sandbox, &mut oci, d.use_sandbox_pidns);

            let msg = format!("{}, result: {:?}", msg, result);

            assert_result!(d.result, result, msg);
            if let Some(linux) = oci.linux() {
                assert_eq!(
                    d.expected_namespaces,
                    linux.namespaces().clone().unwrap(),
                    "{}",
                    msg
                );
            }
        }
    }

    #[tokio::test]
    async fn test_get_memory_info() {
        #[derive(Debug)]
        struct TestData<'a> {
            // if None is provided, no file will be generated, else the data in the Option will populate the file
            block_size_data: Option<&'a str>,

            hotplug_probe_data: bool,
            get_block_size: bool,
            get_hotplug: bool,
            result: Result<(u64, bool)>,
        }

        let tests = &[
            TestData {
                block_size_data: Some("10000000"),
                hotplug_probe_data: true,
                get_block_size: true,
                get_hotplug: true,
                result: Ok((268435456, true)),
            },
            TestData {
                block_size_data: Some("100"),
                hotplug_probe_data: false,
                get_block_size: true,
                get_hotplug: true,
                result: Ok((256, false)),
            },
            TestData {
                block_size_data: None,
                hotplug_probe_data: false,
                get_block_size: true,
                get_hotplug: true,
                result: Ok((0, false)),
            },
            TestData {
                block_size_data: Some(""),
                hotplug_probe_data: false,
                get_block_size: true,
                get_hotplug: false,
                result: Err(anyhow!(ERR_INVALID_BLOCK_SIZE)),
            },
            TestData {
                block_size_data: Some("-1"),
                hotplug_probe_data: false,
                get_block_size: true,
                get_hotplug: false,
                result: Err(anyhow!(ERR_INVALID_BLOCK_SIZE)),
            },
            TestData {
                block_size_data: Some("    "),
                hotplug_probe_data: false,
                get_block_size: true,
                get_hotplug: false,
                result: Err(anyhow!(ERR_INVALID_BLOCK_SIZE)),
            },
            TestData {
                block_size_data: Some("some data"),
                hotplug_probe_data: false,
                get_block_size: true,
                get_hotplug: false,
                result: Err(anyhow!(ERR_INVALID_BLOCK_SIZE)),
            },
            TestData {
                block_size_data: Some("some data"),
                hotplug_probe_data: true,
                get_block_size: false,
                get_hotplug: false,
                result: Ok((0, false)),
            },
            TestData {
                block_size_data: Some("100"),
                hotplug_probe_data: true,
                get_block_size: false,
                get_hotplug: false,
                result: Ok((0, false)),
            },
            TestData {
                block_size_data: Some("100"),
                hotplug_probe_data: true,
                get_block_size: false,
                get_hotplug: true,
                result: Ok((0, true)),
            },
        ];

        for (i, d) in tests.iter().enumerate() {
            let msg = format!("test[{}]: {:?}", i, d);

            let dir = tempdir().expect("failed to make tempdir");
            let block_size_path = dir.path().join("block_size_bytes");
            let hotplug_probe_path = dir.path().join("probe");

            if let Some(block_size_data) = d.block_size_data {
                fs::write(&block_size_path, block_size_data).unwrap();
            }
            if d.hotplug_probe_data {
                fs::write(&hotplug_probe_path, []).unwrap();
            }

            let result = get_memory_info(
                d.get_block_size,
                d.get_hotplug,
                block_size_path.to_str().unwrap(),
                hotplug_probe_path.to_str().unwrap(),
            );

            let msg = format!("{}, result: {:?}", msg, result);

            assert_result!(d.result, result, msg);
        }
    }

    #[tokio::test]
    async fn test_is_signal_handled() {
        #[derive(Debug)]
        struct TestData<'a> {
            status_file_data: Option<&'a str>,
            signum: u32,
            result: bool,
        }

        let tests = &[
            TestData {
                status_file_data: Some(
                    r#"
SigBlk:0000000000010000
SigCgt:0000000000000001
OtherField:other
                "#,
                ),
                signum: 1,
                result: true,
            },
            TestData {
                status_file_data: Some("SigCgt:000000004b813efb"),
                signum: 4,
                result: true,
            },
            TestData {
                status_file_data: Some("SigCgt:\t000000004b813efb"),
                signum: 4,
                result: true,
            },
            TestData {
                status_file_data: Some("SigCgt: 000000004b813efb"),
                signum: 4,
                result: true,
            },
            TestData {
                status_file_data: Some("SigCgt:000000004b813efb "),
                signum: 4,
                result: true,
            },
            TestData {
                status_file_data: Some("SigCgt:\t000000004b813efb "),
                signum: 4,
                result: true,
            },
            TestData {
                status_file_data: Some("SigCgt:000000004b813efb"),
                signum: 3,
                result: false,
            },
            TestData {
                status_file_data: Some("SigCgt:000000004b813efb"),
                signum: 65,
                result: false,
            },
            TestData {
                status_file_data: Some("SigCgt:000000004b813efb"),
                signum: 0,
                result: true,
            },
            TestData {
                status_file_data: Some("SigCgt:ZZZZZZZZ"),
                signum: 1,
                result: false,
            },
            TestData {
                status_file_data: Some("SigCgt:-1"),
                signum: 1,
                result: false,
            },
            TestData {
                status_file_data: Some("SigCgt"),
                signum: 1,
                result: false,
            },
            TestData {
                status_file_data: Some("any data"),
                signum: 0,
                result: true,
            },
            TestData {
                status_file_data: Some("SigBlk:0000000000000001"),
                signum: 1,
                result: true,
            },
            TestData {
                status_file_data: Some("SigIgn:0000000000000001"),
                signum: 1,
                result: true,
            },
            TestData {
                status_file_data: None,
                signum: 1,
                result: false,
            },
            TestData {
                status_file_data: None,
                signum: 0,
                result: false,
            },
        ];

        for (i, d) in tests.iter().enumerate() {
            let msg = format!("test[{}]: {:?}", i, d);

            let dir = tempdir().expect("failed to make tempdir");
            let proc_status_file_path = dir.path().join("status");

            if let Some(file_data) = d.status_file_data {
                fs::write(&proc_status_file_path, file_data).unwrap();
            }

            let result = is_signal_handled(proc_status_file_path.to_str().unwrap(), d.signum);

            let msg = format!("{}, result: {:?}", msg, result);

            assert_eq!(d.result, result, "{}", msg);
        }
    }

    #[tokio::test]
    async fn test_volume_capacity_stats() {
        skip_if_not_root!();

        // Verify error if path does not exist
        assert!(get_volume_capacity_stats("/does-not-exist").is_err());

        // Create a new tmpfs mount, and verify the initial values
        let mount_dir = tempfile::tempdir().unwrap();
        mount::mount(
            Some("tmpfs"),
            mount_dir.path().to_str().unwrap(),
            Some("tmpfs"),
            mount::MsFlags::empty(),
            None::<&str>,
        )
        .unwrap();
        let mut stats = get_volume_capacity_stats(mount_dir.path().to_str().unwrap()).unwrap();
        assert_eq!(stats.used, 0);
        assert_ne!(stats.available, 0);
        let available = stats.available;

        // Verify that writing a file will result in increased utilization
        fs::write(mount_dir.path().join("file.dat"), "foobar").unwrap();
        stats = get_volume_capacity_stats(mount_dir.path().to_str().unwrap()).unwrap();

        let size = get_block_size(mount_dir.path().to_str().unwrap()).unwrap();

        assert_eq!(stats.used, size);
        assert_eq!(stats.available, available - size);
    }

    fn get_block_size(path: &str) -> Result<u64, Errno> {
        let stat = statfs::statfs(path)?;
        let block_size = stat.block_size() as u64;
        Ok(block_size)
    }

    #[tokio::test]
    async fn test_get_volume_inode_stats() {
        skip_if_not_root!();

        // Verify error if path does not exist
        assert!(get_volume_inode_stats("/does-not-exist").is_err());

        // Create a new tmpfs mount, and verify the initial values
        let mount_dir = tempfile::tempdir().unwrap();
        mount::mount(
            Some("tmpfs"),
            mount_dir.path().to_str().unwrap(),
            Some("tmpfs"),
            mount::MsFlags::empty(),
            None::<&str>,
        )
        .unwrap();
        let mut stats = get_volume_inode_stats(mount_dir.path().to_str().unwrap()).unwrap();
        assert_eq!(stats.used, 1);
        assert_ne!(stats.available, 0);
        let available = stats.available;

        // Verify that creating a directory and writing a file will result in increased utilization
        let dir = mount_dir.path().join("foobar");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.as_path().join("file.dat"), "foobar").unwrap();
        stats = get_volume_inode_stats(mount_dir.path().to_str().unwrap()).unwrap();

        assert_eq!(stats.used, 3);
        assert_eq!(stats.available, available - 2);
    }

    #[tokio::test]
    async fn test_ip_tables() {
        skip_if_not_root!();

        let iptables_cmd_list = [
            USR_IPTABLES_SAVE,
            USR_IP6TABLES_SAVE,
            USR_IPTABLES_RESTORE,
            USR_IP6TABLES_RESTORE,
            IPTABLES_SAVE,
            IP6TABLES_SAVE,
            IPTABLES_RESTORE,
            IP6TABLES_RESTORE,
        ];

        for cmd in iptables_cmd_list {
            if !check_command(cmd) {
                warn!(
                    sl(),
                    "one or more commands for ip tables test are missing, skip it"
                );
                return;
            }
        }

        let logger = slog::Logger::root(slog::Discard, o!());
        let sandbox = Sandbox::new(&logger).unwrap();
        let agent_service = Box::new(AgentService {
            sandbox: Arc::new(Mutex::new(sandbox)),
            init_mode: true,
            oma: None,
        });

        let ctx = mk_ttrpc_context();

        // Move to a new netns in order to ensure we don't trash the hosts' iptables
        unshare(CloneFlags::CLONE_NEWNET).unwrap();

        // Get initial iptables, we expect to be empty:
        let result = agent_service
            .get_ip_tables(
                &ctx,
                GetIPTablesRequest {
                    is_ipv6: false,
                    ..Default::default()
                },
            )
            .await;
        assert!(result.is_ok(), "get ip tables should succeed");
        assert_eq!(
            result.unwrap().data.len(),
            0,
            "ip tables should be empty initially"
        );

        // Initial ip6 ip tables should also be empty:
        let result = agent_service
            .get_ip_tables(
                &ctx,
                GetIPTablesRequest {
                    is_ipv6: true,
                    ..Default::default()
                },
            )
            .await;
        assert!(result.is_ok(), "get ip6 tables should succeed");
        assert_eq!(
            result.unwrap().data.len(),
            0,
            "ip tables should be empty initially"
        );

        // Verify that attempting to write 'empty' iptables results in no error:
        let empty_rules = "";
        let result = agent_service
            .set_ip_tables(
                &ctx,
                SetIPTablesRequest {
                    is_ipv6: false,
                    data: empty_rules.as_bytes().to_vec(),
                    ..Default::default()
                },
            )
            .await;
        assert!(result.is_ok(), "set ip tables with no data should succeed");

        // Verify that attempting to write "garbage" iptables results in an error:
        let garbage_rules = r#"
this
is
just garbage
"#;
        let result = agent_service
            .set_ip_tables(
                &ctx,
                SetIPTablesRequest {
                    is_ipv6: false,
                    data: garbage_rules.as_bytes().to_vec(),
                    ..Default::default()
                },
            )
            .await;
        assert!(result.is_err(), "set iptables with garbage should fail");

        // Verify setup of valid iptables:Setup  valid set of iptables:
        let valid_rules = r#"
*nat
-A PREROUTING -d 192.168.103.153/32 -j DNAT --to-destination 192.168.188.153

COMMIT

"#;
        let result = agent_service
            .set_ip_tables(
                &ctx,
                SetIPTablesRequest {
                    is_ipv6: false,
                    data: valid_rules.as_bytes().to_vec(),
                    ..Default::default()
                },
            )
            .await;
        assert!(result.is_ok(), "set ip tables should succeed");

        let result = agent_service
            .get_ip_tables(
                &ctx,
                GetIPTablesRequest {
                    is_ipv6: false,
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert!(!result.data.is_empty(), "we should have non-zero output:");
        assert!(
            std::str::from_utf8(&result.data).unwrap().contains(
                "PREROUTING -d 192.168.103.153/32 -j DNAT --to-destination 192.168.188.153"
            ),
            "We should see the resulting rule"
        );

        // Verify setup of valid ip6tables:
        let valid_ipv6_rules = r#"
*filter
-A INPUT -s 2001:db8:100::1/128 -i sit+ -p tcp -m tcp --sport 512:65535

COMMIT

"#;
        let result = agent_service
            .set_ip_tables(
                &ctx,
                SetIPTablesRequest {
                    is_ipv6: true,
                    data: valid_ipv6_rules.as_bytes().to_vec(),
                    ..Default::default()
                },
            )
            .await;
        assert!(result.is_ok(), "set ip6 tables should succeed");

        let result = agent_service
            .get_ip_tables(
                &ctx,
                GetIPTablesRequest {
                    is_ipv6: true,
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert!(!result.data.is_empty(), "we should have non-zero output:");
        assert!(
            std::str::from_utf8(&result.data)
                .unwrap()
                .contains("INPUT -s 2001:db8:100::1/128 -i sit+ -p tcp -m tcp --sport 512:65535"),
            "We should see the resulting rule"
        );
    }

    #[tokio::test]
    async fn test_is_sealed_secret_path() {
        #[derive(Debug)]
        struct TestData<'a> {
            source_path: &'a str,
            result: bool,
        }

        let tests = &[
            TestData {
                source_path: "/run/kata-containers/shared/containers/somefile",
                result: true,
            },
            TestData {
                source_path: "/run/kata-containers/shared/containers/a128482812bad768f404e063f225decd425fc94a673aec4add45a9caa1122ccb-75490e32e51da3ff-resolv.conf",
                result: false,
            },
            TestData {
                source_path: "/run/kata-containers/shared/containers/a128482812bad768f404e063f225decd425fc94a673aec4add45a9caa1122ccb-75490e32e51da3ff-termination-log",
                result: false,
            },
            TestData {
                source_path: "/run/kata-containers/shared/containers/a128482812bad768f404e063f225decd425fc94a673aec4add45a9caa1122ccb-75490e32e51da3ff-hostname",
                result: false,
            },
            TestData {
                source_path: "/run/kata-containers/shared/containers/a128482812bad768f404e063f225decd425fc94a673aec4add45a9caa1122ccb-75490e32e51da3ff-hosts",
                result: false,
            },
            TestData {
                source_path: "/run/kata-containers/shared/containers/a128482812bad768f404e063f225decd425fc94a673aec4add45a9caa1122ccb-75490e32e51da3ff-serviceaccount",
                result: false,
            },
            TestData {
                source_path: "/run/kata-containers/shared/containers/a128482812bad768f404e063f225decd425fc94a673aec4add45a9caa1122ccb-75490e32e51da3ff-mysecret",
                result: true,
            },
            TestData {
                source_path: "/some/other/path",
                result: false,
            },
        ];

        for (i, d) in tests.iter().enumerate() {
            let msg = format!("test[{}]: {:?}", i, d);
            let result = is_sealed_secret_path(d.source_path);
            assert_eq!(d.result, result, "{}", msg);
        }
    }
}
