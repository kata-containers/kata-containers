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
use std::ffi::CString;
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

// A configured debug_console_shell is honored only if it resolves under this
// prefix (a guest extension mount). Extension images are dm-verity measured and
// read-only, so this confines the debug console to trusted binaries: a host that
// can influence the agent cmdline (untrusted under CoCo) cannot repoint it at a
// container rootfs to run tenant binaries or read guest-root data.
//
// Only referenced under `#[cfg(not(test))]`, so it reads as dead code in tests.
#[cfg_attr(test, allow(dead_code))]
const DEBUG_CONSOLE_SHELL_ALLOWED_PREFIX: &str = "/run/kata-extensions/";

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

// Honor `shell` only if it canonicalizes (resolving symlinks and `..`) to a path
// under `prefix`. Canonicalization is what makes this safe: a raw prefix check
// could be bypassed by a symlink or `..` escape. The original path is returned so
// an entrypoint symlink under the prefix (devkit-sh -> devkit-enter) stays the
// exec target, since it is equally trusted.
fn shell_under_prefix(shell: &str, prefix: &str) -> Option<String> {
    if shell.is_empty() {
        return None;
    }

    let canonical = std::fs::canonicalize(shell).ok()?;
    canonical.starts_with(prefix).then(|| shell.to_string())
}

pub async fn debug_console_handler(
    logger: Logger,
    port: u32,
    mut shutdown: Receiver<bool>,
) -> Result<()> {
    let logger = logger.new(o!("subsystem" => "debug-console"));

    // Only mutated under `#[cfg(not(test))]`, so it need not be mut in tests.
    #[cfg_attr(test, allow(unused_mut))]
    let mut shells = SHELLS.lock().unwrap().to_vec();

    // Prefer a configured debug_console_shell over the built-ins, but only if it
    // passes the extension-prefix guard.
    #[cfg(not(test))]
    {
        let configured = crate::AGENT_CONFIG.debug_console_shell.clone();
        match shell_under_prefix(&configured, DEBUG_CONSOLE_SHELL_ALLOWED_PREFIX) {
            Some(sh) => shells.insert(0, sh),
            None if !configured.is_empty() => warn!(
                logger,
                "ignoring debug_console_shell outside {}: {}",
                DEBUG_CONSOLE_SHELL_ALLOWED_PREFIX,
                configured
            ),
            None => {}
        }
    }

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

fn run_in_child(slave_fd: libc::c_int, shell: String) -> Result<()> {
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

    let cmd = CString::new(shell).unwrap();
    let args: Vec<CString> = Vec::new();

    // run shell
    let _ = unistd::execvp(cmd.as_c_str(), &args).map_err(|e| match e {
        nix::Error::UnknownErrno => std::process::exit(-2),
        _ => std::process::exit(e as i32),
    });

    Ok(())
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
            // the shell run in child may not be exited, in some scenes
            // eg. directly Ctrl-C in the host to terminate the kata-runtime process
            // that will block this task，while waiting for the child to exit.
            //
            let _ = signal::kill(child_pid, Some(signal::Signal::SIGKILL))
                .map_err(|e| warn!(logger, "kill child shell process {:?}", e));

            info!(
                logger,
                "socket closed: {:?}", res
            );
        }
    }

    let wait_status = wait::waitpid(child_pid, None);
    info!(logger, "debug console process exit code: {:?}", wait_status);

    Ok(())
}

async fn run_debug_console_vsock<T: AsyncRead + AsyncWrite>(
    logger: Logger,
    shell: String,
    stream: T,
) -> Result<()> {
    let logger = logger.new(o!("subsystem" => "debug-console-shell"));

    let pseudo = openpty(None, None)?;
    let slave_fd = pseudo.slave.as_fd().as_raw_fd();

    let _ = fcntl::fcntl(pseudo.master.as_fd(), FcntlArg::F_SETFD(FdFlag::FD_CLOEXEC));
    let _ = fcntl::fcntl(pseudo.slave.as_fd(), FcntlArg::F_SETFD(FdFlag::FD_CLOEXEC));

    match unsafe { fork() } {
        Ok(ForkResult::Child) => run_in_child(slave_fd, shell),
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

    #[test]
    fn test_shell_under_prefix() {
        use std::os::unix::fs::symlink;

        let dir = tempdir().expect("failed to create tmpdir");
        let root = dir.path();

        // Simulate an extension mount ("allowed") and a container rootfs
        // ("other") side by side under the same tmpdir.
        let allowed = root.join("allowed");
        let other = root.join("other");
        std::fs::create_dir_all(allowed.join("bin")).unwrap();
        std::fs::create_dir_all(&other).unwrap();

        let prefix = allowed.to_str().unwrap();

        // Inside the prefix: honored.
        let inside = allowed.join("bin").join("devkit-enter");
        std::fs::write(&inside, b"#!/bin/sh\n").unwrap();
        let inside = inside.to_str().unwrap();
        assert_eq!(shell_under_prefix(inside, prefix), Some(inside.to_string()));

        // A symlink that lives inside the prefix but targets another file inside
        // the prefix is honored, and the (symlink) path is what gets returned.
        let link = allowed.join("bin").join("devkit-sh");
        symlink("devkit-enter", &link).unwrap();
        let link = link.to_str().unwrap();
        assert_eq!(shell_under_prefix(link, prefix), Some(link.to_string()));

        // Outside the prefix (e.g. a container binary): rejected.
        let outside = other.join("sh");
        std::fs::write(&outside, b"#!/bin/sh\n").unwrap();
        assert_eq!(shell_under_prefix(outside.to_str().unwrap(), prefix), None);

        // A symlink under the prefix that escapes it via its target: rejected,
        // because canonicalization resolves it outside the prefix.
        let escape = allowed.join("escape");
        symlink(&outside, &escape).unwrap();
        assert_eq!(shell_under_prefix(escape.to_str().unwrap(), prefix), None);

        // A `..` traversal that escapes the prefix: rejected.
        let traversal = allowed.join("bin").join("..").join("..").join("other");
        let traversal = traversal.join("sh");
        assert_eq!(
            shell_under_prefix(traversal.to_str().unwrap(), prefix),
            None
        );

        // Empty and non-existent paths: rejected (no shell configured / bad path).
        assert_eq!(shell_under_prefix("", prefix), None);
        assert_eq!(
            shell_under_prefix(allowed.join("bin").join("enoent").to_str().unwrap(), prefix),
            None
        );
    }
}
