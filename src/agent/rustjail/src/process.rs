// Copyright (c) 2019 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

// use std::process::{Stdio, Command, ExitStatus};
use libc::pid_t;
use std::fs::File;
use std::os::unix::io::RawFd;

// use crate::configs::{Capabilities, Rlimit};
// use crate::cgroups::Manager as CgroupManager;
// use crate::intelrdt::Manager as RdtManager;

use nix::fcntl::OFlag;
use nix::sys::signal::{self, Signal};
use nix::sys::socket::{self, AddressFamily, SockFlag, SockType};
use nix::sys::wait::{self, WaitStatus};
use nix::unistd::{self, Pid};
use nix::Result;

use nix::Error;
use protocols::oci::Process as OCIProcess;
use slog::Logger;

#[derive(Debug)]
pub struct Process {
    pub exec_id: String,
    pub stdin: Option<RawFd>,
    pub stdout: Option<RawFd>,
    pub stderr: Option<RawFd>,
    pub exit_pipe_r: Option<RawFd>,
    pub exit_pipe_w: Option<RawFd>,
    pub extra_files: Vec<File>,
    //	pub caps: Capabilities,
    //	pub rlimits: Vec<Rlimit>,
    pub console_socket: Option<RawFd>,
    pub term_master: Option<RawFd>,
    // parent end of fds
    pub parent_console_socket: Option<RawFd>,
    pub parent_stdin: Option<RawFd>,
    pub parent_stdout: Option<RawFd>,
    pub parent_stderr: Option<RawFd>,
    pub init: bool,
    // pid of the init/exec process. since we have no command
    // struct to store pid, we must store pid here.
    pub pid: pid_t,

    pub exit_code: i32,
    pub oci: OCIProcess,
    pub logger: Logger,
}

pub trait ProcessOperations {
    fn pid(&self) -> Pid;
    fn wait(&self) -> Result<WaitStatus>;
    fn signal(&self, sig: Signal) -> Result<()>;
}

impl ProcessOperations for Process {
    fn pid(&self) -> Pid {
        Pid::from_raw(self.pid)
    }

    fn wait(&self) -> Result<WaitStatus> {
        wait::waitpid(Some(self.pid()), None)
    }

    fn signal(&self, sig: Signal) -> Result<()> {
        signal::kill(self.pid(), Some(sig))
    }
}

impl Process {
    pub fn new(logger: &Logger, ocip: &OCIProcess, id: &str, init: bool) -> Result<Self> {
        let logger = logger.new(o!("subsystem" => "process"));

        let mut p = Process {
            exec_id: String::from(id),
            stdin: None,
            stdout: None,
            stderr: None,
            exit_pipe_w: None,
            exit_pipe_r: None,
            extra_files: Vec::new(),
            console_socket: None,
            term_master: None,
            parent_console_socket: None,
            parent_stdin: None,
            parent_stdout: None,
            parent_stderr: None,
            init,
            pid: -1,
            exit_code: 0,
            oci: ocip.clone(),
            logger: logger.clone(),
        };

        info!(logger, "before create console socket!");

        if ocip.Terminal {
            let (psocket, csocket) = match socket::socketpair(
                AddressFamily::Unix,
                SockType::Stream,
                None,
                SockFlag::SOCK_CLOEXEC,
            ) {
                Ok((u, v)) => (u, v),
                Err(e) => {
                    match e {
                        Error::Sys(errno) => {
                            info!(logger, "socketpair: {}", errno.desc());
                        }
                        _ => {
                            info!(logger, "socketpair: other error!");
                        }
                    }
                    return Err(e);
                }
            };
            p.parent_console_socket = Some(psocket);
            p.console_socket = Some(csocket);
        }

        info!(logger, "created console socket!");

        let (stdin, pstdin) = unistd::pipe2(OFlag::O_CLOEXEC)?;
        p.parent_stdin = Some(pstdin);
        p.stdin = Some(stdin);

        let (pstdout, stdout) = unistd::pipe2(OFlag::O_CLOEXEC)?;
        p.parent_stdout = Some(pstdout);
        p.stdout = Some(stdout);

        let (pstderr, stderr) = unistd::pipe2(OFlag::O_CLOEXEC)?;
        p.parent_stderr = Some(pstderr);
        p.stderr = Some(stderr);

        Ok(p)
    }
}
