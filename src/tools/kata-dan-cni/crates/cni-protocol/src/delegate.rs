// Copyright (c) 2026 Kata Containers community
// SPDX-License-Identifier: Apache-2.0

use crate::{CniCommand, CniEnvironment, CniError, CniErrorCode, CniErrorResponse, NetConfList};
use serde_json::{Map, Value};
use std::fs;
use std::io::{self, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::thread;

const MAX_ERROR_OUTPUT_BYTES: usize = 4096;

/// Executes the configured primary CNI plugin.
#[derive(Clone, Copy, Debug, Default)]
pub struct DelegateExecutor;

impl DelegateExecutor {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    pub fn add(
        &self,
        delegate: &NetConfList,
        cni_version: &str,
        runtime_config: &Map<String, Value>,
        environment: &CniEnvironment,
        initial_result: Option<&Value>,
    ) -> Result<Value, CniError> {
        let request = delegate.derive_plugin_request(
            cni_version,
            CniCommand::Add,
            runtime_config,
            initial_result,
        );
        let output = self.invoke_request(delegate, CniCommand::Add, environment, &request)?;
        serde_json::from_slice::<Value>(&output).map_err(|source| {
            CniError::new(
                CniErrorCode::DelegateProtocol,
                "delegate ADD returned an invalid CNI Result",
                source.to_string(),
            )
        })
    }

    pub fn delete(
        &self,
        delegate: &NetConfList,
        cni_version: &str,
        runtime_config: &Map<String, Value>,
        environment: &CniEnvironment,
        final_result: Option<&Value>,
    ) -> Result<(), CniError> {
        self.invoke_no_result(
            delegate,
            cni_version,
            CniCommand::Del,
            runtime_config,
            environment,
            final_result,
        )
    }

    pub fn check(
        &self,
        delegate: &NetConfList,
        cni_version: &str,
        runtime_config: &Map<String, Value>,
        environment: &CniEnvironment,
        final_result: Option<&Value>,
    ) -> Result<(), CniError> {
        // disableCheck still requires a CNI version that supports CHECK.
        if matches!(cni_version, "0.1.0" | "0.2.0" | "0.3.0" | "0.3.1") {
            return Err(CniError::new(
                CniErrorCode::IncompatibleVersion,
                "CNI configuration version does not allow CHECK",
                format!("cniVersion={cni_version} is older than 0.4.0"),
            ));
        }
        if delegate.disable_check {
            return Ok(());
        }
        let final_result = final_result.ok_or_else(|| {
            CniError::new(
                CniErrorCode::InvalidNetworkConfig,
                "CHECK requires prevResult",
                "the runtime must provide the final ADD Result",
            )
        })?;
        self.invoke_no_result(
            delegate,
            cni_version,
            CniCommand::Check,
            runtime_config,
            environment,
            Some(final_result),
        )
    }

    fn invoke_no_result(
        &self,
        delegate: &NetConfList,
        cni_version: &str,
        command: CniCommand,
        runtime_config: &Map<String, Value>,
        environment: &CniEnvironment,
        previous_result: Option<&Value>,
    ) -> Result<(), CniError> {
        let request =
            delegate.derive_plugin_request(cni_version, command, runtime_config, previous_result);
        let output = self.invoke_request(delegate, command, environment, &request)?;
        if output.iter().any(|byte| !byte.is_ascii_whitespace()) {
            return Err(CniError::new(
                CniErrorCode::DelegateProtocol,
                format!("delegate {command} wrote unexpected stdout"),
                format!("delegate must not return a Result for {command}"),
            ));
        }
        Ok(())
    }

    fn invoke_request(
        &self,
        delegate: &NetConfList,
        command: CniCommand,
        environment: &CniEnvironment,
        config: &Value,
    ) -> Result<Vec<u8>, CniError> {
        delegate.validate()?;
        let plugin = delegate.plugin();
        let search_paths = plugin_search_paths(&environment.cni_path);
        let executable = self.resolve_plugin(&plugin.plugin_type, &search_paths)?;
        let input = serde_json::to_vec(config).map_err(|source| {
            CniError::new(
                CniErrorCode::DecodeFailure,
                "failed to encode delegate input",
                source.to_string(),
            )
        })?;

        let mut process = Command::new(&executable);
        process
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .env("CNI_COMMAND", command.as_str())
            .env(
                "CNI_PATH",
                std::env::join_paths(&search_paths).map_err(|source| {
                    CniError::new(
                        CniErrorCode::InvalidEnvironment,
                        "failed to encode CNI_PATH",
                        source.to_string(),
                    )
                })?,
            );
        process
            .env(
                "CNI_CONTAINERID",
                environment.container_id.as_deref().unwrap_or(""),
            )
            .env("CNI_NETNS", environment.netns.as_deref().unwrap_or(""))
            .env("CNI_IFNAME", environment.if_name.as_deref().unwrap_or(""));
        if let Some(cni_args) = &environment.cni_args {
            process.env("CNI_ARGS", cni_args);
        } else {
            process.env_remove("CNI_ARGS");
        }

        let output = run_child(&mut process, &input).map_err(|source| {
            CniError::new(
                CniErrorCode::IoFailure,
                "failed to execute delegate plugin",
                format!("{}: {source}", executable.display()),
            )
        })?;
        if output.status.success() {
            return Ok(output.stdout);
        }

        if let Some(error) = decode_delegate_error(&output.stdout) {
            return Err(error);
        }
        let details = String::from_utf8_lossy(
            &output.stdout[..output.stdout.len().min(MAX_ERROR_OUTPUT_BYTES)],
        );
        Err(CniError::new(
            CniErrorCode::DelegateProtocol,
            "delegate plugin failed without a valid CNI Error",
            format!(
                "{} exited with {}; stdout={details:?}",
                plugin.plugin_type, output.status
            ),
        ))
    }

    fn resolve_plugin(
        &self,
        plugin_type: &str,
        search_path: &[PathBuf],
    ) -> Result<PathBuf, CniError> {
        for directory in search_path {
            let candidate = directory.join(plugin_type);
            let metadata = match fs::metadata(&candidate) {
                Ok(metadata) => metadata,
                Err(source) if source.kind() == io::ErrorKind::NotFound => continue,
                Err(source) => {
                    return Err(CniError::new(
                        CniErrorCode::IoFailure,
                        "failed to inspect delegate plugin",
                        format!("{}: {source}", candidate.display()),
                    ));
                }
            };
            if !metadata.is_file() || metadata.permissions().mode() & 0o111 == 0 {
                continue;
            }
            return Ok(candidate);
        }
        Err(CniError::new(
            CniErrorCode::IoFailure,
            "delegate plugin not found",
            format!(
                "{plugin_type} was not found as an executable file in CNI_PATH={}",
                display_search_path(search_path)
            ),
        ))
    }
}

fn plugin_search_paths(paths: &[PathBuf]) -> Vec<PathBuf> {
    if !paths.is_empty() {
        return paths.to_vec();
    }
    let mut fallback = Vec::new();
    if let Ok(executable) = std::env::current_exe() {
        if let Some(directory) = executable.parent() {
            fallback.push(directory.to_path_buf());
        }
    }
    fallback.push(PathBuf::from("/opt/cni/bin"));
    fallback
}

// Write stdin while draining output to avoid a pipe deadlock.
fn run_child(process: &mut Command, input: &[u8]) -> io::Result<Output> {
    let mut child = process.spawn()?;
    let mut stdin = child.stdin.take().expect("delegate stdin is piped");
    thread::scope(|scope| {
        let writer = scope.spawn(move || stdin.write_all(input));
        let output = child.wait_with_output()?;
        let written = writer
            .join()
            .map_err(|_| io::Error::other("delegate stdin writer panicked"))?;
        // Preserve a failed delegate's CNI Error if it closed stdin early.
        if output.status.success() {
            written?;
        }
        Ok(output)
    })
}

fn display_search_path(paths: &[PathBuf]) -> String {
    paths
        .iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>()
        .join(":")
}

fn decode_delegate_error(output: &[u8]) -> Option<CniError> {
    let response = serde_json::from_slice::<CniErrorResponse>(output).ok()?;
    if response.code == 0 || response.msg.trim().is_empty() {
        return None;
    }
    Some(CniError::from_code(
        response.code,
        response.msg,
        response.details,
    ))
}

#[cfg(test)]
mod tests {
    use crate::{CniCommand, CniEnvironment, CniErrorCode, DelegateExecutor, NetConfList};
    use serde_json::json;
    use std::error::Error;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    type TestResult = Result<(), Box<dyn Error>>;
    static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    struct Plugin {
        directory: PathBuf,
    }

    impl Plugin {
        fn new(body: &str) -> Result<Self, Box<dyn Error>> {
            let directory = std::env::temp_dir().join(format!(
                "kata-dan-delegate-{}-{}",
                std::process::id(),
                NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&directory)?;
            let fixture = Self { directory };
            fs::write(
                fixture.directory.join("primary"),
                format!("#!/bin/sh\nset -eu\n{body}\n"),
            )?;
            fs::set_permissions(
                fixture.directory.join("primary"),
                fs::Permissions::from_mode(0o755),
            )?;
            Ok(fixture)
        }

        fn environment(&self) -> CniEnvironment {
            CniEnvironment {
                command: CniCommand::Del,
                container_id: Some("sandbox-123".into()),
                netns: Some("/run/netns/example".into()),
                if_name: Some("net1".into()),
                cni_args: Some("K8S_POD_NAMESPACE=apps;K8S_POD_NAME=demo".into()),
                cni_path: vec![self.directory.clone()],
            }
        }
    }

    impl Drop for Plugin {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.directory);
        }
    }

    fn delegate() -> NetConfList {
        serde_json::from_value(json!({
            "name": "ordinary-network",
            "plugins": [{"type": "primary", "server_socket": "/run/ovn.sock"}]
        }))
        .expect("test delegate config")
    }

    #[test]
    fn delegate_error() -> TestResult {
        let plugin = Plugin::new(
            r#"printf '%s\n' '{"cniVersion":"0.4.0","code":42,"msg":"IPAM unavailable","details":"allocation pending"}'
    exit 7"#,
        )?;
        let mut delegate = delegate();
        // A request larger than a pipe exercises early child exit while stdin is being written.
        delegate.plugins[0]
            .plugin_fields
            .insert("large".into(), json!("x".repeat(256 * 1024)));
        let error = DelegateExecutor::new()
            .add(
                &delegate,
                "1.0.0",
                &Default::default(),
                &plugin.environment(),
                None,
            )
            .expect_err("upstream failure");
        assert_eq!(error.code(), 42);
        assert_eq!(error.response().msg, "IPAM unavailable");
        assert_eq!(error.response().details, "allocation pending");
        Ok(())
    }

    #[test]
    fn check_preconditions() {
        let executor = DelegateExecutor::new();
        let environment = CniEnvironment {
            command: CniCommand::Check,
            container_id: None,
            netns: None,
            if_name: None,
            cni_args: None,
            cni_path: vec![],
        };
        let check = |delegate: &NetConfList, version| {
            executor.check(delegate, version, &Default::default(), &environment, None)
        };
        let mut delegate = delegate();
        assert_eq!(
            check(&delegate, "1.0.0")
                .expect_err("CHECK needs prevResult")
                .code(),
            CniErrorCode::InvalidNetworkConfig.value()
        );
        delegate.disable_check = true;
        check(&delegate, "1.0.0").expect("disableCheck skips execution");
        assert_eq!(
            check(&delegate, "0.3.1")
                .expect_err("disableCheck cannot enable old CHECK")
                .code(),
            CniErrorCode::IncompatibleVersion.value()
        );
    }
}
