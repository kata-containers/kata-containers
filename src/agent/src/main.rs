// Copyright (c) 2019 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

#![allow(non_camel_case_types)]
#![allow(unused_parens)]
#![allow(unused_unsafe)]
#![allow(dead_code)]
#![allow(non_snake_case)]
#[macro_use]
extern crate lazy_static;
extern crate oci;
extern crate prctl;
extern crate prometheus;
extern crate protocols;
extern crate regex;
extern crate scan_fmt;
extern crate serde_json;
extern crate signal_hook;

#[macro_use]
extern crate scopeguard;

#[macro_use]
extern crate slog;

use anyhow::{anyhow, Context, Result};
use nix::fcntl::{self, OFlag};
use nix::fcntl::{FcntlArg, FdFlag};
use nix::libc::{STDERR_FILENO, STDIN_FILENO, STDOUT_FILENO};
use nix::pty;
use nix::sys::select::{select, FdSet};
use nix::sys::socket::{self, AddressFamily, SockAddr, SockFlag, SockType};
use nix::sys::wait::{self, WaitStatus};
use nix::unistd::{self, close, dup, dup2, fork, setsid, ForkResult};
use prctl::set_child_subreaper;
use std::collections::HashMap;
use std::env;
use std::ffi::{CStr, CString, OsStr};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs as unixfs;
use std::os::unix::io::AsRawFd;
use std::path::Path;
use std::sync::Arc;
use unistd::Pid;

mod config;
mod device;
mod linux_abi;
mod metrics;
mod mount;
mod namespace;
mod netlink;
mod network;
pub mod random;
mod sandbox;
#[cfg(test)]
mod test_utils;
mod uevent;
mod version;

use mount::{cgroups_mount, general_mount};
use sandbox::Sandbox;
use slog::Logger;
use uevent::watch_uevents;

use std::sync::Mutex as SyncMutex;

use futures::StreamExt as _;
use rustjail::pipestream::PipeStream;
use tokio::{
    signal::unix::{signal, SignalKind},
    sync::{oneshot::Sender, Mutex, RwLock},
};
use tokio_vsock::{Incoming, VsockListener, VsockStream};

mod rpc;

const NAME: &str = "kata-agent";
const KERNEL_CMDLINE_FILE: &str = "/proc/cmdline";
const CONSOLE_PATH: &str = "/dev/console";

const DEFAULT_BUF_SIZE: usize = 8 * 1024;

lazy_static! {
    static ref GLOBAL_DEVICE_WATCHER: Arc<Mutex<HashMap<String, Option<Sender<String>>>>> =
        Arc::new(Mutex::new(HashMap::new()));
    static ref AGENT_CONFIG: Arc<RwLock<agentConfig>> =
        Arc::new(RwLock::new(config::agentConfig::new()));
}

fn announce(logger: &Logger, config: &agentConfig) {
    info!(logger, "announce";
    "agent-commit" => version::VERSION_COMMIT,

    // Avoid any possibility of confusion with the old agent
    "agent-type" => "rust",

    "agent-version" =>  version::AGENT_VERSION,
    "api-version" => version::API_VERSION,
    "config" => format!("{:?}", config),
    );
}

fn set_fd_close_exec(fd: RawFd) -> Result<RawFd> {
    if let Err(e) = fcntl::fcntl(fd, FcntlArg::F_SETFD(FdFlag::FD_CLOEXEC)) {
        return Err(anyhow!("failed to set fd: {} as close-on-exec: {}", fd, e));
    }
    Ok(fd)
}

fn get_vsock_incoming(fd: RawFd) -> Incoming {
    let incoming;
    unsafe {
        incoming = VsockListener::from_raw_fd(fd).incoming();
    }
    incoming
}

async fn get_vsock_stream(fd: RawFd) -> Result<VsockStream> {
    let stream = get_vsock_incoming(fd).next().await.unwrap().unwrap();
    set_fd_close_exec(stream.as_raw_fd())?;
    Ok(stream)
}

fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();

    if args.len() == 2 && args[1] == "--version" {
        println!(
            "{} version {} (api version: {}, commit version: {}, type: rust)",
            NAME,
            version::AGENT_VERSION,
            version::API_VERSION,
            version::VERSION_COMMIT,
        );

        exit(0);
    }

    if args.len() == 2 && args[1] == "init" {
        rustjail::container::init_child();
        exit(0);
    }

    let mut rt = tokio::runtime::Builder::new()
        .basic_scheduler()
        .max_threads(1)
        .enable_all()
        .build()?;

    rt.block_on(async {
        env::set_var("RUST_BACKTRACE", "full");

        lazy_static::initialize(&SHELLS);

        lazy_static::initialize(&AGENT_CONFIG);

        // support vsock log
        let (rfd, wfd) = unistd::pipe2(OFlag::O_CLOEXEC)?;

        let agentConfig = AGENT_CONFIG.clone();

        let init_mode = unistd::getpid() == Pid::from_raw(1);
        if init_mode {
            // dup a new file descriptor for this temporary logger writer,
            // since this logger would be dropped and it's writer would
            // be closed out of this code block.
            let newwfd = dup(wfd)?;
            let writer = unsafe { File::from_raw_fd(newwfd) };

            // Init a temporary logger used by init agent as init process
            // since before do the base mount, it wouldn't access "/proc/cmdline"
            // to get the customzied debug level.
            let logger = logging::create_logger(NAME, "agent", slog::Level::Debug, writer);

            // Must mount proc fs before parsing kernel command line
            general_mount(&logger).map_err(|e| {
                error!(logger, "fail general mount: {}", e);
                e
            })?;

            let mut config = agentConfig.write().await;
            config.parse_cmdline(KERNEL_CMDLINE_FILE)?;

            init_agent_as_init(&logger, config.unified_cgroup_hierarchy)?;
        } else {
            // once parsed cmdline and set the config, release the write lock
            // as soon as possible in case other thread would get read lock on
            // it.
            let mut config = agentConfig.write().await;
            config.parse_cmdline(KERNEL_CMDLINE_FILE)?;
        }
        let config = agentConfig.read().await;

        let log_vport = config.log_vport as u32;
        let log_handle = tokio::spawn(async move {
            let mut reader = PipeStream::from_fd(rfd);

            if log_vport > 0 {
                let listenfd = socket::socket(
                    AddressFamily::Vsock,
                    SockType::Stream,
                    SockFlag::SOCK_CLOEXEC,
                    None,
                )
                .unwrap();

                let addr = SockAddr::new_vsock(libc::VMADDR_CID_ANY, log_vport);
                socket::bind(listenfd, &addr).unwrap();
                socket::listen(listenfd, 1).unwrap();

                let mut vsock_stream = get_vsock_stream(listenfd).await.unwrap();

                // copy log to stdout
                tokio::io::copy(&mut reader, &mut vsock_stream)
                    .await
                    .unwrap();
            }

            // copy log to stdout
            let mut stdout_writer = tokio::io::stdout();
            let _ = tokio::io::copy(&mut reader, &mut stdout_writer).await;
        });

        let writer = unsafe { File::from_raw_fd(wfd) };

        // Recreate a logger with the log level get from "/proc/cmdline".
        let logger = logging::create_logger(NAME, "agent", config.log_level, writer);

        announce(&logger, &config);

        // This "unused" variable is required as it enables the global (and crucially static) logger,
        // which is required to satisfy the the lifetime constraints of the auto-generated gRPC code.
        let _guard = slog_scope::set_global_logger(logger.new(o!("subsystem" => "rpc")));

        let mut _log_guard: Result<(), log::SetLoggerError> = Ok(());

        if config.log_level == slog::Level::Trace {
            // Redirect ttrpc log calls to slog iff full debug requested
            _log_guard = Ok(slog_stdlog::init().map_err(|e| e)?);
        }

        start_sandbox(&logger, &config, init_mode).await?;

        let _ = log_handle.await.unwrap();

        Ok(())
    })
}

async fn start_sandbox(logger: &Logger, config: &agentConfig, init_mode: bool) -> Result<()> {
    let shells = SHELLS.clone();
    let debug_console_vport = config.debug_console_vport as u32;

    let shell_handle = if config.debug_console {
        let thread_logger = logger.clone();
        let shells = shells.lock().unwrap().to_vec();

        let handle = tokio::task::spawn_blocking(move || {
            let result = setup_debug_console(&thread_logger, shells, debug_console_vport);
            if result.is_err() {
                // Report error, but don't fail
                warn!(thread_logger, "failed to setup debug console";
                    "error" => format!("{}", result.unwrap_err()));
            }
        });

        Some(handle)
    } else {
        None
    };

    // Initialize unique sandbox structure.
    let s = Sandbox::new(&logger).context("Failed to create sandbox")?;
    if init_mode {
        s.rtnl.handle_localhost().await?;
    }

    let sandbox = Arc::new(Mutex::new(s));

    setup_signal_handler(&logger, sandbox.clone())
        .await
        .unwrap();
    watch_uevents(sandbox.clone()).await;

    let (tx, rx) = tokio::sync::oneshot::channel();
    sandbox.lock().await.sender = Some(tx);

    // vsock:///dev/vsock, port
    let mut server = rpc::start(sandbox.clone(), config.server_addr.as_str());
    server.start().await?;

    let _ = rx.await?;
    server.shutdown().await?;

    if let Some(handle) = shell_handle {
        handle.await.map_err(|e| anyhow!("{:?}", e))?;
    }

    Ok(())
}

