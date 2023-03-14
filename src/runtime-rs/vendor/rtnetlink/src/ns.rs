// SPDX-License-Identifier: MIT

use crate::Error;
use nix::{
    fcntl::OFlag,
    sched::CloneFlags,
    sys::{
        stat::Mode,
        wait::{waitpid, WaitStatus},
    },
    unistd::{fork, ForkResult},
};
use std::{option::Option, path::Path, process::exit};

// if "only" smol or smol+tokio were enabled, we use smol because
// it doesn't require an active tokio runtime - just to be sure.
#[cfg(feature = "smol_socket")]
async fn try_spawn_blocking<F, R>(fut: F) -> R
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    async_global_executor::spawn_blocking(fut).await
}

// only tokio enabled, so use tokio
#[cfg(all(not(feature = "smol_socket"), feature = "tokio_socket"))]
async fn try_spawn_blocking<F, R>(fut: F) -> R
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    match tokio::task::spawn_blocking(fut).await {
        Ok(v) => v,
        Err(err) => {
            std::panic::resume_unwind(err.into_panic());
        }
    }
}

// neither smol nor tokio - just run blocking op directly.
// hopefully not too blocking...
#[cfg(all(not(feature = "smol_socket"), not(feature = "tokio_socket")))]
async fn try_spawn_blocking<F, R>(fut: F) -> R
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    fut()
}

pub const NETNS_PATH: &str = "/run/netns/";
pub const SELF_NS_PATH: &str = "/proc/self/ns/net";
pub const NONE_FS: &str = "none";

pub struct NetworkNamespace();

impl NetworkNamespace {
    /// Add a new network namespace.
    /// This is equivalent to `ip netns add NS_NAME`.
    pub async fn add(ns_name: String) -> Result<(), Error> {
        // Forking process to avoid moving caller into new namespace
        NetworkNamespace::prep_for_fork()?;
        log::trace!("Forking...");
        match unsafe { fork() } {
            Ok(ForkResult::Parent { child, .. }) => NetworkNamespace::parent_process(child),
            Ok(ForkResult::Child) => {
                NetworkNamespace::child_process(ns_name);
            }
            Err(e) => {
                let err_msg = format!("Fork failed: {}", e);
                Err(Error::NamespaceError(err_msg))
            }
        }
    }

    /// Remove a network namespace
    /// This is equivalent to `ip netns del NS_NAME`.
    pub async fn del(ns_name: String) -> Result<(), Error> {
        try_spawn_blocking(move || {
            let mut netns_path = String::new();
            netns_path.push_str(NETNS_PATH);
            netns_path.push_str(&ns_name);
            let ns_path = Path::new(&netns_path);

            if nix::mount::umount2(ns_path, nix::mount::MntFlags::MNT_DETACH).is_err() {
                let err_msg = String::from("Namespace unmount failed (are you running as root?)");
                return Err(Error::NamespaceError(err_msg));
            }

            if nix::unistd::unlink(ns_path).is_err() {
                let err_msg =
                    String::from("Namespace file remove failed (are you running as root?)");
                return Err(Error::NamespaceError(err_msg));
            }

            Ok(())
        })
        .await
    }

    pub fn prep_for_fork() -> Result<(), Error> {
        // Placeholder function, nothing to do here.
        Ok(())
    }

    /// This is the parent process form the fork, it waits for the
    /// child to exit properly
    pub fn parent_process(child: nix::unistd::Pid) -> Result<(), Error> {
        log::trace!("parent_process child PID: {}", child);
        log::trace!("Waiting for child to finish...");
        match waitpid(child, None) {
            Ok(wait_status) => match wait_status {
                WaitStatus::Exited(_, res) => {
                    log::trace!("Child exited with: {}", res);
                    if res == 0 {
                        return Ok(());
                    }
                    log::error!("Error child result: {}", res);
                    let err_msg = format!("Error child result: {}", res);
                    Err(Error::NamespaceError(err_msg))
                }
                WaitStatus::Signaled(_, signal, has_dump) => {
                    log::error!("Error child killed by signal: {}", signal);
                    let err_msg = format!(
                        "Error child process was killed by signal: {} with core dump {}",
                        signal, has_dump
                    );
                    Err(Error::NamespaceError(err_msg))
                }
                _ => {
                    log::error!("Unknown child process status");
                    let err_msg = String::from("Unknown child process status");
                    Err(Error::NamespaceError(err_msg))
                }
            },
            Err(e) => {
                log::error!("wait error: {}", e);
                let err_msg = format!("wait error: {}", e);
                Err(Error::NamespaceError(err_msg))
            }
        }
    }

    fn child_process(ns_name: String) -> ! {
        let res = std::panic::catch_unwind(|| -> Result<(), Error> {
            let netns_path = NetworkNamespace::child_process_create_ns(ns_name)?;
            NetworkNamespace::unshare_processing(netns_path)?;
            Ok(())
        });
        match res {
            Err(_panic) => {
                // panic should have already been printed by the handler
                log::error!("child process crashed");
                std::process::abort()
            }
            Ok(Err(fail)) => {
                log::error!("child process failed: {}", fail);
                exit(1)
            }
            Ok(Ok(())) => exit(0),
        }
    }

