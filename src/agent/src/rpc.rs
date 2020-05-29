// Copyright (c) 2019 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

use std::sync::{Arc, Mutex};
use ttrpc;

use oci::{LinuxNamespace, Spec};
use protobuf::{RepeatedField, SingularPtrField};
use protocols::agent::{
    AgentDetails, CopyFileRequest, GuestDetailsResponse, Interfaces, ListProcessesResponse,
    ReadStreamResponse, Routes, StatsContainerResponse, WaitProcessResponse, WriteStreamResponse,
};
use protocols::empty::Empty;
use protocols::health::{
    HealthCheckResponse, HealthCheckResponse_ServingStatus, VersionCheckResponse,
};
use protocols::types::Interface;
use rustjail;
use rustjail::container::{BaseContainer, LinuxContainer};
use rustjail::errors::*;
use rustjail::process::Process;
use rustjail::specconv::CreateOpts;

use nix::errno::Errno;
use nix::sys::signal::Signal;
use nix::sys::stat;
use nix::unistd::{self, Pid};
use rustjail::process::ProcessOperations;

use crate::device::{add_devices, rescan_pci_bus};
use crate::linux_abi::*;
use crate::mount::{add_storages, remove_mounts, STORAGEHANDLERLIST};
use crate::namespace::{NSTYPEIPC, NSTYPEPID, NSTYPEUTS};
use crate::random;
use crate::sandbox::Sandbox;
use crate::version::{AGENT_VERSION, API_VERSION};
use crate::AGENT_CONFIG;
use netlink::{RtnlHandle, NETLINK_ROUTE};

use libc::{self, c_ushort, pid_t, winsize, TIOCSWINSZ};
use serde_json;
use std::convert::TryFrom;
use std::fs;
use std::os::unix::io::RawFd;
use std::os::unix::prelude::PermissionsExt;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
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
pub struct agentService {
    sandbox: Arc<Mutex<Sandbox>>,
    test: u32,
}

impl agentService {
    fn do_create_container(&self, req: protocols::agent::CreateContainerRequest) -> Result<()> {
        let cid = req.container_id.clone();

        let mut oci_spec = req.OCI.clone();

        let sandbox;
        let mut s;

        let mut oci = match oci_spec.as_mut() {
            Some(spec) => rustjail::grpc_to_oci(spec),
            None => {
                error!(sl!(), "no oci spec in the create container request!");
                return Err(
                    ErrorKind::Nix(nix::Error::from_errno(nix::errno::Errno::EINVAL)).into(),
                );
            }
        };

        info!(sl!(), "receive createcontainer {}", &cid);

        // re-scan PCI bus
        // looking for hidden devices

        rescan_pci_bus().chain_err(|| "Could not rescan PCI bus")?;

        // Some devices need some extra processing (the ones invoked with
        // --device for instance), and that's what this call is doing. It
        // updates the devices listed in the OCI spec, so that they actually
        // match real devices inside the VM. This step is necessary since we
        // cannot predict everything from the caller.
        add_devices(&req.devices.to_vec(), &mut oci, &self.sandbox)?;

        // Both rootfs and volumes (invoked with --volume for instance) will
        // be processed the same way. The idea is to always mount any provided
        // storage to the specified MountPoint, so that it will match what's
        // inside oci.Mounts.
        // After all those storages have been processed, no matter the order
        // here, the agent will rely on rustjail (using the oci.Mounts
        // list) to bind mount all of them inside the container.
        let m = add_storages(sl!(), req.storages.to_vec(), self.sandbox.clone())?;
        {
            sandbox = self.sandbox.clone();
            s = sandbox.lock().unwrap();
            s.container_mounts.insert(cid.clone(), m);
        }

        update_container_namespaces(&s, &mut oci)?;

        // write spec to bundle path, hooks might
        // read ocispec
        let olddir = setup_bundle(&oci)?;
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

        let pipe_size = AGENT_CONFIG.read().unwrap().container_pipe_size;
        let p = if oci.process.is_some() {
            let tp = Process::new(
                &sl!(),
                &oci.process.as_ref().unwrap(),
                cid.as_str(),
                true,
                pipe_size,
            )?;
            tp
        } else {
            info!(sl!(), "no process configurations!");
            return Err(ErrorKind::Nix(nix::Error::from_errno(nix::errno::Errno::EINVAL)).into());
        };

        ctr.start(p)?;

        s.add_container(ctr);
        info!(sl!(), "created container!");

        Ok(())
    }

    fn do_start_container(&self, req: protocols::agent::StartContainerRequest) -> Result<()> {
        let cid = req.container_id.clone();

        let sandbox = self.sandbox.clone();
        let mut s = sandbox.lock().unwrap();

        let ctr: &mut LinuxContainer = match s.get_container(cid.as_str()) {
            Some(cr) => cr,
            None => {
                return Err(ErrorKind::Nix(nix::Error::from_errno(Errno::EINVAL)).into());
            }
        };

        ctr.exec()?;

        Ok(())
    }

