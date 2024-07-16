// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::{
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
};

use crate::{Args, Error};
use anyhow::{anyhow, Context, Result};
use oci_spec::runtime as oci;
use runtime_spec as spec;
use sha2::Digest;
const SOCKET_ROOT: &str = "/run/containerd";
const SHIM_PID_FILE: &str = "shim.pid";

pub(crate) const ENV_KATA_RUNTIME_BIND_FD: &str = "KATA_RUNTIME_BIND_FD";

/// Command executor for shim.
#[derive(Debug)]
pub struct ShimExecutor {
    pub(crate) args: Args,
}

impl ShimExecutor {
    /// Create a new instance of [`Shim`].
    pub fn new(args: Args) -> Self {
        ShimExecutor { args }
    }

    pub(crate) fn load_oci_spec(&self, path: &Path) -> Result<oci::Spec> {
        let spec_file = path.join(spec::OCI_SPEC_CONFIG_FILE_NAME);
        oci::Spec::load(spec_file.to_str().unwrap_or_default()).context("load spec")
    }

    pub(crate) fn write_address(&self, path: &Path, address: &Path) -> Result<()> {
        let file_path = &path.join("address");
        std::fs::write(file_path, address.as_os_str().as_bytes())
            .context(Error::FileWrite(format!("{:?}", &file_path)))
    }

    pub(crate) fn write_pid_file(&self, path: &Path, pid: u32) -> Result<()> {
        let file_path = &path.join(SHIM_PID_FILE);
        std::fs::write(file_path, format!("{}", pid))
            .context(Error::FileWrite(format!("{:?}", &file_path)))
    }

    // There may be a multi-container for a Pod, each container has a bundle path, we need to write
    // the PID to the file for each container in their own bundle path, so we can directly get the
    // `bundle_path()` and write the PID.
    // While the real runtime process's PID is stored in the file in the sandbox container's bundle
    // path, so needs to read from the sandbox container's bundle path.
    pub(crate) fn read_pid_file(&self, path: &Path) -> Result<u32> {
        let file_path = path.join(SHIM_PID_FILE);
        let data = std::fs::read_to_string(&file_path)
            .context(Error::FileOpen(format!("{:?}", file_path)))?;

        data.parse::<u32>().context(Error::ParsePid)
    }

    pub(crate) fn socket_address(&self, id: &str) -> Result<PathBuf> {
        if id.is_empty() {
            return Err(anyhow!(Error::EmptySandboxId));
        }

        let data = [&self.args.address, &self.args.namespace, id].join("/");
        let mut hasher = sha2::Sha256::new();
        hasher.update(data);
        // https://github.com/containerd/containerd/blob/v1.6.8/runtime/v2/shim/util_unix.go#L68 to
        // generate a shim socket path.
        Ok(PathBuf::from(format!(
            "unix://{}/s/{:X}",
            SOCKET_ROOT,
            hasher.finalize()
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    use kata_sys_util::spec::get_bundle_path;

    #[test]
    #[serial]
    fn test_shim_executor() {
        let dir = tempfile::tempdir().unwrap();
        let bundle_path = dir.path();
        std::env::set_current_dir(bundle_path).unwrap();

        let args = Args {
            id: "default_id".into(),
            namespace: "default_namespace".into(),
            address: "default_address".into(),
            publish_binary: "containerd".into(),
            bundle: bundle_path.to_str().unwrap().into(),
            ..Default::default()
        };

        let executor = ShimExecutor::new(args);

        executor
            .write_address(bundle_path, Path::new("12345"))
            .unwrap();
        let dir = get_bundle_path().unwrap();
        let file_path = &dir.join("address");
        let buf = std::fs::read_to_string(file_path).unwrap();
        assert_eq!(&buf, "12345");

        executor.write_pid_file(&dir, 1267).unwrap();
        let read_pid = executor.read_pid_file(&dir).unwrap();
        assert_eq!(read_pid, 1267);
    }
}
