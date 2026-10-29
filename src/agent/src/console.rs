// Copyright (c) 2021 Ant Group
// Copyright (c) 2021 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::util;
use anyhow::{anyhow, Result};
use nix::fcntl::{self, FcntlArg, FdFlag, OFlag};
use nix::libc::{STDERR_FILENO, STDIN_FILENO, STDOUT_FILENO};
use nix::pty::{openpty, OpenptyResult};
use nix::sys::socket::{self, AddressFamily, SockFlag, SockType, VsockAddr};
use nix::sys::stat::Mode;
use nix::sys::{signal, wait};
use nix::unistd::{self, close, dup2, fork, setsid, ForkResult, Pid};
use rustjail::pipestream::PipeStream;
use slog::Logger;
use std::ffi::{CStr, CString};
use std::os::unix::io::{AsFd, AsRawFd, BorrowedFd, FromRawFd, IntoRawFd, OwnedFd, RawFd};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::Mutex as SyncMutex;

use futures::StreamExt;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::select;
use tokio::sync::watch::Receiver;

const CONSOLE_PATH: &str = "/dev/console";

// Shell shipped by the devkit debug guest extension. A fixed path under
// /run/kata-extensions, where extension images are mounted read-only and
// dm-verity measured, so nothing outside the agent selects what the console
// execs. It only exists when the extension is cold-plugged, which is itself the
// operator opting in to debugging this sandbox.
const DEVKIT_DEBUG_CONSOLE_SHELL: &str = "/run/kata-extensions/devkit/bin/devkit-sh";

