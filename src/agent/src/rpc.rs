// Copyright (c) 2019 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

use async_trait::async_trait;
use rustjail::{pipestream::PipeStream, process::StreamType};
use tokio::io::{AsyncReadExt, AsyncWriteExt, ReadHalf};
use tokio::sync::Mutex;

use std::path::Path;
use std::sync::Arc;
use ttrpc::{
    self,
    error::get_rpc_status as ttrpc_error,
    r#async::{Server as TtrpcServer, TtrpcContext},
};

use anyhow::{anyhow, Context, Result};
use oci::{LinuxNamespace, Root, Spec};
use protobuf::{RepeatedField, SingularPtrField};
use protocols::agent::{
    AgentDetails, CopyFileRequest, GuestDetailsResponse, Interfaces, Metrics, OOMEvent,
    ReadStreamResponse, Routes, StatsContainerResponse, WaitProcessResponse, WriteStreamResponse,
};
use protocols::empty::Empty;
use protocols::health::{
    HealthCheckResponse, HealthCheckResponse_ServingStatus, VersionCheckResponse,
};
use protocols::types::Interface;
use rustjail::cgroups::notifier;
use rustjail::container::{BaseContainer, Container, LinuxContainer};
use rustjail::process::Process;
use rustjail::specconv::CreateOpts;

use nix::errno::Errno;
use nix::mount::MsFlags;
use nix::sys::signal::Signal;
use nix::sys::stat;
use nix::unistd::{self, Pid};
use rustjail::process::ProcessOperations;

use crate::device::{add_devices, rescan_pci_bus, update_device_cgroup};
use crate::linux_abi::*;
use crate::metrics::get_metrics;
use crate::mount::{add_storages, remove_mounts, BareMount, STORAGE_HANDLER_LIST};
use crate::namespace::{NSTYPEIPC, NSTYPEPID, NSTYPEUTS};
use crate::network::setup_guest_dns;
use crate::random;
use crate::sandbox::Sandbox;
use crate::version::{AGENT_VERSION, API_VERSION};
use crate::AGENT_CONFIG;

use libc::{self, c_ushort, pid_t, winsize, TIOCSWINSZ};
use std::convert::TryFrom;
use std::fs;
use std::os::unix::prelude::PermissionsExt;
use std::process::{Command, Stdio};
use std::time::Duration;

use nix::unistd::{Gid, Uid};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader};
use std::os::unix::fs::FileExt;
use std::path::PathBuf;

const CONTAINER_BASE: &str = "/run/kata-containers";
const MODPROBE_PATH: &str = "/sbin/modprobe";

// Convenience macro to obtain the scope logger
macro_rules! sl {
    () => {
        slog_scope::logger()
    };
}

#[derive(Clone)]
pub struct AgentService {
    sandbox: Arc<Mutex<Sandbox>>,
}

// A container ID must match this regex:
//
//     ^[a-zA-Z0-9][a-zA-Z0-9_.-]+$
//
fn verify_cid(id: &str) -> Result<()> {
    let valid = id.len() > 1
        && id.chars().next().unwrap().is_alphanumeric()
        && id
            .chars()
            .all(|c| (c.is_alphanumeric() || ['.', '-', '_'].contains(&c)));

    match valid {
        true => Ok(()),
        false => Err(anyhow!("invalid container ID: {:?}", id)),
    }
}

impl AgentService {
    async fn do_create_container(
        &self,
        req: protocols::agent::CreateContainerRequest,
    ) -> Result<()> {
        let cid = req.container_id.clone();

        verify_cid(&cid)?;

        let mut oci_spec = req.OCI.clone();
        let use_sandbox_pidns = req.get_sandbox_pidns();

        let sandbox;
        let mut s;

        let mut oci = match oci_spec.as_mut() {
            Some(spec) => rustjail::grpc_to_oci(spec),
            None => {
                error!(sl!(), "no oci spec in the create container request!");
                return Err(anyhow!(nix::Error::from_errno(nix::errno::Errno::EINVAL)));
            }
        };

        info!(sl!(), "receive createcontainer, spec: {:?}", &oci);

        // re-scan PCI bus
        // looking for hidden devices
        rescan_pci_bus().context("Could not rescan PCI bus")?;

        // Some devices need some extra processing (the ones invoked with
        // --device for instance), and that's what this call is doing. It
        // updates the devices listed in the OCI spec, so that they actually
        // match real devices inside the VM. This step is necessary since we
        // cannot predict everything from the caller.
        add_devices(&req.devices.to_vec(), &mut oci, &self.sandbox).await?;

        // Both rootfs and volumes (invoked with --volume for instance) will
        // be processed the same way. The idea is to always mount any provided
        // storage to the specified MountPoint, so that it will match what's
        // inside oci.Mounts.
        // After all those storages have been processed, no matter the order
        // here, the agent will rely on rustjail (using the oci.Mounts
        // list) to bind mount all of them inside the container.
        let m = add_storages(sl!(), req.storages.to_vec(), self.sandbox.clone()).await?;
        {
            sandbox = self.sandbox.clone();
            s = sandbox.lock().await;
            s.container_mounts.insert(cid.clone(), m);
        }

        update_container_namespaces(&s, &mut oci, use_sandbox_pidns)?;

        // Add the root partition to the device cgroup to prevent access
        update_device_cgroup(&mut oci)?;

        // Append guest hooks
        append_guest_hooks(&s, &mut oci);

        // write spec to bundle path, hooks might
        // read ocispec
        let olddir = setup_bundle(&cid, &mut oci)?;
        // restore the cwd for kata-agent process.
        defer!(unistd::chdir(&olddir).unwrap());

        let opts = CreateOpts {
            cgroup_name: "".to_string(),
            use_systemd_cgroup: false,
            no_pivot_root: s.no_pivot_root,
            no_new_keyring: false,
            spec: Some(oci.clone()),
            rootless_euid: false,
            rootless_cgroup: false,
        };

        let mut ctr: LinuxContainer =
            LinuxContainer::new(cid.as_str(), CONTAINER_BASE, opts, &sl!())?;

        let pipe_size = AGENT_CONFIG.read().await.container_pipe_size;
        let p = if oci.process.is_some() {
            Process::new(
                &sl!(),
                &oci.process.as_ref().unwrap(),
                cid.as_str(),
                true,
                pipe_size,
            )?
        } else {
            info!(sl!(), "no process configurations!");
            return Err(anyhow!(nix::Error::from_errno(nix::errno::Errno::EINVAL)));
        };

        ctr.start(p).await?;

        s.update_shared_pidns(&ctr)?;
        s.add_container(ctr);
        info!(sl!(), "created container!");

        Ok(())
    }

