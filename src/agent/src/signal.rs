// Copyright (c) 2019-2020 Ant Financial
// Copyright (c) 2020 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::sandbox::Sandbox;
use anyhow::{anyhow, Result};
use capctl::prctl::set_subreaper;
use nix::sys::wait::WaitPidFlag;
use nix::sys::wait::{self, WaitStatus};
use nix::unistd;
use slog::{error, info, o, Logger};
use std::sync::Arc;
use tokio::select;
use tokio::signal::unix::{signal, SignalKind};
use tokio::sync::watch::Receiver;
use tokio::sync::Mutex;
use unistd::Pid;

async fn handle_sigchild(logger: Logger, sandbox: Arc<Mutex<Sandbox>>) -> Result<()> {
    info!(logger, "handling signal"; "signal" => "SIGCHLD");

    loop {
        // Avoid reaping the undesirable child's signal, e.g., execute_hook's
        // The lock should be released immediately.
        rustjail::container::WAIT_PID_LOCKER.lock().await;
        let result = wait::waitpid(
            Some(Pid::from_raw(-1)),
            Some(WaitPidFlag::WNOHANG | WaitPidFlag::__WALL),
        );

        let wait_status = match result {
            Ok(s) => {
                if s == WaitStatus::StillAlive {
                    return Ok(());
                }
                s
            }
            Err(e) => return Err(anyhow!(e).context("waitpid reaper failed")),
        };

        info!(logger, "wait_status"; "wait_status result" => format!("{:?}", wait_status));

        if let Some(pid) = wait_status.pid() {
            let raw_pid = pid.as_raw();
            let child_pid = format!("{}", raw_pid);

            let logger = logger.new(o!("child-pid" => child_pid));

            let sandbox_ref = sandbox.clone();
            let mut sandbox = sandbox_ref.lock().await;

            let process = sandbox.find_process(raw_pid);
            if process.is_none() {
                info!(logger, "child exited unexpectedly");
                continue;
            }

            let mut p = process.unwrap();
            let ret: i32;

            match wait_status {
                WaitStatus::Exited(_, c) => ret = c,
                WaitStatus::Signaled(_, sig, _) => ret = sig as i32,
                _ => {
                    info!(logger, "got wrong status for process";
                                  "child-status" => format!("{:?}", wait_status));
                    continue;
                }
            }

            p.exit_code = ret;
            let _ = p.exit_tx.take();

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
    use tokio::pin;
    use tokio::sync::watch::channel;
    use tokio::time::Duration;

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

        loop {
            select! {
                _ = handle => {
                    println!("INFO: task completed");
                    break;
                },
                _ = &mut timeout => {
                    panic!("signal thread failed to stop");
                }
            }
        }
    }
}
