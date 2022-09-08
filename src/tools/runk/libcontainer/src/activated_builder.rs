// Copyright 2021-2022 Sony Group Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::container::{create_linux_container, Container, ContainerLauncher};
use crate::status::Status;
use crate::utils::validate_spec;
use anyhow::{anyhow, Result};
use derive_builder::Builder;
use oci::{ContainerState, Process as OCIProcess, Spec};
use rustjail::container::update_namespaces;
use slog::{debug, Logger};
use std::fs::File;
use std::path::{Path, PathBuf};

/// Used for exec command. It will prepare the options for joining an existing container.
#[derive(Default, Builder, Debug, Clone)]
#[builder(build_fn(validate = "Self::validate"))]
pub struct ActivatedContainer {
    pub id: String,
    pub root: PathBuf,
    pub console_socket: Option<PathBuf>,
    pub pid_file: Option<PathBuf>,
    pub tty: bool,
    pub cwd: Option<PathBuf>,
    pub env: Vec<(String, String)>,
    pub no_new_privs: bool,
    pub args: Vec<String>,
    pub process: Option<PathBuf>,
}

impl ActivatedContainerBuilder {
    /// pre-validate before building ActivatedContainer
    fn validate(&self) -> Result<(), String> {
        // ensure container exists
        let id = self.id.as_ref().unwrap();
        let root = self.root.as_ref().unwrap();
        let status_path = Status::get_dir_path(root, id);
        if !status_path.exists() {
            return Err(format!(
                "container {} does not exist at path {:?}",
                id, root
            ));
        }

        // ensure argv will not be empty in process exec phase later
        let process = self.process.as_ref().unwrap();
        let args = self.args.as_ref().unwrap();
        if process.is_none() && args.is_empty() {
            return Err("process and args cannot be all empty".to_string());
        }
        Ok(())
    }
}

impl ActivatedContainer {
    /// Create ContainerLauncher that can be used to spawn a process in an existing container.
    /// This reads the spec from status file of an existing container and adapts it with given process file
    /// or other options like args, env, etc. It also changes the namespace in spec to join the container.
    pub fn create_launcher(self, logger: &Logger) -> Result<ContainerLauncher> {
        debug!(
            logger,
            "enter ActivatedContainer::create_launcher {:?}", self
        );
        let container = Container::load(&self.root, &self.id)?;

        // If state is Created or Running, we can execute the process.
        if container.state != ContainerState::Created && container.state != ContainerState::Running
        {
            return Err(anyhow!(
                "cannot exec in a stopped or paused container, state: {:?}",
                container.state
            ));
        }

        let mut config = container.status.config;
        let spec = config.spec.as_mut().unwrap();
        self.adapt_exec_spec(spec, container.status.pid, logger)?;
        debug!(logger, "adapted spec: {:?}", spec);
        validate_spec(spec, &self.console_socket)?;

        debug!(logger, "create LinuxContainer with config: {:?}", config);
        // Maybe we should move some properties from status into LinuxContainer,
        // like pid, process_start_time, created, cgroup_manager, etc. But it works now.
        let runner =
            create_linux_container(&self.id, &self.root, config, self.console_socket, logger)?;

        Ok(ContainerLauncher::new(
            &self.id,
            &container.status.bundle,
            &self.root,
            false,
            runner,
            self.pid_file,
        ))
    }

    /// Adapt spec to execute a new process which will join the container.
    fn adapt_exec_spec(&self, spec: &mut Spec, pid: i32, logger: &Logger) -> Result<()> {
        // If with --process, load process from file.
        // Otherwise, update process with args and other options.
        if let Some(process_path) = self.process.as_ref() {
            spec.process = Some(Self::get_process(process_path)?);
        } else if let Some(process) = spec.process.as_mut() {
            self.update_process(process)?;
        } else {
            return Err(anyhow!("process is empty in spec"));
        };
        // Exec process will join the container's namespaces
        update_namespaces(logger, spec, pid)?;
        Ok(())
    }

    /// Update process with args and other options.
    fn update_process(&self, process: &mut OCIProcess) -> Result<()> {
        process.args = self.args.clone();
        process.no_new_privileges = self.no_new_privs;
        process.terminal = self.tty;
        if let Some(cwd) = self.cwd.as_ref() {
            process.cwd = cwd.as_path().display().to_string();
        }
        process
            .env
            .extend(self.env.iter().map(|kv| format!("{}={}", kv.0, kv.1)));
        Ok(())
    }

