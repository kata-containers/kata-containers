// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::{
    fs,
    io::Read,
    os::unix::{io::IntoRawFd, net::UnixStream},
    path::{Path, PathBuf},
};

use anyhow::{anyhow, Context, Result};
use kata_sys_util::spec::get_bundle_path;
use kata_types::{container::ContainerType, k8s};
use protobuf::Message;
use unix_socket::UnixListener;

use crate::{
    shim::{socket_path, ShimExecutor, ENV_KATA_RUNTIME_BIND_FD},
    Error,
};

impl ShimExecutor {
    pub fn start(&mut self) -> Result<()> {
        self.args.validate(false).context("validate")?;

        let mut input = Vec::new();
        std::io::stdin()
            .read_to_end(&mut input)
            .context("read runtime options")?;
        let config_path = bootstrap_config_path(&input)?;
        let address = self.do_start(&config_path).context("do start")?;
        serde_json::to_writer(std::io::stdout().lock(), &bootstrap_result(&address))
            .context("failed to write shim bootstrap result")?;

        Ok(())
    }

    fn do_start(&mut self, config_path: &str) -> Result<PathBuf> {
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
                let child_pid = self.create_shim_process(socket, config_path)?;
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

    fn create_shim_process<T: IntoRawFd>(&self, socket: T, config_path: &str) -> Result<u32> {
        let mut cmd = self.new_command().context("new command")?;
        cmd.env(common::config::SHIM_CONFIG_PATH_ENV, config_path);
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

fn bootstrap_config_path(input: &[u8]) -> Result<String> {
    if input.is_empty() {
        return common::config::runtime_config_path(None);
    }

    let is_runtime_options = |value: &protobuf::well_known_types::any::Any| {
        value.type_url.rsplit('/').next() == Some("runtimeoptions.v1.Options")
    };
    let legacy = protobuf::well_known_types::any::Any::parse_from_bytes(input)
        .context("decode bootstrap input")?;
    if is_runtime_options(&legacy) {
        return common::config::runtime_config_path(Some(&legacy.value));
    }

    let params = containerd_shim_protos::bootstrap_api::BootstrapParams::parse_from_bytes(input)
        .context("decode bootstrap parameters")?;
    let options = params
        .extensions
        .iter()
        .filter_map(|extension| extension.value.as_ref())
        .find(|value| is_runtime_options(value));

    common::config::runtime_config_path(options.map(|options| options.value.as_slice()))
}

fn bootstrap_result(address: &Path) -> serde_json::Value {
    serde_json::json!({
        "version": 3,
        "address": address.as_os_str().to_string_lossy(),
        "protocol": "ttrpc",
    })
}

fn new_listener(address: &Path) -> Result<UnixListener> {
    let file_path = socket_path(address)?;
    let file_path = file_path.as_path();
    if let Some(parent_dir) = file_path.parent() {
        fs::create_dir_all(parent_dir).context("create parent dir")?;
    }

    match UnixListener::bind(file_path) {
        Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
            // The socket file of a shim that exited without cleaning up stays
            // at the address and makes every later start of the same sandbox
            // fail. Only a live shim accepts connections, so leave it alone.
            if UnixStream::connect(file_path).is_ok() {
                return Err(e).context("bind address: socket in use by a running shim");
            }
            fs::remove_file(file_path).context("remove stale socket")?;
            UnixListener::bind(file_path).context("bind address after removing stale socket")
        }
        res => res.context("bind address"),
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use serial_test::serial;

    use super::*;
    use crate::Args;

    #[test]
    #[serial]
    fn test_bootstrap_config_selection() {
        struct RestoreEnv(Vec<(&'static str, Option<std::ffi::OsString>)>);
        impl Drop for RestoreEnv {
            fn drop(&mut self) {
                for (key, value) in &self.0 {
                    match value {
                        Some(value) => std::env::set_var(key, value),
                        None => std::env::remove_var(key),
                    }
                }
            }
        }
        let keys = ["KATA_CONF_FILE", common::config::SHIM_CONFIG_PATH_ENV];
        let _restore = RestoreEnv(
            keys.iter()
                .map(|key| (*key, std::env::var_os(key)))
                .collect(),
        );
        for key in keys {
            std::env::remove_var(key);
        }
        assert_eq!(bootstrap_config_path(&[]).unwrap(), "");
        std::env::set_var(common::config::SHIM_CONFIG_PATH_ENV, "/bootstrap.toml");
        assert_eq!(bootstrap_config_path(&[]).unwrap(), "/bootstrap.toml");
        assert_eq!(
            common::config::runtime_config_path(Some(&[])).unwrap(),
            "/bootstrap.toml"
        );
        assert_eq!(
            common::config::runtime_config_path(Some(&[0xff])).unwrap(),
            "/bootstrap.toml"
        );

        let mut empty_options = protobuf::well_known_types::any::Any::new();
        empty_options.type_url = "runtimeoptions.v1.Options".to_owned();
        assert_eq!(
            bootstrap_config_path(&empty_options.write_to_bytes().unwrap()).unwrap(),
            "/bootstrap.toml"
        );

        let path = "/options.toml";
        // runtimeoptions.v1.Options.config_path is protobuf field 2.
        let mut value = vec![0x12, path.len() as u8];
        value.extend_from_slice(path.as_bytes());
        let mut options = protobuf::well_known_types::any::Any::new();
        options.type_url = "runtimeoptions.v1.Options".to_owned();
        options.value = value;
        let bytes = options.write_to_bytes().unwrap();
        assert_eq!(bootstrap_config_path(&bytes).unwrap(), path);
        let mut params = containerd_shim_protos::bootstrap_api::BootstrapParams::new();
        params.instance_id = "test-sandbox".to_owned();
        params.namespace = "k8s.io".to_owned();
        let mut extension = containerd_shim_protos::bootstrap_api::Extension::new();
        extension.value = Some(options).into();
        params.extensions.push(extension);
        assert_eq!(
            bootstrap_config_path(&params.write_to_bytes().unwrap()).unwrap(),
            path
        );
        std::env::set_var("KATA_CONF_FILE", "/non-shipped-env.toml");
        assert!(bootstrap_config_path(&bytes)
            .unwrap_err()
            .to_string()
            .contains("invalid KATA_CONF_FILE"));
    }

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

    #[test]
    fn test_new_listener_stale_socket() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("stale.sock");
        let uds_path = format!("unix://{}", path.display());

        // A listener that is dropped leaves its socket file behind, like a shim
        // that exited without cleaning up.
        drop(new_listener(Path::new(&uds_path)).unwrap());
        assert!(path.exists());

        let _listener = new_listener(Path::new(&uds_path)).unwrap();
    }

    #[test]
    fn test_new_listener_live_socket() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("live.sock");
        let uds_path = format!("unix://{}", path.display());

        let _live = new_listener(Path::new(&uds_path)).unwrap();

        assert!(new_listener(Path::new(&uds_path)).is_err());
        assert!(path.exists());
    }
}
