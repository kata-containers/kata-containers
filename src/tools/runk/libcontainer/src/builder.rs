// Copyright 2021-2022 Sony Group Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::container::{get_config_path, Container, ContainerLauncher};
use crate::utils::validate_process_spec;
use anyhow::{anyhow, Result};
use derive_builder::Builder;
use oci::{ContainerState, Process as OCIProcess, Spec};
use rustjail::container::update_namespaces;
use rustjail::{container::LinuxContainer, specconv::CreateOpts};
use slog::{debug, Logger};
use std::fs::File;
use std::path::{Path, PathBuf};

/// Used for create and run commands. It will prepare the options used for creating a new container.
#[derive(Default, Builder, Debug, Clone)]
#[builder(build_fn(validate = "Self::validate"))]
pub struct InitContainer {
    id: String,
    bundle: PathBuf,
    root: PathBuf,
    console_socket: Option<PathBuf>,
    pid_file: Option<PathBuf>,
}

impl InitContainerBuilder {
    /// Pre-validate before building InitContainer
    fn validate(&self) -> Result<(), String> {
        // ensure container hasn't already been created
        let id = self.id.as_ref().unwrap();
        let root = self.root.as_ref().unwrap();
        let path = root.join(id);
        if path.as_path().exists() {
            return Err(format!(
                "container {} already exists at path {:?}",
                id, root
            ));
        }
        Ok(())
    }
}

impl InitContainer {
    /// Create ContainerLauncher that can be used to launch a new container.
    /// It will read the spec under bundle path.
    pub fn create_launcher(self, logger: &Logger) -> Result<ContainerLauncher> {
        debug!(logger, "enter InitContainer::create_launcher {:?}", self);
        let bundle_canon = self.bundle.canonicalize()?;
        let config_path = get_config_path(&bundle_canon);
        let mut spec = Spec::load(
            config_path
                .to_str()
                .ok_or_else(|| anyhow!("invalid config path"))?,
        )?;
        // Only absolute rootfs path is valid when creating LinuxContainer later.
        canonicalize_spec_root(&mut spec, &bundle_canon)?;
        debug!(logger, "load spec from config file: {:?}", spec);
        validate_spec(&spec, &self.console_socket)?;

        let config = CreateOpts {
            cgroup_name: "".to_string(),
            use_systemd_cgroup: false,
            // TODO: liboci-cli does not support --no-pivot option for create and run command.
            // After liboci-cli supports the option, we will change the following code.
            // no_pivot_root: self.no_pivot,
            no_pivot_root: false,
            no_new_keyring: false,
            spec: Some(spec),
            rootless_euid: false,
            rootless_cgroup: false,
        };
        debug!(logger, "create LinuxContainer with config: {:?}", config);
        let container =
            create_linux_container(&self.id, &self.root, config, self.console_socket, logger)?;

        Ok(ContainerLauncher::new(
            &self.id,
            &bundle_canon,
            &self.root,
            true,
            container,
            self.pid_file,
        ))
    }
}

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
        let path = root.join(id);
        if !path.as_path().exists() {
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
    /// It reads the spec from status file of an existing container, and adapted it with given process file
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

/// If root in spec is a relative path, make it absolute.
fn canonicalize_spec_root(spec: &mut Spec, bundle_canon: &Path) -> Result<()> {
    let mut spec_root = spec
        .root
        .as_mut()
        .ok_or_else(|| anyhow!("root config was not present in the spec file"))?;
    let rootfs_path = Path::new(&spec_root.path);
    if !rootfs_path.is_absolute() {
        spec_root.path = bundle_canon
            .join(rootfs_path)
            .canonicalize()?
            .to_str()
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow!("failed to convert a rootfs path into a canonical path"))?;
    }
    Ok(())
}

fn create_linux_container(
    id: &str,
    root: &Path,
    config: CreateOpts,
    console_socket: Option<PathBuf>,
    logger: &Logger,
) -> Result<LinuxContainer> {
    let mut container = LinuxContainer::new(
        id,
        root.to_str()
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow!("failed to convert bundle path"))?
            .as_str(),
        config,
        logger,
    )?;
    if let Some(socket_path) = console_socket.as_ref() {
        container.set_console_socket(socket_path)?;
    }
    Ok(container)
}

