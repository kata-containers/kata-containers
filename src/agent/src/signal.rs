// Copyright (c) 2019-2020 Ant Financial
// Copyright (c) 2020 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::sandbox::Sandbox;
use crossbeam_channel::{select, unbounded, Receiver, Sender};
use nix::sys::wait::WaitPidFlag;
use nix::sys::wait::{self, WaitStatus};
use nix::unistd;
use prctl::set_child_subreaper;
use rustjail::errors::*;
use signal_hook::{iterator::Signals, SIGCHLD};
use slog::{error, info, o, warn, Logger};
use std::sync::{Arc, Mutex};
use std::thread;
use std::thread::JoinHandle;
use unistd::Pid;

fn handle_signals(logger: Logger, ch: Sender<i32>, signals: Signals) {
    for sig in signals.forever() {
        let result = ch.send(sig);

        if result.is_err() {
            error!(logger, "failed to send signal"; "signal" => sig, "error" => format!("{:?}", result.err()));
        }
    }
}

pub fn setup_signal_handler(
    logger: &Logger,
    shutdown: Receiver<bool>,
    sandbox: Arc<Mutex<Sandbox>>,
) -> Result<JoinHandle<()>> {
    let logger = logger.new(o!("subsystem" => "signals"));

    set_child_subreaper(true).map_err(|err| {
        format!(
            "failed to setup agent as a child subreaper, failed with {}",
            err
        )
    })?;

    const HANDLED_SIGNALS: &[i32] = &[SIGCHLD];

    let signals = Signals::new(HANDLED_SIGNALS)?;

    let thread_signals = signals.clone();

    let handle = thread::spawn(move || {
        let s = sandbox.clone();

        let (tx, rx) = unbounded::<i32>();

        let thread_logger = logger.clone();

        let signals_thread =
            thread::spawn(move || handle_signals(thread_logger, tx, thread_signals));

        'nextevent: loop {
            select! {
            recv(rx) -> sig => {
                    let sig = match sig {
                        Ok(sig) => sig,
                        Err(e) => {
                            error!(logger, "failed to receive signal"; "error" => format!("{:?}", e));

                            break 'nextevent;
                        },
                    };

                    info!(logger, "received signal"; "signal" => format!("{:?}", sig));

                    // several signals can be combined together
                    // as one. So loop around to reap all
                    // exited children
                    'inner: loop {
                        let wait_status = match wait::waitpid(
                            Some(Pid::from_raw(-1)),
                            Some(WaitPidFlag::WNOHANG | WaitPidFlag::__WALL),
                            ) {
                            Ok(s) => {
                                if s == WaitStatus::StillAlive {
                                    break 'nextevent;
                                }
                                s
                            }
                            Err(e) => {
                                info!(
                                    logger,
                                    "waitpid reaper failed";
                                    "error" => e.as_errno().unwrap().desc()
                                );
                                break 'nextevent;
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
            },
            recv(shutdown) -> _ => {
                signals.close();

                let result = signals_thread.join();

                if result.is_err() {
                    warn!(logger, "failed to wait for signal handler thread"; "error" => format!("{:?}", result.err()));
                }
                break;
            },
            };
        }
    });

    Ok(handle)
}
