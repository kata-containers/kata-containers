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
        let mut already_reaped = false;
        let pending = loop {
            let result = wait::waitid(
                Id::All,
                WaitPidFlag::WEXITED
                    | WaitPidFlag::WNOHANG
                    | WaitPidFlag::WNOWAIT
                    | WaitPidFlag::__WALL,
            );
            match result {
                Err(Errno::EINTR) => continue,
                Err(Errno::ENOSYS | Errno::EINVAL | Errno::EPERM | Errno::EACCES) => {
                    // The whole waitid syscall can be unavailable, not only
                    // P_PIDFD. Keep the legacy reaper working in that case.
                    already_reaped = true;
                    break loop {
                        match wait::waitpid(None, Some(WaitPidFlag::WNOHANG | WaitPidFlag::__WALL))
                        {
                            Err(Errno::EINTR) => continue,
                            result => break result,
                        }
                    };
                }
                result => break result,
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
            Err(e) => return Err(anyhow!(e).context("find exited child")),
        };

        if let Some(pid) = pending.pid() {
            let raw_pid = pid.as_raw();
            let child_pid = format!("{raw_pid}");

            let logger = logger.new(o!("child-pid" => child_pid));
            let process = sandbox.find_process(raw_pid);
            if process.is_none() {
                // Helpers and reparented descendants have no Process handle.
                // Unless the legacy wait already consumed its status, the
                // helper's PID is still reserved by the peeked zombie.
                if !already_reaped {
                    loop {
                        match wait::waitpid(
                            Some(pid),
                            Some(WaitPidFlag::WNOHANG | WaitPidFlag::__WALL),
                        ) {
                            Err(Errno::EINTR) => continue,
                            Err(Errno::ECHILD) | Ok(_) => break,
                            Err(e) => return Err(anyhow!(e).context("reap untracked child")),
                        }
                    }
                }
                info!(logger, "child exited unexpectedly");
                continue;
            }

            let p = process.unwrap();
            let wait_status = if already_reaped {
                pending
            } else {
                loop {
                    match p.wait_with_flags(
                        WaitPidFlag::WEXITED | WaitPidFlag::WNOHANG | WaitPidFlag::__WALL,
                    ) {
                        Err(Errno::EINTR) => continue,
                        Err(Errno::ENOSYS | Errno::EINVAL | Errno::EPERM | Errno::EACCES)
                            if p.has_pidfd() =>
                        {
                            // P_ALL peek succeeded and both locks still exclude
                            // competing reapers. This zombie still owns pid,
                            // even if P_PIDFD became unavailable after binding.
                            break loop {
                                match wait::waitpid(
                                    Some(pid),
                                    Some(WaitPidFlag::WNOHANG | WaitPidFlag::__WALL),
                                ) {
                                    Err(Errno::EINTR) => continue,
                                    result => break result,
                                }
                            };
                        }
                        result => break result,
                    }
                }
                .map_err(|e| anyhow!(e).context("reap process handle"))?
            };
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

    #[tokio::test]
    async fn test_reaper_when_waitid_is_unavailable() {
        use oci_spec::runtime::{LinuxBuilder, ProcessBuilder, RootBuilder, SpecBuilder};
        use rustjail::container::LinuxContainer;
        use rustjail::process::Process;
        use rustjail::process::ProcessOperations;
        use rustjail::specconv::CreateOpts;
        use std::process::Command;

        if !nix::unistd::Uid::effective().is_root() {
            return;
        }
        const SCENARIO: &str = "KATA_REAPER_WAITID_ERRNO";
        let scenario = match std::env::var(SCENARIO) {
            Ok(scenario) => scenario,
            Err(_) => {
                // A fresh process isolates both seccomp and the pidfd support cache.
                for (backend, errno) in [
                    ("all", 0),
                    ("all", libc::ENOSYS),
                    ("all", libc::EINVAL),
                    ("all", libc::EPERM),
                    ("all", libc::EACCES),
                    ("pidfd", libc::ENOSYS),
                    ("pidfd", libc::EINVAL),
                    ("pidfd", libc::EPERM),
                    ("pidfd", libc::EACCES),
                    ("pidfd", libc::ESRCH),
                    ("pidfd", libc::EMFILE),
                ] {
                    let scenario = format!("{backend}:{errno}");
                    let output = Command::new(std::env::current_exe().unwrap())
                        .args([
                            "--exact",
                            "signal::tests::test_reaper_when_waitid_is_unavailable",
                            "--test-threads=1",
                            "--nocapture",
                        ])
                        .env(SCENARIO, &scenario)
                        .output()
                        .unwrap();
                    assert!(
                        output.status.success(),
                        "waitid scenario {scenario}: {}{}",
                        String::from_utf8_lossy(&output.stdout),
                        String::from_utf8_lossy(&output.stderr)
                    );
                }
                return;
            }
        };
        let (backend, errno) = scenario.split_once(':').unwrap();
        let pidfd_only = backend == "pidfd";
        let errno: u32 = errno.parse().unwrap();
        if pidfd_only && !rustjail::pidfd::PidFd::supported().unwrap() {
            eprintln!("skipping late P_PIDFD restriction: kernel lacks pidfd");
            return;
        }
        let logger = slog::Logger::root(slog::Discard, o!());
        let dir = tempfile::tempdir().unwrap();
        let spec = SpecBuilder::default()
            .root(RootBuilder::default().path("/").build().unwrap())
            .linux(
                LinuxBuilder::default()
                    .cgroups_path(format!(
                        "/kata.agent.test.k8s.io/dummycontainer-reaper-{}",
                        std::process::id()
                    ))
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap();
        let mut container = LinuxContainer::new(
            "reaper",
            dir.path().join("rootfs").to_str().unwrap(),
            None,
            CreateOpts {
                spec: Some(spec),
                ..Default::default()
            },
            &logger,
        )
        .unwrap();
        let _cleanup = scopeguard::guard(container.cgroup_manager.clone(), |manager| {
            let _ = manager.destroy();
        });
        let mut sandbox = Sandbox::new(&logger).unwrap();

        if errno != 0 && !pidfd_only {
            // Deny the entire waitid syscall, including P_ALL and P_PIDFD.
            let filter = [
                libc::sock_filter {
                    code: 0x20,
                    jt: 0,
                    jf: 0,
                    k: 0,
                },
                libc::sock_filter {
                    code: 0x15,
                    jt: 0,
                    jf: 1,
                    k: libc::SYS_waitid as u32,
                },
                libc::sock_filter {
                    code: 0x06,
                    jt: 0,
                    jf: 0,
                    k: libc::SECCOMP_RET_ERRNO | errno,
                },
                libc::sock_filter {
                    code: 0x06,
                    jt: 0,
                    jf: 0,
                    k: libc::SECCOMP_RET_ALLOW,
                },
            ];
            let program = libc::sock_fprog {
                len: filter.len() as u16,
                filter: filter.as_ptr() as *mut libc::sock_filter,
            };
            // SAFETY: the filter program stays valid throughout prctl.
            unsafe {
                assert_eq!(libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0), 0);
                assert_eq!(libc::prctl(libc::PR_SET_SECCOMP, 2, &program), 0);
            }
        }
        let child = |code: i32| {
            scopeguard::guard(
                Command::new("/bin/sh")
                    .args(["-c", &format!("exit {code}")])
                    .spawn()
                    .unwrap(),
                |mut child| {
                    if matches!(child.try_wait(), Ok(None)) {
                        let _ = child.kill();
                    }
                    let _ = child.wait();
                },
            )
        };
        let managed = child(23);
        let untracked = child(7);
        let oci_process = ProcessBuilder::default().terminal(true).build().unwrap();
        let mut process = Process::new(&logger, &oci_process, "managed", false, 0, None).unwrap();
        process.bind_pid(managed.id() as i32).unwrap();
        if errno != 0 && !pidfd_only {
            assert!(!process.has_pidfd());
        }
        if pidfd_only {
            assert!(process.has_pidfd());
            let filter = [
                libc::sock_filter {
                    code: 0x20,
                    jt: 0,
                    jf: 0,
                    k: 0,
                },
                libc::sock_filter {
                    code: 0x15,
                    jt: 0,
                    jf: 3,
                    k: libc::SYS_waitid as u32,
                },
                libc::sock_filter {
                    code: 0x20,
                    jt: 0,
                    jf: 0,
                    k: if cfg!(target_endian = "little") {
                        16
                    } else {
                        20
                    },
                },
                libc::sock_filter {
                    code: 0x15,
                    jt: 0,
                    jf: 1,
                    k: libc::P_PIDFD,
                },
                libc::sock_filter {
                    code: 0x06,
                    jt: 0,
                    jf: 0,
                    k: libc::SECCOMP_RET_ERRNO | errno,
                },
                libc::sock_filter {
                    code: 0x06,
                    jt: 0,
                    jf: 0,
                    k: libc::SECCOMP_RET_ALLOW,
                },
            ];
            let program = libc::sock_fprog {
                len: filter.len() as u16,
                filter: filter.as_ptr() as *mut libc::sock_filter,
            };
            // SAFETY: only this isolated subprocess is restricted, after bind.
            unsafe {
                assert_eq!(libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0), 0);
                assert_eq!(libc::prctl(libc::PR_SET_SECCOMP, 2, &program), 0);
            }
        }
        let mut exit_rx = process.exit_rx.take().unwrap();
        container.processes.insert("managed".into(), process);
        sandbox.add_container(container);
        let sandbox = Arc::new(Mutex::new(sandbox));
        // Observe zombies without consuming their exit statuses.
        tokio::time::timeout(Duration::from_secs(5), async {
            for child in [&managed, &untracked] {
                loop {
                    let stat =
                        std::fs::read_to_string(format!("/proc/{}/stat", child.id())).unwrap();
                    if stat.split_once(") ").unwrap().1.starts_with('Z') {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(1)).await;
                }
            }
        })
        .await
        .unwrap();
        let result = handle_sigchild(logger.clone(), sandbox.clone()).await;
        if pidfd_only && matches!(errno as i32, libc::ESRCH | libc::EMFILE) {
            assert_eq!(
                result.unwrap_err().downcast_ref::<Errno>(),
                Some(&Errno::from_raw(errno as i32))
            );
            let mut sandbox = sandbox.lock().await;
            let p = sandbox.find_container_process("reaper", "managed").unwrap();
            assert!(p.has_pidfd());
            assert!(p.exit_tx.is_some());
            assert!(!exit_rx.has_changed().unwrap());
            for child in [&managed, &untracked] {
                assert!(matches!(
                    wait::waitpid(Pid::from_raw(child.id() as i32), Some(WaitPidFlag::WNOHANG)),
                    Ok(WaitStatus::Exited(_, _))
                ));
            }
            return;
        }
        result.unwrap();
        assert!(
            tokio::time::timeout(Duration::from_secs(5), exit_rx.changed())
                .await
                .unwrap()
                .is_err()
        );
        {
            let mut sandbox = sandbox.lock().await;
            let p = sandbox.find_container_process("reaper", "managed").unwrap();
            assert_eq!(p.exit_code, 23);
            // Retain the exit record without invoking WaitProcess.
            assert!(!p.has_pidfd());
            assert_eq!(p.signal(0), Err(Errno::ESRCH));
        }
        handle_sigchild(logger, sandbox.clone()).await.unwrap();
        for child in [&managed, &untracked] {
            assert_eq!(
                wait::waitpid(Pid::from_raw(child.id() as i32), Some(WaitPidFlag::WNOHANG)),
                Err(Errno::ECHILD)
            );
        }
    }

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