    fn do_remove_container(&self, req: protocols::agent::RemoveContainerRequest) -> Result<()> {
        let cid = req.container_id.clone();
        let mut cmounts: Vec<String> = vec![];

        if req.timeout == 0 {
            let s = Arc::clone(&self.sandbox);
            let mut sandbox = s.lock().unwrap();
            let ctr: &mut LinuxContainer = match sandbox.get_container(cid.as_str()) {
                Some(cr) => cr,
                None => {
                    return Err(ErrorKind::Nix(nix::Error::from_errno(Errno::EINVAL)).into());
                }
            };

            ctr.destroy()?;

            // Find the sandbox storage used by this container
            let mounts = sandbox.container_mounts.get(&cid);
            if mounts.is_some() {
                let mounts = mounts.unwrap();

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

            return Ok(());
        }

        // timeout != 0
        let s = self.sandbox.clone();
        let cid2 = cid.clone();
        let (tx, rx) = mpsc::channel();

        let handle = thread::spawn(move || {
            let mut sandbox = s.lock().unwrap();
            let ctr: &mut LinuxContainer = match sandbox.get_container(cid2.as_str()) {
                Some(cr) => cr,
                None => {
                    return;
                }
            };

            ctr.destroy().unwrap();
            tx.send(1).unwrap();
        });

        if let Err(_) = rx.recv_timeout(Duration::from_secs(req.timeout as u64)) {
            return Err(ErrorKind::Nix(nix::Error::from_errno(nix::errno::Errno::ETIME)).into());
        }

        if let Err(_) = handle.join() {
            return Err(
                ErrorKind::Nix(nix::Error::from_errno(nix::errno::Errno::UnknownErrno)).into(),
            );
        }

        let s = self.sandbox.clone();
        let mut sandbox = s.lock().unwrap();

        // Find the sandbox storage used by this container
        let mounts = sandbox.container_mounts.get(&cid);
        if mounts.is_some() {
            let mounts = mounts.unwrap();

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

        sandbox.container_mounts.remove(&cid);
        sandbox.containers.remove(cid.as_str());

        Ok(())
    }

    fn do_exec_process(&self, req: protocols::agent::ExecProcessRequest) -> Result<()> {
        let cid = req.container_id.clone();
        let exec_id = req.exec_id.clone();

        info!(sl!(), "cid: {} eid: {}", cid.clone(), exec_id.clone());

        let s = self.sandbox.clone();
        let mut sandbox = s.lock().unwrap();

        // ignore string_user, not sure what it is
        let process = if req.process.is_some() {
            req.process.as_ref().unwrap()
        } else {
            return Err(ErrorKind::Nix(nix::Error::from_errno(nix::errno::Errno::EINVAL)).into());
        };

        let pipe_size = AGENT_CONFIG.read().unwrap().container_pipe_size;
        let ocip = rustjail::process_grpc_to_oci(process);
        let p = Process::new(&sl!(), &ocip, exec_id.as_str(), false, pipe_size)?;

        let ctr = match sandbox.get_container(cid.as_str()) {
            Some(v) => v,
            None => {
                return Err(
                    ErrorKind::Nix(nix::Error::from_errno(nix::errno::Errno::EINVAL)).into(),
                );
            }
        };

        ctr.run(p)?;

        Ok(())
    }

    fn do_signal_process(&self, req: protocols::agent::SignalProcessRequest) -> Result<()> {
        let cid = req.container_id.clone();
        let eid = req.exec_id.clone();
        let s = self.sandbox.clone();
        let mut sandbox = s.lock().unwrap();
        let mut init = false;

        info!(
            sl!(),
            "signal process";
            "container-id" => cid.clone(),
            "exec-id" => eid.clone()
        );

        if eid == "" {
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

    fn do_wait_process(
        &self,
        req: protocols::agent::WaitProcessRequest,
    ) -> Result<protocols::agent::WaitProcessResponse> {
        let cid = req.container_id.clone();
        let eid = req.exec_id.clone();
        let s = self.sandbox.clone();
        let mut resp = WaitProcessResponse::new();
        let pid: pid_t;
        let mut exit_pipe_r: RawFd = -1;
        let mut buf: Vec<u8> = vec![0, 1];

        info!(
            sl!(),
            "wait process";
            "container-id" => cid.clone(),
            "exec-id" => eid.clone()
        );

        {
            let mut sandbox = s.lock().unwrap();

            let p = find_process(&mut sandbox, cid.as_str(), eid.as_str(), false)?;

            if p.exit_pipe_r.is_some() {
                exit_pipe_r = p.exit_pipe_r.unwrap();
            }

            pid = p.pid;
        }

        if exit_pipe_r != -1 {
            info!(sl!(), "reading exit pipe");
            let _ = unistd::read(exit_pipe_r, buf.as_mut_slice());
        }

        let mut sandbox = s.lock().unwrap();
        let ctr: &mut LinuxContainer = match sandbox.get_container(cid.as_str()) {
            Some(cr) => cr,
            None => {
                return Err(ErrorKind::Nix(nix::Error::from_errno(Errno::EINVAL)).into());
            }
        };

        // need to close all fds
        let mut p = ctr.processes.get_mut(&pid).unwrap();

        if p.parent_stdin.is_some() {
            let _ = unistd::close(p.parent_stdin.unwrap());
        }

        if p.parent_stdout.is_some() {
            let _ = unistd::close(p.parent_stdout.unwrap());
        }

        if p.parent_stderr.is_some() {
            let _ = unistd::close(p.parent_stderr.unwrap());
        }

        if p.term_master.is_some() {
            let _ = unistd::close(p.term_master.unwrap());
        }

        if p.exit_pipe_r.is_some() {
            let _ = unistd::close(p.exit_pipe_r.unwrap());
        }

        p.parent_stdin = None;
        p.parent_stdout = None;
        p.parent_stderr = None;
        p.term_master = None;

        resp.status = p.exit_code;

        ctr.processes.remove(&pid);

        Ok(resp)
    }

    fn do_write_stream(
        &self,
        req: protocols::agent::WriteStreamRequest,
    ) -> Result<protocols::agent::WriteStreamResponse> {
        let cid = req.container_id.clone();
        let eid = req.exec_id.clone();

        info!(
            sl!(),
            "write stdin";
            "container-id" => cid.clone(),
            "exec-id" => eid.clone()
        );

        let s = self.sandbox.clone();
        let mut sandbox = s.lock().unwrap();
        let p = find_process(&mut sandbox, cid.as_str(), eid.as_str(), false)?;

        // use ptmx io
        let fd = if p.term_master.is_some() {
            p.term_master.unwrap()
        } else {
            // use piped io
            p.parent_stdin.unwrap()
        };

        let mut l = req.data.len();
        match unistd::write(fd, req.data.as_slice()) {
            Ok(v) => {
                if v < l {
                    info!(sl!(), "write {} bytes", v);
                    l = v;
                }
            }
            Err(e) => match e {
                nix::Error::Sys(nix::errno::Errno::EAGAIN) => l = 0,
                _ => {
                    return Err(
                        ErrorKind::Nix(nix::Error::from_errno(nix::errno::Errno::EIO)).into(),
                    );
                }
            },
        }

        let mut resp = WriteStreamResponse::new();
        resp.set_len(l as u32);

        Ok(resp)
    }

    fn do_read_stream(
        &self,
        req: protocols::agent::ReadStreamRequest,
        stdout: bool,
    ) -> Result<protocols::agent::ReadStreamResponse> {
        let cid = req.container_id;
        let eid = req.exec_id;

        let mut fd: RawFd = -1;
        info!(sl!(), "read stdout for {}/{}", cid.clone(), eid.clone());
        {
            let s = self.sandbox.clone();
            let mut sandbox = s.lock().unwrap();

            let p = find_process(&mut sandbox, cid.as_str(), eid.as_str(), false)?;

            if p.term_master.is_some() {
                fd = p.term_master.unwrap();
            } else if stdout {
                if p.parent_stdout.is_some() {
                    fd = p.parent_stdout.unwrap();
                }
            } else {
                fd = p.parent_stderr.unwrap();
            }
        }

        if fd == -1 {
            return Err(ErrorKind::Nix(nix::Error::from_errno(nix::errno::Errno::EINVAL)).into());
        }

        let vector = read_stream(fd, req.len as usize)?;

        let mut resp = ReadStreamResponse::new();
        resp.set_data(vector);

        Ok(resp)
    }
}

impl protocols::agent_ttrpc::AgentService for agentService {
    fn create_container(
        &self,
        _ctx: &ttrpc::TtrpcContext,
        req: protocols::agent::CreateContainerRequest,
    ) -> ttrpc::Result<Empty> {
        match self.do_create_container(req) {
            Err(e) => Err(ttrpc::Error::RpcStatus(ttrpc::get_status(
                ttrpc::Code::INTERNAL,
                e.to_string(),
            ))),
            Ok(_) => Ok(Empty::new()),
        }
    }

    fn start_container(
        &self,
        _ctx: &ttrpc::TtrpcContext,
        req: protocols::agent::StartContainerRequest,
    ) -> ttrpc::Result<Empty> {
        match self.do_start_container(req) {
            Err(e) => Err(ttrpc::Error::RpcStatus(ttrpc::get_status(
                ttrpc::Code::INTERNAL,
                e.to_string(),
            ))),
            Ok(_) => {
                info!(sl!(), "exec process!\n");
                Ok(Empty::new())
            }
        }
    }

    fn remove_container(
        &self,
        _ctx: &ttrpc::TtrpcContext,
        req: protocols::agent::RemoveContainerRequest,
    ) -> ttrpc::Result<Empty> {
        match self.do_remove_container(req) {
            Err(e) => Err(ttrpc::Error::RpcStatus(ttrpc::get_status(
                ttrpc::Code::INTERNAL,
                e.to_string(),
            ))),
            Ok(_) => Ok(Empty::new()),
        }
    }
    fn exec_process(
        &self,
        _ctx: &ttrpc::TtrpcContext,
        req: protocols::agent::ExecProcessRequest,
    ) -> ttrpc::Result<Empty> {
        match self.do_exec_process(req) {
            Err(e) => Err(ttrpc::Error::RpcStatus(ttrpc::get_status(
                ttrpc::Code::INTERNAL,
                e.to_string(),
            ))),
            Ok(_) => Ok(Empty::new()),
        }
    }
    fn signal_process(
        &self,
        _ctx: &ttrpc::TtrpcContext,
        req: protocols::agent::SignalProcessRequest,
    ) -> ttrpc::Result<Empty> {
        match self.do_signal_process(req) {
            Err(e) => Err(ttrpc::Error::RpcStatus(ttrpc::get_status(
                ttrpc::Code::INTERNAL,
                e.to_string(),
            ))),
            Ok(_) => Ok(Empty::new()),
        }
    }
    fn wait_process(
        &self,
        _ctx: &ttrpc::TtrpcContext,
        req: protocols::agent::WaitProcessRequest,
    ) -> ttrpc::Result<WaitProcessResponse> {
        match self.do_wait_process(req) {
            Err(e) => Err(ttrpc::Error::RpcStatus(ttrpc::get_status(
                ttrpc::Code::INTERNAL,
                e.to_string(),
            ))),
            Ok(resp) => Ok(resp),
        }
    }
    fn list_processes(
        &self,
        _ctx: &ttrpc::TtrpcContext,
        req: protocols::agent::ListProcessesRequest,
    ) -> ttrpc::Result<ListProcessesResponse> {
        let cid = req.container_id.clone();
        let format = req.format.clone();
        let mut args = req.args.clone().into_vec();
        let mut resp = ListProcessesResponse::new();

        let s = Arc::clone(&self.sandbox);
        let mut sandbox = s.lock().unwrap();

        let ctr: &mut LinuxContainer = match sandbox.get_container(cid.as_str()) {
            Some(cr) => cr,
            None => {
                return Err(ttrpc::Error::RpcStatus(ttrpc::get_status(
                    ttrpc::Code::INVALID_ARGUMENT,
                    "invalid container id".to_string(),
                )));
            }
        };

        let pids = ctr.processes().unwrap();

        match format.as_str() {
            "table" => {}
            "json" => {
                resp.process_list = serde_json::to_vec(&pids).unwrap();
                return Ok(resp);
            }
            _ => {
                return Err(ttrpc::Error::RpcStatus(ttrpc::get_status(
                    ttrpc::Code::INVALID_ARGUMENT,
                    "invalid format!".to_string(),
                )));
            }
        }

        // format "table"
        if args.len() == 0 {
            // default argument
            args = vec!["-ef".to_string()];
        }

        let output = Command::new("ps")
            .args(args.as_slice())
            .stdout(Stdio::piped())
            .output()
            .expect("ps failed");

        let out: String = String::from_utf8(output.stdout).unwrap();
        let mut lines: Vec<String> = out.split('\n').map(|v| v.to_string()).collect();

        let predicate = |v| {
            if v == "PID" {
                return true;
            } else {
                return false;
            }
        };

        let pid_index = lines[0].split_whitespace().position(predicate).unwrap();

        let mut result = String::new();
        result.push_str(lines[0].as_str());

        lines.remove(0);
        for line in &lines {
            if line.trim().is_empty() {
                continue;
            }

            let fields: Vec<String> = line.split_whitespace().map(|v| v.to_string()).collect();

            if fields.len() < pid_index + 1 {
                warn!(sl!(), "corrupted output?");
                continue;
            }
            let pid = fields[pid_index].trim().parse::<i32>().unwrap();

            for p in &pids {
                if pid == *p {
                    result.push_str(line.as_str());
                }
            }
        }

        resp.process_list = Vec::from(result);
        Ok(resp)
    }
    fn update_container(
        &self,
        _ctx: &ttrpc::TtrpcContext,
        req: protocols::agent::UpdateContainerRequest,
    ) -> ttrpc::Result<Empty> {
        let cid = req.container_id.clone();
        let res = req.resources;

        let s = Arc::clone(&self.sandbox);
        let mut sandbox = s.lock().unwrap();

        let ctr: &mut LinuxContainer = match sandbox.get_container(cid.as_str()) {
            Some(cr) => cr,
            None => {
                return Err(ttrpc::Error::RpcStatus(ttrpc::get_status(
                    ttrpc::Code::INTERNAL,
                    "invalid container id".to_string(),
                )));
            }
        };

        let resp = Empty::new();

        if res.is_some() {
            let ociRes = rustjail::resources_grpc_to_oci(&res.unwrap());
            match ctr.set(ociRes) {
                Err(e) => {
                    return Err(ttrpc::Error::RpcStatus(ttrpc::get_status(
                        ttrpc::Code::INTERNAL,
                        e.to_string(),
                    )));
                }

                Ok(_) => return Ok(resp),
            }
        }

        Ok(resp)
    }
    fn stats_container(
        &self,
        _ctx: &ttrpc::TtrpcContext,
        req: protocols::agent::StatsContainerRequest,
    ) -> ttrpc::Result<StatsContainerResponse> {
        let cid = req.container_id.clone();
        let s = Arc::clone(&self.sandbox);
        let mut sandbox = s.lock().unwrap();

        let ctr: &mut LinuxContainer = match sandbox.get_container(cid.as_str()) {
            Some(cr) => cr,
            None => {
                return Err(ttrpc::Error::RpcStatus(ttrpc::get_status(
                    ttrpc::Code::INTERNAL,
                    "invalid container id".to_string(),
                )));
            }
        };

        match ctr.stats() {
            Err(e) => Err(ttrpc::Error::RpcStatus(ttrpc::get_status(
                ttrpc::Code::INTERNAL,
                e.to_string(),
            ))),
            Ok(resp) => Ok(resp),
        }
    }
    fn write_stdin(
        &self,
        _ctx: &ttrpc::TtrpcContext,
        req: protocols::agent::WriteStreamRequest,
    ) -> ttrpc::Result<WriteStreamResponse> {
        match self.do_write_stream(req) {
            Err(e) => Err(ttrpc::Error::RpcStatus(ttrpc::get_status(
                ttrpc::Code::INTERNAL,
                e.to_string(),
            ))),
            Ok(resp) => Ok(resp),
        }
    }
    fn read_stdout(
        &self,
        _ctx: &ttrpc::TtrpcContext,
        req: protocols::agent::ReadStreamRequest,
    ) -> ttrpc::Result<ReadStreamResponse> {
        match self.do_read_stream(req, true) {
            Err(e) => Err(ttrpc::Error::RpcStatus(ttrpc::get_status(
                ttrpc::Code::INTERNAL,
                e.to_string(),
            ))),
            Ok(resp) => Ok(resp),
        }
    }
    fn read_stderr(
        &self,
        _ctx: &ttrpc::TtrpcContext,
        req: protocols::agent::ReadStreamRequest,
    ) -> ttrpc::Result<ReadStreamResponse> {
        match self.do_read_stream(req, false) {
            Err(e) => Err(ttrpc::Error::RpcStatus(ttrpc::get_status(
                ttrpc::Code::INTERNAL,
                e.to_string(),
            ))),
            Ok(resp) => Ok(resp),
        }
    }
    fn close_stdin(
        &self,
        _ctx: &ttrpc::TtrpcContext,
        req: protocols::agent::CloseStdinRequest,
    ) -> ttrpc::Result<Empty> {
        let cid = req.container_id.clone();
        let eid = req.exec_id.clone();
        let s = Arc::clone(&self.sandbox);
        let mut sandbox = s.lock().unwrap();

        let p = match find_process(&mut sandbox, cid.as_str(), eid.as_str(), false) {
            Ok(v) => v,
            Err(_) => {
                return Err(ttrpc::Error::RpcStatus(ttrpc::get_status(
                    ttrpc::Code::INVALID_ARGUMENT,
                    "invalid argument".to_string(),
                )));
            }
        };

        if p.term_master.is_some() {
            let _ = unistd::close(p.term_master.unwrap());
            p.term_master = None;
        }

        if p.parent_stdin.is_some() {
            let _ = unistd::close(p.parent_stdin.unwrap());
            p.parent_stdin = None;
        }

        Ok(Empty::new())
    }

    fn tty_win_resize(
        &self,
        _ctx: &ttrpc::TtrpcContext,
        req: protocols::agent::TtyWinResizeRequest,
    ) -> ttrpc::Result<Empty> {
        let cid = req.container_id.clone();
        let eid = req.exec_id.clone();
        let s = Arc::clone(&self.sandbox);
        let mut sandbox = s.lock().unwrap();
        let p = match find_process(&mut sandbox, cid.as_str(), eid.as_str(), false) {
            Ok(v) => v,
            Err(_e) => {
                return Err(ttrpc::Error::RpcStatus(ttrpc::get_status(
                    ttrpc::Code::UNAVAILABLE,
                    "cannot find the process".to_string(),
                )));
            }
        };

        if p.term_master.is_none() {
            return Err(ttrpc::Error::RpcStatus(ttrpc::get_status(
                ttrpc::Code::UNAVAILABLE,
                "no tty".to_string(),
            )));
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
            if let Err(_) = Errno::result(err).map(drop) {
                return Err(ttrpc::Error::RpcStatus(ttrpc::get_status(
                    ttrpc::Code::INTERNAL,
                    "ioctl error".to_string(),
                )));
            }
        }

        Ok(Empty::new())
    }

    fn update_interface(
        &self,
        _ctx: &ttrpc::TtrpcContext,
        req: protocols::agent::UpdateInterfaceRequest,
    ) -> ttrpc::Result<Interface> {
        let interface = req.interface.clone();
        let s = Arc::clone(&self.sandbox);
        let mut sandbox = s.lock().unwrap();

        if sandbox.rtnl.is_none() {
            sandbox.rtnl = Some(RtnlHandle::new(NETLINK_ROUTE, 0).unwrap());
        }

        let rtnl = sandbox.rtnl.as_mut().unwrap();

        let iface = match rtnl.update_interface(interface.as_ref().unwrap()) {
            Ok(v) => v,
            Err(_) => {
                return Err(ttrpc::Error::RpcStatus(ttrpc::get_status(
                    ttrpc::Code::INTERNAL,
                    "update interface".to_string(),
                )));
            }
        };

        Ok(iface)
    }
    fn update_routes(
        &self,
        _ctx: &ttrpc::TtrpcContext,
        req: protocols::agent::UpdateRoutesRequest,
    ) -> ttrpc::Result<Routes> {
        let mut routes = protocols::agent::Routes::new();
        let rs = req.routes.clone().unwrap().Routes.into_vec();

        let s = Arc::clone(&self.sandbox);
        let mut sandbox = s.lock().unwrap();

        if sandbox.rtnl.is_none() {
            sandbox.rtnl = Some(RtnlHandle::new(NETLINK_ROUTE, 0).unwrap());
        }

        let rtnl = sandbox.rtnl.as_mut().unwrap();
        // get current routes to return when error out
        let crs = match rtnl.list_routes() {
            Ok(routes) => routes,
            Err(_) => {
                return Err(ttrpc::Error::RpcStatus(ttrpc::get_status(
                    ttrpc::Code::INTERNAL,
                    "update routes".to_string(),
                )));
            }
        };
        let v = match rtnl.update_routes(rs.as_ref()) {
            Ok(value) => value,
            Err(_) => crs,
        };

        routes.set_Routes(RepeatedField::from_vec(v));

        Ok(routes)
    }
    fn list_interfaces(
        &self,
        _ctx: &ttrpc::TtrpcContext,
        _req: protocols::agent::ListInterfacesRequest,
    ) -> ttrpc::Result<Interfaces> {
        let mut interface = protocols::agent::Interfaces::new();
        let s = Arc::clone(&self.sandbox);
        let mut sandbox = s.lock().unwrap();

        if sandbox.rtnl.is_none() {
            sandbox.rtnl = Some(RtnlHandle::new(NETLINK_ROUTE, 0).unwrap());
        }

        let rtnl = sandbox.rtnl.as_mut().unwrap();
        let v = match rtnl.list_interfaces() {
            Ok(value) => value,
            Err(_) => {
                return Err(ttrpc::Error::RpcStatus(ttrpc::get_status(
                    ttrpc::Code::INTERNAL,
                    "list interface".to_string(),
                )));
            }
        };

        interface.set_Interfaces(RepeatedField::from_vec(v));

        Ok(interface)
    }
    fn list_routes(
        &self,
        _ctx: &ttrpc::TtrpcContext,
        _req: protocols::agent::ListRoutesRequest,
    ) -> ttrpc::Result<Routes> {
        let mut routes = protocols::agent::Routes::new();
        let s = Arc::clone(&self.sandbox);
        let mut sandbox = s.lock().unwrap();

        if sandbox.rtnl.is_none() {
            sandbox.rtnl = Some(RtnlHandle::new(NETLINK_ROUTE, 0).unwrap());
        }

        let rtnl = sandbox.rtnl.as_mut().unwrap();

        let v = match rtnl.list_routes() {
            Ok(value) => value,
            Err(_) => {
                return Err(ttrpc::Error::RpcStatus(ttrpc::get_status(
                    ttrpc::Code::INTERNAL,
                    "list routes".to_string(),
                )));
            }
        };

        routes.set_Routes(RepeatedField::from_vec(v));

        Ok(routes)
    }
    fn start_tracing(
        &self,
        _ctx: &ttrpc::TtrpcContext,
        req: protocols::agent::StartTracingRequest,
    ) -> ttrpc::Result<Empty> {
        info!(sl!(), "start_tracing {:?} self.test={}", req, self.test);
        Ok(Empty::new())
    }
    fn stop_tracing(
        &self,
        _ctx: &ttrpc::TtrpcContext,
        _req: protocols::agent::StopTracingRequest,
    ) -> ttrpc::Result<Empty> {
        Ok(Empty::new())
    }
    fn create_sandbox(
        &self,
        _ctx: &ttrpc::TtrpcContext,
        req: protocols::agent::CreateSandboxRequest,
    ) -> ttrpc::Result<Empty> {
        {
            let sandbox = self.sandbox.clone();
            let mut s = sandbox.lock().unwrap();

            let _ = fs::remove_dir_all(CONTAINER_BASE);
            let _ = fs::create_dir_all(CONTAINER_BASE);

            s.hostname = req.hostname.clone();
            s.running = true;

            if req.sandbox_id.len() > 0 {
                s.id = req.sandbox_id.clone();
            }

            for m in req.kernel_modules.iter() {
                match load_kernel_module(m) {
                    Ok(_) => (),
                    Err(e) => {
                        return Err(ttrpc::Error::RpcStatus(ttrpc::get_status(
                            ttrpc::Code::INTERNAL,
                            e.to_string(),
                        )))
                    }
                }
            }

            match s.setup_shared_namespaces() {
                Ok(_) => (),
                Err(e) => {
                    return Err(ttrpc::Error::RpcStatus(ttrpc::get_status(
                        ttrpc::Code::INTERNAL,
                        e.to_string(),
                    )))
                }
            }
        }

        match add_storages(sl!(), req.storages.to_vec(), self.sandbox.clone()) {
            Ok(m) => {
                let sandbox = self.sandbox.clone();
                let mut s = sandbox.lock().unwrap();
                s.mounts = m
            }
            Err(e) => {
                return Err(ttrpc::Error::RpcStatus(ttrpc::get_status(
                    ttrpc::Code::INTERNAL,
                    e.to_string(),
                )))
            }
        };

        Ok(Empty::new())
    }
    fn destroy_sandbox(
        &self,
        _ctx: &ttrpc::TtrpcContext,
        _req: protocols::agent::DestroySandboxRequest,
    ) -> ttrpc::Result<Empty> {
        let s = Arc::clone(&self.sandbox);
        let mut sandbox = s.lock().unwrap();
        // destroy all containers, clean up, notify agent to exit
        // etc.
        sandbox.destroy().unwrap();

        sandbox.sender.as_ref().unwrap().send(1).unwrap();
        sandbox.sender = None;

        Ok(Empty::new())
    }
    fn add_arp_neighbors(
        &self,
        _ctx: &ttrpc::TtrpcContext,
        req: protocols::agent::AddARPNeighborsRequest,
    ) -> ttrpc::Result<Empty> {
        let neighs = req.neighbors.clone().unwrap().ARPNeighbors.into_vec();

        let s = Arc::clone(&self.sandbox);
        let mut sandbox = s.lock().unwrap();

        if sandbox.rtnl.is_none() {
            sandbox.rtnl = Some(RtnlHandle::new(NETLINK_ROUTE, 0).unwrap());
        }

        let rtnl = sandbox.rtnl.as_mut().unwrap();

        if let Err(e) = rtnl.add_arp_neighbors(neighs.as_ref()) {
            return Err(ttrpc::Error::RpcStatus(ttrpc::get_status(
                ttrpc::Code::INTERNAL,
                e.to_string(),
            )));
        }

        Ok(Empty::new())
    }
    fn online_cpu_mem(
        &self,
        _ctx: &ttrpc::TtrpcContext,
        req: protocols::agent::OnlineCPUMemRequest,
    ) -> ttrpc::Result<Empty> {
        // sleep 5 seconds for debug
        // thread::sleep(Duration::new(5, 0));
        let s = Arc::clone(&self.sandbox);
        let sandbox = s.lock().unwrap();

        if let Err(e) = sandbox.online_cpu_memory(&req) {
            return Err(ttrpc::Error::RpcStatus(ttrpc::get_status(
                ttrpc::Code::INTERNAL,
                e.to_string(),
            )));
        }

        Ok(Empty::new())
    }
    fn reseed_random_dev(
        &self,
        _ctx: &ttrpc::TtrpcContext,
        req: protocols::agent::ReseedRandomDevRequest,
    ) -> ttrpc::Result<Empty> {
        if let Err(e) = random::reseed_rng(req.data.as_slice()) {
            return Err(ttrpc::Error::RpcStatus(ttrpc::get_status(
                ttrpc::Code::INTERNAL,
                e.to_string(),
            )));
        }

        Ok(Empty::new())
    }
    fn get_guest_details(
        &self,
        _ctx: &ttrpc::TtrpcContext,
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
                return Err(ttrpc::Error::RpcStatus(ttrpc::get_status(
                    ttrpc::Code::INTERNAL,
                    e.to_string(),
                )));
            }
        }

        // to get agent details
        let detail = get_agent_details();
        resp.agent_details = SingularPtrField::some(detail);

        Ok(resp)
    }
    fn mem_hotplug_by_probe(
        &self,
        _ctx: &ttrpc::TtrpcContext,
        req: protocols::agent::MemHotplugByProbeRequest,
    ) -> ttrpc::Result<Empty> {
        if let Err(e) = do_mem_hotplug_by_probe(&req.memHotplugProbeAddr) {
            return Err(ttrpc::Error::RpcStatus(ttrpc::get_status(
                ttrpc::Code::INTERNAL,
                e.to_string(),
            )));
        }

        Ok(Empty::new())
    }
    fn set_guest_date_time(
        &self,
        _ctx: &ttrpc::TtrpcContext,
        req: protocols::agent::SetGuestDateTimeRequest,
    ) -> ttrpc::Result<Empty> {
        if let Err(e) = do_set_guest_date_time(req.Sec, req.Usec) {
            return Err(ttrpc::Error::RpcStatus(ttrpc::get_status(
                ttrpc::Code::INTERNAL,
                e.to_string(),
            )));
        }

        Ok(Empty::new())
    }
    fn copy_file(
        &self,
        _ctx: &ttrpc::TtrpcContext,
        req: protocols::agent::CopyFileRequest,
    ) -> ttrpc::Result<Empty> {
        if let Err(e) = do_copy_file(&req) {
            return Err(ttrpc::Error::RpcStatus(ttrpc::get_status(
                ttrpc::Code::INTERNAL,
                e.to_string(),
            )));
        }

        Ok(Empty::new())
    }
}

#[derive(Clone)]
struct healthService;
impl protocols::health_ttrpc::Health for healthService {
    fn check(
        &self,
        _ctx: &ttrpc::TtrpcContext,
        _req: protocols::health::CheckRequest,
    ) -> ttrpc::Result<HealthCheckResponse> {
        let mut resp = HealthCheckResponse::new();
        resp.set_status(HealthCheckResponse_ServingStatus::SERVING);

        Ok(resp)
    }
    fn version(
        &self,
        _ctx: &ttrpc::TtrpcContext,
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
                if v.len() == 0 {
                    info!(sl!(), "string in empty???");
                    return Err(ErrorKind::ErrorCode("Invalid block size".to_string()).into());
                }

                size = v.trim().parse::<u64>()?;
            }
            Err(e) => {
                info!(sl!(), "memory block size error: {:?}", e.kind());
                if e.kind() != std::io::ErrorKind::NotFound {
                    return Err(ErrorKind::Io(e).into());
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
                        _ => return Err(ErrorKind::Nix(e).into()),
                    },
                    _ => return Err(ErrorKind::Nix(e).into()),
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
    detail.init_daemon = { unistd::getpid() == Pid::from_raw(1) };

    detail.device_handlers = RepeatedField::new();
    detail.storage_handlers = RepeatedField::from_vec(
        STORAGEHANDLERLIST
            .keys()
            .cloned()
            .map(|x| x.into())
            .collect(),
    );

    detail
}

fn read_stream(fd: RawFd, l: usize) -> Result<Vec<u8>> {
    let mut v: Vec<u8> = Vec::with_capacity(l);
    unsafe {
        v.set_len(l);
    }

    match unistd::read(fd, v.as_mut_slice()) {
        Ok(len) => {
            v.resize(len, 0);
            // Rust didn't return an EOF error when the reading peer point
            // was closed, instead it would return a 0 reading length, please
            // see https://github.com/rust-lang/rfcs/blob/master/text/0517-io-os-reform.md#errors
            if len == 0 {
                return Err(ErrorKind::ErrorCode("read  meet eof".to_string()).into());
            }
        }
        Err(e) => match e {
            nix::Error::Sys(errno) => match errno {
                Errno::EAGAIN => v.resize(0, 0),
                _ => return Err(ErrorKind::Nix(nix::Error::Sys(errno)).into()),
            },
            _ => return Err(ErrorKind::ErrorCode("read error".to_string()).into()),
        },
    }

    Ok(v)
}

fn find_process<'a>(
    sandbox: &'a mut Sandbox,
    cid: &'a str,
    eid: &'a str,
    init: bool,
) -> Result<&'a mut Process> {
    let ctr = match sandbox.get_container(cid) {
        Some(v) => v,
        None => return Err(ErrorKind::ErrorCode(String::from("Invalid container id")).into()),
    };

    if init || eid == "" {
        let p = match ctr.processes.get_mut(&ctr.init_process_pid) {
            Some(v) => v,
            None => {
                return Err(ErrorKind::ErrorCode(String::from("cannot find init process!")).into())
            }
        };

        return Ok(p);
    }

    let p = match ctr.get_process(eid) {
        Ok(v) => v,
        Err(_) => return Err(ErrorKind::ErrorCode("Invalid exec id".to_string()).into()),
    };

    Ok(p)
}

pub fn start<S: Into<String>>(s: Arc<Mutex<Sandbox>>, host: S, port: u16) -> ttrpc::Server {
    let agent_service = Box::new(agentService {
        sandbox: s,
        test: 1,
    }) as Box<dyn protocols::agent_ttrpc::AgentService + Send + Sync>;

    let agent_worker = Arc::new(agent_service);

    let health_service =
        Box::new(healthService {}) as Box<dyn protocols::health_ttrpc::Health + Send + Sync>;
    let health_worker = Arc::new(health_service);

    let aservice = protocols::agent_ttrpc::create_agent_service(agent_worker);

    let hservice = protocols::health_ttrpc::create_health(health_worker);

    let mut addr: String = host.into();
    addr.push_str(":");
    addr.push_str(&port.to_string());

    let server = ttrpc::Server::new()
        .bind(addr.as_str())
        .unwrap()
        .register_service(aservice)
        .register_service(hservice);

    info!(sl!(), "ttRPC server started");

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
fn update_container_namespaces(sandbox: &Sandbox, spec: &mut Spec) -> Result<()> {
    let linux = match spec.linux.as_mut() {
        None => {
            return Err(
                ErrorKind::ErrorCode("Spec didn't container linux field".to_string()).into(),
            )
        }
        Some(l) => l,
    };

    let mut pidNs = false;

    let namespaces = linux.namespaces.as_mut_slice();
    for namespace in namespaces.iter_mut() {
        if namespace.r#type == NSTYPEPID {
            pidNs = true;
            continue;
        }
        if namespace.r#type == NSTYPEIPC {
            namespace.path = sandbox.shared_ipcns.path.clone();
            continue;
        }
        if namespace.r#type == NSTYPEUTS {
            namespace.path = sandbox.shared_utsns.path.clone();
            continue;
        }
    }

    if !pidNs && !sandbox.sandbox_pid_ns {
        let mut pid_ns = LinuxNamespace::default();
        pid_ns.r#type = NSTYPEPID.to_string();
        linux.namespaces.push(pid_ns);
    }

    Ok(())
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
            let mask_vec: Vec<&str> = line.split(":").collect();
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

