// Copyright 2021-2022 Sony Group Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::container::{get_config_path, ContainerContext};
use anyhow::{anyhow, Result};
use derive_builder::Builder;
use oci::Spec;
use std::path::{Path, PathBuf};

#[derive(Default, Builder, Debug)]
pub struct Container {
    id: String,
    bundle: PathBuf,
    root: PathBuf,
    console_socket: Option<PathBuf>,
}

impl Container {
    pub fn create_ctx(self) -> Result<ContainerContext> {
        let bundle_canon = self.bundle.canonicalize()?;
        let config_path = get_config_path(&bundle_canon);
        let mut spec = Spec::load(
            config_path
                .to_str()
                .ok_or_else(|| anyhow!("invalid config path"))?,
        )?;

        if spec.root.is_some() {
            let mut spec_root = spec
                .root
                .as_mut()
                .ok_or_else(|| anyhow!("root config was not present in the spec file"))?;
            let rootfs_path = Path::new(&spec_root.path);

            // If the rootfs path in the spec file is a relative path,
            // convert it into a canonical path to pass validation of rootfs in the agent.
            if !&rootfs_path.is_absolute() {
                let rootfs_name = rootfs_path
                    .file_name()
                    .ok_or_else(|| anyhow!("invalid rootfs name"))?;
                spec_root.path = bundle_canon
                    .join(rootfs_name)
                    .to_str()
                    .map(|s| s.to_string())
                    .ok_or_else(|| anyhow!("failed to convert bundle path"))?;
            }
        }

        Ok(ContainerContext {
            id: self.id,
            bundle: self.bundle,
            state_root: self.root,
            spec,
            // TODO: liboci-cli does not support --no-pivot option for create and run command.
            // After liboci-cli supports the option, we will change the following code.
            // no_pivot_root: self.no_pivot,
            no_pivot_root: false,
            console_socket: self.console_socket,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::container::CONFIG_FILE_NAME;
    use oci::Spec;
    use std::{fs::File, path::PathBuf};
    use tempfile::tempdir;

    #[derive(Debug)]
    struct TestData {
        id: String,
        bundle: PathBuf,
        root: PathBuf,
        console_socket: Option<PathBuf>,
        spec: Spec,
        no_pivot_root: bool,
    }

    #[test]
    fn test_create_ctx() {
        let bundle_dir = tempdir().unwrap();
        let config_file = bundle_dir.path().join(CONFIG_FILE_NAME);
        let spec = Spec::default();
        let file = File::create(config_file).unwrap();
        serde_json::to_writer(&file, &spec).unwrap();

        let test_data = TestData {
            id: String::from("test"),
            bundle: PathBuf::from(bundle_dir.into_path()),
            root: PathBuf::from("test"),
            console_socket: Some(PathBuf::from("test")),
            spec: Spec::default(),
            no_pivot_root: false,
        };

        let test_ctx = ContainerContext {
            id: test_data.id.clone(),
            bundle: test_data.bundle.clone(),
            state_root: test_data.root.clone(),
            spec: test_data.spec.clone(),
            no_pivot_root: test_data.no_pivot_root,
            console_socket: test_data.console_socket.clone(),
        };

        let ctx = ContainerBuilder::default()
            .id(test_data.id.clone())
            .bundle(test_data.bundle.clone())
            .root(test_data.root.clone())
            .console_socket(test_data.console_socket.clone())
            .build()
            .unwrap()
            .create_ctx()
            .unwrap();

        assert_eq!(test_ctx, ctx);
    }
}
