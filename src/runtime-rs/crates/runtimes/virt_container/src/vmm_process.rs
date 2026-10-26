// Copyright (c) 2026 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0

use std::convert::TryFrom;
use std::fs;
use std::io::{self, ErrorKind};
use std::time::Duration;

use anyhow::{anyhow, Context, Result};

/// The exit watcher may reap the VMM while a stop fallback still holds its
/// PID, and the kernel may then hand that PID to another process. The start
/// time tells the two apart.
#[derive(Clone, Debug)]
pub struct VmmProcessIdentity {
    pid: i32,
    start_time: u64,
}

struct ProcessStat {
    state: char,
    start_time: u64,
}

fn read_tgid(pid: i32) -> Result<u32> {
    let status =
        fs::read_to_string(format!("/proc/{pid}/status")).context("read VMM process status")?;
    status
        .lines()
        .find_map(|line| line.strip_prefix("Tgid:"))
        .context("missing VMM thread group ID")?
        .trim()
        .parse()
        .context("parse VMM thread group ID")
}

fn parse_stat(stat: &str) -> Result<ProcessStat> {
    // The comm field is parenthesized and may itself contain spaces or ')'.
    let end = stat.rfind(") ").context("missing process comm")?;
    let mut fields = stat[end + 2..].split_whitespace();
    let state = fields
        .next()
        .and_then(|state| state.chars().next())
        .context("missing process state")?;
    // After state (field 3), starttime is field 22.
    let start_time = fields
        .nth(18)
        .context("missing process start time")?
        .parse()
        .context("parse process start time")?;
    Ok(ProcessStat { state, start_time })
}

fn read_stat(pid: i32) -> io::Result<Option<ProcessStat>> {
    match fs::read_to_string(format!("/proc/{pid}/stat")) {
        Ok(stat) => parse_stat(&stat)
            .map(Some)
            .map_err(|err| io::Error::new(ErrorKind::InvalidData, err)),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err),
    }
}

impl VmmProcessIdentity {
    pub(crate) fn capture(pid: u32) -> Result<Self> {
        let pid = i32::try_from(pid).context("VMM PID is out of range")?;
        // Some hypervisors return a shim thread ID, which must never become
        // the target of the saved-identity kill path.
        if read_tgid(pid)? == std::process::id() {
            return Err(anyhow!("VMM runs inside the shim process"));
        }
        let stat = read_stat(pid)
            .context("read VMM process state")?
            .context("VMM exited before its identity could be saved")?;
        Ok(Self {
            pid,
            start_time: stat.start_time,
        })
    }

    pub(crate) async fn terminate_and_wait(&self, timeout: Duration) -> Result<()> {
        let identity = self.clone();
        tokio::task::spawn_blocking(move || identity.terminate_and_wait_blocking(timeout))
            .await
            .context("join VMM stop")?
    }

    fn terminate_and_wait_blocking(&self, timeout: Duration) -> Result<()> {
        use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
        use std::time::Instant;

        // Open a pidfd before checking the start time. Signals sent through
        // this fd cannot hit a process which later reuses the numeric PID.
        let raw_fd = unsafe { libc::syscall(libc::SYS_pidfd_open, self.pid, 0) };
        if raw_fd < 0 {
            let err = io::Error::last_os_error();
            if err.raw_os_error() == Some(libc::ESRCH) {
                return Ok(());
            }
            return Err(err).context("open VMM pidfd");
        }
        let pidfd = unsafe { OwnedFd::from_raw_fd(raw_fd as i32) };

        let Some(stat) = read_stat(self.pid).context("read VMM state")? else {
            // A missing /proc entry can mean exit, but also a restricted proc
            // mount. The pidfd must report exit before cleanup is safe.
            let mut pollfd = libc::pollfd {
                fd: pidfd.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            };
            let rc = unsafe { libc::poll(&mut pollfd, 1, 0) };
            if rc > 0 && pollfd.revents & libc::POLLIN != 0 {
                return Ok(());
            }
            return Err(anyhow!("cannot verify VMM {} exited", self.pid));
        };
        if stat.start_time != self.start_time || matches!(stat.state, 'Z' | 'X') {
            return Ok(());
        }

        // pidfd_open() only accepts thread-group leaders, so the PID is the
        // thread group. Reading /proc here instead could race the reap.
        if u32::try_from(self.pid).ok() == Some(std::process::id()) {
            return Err(anyhow!("refusing to signal the shim process"));
        }

        let rc = unsafe {
            libc::syscall(
                libc::SYS_pidfd_send_signal,
                pidfd.as_raw_fd(),
                libc::SIGKILL,
                std::ptr::null::<libc::siginfo_t>(),
                0,
            )
        };
        if rc < 0 {
            let err = io::Error::last_os_error();
            if err.raw_os_error() != Some(libc::ESRCH) {
                return Err(err).context("kill VMM through pidfd");
            }
        }

        let deadline = Instant::now() + timeout;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(anyhow!("VMM {} did not exit", self.pid));
            }
            let mut pollfd = libc::pollfd {
                fd: pidfd.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            };
            let wait_ms = remaining.as_millis().min(i32::MAX as u128) as i32;
            let rc = unsafe { libc::poll(&mut pollfd, 1, wait_ms.max(1)) };
            if rc > 0 && pollfd.revents & libc::POLLIN != 0 {
                return Ok(());
            }
            if rc < 0 && io::Error::last_os_error().kind() != ErrorKind::Interrupted {
                return Err(io::Error::last_os_error()).context("wait for VMM exit");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_process_name_with_parenthesis() {
        let stat = "17 (qemu worker) x) S 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 12345";
        let parsed = parse_stat(stat).unwrap();
        assert_eq!(parsed.state, 'S');
        assert_eq!(parsed.start_time, 12345);
    }

    #[test]
    fn refuses_shim_process_and_thread_identities() {
        assert!(VmmProcessIdentity::capture(std::process::id())
            .unwrap_err()
            .to_string()
            .contains("inside the shim"));

        let (tid_tx, tid_rx) = std::sync::mpsc::channel();
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let thread = std::thread::spawn(move || {
            let tid = unsafe { libc::syscall(libc::SYS_gettid) as u32 };
            tid_tx.send(tid).unwrap();
            done_rx.recv().unwrap();
        });
        let tid = tid_rx.recv().unwrap();
        assert_ne!(tid, std::process::id());
        assert!(VmmProcessIdentity::capture(tid)
            .unwrap_err()
            .to_string()
            .contains("inside the shim"));
        done_tx.send(()).unwrap();
        thread.join().unwrap();
    }

    #[tokio::test]
    async fn never_signals_the_shim_process() {
        let pid = std::process::id() as i32;
        let start_time = read_stat(pid).unwrap().unwrap().start_time;
        let shim = VmmProcessIdentity { pid, start_time };
        assert!(shim
            .terminate_and_wait(Duration::from_secs(1))
            .await
            .unwrap_err()
            .to_string()
            .contains("refusing to signal the shim"));
    }

    #[tokio::test]
    async fn recovered_identity_stops_only_the_matching_process() {
        let mut child = std::process::Command::new("sleep")
            .arg("30")
            .spawn()
            .unwrap();
        let identity = VmmProcessIdentity::capture(child.id()).unwrap();

        let mut different = identity.clone();
        different.start_time += 1;
        different
            .terminate_and_wait(Duration::from_secs(1))
            .await
            .unwrap();
        assert!(child.try_wait().unwrap().is_none());

        identity
            .terminate_and_wait(Duration::from_secs(5))
            .await
            .unwrap();
        child.wait().unwrap();
    }
}
