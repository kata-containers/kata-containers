// Copyright (c) 2019 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

use async_trait::async_trait;
use rustjail::{pipestream::PipeStream, process::StreamType};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt, ReadHalf};
use tokio::sync::Mutex;

use std::ffi::CString;
use std::io;
use std::path::Path;
use std::sync::Arc;
use ttrpc::{
    self,
    error::get_rpc_status,
    r#async::{Server as TtrpcServer, TtrpcContext},
};

use anyhow::{anyhow, Context, Result};
use cgroups::freezer::FreezerState;
use oci::{LinuxNamespace, Root, Spec};
use protobuf::{MessageDyn, MessageField};
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
use protocols::{
    agent_ttrpc_async as agent_ttrpc, health_ttrpc_async as health_ttrpc,
    image_ttrpc_async as image_ttrpc,
};
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

use crate::device::{
    add_devices, get_virtio_blk_pci_device_name, update_device_cgroup, update_env_pci,
};
use crate::image_rpc;
use crate::linux_abi::*;
use crate::metrics::get_metrics;
use crate::mount::{add_storages, baremount, update_ephemeral_mounts, STORAGE_HANDLER_LIST};
use crate::namespace::{NSTYPEIPC, NSTYPEPID, NSTYPEUTS};
use crate::network::setup_guest_dns;
use crate::pci;
use crate::random;
use crate::sandbox::Sandbox;
use crate::version::{AGENT_VERSION, API_VERSION};
use crate::AGENT_CONFIG;

#[cfg(feature = "agent-policy")]
use crate::AGENT_POLICY;

use crate::trace_rpc_call;
use crate::tracer::extract_carrier_from_ttrpc;
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

pub const CONTAINER_BASE: &str = "/run/kata-containers";
const MODPROBE_PATH: &str = "/sbin/modprobe";
const ANNO_K8S_IMAGE_NAME: &str = "io.kubernetes.cri.image-name";
const CONFIG_JSON: &str = "config.json";
const INIT_TRUSTED_STORAGE: &str = "/usr/bin/kata-init-trusted-storage";
const TRUSTED_STORAGE_DEVICE: &str = "/dev/trusted_store";

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

// Convenience macro to obtain the scope logger
macro_rules! sl {
    () => {
        slog_scope::logger()
    };
}

// Convenience macro to wrap an error and response to ttrpc client
macro_rules! ttrpc_error {
    ($code:path, $err:expr $(,)?) => {
        get_rpc_status($code, format!("{:?}", $err))
    };
}

macro_rules! config_allows {
    ($req:ident) => {
        if !AGENT_CONFIG
            .read()
            .await
            .is_allowed_endpoint($req.descriptor_dyn().name())
        {
            return Err(ttrpc_error!(
                ttrpc::Code::UNIMPLEMENTED,
                format!("{} is blocked", $req.descriptor_dyn().name()),
            ));
        }
    };
}

#[cfg(feature = "agent-policy")]
macro_rules! policy_allows {
    ($req:ident, $serialized_req:ident) => {
        let mut policy = AGENT_POLICY.lock().await;
        if !policy
            .is_allowed_endpoint($req.descriptor_dyn().name(), &$serialized_req)
            .await
        {
            warn!(
                sl!(),
                "{} is blocked by policy",
                $req.descriptor_dyn().name()
            );
            return Err(ttrpc_error!(
                ttrpc::Code::PERMISSION_DENIED,
                format!("{} is blocked by policy", $req.descriptor_dyn().name()),
            ));
        }
    };
}

macro_rules! is_allowed {
    ($req:ident) => {
        config_allows!($req);
        #[cfg(feature = "agent-policy")]
        {
            let request = serde_json::to_string(&$req).unwrap();
            policy_allows!($req, request);
        }
    };
}

/// OPA input data for CreateContainerRequest. The "OCI" field of
/// the input request is converted into the "oci" field below.
#[derive(Debug, Serialize, Deserialize)]
struct CreateContainerRequestData {
    oci: oci::Spec,
    storages: Vec<protocols::agent::Storage>,
}

#[derive(Clone, Debug)]
pub struct AgentService {
    sandbox: Arc<Mutex<Sandbox>>,
    init_mode: bool,
}

// A container ID must match this regex:
//
//     ^[a-zA-Z0-9][a-zA-Z0-9_.-]+$
//
pub fn verify_cid(id: &str) -> Result<()> {
    let mut chars = id.chars();

    let valid = matches!(chars.next(), Some(first) if first.is_alphanumeric()
                && id.len() > 1
                && chars.all(|c| c.is_alphanumeric() || ['.', '-', '_'].contains(&c)));

    match valid {
        true => Ok(()),
        false => Err(anyhow!("invalid container ID: {:?}", id)),
    }
}

// Partially merge an OCI process specification into another one.
fn merge_oci_process(target: &mut oci::Process, source: &oci::Process) {
    if target.args.is_empty() && !source.args.is_empty() {
        target.args.append(&mut source.args.clone());
    }

    if target.cwd == "/" && source.cwd != "/" {
        target.cwd = String::from(&source.cwd);
    }

    for source_env in &source.env {
        let variable_name: Vec<&str> = source_env.split('=').collect();
        if !target.env.iter().any(|i| i.contains(variable_name[0])) {
            target.env.push(source_env.to_string());
        }
    }
}

impl AgentService {
    #[instrument]
    async fn do_create_container(
        &self,
        req: protocols::agent::CreateContainerRequest,
    ) -> Result<()> {
        let cid = req.container_id.clone();

        kata_sys_util::validate::verify_id(&cid)?;

        let mut oci_spec = req.OCI.clone();
        let use_sandbox_pidns = req.sandbox_pidns();

        let sandbox;
        let mut s;

        let mut oci = match oci_spec.as_mut() {
            Some(spec) => rustjail::grpc_to_oci(spec),
            None => {
                error!(sl!(), "no oci spec in the create container request!");
                return Err(anyhow!(nix::Error::EINVAL));
            }
        };

        info!(sl!(), "receive createcontainer, spec: {:?}", &oci);
        info!(
            sl!(),
            "receive createcontainer, storages: {:?}", &req.storages
        );

        // Merge the image bundle OCI spec into the container creation request OCI spec.
        self.merge_bundle_oci(&mut oci).await?;

        // Some devices need some extra processing (the ones invoked with
        // --device for instance), and that's what this call is doing. It
        // updates the devices listed in the OCI spec, so that they actually
        // match real devices inside the VM. This step is necessary since we
        // cannot predict everything from the caller.
        add_devices(&req.devices.to_vec(), &mut oci, &self.sandbox).await?;

        let linux = oci
            .linux
            .as_mut()
            .ok_or_else(|| anyhow!("Spec didn't contain linux field"))?;

        for specdev in &mut linux.devices {
            let dev_major_minor = format!("{}:{}", specdev.major, specdev.minor);

            if specdev.path == TRUSTED_STORAGE_DEVICE {
                let data_integrity = AGENT_CONFIG.read().await.data_integrity;
                info!(
                    sl!(),
                    "trusted_store device major:min {}, enable data integrity {}",
                    dev_major_minor,
                    data_integrity.to_string()
                );

                Command::new(INIT_TRUSTED_STORAGE)
                    .args([&dev_major_minor, &data_integrity.to_string()])
                    .output()
                    .expect("Failed to initialize confidential storage");
            }
        }

        // Both rootfs and volumes (invoked with --volume for instance) will
        // be processed the same way. The idea is to always mount any provided
        // storage to the specified MountPoint, so that it will match what's
        // inside oci.Mounts.
        // After all those storages have been processed, no matter the order
        // here, the agent will rely on rustjail (using the oci.Mounts
        // list) to bind mount all of them inside the container.
        let m = add_storages(
            sl!(),
            req.storages.to_vec(),
            self.sandbox.clone(),
            Some(req.container_id.clone()),
        )
        .await?;
        {
            sandbox = self.sandbox.clone();
            s = sandbox.lock().await;
            s.container_mounts.insert(cid.clone(), m);
        }

        update_container_namespaces(&s, &mut oci, use_sandbox_pidns)?;

        // Add the root partition to the device cgroup to prevent access
        update_device_cgroup(&mut oci)?;

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
        let cgroups_path = oci.linux.as_ref().map_or("", |linux| &linux.cgroups_path);
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
        };