use nix::sys::wait::WaitPidFlag;

async fn setup_signal_handler(logger: &Logger, sandbox: Arc<Mutex<Sandbox>>) -> Result<()> {
    let logger = logger.new(o!("subsystem" => "signals"));

    set_child_subreaper(true)
        .map_err(|err| anyhow!(err).context("failed to setup agent as a child subreaper"))?;

    let mut signal_stream = signal(SignalKind::child())?;

    tokio::spawn(async move {
        'outer: loop {
            signal_stream.recv().await;
            info!(logger, "received signal"; "signal" => "SIGCHLD");

            // sevral signals can be combined together
            // as one. So loop around to reap all
            // exited children
            'inner: loop {
                let wait_status = match wait::waitpid(
                    Some(Pid::from_raw(-1)),
                    Some(WaitPidFlag::WNOHANG | WaitPidFlag::__WALL),
                ) {
                    Ok(s) => {
                        if s == WaitStatus::StillAlive {
                            continue 'outer;
                        }
                        s
                    }
                    Err(e) => {
                        info!(
                            logger,
                            "waitpid reaper failed";
                            "error" => e.as_errno().unwrap().desc()
                        );
                        continue 'outer;
                    }
                };
                info!(logger, "wait_status"; "wait_status result" => format!("{:?}", wait_status));

                let pid = wait_status.pid();
                if let Some(pid) = pid {
                    let raw_pid = pid.as_raw();
                    let child_pid = format!("{}", raw_pid);

                    let logger = logger.new(o!("child-pid" => child_pid));

                    let mut sandbox = sandbox.lock().await;
                    let process = sandbox.find_process(raw_pid);
                    if process.is_none() {
                        info!(logger, "child exited unexpectedly");
                        continue 'inner;
                    }

                    let mut p = process.unwrap();

                    if p.exit_pipe_w.is_none() {
                        error!(logger, "the process's exit_pipe_w isn't set");
                        continue 'inner;
                    }
                    let pipe_write = p.exit_pipe_w.unwrap();
                    let ret: i32;

                    match wait_status {
                        WaitStatus::Exited(_, c) => ret = c,
                        WaitStatus::Signaled(_, sig, _) => ret = sig as i32,
                        _ => {
                            info!(logger, "got wrong status for process";
                                  "child-status" => format!("{:?}", wait_status));
                            continue 'inner;
                        }
                    }

                    p.exit_code = ret;
                    let _ = unistd::close(pipe_write);

                    info!(logger, "notify term to close");
                    // close the socket file to notify readStdio to close terminal specifically
                    // in case this process's terminal has been inherited by its children.
                    p.notify_term_close();
                }
            }
        }
    });
    Ok(())
}

// init_agent_as_init will do the initializations such as setting up the rootfs
// when this agent has been run as the init process.
fn init_agent_as_init(logger: &Logger, unified_cgroup_hierarchy: bool) -> Result<()> {
    cgroups_mount(logger, unified_cgroup_hierarchy).map_err(|e| {
        error!(
            logger,
            "fail cgroups mount, unified_cgroup_hierarchy {}: {}", unified_cgroup_hierarchy, e
        );
        e
    })?;

    fs::remove_file(Path::new("/dev/ptmx"))?;
    unixfs::symlink(Path::new("/dev/pts/ptmx"), Path::new("/dev/ptmx"))?;

    unistd::setsid()?;

    unsafe {
        libc::ioctl(std::io::stdin().as_raw_fd(), libc::TIOCSCTTY, 1);
    }

    env::set_var("PATH", "/bin:/sbin/:/usr/bin/:/usr/sbin/");

    let contents =
        std::fs::read_to_string("/etc/hostname").unwrap_or_else(|_| String::from("localhost"));
    let contents_array: Vec<&str> = contents.split(' ').collect();
    let hostname = contents_array[0].trim();

    if sethostname(OsStr::new(hostname)).is_err() {
        warn!(logger, "failed to set hostname");
    }

    Ok(())
}

fn sethostname(hostname: &OsStr) -> Result<()> {
    let size = hostname.len() as usize;

    let result =
        unsafe { libc::sethostname(hostname.as_bytes().as_ptr() as *const libc::c_char, size) };

    if result != 0 {
        Err(anyhow!("failed to set hostname"))
    } else {
        Ok(())
    }
}

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

