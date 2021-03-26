// Copyright (c) 2019 Ant Financial
// Copyright (c) 2021 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{anyhow, Result};
use lazy_static;
use nix::fcntl::{self, OFlag};
use nix::fcntl::{FcntlArg, FdFlag};
use nix::libc::{STDERR_FILENO, STDIN_FILENO, STDOUT_FILENO};
use nix::pty::{openpty, OpenptyResult};
use nix::sys::socket::{self, AddressFamily, SockAddr, SockFlag, SockType};
use nix::sys::stat::Mode;
use nix::sys::wait;
use nix::unistd::{self, close, dup2, fork, setsid, ForkResult, Pid};
use rustjail::pipestream::PipeStream;
use slog::Logger;
use std::ffi::{CStr, CString};
use std::fs::File;
use std::io::{Read, Write};
use std::os::unix::io::{FromRawFd, RawFd};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex as SyncMutex;
use tokio::sync::watch::Receiver;
use tokio::{pin, select};

const CONSOLE_PATH: &str = "/dev/console";
const DEFAULT_BUF_SIZE: usize = 8 * 1024;

lazy_static! {
    static ref SHELLS: Arc<SyncMutex<Vec<String>>> = {
        let mut v = Vec::new();

        if !cfg!(test) {
            v.push("/bin/bash".to_string());
            v.push("/bin/sh".to_string());
        }

        Arc::new(SyncMutex::new(v))
    };
}

pub fn initialize() {
    lazy_static::initialize(&SHELLS);
}

pub async fn debug_console_handler(
    logger: Logger,
    port: u32,
    mut shutdown: Receiver<bool>,
) -> Result<()> {
    let logger = logger.new(o!("subsystem" => "debug-console"));

    let shells = SHELLS.clone();
    let shells = shells.lock().unwrap().to_vec();

    let shell = shells
        .iter()
        .find(|sh| PathBuf::from(sh).exists())
        .ok_or_else(|| anyhow!("no shell found to launch debug console"))?;

    let fd: RawFd;

    if port > 0 {
        let listenfd = socket::socket(
            AddressFamily::Vsock,
            SockType::Stream,
            SockFlag::SOCK_CLOEXEC,
            None,
        )?;
        let addr = SockAddr::new_vsock(libc::VMADDR_CID_ANY, port);
        socket::bind(listenfd, &addr)?;
        socket::listen(listenfd, 1)?;

        fd = socket::accept4(listenfd, SockFlag::SOCK_CLOEXEC)?;
    } else {
        let mut flags = OFlag::empty();
        flags.insert(OFlag::O_RDWR);
        flags.insert(OFlag::O_CLOEXEC);

        fd = fcntl::open(CONSOLE_PATH, flags, Mode::empty())?;
    };

    loop {
        select! {
            _ = shutdown.changed() => {
                info!(logger, "got shutdown request");
                break;
            }

            // BUG: FIXME: wait on parent.
            //result = run_debug_console_shell(logger.clone(), shell, fd, shutdown.clone()) => {
            //    match result {
            //        Ok(_) => {
            //            info!(logger, "run_debug_console_shell session finished");
            //        }
            //        Err(err) => {
            //            error!(logger, "run_debug_console_shell failed: {:?}", err);
            //        }
            //    }
            //}
        }
    }

    Ok(())
}

fn run_in_child(slave_fd: libc::c_int, shell: &str) -> Result<()> {
    // create new session with child as session leader
    setsid()?;

    // dup stdin, stdout, stderr to let child act as a terminal
    dup2(slave_fd, STDIN_FILENO)?;
    dup2(slave_fd, STDOUT_FILENO)?;
    dup2(slave_fd, STDERR_FILENO)?;

    // set tty
    unsafe {
        libc::ioctl(0, libc::TIOCSCTTY);
    }

    let cmd = CString::new(shell).unwrap();
    let args: Vec<&CStr> = vec![];

    // run shell
    let _ = unistd::execvp(cmd.as_c_str(), args.as_slice()).map_err(|e| match e {
        nix::Error::Sys(errno) => {
            std::process::exit(errno as i32);
        }
        _ => std::process::exit(-2),
    });

    Ok(())
}

