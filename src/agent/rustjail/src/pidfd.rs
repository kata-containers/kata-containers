// Copyright (c) 2026 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0
//

//! Stable process handles. Only kernels lacking the required syscalls fall back
//! to numeric PIDs; operational errors must never silently lose PID identity.

use nix::errno::Errno;
use nix::sys::wait::{waitid, Id, WaitPidFlag, WaitStatus};
use nix::unistd::getpid;
use std::os::fd::{AsFd, AsRawFd, FromRawFd, OwnedFd};
use std::sync::OnceLock;

#[derive(Debug)]
pub struct PidFd(OwnedFd);

fn probe_result(result: nix::Result<()>) -> nix::Result<bool> {
    match result {
        Ok(()) => Ok(true),
        Err(Errno::ENOSYS | Errno::EINVAL) => Ok(false),
        Err(e) => Err(e),
    }
}

fn cached_support(
    cache: &OnceLock<bool>,
    probe: impl FnOnce() -> nix::Result<bool>,
) -> nix::Result<bool> {
    if let Some(supported) = cache.get() {
        return Ok(*supported);
    }
    // Resource or permission errors must not poison the process-wide cache.
    let supported = probe()?;
    Ok(*cache.get_or_init(|| supported))
}

fn retry<T>(mut f: impl FnMut() -> nix::Result<T>) -> nix::Result<T> {
    loop {
        match f() {
            Err(Errno::EINTR) => continue,
            result => return result,
        }
    }
}

impl PidFd {
    fn open_raw(pid: libc::pid_t) -> nix::Result<Self> {
        if pid <= 0 {
            return Err(Errno::EINVAL);
        }
        let fd = retry(|| {
            let fd = unsafe { libc::syscall(libc::SYS_pidfd_open, pid, 0) };
            Errno::result(fd)
        })?;
        // SAFETY: a successful pidfd_open returns a new descriptor owned by us.
        // The kernel sets FD_CLOEXEC on it.
        Ok(Self(unsafe { OwnedFd::from_raw_fd(fd as i32) }))
    }

    /// Probe open, signal and wait together. A kernel may
    /// support pidfd_open while still lacking waitid(P_PIDFD).
    fn probe() -> nix::Result<bool> {
        let result = (|| {
            let fd = Self::open_raw(getpid().as_raw())?;
            fd.signal(0)?;
            match fd.wait(WaitPidFlag::WEXITED | WaitPidFlag::WNOHANG) {
                Err(Errno::ECHILD) => Ok(()), // self is not our child
                Err(e) => Err(e),
                Ok(_) => Err(Errno::EIO),
            }
        })();
        probe_result(result)
    }

    pub fn supported() -> nix::Result<bool> {
        static SUPPORTED: OnceLock<bool> = OnceLock::new();
        cached_support(&SUPPORTED, Self::probe)
    }

    /// Call while the startup handshake still prevents the child from execing
    /// and while its owner excludes the agent reaper.
    pub fn open(pid: libc::pid_t) -> nix::Result<Option<Self>> {
        if pid <= 0 {
            return Err(Errno::EINVAL);
        }
        if Self::supported()? {
            Self::open_raw(pid).map(Some)
        } else {
            Ok(None)
        }
    }

    /// Signal the process referenced by this descriptor, even if its PID is reused.
    pub fn signal(&self, signal: libc::c_int) -> nix::Result<()> {
        retry(|| {
            let result = unsafe {
                libc::syscall(
                    libc::SYS_pidfd_send_signal,
                    self.0.as_raw_fd(),
                    signal,
                    std::ptr::null::<libc::siginfo_t>(),
                    0,
                )
            };
            Errno::result(result).map(drop)
        })
    }