    async fn do_start_container(&self, req: protocols::agent::StartContainerRequest) -> Result<()> {
        let cid = req.container_id;

        let sandbox = self.sandbox.clone();
        let mut s = sandbox.lock().await;
        let sid = s.id.clone();

        let ctr = s
            .get_container(&cid)
            .ok_or_else(|| anyhow!("Invalid container id"))?;

        ctr.exec()?;

        // start oom event loop
        if sid != cid && ctr.cgroup_manager.is_some() {
            let cg_path = ctr.cgroup_manager.as_ref().unwrap().get_cg_path("memory");
            if cg_path.is_some() {
                let rx = notifier::notify_oom(cid.as_str(), cg_path.unwrap()).await?;
                s.run_oom_event_monitor(rx, cid.clone()).await;
            }
        }

        Ok(())
    }

    async fn do_remove_container(
        &self,
        req: protocols::agent::RemoveContainerRequest,
    ) -> Result<()> {
        let cid = req.container_id.clone();
        let mut cmounts: Vec<String> = vec![];

        let mut remove_container_resources = |sandbox: &mut Sandbox| -> Result<()> {
            // Find the sandbox storage used by this container
            let mounts = sandbox.container_mounts.get(&cid);
            if let Some(mounts) = mounts {
                remove_mounts(&mounts)?;

                for m in mounts.iter() {
                    if sandbox.storages.get(m).is_some() {
                        cmounts.push(m.to_string());
                    }
                }
            }

            for m in cmounts.iter() {
                sandbox.unset_and_remove_sandbox_storage(m)?;
            }

            sandbox.container_mounts.remove(cid.as_str());
            sandbox.containers.remove(cid.as_str());
            Ok(())
        };

        if req.timeout == 0 {
            let s = Arc::clone(&self.sandbox);
            let mut sandbox = s.lock().await;
            let ctr = sandbox
                .get_container(&cid)
                .ok_or_else(|| anyhow!("Invalid container id"))?;

            ctr.destroy().await?;

            remove_container_resources(&mut sandbox)?;

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
                tx.send(1).unwrap();
            };
        });

        if tokio::time::timeout(Duration::from_secs(req.timeout.into()), rx)
            .await
            .is_err()
        {
            return Err(anyhow!(nix::Error::from_errno(nix::errno::Errno::ETIME)));
        }

        if handle.await.is_err() {
            return Err(anyhow!(nix::Error::from_errno(
                nix::errno::Errno::UnknownErrno
            )));
        }

        let s = self.sandbox.clone();
        let mut sandbox = s.lock().await;

        remove_container_resources(&mut sandbox)?;

        Ok(())
    }

    async fn do_exec_process(&self, req: protocols::agent::ExecProcessRequest) -> Result<()> {
        let cid = req.container_id.clone();
        let exec_id = req.exec_id.clone();

        info!(sl!(), "do_exec_process cid: {} eid: {}", cid, exec_id);

        let s = self.sandbox.clone();
        let mut sandbox = s.lock().await;

        let process = if req.process.is_some() {
            req.process.as_ref().unwrap()
        } else {
            return Err(anyhow!(nix::Error::from_errno(nix::errno::Errno::EINVAL)));
        };

        let pipe_size = AGENT_CONFIG.read().await.container_pipe_size;
        let ocip = rustjail::process_grpc_to_oci(process);
        let p = Process::new(&sl!(), &ocip, exec_id.as_str(), false, pipe_size)?;

        let ctr = sandbox
            .get_container(&cid)
            .ok_or_else(|| anyhow!("Invalid container id"))?;

        ctr.run(p).await?;

        Ok(())
    }

    async fn do_signal_process(&self, req: protocols::agent::SignalProcessRequest) -> Result<()> {
        let cid = req.container_id.clone();
        let eid = req.exec_id.clone();
        let s = self.sandbox.clone();
        let mut sandbox = s.lock().await;
        let mut init = false;

        info!(
            sl!(),
            "signal process";
            "container-id" => cid.clone(),
            "exec-id" => eid.clone(),
        );

        if eid.is_empty() {
            init = true;
        }

        let p = find_process(&mut sandbox, cid.as_str(), eid.as_str(), init)?;

        let mut signal = Signal::try_from(req.signal as i32).unwrap();

        // For container initProcess, if it hasn't installed handler for "SIGTERM" signal,
        // it will ignore the "SIGTERM" signal sent to it, thus send it "SIGKILL" signal
        // instead of "SIGTERM" to terminate it.
        if p.init && signal == Signal::SIGTERM && !is_signal_handled(p.pid, req.signal) {
            signal = Signal::SIGKILL;
        }

        p.signal(signal)?;

        Ok(())
    }

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
            let p = find_process(&mut sandbox, cid.as_str(), eid.as_str(), false)?;

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

        let mut p = match ctr.processes.get_mut(&pid) {
            Some(p) => p,
            None => {
                // Lost race, pick up exit code from channel
                resp.status = exit_recv.recv().await.unwrap();
                return Ok(resp);
            }
        };

        // need to close all fd
        // ignore errors for some fd might be closed by stream
        let _ = cleanup_process(&mut p);

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
            let p = find_process(&mut sandbox, cid.as_str(), eid.as_str(), false)?;

            // use ptmx io
            if p.term_master.is_some() {
                p.get_writer(StreamType::TermMaster)
            } else {
                // use piped io
                p.get_writer(StreamType::ParentStdin)
            }
        };

        let writer = writer.unwrap();
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

            let p = find_process(&mut sandbox, cid.as_str(), eid.as_str(), false)?;

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
            return Err(anyhow!(nix::Error::from_errno(nix::errno::Errno::EINVAL)));
        }

        let reader = reader.unwrap();

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
}

