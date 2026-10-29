// Copyright (c) 2019 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{anyhow, bail, Context, Result};
use nix::mount::{mount, umount2, MntFlags, MsFlags};
use nix::sched::{unshare, CloneFlags};
use nix::unistd::{getpid, gettid};
use slog::Logger;
use std::fmt;
use std::fs;
use std::fs::File;
use std::io::Read;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
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
const PIDNS_KEEPER_ROOT: &str = "/run/kata-containers/sandbox/pidns-keeper";

/// A namespace whose PID 1 was an init container cannot be reused after that
/// container exits, even if its namespace file was pinned, so the sandbox
/// keeps a PID 1 of its own there.
#[derive(Debug)]
pub struct PidNamespaceKeeper {
    child: Child,
    pidfd: OwnedFd,
    namespace: File,
    stopped: bool,
}

impl PidNamespaceKeeper {
    pub async fn start() -> Result<Self> {
        let mut command = Command::new("/proc/self/exe");
        command.arg("pause");
        tokio::task::spawn_blocking(move || Self::spawn(command))
            .await
            .context("PID namespace keeper startup task")?
    }

    fn spawn(mut command: Command) -> Result<Self> {
        // CLONE_NEWPID applies to future children of the calling thread. Do
        // this on a disposable thread so the agent's other children retain
        // their normal namespace, and let Command perform the fork/exec.
        let (child, pidfd, namespace) = std::thread::spawn(move || -> Result<_> {
            unshare(CloneFlags::CLONE_NEWPID)?;
            // Keep the SIGCHLD reaper from releasing the PID before both
            // handles are open, including when the child exits immediately.
            let _reaper = rustjail::container::WAIT_PID_LOCKER.blocking_lock();
            let mut child = command
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()?;
            let handles = (|| -> Result<_> {
                // SAFETY: pidfd_open takes a PID and flags, with no pointers.
                let fd = unsafe { libc::syscall(libc::SYS_pidfd_open, child.id(), 0u32) };
                if fd < 0 {
                    return Err(std::io::Error::last_os_error().into());
                }
                // SAFETY: the successful syscall returned a new owned fd.
                let pidfd = unsafe { OwnedFd::from_raw_fd(fd as i32) };
                let namespace = File::open(format!("/proc/{}/ns/pid", child.id()))?;
                Ok((pidfd, namespace))
            })();
            match handles {
                Ok((pidfd, namespace)) => Ok((child, pidfd, namespace)),
                Err(error) => {
                    // The reaper is excluded and no workloads have joined yet.
                    let _ = child.kill();
                    let _ = child.wait();
                    Err(error)
                }
            }
        })
        .join()
        .map_err(|_| anyhow!("PID namespace keeper thread panicked"))??;
        let mut keeper = Self {
            child,
            pidfd,
            namespace,
            stopped: false,
        };
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
        // rustjail reexecs before joining, so refer to the agent's fd table.
        format!("/proc/{}/fd/{}", getpid(), self.namespace.as_raw_fd())
    }

    pub fn stop(&mut self) -> Result<()> {
        if self.stopped {
            return Ok(());
        }
        // A pidfd cannot signal a reused PID, even if the SIGCHLD handler
        // has already reaped the keeper. Do not wait here: the reaper needs
        // the sandbox lock held by our caller to finish namespace teardown.
        // SAFETY: pidfd is owned by self; a null siginfo requests kill semantics.
        let result = unsafe {
            libc::syscall(
                libc::SYS_pidfd_send_signal,
                self.pidfd.as_raw_fd(),
                libc::SIGKILL,
                std::ptr::null::<libc::siginfo_t>(),
                0u32,
            )
        };
        if result < 0 {
            let error = std::io::Error::last_os_error();
            if error.raw_os_error() != Some(libc::ESRCH) {
                return Err(error.into());
            }
        }
        self.stopped = true;
        Ok(())
    }
}

