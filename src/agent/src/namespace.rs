// Copyright (c) 2019 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{anyhow, Result};
use nix::mount::MsFlags;
use nix::sched::{unshare, CloneFlags};
use nix::unistd::{getpid, gettid};
use slog::Logger;
use std::fmt;
use std::fs;
use std::fs::File;
use std::path::{Path, PathBuf};
use tracing::instrument;

use crate::mount::baremount;

const PERSISTENT_NS_DIR: &str = "/var/run/sandbox-ns";
pub const NSTYPEIPC: &str = "ipc";
pub const NSTYPEUTS: &str = "uts";
pub const NSTYPEPID: &str = "pid";

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

                if ns_type == NamespaceType::Uts && hostname.is_some() {
                    nix::unistd::sethostname(hostname.unwrap())?;
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
    use super::{Namespace, NamespaceType};
    use crate::mount::remove_mounts;
    use nix::sched::CloneFlags;
    use rstest::rstest;
    use tempfile::Builder;
    use test_utils::skip_if_not_root;

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
