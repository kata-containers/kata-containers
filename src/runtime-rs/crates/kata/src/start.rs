// Copyright (c) 2021 Alibaba Cloud
// Copyright (c) 2021 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::ffi::{OsStr, OsString};
use std::io::Write;
use std::os::unix::io::IntoRawFd;
use std::os::unix::prelude::OsStrExt;
use std::path::PathBuf;

use unix_socket::UnixListener;

use virtcontainers::spec_info;

use crate::{Error, Result, ShimExecutor, KATA_BIND_FD};

impl ShimExecutor {
    // Implement start subcommnad
    // This functions should be the most outside one for `start` command action
    // So it does not return errors but directly write errors to stderr
    pub fn start(&mut self) {
        if let Err(e) = self.args.validate(false) {
            eprintln!("start shim err: {} with invalid inputs {:?}", e, self.args);
            return;
        }

        match self.do_start() {
            Ok(address) => {
                std::io::stdout()
                    .write_all(address.as_os_str().as_bytes())
                    .expect("failed to write stdout");
            }
            Err(e) => eprintln!("start shim err: {}", e),
        }
    }

    fn do_start(&mut self) -> Result<PathBuf> {
        let spec = self.load_image_spec()?;
        let info = spec_info::container_sandbox_info(&spec).map_err(Error::OciSpecInfo)?;

        match info {
            spec_info::ContainerSandboxInfo::Sandbox => {
                let address = self.socket_address(&self.args.id)?;
                let socket = new_listener(&address)?;
                let child_pid = self.create_shim_process(socket)?;
                self.write_pid_file(&format!("{}", child_pid))?;
                self.write_address(&address)?;
                Ok(address)
            }
            spec_info::ContainerSandboxInfo::Container(sandbox_id) => {
                let (address, pid) = self.get_shim_info_from_sandbox(&sandbox_id)?;
                self.write_pid_file(&pid)?;
                self.write_address(&address)?;
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
            return Err(Error::InvalidArgument);
        }

        let bundle_path = self.get_bundle_path()?;
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
        let mut cmd = self.new_command()?;
        cmd.env(KATA_BIND_FD, format!("{}", socket.into_raw_fd()));
        let child = cmd.spawn().map_err(Error::SpawnChild)?;

        Ok(child.id())
    }

    fn get_shim_info_from_sandbox(&self, sandbox_id: &str) -> Result<(PathBuf, String)> {
        // All containers of a pod share the same pod socket address.
        let address = self.socket_address(sandbox_id)?;
        let bundle_path = self.get_bundle_path()?;
        let sandbox_bundle_path = bundle_path
            .join("..")
            .join(sandbox_id)
            .canonicalize()
            .map_err(Error::BundlePath)?;
        let pid = self.read_pid_file(&sandbox_bundle_path)?;

        Ok((address, pid))
    }
}

fn new_listener<T: AsRef<OsStr>>(address: T) -> Result<UnixListener> {
    // Listen on an abstract Unix Domain Socket.
    let mut bind_address = OsString::from("\0");
    bind_address.push(address);
    bind_address.push("\0");

    UnixListener::bind(&bind_address).map_err(|e| Error::BindSocket(e, bind_address.into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ShimArgs;
    use rand::distributions::Alphanumeric;
    use rand::{thread_rng, Rng};
    use serial_test::serial;

    #[test]
    #[serial]
    fn test_new_command() {
        let dir = tempfile::tempdir().unwrap();
        let bundle_path = dir.path();
        std::env::set_current_dir(bundle_path).unwrap();

        let args = ShimArgs {
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
        assert_eq!(
            cmd.get_current_dir().unwrap(),
            executor.get_bundle_path().unwrap()
        );

        executor.args.debug = true;
        let cmd = executor.new_command().unwrap();
        assert_eq!(cmd.get_args().len(), 9);
        assert_eq!(cmd.get_envs().len(), 1);
        assert_eq!(
            cmd.get_current_dir().unwrap(),
            executor.get_bundle_path().unwrap()
        );
    }

    #[test]
    #[serial]
    fn test_get_info_from_sandbox() {
        let dir = tempfile::tempdir().unwrap();

        let sandbox_id: String = thread_rng()
            .sample_iter(&Alphanumeric)
            .take(16)
            .map(char::from)
            .collect();
        let bundle_path = &dir.path().join(&sandbox_id);
        std::fs::create_dir(bundle_path).unwrap();
        std::env::set_current_dir(bundle_path).unwrap();

        let args = ShimArgs {
            id: sandbox_id.to_owned(),
            namespace: "ns1".into(),
            address: "containerd_socket".into(),
            publish_binary: "containerd".into(),
            bundle: bundle_path.to_str().unwrap().into(),
            debug: false,
        };
        let executor = ShimExecutor::new(args);

        let addr = executor.socket_address(&executor.args.id).unwrap();
        executor.write_address(&addr).unwrap();
        executor.write_pid_file("1267").unwrap();

        let container_id: String = thread_rng()
            .sample_iter(&Alphanumeric)
            .take(16)
            .map(char::from)
            .collect();
        let bundle_path2 = &dir.path().join(&container_id);
        std::fs::create_dir(bundle_path2).unwrap();
        std::env::set_current_dir(bundle_path2).unwrap();

        let args = ShimArgs {
            id: container_id.to_owned(),
            namespace: "ns1".into(),
            address: "containerd_socket".into(),
            publish_binary: "containerd".into(),
            bundle: bundle_path2.to_str().unwrap().into(),
            debug: false,
        };
        let executor2 = ShimExecutor::new(args);

        let (address, pid) = executor2.get_shim_info_from_sandbox(&sandbox_id).unwrap();

        assert_eq!(&pid, "1267");
        assert_eq!(&address, &addr);
    }

    #[test]
    fn test_new_listener() {
        let result = new_listener("aaabbbccc");
        assert!(result.is_ok())
    }
}