/// Check whether spec is valid. Now runk only support detach mode.
pub fn validate_spec(spec: &Spec, console_socket: &Option<PathBuf>) -> Result<()> {
    validate_process_spec(&spec.process)?;
    if let Some(process) = spec.process.as_ref() {
        // runk always launches containers with detached mode, so users have to
        // use a console socket with run or create operation when a terminal is used.
        if process.terminal && console_socket.is_none() {
            return Err(anyhow!(
                "cannot allocate a pseudo-TTY without setting a console socket"
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::container::CONFIG_FILE_NAME;
    use crate::status::Status;
    use crate::utils::test_utils::*;
    use chrono::DateTime;
    use nix::unistd::getpid;
    use oci::{self, Root, Spec};
    use oci::{Linux, LinuxNamespace, User};
    use rustjail::container::TYPETONAME;
    use scopeguard::defer;
    use slog::o;
    use std::fs::create_dir;
    use std::time::SystemTime;
    use std::{
        fs::{create_dir_all, File},
        path::PathBuf,
    };
    use tempfile::tempdir;
    use test_utils::skip_if_not_root;

    #[derive(Debug)]
    struct TestData {
        id: String,
        bundle: PathBuf,
        root: PathBuf,
        console_socket: Option<PathBuf>,
        pid_file: Option<PathBuf>,
        config: CreateOpts,
    }

    #[test]
    fn test_init_container_validate() {
        let root = tempdir().unwrap();
        let id = "test".to_string();
        Status::create_dir(root.path(), id.as_str()).unwrap();
        let result = InitContainerBuilder::default()
            .id(id)
            .root(root.path().to_path_buf())
            .bundle(PathBuf::from("test"))
            .pid_file(None)
            .console_socket(None)
            .build();
        assert!(result.is_err());
    }

    #[test]
    fn test_init_container_create_launcher() {
        let logger = slog::Logger::root(slog::Discard, o!());
        let root_dir = tempdir().unwrap();
        let bundle_dir = tempdir().unwrap();
        // create dummy rootfs
        create_dir(bundle_dir.path().join("rootfs")).unwrap();
        let config_file = bundle_dir.path().join(CONFIG_FILE_NAME);
        let mut spec = create_dummy_spec();
        let file = File::create(config_file).unwrap();
        serde_json::to_writer(&file, &spec).unwrap();

        spec.root.as_mut().unwrap().path = bundle_dir
            .path()
            .join(TEST_ROOTFS_PATH)
            .to_string_lossy()
            .to_string();
        let test_data = TestData {
            // Since tests are executed concurrently, container_id must be unique in tests with cgroup.
            // Or the cgroup directory may be removed by other tests in advance.
            id: String::from("test_init_container_create_launcher"),
            bundle: bundle_dir.path().to_path_buf(),
            root: root_dir.into_path(),
            console_socket: Some(PathBuf::from("test")),
            config: CreateOpts {
                spec: Some(spec),
                ..Default::default()
            },
            pid_file: Some(PathBuf::from("test")),
        };

        let launcher = InitContainerBuilder::default()
            .id(test_data.id.clone())
            .bundle(test_data.bundle.clone())
            .root(test_data.root.clone())
            .console_socket(test_data.console_socket.clone())
            .pid_file(test_data.pid_file.clone())
            .build()
            .unwrap()
            .create_launcher(&logger)
            .unwrap();

        // LinuxContainer doesn't impl PartialEq, so we need to compare the fields manually.
        assert!(launcher.init);
        assert_eq!(launcher.bundle, test_data.bundle);
        assert_eq!(launcher.state_root, test_data.root);
        assert_eq!(launcher.pid_file, test_data.pid_file);
        assert_eq!(launcher.runner.id, test_data.id);
        assert_eq!(launcher.runner.config.spec, test_data.config.spec);
        assert_eq!(
            Some(launcher.runner.console_socket),
            test_data.console_socket
        );
        // If it is run by root, create_launcher will create cgroup dirs successfully. So we need to do some cleanup stuff.
        if nix::unistd::Uid::effective().is_root() {
            clean_up_cgroup(Path::new(&test_data.id));
        }
    }

    #[test]
    fn test_init_container_tty_err() {
        let logger = slog::Logger::root(slog::Discard, o!());
        let bundle_dir = tempdir().unwrap();
        let config_file = bundle_dir.path().join(CONFIG_FILE_NAME);

        let mut spec = oci::Spec {
            process: Some(oci::Process::default()),
            ..Default::default()
        };
        spec.process.as_mut().unwrap().terminal = true;

        let file = File::create(config_file).unwrap();
        serde_json::to_writer(&file, &spec).unwrap();

        let test_data = TestData {
            id: String::from("test"),
            bundle: bundle_dir.into_path(),
            root: tempdir().unwrap().into_path(),
            console_socket: None,
            config: CreateOpts {
                spec: Some(spec),
                ..Default::default()
            },
            pid_file: None,
        };

        let result = InitContainerBuilder::default()
            .id(test_data.id.clone())
            .bundle(test_data.bundle.clone())
            .root(test_data.root.clone())
            .console_socket(test_data.console_socket.clone())
            .pid_file(test_data.pid_file)
            .build()
            .unwrap()
            .create_launcher(&logger);

        assert!(result.is_err());
    }

    #[test]
    fn test_canonicalize_spec_root() {
        let gen_spec = |p: &str| -> Spec {
            Spec {
                root: Some(Root {
                    path: p.to_string(),
                    readonly: false,
                }),
                ..Default::default()
            }
        };

        let rootfs_name = TEST_ROOTFS_PATH;
        let temp_dir = tempdir().unwrap();
        let bundle_dir = temp_dir.path();
        let abs_root = bundle_dir.join(rootfs_name);
        create_dir_all(abs_root.clone()).unwrap();
        let mut spec = gen_spec(abs_root.to_str().unwrap());
        assert!(canonicalize_spec_root(&mut spec, bundle_dir).is_ok());
        assert_eq!(spec.root.unwrap().path, abs_root.to_str().unwrap());
        let mut spec = gen_spec(rootfs_name);
        assert!(canonicalize_spec_root(&mut spec, bundle_dir).is_ok());
        assert_eq!(spec.root.unwrap().path, abs_root.to_str().unwrap());
    }

    fn create_dummy_spec() -> Spec {
        let linux = oci::Linux {
            namespaces: TYPETONAME
                .iter()
                .filter(|&(_, &name)| name != "user")
                .map(|ns| LinuxNamespace {
                    r#type: ns.0.to_string(),
                    path: "".to_string(),
                })
                .collect(),
            ..Default::default()
        };
        Spec {
            version: "1.0".to_string(),
            process: Some(OCIProcess {
                args: vec!["sleep".to_string(), "10".to_string()],
                env: vec!["PATH=/bin:/usr/bin".to_string()],
                cwd: "/".to_string(),
                ..Default::default()
            }),
            hostname: "runk".to_string(),
            root: Some(Root {
                path: TEST_ROOTFS_PATH.to_string(),
                readonly: false,
            }),
            linux: Some(linux),
            ..Default::default()
        }
    }

    fn create_dummy_status(id: &str, pid: i32, root: &Path, spec: &Spec) -> Status {
        let start_time = procfs::process::Process::new(pid)
            .unwrap()
            .stat()
            .unwrap()
            .starttime;
        Status {
            oci_version: spec.version.clone(),
            id: id.to_string(),
            pid,
            root: root.to_path_buf(),
            bundle: PathBuf::from("/tmp"),
            rootfs: TEST_ROOTFS_PATH.to_string(),
            process_start_time: start_time,
            created: DateTime::from(SystemTime::now()),
            cgroup_manager: serde_json::from_str(TEST_CGM_DATA).unwrap(),
            config: CreateOpts {
                spec: Some(spec.clone()),
                ..Default::default()
            },
        }
    }

    fn create_activated_dirs(root: &Path, id: &str, bundle: &Path) {
        Status::create_dir(root, id).unwrap();
        create_dir_all(bundle.join(TEST_ROOTFS_PATH)).unwrap();
    }

    #[test]
    fn test_activated_container_validate() {
        let root = tempdir().unwrap();
        let id = "test".to_string();
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

        let status = create_dummy_status(&id, pid, root.path(), &spec);
        status.save().unwrap();

        // create empty cgroup directory to avoid is_pause failing
        let cgroup = create_dummy_cgroup(Path::new(id.as_str()));
        defer!(cgroup.delete().unwrap());

        let result = ActivatedContainerBuilder::default()
            .id(id)
            .root(root.into_path())
            .console_socket(Some(PathBuf::from("/var/run/test.sock")))
            .pid_file(Some(PathBuf::from("test")))
            .tty(true)
            .cwd(Some(PathBuf::from("/tmp")))
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
        const PROCESS_FILE_NAME: &str = "process.json";
        let bundle_dir = tempdir().unwrap();
        let process_file = bundle_dir.path().join(PROCESS_FILE_NAME);
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

        let status = create_dummy_status(&id, pid, root.path(), &spec);
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
            .cwd(Some(PathBuf::from("/tmp")))
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
