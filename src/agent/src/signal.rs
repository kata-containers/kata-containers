// Copyright (c) 2019-2020 Ant Financial
// Copyright (c) 2020 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::sandbox::Sandbox;
use anyhow::{anyhow, Result};
use capctl::prctl::set_subreaper;
use nix::errno::Errno;
use nix::sys::wait::WaitPidFlag;
use nix::sys::wait::{self, Id, WaitStatus};
#[cfg(test)]
use nix::unistd::Pid;
use slog::{error, info, o, Logger};
use std::sync::Arc;
use tokio::select;
use tokio::signal::unix::{signal, SignalKind};
use tokio::sync::watch::Receiver;
use tokio::sync::Mutex;

// Offset added to a signal number to derive the exit code of a process that
// was terminated by that signal, following the conventional shell semantics
// described in https://tldp.org/LDP/abs/html/exitcodes.html.
const SIGNAL_EXIT_CODE_BASE: i32 = 128;

/// Derive a process exit code from its `WaitStatus`.
///
/// A process that exits normally reports its own exit code, while a process
/// terminated by a signal reports `128 + signal_number` (e.g. SIGKILL(9) ->
/// 137, SIGTERM(15) -> 143). Returns `None` for statuses that do not represent
/// process termination (e.g. stopped/continued), which the caller should skip.
fn exit_code_from_wait_status(wait_status: WaitStatus) -> Option<i32> {
    match wait_status {
        WaitStatus::Exited(_, code) => Some(code),
        WaitStatus::Signaled(_, sig, _) => Some(SIGNAL_EXIT_CODE_BASE + (sig as i32)),
        _ => None,
    }
}

async fn handle_sigchild(logger: Logger, sandbox: Arc<Mutex<Sandbox>>) -> Result<()> {
    info!(logger, "handling signal"; "signal" => "SIGCHLD");

    loop {
        // Match before reaping: a zombie still owns its numeric PID. Use the
        // same sandbox -> wait lock order as RPC/storage paths.
        let mut sandbox = sandbox.lock().await;
        // Hook executors also wait for children. Exclude them during peek/reap,
        // but release this lock before waiting for output to drain.
        let locker = rustjail::container::WAIT_PID_LOCKER.lock().await;
        let pending = loop {
            let result = wait::waitid(
                Id::All,
                WaitPidFlag::WEXITED
                    | WaitPidFlag::WNOHANG
                    | WaitPidFlag::WNOWAIT
                    | WaitPidFlag::__WALL,
            );
            if result != Err(Errno::EINTR) {
                break result;
            }
        };
        let pending = match pending {
            Ok(s) => {
                if s == WaitStatus::StillAlive {
                    return Ok(());
                }
                s
            }
            Err(Errno::ECHILD) => return Ok(()),
            Err(e) => return Err(anyhow!(e).context("waitid reaper peek failed")),
        };

        if let Some(pid) = pending.pid() {
            let raw_pid = pid.as_raw();
            let child_pid = format!("{raw_pid}");

            let logger = logger.new(o!("child-pid" => child_pid));
            let process = sandbox.find_process(raw_pid);
            if process.is_none() {
                // Helpers and reparented descendants have no Process handle.
                // Their PID is still reserved by the peeked zombie.
                loop {
                    match wait::waitpid(Some(pid), Some(WaitPidFlag::WNOHANG | WaitPidFlag::__WALL))
                    {
                        Err(Errno::EINTR) => continue,
                        Err(Errno::ECHILD) | Ok(_) => break,
                        Err(e) => return Err(anyhow!(e).context("reap untracked child")),
                    }
                }
                info!(logger, "child exited unexpectedly");
                continue;
            }

            let p = process.unwrap();
            let wait_status = p
                .wait_with_flags(WaitPidFlag::WEXITED | WaitPidFlag::WNOHANG | WaitPidFlag::__WALL)
                .map_err(|e| anyhow!(e).context("reap process handle"))?;
            // Do not hold the wait lock during potentially slow output drain.
            drop(locker);
            info!(logger, "wait_status"; "wait_status result" => format!("{:?}", wait_status),
                  "pidfd" => p.has_pidfd());

            let ret: i32 = match exit_code_from_wait_status(wait_status) {
                Some(code) => code,
                None => {
                    info!(logger, "got wrong status for process";
                                  "child-status" => format!("{:?}", wait_status));
                    continue;
                }
            };

            // In passfd io mode, we need to wait for the copy task end.
            if let Some(proc_io) = &mut p.proc_io {
                proc_io.wg_output.wait().await;
            }

            p.complete_exit(ret);

            info!(logger, "notify term to close");
            // close the socket file to notify readStdio to close terminal specifically
            // in case this process's terminal has been inherited by its children.
            p.notify_term_close();
        }
    }
}