#[async_trait]
impl protocols::agent_ttrpc::AgentService for AgentService {
    async fn create_container(
        &self,
        _ctx: &TtrpcContext,
        req: protocols::agent::CreateContainerRequest,
    ) -> ttrpc::Result<Empty> {
        match self.do_create_container(req).await {
            Err(e) => Err(ttrpc_error(ttrpc::Code::INTERNAL, e.to_string())),
            Ok(_) => Ok(Empty::new()),
        }
    }

    async fn start_container(
        &self,
        _ctx: &TtrpcContext,
        req: protocols::agent::StartContainerRequest,
    ) -> ttrpc::Result<Empty> {
        match self.do_start_container(req).await {
            Err(e) => Err(ttrpc_error(ttrpc::Code::INTERNAL, e.to_string())),
            Ok(_) => Ok(Empty::new()),
        }
    }

    async fn remove_container(
        &self,
        _ctx: &TtrpcContext,
        req: protocols::agent::RemoveContainerRequest,
    ) -> ttrpc::Result<Empty> {
        match self.do_remove_container(req).await {
            Err(e) => Err(ttrpc_error(ttrpc::Code::INTERNAL, e.to_string())),
            Ok(_) => Ok(Empty::new()),
        }
    }

    async fn exec_process(
        &self,
        _ctx: &TtrpcContext,
        req: protocols::agent::ExecProcessRequest,
    ) -> ttrpc::Result<Empty> {
        match self.do_exec_process(req).await {
            Err(e) => Err(ttrpc_error(ttrpc::Code::INTERNAL, e.to_string())),
            Ok(_) => Ok(Empty::new()),
        }
    }

    async fn signal_process(
        &self,
        _ctx: &TtrpcContext,
        req: protocols::agent::SignalProcessRequest,
    ) -> ttrpc::Result<Empty> {
        match self.do_signal_process(req).await {
            Err(e) => Err(ttrpc_error(ttrpc::Code::INTERNAL, e.to_string())),
            Ok(_) => Ok(Empty::new()),
        }
    }

    async fn wait_process(
        &self,
        _ctx: &TtrpcContext,
        req: protocols::agent::WaitProcessRequest,
    ) -> ttrpc::Result<WaitProcessResponse> {
        self.do_wait_process(req)
            .await
            .map_err(|e| ttrpc_error(ttrpc::Code::INTERNAL, e.to_string()))
    }

    async fn update_container(
        &self,
        _ctx: &TtrpcContext,
        req: protocols::agent::UpdateContainerRequest,
    ) -> ttrpc::Result<Empty> {
        let cid = req.container_id.clone();
        let res = req.resources;

        let s = Arc::clone(&self.sandbox);
        let mut sandbox = s.lock().await;

        let ctr = sandbox.get_container(&cid).ok_or_else(|| {
            ttrpc_error(
                ttrpc::Code::INVALID_ARGUMENT,
                "invalid container id".to_string(),
            )
        })?;

        let resp = Empty::new();

        if res.is_some() {
            let oci_res = rustjail::resources_grpc_to_oci(&res.unwrap());
            match ctr.set(oci_res) {
                Err(e) => {
                    return Err(ttrpc_error(ttrpc::Code::INTERNAL, e.to_string()));
                }

                Ok(_) => return Ok(resp),
            }
        }

        Ok(resp)
    }

    async fn stats_container(
        &self,
        _ctx: &TtrpcContext,
        req: protocols::agent::StatsContainerRequest,
    ) -> ttrpc::Result<StatsContainerResponse> {
        let cid = req.container_id;
        let s = Arc::clone(&self.sandbox);
        let mut sandbox = s.lock().await;

        let ctr = sandbox.get_container(&cid).ok_or_else(|| {
            ttrpc_error(
                ttrpc::Code::INVALID_ARGUMENT,
                "invalid container id".to_string(),
            )
        })?;

        ctr.stats()
            .map_err(|e| ttrpc_error(ttrpc::Code::INTERNAL, e.to_string()))
    }

    async fn pause_container(
        &self,
        _ctx: &TtrpcContext,
        req: protocols::agent::PauseContainerRequest,
    ) -> ttrpc::Result<protocols::empty::Empty> {
        let cid = req.get_container_id();
        let s = Arc::clone(&self.sandbox);
        let mut sandbox = s.lock().await;

        let ctr = sandbox.get_container(&cid).ok_or_else(|| {
            ttrpc_error(
                ttrpc::Code::INVALID_ARGUMENT,
                "invalid container id".to_string(),
            )
        })?;

        ctr.pause()
            .map_err(|e| ttrpc_error(ttrpc::Code::INTERNAL, e.to_string()))?;

        Ok(Empty::new())
    }