use crate::config::agentConfig;
use nix::sys::stat::Mode;
use std::os::unix::io::{FromRawFd, RawFd};
use std::path::PathBuf;
use std::process::exit;

fn setup_debug_console(logger: &Logger, shells: Vec<String>, port: u32) -> Result<()> {
    let shell = shells
        .iter()
        .find(|sh| PathBuf::from(sh).exists())
        .ok_or_else(|| anyhow!("no shell found to launch debug console"))?;

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
        loop {
            let f: RawFd = socket::accept4(listenfd, SockFlag::SOCK_CLOEXEC)?;
            match run_debug_console_shell(logger, shell, f) {
                Ok(_) => {
                    info!(logger, "run_debug_console_shell session finished");
                }
                Err(err) => {
                    error!(logger, "run_debug_console_shell failed: {:?}", err);
                }
            }
        }
    } else {
        let mut flags = OFlag::empty();
        flags.insert(OFlag::O_RDWR);
        flags.insert(OFlag::O_CLOEXEC);
        loop {
            let f: RawFd = fcntl::open(CONSOLE_PATH, flags, Mode::empty())?;
            match run_debug_console_shell(logger, shell, f) {
                Ok(_) => {
                    info!(logger, "run_debug_console_shell session finished");
                }
                Err(err) => {
                    error!(logger, "run_debug_console_shell failed: {:?}", err);
                }
            }
        }
    };
}

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

fn run_debug_console_shell(logger: &Logger, shell: &str, socket_fd: RawFd) -> Result<()> {
    let pseduo = pty::openpty(None, None)?;
    let _ = fcntl::fcntl(pseduo.master, FcntlArg::F_SETFD(FdFlag::FD_CLOEXEC));
    let _ = fcntl::fcntl(pseduo.slave, FcntlArg::F_SETFD(FdFlag::FD_CLOEXEC));

    let slave_fd = pseduo.slave;

    match fork() {
        Ok(ForkResult::Child) => {
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
        }

        Ok(ForkResult::Parent { child: child_pid }) => {
            info!(logger, "get debug shell pid {:?}", child_pid);

            let (rfd, wfd) = unistd::pipe2(OFlag::O_CLOEXEC)?;
            let master_fd = pseduo.master;
            let debug_shell_logger = logger.clone();

            // channel that used to sync between thread and main process
            let (tx, rx) = std::sync::mpsc::channel::<i32>();

            // start a thread to do IO copy between socket and pseduo.master
            std::thread::spawn(move || {
                let mut master_reader = unsafe { File::from_raw_fd(master_fd) };
                let mut master_writer = unsafe { File::from_raw_fd(master_fd) };
                let mut socket_reader = unsafe { File::from_raw_fd(socket_fd) };
                let mut socket_writer = unsafe { File::from_raw_fd(socket_fd) };

                loop {
                    let mut fd_set = FdSet::new();
                    fd_set.insert(rfd);
                    fd_set.insert(master_fd);
                    fd_set.insert(socket_fd);

                    match select(
                        Some(fd_set.highest().unwrap() + 1),
                        &mut fd_set,
                        None,
                        None,
                        None,
                    ) {
                        Ok(_) => (),
                        Err(e) => {
                            if e == nix::Error::from(nix::errno::Errno::EINTR) {
                                continue;
                            } else {
                                error!(debug_shell_logger, "select error {:?}", e);
                                tx.send(1).unwrap();
                                break;
                            }
                        }
                    }

                    if fd_set.contains(rfd) {
                        info!(
                            debug_shell_logger,
                            "debug shell process {} exited", child_pid
                        );
                        tx.send(1).unwrap();
                        break;
                    }

                    if fd_set.contains(master_fd) {
                        match io_copy(&mut master_reader, &mut socket_writer) {
                            Ok(0) => {
                                debug!(debug_shell_logger, "master fd closed");
                                tx.send(1).unwrap();
                                break;
                            }
                            Ok(_) => {}
                            Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                            Err(e) => {
                                error!(debug_shell_logger, "read master fd error {:?}", e);
                                tx.send(1).unwrap();
                                break;
                            }
                        }
                    }

                    if fd_set.contains(socket_fd) {
                        match io_copy(&mut socket_reader, &mut master_writer) {
                            Ok(0) => {
                                debug!(debug_shell_logger, "socket fd closed");
                                tx.send(1).unwrap();
                                break;
                            }
                            Ok(_) => {}
                            Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                            Err(e) => {
                                error!(debug_shell_logger, "read socket fd error {:?}", e);
                                tx.send(1).unwrap();
                                break;
                            }
                        }
                    }
                }
            });

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
        }
        Err(err) => {
            return Err(anyhow!("fork error: {:?}", err));
        }
    }

    Ok(())
}

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