impl Drop for PidNamespaceKeeper {
    fn drop(&mut self) {
        if let Err(error) = self.stop() {
            warn!(slog_scope::logger(), "failed to stop PID namespace keeper"; "error" => %error);
        }
    }
}

// Workloads see the keeper as PID 1, and it starts out as the agent: with the
// guest's root directory and every capability. Unlike a pause container it
// would let a workload reach the guest through /proc/1/root, or by attaching
// to it with CAP_SYS_PTRACE and running code as the keeper. So leave it with
// an empty root, no capabilities to get back out, and no readable memory.
fn confine_pidns_keeper(root: &Path) -> Result<()> {
    fs::create_dir_all(root).context("create the PID namespace keeper root")?;
    unshare(CloneFlags::CLONE_NEWNS).context("unshare the PID namespace keeper mounts")?;
    mount(
        None::<&str>,
        "/",
        None::<&str>,
        MsFlags::MS_REC | MsFlags::MS_PRIVATE,
        None::<&str>,
    )?;
    // Read before /proc itself is dropped below.
    let inherited = inherited_mount_points(&fs::read_to_string("/proc/self/mountinfo")?);
    mount(
        Some("tmpfs"),
        root,
        Some("tmpfs"),
        MsFlags::MS_RDONLY | MsFlags::MS_NOSUID | MsFlags::MS_NODEV | MsFlags::MS_NOEXEC,
        Some("size=4096,mode=0555"),
    )
    .context("mount the PID namespace keeper root")?;
    nix::unistd::chdir(root)?;
    // Keep cwd on the tmpfs while detaching it from the guest filesystem.
    // Unlike chroot alone, /proc/<pid>/root/.. cannot walk out of this mount.
    // This also works on initrd rootfs, where pivot_root is unavailable.
    umount2(root, MntFlags::MNT_DETACH).context("detach the PID namespace keeper root")?;
    // The new mount namespace holds a copy of every guest mount, and would
    // keep volumes the agent unmounts later busy for as long as the sandbox
    // lives. The guest root cannot be unmounted, and is always there anyway.
    for mount_point in inherited.iter().rev().filter(|path| path.as_str() != "/") {
        match umount2(mount_point.as_str(), MntFlags::MNT_DETACH) {
            // Already gone along with a parent mount.
            Ok(()) | Err(nix::errno::Errno::EINVAL) | Err(nix::errno::Errno::ENOENT) => (),
            Err(error) => {
                return Err(error).with_context(|| format!("release inherited mount {mount_point}"))
            }
        }
    }
    nix::unistd::chroot(".").context("chroot the PID namespace keeper")?;
    nix::unistd::chdir("/").context("enter the PID namespace keeper root")?;

    capctl::prctl::set_no_new_privs()
        .map_err(|e| anyhow!("set no new privileges for the PID namespace keeper: {e}"))?;
    capctl::ambient::clear()
        .and_then(|_| capctl::bounding::clear())
        .and_then(|_| capctl::CapState::empty().set_current())
        .map_err(|e| anyhow!("drop the PID namespace keeper capabilities: {e}"))?;
    capctl::prctl::set_dumpable(false)
        .map_err(|e| anyhow!("make the PID namespace keeper non-dumpable: {e}"))?;
    Ok(())
}

fn inherited_mount_points(mountinfo: &str) -> Vec<String> {
    mountinfo
        .lines()
        .filter_map(|line| line.split(' ').nth(4))
        .map(|path| {
            // The kernel writes space, tab, newline and backslash as octal.
            let mut bytes = Vec::with_capacity(path.len());
            let mut rest = path.as_bytes();
            while let Some((&byte, tail)) = rest.split_first() {
                let escaped = (byte == b'\\' && tail.len() >= 3)
                    .then(|| std::str::from_utf8(&tail[..3]).ok())
                    .flatten()
                    .and_then(|digits| u8::from_str_radix(digits, 8).ok());
                match escaped {
                    Some(value) => {
                        bytes.push(value);
                        rest = &tail[3..];
                    }
                    None => {
                        bytes.push(byte);
                        rest = tail;
                    }
                }
            }
            String::from_utf8_lossy(&bytes).into_owned()
        })
        .collect()
}