    async fn resume_container(
        &self,
        _ctx: &TtrpcContext,
        req: protocols::agent::ResumeContainerRequest,
    ) -> ttrpc::Result<protocols::empty::Empty> {
        let cid = req.get_container_id();
        let s = Arc::clone(&self.sandbox);
        let mut sandbox = s.lock().await;

        let ctr = sandbox.get_container(&cid).ok_or_else(|| {
            ttrpc_error(
                ttrpc::Code::INVALID_ARGUMENT,
                "invalid container id".to_string(),
            )
        })?;

        ctr.resume()
            .map_err(|e| ttrpc_error(ttrpc::Code::INTERNAL, e.to_string()))?;

        Ok(Empty::new())
    }

    async fn write_stdin(
        &self,
        _ctx: &TtrpcContext,
        req: protocols::agent::WriteStreamRequest,
    ) -> ttrpc::Result<WriteStreamResponse> {
        self.do_write_stream(req)
            .await
            .map_err(|e| ttrpc_error(ttrpc::Code::INTERNAL, e.to_string()))
    }

    async fn read_stdout(
        &self,
        _ctx: &TtrpcContext,
        req: protocols::agent::ReadStreamRequest,
    ) -> ttrpc::Result<ReadStreamResponse> {
        self.do_read_stream(req, true)
            .await
            .map_err(|e| ttrpc_error(ttrpc::Code::INTERNAL, e.to_string()))
    }

    async fn read_stderr(
        &self,
        _ctx: &TtrpcContext,
        req: protocols::agent::ReadStreamRequest,
    ) -> ttrpc::Result<ReadStreamResponse> {
        self.do_read_stream(req, false)
            .await
            .map_err(|e| ttrpc_error(ttrpc::Code::INTERNAL, e.to_string()))
    }

    async fn close_stdin(
        &self,
        _ctx: &TtrpcContext,
        req: protocols::agent::CloseStdinRequest,
    ) -> ttrpc::Result<Empty> {
        let cid = req.container_id.clone();
        let eid = req.exec_id;
        let s = Arc::clone(&self.sandbox);
        let mut sandbox = s.lock().await;

        let p = find_process(&mut sandbox, cid.as_str(), eid.as_str(), false).map_err(|e| {
            ttrpc_error(
                ttrpc::Code::INVALID_ARGUMENT,
                format!("invalid argument: {:?}", e),
            )
        })?;

        if p.term_master.is_some() {
            p.close_stream(StreamType::TermMaster);
            let _ = unistd::close(p.term_master.unwrap());
            p.term_master = None;
        }

        if p.parent_stdin.is_some() {
            p.close_stream(StreamType::ParentStdin);
            let _ = unistd::close(p.parent_stdin.unwrap());
            p.parent_stdin = None;
        }

        p.notify_term_close();

        Ok(Empty::new())
    }

    async fn tty_win_resize(
        &self,
        _ctx: &TtrpcContext,
        req: protocols::agent::TtyWinResizeRequest,
    ) -> ttrpc::Result<Empty> {
        let cid = req.container_id.clone();
        let eid = req.exec_id.clone();
        let s = Arc::clone(&self.sandbox);
        let mut sandbox = s.lock().await;
        let p = find_process(&mut sandbox, cid.as_str(), eid.as_str(), false).map_err(|e| {
            ttrpc_error(
                ttrpc::Code::UNAVAILABLE,
                format!("invalid argument: {:?}", e),
            )
        })?;

        if p.term_master.is_none() {
            return Err(ttrpc_error(ttrpc::Code::UNAVAILABLE, "no tty".to_string()));
        }

        let fd = p.term_master.unwrap();
        unsafe {
            let win = winsize {
                ws_row: req.row as c_ushort,
                ws_col: req.column as c_ushort,
                ws_xpixel: 0,
                ws_ypixel: 0,
            };

            let err = libc::ioctl(fd, TIOCSWINSZ, &win);
            Errno::result(err)
                .map(drop)
                .map_err(|e| ttrpc_error(ttrpc::Code::INTERNAL, format!("ioctl error: {:?}", e)))?;
        }

        Ok(Empty::new())
    }