async fn run_in_parent(
    logger: Logger,
    mut shutdown: Receiver<bool>,
    socket_fd: RawFd,
    pseudo: OpenptyResult,
    child_pid: Pid,
) -> Result<()> {
    info!(logger, "get debug shell pid {:?}", child_pid);

    let (rfd, wfd) = unistd::pipe2(OFlag::O_CLOEXEC)?;
    let master_fd = pseudo.master;
    let slave_fd = pseudo.slave;
    //let debug_shell_logger = logger.clone();

    let logger = logger.clone();

    // channel that used to sync between thread and main process
    let (tx, rx) = std::sync::mpsc::channel::<i32>();

    // start a thread to do IO copy between socket and pseudo.master
    //tokio::spawn(async move {
    //let logger = logger.clone();
    //let mut shutdown = shutdown.clone();

    //let mut master_reader = unsafe { File::from_raw_fd(master_fd) };
    //let mut master_writer = unsafe { File::from_raw_fd(master_fd) };
    //let mut socket_reader = unsafe { File::from_raw_fd(socket_fd) };
    //let mut socket_writer = unsafe { File::from_raw_fd(socket_fd) };

    let mut pipe_reader = PipeStream::from_fd(rfd);

    //let mut pty_master_reader = PipeStream::from_fd(master_fd);
    //let mut socket_reader = PipeStream::from_fd(socket_fd);

    //pin!(pipe_reader);

    // BUG: FIXME: add blocks for pipe_reader, master_fd and socket_fd
    // (see commented out code below).
    loop {
        select! {
            _ = shutdown.changed() => {
                info!(logger, "got shutdown request");
                break;
            },
            _ = pipe_reader => {
                info!(
                    debug_shell_logger,
                    "debug shell process {} exited", child_pid
                );
                tx.send(1).unwrap();
            },

        }
    }

    //    if fd_set.contains(master_fd) {
    //        match io_copy(&mut master_reader, &mut socket_writer) {
    //            Ok(0) => {
    //                debug!(debug_shell_logger, "master fd closed");
    //                tx.send(1).unwrap();
    //                break;
    //            }
    //            Ok(_) => {}
    //            Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
    //            Err(e) => {
    //                error!(debug_shell_logger, "read master fd error {:?}", e);
    //                tx.send(1).unwrap();
    //                break;
    //            }
    //        }
    //    }

    //    if fd_set.contains(socket_fd) {
    //        match io_copy(&mut socket_reader, &mut master_writer) {
    //            Ok(0) => {
    //                debug!(debug_shell_logger, "socket fd closed");
    //                tx.send(1).unwrap();
    //                break;
    //            }
    //            Ok(_) => {}
    //            Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
    //            Err(e) => {
    //                error!(debug_shell_logger, "read socket fd error {:?}", e);
    //                tx.send(1).unwrap();
    //                break;
    //            }
    //        }
    //    }
    //}
    //})
    //.await;

    let wait_status = wait::waitpid(child_pid, None);
    info!(logger, "debug console process exit code: {:?}", wait_status);

    info!(logger, "notify debug monitor thread to exit");
    // close pipe to exit select loop
    let _ = close(wfd);

    // wait for thread exit.
    let _ = rx.recv().unwrap();
    info!(logger, "debug monitor thread has exited");

    // close files
    let _ = close(rfd);
    let _ = close(master_fd);
    let _ = close(slave_fd);

    Ok(())
}

async fn run_debug_console_shell(
    logger: Logger,
    shell: &str,
    socket_fd: RawFd,
    shutdown: Receiver<bool>,
) -> Result<()> {
    let logger = logger.new(o!("subsystem" => "debug-console-shell"));

    let pseudo = openpty(None, None)?;
    let _ = fcntl::fcntl(pseudo.master, FcntlArg::F_SETFD(FdFlag::FD_CLOEXEC));
    let _ = fcntl::fcntl(pseudo.slave, FcntlArg::F_SETFD(FdFlag::FD_CLOEXEC));

    let slave_fd = pseudo.slave;

    match fork() {
        Ok(ForkResult::Child) => run_in_child(slave_fd, shell),
        Ok(ForkResult::Parent { child: child_pid }) => {
            run_in_parent(
                logger.clone(),
                shutdown.clone(),
                socket_fd,
                pseudo,
                child_pid,
            )
            .await
        }
        Err(err) => Err(anyhow!("fork error: {:?}", err)),
    }
}

// BUG: FIXME:
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_setup_debug_console_no_shells() {
        // Guarantee no shells have been added
        // (required to avoid racing with
        // test_setup_debug_console_invalid_shell()).
        let shells_ref = SHELLS.clone();
        let mut shells = shells_ref.lock().unwrap();
        shells.clear();
        let logger = slog_scope::logger();

        let result = setup_debug_console(&logger, shells.to_vec(), 0);

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "no shell found to launch debug console"
        );
    }

    #[test]
    fn test_setup_debug_console_invalid_shell() {
        let shells_ref = SHELLS.clone();
        let mut shells = shells_ref.lock().unwrap();

        let dir = tempdir().expect("failed to create tmpdir");

        // Add an invalid shell
        let shell = dir
            .path()
            .join("enoent")
            .to_str()
            .expect("failed to construct shell path")
            .to_string();

        shells.push(shell);
        let logger = slog_scope::logger();

        let result = setup_debug_console(&logger, shells.to_vec(), 0);

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "no shell found to launch debug console"
        );
    }
}

// BUG: FIXME: should not be required as we can use the
// interruptable_io_copier(). But if it is still needed, move to utils.rs.
fn io_copy<R: ?Sized, W: ?Sized>(reader: &mut R, writer: &mut W) -> std::io::Result<u64>
where
    R: Read,
    W: Write,
{
    let mut buf = [0; DEFAULT_BUF_SIZE];
    let buf_len;

    match reader.read(&mut buf) {
        Ok(0) => return Ok(0),
        Ok(len) => buf_len = len,
        Err(err) => return Err(err),
    };

    // write and return
    match writer.write_all(&buf[..buf_len]) {
        Ok(_) => Ok(buf_len as u64),
        Err(err) => Err(err),
    }
}
