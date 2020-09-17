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
extern crate rustjail;
extern crate scan_fmt;
extern crate serde_json;
extern crate signal_hook;

#[macro_use]
extern crate scopeguard;

#[macro_use]
extern crate slog;
#[macro_use]
extern crate netlink;

use crate::netlink::{RtnlHandle, NETLINK_ROUTE};
use anyhow::{anyhow, Context, Result};
use nix::fcntl::{self, OFlag};
use nix::sys::socket::{self, AddressFamily, SockAddr, SockFlag, SockType};
use nix::sys::wait::{self, WaitStatus};
use nix::unistd;
use nix::unistd::dup;
use prctl::set_child_subreaper;
use signal_hook::{iterator::Signals, SIGCHLD};
use std::collections::HashMap;
use std::env;
use std::ffi::OsStr;
use std::fs::{self, File};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs as unixfs;
use std::os::unix::io::AsRawFd;
use std::path::Path;
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex, RwLock};
use std::{io, thread, thread::JoinHandle};
use unistd::Pid;

mod config;
mod device;
mod generated;
mod linux_abi;
mod metrics;
mod mount;
mod namespace;
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

mod rpc;

const NAME: &str = "kata-agent";
const KERNEL_CMDLINE_FILE: &str = "/proc/cmdline";
const CONSOLE_PATH: &str = "/dev/console";

lazy_static! {
    static ref GLOBAL_DEVICE_WATCHER: Arc<Mutex<HashMap<String, Sender<String>>>> =
        Arc::new(Mutex::new(HashMap::new()));
    static ref AGENT_CONFIG: Arc<RwLock<agentConfig>> =
        Arc::new(RwLock::new(config::agentConfig::new()));
}

fn announce(logger: &Logger, config: &agentConfig) {
    info!(logger, "announce";
    "agent-commit" => generated::VERSION_COMMIT,

    // Avoid any possibility of confusion with the old agent
    "agent-type" => "rust",

    "agent-version" =>  version::AGENT_VERSION,
    "api-version" => version::API_VERSION,
    "config" => format!("{:?}", config),
    );
}

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    if args.len() == 2 && args[1] == "--version" {
        println!(
            "{} version {} (api version: {}, commit version: {}, type: rust)",
            NAME,
            version::AGENT_VERSION,
            version::API_VERSION,
            generated::VERSION_COMMIT,
        );

        exit(0);
    }

    if args.len() == 2 && args[1] == "init" {
        rustjail::container::init_child();
        exit(0);
    }

    env::set_var("RUST_BACKTRACE", "full");

    lazy_static::initialize(&SHELLS);

    lazy_static::initialize(&AGENT_CONFIG);

    // support vsock log
    let (rfd, wfd) = unistd::pipe2(OFlag::O_CLOEXEC)?;
    let writer = unsafe { File::from_raw_fd(wfd) };

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
        init_agent_as_init(&logger)?;
    }

    // once parsed cmdline and set the config, release the write lock
    // as soon as possible in case other thread would get read lock on
    // it.
    {
        let mut config = agentConfig.write().unwrap();
        config.parse_cmdline(KERNEL_CMDLINE_FILE)?;
    }

    let config = agentConfig.read().unwrap();
    let log_vport = config.log_vport as u32;
    let log_handle = thread::spawn(move || -> Result<()> {
        let mut reader = unsafe { File::from_raw_fd(rfd) };
        if log_vport > 0 {
            let listenfd = socket::socket(
                AddressFamily::Vsock,
                SockType::Stream,
                SockFlag::SOCK_CLOEXEC,
                None,
            )?;
            let addr = SockAddr::new_vsock(libc::VMADDR_CID_ANY, log_vport);
            socket::bind(listenfd, &addr)?;
            socket::listen(listenfd, 1)?;
            let datafd = socket::accept4(listenfd, SockFlag::SOCK_CLOEXEC)?;
            let mut log_writer = unsafe { File::from_raw_fd(datafd) };
            let _ = io::copy(&mut reader, &mut log_writer)?;
            let _ = unistd::close(listenfd);
            let _ = unistd::close(datafd);
        }
        // copy log to stdout
        let mut stdout_writer = io::stdout();
        let _ = io::copy(&mut reader, &mut stdout_writer)?;
        Ok(())
    });

    let writer = unsafe { File::from_raw_fd(wfd) };
    // Recreate a logger with the log level get from "/proc/cmdline".
    let logger = logging::create_logger(NAME, "agent", config.log_level, writer);

    announce(&logger, &config);

    // This "unused" variable is required as it enables the global (and crucially static) logger,
    // which is required to satisfy the the lifetime constraints of the auto-generated gRPC code.
    let _guard = slog_scope::set_global_logger(logger.new(o!("subsystem" => "rpc")));

    start_sandbox(&logger, &config, init_mode)?;

    let _ = log_handle.join();

    Ok(())
}