pub fn run_pidns_keeper() -> Result<()> {
    if getpid().as_raw() != 1 {
        bail!("PID namespace keeper must run as PID 1");
    }
    let signals = pidns_keeper_signals()?;
    confine_pidns_keeper(Path::new(PIDNS_KEEPER_ROOT))?;
    nix::unistd::write(std::io::stdout(), &[PIDNS_KEEPER_READY])?;
    reap_pidns_keeper(signals)
}

fn pidns_keeper_signals() -> Result<nix::sys::signal::SigSet> {
    use nix::sys::signal::{pthread_sigmask, SigSet, SigmaskHow, Signal};
    let mut signals = SigSet::empty();
    // Blocked SIGCHLD remains pending for sigwait even without a handler.
    // Leave TERM/INT at their defaults so namespace PID 1 ignores them.
    signals.add(Signal::SIGCHLD);
    pthread_sigmask(SigmaskHow::SIG_BLOCK, Some(&signals), None)?;
    Ok(signals)
}

fn reap_pidns_keeper(signals: nix::sys::signal::SigSet) -> Result<()> {
    use nix::sys::wait::{waitpid, WaitPidFlag, WaitStatus};
    loop {
        loop {
            match waitpid(None, Some(WaitPidFlag::WNOHANG)) {
                Ok(WaitStatus::StillAlive) | Err(nix::errno::Errno::ECHILD) => break,
                Ok(_) => (),
                Err(nix::errno::Errno::EINTR) => continue,
                Err(error) => return Err(error.into()),
            }
        }
        signals.wait()?;
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
    use super::{confine_pidns_keeper, Namespace, NamespaceType, PidNamespaceKeeper};
    use crate::mount::remove_mounts;
    use nix::sched::CloneFlags;
    use rstest::rstest;
    use std::fs::File;
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

    impl PidNamespaceKeeper {
        pub(crate) fn test_keeper_with_pid() -> (Self, nix::unistd::Pid) {
            let keeper = test_keeper();
            let pid = nix::unistd::Pid::from_raw(keeper.child.id() as i32);
            (keeper, pid)
        }
    }

    const KEEPER_ROOT_ENV: &str = "KATA_TEST_PIDNS_KEEPER_ROOT";

    // Only does something when pidns_keeper_is_confined() runs it in a
    // process of its own, which it can then inspect from outside.
    #[test]
    #[ignore]
    fn confined_pidns_keeper_process() {
        let Ok(root) = std::env::var(KEEPER_ROOT_ENV) else {
            return;
        };
        let signals = super::pidns_keeper_signals().unwrap();
        confine_pidns_keeper(std::path::Path::new(&root)).unwrap();
        println!("keeper-tid={}", nix::unistd::gettid());
        super::reap_pidns_keeper(signals).unwrap();
    }

    fn confined_keeper(new_pidns: bool) -> (std::process::Child, String, tempfile::TempDir) {
        use std::io::BufRead;

        let root = tempfile::tempdir().unwrap();
        let mut command = std::process::Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "namespace::tests::confined_pidns_keeper_process",
                "--ignored",
                "--nocapture",
            ])
            .env(KEEPER_ROOT_ENV, root.path())
            .stdout(std::process::Stdio::piped());
        let mut child = std::thread::spawn(move || {
            if new_pidns {
                nix::sched::unshare(CloneFlags::CLONE_NEWPID).unwrap();
            }
            command.spawn().unwrap()
        })
        .join()
        .unwrap();
        let tid = std::io::BufReader::new(child.stdout.take().unwrap())
            .lines()
            .map_while(Result::ok)
            .find_map(|line| {
                line.split_once("keeper-tid=")
                    .map(|(_, tid)| tid.to_string())
            })
            .expect("the keeper process should report that it is confined");
        (child, tid, root)
    }

    #[test]
    fn pidns_keeper_is_confined() {
        use std::os::unix::fs::MetadataExt;

        skip_if_not_root!();
        let (child, tid, _root) = confined_keeper(false);
        let child = scopeguard::guard(child, |mut child| {
            let _ = child.kill();
            let _ = child.wait();
        });
        let pid = child.id();
        // unshare also separates this thread's filesystem state from the
        // test harness, so inspect the thread that performed confinement.
        let task = format!("/proc/{pid}/task/{tid}");
        let root = format!("{task}/root");
        let identity = |path: &str| {
            let metadata = std::fs::metadata(path).unwrap();
            (metadata.dev(), metadata.ino())
        };
        let expected = identity(&root);
        for path in [
            format!("{task}/cwd"),
            format!("{root}/.."),
            format!("{root}/../../.."),
        ] {
            assert_eq!(identity(&path), expected, "{path} escapes the keeper root");
        }
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0);
        assert_eq!(
            std::fs::write(format!("{root}/file"), b"test")
                .unwrap_err()
                .raw_os_error(),
            Some(libc::EROFS)
        );
        assert_ne!(
            std::fs::read_link(format!("{task}/ns/mnt")).unwrap(),
            std::fs::read_link("/proc/self/ns/mnt").unwrap()
        );
        // Capabilities are per thread, and the test harness runs the test in
        // one of its own.
        let status = std::fs::read_to_string(format!("/proc/{pid}/task/{tid}/status")).unwrap();
        let field = |name: &str| {
            status
                .lines()
                .find_map(|line| line.strip_prefix(name))
                .map(|value| value.trim().to_string())
                .unwrap_or_else(|| panic!("no {} in the keeper status", name))
        };
        for set in ["CapInh:", "CapPrm:", "CapEff:", "CapBnd:", "CapAmb:"] {
            assert_eq!(u64::from_str_radix(&field(set), 16).unwrap(), 0, "{set}");
        }
        assert_eq!(field("NoNewPrivs:"), "1");
    }

    #[test]
    fn inherited_mount_points_are_unescaped() {
        let mountinfo = "22 1 0:21 / / rw - ext4 /dev/vda1 rw\n\
                         23 22 0:22 / /run rw shared:2 - tmpfs tmpfs rw\n\
                         24 23 0:23 / /run/a\\040b\\134c rw - tmpfs tmpfs rw\n";
        assert_eq!(
            super::inherited_mount_points(mountinfo),
            ["/", "/run", "/run/a b\\c"]
        );
    }

    #[test]
    fn pidns_keeper_releases_inherited_mounts() {
        skip_if_not_root!();
        let volume = tempfile::tempdir().unwrap();
        nix::mount::mount(
            Some("tmpfs"),
            volume.path(),
            Some("tmpfs"),
            nix::mount::MsFlags::empty(),
            None::<&str>,
        )
        .unwrap();
        let volume = scopeguard::guard(volume, |volume| {
            let _ = nix::mount::umount(volume.path());
        });
        let marker = volume.path().join("marker");
        File::create(&marker).unwrap();

        let (child, tid, _root) = confined_keeper(false);
        let child = scopeguard::guard(child, |mut child| {
            let _ = child.kill();
            let _ = child.wait();
        });
        let namespace = File::open(format!("/proc/{}/task/{tid}/ns/mnt", child.id())).unwrap();
        let pinned = std::thread::spawn(move || {
            // A mount namespace cannot be joined while sharing filesystem
            // state with the other test threads.
            nix::sched::unshare(CloneFlags::CLONE_FS).unwrap();
            nix::sched::setns(namespace, CloneFlags::CLONE_NEWNS).unwrap();
            marker.exists()
        })
        .join()
        .unwrap();
        assert!(!pinned, "the keeper still holds a mount it inherited");
    }

    #[test]
    fn shared_pidns_ignores_workload_termination_signals() {
        skip_if_not_root!();
        let (child, _tid, _root) = confined_keeper(true);
        let mut child = scopeguard::guard(child, |mut child| {
            let _ = child.kill();
            let _ = child.wait();
        });
        let path = format!("/proc/{}/ns/pid", child.id());
        for _ in 0..2 {
            let namespace = File::open(&path).unwrap();
            let status = std::thread::spawn(move || {
                nix::sched::setns(namespace, CloneFlags::CLONE_NEWPID).unwrap();
                capctl::prctl::set_no_new_privs().unwrap();
                capctl::CapState::empty().set_current().unwrap();
                std::process::Command::new("sh")
                    .args(["-c", "kill -TERM 1 && kill -INT 1"])
                    .status()
                    .unwrap()
            })
            .join()
            .unwrap();
            assert!(status.success());
            assert!(child.try_wait().unwrap().is_none());
        }
    }

    #[test]
    fn shared_pidns_rejects_keeper_early_exit() {
        skip_if_not_root!();
        assert!(PidNamespaceKeeper::spawn(std::process::Command::new("true")).is_err());
    }

    #[test]
    fn shared_pidns_cleanup_tolerates_signal_reaper() {
        skip_if_not_root!();
        let mut keeper = test_keeper();
        let path = keeper.path();
        let namespace = std::fs::read_link(&path).unwrap();
        let pid = nix::unistd::Pid::from_raw(keeper.child.id() as i32);
        // Model the agent SIGCHLD handler reaping a child before Child::wait.
        nix::sys::signal::kill(pid, nix::sys::signal::Signal::SIGKILL).unwrap();
        nix::sys::wait::waitpid(pid, None).unwrap();
        assert_eq!(std::fs::read_link(&path).unwrap(), namespace);
        keeper.stop().unwrap();
    }

    #[test]
    fn shared_pidns_survives_workload_exit_and_is_cleaned_up() {
        skip_if_not_root!();
        let mut keeper = test_keeper();
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
        keeper.stop().unwrap();
        keeper.child.wait().unwrap();
        drop(keeper);
        assert!(!std::path::Path::new(&path).exists());
    }

    #[test]
    fn shared_pidns_keepers_are_isolated() {
        skip_if_not_root!();
        let mut first = test_keeper();
        let mut second = test_keeper();
        assert_ne!(
            std::fs::read_link(first.path()).unwrap(),
            std::fs::read_link(second.path()).unwrap()
        );
        first.stop().unwrap();
        second.stop().unwrap();
        first.child.wait().unwrap();
        second.child.wait().unwrap();
    }

    #[test]
    fn shared_pidns_stop_does_not_wait_for_unreaped_workload() {
        skip_if_not_root!();
        let mut keeper = test_keeper();
        let namespace = File::open(keeper.path()).unwrap();
        let mut workload = std::thread::spawn(move || {
            nix::sched::setns(namespace, CloneFlags::CLONE_NEWPID).unwrap();
            std::process::Command::new("sleep")
                .arg("60")
                .spawn()
                .unwrap()
        })
        .join()
        .unwrap();
        let (stopped_tx, stopped_rx) = std::sync::mpsc::channel();
        let reaper = std::thread::spawn(move || {
            // Unblock a broken stop implementation after a deadline, so a
            // regression fails the test instead of hanging the test suite.
            let stopped = stopped_rx
                .recv_timeout(std::time::Duration::from_secs(5))
                .is_ok();
            workload.wait().unwrap();
            stopped
        });
        keeper.stop().unwrap();
        // Namespace PID 1 cannot finish exiting until this agent-parented
        // workload is reaped. stop must return so the reaper can do that.
        let _ = stopped_tx.send(());
        assert!(
            reaper.join().unwrap(),
            "stop waited for the workload reaper"
        );
        keeper.child.wait().unwrap();
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