    /// Read and parse OCI Process from path
    fn get_process(process_path: &Path) -> Result<OCIProcess> {
        let f = File::open(process_path)?;
        Ok(serde_json::from_reader(f)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::status::Status;
    use crate::utils::test_utils::*;
    use nix::unistd::getpid;
    use oci::{Linux, LinuxNamespace, User};
    use rustjail::container::TYPETONAME;
    use scopeguard::defer;
    use slog::o;
    use std::{
        fs::{create_dir_all, File},
        path::PathBuf,
    };
    use tempfile::tempdir;
    use test_utils::skip_if_not_root;

    fn create_activated_dirs(root: &Path, id: &str, bundle: &Path) {
        Status::create_dir(root, id).unwrap();
        create_dir_all(bundle.join(TEST_ROOTFS_PATH)).unwrap();
    }

    #[test]
    fn test_activated_container_validate() {
        let root = tempdir().unwrap();
        let id = TEST_CONTAINER_ID.to_string();
        Status::create_dir(root.path(), &id).unwrap();
        let result = ActivatedContainerBuilder::default()
            .id(id)
            .root(root.into_path())
            .console_socket(None)
            .pid_file(None)
            .tty(false)
            .cwd(None)
            .env(Vec::new())
            .no_new_privs(false)
            .process(None)
            .args(vec!["sleep".to_string(), "10".to_string()])
            .build();
        assert!(result.is_ok());
    }

    #[test]
    fn test_activated_container_create() {
        // create cgroup directory needs root permission
        skip_if_not_root!();
        let logger = slog::Logger::root(slog::Discard, o!());
        let bundle_dir = tempdir().unwrap();
        let root = tempdir().unwrap();
        // Since tests are executed concurrently, container_id must be unique in tests with cgroup.
        // Or the cgroup directory may be removed by other tests in advance.
        let id = "test_activated_container_create".to_string();
        create_activated_dirs(root.path(), &id, bundle_dir.path());
        let pid = getpid().as_raw();

        let mut spec = create_dummy_spec();
        spec.root.as_mut().unwrap().path = bundle_dir
            .path()
            .join(TEST_ROOTFS_PATH)
            .to_string_lossy()
            .to_string();

        let status = create_custom_dummy_status(&id, pid, root.path(), &spec);
        status.save().unwrap();

        // create empty cgroup directory to avoid is_pause failing
        let cgroup = create_dummy_cgroup(Path::new(id.as_str()));
        defer!(cgroup.delete().unwrap());

        let result = ActivatedContainerBuilder::default()
            .id(id)
            .root(root.into_path())
            .console_socket(Some(PathBuf::from(TEST_CONSOLE_SOCKET_PATH)))
            .pid_file(Some(PathBuf::from(TEST_PID_FILE_PATH)))
            .tty(true)
            .cwd(Some(PathBuf::from(TEST_BUNDLE_PATH)))
            .env(vec![
                ("K1".to_string(), "V1".to_string()),
                ("K2".to_string(), "V2".to_string()),
            ])
            .no_new_privs(true)
            .process(None)
            .args(vec!["sleep".to_string(), "10".to_string()])
            .build()
            .unwrap();

        let linux = Linux {
            namespaces: TYPETONAME
                .iter()
                .filter(|&(_, &name)| name != "user")
                .map(|ns| LinuxNamespace {
                    r#type: ns.0.to_string(),
                    path: format!("/proc/{}/ns/{}", pid, ns.1),
                })
                .collect(),
            ..Default::default()
        };
        spec.linux = Some(linux);
        spec.process = Some(OCIProcess {
            terminal: result.tty,
            console_size: None,
            user: User::default(),
            args: result.args.clone(),
            cwd: result.cwd.clone().unwrap().to_string_lossy().to_string(),
            env: vec![
                "PATH=/bin:/usr/bin".to_string(),
                "K1=V1".to_string(),
                "K2=V2".to_string(),
            ],
            capabilities: None,
            rlimits: Vec::new(),
            no_new_privileges: result.no_new_privs,
            apparmor_profile: "".to_string(),
            oom_score_adj: None,
            selinux_label: "".to_string(),
        });
        let launcher = result.clone().create_launcher(&logger).unwrap();
        assert!(!launcher.init);
        assert_eq!(launcher.runner.config.spec.unwrap(), spec);
        assert_eq!(
            launcher.runner.console_socket,
            result.console_socket.unwrap()
        );
        assert_eq!(launcher.pid_file, result.pid_file);
    }

    #[test]
    fn test_activated_container_create_with_process() {
        // create cgroup directory needs root permission
        skip_if_not_root!();
        let bundle_dir = tempdir().unwrap();
        let process_file = bundle_dir.path().join(TEST_PROCESS_FILE_NAME);
        let process_template = OCIProcess {
            args: vec!["sleep".to_string(), "10".to_string()],
            cwd: "/".to_string(),
            ..Default::default()
        };
        let file = File::create(process_file.clone()).unwrap();
        serde_json::to_writer(&file, &process_template).unwrap();

        let logger = slog::Logger::root(slog::Discard, o!());
        let root = tempdir().unwrap();
        // Since tests are executed concurrently, container_id must be unique in tests with cgroup.
        // Or the cgroup directory may be removed by other tests in advance.
        let id = "test_activated_container_create_with_process".to_string();
        let pid = getpid().as_raw();
        let mut spec = create_dummy_spec();
        spec.root.as_mut().unwrap().path = bundle_dir
            .path()
            .join(TEST_ROOTFS_PATH)
            .to_string_lossy()
            .to_string();
        create_activated_dirs(root.path(), &id, bundle_dir.path());

        let status = create_custom_dummy_status(&id, pid, root.path(), &spec);
        status.save().unwrap();
        // create empty cgroup directory to avoid is_pause failing
        let cgroup = create_dummy_cgroup(Path::new(id.as_str()));
        defer!(cgroup.delete().unwrap());

        let launcher = ActivatedContainerBuilder::default()
            .id(id)
            .root(root.into_path())
            .console_socket(None)
            .pid_file(None)
            .tty(true)
            .cwd(Some(PathBuf::from(TEST_BUNDLE_PATH)))
            .env(vec![
                ("K1".to_string(), "V1".to_string()),
                ("K2".to_string(), "V2".to_string()),
            ])
            .no_new_privs(true)
            .process(Some(process_file))
            .args(vec!["sleep".to_string(), "10".to_string()])
            .build()
            .unwrap()
            .create_launcher(&logger)
            .unwrap();

        assert!(!launcher.init);

        assert_eq!(
            launcher.runner.config.spec.unwrap().process.unwrap(),
            process_template
        );
    }
}
