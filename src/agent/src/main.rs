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
extern crate prctl;
extern crate protocols;
extern crate regex;
extern crate rustjail;
extern crate serde_json;
extern crate signal_hook;
#[macro_use]
extern crate scan_fmt;
extern crate oci;

#[macro_use]
extern crate slog;
extern crate slog_async;
extern crate slog_json;

use futures::*;
use nix::sys::wait::{self, WaitStatus};
use nix::unistd;
use prctl::set_child_subreaper;
use rustjail::errors::*;
use signal_hook::{iterator::Signals, SIGCHLD};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::os::unix::fs::{self as unixfs};
use std::os::unix::io::AsRawFd;
use std::path::Path;
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::{io, thread};
use unistd::Pid;

mod config;
mod device;
mod logging;
mod mount;
mod namespace;
pub mod netlink;
mod network;
pub mod random;
mod sandbox;
mod uevent;
mod version;
#[cfg(test)] mod test_utils;

use mount::{cgroups_mount, general_mount};
use sandbox::Sandbox;
use slog::Logger;
use uevent::watch_uevents;

mod grpc;

const NAME: &'static str = "kata-agent";
const VSOCK_ADDR: &'static str = "vsock://-1";
const VSOCK_PORT: u16 = 1024;
const KERNEL_CMDLINE_FILE: &'static str = "/proc/cmdline";
const CONSOLE_PATH: &'static str = "/dev/console";

lazy_static! {
    static ref GLOBAL_DEVICE_WATCHER: Arc<Mutex<HashMap<String, Sender<String>>>> =
        Arc::new(Mutex::new(HashMap::new()));
}

use std::mem::MaybeUninit;

fn announce(logger: &Logger) {
    let commit = match env::var("VERSION_COMMIT") {
        Ok(s) => s,
        Err(_) => String::from(""),
    };
    info!(logger, "announce";
    "agent-commit" => commit.as_str(),
    "agent-version" =>  version::AGENT_VERSION,
    "api-version" => version::API_VERSION,
    );
}

fn main() -> Result<()> {
    env::set_var("RUST_BACKTRACE", "full");

    lazy_static::initialize(&SHELLS);

    let mut config = config::agentConfig::new();

    config.parse_cmdline(KERNEL_CMDLINE_FILE)?;

    let writer = io::stdout();

    let logger = logging::create_logger(NAME, "agent", config.log_level, writer);

    announce(&logger);

    // This "unused" variable is required as it enables the global (and crucially static) logger,
    // which is required to satisfy the the lifetime constraints of the auto-generated gRPC code.
    let _guard = slog_scope::set_global_logger(logger.new(o!("subsystem" => "grpc")));

    let shells = SHELLS.clone();

    let shell_handle = if config.debug_console {
        let thread_logger = logger.clone();

        thread::spawn(move || {
            let shells = shells.lock().unwrap();
            let result = setup_debug_console(shells.to_vec());
            if result.is_err() {
                // Report error, but don't fail
                warn!(thread_logger, "failed to setup debug console";
                    "error" => format!("{}", result.unwrap_err()));
            }
        })
    } else {
        unsafe { MaybeUninit::zeroed().assume_init() }
    };

    if unistd::getpid() == Pid::from_raw(1) {
        init_agent_as_init(&logger)?;
    }

    // Initialize unique sandbox structure.
    let s = Sandbox::new(&logger).map_err(|e| {
        error!(logger, "Failed to create sandbox with error: {:?}", e);
        e
    })?;

    let sandbox = Arc::new(Mutex::new(s));

    setup_signal_handler(&logger, sandbox.clone()).unwrap();
    watch_uevents(sandbox.clone());

    let (tx, rx) = mpsc::channel::<i32>();
    sandbox.lock().unwrap().sender = Some(tx);

    //vsock:///dev/vsock, port
    let mut server = grpc::start(sandbox.clone(), VSOCK_ADDR, VSOCK_PORT);

    /*
        let _ = fs::remove_file("/tmp/testagent");
        let _ = fs::remove_dir_all("/run/agent");
        let mut server = grpc::start(sandbox.clone(), "unix:///tmp/testagent", 1);
    */

    let handle = thread::spawn(move || {
        // info!("Press ENTER to exit...");
        // let _ = io::stdin().read(&mut [0]).unwrap();
        // thread::sleep(Duration::from_secs(3000));

        let _ = rx.recv().unwrap();
    });
    // receive something from destroy_sandbox here?
    // or in the thread above? It depneds whether grpc request
    // are run in another thread or in the main thead?
    // let _ = rx.wait();

    handle.join().unwrap();

    if config.debug_console {
        shell_handle.join().unwrap();
    }

    let _ = server.shutdown().wait();
    let _ = fs::remove_file("/tmp/testagent");

    Ok(())
}

use nix::sys::wait::WaitPidFlag;

fn setup_signal_handler(logger: &Logger, sandbox: Arc<Mutex<Sandbox>>) -> Result<()> {
    let logger = logger.new(o!("subsystem" => "signals"));

    set_child_subreaper(true).map_err(|err| {
        format!(
            "failed to setup agent as a child subreaper, failed with {}",
            err
        )
    })?;

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

    Ok(())
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

use nix::fcntl::{self, OFlag};
use nix::sys::stat::Mode;
use std::os::unix::io::{FromRawFd, RawFd};
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn setup_debug_console(shells: Vec<String>) -> Result<()> {
    for shell in shells.iter() {
        let binary = PathBuf::from(shell);
        if binary.exists() {
            let f: RawFd = fcntl::open(CONSOLE_PATH, OFlag::O_RDWR, Mode::empty())?;
            let cmd = Command::new(shell)
                .stdin(unsafe { Stdio::from_raw_fd(f) })
                .stdout(unsafe { Stdio::from_raw_fd(f) })
                .stderr(unsafe { Stdio::from_raw_fd(f) })
                .spawn();

            let mut cmd = match cmd {
                Ok(c) => c,
                Err(_) => {
                    return Err(ErrorKind::ErrorCode("failed to spawn shell".to_string()).into())
                }
            };

            cmd.wait()?;

            return Ok(());
        } else {
            return Err(ErrorKind::ErrorCode("invalid shell".to_string()).into());
        }
    }

    Err(ErrorKind::ErrorCode("no shell".to_string()).into())
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

        let result = setup_debug_console(shells.to_vec());

        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "Error Code: 'no shell'");
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

        let result = setup_debug_console(shells.to_vec());

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Error Code: 'invalid shell'"
        );
    }
}
