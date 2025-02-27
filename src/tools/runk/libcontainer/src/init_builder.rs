// Copyright 2021-2022 Sony Group Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::container::{create_linux_container, get_config_path, ContainerLauncher};
use crate::status::Status;
use crate::utils::{canonicalize_spec_root, validate_spec};
use anyhow::{anyhow, Result};
use derive_builder::Builder;
use oci_spec::runtime::Spec;
use rustjail::specconv::CreateOpts;
use slog::{debug, Logger};
use std::path::PathBuf;

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
    /// pre-validate before building InitContainer
    fn validate(&self) -> Result<(), String> {
        // ensure container hasn't already been created
        let id = self.id.as_ref().unwrap();
        let root = self.root.as_ref().unwrap();
        let status_path = Status::get_dir_path(root, id);
        if status_path.exists() {
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
            container_name: "".to_string(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::container::CONFIG_FILE_NAME;
    use crate::utils::test_utils::*;
    use oci_spec::runtime::Process;
    use slog::o;
    use std::fs::{create_dir, File};
    use std::path::Path;
    use tempfile::tempdir;

    #[test]
    fn test_init_container_validate() {
        let root = tempdir().unwrap();
        let id = TEST_CONTAINER_ID.to_string();
        Status::create_dir(root.path(), id.as_str()).unwrap();
        let result = InitContainerBuilder::default()
            .id(id)
            .root(root.path().to_path_buf())
            .bundle(PathBuf::from(TEST_BUNDLE_PATH))
            .pid_file(None)
            .console_socket(None)
            .build();
        assert!(result.is_err());
    }

    #[test]
    fn test_init_container_create_launcher() {
        #[cfg(all(target_arch = "powerpc64", target_endian = "little"))]
        skip_if_not_root!();
        let logger = slog::Logger::root(slog::Discard, o!());
        let root_dir = tempdir().unwrap();
        let bundle_dir = tempdir().unwrap();
        // create dummy rootfs
        create_dir(bundle_dir.path().join(TEST_ROOTFS_PATH)).unwrap();
        let config_file = bundle_dir.path().join(CONFIG_FILE_NAME);
        let mut spec = create_dummy_spec();
        let file = File::create(config_file).unwrap();
        serde_json::to_writer(&file, &spec).unwrap();

        spec.root_mut()
            .as_mut()
            .unwrap()
            .set_path(bundle_dir.path().join(TEST_ROOTFS_PATH));
        let test_data = TestContainerData {
            // Since tests are executed concurrently, container_id must be unique in tests with cgroup.
            // Or the cgroup directory may be removed by other tests in advance.
            id: String::from("test_init_container_create_launcher"),
            bundle: bundle_dir.path().to_path_buf(),
            root: root_dir.into_path(),
            console_socket: Some(PathBuf::from(TEST_CONSOLE_SOCKET_PATH)),
            config: CreateOpts {
                spec: Some(spec),
                ..Default::default()
            },
            pid_file: Some(PathBuf::from(TEST_PID_FILE_PATH)),
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

        let mut spec = Spec::default();
        spec.set_process(Some(Process::default()));
        spec.process_mut()
            .as_mut()
            .unwrap()
            .set_terminal(Some(true));

        let file = File::create(config_file).unwrap();
        serde_json::to_writer(&file, &spec).unwrap();

        let test_data = TestContainerData {
            id: String::from(TEST_CONTAINER_ID),
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
}