fn do_mem_hotplug_by_probe(addrs: &Vec<u64>) -> Result<()> {
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

    let ret =
        unsafe { libc::settimeofday(&tv as *const libc::timeval, 0 as *const libc::timezone) };

    Errno::result(ret).map(drop)?;

    Ok(())
}

fn do_copy_file(req: &CopyFileRequest) -> Result<()> {
    let path = PathBuf::from(req.path.as_str());

    if !path.starts_with(CONTAINER_BASE) {
        return Err(nix::Error::Sys(Errno::EINVAL).into());
    }

    let parent = path.parent();

    let dir = if parent.is_some() {
        parent.unwrap().to_path_buf()
    } else {
        PathBuf::from("/")
    };

    if let Err(e) = fs::create_dir_all(dir.to_str().unwrap()) {
        if e.kind() != std::io::ErrorKind::AlreadyExists {
            return Err(e.into());
        }
    }

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

fn setup_bundle(spec: &Spec) -> Result<PathBuf> {
    if spec.root.is_none() {
        return Err(nix::Error::Sys(Errno::EINVAL).into());
    }
    let root = spec.root.as_ref().unwrap().path.as_str();

    let rootfs = fs::canonicalize(root)?;
    let bundle_path = rootfs.parent().unwrap().to_str().unwrap();

    let config = format!("{}/{}", bundle_path, "config.json");

    info!(
        sl!(),
        "{:?}",
        spec.process.as_ref().unwrap().console_size.as_ref()
    );
    let _ = spec.save(config.as_str());

    let olddir = unistd::getcwd().chain_err(|| "cannot getcwd")?;
    unistd::chdir(bundle_path)?;

    Ok(olddir)
}

fn load_kernel_module(module: &protocols::agent::KernelModule) -> Result<()> {
    if module.name == "" {
        return Err(ErrorKind::ErrorCode("Kernel module name is empty".to_string()).into());
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
            return Err(ErrorKind::ErrorCode(msg).into());
        }
        None => {
            return Err(ErrorKind::ErrorCode("Process terminated by signal".to_string()).into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_kernel_module() {
        let mut m = protocols::agent::KernelModule::default();

        // case 1: module not exists
        m.name = "module_not_exists".to_string();
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
}