    /// Read the exit status of a child using its stable process identity.
    pub fn wait(&self, flags: WaitPidFlag) -> nix::Result<WaitStatus> {
        retry(|| waitid(Id::PIDFd(self.0.as_fd()), flags))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nix::unistd::Pid;
    use scopeguard::{guard, ScopeGuard};
    use std::process::Child;
    use std::process::Command;

    fn child(command: &str, args: &[&str]) -> ScopeGuard<Child, impl FnOnce(Child)> {
        guard(
            Command::new(command).args(args).spawn().unwrap(),
            |mut child| {
                // waitid(P_PIDFD) may already have reaped the child. Only signal a
                // numeric PID when try_wait confirms that we still own a live child.
                if matches!(child.try_wait(), Ok(None)) {
                    let _ = child.kill();
                }
                let _ = child.wait();
            },
        )
    }

    fn supported() -> bool {
        let supported = PidFd::supported().unwrap();
        if !supported {
            eprintln!("skipping pidfd syscall test: kernel lacks pidfd support");
        }
        supported
    }

    #[test]
    fn test_probe_result() {
        for (result, expected) in [
            (Ok(()), Ok(true)),
            (Err(Errno::ENOSYS), Ok(false)),
            (Err(Errno::EINVAL), Ok(false)),
            (Err(Errno::EPERM), Err(Errno::EPERM)),
            (Err(Errno::EACCES), Err(Errno::EACCES)),
            (Err(Errno::EMFILE), Err(Errno::EMFILE)),
            (Err(Errno::ENFILE), Err(Errno::ENFILE)),
            (Err(Errno::ESRCH), Err(Errno::ESRCH)),
        ] {
            assert_eq!(probe_result(result), expected);
        }
    }

    #[test]
    fn test_support_cache_retries_operational_errors() {
        let cache = OnceLock::new();
        assert_eq!(
            cached_support(&cache, || Err(Errno::EMFILE)),
            Err(Errno::EMFILE)
        );
        assert!(cache.get().is_none());
        assert_eq!(cached_support(&cache, || Ok(true)), Ok(true));
        assert_eq!(
            cached_support(&cache, || panic!("unexpected probe")),
            Ok(true)
        );
    }

    #[test]
    fn test_support_cache_remembers_unsupported_kernel() {
        let cache = OnceLock::new();
        assert_eq!(cached_support(&cache, || Ok(false)), Ok(false));
        assert_eq!(
            cached_support(&cache, || panic!("unexpected probe")),
            Ok(false)
        );
    }

    #[test]
    fn pidfd_wait_preserves_exit_status() {
        if !supported() {
            return;
        }
        let child = child("/bin/sh", &["-c", "exit 42"]);
        let pid = child.id() as i32;
        let fd = PidFd::open(pid).unwrap().unwrap();
        assert_eq!(
            fd.wait(WaitPidFlag::WEXITED).unwrap(),
            WaitStatus::Exited(Pid::from_raw(pid), 42)
        );
        assert_eq!(fd.signal(0), Err(Errno::ESRCH));
    }

    #[test]
    fn pidfd_signal_and_cloexec() {
        if !supported() {
            return;
        }
        let child = child("/bin/sleep", &["30"]);
        let pid = child.id() as i32;
        let fd = PidFd::open(pid).unwrap().unwrap();
        let flags = nix::fcntl::fcntl(&fd.0, nix::fcntl::FcntlArg::F_GETFD).unwrap();
        assert_ne!(flags & libc::FD_CLOEXEC, 0);
        fd.signal(libc::SIGKILL).unwrap();
        assert_eq!(
            fd.wait(WaitPidFlag::WEXITED).unwrap(),
            WaitStatus::Signaled(Pid::from_raw(pid), nix::sys::signal::Signal::SIGKILL, false)
        );
        // A stale handle can never signal another process, even after reaping.
        assert_eq!(fd.signal(libc::SIGTERM), Err(Errno::ESRCH));
    }

    #[test]
    fn pidfd_nonblocking_wait() {
        if !supported() {
            return;
        }
        let child = child("/bin/sleep", &["30"]);
        let pid = child.id() as i32;
        let fd = PidFd::open(pid).unwrap().unwrap();
        assert_eq!(
            fd.wait(WaitPidFlag::WEXITED | WaitPidFlag::WNOHANG)
                .unwrap(),
            WaitStatus::StillAlive
        );
        fd.signal(libc::SIGKILL).unwrap();
        fd.wait(WaitPidFlag::WEXITED).unwrap();
        assert_eq!(fd.signal(0), Err(Errno::ESRCH));
    }

    #[test]
    fn pidfd_rejects_process_groups_and_missing_pids() {
        for pid in [-1, 0] {
            assert_eq!(PidFd::open(pid).unwrap_err(), Errno::EINVAL);
        }
        if supported() {
            assert_eq!(PidFd::open(i32::MAX).unwrap_err(), Errno::ESRCH);
        }
    }
}
