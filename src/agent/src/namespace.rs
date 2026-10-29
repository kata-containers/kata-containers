// Copyright (c) 2019 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{anyhow, bail, Context, Result};
use nix::mount::MsFlags;
use nix::sched::{unshare, CloneFlags};
use nix::unistd::{getpid, gettid};
use slog::Logger;
use std::fmt;
use std::fs;
use std::fs::File;
use std::io::Read;
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};
use tracing::instrument;

use crate::mount::baremount;

const PERSISTENT_NS_DIR: &str = "/var/run/sandbox-ns";
pub const NSTYPEIPC: &str = "ipc";
pub const NSTYPEUTS: &str = "uts";
pub const NSTYPEPID: &str = "pid";
const PIDNS_KEEPER_READY: u8 = 1;
const PIDNS_KEEPER_START_TIMEOUT: Duration = Duration::from_secs(5);

/// Owns PID 1 of a shared container PID namespace independently of workloads.
/// A namespace whose PID 1 was an init container cannot be reused after that
/// container exits, even if its namespace file was pinned.
#[derive(Debug)]
pub struct PidNamespaceKeeper {
    child: Child,
}

impl PidNamespaceKeeper {
    pub fn start() -> Result<Self> {
        let mut command = Command::new("/proc/self/exe");
        command.arg("pause");
        Self::spawn(command)
    }

    fn spawn(mut command: Command) -> Result<Self> {
        // CLONE_NEWPID applies to future children of the calling thread. Do
        // this on a disposable thread so the agent's other children retain
        // their normal namespace, and let Command perform the fork/exec.
        let child = std::thread::spawn(move || -> Result<Child> {
            unshare(CloneFlags::CLONE_NEWPID)?;
            Ok(command
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()?)
        })
        .join()
        .map_err(|_| anyhow!("PID namespace keeper thread panicked"))??;
        let mut keeper = Self { child };
        keeper
            .wait_for_ready()
            .context("PID namespace keeper startup")?;
        Ok(keeper)
    }

    fn wait_for_ready(&mut self) -> Result<()> {
        let mut output = self
            .child
            .stdout
            .take()
            .context("missing keeper ready pipe")?;
        let started = Instant::now();
        loop {
            let remaining = PIDNS_KEEPER_START_TIMEOUT.saturating_sub(started.elapsed());
            let mut descriptor = libc::pollfd {
                fd: output.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            };
            // output owns this descriptor and remains alive until poll returns.
            let result = unsafe { libc::poll(&mut descriptor, 1, remaining.as_millis() as i32) };
            if result == 0 {
                bail!("timed out waiting for PID namespace keeper readiness");
            }
            if result < 0 {
                let error = std::io::Error::last_os_error();
                if error.kind() == std::io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(error.into());
            }
            let mut ready = [0];
            output
                .read_exact(&mut ready)
                .context("keeper exited before becoming ready")?;
            if ready[0] != PIDNS_KEEPER_READY {
                bail!("invalid PID namespace keeper readiness message");
            }
            return Ok(());
        }
    }

    pub fn path(&self) -> String {
        format!("/proc/{}/ns/pid", self.child.id())
    }

    pub fn stop(&mut self) -> Result<()> {
        match self.child.try_wait() {
            Ok(Some(_)) => return Ok(()),
            // The agent's SIGCHLD handler may already have reaped the child.
            Err(error) if error.raw_os_error() == Some(libc::ECHILD) => return Ok(()),
            Err(error) => return Err(error.into()),
            Ok(None) => (),
        }
        if let Err(error) = self.child.kill() {
            // A SIGCHLD reap may race with try_wait and kill.
            if error.raw_os_error() != Some(libc::ESRCH) {
                return Err(error.into());
            }
        }
        match self.child.wait() {
            Ok(_) => Ok(()),
            Err(error) if error.raw_os_error() == Some(libc::ECHILD) => Ok(()),
            Err(error) => Err(error.into()),
        }
    }
}