fn start_sandbox(logger: &Logger, config: &agentConfig, init_mode: bool) -> Result<()> {
    let shells = SHELLS.clone();
    let debug_console_vport = config.debug_console_vport as u32;

    let mut shell_handle: Option<JoinHandle<()>> = None;
    if config.debug_console {
        let thread_logger = logger.clone();

        let builder = thread::Builder::new();

        let handle = builder.spawn(move || {
            let shells = shells.lock().unwrap();
            let result = setup_debug_console(shells.to_vec(), debug_console_vport);
            if result.is_err() {
                // Report error, but don't fail
                warn!(thread_logger, "failed to setup debug console";
                    "error" => format!("{}", result.unwrap_err()));
            }
        })?;

        shell_handle = Some(handle);
    }

    // Initialize unique sandbox structure.
    let mut s = Sandbox::new(&logger).context("Failed to create sandbox")?;

    if init_mode {
        let mut rtnl = RtnlHandle::new(NETLINK_ROUTE, 0).unwrap();
        rtnl.handle_localhost()?;

        s.rtnl = Some(rtnl);
    }

    let sandbox = Arc::new(Mutex::new(s));

    setup_signal_handler(&logger, sandbox.clone()).unwrap();
    watch_uevents(sandbox.clone());

    let (tx, rx) = mpsc::channel::<i32>();
    sandbox.lock().unwrap().sender = Some(tx);

    //vsock:///dev/vsock, port
    let mut server = rpc::start(sandbox.clone(), config.server_addr.as_str());

    let _ = server.start().unwrap();

    let _ = rx.recv()?;

    server.shutdown();

    if let Some(handle) = shell_handle {
        handle.join().map_err(|e| anyhow!("{:?}", e))?;
    }

    Ok(())
}

use nix::sys::wait::WaitPidFlag;

fn setup_signal_handler(logger: &Logger, sandbox: Arc<Mutex<Sandbox>>) -> Result<()> {
    let logger = logger.new(o!("subsystem" => "signals"));

    set_child_subreaper(true)
        .map_err(|err| anyhow!(err).context("failed to setup agent as a child subreaper"))?;

    let signals = Signals::new(&[SIGCHLD])?;

    let s = sandbox.clone();

    thread::spawn(move || {
        'outer: for sig in signals.forever() {
            info!(logger, "received signal"; "signal" => sig);

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

                let pid = wait_status.pid();
                if pid.is_some() {
                    let raw_pid = pid.unwrap().as_raw();
                    let child_pid = format!("{}", raw_pid);

                    let logger = logger.new(o!("child-pid" => child_pid));

                    let mut sandbox = s.lock().unwrap();
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
                }
            }
        }
    });
    Ok(())
}

// init_agent_as_init will do the initializations such as setting up the rootfs
// when this agent has been run as the init process.
fn init_agent_as_init(logger: &Logger) -> Result<()> {
    general_mount(logger)?;
    cgroups_mount(logger)?;

    fs::remove_file(Path::new("/dev/ptmx"))?;
    unixfs::symlink(Path::new("/dev/pts/ptmx"), Path::new("/dev/ptmx"))?;

    unistd::setsid()?;

    unsafe {
        libc::ioctl(io::stdin().as_raw_fd(), libc::TIOCSCTTY, 1);
    }

    env::set_var("PATH", "/bin:/sbin/:/usr/bin/:/usr/sbin/");

    let contents = std::fs::read_to_string("/etc/hostname").unwrap_or(String::from("localhost"));
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
    static ref SHELLS: Arc<Mutex<Vec<String>>> = {
        let mut v = Vec::new();

        if !cfg!(test) {
            v.push("/bin/bash".to_string());
            v.push("/bin/sh".to_string());
        }

        Arc::new(Mutex::new(v))
    };
}

// pub static mut LOG_LEVEL: ;
// pub static mut TRACE_MODE: ;

use crate::config::agentConfig;
use nix::sys::stat::Mode;
use std::os::unix::io::{FromRawFd, RawFd};
use std::path::PathBuf;
use std::process::{exit, Command, Stdio};

fn setup_debug_console(shells: Vec<String>, port: u32) -> Result<()> {
    let mut shell: &str = "";
    for sh in shells.iter() {
        let binary = PathBuf::from(sh);
        if binary.exists() {
            shell = sh;
            break;
        }
    }

    if shell == "" {
        return Err(anyhow!("no shell found to launch debug console"));
    }

    let f: RawFd = if port > 0 {
        let listenfd = socket::socket(
            AddressFamily::Vsock,
            SockType::Stream,
            SockFlag::SOCK_CLOEXEC,
            None,
        )?;
        let addr = SockAddr::new_vsock(libc::VMADDR_CID_ANY, port);
        socket::bind(listenfd, &addr)?;
        socket::listen(listenfd, 1)?;
        socket::accept4(listenfd, SockFlag::SOCK_CLOEXEC)?
    } else {
        let mut flags = OFlag::empty();
        flags.insert(OFlag::O_RDWR);
        flags.insert(OFlag::O_CLOEXEC);
        fcntl::open(CONSOLE_PATH, flags, Mode::empty())?
    };

    let cmd = Command::new(shell)
        .arg("-i")
        .stdin(unsafe { Stdio::from_raw_fd(f) })
        .stdout(unsafe { Stdio::from_raw_fd(f) })
        .stderr(unsafe { Stdio::from_raw_fd(f) })
        .spawn();

    let mut cmd = match cmd {
        Ok(c) => c,
        Err(_) => return Err(anyhow!("failed to spawn shell")),
    };

    cmd.wait()?;

    return Ok(());
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_setup_debug_console_no_shells() {
        // Guarantee no shells have been added
        // (required to avoid racing with
        // test_setup_debug_console_invalid_shell()).
        let shells_ref = SHELLS.clone();
        let mut shells = shells_ref.lock().unwrap();
        shells.clear();

        let result = setup_debug_console(shells.to_vec(), 0);

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

        let result = setup_debug_console(shells.to_vec(), 0);

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "no shell found to launch debug console"
        );
    }
}
