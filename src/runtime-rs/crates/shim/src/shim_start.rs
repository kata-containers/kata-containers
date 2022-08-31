// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::{
    fs,
    io::Write,
    os::unix::{io::IntoRawFd, prelude::OsStrExt},
    path::{Path, PathBuf},
};

use anyhow::{anyhow, Context, Result};
use kata_sys_util::spec::get_bundle_path;
use kata_types::{container::ContainerType, k8s};
use unix_socket::UnixListener;

use crate::{
    shim::{ShimExecutor, ENV_KATA_RUNTIME_BIND_FD},
    Error,
};

impl ShimExecutor {
    pub fn start(&mut self) -> Result<()> {
        self.args.validate(false).context("validate")?;

        let address = self.do_start().context("do start")?;
        std::io::stdout()
            .write_all(address.as_os_str().as_bytes())
            .context("failed to write stdout")?;
        Ok(())
    }

    fn do_start(&mut self) -> Result<PathBuf> {
        let bundle_path = get_bundle_path().context("get bundle path")?;
        let spec = self.load_oci_spec(&bundle_path)?;
        let (container_type, id) = k8s::container_type_with_id(&spec);

        match container_type {
            ContainerType::PodSandbox => {
                let address = self.socket_address(&self.args.id)?;
                let socket = new_listener(&address)?;
                let child_pid = self.create_shim_process(socket)?;
                self.write_pid_file(&bundle_path, child_pid)?;
                self.write_address(&bundle_path, &address)?;
                Ok(address)
            }
            ContainerType::PodContainer => {
                let sid = id
                    .ok_or(Error::InvalidArgument)
                    .context("get sid for container")?;
                let (address, pid) = self.get_shim_info_from_sandbox(&sid)?;
                self.write_pid_file(&bundle_path, pid)?;
                self.write_address(&bundle_path, &address)?;
                Ok(address)
            }
        }
    }

    fn new_command(&self) -> Result<std::process::Command> {
        if self.args.id.is_empty()
            || self.args.namespace.is_empty()
            || self.args.address.is_empty()
            || self.args.publish_binary.is_empty()
        {
            return Err(anyhow!("invalid param"));
        }

        let bundle_path = get_bundle_path().context("get bundle path")?;
        let self_exec = std::env::current_exe().map_err(Error::SelfExec)?;
        let mut command = std::process::Command::new(self_exec);

        command
            .current_dir(bundle_path)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .arg("-id")
            .arg(&self.args.id)
            .arg("-namespace")
            .arg(&self.args.namespace)
            .arg("-address")
            .arg(&self.args.address)
            .arg("-publish-binary")
            .arg(&self.args.publish_binary)
            .env("RUST_BACKTRACE", "1");

        if self.args.debug {
            command.arg("-debug");
        }

        Ok(command)
    }

    fn create_shim_process<T: IntoRawFd>(&self, socket: T) -> Result<u32> {
        let mut cmd = self.new_command().context("new command")?;
        cmd.env(
            ENV_KATA_RUNTIME_BIND_FD,
            format!("{}", socket.into_raw_fd()),
        );
        let child = cmd
            .spawn()
            .map_err(Error::SpawnChild)
            .context("spawn child")?;

        Ok(child.id())
    }

    fn get_shim_info_from_sandbox(&self, sandbox_id: &str) -> Result<(PathBuf, u32)> {
        // All containers of a pod share the same pod socket address.
        let address = self.socket_address(sandbox_id).context("socket address")?;
        let bundle_path = get_bundle_path().context("get bundle path")?;
        let parent_bundle_path = Path::new(&bundle_path)
            .parent()
            .unwrap_or_else(|| Path::new(""));
        let sandbox_bundle_path = parent_bundle_path
            .join(sandbox_id)
            .canonicalize()
            .context(Error::GetBundlePath)?;
        let pid = self.read_pid_file(&sandbox_bundle_path)?;

        Ok((address, pid))
    }
}

fn new_listener(address: &Path) -> Result<UnixListener> {
    let trim_path = address.strip_prefix("unix:").context("trim path")?;
    let file_path = Path::new("/").join(trim_path);
    let file_path = file_path.as_path();
    if let Some(parent_dir) = file_path.parent() {
        fs::create_dir_all(parent_dir).context("create parent dir")?;
    }

    UnixListener::bind(file_path).context("bind address")
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use serial_test::serial;
    use tests_utils::gen_id;

    use super::*;
    use crate::Args;

    #[test]
    #[serial]
    fn test_new_command() {
        let dir = tempfile::tempdir().unwrap();
        let bundle_path = dir.path();
        std::env::set_current_dir(bundle_path).unwrap();

        let args = Args {
            id: "sandbox1".into(),
            namespace: "ns".into(),
            address: "address".into(),
            publish_binary: "containerd".into(),
            bundle: bundle_path.to_str().unwrap().into(),
            debug: false,
        };
        let mut executor = ShimExecutor::new(args);

        let cmd = executor.new_command().unwrap();
        assert_eq!(cmd.get_args().len(), 8);
        assert_eq!(cmd.get_envs().len(), 1);
        assert_eq!(cmd.get_current_dir().unwrap(), get_bundle_path().unwrap());

        executor.args.debug = true;
        let cmd = executor.new_command().unwrap();
        assert_eq!(cmd.get_args().len(), 9);
        assert_eq!(cmd.get_envs().len(), 1);
        assert_eq!(cmd.get_current_dir().unwrap(), get_bundle_path().unwrap());
    }

    #[test]
    #[serial]
    fn test_get_info_from_sandbox() {
        let dir = tempfile::tempdir().unwrap();
        let sandbox_id = gen_id(16);
        let bundle_path = &dir.path().join(&sandbox_id);
        std::fs::create_dir(bundle_path).unwrap();
        std::env::set_current_dir(bundle_path).unwrap();

        let args = Args {
            id: sandbox_id.to_owned(),
            namespace: "ns1".into(),
            address: "containerd_socket".into(),
            publish_binary: "containerd".into(),
            bundle: bundle_path.to_str().unwrap().into(),
            debug: false,
        };
        let executor = ShimExecutor::new(args);

        let addr = executor.socket_address(&executor.args.id).unwrap();
        executor.write_address(bundle_path, &addr).unwrap();
        executor.write_pid_file(bundle_path, 1267).unwrap();

        let container_id = gen_id(16);
        let bundle_path2 = &dir.path().join(&container_id);
        std::fs::create_dir(bundle_path2).unwrap();
        std::env::set_current_dir(bundle_path2).unwrap();

        let args = Args {
            id: container_id,
            namespace: "ns1".into(),
            address: "containerd_socket".into(),
            publish_binary: "containerd".into(),
            bundle: bundle_path2.to_str().unwrap().into(),
            debug: false,
        };
        let executor2 = ShimExecutor::new(args);

        let (address, pid) = executor2.get_shim_info_from_sandbox(&sandbox_id).unwrap();

        assert_eq!(pid, 1267);
        assert_eq!(&address, &addr);
    }

    #[test]
    #[serial]
    fn test_new_listener() {
        let path = "/tmp/aaabbbccc";
        let uds_path = format!("unix://{}", path);
        std::fs::remove_file(path).ok();

        let _ = new_listener(Path::new(&uds_path)).unwrap();
        std::fs::remove_file(path).ok();
    }
}