lazy_static! {
    static ref SHELLS: Arc<SyncMutex<Vec<String>>> = {
        let mut v = Vec::new();

        if !cfg!(test) {
            // Ahead of the built-ins: some bases are shell-less, and where a
            // shell does exist the devkit one is the richer choice.
            v.push(DEVKIT_DEBUG_CONSOLE_SHELL.to_string());
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

    let shells = SHELLS.lock().unwrap().to_vec();

    let shell = shells
        .into_iter()
        .find(|sh| PathBuf::from(sh).exists())
        .ok_or_else(|| anyhow!("no shell found to launch debug console"))?;

    if port > 0 {
        let listenfd = socket::socket(
            AddressFamily::Vsock,
            SockType::Stream,
            SockFlag::SOCK_CLOEXEC,
            None,
        )?;
        let addr = VsockAddr::new(libc::VMADDR_CID_ANY, port);
        socket::bind(listenfd.as_raw_fd(), &addr)?;
        socket::listen(&listenfd, nix::sys::socket::Backlog::new(1).unwrap())?;

        let mut incoming = util::get_vsock_incoming(listenfd.into_raw_fd());

        loop {
            select! {
                _ = shutdown.changed() => {
                    info!(logger, "debug console got shutdown request");
                    break;
                }

                conn = incoming.next() => {
                    if let Some(conn) = conn {
                        // Accept a new connection
                        match conn {
                            Ok(stream) => {
                                let logger = logger.clone();
                                let shell = shell.clone();
                                // Do not block(await) here, or we'll never receive the shutdown signal
                                tokio::spawn(async move {
                                    let _ = run_debug_console_vsock(logger, shell, stream).await;
                                });
                            }
                            Err(e) => {
                                error!(logger, "{:?}", e);
                            }
                        }
                    } else {
                        break;
                    }
                }
            }
        }
    } else {
        let mut flags = OFlag::empty();
        flags.insert(OFlag::O_RDWR);
        flags.insert(OFlag::O_CLOEXEC);

        let fd = fcntl::open(CONSOLE_PATH, flags, Mode::empty())?;

        select! {
            _ = shutdown.changed() => {
                info!(logger, "debug console got shutdown request");
            }

            result = run_debug_console_serial(shell.clone(), fd.into_raw_fd()) => {
               match result {
                   Ok(_) => {
                       info!(logger, "run_debug_console_shell session finished");
                   }
                   Err(err) => {
                       error!(logger, "run_debug_console_shell failed: {:?}", err);
                   }
               }
            }
        }
    };

    Ok(())
}

// Runs between fork and exec in a copy of a multi-threaded process, so it must
// not allocate, nor return into the runtime that copy inherited.
fn run_in_child(slave_fd: libc::c_int, shell: &CStr) -> Result<()> {
    // create new session with child as session leader
    setsid()?;

    // dup stdin, stdout, stderr to let child act as a terminal
    let borrowed_fd = unsafe { BorrowedFd::borrow_raw(slave_fd) };
    let mut stdin_fd = unsafe { OwnedFd::from_raw_fd(STDIN_FILENO) };
    let mut stdout_fd = unsafe { OwnedFd::from_raw_fd(STDOUT_FILENO) };
    let mut stderr_fd = unsafe { OwnedFd::from_raw_fd(STDERR_FILENO) };
    dup2(borrowed_fd, &mut stdin_fd)?;
    dup2(borrowed_fd, &mut stdout_fd)?;
    dup2(borrowed_fd, &mut stderr_fd)?;
    // Prevent closing of stdio fds
    let _ = stdin_fd.into_raw_fd();
    let _ = stdout_fd.into_raw_fd();
    let _ = stderr_fd.into_raw_fd();

    // set tty
    unsafe {
        libc::ioctl(0, libc::TIOCSCTTY);
    }

    let args: [&CStr; 0] = [];

    // run shell
    let e = match unistd::execvp(shell, &args) {
        Err(nix::Error::UnknownErrno) => -2,
        Err(e) => e as i32,
        Ok(never) => match never {},
    };

    unsafe { libc::_exit(e) }
}

async fn run_in_parent<T: AsyncRead + AsyncWrite>(
    logger: Logger,
    stream: T,
    pseudo: OpenptyResult,
    child_pid: Pid,
) -> Result<()> {
    info!(logger, "get debug shell pid {:?}", child_pid);

    let master_fd = pseudo.master;
    let _ = close(pseudo.slave.into_raw_fd());

    let (mut socket_reader, mut socket_writer) = tokio::io::split(stream);
    let (mut master_reader, mut master_writer) =
        tokio::io::split(PipeStream::from_fd(master_fd.into_raw_fd()));

    select! {
        res = tokio::io::copy(&mut master_reader, &mut socket_writer) => {
            debug!(
                logger,
                "master closed: {:?}", res
            );
        }
        res = tokio::io::copy(&mut socket_reader, &mut master_writer) => {
            info!(
                logger,
                "socket closed: {:?}", res
            );
        }
    }

    // Either way the session is over, but the shell may well still be running:
    // the master side also "closes" when relaying its output fails because the
    // client went away, and the input the client sent last (say, "exit") can
    // be left unread on the socket when that branch wins.
    if let Err(e) = signal::kill(child_pid, Some(signal::Signal::SIGKILL)) {
        if e != nix::errno::Errno::ESRCH {
            warn!(logger, "kill child shell process {:?}", e);
        }
    }

    // waitpid blocks, and a runtime worker stuck in it stalls every task that
    // worker serves - on a single vCPU guest, the whole agent.
    let wait_status = tokio::task::spawn_blocking(move || wait::waitpid(child_pid, None)).await;
    info!(logger, "debug console process exit code: {:?}", wait_status);

    Ok(())
}

async fn run_debug_console_vsock<T: AsyncRead + AsyncWrite>(
    logger: Logger,
    shell: String,
    stream: T,
) -> Result<()> {
    let logger = logger.new(o!("subsystem" => "debug-console-shell"));

    let shell = CString::new(shell)?;
    let pseudo = openpty(None, None)?;
    let slave_fd = pseudo.slave.as_fd().as_raw_fd();

    let _ = fcntl::fcntl(pseudo.master.as_fd(), FcntlArg::F_SETFD(FdFlag::FD_CLOEXEC));
    let _ = fcntl::fcntl(pseudo.slave.as_fd(), FcntlArg::F_SETFD(FdFlag::FD_CLOEXEC));

    match unsafe { fork() } {
        Ok(ForkResult::Child) => {
            let _ = run_in_child(slave_fd, &shell);
            unsafe { libc::_exit(127) }
        }
        Ok(ForkResult::Parent { child: child_pid }) => {
            run_in_parent(logger.clone(), stream, pseudo, child_pid).await
        }
        Err(err) => Err(anyhow!("fork error: {:?}", err)),
    }
}

async fn run_debug_console_serial(shell: String, fd: RawFd) -> Result<()> {
    let mut child = match tokio::process::Command::new(shell)
        .arg("-i")
        .kill_on_drop(true)
        .stdin(unsafe { Stdio::from_raw_fd(fd) })
        .stdout(unsafe { Stdio::from_raw_fd(fd) })
        .stderr(unsafe { Stdio::from_raw_fd(fd) })
        .spawn()
    {
        Ok(c) => c,
        Err(_) => return Err(anyhow!("failed to spawn shell")),
    };

    child.wait().await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use tokio::sync::watch;

    /// A client that stops reading ends the session through the master side,
    /// with the shell still alive. That must neither leave the shell behind
    /// nor block the runtime waiting for it, which on a single worker stalls
    /// the whole agent.
    #[test]
    fn test_session_ends_when_the_client_stops_reading() {
        use std::os::unix::process::CommandExt;
        use std::process::Command;
        use std::sync::mpsc;
        use std::time::Duration;

        let pseudo = openpty(None, None).unwrap();
        let slave = pseudo.slave.try_clone().unwrap();
        let mut shell = unsafe {
            Command::new("sh")
                .args(["-c", "echo ready; exec sleep 600"])
                .stdin(Stdio::from(slave.try_clone().unwrap()))
                .stdout(Stdio::from(slave.try_clone().unwrap()))
                .stderr(Stdio::from(slave))
                .pre_exec(|| {
                    setsid()?;
                    libc::ioctl(0, libc::TIOCSCTTY, 0);
                    Ok(())
                })
                .spawn()
                .unwrap()
        };
        let child_pid = Pid::from_raw(shell.id() as i32);

        let (ours, theirs) = std::os::unix::net::UnixStream::pair().unwrap();
        theirs.shutdown(std::net::Shutdown::Read).unwrap();
        ours.set_nonblocking(true).unwrap();

        let (done_tx, done_rx) = mpsc::channel();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            let res = rt.block_on(async {
                let stream = tokio::net::UnixStream::from_std(ours).unwrap();
                run_in_parent(slog_scope::logger(), stream, pseudo, child_pid).await
            });
            let _ = done_tx.send(res.is_ok());
        });

        let finished = done_rx.recv_timeout(Duration::from_secs(30));
        let reaped = signal::kill(child_pid, None).is_err();
        if !reaped {
            let _ = shell.kill();
            let _ = shell.wait();
        }
        drop(theirs);

        assert_eq!(finished, Ok(true), "the debug console session never ended");
        assert!(reaped, "the debug console shell outlived its session");
    }

    #[tokio::test]
    async fn test_setup_debug_console_no_shells() {
        {
            // Guarantee no shells have been added
            // (required to avoid racing with
            // test_setup_debug_console_invalid_shell()).
            let shells_ref = SHELLS.clone();
            let mut shells = shells_ref.lock().unwrap();
            shells.clear();
        }

        let logger = slog_scope::logger();

        let (_, rx) = watch::channel(true);
        let result = debug_console_handler(logger, 0, rx).await;

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "no shell found to launch debug console"
        );
    }

    #[tokio::test]
    async fn test_setup_debug_console_invalid_shell() {
        {
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
        }

        let logger = slog_scope::logger();

        let (_, rx) = watch::channel(true);
        let result = debug_console_handler(logger, 0, rx).await;

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "no shell found to launch debug console"
        );
    }
}