    async fn update_interface(
        &self,
        _ctx: &TtrpcContext,
        req: protocols::agent::UpdateInterfaceRequest,
    ) -> ttrpc::Result<Interface> {
        let interface = req.interface.into_option().ok_or_else(|| {
            ttrpc_error(
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
                ttrpc_error(ttrpc::Code::INTERNAL, format!("update interface: {:?}", e))
            })?;

        Ok(interface)
    }

    async fn update_routes(
        &self,
        _ctx: &TtrpcContext,
        req: protocols::agent::UpdateRoutesRequest,
    ) -> ttrpc::Result<Routes> {
        let new_routes = req
            .routes
            .into_option()
            .map(|r| r.Routes.into_vec())
            .ok_or_else(|| {
                ttrpc_error(
                    ttrpc::Code::INVALID_ARGUMENT,
                    "empty update routes request".to_string(),
                )
            })?;

        let mut sandbox = self.sandbox.lock().await;

        sandbox.rtnl.update_routes(new_routes).await.map_err(|e| {
            ttrpc_error(
                ttrpc::Code::INTERNAL,
                format!("Failed to update routes: {:?}", e),
            )
        })?;

        let list = sandbox.rtnl.list_routes().await.map_err(|e| {
            ttrpc_error(
                ttrpc::Code::INTERNAL,
                format!("Failed to list routes after update: {:?}", e),
            )
        })?;

        Ok(protocols::agent::Routes {
            Routes: RepeatedField::from_vec(list),
            ..Default::default()
        })
    }

    async fn list_interfaces(
        &self,
        _ctx: &TtrpcContext,
        _req: protocols::agent::ListInterfacesRequest,
    ) -> ttrpc::Result<Interfaces> {
        let list = self
            .sandbox
            .lock()
            .await
            .rtnl
            .list_interfaces()
            .await
            .map_err(|e| {
                ttrpc_error(
                    ttrpc::Code::INTERNAL,
                    format!("Failed to list interfaces: {:?}", e),
                )
            })?;

        Ok(protocols::agent::Interfaces {
            Interfaces: RepeatedField::from_vec(list),
            ..Default::default()
        })
    }

    async fn list_routes(
        &self,
        _ctx: &TtrpcContext,
        _req: protocols::agent::ListRoutesRequest,
    ) -> ttrpc::Result<Routes> {
        let list = self
            .sandbox
            .lock()
            .await
            .rtnl
            .list_routes()
            .await
            .map_err(|e| ttrpc_error(ttrpc::Code::INTERNAL, format!("list routes: {:?}", e)))?;

        Ok(protocols::agent::Routes {
            Routes: RepeatedField::from_vec(list),
            ..Default::default()
        })
    }

    async fn start_tracing(
        &self,
        _ctx: &TtrpcContext,
        req: protocols::agent::StartTracingRequest,
    ) -> ttrpc::Result<Empty> {
        info!(sl!(), "start_tracing {:?}", req);
        Ok(Empty::new())
    }

    async fn stop_tracing(
        &self,
        _ctx: &TtrpcContext,
        _req: protocols::agent::StopTracingRequest,
    ) -> ttrpc::Result<Empty> {
        Ok(Empty::new())
    }

    async fn create_sandbox(
        &self,
        _ctx: &TtrpcContext,
        req: protocols::agent::CreateSandboxRequest,
    ) -> ttrpc::Result<Empty> {
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
                load_kernel_module(m)
                    .map_err(|e| ttrpc_error(ttrpc::Code::INTERNAL, e.to_string()))?;
            }

            s.setup_shared_namespaces()
                .await
                .map_err(|e| ttrpc_error(ttrpc::Code::INTERNAL, e.to_string()))?;
        }

        match add_storages(sl!(), req.storages.to_vec(), self.sandbox.clone()).await {
            Ok(m) => {
                let sandbox = self.sandbox.clone();
                let mut s = sandbox.lock().await;
                s.mounts = m
            }
            Err(e) => return Err(ttrpc_error(ttrpc::Code::INTERNAL, e.to_string())),
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
            Err(e) => return Err(ttrpc_error(ttrpc::Code::INTERNAL, e.to_string())),
        };