impl Drop for PidNamespaceKeeper {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

/// Reap orphaned workload processes while keeping their PID namespace alive.
pub fn run_pidns_keeper() -> Result<()> {
    use nix::sys::signal::{
        pthread_sigmask, sigaction, SaFlags, SigAction, SigHandler, SigSet, SigmaskHow, Signal,
    };
    use nix::sys::wait::{waitpid, WaitPidFlag, WaitStatus};

    extern "C" fn wakeup(_: libc::c_int) {}

    if getpid().as_raw() != 1 {
        bail!("PID namespace keeper must run as PID 1");
    }

    let mut signals = SigSet::empty();
    for signal in [Signal::SIGCHLD, Signal::SIGTERM, Signal::SIGINT] {
        signals.add(signal);
        // PID 1 ignores signals without installed handlers. This handler is
        // empty and signals are consumed synchronously through sigwait below.
        unsafe {
            sigaction(
                signal,
                &SigAction::new(
                    SigHandler::Handler(wakeup),
                    SaFlags::SA_RESTART,
                    SigSet::empty(),
                ),
            )?;
        }
    }
    pthread_sigmask(SigmaskHow::SIG_BLOCK, Some(&signals), None)?;
    nix::unistd::write(std::io::stdout(), &[PIDNS_KEEPER_READY])?;
    loop {
        loop {
            match waitpid(None, Some(WaitPidFlag::WNOHANG)) {
                Ok(WaitStatus::StillAlive) | Err(nix::errno::Errno::ECHILD) => break,
                Ok(_) => (),
                Err(nix::errno::Errno::EINTR) => continue,
                Err(error) => return Err(error.into()),
            }
        }
        if matches!(signals.wait()?, Signal::SIGTERM | Signal::SIGINT) {
            return Ok(());
        }
    }
}

#[instrument]
pub fn get_current_thread_ns_path(ns_type: &str) -> String {
    format!("/proc/{}/task/{}/ns/{}", getpid(), gettid(), ns_type)
}

#[derive(Debug)]
pub struct Namespace {
    logger: Logger,
    pub path: String,
    persistent_ns_dir: String,
    ns_type: NamespaceType,
    //only used for uts namespace
    pub hostname: Option<String>,
}

impl Namespace {
    #[instrument]
    pub fn new(logger: &Logger) -> Self {
        Namespace {
            logger: logger.clone(),
            path: String::from(""),
            persistent_ns_dir: String::from(PERSISTENT_NS_DIR),
            ns_type: NamespaceType::Ipc,
            hostname: None,
        }
    }

    #[instrument]
    pub fn get_ipc(mut self) -> Self {
        self.ns_type = NamespaceType::Ipc;
        self
    }

    #[instrument]
    pub fn get_uts(mut self, hostname: &str) -> Self {
        self.ns_type = NamespaceType::Uts;
        if !hostname.is_empty() {
            self.hostname = Some(String::from(hostname));
        }
        self
    }

    #[instrument]
    pub fn get_pid(mut self) -> Self {
        self.ns_type = NamespaceType::Pid;
        self
    }

    #[allow(dead_code)]
    pub fn set_root_dir(mut self, dir: &str) -> Self {
        self.persistent_ns_dir = dir.to_string();
        self
    }

    // setup creates persistent namespace without switching to it.
    // Note, pid namespaces cannot be persisted.
    #[instrument]
    #[allow(clippy::question_mark)]
    pub async fn setup(mut self) -> Result<Self> {
        fs::create_dir_all(&self.persistent_ns_dir)?;

        let ns_path = PathBuf::from(&self.persistent_ns_dir);
        let ns_type = self.ns_type;
        if ns_type == NamespaceType::Pid {
            return Err(anyhow!("Cannot persist namespace of PID type"));
        }
        let logger = self.logger.clone();

        let new_ns_path = ns_path.join(ns_type.get());

        File::create(new_ns_path.as_path())?;

        self.path = new_ns_path.clone().into_os_string().into_string().unwrap();
        let hostname = self.hostname.clone();

        let new_thread = std::thread::spawn(move || {
            if let Err(err) = || -> Result<()> {
                let origin_ns_path = get_current_thread_ns_path(ns_type.get());

                let source = Path::new(&origin_ns_path);
                let destination = new_ns_path.as_path();

                File::open(source)?;

                // Create a new netns on the current thread.
                let cf = ns_type.get_flags();

                unshare(cf)?;

                if ns_type == NamespaceType::Uts {
                    if let Some(host) = hostname {
                        nix::unistd::sethostname(host)?;
                    }
                }
                // Bind mount the new namespace from the current thread onto the mount point to persist it.

                let mut flags = MsFlags::empty();
                flags |= MsFlags::MS_BIND | MsFlags::MS_REC;

                baremount(source, destination, "none", flags, "", &logger).map_err(|e| {
                    anyhow!(
                        "Failed to mount {:?} to {:?} with err:{:?}",
                        source,
                        destination,
                        e
                    )
                })?;

                Ok(())
            }() {
                return Err(err);
            }

            Ok(())
        });

        new_thread
            .join()
            .map_err(|e| anyhow!("Failed to join thread {:?}!", e))??;

        Ok(self)
    }
}

/// Represents the Namespace type.
#[derive(Clone, Copy, PartialEq)]
enum NamespaceType {
    Ipc,
    Uts,
    Pid,
}

impl NamespaceType {
    /// Get the string representation of the namespace type.
    pub fn get(&self) -> &str {
        match *self {
            Self::Ipc => "ipc",
            Self::Uts => "uts",
            Self::Pid => "pid",
        }
    }