pub async fn setup_signal_handler(
    logger: Logger,
    sandbox: Arc<Mutex<Sandbox>>,
    mut shutdown: Receiver<bool>,
) -> Result<()> {
    let logger = logger.new(o!("subsystem" => "signals"));

    set_subreaper(true)
        .map_err(|err| anyhow!(err).context("failed to setup agent as a child subreaper"))?;

    let mut sigchild_stream = signal(SignalKind::child())?;

    loop {
        select! {
            _ = shutdown.changed() => {
                info!(logger, "got shutdown request");
                break;
            }

            _ = sigchild_stream.recv() => {
                let result = handle_sigchild(logger.clone(), sandbox.clone()).await;

                match result {
                    Ok(()) => (),
                    Err(e) => {
                        // Log errors, but don't abort - just wait for more signals!
                        error!(logger, "failed to handle signal"; "error" => format!("{:?}", e));
                    }
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use nix::sys::signal::Signal;
    use tokio::pin;
    use tokio::sync::watch::channel;
    use tokio::time::Duration;

    #[test]
    fn test_exit_code_from_wait_status() {
        let pid = Pid::from_raw(1);

        // Normal exits report their own code unchanged.
        assert_eq!(
            exit_code_from_wait_status(WaitStatus::Exited(pid, 0)),
            Some(0)
        );
        assert_eq!(
            exit_code_from_wait_status(WaitStatus::Exited(pid, 42)),
            Some(42)
        );

        // Signal-terminated processes report 128 + signal number.
        // SIGKILL(9) -> 137, SIGTERM(15) -> 143, SIGINT(2) -> 130,
        // SIGSEGV(11) -> 139.
        for (sig, expected) in [
            (Signal::SIGKILL, 137),
            (Signal::SIGTERM, 143),
            (Signal::SIGINT, 130),
            (Signal::SIGSEGV, 139),
        ] {
            assert_eq!(
                exit_code_from_wait_status(WaitStatus::Signaled(pid, sig, false)),
                Some(expected),
                "unexpected exit code for {sig:?}"
            );
        }

        // Non-terminating statuses are skipped.
        assert_eq!(
            exit_code_from_wait_status(WaitStatus::Stopped(pid, Signal::SIGSTOP)),
            None
        );
        assert_eq!(exit_code_from_wait_status(WaitStatus::StillAlive), None);
    }

    #[tokio::test]
    async fn test_setup_signal_handler() {
        let logger = slog::Logger::root(slog::Discard, o!());
        let s = Sandbox::new(&logger).unwrap();

        let sandbox = Arc::new(Mutex::new(s));

        let (tx, rx) = channel(true);

        let handle = tokio::spawn(setup_signal_handler(logger, sandbox, rx));

        let timeout = tokio::time::sleep(Duration::from_secs(1));
        pin!(timeout);

        tx.send(true).expect("failed to request shutdown");

        select! {
            _ = handle => {
                println!("INFO: task completed");
            },
            _ = &mut timeout => {
                panic!("signal thread failed to stop");
            }
        }
    }
}