        Ok(Empty::new())
    }

    async fn destroy_sandbox(
        &self,
        _ctx: &TtrpcContext,
        _req: protocols::agent::DestroySandboxRequest,
    ) -> ttrpc::Result<Empty> {
        let s = Arc::clone(&self.sandbox);
        let mut sandbox = s.lock().await;
        // destroy all containers, clean up, notify agent to exit
        // etc.
        sandbox.destroy().await.unwrap();
        // Close get_oom_event connection,
        // otherwise it will block the shutdown of ttrpc.
        sandbox.event_tx.take();

        sandbox.sender.take().unwrap().send(1).unwrap();

        Ok(Empty::new())
    }

    async fn add_arp_neighbors(
        &self,
        _ctx: &TtrpcContext,
        req: protocols::agent::AddARPNeighborsRequest,
    ) -> ttrpc::Result<Empty> {
        let neighs = req
            .neighbors
            .into_option()
            .map(|n| n.ARPNeighbors.into_vec())
            .ok_or_else(|| {
                ttrpc_error(
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
                ttrpc_error(
                    ttrpc::Code::INTERNAL,
                    format!("Failed to add ARP neighbours: {:?}", e),
                )
            })?;

        Ok(Empty::new())
    }

    async fn online_cpu_mem(
        &self,
        _ctx: &TtrpcContext,
        req: protocols::agent::OnlineCPUMemRequest,
    ) -> ttrpc::Result<Empty> {
        let s = Arc::clone(&self.sandbox);
        let sandbox = s.lock().await;

        sandbox
            .online_cpu_memory(&req)
            .map_err(|e| ttrpc_error(ttrpc::Code::INTERNAL, e.to_string()))?;

        Ok(Empty::new())
    }

    async fn reseed_random_dev(
        &self,
        _ctx: &TtrpcContext,
        req: protocols::agent::ReseedRandomDevRequest,
    ) -> ttrpc::Result<Empty> {
        random::reseed_rng(req.data.as_slice())
            .map_err(|e| ttrpc_error(ttrpc::Code::INTERNAL, e.to_string()))?;

        Ok(Empty::new())
    }

    async fn get_guest_details(
        &self,
        _ctx: &TtrpcContext,
        req: protocols::agent::GuestDetailsRequest,
    ) -> ttrpc::Result<GuestDetailsResponse> {
        info!(sl!(), "get guest details!");
        let mut resp = GuestDetailsResponse::new();
        // to get memory block size
        match get_memory_info(req.mem_block_size, req.mem_hotplug_probe) {
            Ok((u, v)) => {
                resp.mem_block_size_bytes = u;
                resp.support_mem_hotplug_probe = v;
            }
            Err(e) => {
                info!(sl!(), "fail to get memory info!");
                return Err(ttrpc_error(ttrpc::Code::INTERNAL, e.to_string()));
            }
        }

        // to get agent details
        let detail = get_agent_details();
        resp.agent_details = SingularPtrField::some(detail);

        Ok(resp)
    }

    async fn mem_hotplug_by_probe(
        &self,
        _ctx: &TtrpcContext,
        req: protocols::agent::MemHotplugByProbeRequest,
    ) -> ttrpc::Result<Empty> {
        do_mem_hotplug_by_probe(&req.memHotplugProbeAddr)
            .map_err(|e| ttrpc_error(ttrpc::Code::INTERNAL, e.to_string()))?;

        Ok(Empty::new())
    }

    async fn set_guest_date_time(
        &self,
        _ctx: &TtrpcContext,
        req: protocols::agent::SetGuestDateTimeRequest,
    ) -> ttrpc::Result<Empty> {
        do_set_guest_date_time(req.Sec, req.Usec)
            .map_err(|e| ttrpc_error(ttrpc::Code::INTERNAL, e.to_string()))?;

        Ok(Empty::new())
    }

    async fn copy_file(
        &self,
        _ctx: &TtrpcContext,
        req: protocols::agent::CopyFileRequest,
    ) -> ttrpc::Result<Empty> {
        do_copy_file(&req).map_err(|e| ttrpc_error(ttrpc::Code::INTERNAL, e.to_string()))?;

        Ok(Empty::new())
    }

    async fn get_metrics(
        &self,
        _ctx: &TtrpcContext,
        req: protocols::agent::GetMetricsRequest,
    ) -> ttrpc::Result<Metrics> {
        match get_metrics(&req) {
            Err(e) => Err(ttrpc_error(ttrpc::Code::INTERNAL, e.to_string())),
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
        _req: protocols::agent::GetOOMEventRequest,
    ) -> ttrpc::Result<OOMEvent> {
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

        Err(ttrpc_error(ttrpc::Code::INTERNAL, ""))
    }
}

#[derive(Clone)]
struct HealthService;

#[async_trait]
impl protocols::health_ttrpc::Health for HealthService {
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

fn get_memory_info(block_size: bool, hotplug: bool) -> Result<(u64, bool)> {
    let mut size: u64 = 0;
    let mut plug: bool = false;
    if block_size {
        match fs::read_to_string(SYSFS_MEMORY_BLOCK_SIZE_PATH) {
            Ok(v) => {
                if v.is_empty() {
                    info!(sl!(), "string in empty???");
                    return Err(anyhow!("Invalid block size"));
                }

                size = u64::from_str_radix(v.trim(), 16).map_err(|_| {
                    warn!(sl!(), "failed to parse the str {} to hex", size);
                    anyhow!("Invalid block size")
                })?;
            }
            Err(e) => {
                info!(sl!(), "memory block size error: {:?}", e.kind());
                if e.kind() != std::io::ErrorKind::NotFound {
                    return Err(anyhow!(e));
                }
            }
        }
    }

    if hotplug {
        match stat::stat(SYSFS_MEMORY_HOTPLUG_PROBE_PATH) {
            Ok(_) => plug = true,
            Err(e) => {
                info!(
                    sl!(),
                    "hotplug memory error: {}",
                    e.as_errno().unwrap().desc()
                );
                match e {
                    nix::Error::Sys(errno) => match errno {
                        Errno::ENOENT => plug = false,
                        _ => return Err(anyhow!(e)),
                    },
                    _ => return Err(anyhow!(e)),
                }
            }
        }
    }

    Ok((size, plug))
}

fn get_agent_details() -> AgentDetails {
    let mut detail = AgentDetails::new();

    detail.set_version(AGENT_VERSION.to_string());
    detail.set_supports_seccomp(false);
    detail.init_daemon = unistd::getpid() == Pid::from_raw(1);

    detail.device_handlers = RepeatedField::new();
    detail.storage_handlers = RepeatedField::from_vec(
        STORAGE_HANDLER_LIST
            .to_vec()
            .iter()
            .map(|x| x.to_string())
            .collect(),
    );

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

fn find_process<'a>(
    sandbox: &'a mut Sandbox,
    cid: &'a str,
    eid: &'a str,
    init: bool,
) -> Result<&'a mut Process> {
    let ctr = sandbox
        .get_container(cid)
        .ok_or_else(|| anyhow!("Invalid container id"))?;

    if init || eid.is_empty() {
        return ctr
            .processes
            .get_mut(&ctr.init_process_pid)
            .ok_or_else(|| anyhow!("cannot find init process!"));
    }

    ctr.get_process(eid).map_err(|_| anyhow!("Invalid exec id"))
}

pub fn start(s: Arc<Mutex<Sandbox>>, server_address: &str) -> TtrpcServer {
    let agent_service = Box::new(AgentService { sandbox: s })
        as Box<dyn protocols::agent_ttrpc::AgentService + Send + Sync>;

    let agent_worker = Arc::new(agent_service);

    let health_service =
        Box::new(HealthService {}) as Box<dyn protocols::health_ttrpc::Health + Send + Sync>;
    let health_worker = Arc::new(health_service);

    let aservice = protocols::agent_ttrpc::create_agent_service(agent_worker);

    let hservice = protocols::health_ttrpc::create_health(health_worker);

    let server = TtrpcServer::new()
        .bind(server_address)
        .unwrap()
        .register_service(aservice)
        .register_service(hservice);

    info!(sl!(), "ttRPC server started"; "address" => server_address);

    server
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
        .ok_or_else(|| anyhow!("Spec didn't container linux field"))?;

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
    if sandbox_pidns && sandbox.sandbox_pidns.is_some() {
        pid_ns.path = String::from(sandbox.sandbox_pidns.as_ref().unwrap().path.as_str());
    }

    linux.namespaces.push(pid_ns);
    Ok(())
}

fn append_guest_hooks(s: &Sandbox, oci: &mut Spec) {
    if s.hooks.is_none() {
        return;
    }
    let guest_hooks = s.hooks.as_ref().unwrap();
    let mut hooks = oci.hooks.take().unwrap_or_default();
    hooks.prestart.append(&mut guest_hooks.prestart.clone());
    hooks.poststart.append(&mut guest_hooks.poststart.clone());
    hooks.poststop.append(&mut guest_hooks.poststop.clone());
    oci.hooks = Some(hooks);
}

// Check is the container process installed the
// handler for specific signal.
fn is_signal_handled(pid: pid_t, signum: u32) -> bool {
    let sig_mask: u64 = 1u64 << (signum - 1);
    let file_name = format!("/proc/{}/status", pid);

    // Open the file in read-only mode (ignoring errors).
    let file = match File::open(&file_name) {
        Ok(f) => f,
        Err(_) => {
            warn!(sl!(), "failed to open file {}\n", file_name);
            return false;
        }
    };

    let reader = BufReader::new(file);

    // Read the file line by line using the lines() iterator from std::io::BufRead.
    for (_index, line) in reader.lines().enumerate() {
        let line = match line {
            Ok(l) => l,
            Err(_) => {
                warn!(sl!(), "failed to read file {}\n", file_name);
                return false;
            }
        };
        if line.starts_with("SigCgt:") {
            let mask_vec: Vec<&str> = line.split(':').collect();
            if mask_vec.len() != 2 {
                warn!(sl!(), "parse the SigCgt field failed\n");
                return false;
            }
            let sig_cgt_str = mask_vec[1];
            let sig_cgt_mask = match u64::from_str_radix(sig_cgt_str, 16) {
                Ok(h) => h,
                Err(_) => {
                    warn!(sl!(), "failed to parse the str {} to hex\n", sig_cgt_str);
                    return false;
                }
            };

            return (sig_cgt_mask & sig_mask) == sig_mask;
        }
    }
    false
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
        return Err(nix::Error::Sys(Errno::EINVAL).into());
    }

    let parent = path.parent();

    let dir = if let Some(parent) = parent {
        parent.to_path_buf()
    } else {
        PathBuf::from("/")
    };

    fs::create_dir_all(dir.to_str().unwrap()).or_else(|e| {
        if e.kind() != std::io::ErrorKind::AlreadyExists {
            return Err(e);
        }

        Ok(())
    })?;

    std::fs::set_permissions(
        dir.to_str().unwrap(),
        std::fs::Permissions::from_mode(req.dir_mode),
    )?;

    let mut tmpfile = path.clone();
    tmpfile.set_extension("tmp");

    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .open(tmpfile.to_str().unwrap())?;

    file.write_all_at(req.data.as_slice(), req.offset as u64)?;
    let st = stat::stat(tmpfile.to_str().unwrap())?;

    if st.st_size != req.file_size {
        return Ok(());
    }

    file.set_permissions(std::fs::Permissions::from_mode(req.file_mode))?;

    unistd::chown(
        tmpfile.to_str().unwrap(),
        Some(Uid::from_raw(req.uid as u32)),
        Some(Gid::from_raw(req.gid as u32)),
    )?;

    fs::rename(tmpfile, path)?;

    Ok(())
}

