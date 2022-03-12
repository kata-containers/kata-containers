// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::{
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
};

use anyhow::{anyhow, Context, Result};
use sha2::Digest;

use crate::{Args, Error};

const SOCKET_ROOT: &str = "/run/containerd";
const SHIM_PID_FILE: &str = "shim.pid";

pub(crate) const ENV_KATA_RUNTIME_BIND_FD: &str = "KATA_RUNTIME_BIND_FD";

/// Command executor for shim.
pub struct ShimExecutor {
    pub(crate) args: Args,
}

impl ShimExecutor {
    /// Create a new instance of [`Shim`].
    pub fn new(args: Args) -> Self {
        ShimExecutor { args }
    }

    pub(crate) fn load_oci_spec(&self) -> Result<oci::Spec> {
        let bundle_path = self.get_bundle_path()?;
        let spec_file = bundle_path.join("config.json");

        oci::Spec::load(spec_file.to_str().unwrap_or_default()).context("load spec")
    }

    pub(crate) fn write_address(&self, address: &Path) -> Result<()> {
        let dir = self.get_bundle_path()?;
        let file_path = &dir.join("address");
        std::fs::write(file_path, address.as_os_str().as_bytes())
            .context(Error::FileWrite(format!("{:?}", &file_path)))
    }

    pub(crate) fn write_pid_file(&self, pid: u32) -> Result<()> {
        let dir = self.get_bundle_path()?;
        let file_path = &dir.join(SHIM_PID_FILE);
        std::fs::write(file_path, format!("{}", pid))
            .context(Error::FileWrite(format!("{:?}", &file_path)))
    }

    pub(crate) fn read_pid_file(&self, bundle_path: &Path) -> Result<u32> {
        let file_path = bundle_path.join(SHIM_PID_FILE);
        let data = std::fs::read_to_string(&file_path)
            .context(Error::FileOpen(format!("{:?}", file_path)))?;

        data.parse::<u32>().context(Error::ParsePid)
    }

    pub(crate) fn get_bundle_path(&self) -> Result<PathBuf> {
        std::env::current_dir().context(Error::GetBundlePath)
    }

    pub(crate) fn socket_address(&self, id: &str) -> Result<PathBuf> {
        if id.is_empty() {
            return Err(anyhow!(Error::EmptySandboxId));
        }

        let data = [&self.args.address, &self.args.namespace, id].join("/");
        let mut hasher = sha2::Sha256::new();
        hasher.update(data);
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

    #[test]
    #[serial]
    fn test_shim_executor() {
        let dir = tempfile::tempdir().unwrap();
        let bundle_path = dir.path();
        std::env::set_current_dir(bundle_path).unwrap();

        let args = Args {
            id: "1dfc0567".to_string(),
            namespace: "test_namespace".into(),
            address: "containerd_socket".into(),
            publish_binary: "containerd".into(),
            socket: "socket".into(),
            bundle: bundle_path.to_str().unwrap().into(),
            debug: false,
        };

        let executor = ShimExecutor::new(args);

        executor.write_address(Path::new("12345")).unwrap();
        let dir = executor.get_bundle_path().unwrap();
        let file_path = &dir.join("address");
        let buf = std::fs::read_to_string(file_path).unwrap();
        assert_eq!(&buf, "12345");

        executor.write_pid_file(1267).unwrap();
        let read_pid = executor.read_pid_file(&dir).unwrap();
        assert_eq!(read_pid, 1267);
    }
}
