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

use nix::fcntl::{fcntl, FcntlArg, OFlag};
use nix::sys::signal::{self, Signal};
use nix::sys::socket::{self, AddressFamily, SockFlag, SockType};
use nix::sys::wait::{self, WaitStatus};
use nix::unistd::{self, Pid};
use nix::Result;

use nix::Error;
use oci::Process as OCIProcess;
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
    pub term_master: Option<RawFd>,
    pub tty: bool,
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
    pub fn new(
        logger: &Logger,
        ocip: &OCIProcess,
        id: &str,
        init: bool,
        pipe_size: i32,
    ) -> Result<Self> {
        let logger = logger.new(o!("subsystem" => "process"));

        let mut p = Process {
            exec_id: String::from(id),
            stdin: None,
            stdout: None,
            stderr: None,
            exit_pipe_w: None,
            exit_pipe_r: None,
            extra_files: Vec::new(),
            tty: ocip.terminal,
            term_master: None,
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

        if !p.tty {
            info!(logger, "created console socket!");

            let (stdin, pstdin) = unistd::pipe2(OFlag::O_CLOEXEC)?;
            p.parent_stdin = Some(pstdin);
            p.stdin = Some(stdin);

            let (pstdout, stdout) = create_extended_pipe(OFlag::O_CLOEXEC, pipe_size)?;
            p.parent_stdout = Some(pstdout);
            p.stdout = Some(stdout);

            let (pstderr, stderr) = create_extended_pipe(OFlag::O_CLOEXEC, pipe_size)?;
            p.parent_stderr = Some(pstderr);
            p.stderr = Some(stderr);
        }
        Ok(p)
    }
}

fn create_extended_pipe(flags: OFlag, pipe_size: i32) -> Result<(RawFd, RawFd)> {
    let (r, w) = unistd::pipe2(flags)?;
    if pipe_size > 0 {
        fcntl(w, FcntlArg::F_SETPIPE_SZ(pipe_size))?;
    }
    Ok((r, w))
}

#[cfg(test)]
mod tests {
    use crate::process::create_extended_pipe;
    use nix::fcntl::{fcntl, FcntlArg, OFlag};
    use std::fs;
    use std::os::unix::io::RawFd;

    fn get_pipe_max_size() -> i32 {
        fs::read_to_string("/proc/sys/fs/pipe-max-size")
            .unwrap()
            .trim()
            .parse::<i32>()
            .unwrap()
    }

    fn get_pipe_size(fd: RawFd) -> i32 {
        fcntl(fd, FcntlArg::F_GETPIPE_SZ).unwrap()
    }

    #[test]
    fn test_create_extended_pipe() {
        // Test the default
        let (r, w) = create_extended_pipe(OFlag::O_CLOEXEC, 0).unwrap();

        // Test setting to the max size
        let max_size = get_pipe_max_size();
        let (r, w) = create_extended_pipe(OFlag::O_CLOEXEC, max_size).unwrap();
        let actual_size = get_pipe_size(w);
        assert_eq!(max_size, actual_size);
    }
}