// Setup container bundle under CONTAINER_BASE, which is cleaned up
// before removing a container.
// - bundle path is /<CONTAINER_BASE>/<cid>/
// - config.json at /<CONTAINER_BASE>/<cid>/config.json
// - container rootfs bind mounted at /<CONTAINER_BASE>/<cid>/rootfs
// - modify container spec root to point to /<CONTAINER_BASE>/<cid>/rootfs
fn setup_bundle(cid: &str, spec: &mut Spec) -> Result<PathBuf> {
    if spec.root.is_none() {
        return Err(nix::Error::Sys(Errno::EINVAL).into());
    }
    let spec_root = spec.root.as_ref().unwrap();

    let bundle_path = Path::new(CONTAINER_BASE).join(cid);
    let config_path = bundle_path.join("config.json");
    let rootfs_path = bundle_path.join("rootfs");

    fs::create_dir_all(&rootfs_path)?;
    BareMount::new(
        &spec_root.path,
        rootfs_path.to_str().unwrap(),
        "bind",
        MsFlags::MS_BIND,
        "",
        &sl!(),
    )
    .mount()?;
    spec.root = Some(Root {
        path: rootfs_path.to_str().unwrap().to_owned(),
        readonly: spec_root.readonly,
    });

    info!(
        sl!(),
        "{:?}",
        spec.process.as_ref().unwrap().console_size.as_ref()
    );
    let _ = spec.save(config_path.to_str().unwrap());

    let olddir = unistd::getcwd().context("cannot getcwd")?;
    unistd::chdir(bundle_path.to_str().unwrap())?;

    Ok(olddir)
}