    /// Get the associate flags with the namespace type.
    pub fn get_flags(&self) -> CloneFlags {
        match *self {
            Self::Ipc => CloneFlags::CLONE_NEWIPC,
            Self::Uts => CloneFlags::CLONE_NEWUTS,
            Self::Pid => CloneFlags::CLONE_NEWPID,
        }
    }
}

impl fmt::Debug for NamespaceType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.get())
    }
}

#[cfg(test)]
mod tests {
    use super::{Namespace, NamespaceType, PidNamespaceKeeper};
    use crate::mount::remove_mounts;
    use nix::sched::CloneFlags;
    use rstest::rstest;
    use tempfile::Builder;
    use test_utils::skip_if_not_root;

    fn process_in_namespace(path: &str) -> std::process::Output {
        let path = path.to_string();
        std::thread::spawn(move || {
            let namespace = std::fs::File::open(path).unwrap();
            nix::sched::setns(namespace, CloneFlags::CLONE_NEWPID).unwrap();
            std::process::Command::new("readlink")
                .arg("/proc/self/ns/pid")
                .output()
                .unwrap()
        })
        .join()
        .unwrap()
    }

    fn test_keeper() -> PidNamespaceKeeper {
        let mut command = std::process::Command::new("sh");
        command.args(["-c", "printf '\\001'; exec sleep 60"]);
        PidNamespaceKeeper::spawn(command).unwrap()
    }

    #[test]
    fn shared_pidns_rejects_keeper_early_exit() {
        skip_if_not_root!();
        let error = PidNamespaceKeeper::spawn(std::process::Command::new("true")).unwrap_err();
        assert!(format!("{error:#}").contains("keeper exited before becoming ready"));
    }

    #[test]
    fn shared_pidns_cleanup_tolerates_signal_reaper() {
        skip_if_not_root!();
        let mut command = std::process::Command::new("sh");
        command.args(["-c", "printf '\\001'"]);
        let mut keeper = PidNamespaceKeeper::spawn(command).unwrap();
        let pid = nix::unistd::Pid::from_raw(keeper.child.id() as i32);
        // Model the agent SIGCHLD handler reaping a child before Child::wait.
        nix::sys::wait::waitpid(pid, None).unwrap();
        keeper.stop().unwrap();
    }

    #[test]
    fn shared_pidns_survives_workload_exit_and_is_cleaned_up() {
        skip_if_not_root!();
        let keeper = test_keeper();
        let path = keeper.path();
        let expected = std::fs::read_link(&path).unwrap();
        assert_ne!(expected, std::fs::read_link("/proc/self/ns/pid").unwrap());

        // Each command is a separate workload and exits before the next joins.
        for _ in 0..2 {
            let output = process_in_namespace(&path);
            assert!(output.status.success());
            assert_eq!(
                String::from_utf8(output.stdout).unwrap().trim(),
                expected.to_str().unwrap()
            );
        }
        drop(keeper);
        assert!(!std::path::Path::new(&path).exists());
    }

    #[test]
    fn shared_pidns_keepers_are_isolated() {
        skip_if_not_root!();
        let first = test_keeper();
        let second = test_keeper();
        assert_ne!(
            std::fs::read_link(first.path()).unwrap(),
            std::fs::read_link(second.path()).unwrap()
        );
    }

    #[tokio::test]
    async fn test_setup_persistent_ns() {
        skip_if_not_root!();
        // Create dummy logger and temp folder.
        let logger = slog::Logger::root(slog::Discard, o!());
        let tmpdir = Builder::new().prefix("ipc").tempdir().unwrap();

        let ns_ipc = Namespace::new(&logger)
            .get_ipc()
            .set_root_dir(tmpdir.path().to_str().unwrap())
            .setup()
            .await;

        assert!(ns_ipc.is_ok());
        assert!(remove_mounts(&[ns_ipc.unwrap().path]).is_ok());

        let logger = slog::Logger::root(slog::Discard, o!());
        let tmpdir = Builder::new().prefix("uts").tempdir().unwrap();

        let ns_uts = Namespace::new(&logger)
            .get_uts("test_hostname")
            .set_root_dir(tmpdir.path().to_str().unwrap())
            .setup()
            .await;

        assert!(ns_uts.is_ok());
        assert!(remove_mounts(&[ns_uts.unwrap().path]).is_ok());

        // Check it cannot persist pid namespaces.
        let logger = slog::Logger::root(slog::Discard, o!());
        let tmpdir = Builder::new().prefix("pid").tempdir().unwrap();

        let ns_pid = Namespace::new(&logger)
            .get_pid()
            .set_root_dir(tmpdir.path().to_str().unwrap())
            .setup()
            .await;

        assert!(ns_pid.is_err());
    }

