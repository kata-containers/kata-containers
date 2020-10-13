// Copyright (c) 2019 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

use nix::mount::MsFlags;
use nix::sched::{unshare, CloneFlags};
use nix::unistd::{getpid, gettid};
use std::fmt;
use std::fs;
use std::fs::File;
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};
use std::thread;

use crate::mount::{BareMount, FLAGS};
use slog::Logger;

//use container::Process;
const PERSISTENT_NS_DIR: &str = "/var/run/sandbox-ns";
pub const NSTYPEIPC: &str = "ipc";
pub const NSTYPEUTS: &str = "uts";
pub const NSTYPEPID: &str = "pid";

pub fn get_current_thread_ns_path(ns_type: &str) -> String {
    format!(
        "/proc/{}/task/{}/ns/{}",
        getpid().to_string(),
        gettid().to_string(),
        ns_type
    )
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
    pub fn new(logger: &Logger) -> Self {
        Namespace {
            logger: logger.clone(),
            path: String::from(""),
            persistent_ns_dir: String::from(PERSISTENT_NS_DIR),
            ns_type: NamespaceType::IPC,
            hostname: None,
        }
    }

    pub fn as_ipc(mut self) -> Self {
        self.ns_type = NamespaceType::IPC;
        self
    }

    pub fn as_uts(mut self, hostname: &str) -> Self {
        self.ns_type = NamespaceType::UTS;
        if hostname != "" {
            self.hostname = Some(String::from(hostname));
        }
        self
    }

    pub fn as_pid(mut self) -> Self {
        self.ns_type = NamespaceType::PID;
        self
    }

    pub fn set_root_dir(mut self, dir: &str) -> Self {
        self.persistent_ns_dir = dir.to_string();
        self
    }

    // setup creates persistent namespace without switching to it.
    // Note, pid namespaces cannot be persisted.
    pub fn setup(mut self) -> Result<Self, String> {
        if let Err(err) = fs::create_dir_all(&self.persistent_ns_dir) {
            return Err(err.to_string());
        }

        let ns_path = PathBuf::from(&self.persistent_ns_dir);
        let ns_type = self.ns_type.clone();
        let logger = self.logger.clone();

        let new_ns_path = ns_path.join(&ns_type.get());

        if let Err(err) = File::create(new_ns_path.as_path()) {
            return Err(err.to_string());
        }

        self.path = new_ns_path.clone().into_os_string().into_string().unwrap();
        let hostname = self.hostname.clone();

        let new_thread = thread::spawn(move || {
            let origin_ns_path = get_current_thread_ns_path(&ns_type.get());

            let _origin_ns_fd = match File::open(Path::new(&origin_ns_path)) {
                Err(err) => return Err(err.to_string()),
                Ok(file) => file.as_raw_fd(),
            };

            // Create a new netns on the current thread.
            let cf = ns_type.get_flags().clone();

            if let Err(err) = unshare(cf) {
                return Err(err.to_string());
            }

            if ns_type == NamespaceType::UTS && hostname.is_some() {
                match nix::unistd::sethostname(hostname.unwrap()) {
                    Err(err) => return Err(err.to_string()),
                    Ok(_) => (),
                }
            }
            // Bind mount the new namespace from the current thread onto the mount point to persist it.
            let source: &str = origin_ns_path.as_str();
            let destination: &str = new_ns_path.as_path().to_str().unwrap_or("none");

            let mut flags = MsFlags::empty();

            match FLAGS.get("rbind") {
                Some(x) => {
                    let (_, f) = *x;
                    flags = flags | f;
                }
                None => (),
            };

            let bare_mount = BareMount::new(source, destination, "none", flags, "", &logger);
            bare_mount.mount().map_err(|e| {
                format!(
                    "Failed to mount {} to {} with err:{:?}",
                    source, destination, e
                )
            })?;

            Ok(())
        });

        match new_thread.join() {
            Ok(t) => match t {
                Err(err) => return Err(err),
                Ok(()) => (),
            },
            Err(err) => return Err(format!("Failed to join thread {:?}!", err)),
        }

        Ok(self)
    }
}

/// Represents the Namespace type.
#[derive(Clone, Copy, PartialEq)]
enum NamespaceType {
    IPC,
    UTS,
    PID,
}

impl NamespaceType {
    /// Get the string representation of the namespace type.
    pub fn get(&self) -> &str {
        match *self {
            Self::IPC => "ipc",
            Self::UTS => "uts",
            Self::PID => "pid",
        }
    }

    /// Get the associate flags with the namespace type.
    pub fn get_flags(&self) -> CloneFlags {
        match *self {
            Self::IPC => CloneFlags::CLONE_NEWIPC,
            Self::UTS => CloneFlags::CLONE_NEWUTS,
            Self::PID => CloneFlags::CLONE_NEWPID,
        }
    }
}

impl fmt::Debug for NamespaceType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.get())
    }
}

impl Default for NamespaceType {
    fn default() -> Self {
        NamespaceType::IPC
    }
}

#[cfg(test)]
mod tests {
    use super::{Namespace, NamespaceType};
    use crate::{mount::remove_mounts, skip_if_not_root};
    use nix::sched::CloneFlags;
    use tempfile::Builder;

    #[test]
    fn test_setup_persistent_ns() {
        skip_if_not_root!();
        // Create dummy logger and temp folder.
        let logger = slog::Logger::root(slog::Discard, o!());
        let tmpdir = Builder::new().prefix("ipc").tempdir().unwrap();

        let ns_ipc = Namespace::new(&logger)
            .as_ipc()
            .set_root_dir(tmpdir.path().to_str().unwrap())
            .setup();

        assert!(ns_ipc.is_ok());
        assert!(remove_mounts(&vec![ns_ipc.unwrap().path]).is_ok());

        let logger = slog::Logger::root(slog::Discard, o!());
        let tmpdir = Builder::new().prefix("ipc").tempdir().unwrap();

        let ns_uts = Namespace::new(&logger)
            .as_uts("test_hostname")
            .set_root_dir(tmpdir.path().to_str().unwrap())
            .setup();

        assert!(ns_uts.is_ok());
        assert!(remove_mounts(&vec![ns_uts.unwrap().path]).is_ok());
    }

    #[test]
    fn test_namespace_type() {
        let ipc = NamespaceType::IPC;
        assert_eq!("ipc", ipc.get());
        assert_eq!(CloneFlags::CLONE_NEWIPC, ipc.get_flags());

        let uts = NamespaceType::UTS;
        assert_eq!("uts", uts.get());
        assert_eq!(CloneFlags::CLONE_NEWUTS, uts.get_flags());

        let pid = NamespaceType::PID;
        assert_eq!("pid", pid.get());
        assert_eq!(CloneFlags::CLONE_NEWPID, pid.get_flags());
    }
}