    /// This is the child process, it will actually create the namespace
    /// resources. It creates the folder and namespace file.
    /// Returns the namespace file path
    pub fn child_process_create_ns(ns_name: String) -> Result<String, Error> {
        log::trace!("child_process will create the namespace");

        let mut netns_path = String::new();

        let dir_path = Path::new(NETNS_PATH);
        let mut mkdir_mode = Mode::empty();
        let mut open_flags = OFlag::empty();
        let mut mount_flags = nix::mount::MsFlags::empty();
        let none_fs = Path::new(&NONE_FS);
        let none_p4: Option<&Path> = None;

        // flags in mkdir
        mkdir_mode.insert(Mode::S_IRWXU);
        mkdir_mode.insert(Mode::S_IRGRP);
        mkdir_mode.insert(Mode::S_IXGRP);
        mkdir_mode.insert(Mode::S_IROTH);
        mkdir_mode.insert(Mode::S_IXOTH);

        open_flags.insert(OFlag::O_RDONLY);
        open_flags.insert(OFlag::O_CREAT);
        open_flags.insert(OFlag::O_EXCL);

        netns_path.push_str(NETNS_PATH);
        netns_path.push_str(&ns_name);

        // creating namespaces folder if not exists
        #[allow(clippy::collapsible_if)]
        if nix::sys::stat::stat(dir_path).is_err() {
            if let Err(e) = nix::unistd::mkdir(dir_path, mkdir_mode) {
                log::error!("mkdir error: {}", e);
                let err_msg = format!("mkdir error: {}", e);
                return Err(Error::NamespaceError(err_msg));
            }
        }

        // Try to mount /run/netns, with MS_REC | MS_SHARED
        // If it fails, creates the mount with MS_BIND | MS_REC
        // This is the same strategy used by `ip netns add NS`
        mount_flags.insert(nix::mount::MsFlags::MS_REC);
        mount_flags.insert(nix::mount::MsFlags::MS_SHARED);
        if nix::mount::mount(
            Some(Path::new("")),
            dir_path,
            Some(none_fs),
            mount_flags,
            none_p4,
        )
        .is_err()
        {
            mount_flags = nix::mount::MsFlags::empty();
            mount_flags.insert(nix::mount::MsFlags::MS_BIND);
            mount_flags.insert(nix::mount::MsFlags::MS_REC);

            if let Err(e) = nix::mount::mount(
                Some(Path::new(dir_path)),
                dir_path,
                Some(none_fs),
                mount_flags,
                none_p4,
            ) {
                log::error!("mount error: {}", e);
                let err_msg = format!("mount error: {}", e);
                return Err(Error::NamespaceError(err_msg));
            }
        }

        mount_flags = nix::mount::MsFlags::empty();
        mount_flags.insert(nix::mount::MsFlags::MS_REC);
        mount_flags.insert(nix::mount::MsFlags::MS_SHARED);
        if let Err(e) = nix::mount::mount(
            Some(Path::new("")),
            dir_path,
            Some(none_fs),
            mount_flags,
            none_p4,
        ) {
            log::error!("mount error: {}", e);
            let err_msg = format!("mount error: {}", e);
            return Err(Error::NamespaceError(err_msg));
        }

        let ns_path = Path::new(&netns_path);

        // creating the netns file
        let fd = match nix::fcntl::open(ns_path, open_flags, Mode::empty()) {
            Ok(raw_fd) => raw_fd,
            Err(e) => {
                log::error!("open error: {}", e);
                let err_msg = format!("open error: {}", e);
                return Err(Error::NamespaceError(err_msg));
            }
        };

        if let Err(e) = nix::unistd::close(fd) {
            log::error!("close error: {}", e);
            let err_msg = format!("close error: {}", e);
            let _ = nix::unistd::unlink(ns_path);
            return Err(Error::NamespaceError(err_msg));
        }

        Ok(netns_path)
    }

    /// This function unshare the calling process and move into
    /// the given network namespace
    #[allow(unused)]
    pub fn unshare_processing(netns_path: String) -> Result<(), Error> {
        let mut setns_flags = CloneFlags::empty();
        let mut open_flags = OFlag::empty();
        let ns_path = Path::new(&netns_path);

        let none_fs = Path::new(&NONE_FS);
        let none_p4: Option<&Path> = None;

        // unshare to the new network namespace
        if let Err(e) = nix::sched::unshare(CloneFlags::CLONE_NEWNET) {
            log::error!("unshare error: {}", e);
            let err_msg = format!("unshare error: {}", e);
            let _ = nix::unistd::unlink(ns_path);
            return Err(Error::NamespaceError(err_msg));
        }

        open_flags = OFlag::empty();
        open_flags.insert(OFlag::O_RDONLY);
        open_flags.insert(OFlag::O_CLOEXEC);

        let fd = match nix::fcntl::open(Path::new(&SELF_NS_PATH), open_flags, Mode::empty()) {
            Ok(raw_fd) => raw_fd,
            Err(e) => {
                log::error!("open error: {}", e);
                let err_msg = format!("open error: {}", e);
                return Err(Error::NamespaceError(err_msg));
            }
        };

        let self_path = Path::new(&SELF_NS_PATH);

        // bind to the netns
        if let Err(e) = nix::mount::mount(
            Some(self_path),
            ns_path,
            Some(none_fs),
            nix::mount::MsFlags::MS_BIND,
            none_p4,
        ) {
            log::error!("mount error: {}", e);
            let err_msg = format!("mount error: {}", e);
            let _ = nix::unistd::unlink(ns_path);
            return Err(Error::NamespaceError(err_msg));
        }

        setns_flags.insert(CloneFlags::CLONE_NEWNET);
        if let Err(e) = nix::sched::setns(fd, setns_flags) {
            log::error!("setns error: {}", e);
            let err_msg = format!("setns error: {}", e);
            let _ = nix::unistd::unlink(ns_path);
            return Err(Error::NamespaceError(err_msg));
        }

        Ok(())
    }
}