        let mut ctr: LinuxContainer =
            LinuxContainer::new(cid.as_str(), CONTAINER_BASE, opts, &sl!())?;

        let pipe_size = AGENT_CONFIG.read().await.container_pipe_size;

        let p = if let Some(p) = oci.process {
            Process::new(&sl!(), &p, cid.as_str(), true, pipe_size)?
        } else {
            info!(sl!(), "no process configurations!");
            return Err(anyhow!(nix::Error::EINVAL));
        };

        // if starting container failed, we will do some rollback work
        // to ensure no resources are leaked.
        if let Err(err) = ctr.start(p).await {
            error!(sl!(), "failed to start container: {:?}", err);
            if let Err(e) = ctr.destroy().await {
                error!(sl!(), "failed to destroy container: {:?}", e);
            }
            if let Err(e) = remove_container_resources(&mut s, &cid) {
                error!(sl!(), "failed to remove container resources: {:?}", e);
            }
            return Err(err);
        }

        s.update_shared_pidns(&ctr)?;
        s.add_container(ctr);
        info!(sl!(), "created container!");

        Ok(())
    }

    #[instrument]
    async fn do_start_container(&self, req: protocols::agent::StartContainerRequest) -> Result<()> {
        let cid = req.container_id;

        let sandbox = self.sandbox.clone();
        let mut s = sandbox.lock().await;
        let sid = s.id.clone();

        let ctr = s
            .get_container(&cid)
            .ok_or_else(|| anyhow!("Invalid container id"))?;

        ctr.exec().await?;

        if sid == cid {
            return Ok(());
        }

        // start oom event loop

        let cg_path = ctr.cgroup_manager.as_ref().get_cgroup_path("memory");

        if let Ok(cg_path) = cg_path {
            let rx = notifier::notify_oom(cid.as_str(), cg_path.to_string()).await?;

            s.run_oom_event_monitor(rx, cid.clone()).await;
        }

        Ok(())
    }

    #[instrument]
    async fn do_remove_container(
        &self,
        req: protocols::agent::RemoveContainerRequest,
    ) -> Result<()> {
        let cid = req.container_id.clone();

        if req.timeout == 0 {
            let s = Arc::clone(&self.sandbox);
            let mut sandbox = s.lock().await;

            sandbox.bind_watcher.remove_container(&cid).await;

            sandbox
                .get_container(&cid)
                .ok_or_else(|| anyhow!("Invalid container id"))?
                .destroy()
                .await?;

            remove_container_resources(&mut sandbox, &cid)?;

            return Ok(());
        }

        // timeout != 0
        let s = self.sandbox.clone();
        let cid2 = cid.clone();
        let (tx, rx) = tokio::sync::oneshot::channel::<i32>();

        let handle = tokio::spawn(async move {
            let mut sandbox = s.lock().await;
            if let Some(ctr) = sandbox.get_container(&cid2) {
                ctr.destroy().await.unwrap();
                sandbox.bind_watcher.remove_container(&cid2).await;
                tx.send(1).unwrap();
            };
        });

        if tokio::time::timeout(Duration::from_secs(req.timeout.into()), rx)
            .await
            .is_err()
        {
            return Err(anyhow!(nix::Error::ETIME));
        }

        if handle.await.is_err() {
            return Err(anyhow!(nix::Error::UnknownErrno));
        }

        let s = self.sandbox.clone();
        let mut sandbox = s.lock().await;
        remove_container_resources(&mut sandbox, &cid)?;

        Ok(())
    }

    #[instrument]
    async fn do_exec_process(&self, req: protocols::agent::ExecProcessRequest) -> Result<()> {
        let cid = req.container_id.clone();
        let exec_id = req.exec_id.clone();

        info!(sl!(), "do_exec_process cid: {} eid: {}", cid, exec_id);

        let s = self.sandbox.clone();
        let mut sandbox = s.lock().await;

        let mut process = req
            .process
            .into_option()
            .ok_or_else(|| anyhow!("Unable to parse process from ExecProcessRequest"))?;

        // Apply any necessary corrections for PCI addresses
        update_env_pci(&mut process.Env, &sandbox.pcimap)?;

        let pipe_size = AGENT_CONFIG.read().await.container_pipe_size;
        let ocip = rustjail::process_grpc_to_oci(&process);
        let p = Process::new(&sl!(), &ocip, exec_id.as_str(), false, pipe_size)?;

        let ctr = sandbox
            .get_container(&cid)
            .ok_or_else(|| anyhow!("Invalid container id"))?;

        ctr.run(p).await?;

        Ok(())
    }

    #[instrument]
    async fn do_signal_process(&self, req: protocols::agent::SignalProcessRequest) -> Result<()> {
        let cid = req.container_id.clone();
        let eid = req.exec_id.clone();
        let s = self.sandbox.clone();

        info!(
            sl!(),
            "signal process";
            "container-id" => cid.clone(),
            "exec-id" => eid.clone(),
            "signal" => req.signal,
        );

        let mut sig: libc::c_int = req.signal as libc::c_int;
        {
            let mut sandbox = s.lock().await;
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
                        sl!(),
                        "signal encounter ESRCH, continue";
                        "container-id" => cid.clone(),
                        "exec-id" => eid.clone(),
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
                sl!(),
                "signal all the remaining processes";
                "container-id" => cid.clone(),
                "exec-id" => eid.clone(),
            );

            if let Err(err) = self.freeze_cgroup(&cid, FreezerState::Frozen).await {
                warn!(
                    sl!(),
                    "freeze cgroup failed";
                    "container-id" => cid.clone(),
                    "exec-id" => eid.clone(),
                    "error" => format!("{:?}", err),
                );
            }

            let pids = self.get_pids(&cid).await?;
            for pid in pids.iter() {
                let res = unsafe { libc::kill(*pid, sig) };
                if let Err(err) = Errno::result(res).map(drop) {
                    warn!(
                        sl!(),
                        "signal failed";
                        "container-id" => cid.clone(),
                        "exec-id" => eid.clone(),
                        "pid" => pid,
                        "error" => format!("{:?}", err),
                    );
                }
            }
            if let Err(err) = self.freeze_cgroup(&cid, FreezerState::Thawed).await {
                warn!(
                    sl!(),
                    "unfreeze cgroup failed";
                    "container-id" => cid.clone(),
                    "exec-id" => eid.clone(),
                    "error" => format!("{:?}", err),
                );
            }
        }
        Ok(())
    }

    async fn freeze_cgroup(&self, cid: &str, state: FreezerState) -> Result<()> {
        let s = self.sandbox.clone();
        let mut sandbox = s.lock().await;
        let ctr = sandbox
            .get_container(cid)
            .ok_or_else(|| anyhow!("Invalid container id {}", cid))?;
        ctr.cgroup_manager.as_ref().freeze(state)?;
        Ok(())
    }

    async fn get_pids(&self, cid: &str) -> Result<Vec<i32>> {
        let s = self.sandbox.clone();
        let mut sandbox = s.lock().await;
        let ctr = sandbox
            .get_container(cid)
            .ok_or_else(|| anyhow!("Invalid container id {}", cid))?;
        let pids = ctr.cgroup_manager.as_ref().get_pids()?;
        Ok(pids)
    }

    #[instrument]
    async fn do_wait_process(
        &self,
        req: protocols::agent::WaitProcessRequest,
    ) -> Result<protocols::agent::WaitProcessResponse> {
        let cid = req.container_id.clone();
        let eid = req.exec_id;
        let s = self.sandbox.clone();
        let mut resp = WaitProcessResponse::new();
        let pid: pid_t;

        let (exit_send, mut exit_recv) = tokio::sync::mpsc::channel(100);

        info!(
            sl!(),
            "wait process";
            "container-id" => cid.clone(),
            "exec-id" => eid.clone()
        );

        let exit_rx = {
            let mut sandbox = s.lock().await;
            let p = sandbox.find_container_process(cid.as_str(), eid.as_str())?;

            p.exit_watchers.push(exit_send);
            pid = p.pid;

            p.exit_rx.clone()
        };

        if let Some(mut exit_rx) = exit_rx {
            info!(sl!(), "cid {} eid {} waiting for exit signal", &cid, &eid);
            while exit_rx.changed().await.is_ok() {}
            info!(sl!(), "cid {} eid {} received exit signal", &cid, &eid);
        }

        let mut sandbox = s.lock().await;
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
        let cid = req.container_id.clone();
        let eid = req.exec_id.clone();

        let writer = {
            let s = self.sandbox.clone();
            let mut sandbox = s.lock().await;
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

        let mut resp = WriteStreamResponse::new();
        resp.set_len(req.data.len() as u32);

        Ok(resp)
    }

    async fn do_read_stream(
        &self,
        req: protocols::agent::ReadStreamRequest,
        stdout: bool,
    ) -> Result<protocols::agent::ReadStreamResponse> {
        let cid = req.container_id;
        let eid = req.exec_id;

        let mut term_exit_notifier = Arc::new(tokio::sync::Notify::new());
        let reader = {
            let s = self.sandbox.clone();
            let mut sandbox = s.lock().await;

            let p = sandbox.find_container_process(cid.as_str(), eid.as_str())?;

            if p.term_master.is_some() {
                term_exit_notifier = p.term_exit_notifier.clone();
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

        if reader.is_none() {
            return Err(anyhow!("Unable to determine stream reader, is None"));
        }

        let reader = reader.ok_or_else(|| anyhow!("cannot get stream reader"))?;

        tokio::select! {
            _ = term_exit_notifier.notified() => {
                Err(anyhow!("eof"))
            }
            v = read_stream(reader, req.len as usize)  => {
                let vector = v?;
                let mut resp = ReadStreamResponse::new();
                resp.set_data(vector);

                Ok(resp)
            }
        }
    }

    // When being passed an image name through a container annotation, merge its
    // corresponding bundle OCI specification into the passed container creation one.
    async fn merge_bundle_oci(&self, container_oci: &mut oci::Spec) -> Result<()> {
        if let Some(image_name) = container_oci
            .annotations
            .get(&ANNO_K8S_IMAGE_NAME.to_string())
        {
            if let Some(container_id) = self.sandbox.clone().lock().await.images.get(image_name) {
                let image_oci_config_path = Path::new(CONTAINER_BASE)
                    .join(container_id)
                    .join(CONFIG_JSON);
                debug!(
                    sl!(),
                    "Image bundle config path: {:?}", image_oci_config_path
                );

                let image_oci =
                    oci::Spec::load(image_oci_config_path.to_str().ok_or_else(|| {
                        anyhow!(
                            "Invalid container image OCI config path {:?}",
                            image_oci_config_path
                        )
                    })?)
                    .context("load image bundle")?;

                if let Some(container_root) = container_oci.root.as_mut() {
                    if let Some(image_root) = image_oci.root.as_ref() {
                        let root_path = Path::new(CONTAINER_BASE)
                            .join(container_id)
                            .join(image_root.path.clone());
                        container_root.path =
                            String::from(root_path.to_str().ok_or_else(|| {
                                anyhow!("Invalid container image root path {:?}", root_path)
                            })?);
                    }
                }

                if let Some(container_process) = container_oci.process.as_mut() {
                    if let Some(image_process) = image_oci.process.as_ref() {
                        merge_oci_process(container_process, image_process);
                    }
                }
            }
        }

        Ok(())
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
        is_allowed!(req);
        match self.do_create_container(req).await {
            Err(e) => Err(ttrpc_error!(ttrpc::Code::INTERNAL, e)),
            Ok(_) => Ok(Empty::new()),
        }
    }

    async fn start_container(
        &self,
        ctx: &TtrpcContext,
        req: protocols::agent::StartContainerRequest,
    ) -> ttrpc::Result<Empty> {
        trace_rpc_call!(ctx, "start_container", req);
        is_allowed!(req);
        match self.do_start_container(req).await {
            Err(e) => Err(ttrpc_error!(ttrpc::Code::INTERNAL, e)),
            Ok(_) => Ok(Empty::new()),
        }
    }

    async fn remove_container(
        &self,
        ctx: &TtrpcContext,
        req: protocols::agent::RemoveContainerRequest,
    ) -> ttrpc::Result<Empty> {
        trace_rpc_call!(ctx, "remove_container", req);
        is_allowed!(req);

        match self.do_remove_container(req).await {
            Err(e) => Err(ttrpc_error!(ttrpc::Code::INTERNAL, e)),
            Ok(_) => Ok(Empty::new()),
        }
    }

    async fn exec_process(
        &self,
        ctx: &TtrpcContext,
        req: protocols::agent::ExecProcessRequest,
    ) -> ttrpc::Result<Empty> {
        trace_rpc_call!(ctx, "exec_process", req);
        is_allowed!(req);
        match self.do_exec_process(req).await {
            Err(e) => Err(ttrpc_error!(ttrpc::Code::INTERNAL, e)),
            Ok(_) => Ok(Empty::new()),
        }
    }

    async fn signal_process(
        &self,
        ctx: &TtrpcContext,
        req: protocols::agent::SignalProcessRequest,
    ) -> ttrpc::Result<Empty> {
        trace_rpc_call!(ctx, "signal_process", req);
        is_allowed!(req);
        match self.do_signal_process(req).await {
            Err(e) => Err(ttrpc_error!(ttrpc::Code::INTERNAL, e)),
            Ok(_) => Ok(Empty::new()),
        }
    }

    async fn wait_process(
        &self,
        ctx: &TtrpcContext,
        req: protocols::agent::WaitProcessRequest,
    ) -> ttrpc::Result<WaitProcessResponse> {
        trace_rpc_call!(ctx, "wait_process", req);
        is_allowed!(req);
        self.do_wait_process(req)
            .await
            .map_err(|e| ttrpc_error!(ttrpc::Code::INTERNAL, e))
    }

    async fn update_container(
        &self,
        ctx: &TtrpcContext,
        req: protocols::agent::UpdateContainerRequest,
    ) -> ttrpc::Result<Empty> {
        trace_rpc_call!(ctx, "update_container", req);
        is_allowed!(req);
        let cid = req.container_id.clone();
        let res = req.resources;

        let s = Arc::clone(&self.sandbox);
        let mut sandbox = s.lock().await;

        let ctr = sandbox.get_container(&cid).ok_or_else(|| {
            ttrpc_error!(
                ttrpc::Code::INVALID_ARGUMENT,
                "invalid container id".to_string(),
            )
        })?;

        let resp = Empty::new();

        if let Some(res) = res.as_ref() {
            let oci_res = rustjail::resources_grpc_to_oci(res);
            match ctr.set(oci_res) {
                Err(e) => {
                    return Err(ttrpc_error!(ttrpc::Code::INTERNAL, e));
                }

                Ok(_) => return Ok(resp),
            }
        }

        Ok(resp)
    }

    async fn stats_container(
        &self,
        ctx: &TtrpcContext,
        req: protocols::agent::StatsContainerRequest,
    ) -> ttrpc::Result<StatsContainerResponse> {
        trace_rpc_call!(ctx, "stats_container", req);
        is_allowed!(req);
        let cid = req.container_id;
        let s = Arc::clone(&self.sandbox);
        let mut sandbox = s.lock().await;

        let ctr = sandbox.get_container(&cid).ok_or_else(|| {
            ttrpc_error!(
                ttrpc::Code::INVALID_ARGUMENT,
                "invalid container id".to_string(),
            )
        })?;

        ctr.stats()
            .map_err(|e| ttrpc_error!(ttrpc::Code::INTERNAL, e))
    }

    async fn pause_container(
        &self,
        ctx: &TtrpcContext,
        req: protocols::agent::PauseContainerRequest,
    ) -> ttrpc::Result<protocols::empty::Empty> {
        trace_rpc_call!(ctx, "pause_container", req);
        is_allowed!(req);
        let cid = req.container_id();
        let s = Arc::clone(&self.sandbox);
        let mut sandbox = s.lock().await;

        let ctr = sandbox.get_container(cid).ok_or_else(|| {
            ttrpc_error!(
                ttrpc::Code::INVALID_ARGUMENT,
                "invalid container id".to_string(),
            )
        })?;

        ctr.pause()
            .map_err(|e| ttrpc_error!(ttrpc::Code::INTERNAL, e))?;

        Ok(Empty::new())
    }

    async fn resume_container(
        &self,
        ctx: &TtrpcContext,
        req: protocols::agent::ResumeContainerRequest,
    ) -> ttrpc::Result<protocols::empty::Empty> {
        trace_rpc_call!(ctx, "resume_container", req);
        is_allowed!(req);
        let cid = req.container_id();
        let s = Arc::clone(&self.sandbox);
        let mut sandbox = s.lock().await;

        let ctr = sandbox.get_container(cid).ok_or_else(|| {
            ttrpc_error!(
                ttrpc::Code::INVALID_ARGUMENT,
                "invalid container id".to_string(),
            )
        })?;

        ctr.resume()
            .map_err(|e| ttrpc_error!(ttrpc::Code::INTERNAL, e))?;

        Ok(Empty::new())
    }

    async fn remove_stale_virtiofs_share_mounts(
        &self,
        ctx: &TtrpcContext,
        req: protocols::agent::RemoveStaleVirtiofsShareMountsRequest,
    ) -> ttrpc::Result<Empty> {
        trace_rpc_call!(ctx, "remove_stale_virtiofs_share_mounts", req);
        is_allowed!(req);
        let mount_infos = parse_mount_table("/proc/self/mountinfo")
            .map_err(|e| ttrpc_error!(ttrpc::Code::INTERNAL, e))?;
        for m in &mount_infos {
            if m.mount_point.starts_with(KATA_GUEST_SHARE_DIR) {
                // stat the mount point, virtiofs daemon will remove the stale cache and release the fds if the mount point doesn't exist any more.
                // More details in https://github.com/kata-containers/kata-containers/issues/6455#issuecomment-1477137277
                match stat::stat(Path::new(&m.mount_point)) {
                    Ok(_) => info!(sl!(), "stat {} success", m.mount_point),
                    Err(e) => info!(sl!(), "stat {} failed: {}", m.mount_point, e),
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
        is_allowed!(req);
        self.do_write_stream(req)
            .await
            .map_err(|e| ttrpc_error!(ttrpc::Code::INTERNAL, e))
    }

    async fn read_stdout(
        &self,
        _ctx: &TtrpcContext,
        req: protocols::agent::ReadStreamRequest,
    ) -> ttrpc::Result<ReadStreamResponse> {
        is_allowed!(req);
        self.do_read_stream(req, true)
            .await
            .map_err(|e| ttrpc_error!(ttrpc::Code::INTERNAL, e))
    }

    async fn read_stderr(
        &self,
        _ctx: &TtrpcContext,
        req: protocols::agent::ReadStreamRequest,
    ) -> ttrpc::Result<ReadStreamResponse> {
        is_allowed!(req);
        self.do_read_stream(req, false)
            .await
            .map_err(|e| ttrpc_error!(ttrpc::Code::INTERNAL, e))
    }

    async fn close_stdin(
        &self,
        ctx: &TtrpcContext,
        req: protocols::agent::CloseStdinRequest,
    ) -> ttrpc::Result<Empty> {
        trace_rpc_call!(ctx, "close_stdin", req);
        is_allowed!(req);

        let cid = req.container_id.clone();
        let eid = req.exec_id;
        let s = Arc::clone(&self.sandbox);
        let mut sandbox = s.lock().await;

        let p = sandbox
            .find_container_process(cid.as_str(), eid.as_str())
            .map_err(|e| {
                ttrpc_error!(
                    ttrpc::Code::INVALID_ARGUMENT,
                    format!("invalid argument: {:?}", e),
                )
            })?;

        p.close_stdin();

        Ok(Empty::new())
    }

    async fn tty_win_resize(
        &self,
        ctx: &TtrpcContext,
        req: protocols::agent::TtyWinResizeRequest,
    ) -> ttrpc::Result<Empty> {
        trace_rpc_call!(ctx, "tty_win_resize", req);
        is_allowed!(req);

        let cid = req.container_id.clone();
        let eid = req.exec_id.clone();
        let s = Arc::clone(&self.sandbox);
        let mut sandbox = s.lock().await;
        let p = sandbox
            .find_container_process(cid.as_str(), eid.as_str())
            .map_err(|e| {
                ttrpc_error!(
                    ttrpc::Code::UNAVAILABLE,
                    format!("invalid argument: {:?}", e),
                )
            })?;

        if let Some(fd) = p.term_master {
            unsafe {
                let win = winsize {
                    ws_row: req.row as c_ushort,
                    ws_col: req.column as c_ushort,
                    ws_xpixel: 0,
                    ws_ypixel: 0,
                };

                let err = libc::ioctl(fd, TIOCSWINSZ, &win);
                Errno::result(err).map(drop).map_err(|e| {
                    ttrpc_error!(ttrpc::Code::INTERNAL, format!("ioctl error: {:?}", e))
                })?;
            }
        } else {
            return Err(ttrpc_error!(ttrpc::Code::UNAVAILABLE, "no tty".to_string()));
        }

        Ok(Empty::new())
    }

    async fn update_interface(
        &self,
        ctx: &TtrpcContext,
        req: protocols::agent::UpdateInterfaceRequest,
    ) -> ttrpc::Result<Interface> {
        trace_rpc_call!(ctx, "update_interface", req);
        is_allowed!(req);

        let interface = req.interface.into_option().ok_or_else(|| {
            ttrpc_error!(
                ttrpc::Code::INVALID_ARGUMENT,
                "empty update interface request".to_string(),
            )
        })?;

        self.sandbox
            .lock()
            .await
            .rtnl
            .update_interface(&interface)
            .await
            .map_err(|e| {
                ttrpc_error!(ttrpc::Code::INTERNAL, format!("update interface: {:?}", e))
            })?;

        Ok(interface)
    }

    async fn update_routes(
        &self,
        ctx: &TtrpcContext,
        req: protocols::agent::UpdateRoutesRequest,
    ) -> ttrpc::Result<Routes> {
        trace_rpc_call!(ctx, "update_routes", req);
        is_allowed!(req);

        let new_routes = req.routes.into_option().map(|r| r.Routes).ok_or_else(|| {
            ttrpc_error!(
                ttrpc::Code::INVALID_ARGUMENT,
                "empty update routes request".to_string(),
            )
        })?;

        let mut sandbox = self.sandbox.lock().await;

        sandbox.rtnl.update_routes(new_routes).await.map_err(|e| {
            ttrpc_error!(
                ttrpc::Code::INTERNAL,
                format!("Failed to update routes: {:?}", e),
            )
        })?;

        let list = sandbox.rtnl.list_routes().await.map_err(|e| {
            ttrpc_error!(
                ttrpc::Code::INTERNAL,
                format!("Failed to list routes after update: {:?}", e),
            )
        })?;

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
        is_allowed!(req);

        match update_ephemeral_mounts(sl!(), req.storages.to_vec(), self.sandbox.clone()).await {
            Ok(_) => Ok(Empty::new()),
            Err(e) => Err(ttrpc_error!(
                ttrpc::Code::INTERNAL,
                format!("Failed to update mounts: {:?}", e),
            )),
        }
    }

    async fn get_ip_tables(
        &self,
        ctx: &TtrpcContext,
        req: GetIPTablesRequest,
    ) -> ttrpc::Result<GetIPTablesResponse> {
        trace_rpc_call!(ctx, "get_iptables", req);
        is_allowed!(req);

        info!(sl!(), "get_ip_tables: request received");

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

        match Command::new(cmd.clone()).output() {
            Ok(output) => Ok(GetIPTablesResponse {
                data: output.stdout,
                ..Default::default()
            }),
            Err(e) => {
                warn!(sl!(), "failed to run {}: {:?}", cmd, e.kind());
                return Err(ttrpc_error!(ttrpc::Code::INTERNAL, e));
            }
        }
    }

    async fn set_ip_tables(
        &self,
        ctx: &TtrpcContext,
        req: SetIPTablesRequest,
    ) -> ttrpc::Result<SetIPTablesResponse> {
        trace_rpc_call!(ctx, "set_iptables", req);
        is_allowed!(req);

        info!(sl!(), "set_ip_tables request received");

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

        let mut child = match Command::new(cmd.clone())
            .arg("--wait")
            .arg(IPTABLES_RESTORE_WAIT_SEC.to_string())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(child) => child,
            Err(e) => {
                warn!(sl!(), "failure to spawn {}: {:?}", cmd, e.kind());
                return Err(ttrpc_error!(ttrpc::Code::INTERNAL, e));
            }
        };

        let mut stdin = match child.stdin.take() {
            Some(si) => si,
            None => {
                println!("failed to get stdin from child");
                return Err(ttrpc_error!(
                    ttrpc::Code::INTERNAL,
                    "failed to take stdin from child".to_string()
                ));
            }
        };

        let (tx, rx) = tokio::sync::oneshot::channel::<i32>();
        let handle = tokio::spawn(async move {
            let _ = match stdin.write_all(&req.data) {
                Ok(o) => o,
                Err(e) => {
                    warn!(sl!(), "error writing stdin: {:?}", e.kind());
                    return;
                }
            };
            if tx.send(1).is_err() {
                warn!(sl!(), "stdin writer thread receiver dropped");
            };
        });

        if tokio::time::timeout(Duration::from_secs(IPTABLES_RESTORE_WAIT_SEC), rx)
            .await
            .is_err()
        {
            return Err(ttrpc_error!(
                ttrpc::Code::INTERNAL,
                "timeout waiting for stdin writer to complete".to_string()
            ));
        }

        if handle.await.is_err() {
            return Err(ttrpc_error!(
                ttrpc::Code::INTERNAL,
                "stdin writer thread failure".to_string()
            ));
        }

        let output = match child.wait_with_output() {
            Ok(o) => o,
            Err(e) => {
                warn!(
                    sl!(),
                    "failure waiting for spawned {} to complete: {:?}",
                    cmd,
                    e.kind()
                );
                return Err(ttrpc_error!(ttrpc::Code::INTERNAL, e));
            }
        };

        if !output.status.success() {
            warn!(sl!(), "{} failed: {:?}", cmd, output.stderr);
            return Err(ttrpc_error!(
                ttrpc::Code::INTERNAL,
                format!(
                    "{} failed: {:?}",
                    cmd,
                    String::from_utf8_lossy(&output.stderr)
                )
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
        is_allowed!(req);

        let list = self
            .sandbox
            .lock()
            .await
            .rtnl
            .list_interfaces()
            .await
            .map_err(|e| {
                ttrpc_error!(
                    ttrpc::Code::INTERNAL,
                    format!("Failed to list interfaces: {:?}", e),
                )
            })?;

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
        is_allowed!(req);

        let list = self
            .sandbox
            .lock()
            .await
            .rtnl
            .list_routes()
            .await
            .map_err(|e| ttrpc_error!(ttrpc::Code::INTERNAL, format!("list routes: {:?}", e)))?;

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
        is_allowed!(req);

        {
            let sandbox = self.sandbox.clone();
            let mut s = sandbox.lock().await;

            let _ = fs::remove_dir_all(CONTAINER_BASE);
            let _ = fs::create_dir_all(CONTAINER_BASE);

            s.hostname = req.hostname.clone();
            s.running = true;

            if !req.guest_hook_path.is_empty() {
                let _ = s.add_hooks(&req.guest_hook_path).map_err(|e| {
                    error!(
                        sl!(),
                        "add guest hook {} failed: {:?}", req.guest_hook_path, e
                    );
                });
            }

            if !req.sandbox_id.is_empty() {
                s.id = req.sandbox_id.clone();
            }

            for m in req.kernel_modules.iter() {
                load_kernel_module(m).map_err(|e| ttrpc_error!(ttrpc::Code::INTERNAL, e))?;
            }

            s.setup_shared_namespaces()
                .await
                .map_err(|e| ttrpc_error!(ttrpc::Code::INTERNAL, e))?;
        }

        match add_storages(sl!(), req.storages.to_vec(), self.sandbox.clone(), None).await {
            Ok(m) => {
                let sandbox = self.sandbox.clone();
                let mut s = sandbox.lock().await;
                s.mounts = m
            }
            Err(e) => return Err(ttrpc_error!(ttrpc::Code::INTERNAL, e)),
        };

        match setup_guest_dns(sl!(), req.dns.to_vec()) {
            Ok(_) => {
                let sandbox = self.sandbox.clone();
                let mut s = sandbox.lock().await;
                let _dns = req
                    .dns
                    .to_vec()
                    .iter()
                    .map(|dns| s.network.set_dns(dns.to_string()));
            }
            Err(e) => return Err(ttrpc_error!(ttrpc::Code::INTERNAL, e)),
        };

        Ok(Empty::new())
    }

    async fn destroy_sandbox(
        &self,
        ctx: &TtrpcContext,
        req: protocols::agent::DestroySandboxRequest,
    ) -> ttrpc::Result<Empty> {
        trace_rpc_call!(ctx, "destroy_sandbox", req);
        is_allowed!(req);

        let s = Arc::clone(&self.sandbox);
        let mut sandbox = s.lock().await;
        // destroy all containers, clean up, notify agent to exit
        // etc.
        sandbox
            .destroy()
            .await
            .map_err(|e| ttrpc_error!(ttrpc::Code::INTERNAL, e))?;
        // Close get_oom_event connection,
        // otherwise it will block the shutdown of ttrpc.
        sandbox.event_tx.take();

        sandbox
            .sender
            .take()
            .ok_or_else(|| {
                ttrpc_error!(
                    ttrpc::Code::INTERNAL,
                    "failed to get sandbox sender channel".to_string(),
                )
            })?
            .send(1)
            .map_err(|e| ttrpc_error!(ttrpc::Code::INTERNAL, e))?;

        Ok(Empty::new())
    }

    async fn add_arp_neighbors(
        &self,
        ctx: &TtrpcContext,
        req: protocols::agent::AddARPNeighborsRequest,
    ) -> ttrpc::Result<Empty> {
        trace_rpc_call!(ctx, "add_arp_neighbors", req);
        is_allowed!(req);

        let neighs = req
            .neighbors
            .into_option()
            .map(|n| n.ARPNeighbors)
            .ok_or_else(|| {
                ttrpc_error!(
                    ttrpc::Code::INVALID_ARGUMENT,
                    "empty add arp neighbours request".to_string(),
                )
            })?;

        self.sandbox
            .lock()
            .await
            .rtnl
            .add_arp_neighbors(neighs)
            .await
            .map_err(|e| {
                ttrpc_error!(
                    ttrpc::Code::INTERNAL,
                    format!("Failed to add ARP neighbours: {:?}", e),
                )
            })?;

        Ok(Empty::new())
    }

    async fn online_cpu_mem(
        &self,
        ctx: &TtrpcContext,
        req: protocols::agent::OnlineCPUMemRequest,
    ) -> ttrpc::Result<Empty> {
        is_allowed!(req);
        let s = Arc::clone(&self.sandbox);
        let sandbox = s.lock().await;
        trace_rpc_call!(ctx, "online_cpu_mem", req);

        sandbox
            .online_cpu_memory(&req)
            .map_err(|e| ttrpc_error!(ttrpc::Code::INTERNAL, e))?;

        Ok(Empty::new())
    }

    async fn reseed_random_dev(
        &self,
        ctx: &TtrpcContext,
        req: protocols::agent::ReseedRandomDevRequest,
    ) -> ttrpc::Result<Empty> {
        trace_rpc_call!(ctx, "reseed_random_dev", req);
        is_allowed!(req);

        random::reseed_rng(req.data.as_slice())
            .map_err(|e| ttrpc_error!(ttrpc::Code::INTERNAL, e))?;

        Ok(Empty::new())
    }

    async fn get_guest_details(
        &self,
        ctx: &TtrpcContext,
        req: protocols::agent::GuestDetailsRequest,
    ) -> ttrpc::Result<GuestDetailsResponse> {
        trace_rpc_call!(ctx, "get_guest_details", req);
        is_allowed!(req);

        info!(sl!(), "get guest details!");
        let mut resp = GuestDetailsResponse::new();
        // to get memory block size
        match get_memory_info(
            req.mem_block_size,
            req.mem_hotplug_probe,
            SYSFS_MEMORY_BLOCK_SIZE_PATH,
            SYSFS_MEMORY_HOTPLUG_PROBE_PATH,
        ) {
            Ok((u, v)) => {
                resp.mem_block_size_bytes = u;
                resp.support_mem_hotplug_probe = v;
            }
            Err(e) => {
                info!(sl!(), "fail to get memory info!");
                return Err(ttrpc_error!(ttrpc::Code::INTERNAL, e));
            }
        }

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
        is_allowed!(req);

        do_mem_hotplug_by_probe(&req.memHotplugProbeAddr)
            .map_err(|e| ttrpc_error!(ttrpc::Code::INTERNAL, e))?;

        Ok(Empty::new())
    }

    async fn set_guest_date_time(
        &self,
        ctx: &TtrpcContext,
        req: protocols::agent::SetGuestDateTimeRequest,
    ) -> ttrpc::Result<Empty> {
        trace_rpc_call!(ctx, "set_guest_date_time", req);
        is_allowed!(req);

        do_set_guest_date_time(req.Sec, req.Usec)
            .map_err(|e| ttrpc_error!(ttrpc::Code::INTERNAL, e))?;

        Ok(Empty::new())
    }

    async fn copy_file(
        &self,
        ctx: &TtrpcContext,
        req: protocols::agent::CopyFileRequest,
    ) -> ttrpc::Result<Empty> {
        trace_rpc_call!(ctx, "copy_file", req);
        is_allowed!(req);

        do_copy_file(&req).map_err(|e| ttrpc_error!(ttrpc::Code::INTERNAL, e))?;

        Ok(Empty::new())
    }

    async fn get_metrics(
        &self,
        ctx: &TtrpcContext,
        req: protocols::agent::GetMetricsRequest,
    ) -> ttrpc::Result<Metrics> {
        trace_rpc_call!(ctx, "get_metrics", req);
        is_allowed!(req);

        match get_metrics(&req) {
            Err(e) => Err(ttrpc_error!(ttrpc::Code::INTERNAL, e)),
            Ok(s) => {
                let mut metrics = Metrics::new();
                metrics.set_metrics(s);
                Ok(metrics)
            }
        }
    }

    async fn get_oom_event(
        &self,
        _ctx: &TtrpcContext,
        req: protocols::agent::GetOOMEventRequest,
    ) -> ttrpc::Result<OOMEvent> {
        is_allowed!(req);
        let sandbox = self.sandbox.clone();
        let s = sandbox.lock().await;
        let event_rx = &s.event_rx.clone();
        let mut event_rx = event_rx.lock().await;
        drop(s);
        drop(sandbox);

        if let Some(container_id) = event_rx.recv().await {
            info!(sl!(), "get_oom_event return {}", &container_id);

            let mut resp = OOMEvent::new();
            resp.container_id = container_id;

            return Ok(resp);
        }

        Err(ttrpc_error!(ttrpc::Code::INTERNAL, ""))
    }

    async fn get_volume_stats(
        &self,
        ctx: &TtrpcContext,
        req: VolumeStatsRequest,
    ) -> ttrpc::Result<VolumeStatsResponse> {
        trace_rpc_call!(ctx, "get_volume_stats", req);
        is_allowed!(req);

        info!(sl!(), "get volume stats!");
        let mut resp = VolumeStatsResponse::new();

        let mut condition = VolumeCondition::new();

        match File::open(&req.volume_guest_path) {
            Ok(_) => {
                condition.abnormal = false;
                condition.message = String::from("OK");
            }
            Err(e) => {
                info!(sl!(), "failed to open the volume");
                return Err(ttrpc_error!(ttrpc::Code::INTERNAL, e));
            }
        };

        let mut usage_vec = Vec::new();

        // to get volume capacity stats
        get_volume_capacity_stats(&req.volume_guest_path)
            .map(|u| usage_vec.push(u))
            .map_err(|e| ttrpc_error!(ttrpc::Code::INTERNAL, e))?;

        // to get volume inode stats
        get_volume_inode_stats(&req.volume_guest_path)
            .map(|u| usage_vec.push(u))
            .map_err(|e| ttrpc_error!(ttrpc::Code::INTERNAL, e))?;

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
        is_allowed!(req);

        do_add_swap(&self.sandbox, &req)
            .await
            .map_err(|e| ttrpc_error!(ttrpc::Code::INTERNAL, e))?;

        Ok(Empty::new())
    }

    #[cfg(feature = "agent-policy")]
    async fn set_policy(
        &self,
        ctx: &TtrpcContext,
        req: protocols::agent::SetPolicyRequest,
    ) -> ttrpc::Result<Empty> {
        trace_rpc_call!(ctx, "set_policy", req);
        is_allowed!(req);

        AGENT_POLICY
            .lock()
            .await
            .set_policy(&req.policy)
            .await
            .map_err(|e| ttrpc_error!(ttrpc::Code::INTERNAL, e))?;

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
        info!(sl!(), "version {:?}", req);
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
                    warn!(sl!(), "file {} is empty", block_size_path);
                    return Err(anyhow!(ERR_INVALID_BLOCK_SIZE));
                }

                size = u64::from_str_radix(v.trim(), 16).map_err(|_| {
                    warn!(sl!(), "failed to parse the str {} to hex", size);
                    anyhow!(ERR_INVALID_BLOCK_SIZE)
                })?;
            }
            Err(e) => {
                warn!(sl!(), "memory block size error: {:?}", e.kind());
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
                warn!(sl!(), "hotplug memory error: {:?}", e);
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
    detail.storage_handlers = STORAGE_HANDLER_LIST
        .to_vec()
        .iter()
        .map(|x| x.to_string())
        .collect();

    detail
}

async fn read_stream(reader: Arc<Mutex<ReadHalf<PipeStream>>>, l: usize) -> Result<Vec<u8>> {
    let mut content = vec![0u8; l];

    let mut reader = reader.lock().await;
    let len = reader.read(&mut content).await?;
    content.resize(len, 0);

    if len == 0 {
        return Err(anyhow!("read meet eof"));
    }

    Ok(content)
}

pub fn start(s: Arc<Mutex<Sandbox>>, server_address: &str, init_mode: bool) -> Result<TtrpcServer> {
    let agent_service = Box::new(AgentService {
        sandbox: s.clone(),
        init_mode,
    }) as Box<dyn agent_ttrpc::AgentService + Send + Sync>;

    let agent_worker = Arc::new(agent_service);

    let health_service = Box::new(HealthService {}) as Box<dyn health_ttrpc::Health + Send + Sync>;
    let health_worker = Arc::new(health_service);

    let image_service =
        Box::new(image_rpc::ImageService::new(s)) as Box<dyn image_ttrpc::Image + Send + Sync>;

    let aservice = agent_ttrpc::create_agent_service(agent_worker);

    let hservice = health_ttrpc::create_health(health_worker);

    let iservice = image_ttrpc::create_image(Arc::new(image_service));

    let server = TtrpcServer::new()
        .bind(server_address)?
        .register_service(aservice)
        .register_service(hservice)
        .register_service(iservice);

    info!(sl!(), "ttRPC server started"; "address" => server_address);

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
        .linux
        .as_mut()
        .ok_or_else(|| anyhow!(ERR_NO_LINUX_FIELD))?;

    let namespaces = linux.namespaces.as_mut_slice();
    for namespace in namespaces.iter_mut() {
        if namespace.r#type == NSTYPEIPC {
            namespace.path = sandbox.shared_ipcns.path.clone();
            continue;
        }
        if namespace.r#type == NSTYPEUTS {
            namespace.path = sandbox.shared_utsns.path.clone();
            continue;
        }
    }
    // update pid namespace
    let mut pid_ns = LinuxNamespace {
        r#type: NSTYPEPID.to_string(),
        ..Default::default()
    };

    // Use shared pid ns if useSandboxPidns has been set in either
    // the create_sandbox request or create_container request.
    // Else set this to empty string so that a new pid namespace is
    // created for the container.
    if sandbox_pidns {
        if let Some(ref pidns) = &sandbox.sandbox_pidns {
            pid_ns.path = String::from(pidns.path.as_str());
        } else {
            return Err(anyhow!(ERR_NO_SANDBOX_PIDNS));
        }
    }

    linux.namespaces.push(pid_ns);
    Ok(())
}

fn remove_container_resources(sandbox: &mut Sandbox, cid: &str) -> Result<()> {
    let mut cmounts: Vec<String> = vec![];

    // Find the sandbox storage used by this container
    let mounts = sandbox.container_mounts.get(cid);
    if let Some(mounts) = mounts {
        for m in mounts.iter() {
            if sandbox.storages.get(m).is_some() {
                cmounts.push(m.to_string());
            }
        }
    }

    // Unmount in reverse order so that if the i-th mount depends on the j-th mount (with j < i),
    // then the i-th mount is unmounted first. This way there won't be any references to the j-th
    // mount anymore when it's unmounted.
    for m in cmounts.iter().rev() {
        if let Err(err) = sandbox.unset_and_remove_sandbox_storage(m) {
            error!(
                sl!(),
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
        let mut hooks = oci.hooks.take().unwrap_or_default();
        hooks.prestart.append(&mut guest_hooks.prestart.clone());
        hooks.poststart.append(&mut guest_hooks.poststart.clone());
        hooks.poststop.append(&mut guest_hooks.poststop.clone());
        oci.hooks = Some(hooks);
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
        warn!(sl!(), "received invalid signum {}", signum);
        return false;
    } else {
        (signum - 1).into()
    };

    // Open the file in read-only mode (ignoring errors).
    let file = match File::open(proc_status_file) {
        Ok(f) => f,
        Err(_) => {
            warn!(sl!(), "failed to open file {}", proc_status_file);
            return false;
        }
    };

    let sig_mask: u64 = 1 << shift_count;
    let reader = BufReader::new(file);

    // read lines start with SigBlk/SigIgn/SigCgt and check any match the signal mask
    reader
        .lines()
        .flatten()
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
        fs::create_dir(path.clone()).or_else(|e| {
            if e.kind() != std::io::ErrorKind::AlreadyExists {
                return Err(e);
            }
            Ok(())
        })?;

        std::fs::set_permissions(path.clone(), std::fs::Permissions::from_mode(req.file_mode))?;

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
        // TO-DO: The old timestamped dir still exists due to missing delete api.
        // First Unlink existing symlink.
        if path.is_symlink() && path.exists() {
            unistd::unlink(&path)?;
        }

        let src = PathBuf::from(String::from_utf8(req.data.clone()).unwrap());
        unistd::symlinkat(&src, None, &path)?;
        let path_str = CString::new(path.to_str().unwrap())?;
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
    let spec_root = if let Some(sr) = &spec.root {
        sr
    } else {
        return Err(anyhow!(nix::Error::EINVAL));
    };

    let spec_root_path = Path::new(&spec_root.path);

    let bundle_path = Path::new(CONTAINER_BASE).join(cid);
    let config_path = bundle_path.join(CONFIG_JSON);
    let rootfs_path = bundle_path.join("rootfs");

    let rootfs_exists = Path::new(&rootfs_path).exists();
    info!(
        sl!(),
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
            &sl!(),
        )?;
    }

    let rootfs_path_name = rootfs_path
        .to_str()
        .ok_or_else(|| anyhow!("failed to convert rootfs to unicode"))?
        .to_string();

    spec.root = Some(Root {
        path: rootfs_path_name,
        readonly: spec_root.readonly,
    });

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
        sl!(),
        "load_kernel_module {}: {:?}", module.name, module.parameters
    );

    let mut args = vec!["-v".to_string(), module.name.clone()];

    if !module.parameters.is_empty() {
        args.extend(module.parameters.to_vec())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{namespace::Namespace, protocols::agent_ttrpc_async::AgentService as _};
    use nix::mount;
    use nix::sched::{unshare, CloneFlags};
    use oci::{Hook, Hooks, Linux, LinuxNamespace};
    use tempfile::{tempdir, TempDir};
    use test_utils::{assert_result, skip_if_not_root};
    use ttrpc::{r#async::TtrpcContext, MessageHeader};
    use which::which;

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
        let root = Root {
            path: String::from("/"),
            ..Default::default()
        };

        let spec = Spec {
            linux: Some(oci::Linux::default()),
            root: Some(root),
            ..Default::default()
        };

        CreateOpts {
            cgroup_name: "".to_string(),
            use_systemd_cgroup: false,
            no_pivot_root: false,
            no_new_keyring: false,
            spec: Some(spec),
            rootless_euid: false,
            rootless_cgroup: false,
        }
    }

    fn create_linuxcontainer() -> (LinuxContainer, TempDir) {
        let dir = tempdir().expect("failed to make tempdir");

        (
            LinuxContainer::new(
                "some_id",
                dir.path().join("rootfs").to_str().unwrap(),
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
        s.hooks = Some(Hooks {
            prestart: vec![Hook {
                path: "foo".to_string(),
                ..Default::default()
            }],
            ..Default::default()
        });
        let mut oci = Spec {
            ..Default::default()
        };
        append_guest_hooks(&s, &mut oci).unwrap();
        assert_eq!(s.hooks, oci.hooks);
    }

    #[tokio::test]
    async fn test_update_interface() {
        let logger = slog::Logger::root(slog::Discard, o!());
        let sandbox = Sandbox::new(&logger).unwrap();

        let agent_service = Box::new(AgentService {
            sandbox: Arc::new(Mutex::new(sandbox)),
            init_mode: true,
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
        });

        let req = protocols::agent::AddARPNeighborsRequest::default();
        let ctx = mk_ttrpc_context();

        let result = agent_service.add_arp_neighbors(&ctx, req).await;

        assert!(result.is_err(), "expected add arp neighbors to fail");
    }

    #[tokio::test]
    async fn test_do_write_stream() {
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
                        LinuxNamespace {
                            r#type: NSTYPEIPC.to_string(),
                            path: "ipcpath".to_string(),
                        },
                        LinuxNamespace {
                            r#type: NSTYPEUTS.to_string(),
                            path: "utspath".to_string(),
                        },
                    ],
                    use_sandbox_pidns: false,
                    result: Ok(()),
                    expected_namespaces: vec![
                        LinuxNamespace {
                            r#type: NSTYPEIPC.to_string(),
                            path: "".to_string(),
                        },
                        LinuxNamespace {
                            r#type: NSTYPEUTS.to_string(),
                            path: "".to_string(),
                        },
                        LinuxNamespace {
                            r#type: NSTYPEPID.to_string(),
                            path: "".to_string(),
                        },
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
                    LinuxNamespace {
                        r#type: NSTYPEIPC.to_string(),
                        path: "".to_string(),
                    },
                    LinuxNamespace {
                        r#type: NSTYPEUTS.to_string(),
                        path: "".to_string(),
                    },
                    LinuxNamespace {
                        r#type: NSTYPEPID.to_string(),
                        path: "sharedpidns".to_string(),
                    },
                ],
                ..Default::default()
            },
            TestData {
                namespaces: vec![],
                use_sandbox_pidns: true,
                expected_namespaces: vec![LinuxNamespace {
                    r#type: NSTYPEPID.to_string(),
                    path: "sharedpidns".to_string(),
                }],
                ..Default::default()
            },
            TestData {
                namespaces: vec![],
                use_sandbox_pidns: false,
                expected_namespaces: vec![LinuxNamespace {
                    r#type: NSTYPEPID.to_string(),
                    path: "".to_string(),
                }],
                ..Default::default()
            },
            TestData {
                namespaces: vec![],
                sandbox_pidns_path: None,
                use_sandbox_pidns: true,
                result: Err(anyhow!(ERR_NO_SANDBOX_PIDNS)),
                expected_namespaces: vec![],
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
            if d.has_linux_in_spec {
                oci.linux = Some(Linux {
                    namespaces: d.namespaces.clone(),
                    ..Default::default()
                });
            }

            let result = update_container_namespaces(&sandbox, &mut oci, d.use_sandbox_pidns);

            let msg = format!("{}, result: {:?}", msg, result);

            assert_result!(d.result, result, msg);
            if let Some(linux) = oci.linux {
                assert_eq!(d.expected_namespaces, linux.namespaces, "{}", msg);
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

        assert_eq!(stats.used, 4 * 1024);
        assert_eq!(stats.available, available - 4 * 1024);
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
                    sl!(),
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
    async fn test_merge_cwd() {
        #[derive(Debug)]
        struct TestData<'a> {
            container_process_cwd: &'a str,
            image_process_cwd: &'a str,
            expected: &'a str,
        }

        let tests = &[
            // Image cwd should override blank container cwd
            // TODO - how can we tell the user didn't specifically set it to `/` vs not setting at all? Is that scenario valid?
            TestData {
                container_process_cwd: "/",
                image_process_cwd: "/imageDir",
                expected: "/imageDir",
            },
            // Container cwd should override image cwd
            TestData {
                container_process_cwd: "/containerDir",
                image_process_cwd: "/imageDir",
                expected: "/containerDir",
            },
            // Container cwd should override blank image cwd
            TestData {
                container_process_cwd: "/containerDir",
                image_process_cwd: "/",
                expected: "/containerDir",
            },
        ];

        for (i, d) in tests.iter().enumerate() {
            let msg = format!("test[{}]: {:?}", i, d);

            let mut container_process = oci::Process {
                cwd: d.container_process_cwd.to_string(),
                ..Default::default()
            };

            let image_process = oci::Process {
                cwd: d.image_process_cwd.to_string(),
                ..Default::default()
            };

            merge_oci_process(&mut container_process, &image_process);

            assert_eq!(d.expected, container_process.cwd, "{}", msg);
        }
    }

    #[tokio::test]
    async fn test_merge_env() {
        #[derive(Debug)]
        struct TestData {
            container_process_env: Vec<String>,
            image_process_env: Vec<String>,
            expected: Vec<String>,
        }

        let tests = &[
            // Test that the pods environment overrides the images
            TestData {
                container_process_env: vec!["ISPRODUCTION=true".to_string()],
                image_process_env: vec!["ISPRODUCTION=false".to_string()],
                expected: vec!["ISPRODUCTION=true".to_string()],
            },
            // Test that multiple environment variables can be overrided
            TestData {
                container_process_env: vec![
                    "ISPRODUCTION=true".to_string(),
                    "ISDEVELOPMENT=false".to_string(),
                ],
                image_process_env: vec![
                    "ISPRODUCTION=false".to_string(),
                    "ISDEVELOPMENT=true".to_string(),
                ],
                expected: vec![
                    "ISPRODUCTION=true".to_string(),
                    "ISDEVELOPMENT=false".to_string(),
                ],
            },
            // Test that when none of the variables match do not override them
            TestData {
                container_process_env: vec!["ANOTHERENV=TEST".to_string()],
                image_process_env: vec![
                    "ISPRODUCTION=false".to_string(),
                    "ISDEVELOPMENT=true".to_string(),
                ],
                expected: vec![
                    "ANOTHERENV=TEST".to_string(),
                    "ISPRODUCTION=false".to_string(),
                    "ISDEVELOPMENT=true".to_string(),
                ],
            },
            // Test a mix of both overriding and not
            TestData {
                container_process_env: vec![
                    "ANOTHERENV=TEST".to_string(),
                    "ISPRODUCTION=true".to_string(),
                ],
                image_process_env: vec![
                    "ISPRODUCTION=false".to_string(),
                    "ISDEVELOPMENT=true".to_string(),
                ],
                expected: vec![
                    "ANOTHERENV=TEST".to_string(),
                    "ISPRODUCTION=true".to_string(),
                    "ISDEVELOPMENT=true".to_string(),
                ],
            },
        ];

        for (i, d) in tests.iter().enumerate() {
            let msg = format!("test[{}]: {:?}", i, d);

            let mut container_process = oci::Process {
                env: d.container_process_env.clone(),
                ..Default::default()
            };

            let image_process = oci::Process {
                env: d.image_process_env.clone(),
                ..Default::default()
            };

            merge_oci_process(&mut container_process, &image_process);

            assert_eq!(d.expected, container_process.env, "{}", msg);
        }
    }
}