    #[rstest]
    #[case::ipc(NamespaceType::Ipc, "ipc", CloneFlags::CLONE_NEWIPC)]
    #[case::uts(NamespaceType::Uts, "uts", CloneFlags::CLONE_NEWUTS)]
    #[case::pid(NamespaceType::Pid, "pid", CloneFlags::CLONE_NEWPID)]
    fn test_namespace_type(
        #[case] ns_type: NamespaceType,
        #[case] ns_name: &str,
        #[case] ns_flag: CloneFlags,
    ) {
        assert_eq!(ns_name, ns_type.get());
        assert_eq!(ns_flag, ns_type.get_flags());
    }

    #[test]
    fn test_new() {
        // Create dummy logger and temp folder.
        let logger = slog::Logger::root(slog::Discard, o!());
        let ns_ipc = Namespace::new(&logger);
        assert_eq!(NamespaceType::Ipc, ns_ipc.ns_type);
    }

    #[test]
    fn test_get_ipc() {
        // Create dummy logger and temp folder.
        let logger = slog::Logger::root(slog::Discard, o!());

        let ns_ipc = Namespace::new(&logger).get_ipc();
        assert_eq!(NamespaceType::Ipc, ns_ipc.ns_type);
    }

    #[test]
    fn test_get_uts_with_hostname() {
        let hostname = String::from("a.test.com");
        // Create dummy logger and temp folder.
        let logger = slog::Logger::root(slog::Discard, o!());

        let ns_uts = Namespace::new(&logger).get_uts(hostname.as_str());
        assert_eq!(NamespaceType::Uts, ns_uts.ns_type);
        assert!(ns_uts.hostname.is_some());
    }

    #[test]
    fn test_get_uts() {
        let hostname = String::from("");
        // Create dummy logger and temp folder.
        let logger = slog::Logger::root(slog::Discard, o!());

        let ns_uts = Namespace::new(&logger).get_uts(hostname.as_str());
        assert_eq!(NamespaceType::Uts, ns_uts.ns_type);
        assert!(ns_uts.hostname.is_none());
    }

    #[test]
    fn test_get_pid() {
        // Create dummy logger and temp folder.
        let logger = slog::Logger::root(slog::Discard, o!());

        let ns_pid = Namespace::new(&logger).get_pid();
        assert_eq!(NamespaceType::Pid, ns_pid.ns_type);
    }

    #[test]
    fn test_set_root_dir() {
        // Create dummy logger and temp folder.
        let logger = slog::Logger::root(slog::Discard, o!());
        let tmpdir = Builder::new().prefix("pid").tempdir().unwrap();

        let ns_root = Namespace::new(&logger).set_root_dir(tmpdir.path().to_str().unwrap());
        assert_eq!(NamespaceType::Ipc, ns_root.ns_type);
        assert_eq!(ns_root.persistent_ns_dir, tmpdir.path().to_str().unwrap());
    }

    #[rstest]
    #[case::namespace_type_get_ipc(NamespaceType::Ipc, "ipc")]
    #[case::namespace_type_get_uts(NamespaceType::Uts, "uts")]
    #[case::namespace_type_get_pid(NamespaceType::Pid, "pid")]
    fn test_namespace_type_get(#[case] ns_type: NamespaceType, #[case] ns_name: &str) {
        assert_eq!(ns_name, ns_type.get())
    }

    #[rstest]
    #[case::namespace_type_get_flags_ipc(NamespaceType::Ipc, CloneFlags::CLONE_NEWIPC)]
    #[case::namespace_type_get_flags_uts(NamespaceType::Uts, CloneFlags::CLONE_NEWUTS)]
    #[case::namespace_type_get_flags_pid(NamespaceType::Pid, CloneFlags::CLONE_NEWPID)]
    fn test_namespace_type_get_flags(#[case] ns_type: NamespaceType, #[case] ns_flag: CloneFlags) {
        // Run the tests
        assert_eq!(ns_flag, ns_type.get_flags())
    }
}