fn cleanup_process(p: &mut Process) -> Result<()> {
    if p.parent_stdin.is_some() {
        p.close_stream(StreamType::ParentStdin);
        unistd::close(p.parent_stdin.unwrap())?;
    }

    if p.parent_stdout.is_some() {
        p.close_stream(StreamType::ParentStdout);
        unistd::close(p.parent_stdout.unwrap())?;
    }

    if p.parent_stderr.is_some() {
        p.close_stream(StreamType::ParentStderr);
        unistd::close(p.parent_stderr.unwrap())?;
    }

    if p.term_master.is_some() {
        p.close_stream(StreamType::TermMaster);
        unistd::close(p.term_master.unwrap())?;
    }

    p.notify_term_close();

    p.parent_stdin = None;
    p.parent_stdout = None;
    p.parent_stderr = None;
    p.term_master = None;

    Ok(())
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

    if module.parameters.len() > 0 {
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
            let std_out: String = String::from_utf8(output.stdout).unwrap();
            let std_err: String = String::from_utf8(output.stderr).unwrap();
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
    use crate::protocols::agent_ttrpc::AgentService as _;
    use oci::{Hook, Hooks};
    use ttrpc::{r#async::TtrpcContext, MessageHeader};

    fn mk_ttrpc_context() -> TtrpcContext {
        TtrpcContext {
            fd: -1,
            mh: MessageHeader::default(),
            metadata: std::collections::HashMap::new(),
        }
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
        append_guest_hooks(&s, &mut oci);
        assert_eq!(s.hooks, oci.hooks);
    }

    #[tokio::test]
    async fn test_update_interface() {
        let logger = slog::Logger::root(slog::Discard, o!());
        let sandbox = Sandbox::new(&logger).unwrap();

        let agent_service = Box::new(AgentService {
            sandbox: Arc::new(Mutex::new(sandbox)),
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
        });

        let req = protocols::agent::AddARPNeighborsRequest::default();
        let ctx = mk_ttrpc_context();

        let result = agent_service.add_arp_neighbors(&ctx, req).await;

        assert!(result.is_err(), "expected add arp neighbors to fail");
    }

    #[tokio::test]
    async fn test_verify_cid() {
        #[derive(Debug)]
        struct TestData<'a> {
            id: &'a str,
            expect_error: bool,
        }

        let tests = &[
            TestData {
                // Cannot be blank
                id: "",
                expect_error: true,
            },
            TestData {
                // Cannot be a space
                id: " ",
                expect_error: true,
            },
            TestData {
                // Must start with an alphanumeric
                id: ".",
                expect_error: true,
            },
            TestData {
                // Must start with an alphanumeric
                id: "-",
                expect_error: true,
            },
            TestData {
                // Must start with an alphanumeric
                id: "_",
                expect_error: true,
            },
            TestData {
                // Must start with an alphanumeric
                id: " a",
                expect_error: true,
            },
            TestData {
                // Must start with an alphanumeric
                id: ".a",
                expect_error: true,
            },
            TestData {
                // Must start with an alphanumeric
                id: "-a",
                expect_error: true,
            },
            TestData {
                // Must start with an alphanumeric
                id: "_a",
                expect_error: true,
            },
            TestData {
                // Must start with an alphanumeric
                id: "..",
                expect_error: true,
            },
            TestData {
                // Too short
                id: "a",
                expect_error: true,
            },
            TestData {
                // Too short
                id: "z",
                expect_error: true,
            },
            TestData {
                // Too short
                id: "A",
                expect_error: true,
            },
            TestData {
                // Too short
                id: "Z",
                expect_error: true,
            },
            TestData {
                // Too short
                id: "0",
                expect_error: true,
            },
            TestData {
                // Too short
                id: "9",
                expect_error: true,
            },
            TestData {
                // Must start with an alphanumeric
                id: "-1",
                expect_error: true,
            },
            TestData {
                id: "/",
                expect_error: true,
            },
            TestData {
                id: "a/",
                expect_error: true,
            },
            TestData {
                id: "a/../",
                expect_error: true,
            },
            TestData {
                id: "../a",
                expect_error: true,
            },
            TestData {
                id: "../../a",
                expect_error: true,
            },
            TestData {
                id: "../../../a",
                expect_error: true,
            },
            TestData {
                id: "foo/../bar",
                expect_error: true,
            },
            TestData {
                id: "foo bar",
                expect_error: true,
            },
            TestData {
                id: "a.",
                expect_error: false,
            },
            TestData {
                id: "a..",
                expect_error: false,
            },
            TestData {
                id: "aa",
                expect_error: false,
            },
            TestData {
                id: "aa.",
                expect_error: false,
            },
            TestData {
                id: "hello..world",
                expect_error: false,
            },
            TestData {
                id: "hello/../world",
                expect_error: true,
            },
            TestData {
                id: "aa1245124sadfasdfgasdga.",
                expect_error: false,
            },
            TestData {
                id: "aAzZ0123456789_.-",
                expect_error: false,
            },
            TestData {
                id: "abcdefghijklmnopqrstuvwxyz0123456789.-_",
                expect_error: false,
            },
            TestData {
                id: "0123456789abcdefghijklmnopqrstuvwxyz.-_",
                expect_error: false,
            },
            TestData {
                id: " abcdefghijklmnopqrstuvwxyz0123456789.-_",
                expect_error: true,
            },
            TestData {
                id: ".abcdefghijklmnopqrstuvwxyz0123456789.-_",
                expect_error: true,
            },
            TestData {
                id: "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789.-_",
                expect_error: false,
            },
            TestData {
                id: "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ.-_",
                expect_error: false,
            },
            TestData {
                id: " ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789.-_",
                expect_error: true,
            },
            TestData {
                id: ".ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789.-_",
                expect_error: true,
            },
            TestData {
                id: "/a/b/c",
                expect_error: true,
            },
            TestData {
                id: "a/b/c",
                expect_error: true,
            },
            TestData {
                id: "foo/../../../etc/passwd",
                expect_error: true,
            },
            TestData {
                id: "../../../../../../etc/motd",
                expect_error: true,
            },
            TestData {
                id: "/etc/passwd",
                expect_error: true,
            },
        ];

        for (i, d) in tests.iter().enumerate() {
            let msg = format!("test[{}]: {:?}", i, d);

            let result = verify_cid(d.id);

            let msg = format!("{}, result: {:?}", msg, result);

            if result.is_ok() {
                assert!(!d.expect_error, "{}", msg);
            } else {
                assert!(d.expect_error, "{}", msg);
            }
        }
    }
}
