// Copyright 2022 Sony Group Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::container::{load_linux_container, Container, ContainerLauncher};
use anyhow::{anyhow, Result};
use derive_builder::Builder;
use runtime_spec::ContainerState;
use slog::{debug, Logger};
use std::path::PathBuf;

/// Used for start command. It will prepare the options used for starting a new container.
#[derive(Default, Builder, Debug, Clone)]
#[builder(build_fn(validate = "Self::validate"))]
pub struct CreatedContainer {
    id: String,
    root: PathBuf,
}

impl CreatedContainerBuilder {
    /// pre-validate before building CreatedContainer
    fn validate(&self) -> Result<(), String> {
        // ensure container exists
        let id = self.id.as_ref().unwrap();
        let root = self.root.as_ref().unwrap();
        let path = root.join(id);
        if !path.as_path().exists() {
            return Err(format!("container {} does not exist", id));
        }

        Ok(())
    }
}

impl CreatedContainer {
    /// Create ContainerLauncher that can be used to start a process from an existing init container.
    /// It reads the spec from status file of the init container.
    pub fn create_launcher(self, logger: &Logger) -> Result<ContainerLauncher> {
        debug!(logger, "enter CreatedContainer::create_launcher {:?}", self);
        let container = Container::load(&self.root, &self.id)?;

        if container.state != ContainerState::Created {
            return Err(anyhow!(
                "cannot start a container in the {:?} state",
                container.state
            ));
        }

        let config = container.status.config.clone();

        debug!(
            logger,
            "Prepare LinuxContainer for starting with config: {:?}", config
        );
        let runner = load_linux_container(&container.status, None, logger)?;

        Ok(ContainerLauncher::new(
            &self.id,
            &container.status.bundle,
            &self.root,
            true,
            runner,
            None,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::status::Status;
    use crate::utils::test_utils::*;
    use nix::sys::stat::Mode;
    use nix::unistd::{self, getpid};
    use rustjail::container::EXEC_FIFO_FILENAME;
    use scopeguard::defer;
    use slog::o;
    use std::fs::create_dir_all;
    use std::path::Path;
    use tempfile::tempdir;
    use test_utils::skip_if_not_root;

    fn create_created_container_dirs(root: &Path, id: &str, bundle: &Path) {
        Status::create_dir(root, id).unwrap();
        let fifo = root.join(id).join(EXEC_FIFO_FILENAME);
        unistd::mkfifo(&fifo, Mode::from_bits(0o644).unwrap()).unwrap();
        create_dir_all(bundle.join(TEST_ROOTFS_PATH)).unwrap();
    }

    #[test]
    fn test_created_container_validate() {
        let root = tempdir().unwrap();
        let id = TEST_CONTAINER_ID.to_string();
        let result = CreatedContainerBuilder::default()
            .id(id)
            .root(root.path().to_path_buf())
            .build();
        assert!(result.is_err());
    }

    #[test]
    fn test_created_container_create_launcher() {
        // create cgroup directory needs root permission
        skip_if_not_root!();
        let logger = slog::Logger::root(slog::Discard, o!());
        let bundle_dir = tempdir().unwrap();
        let root = tempdir().unwrap();
        // Since tests are executed concurrently, container_id must be unique in tests with cgroup.
        // Or the cgroup directory may be removed by other tests in advance.
        let id = "test_created_container_create".to_string();
        create_created_container_dirs(root.path(), &id, bundle_dir.path());
        let pid = getpid().as_raw();

        let mut spec = create_dummy_spec();
        spec.root_mut()
            .as_mut()
            .unwrap()
            .set_path(bundle_dir.path().join(TEST_ROOTFS_PATH));

        let status = create_custom_dummy_status(&id, pid, root.path(), &spec);
        status.save().unwrap();

        // create empty cgroup directory to avoid is_pause failing
        let cgroup = create_dummy_cgroup(Path::new(id.as_str()));
        defer!(cgroup.delete().unwrap());

        let launcher = CreatedContainerBuilder::default()
            .id(id.clone())
            .root(root.into_path())
            .build()
            .unwrap()
            .create_launcher(&logger)
            .unwrap();

        assert!(launcher.init);
        assert_eq!(launcher.runner.config.spec.unwrap(), spec);
        assert_eq!(launcher.runner.id, id);
    }
}
