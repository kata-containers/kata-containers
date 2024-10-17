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

        let mut container_type = ContainerType::PodSandbox;
        let mut id = None;

        if let Ok(spec) = self.load_oci_spec(&bundle_path) {
            (container_type, id) = k8s::container_type_with_id(&spec);
        }

        match container_type {
            ContainerType::PodSandbox | ContainerType::SingleContainer => {
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
                let address = self.socket_address(&sid).context("socket address")?;
                self.write_address(&bundle_path, &address)?;
                Ok(address)
            }
        }
    }

    fn new_command(&self) -> Result<std::process::Command> {
        if self.args.id.is_empty()
            || self.args.namespace.is_empty()
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

    use super::*;
    use crate::Args;

    #[test]
    #[serial]
    fn test_new_command() {
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
    fn test_new_listener() {
        let path = "/tmp/aaabbbccc";
        let uds_path = format!("unix://{}", path);
        std::fs::remove_file(path).ok();

        let _ = new_listener(Path::new(&uds_path)).unwrap();
        std::fs::remove_file(path).ok();
    }
}
