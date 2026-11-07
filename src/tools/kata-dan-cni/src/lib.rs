// Copyright (c) 2026 Kata Containers community
// SPDX-License-Identifier: Apache-2.0

mod backend_selection;
mod dan_file;
mod dispatch;
mod provider_selection;
mod request_selection;
mod workload;

use cni_protocol::{version_response, CniCommand, CniEnvironment, CniError, CniErrorCode};
use serde_json::Value;
use std::io::{self, Read, Write};

pub use dispatch::{dispatch, CommandOutput};

pub fn run_process() -> i32 {
    let mut input = Vec::new();
    let result = match io::stdin().read_to_end(&mut input) {
        Ok(_) => execute_from_process(&input),
        Err(source) => Err(CniError::new(
            CniErrorCode::IoFailure,
            "failed to read CNI configuration",
            source.to_string(),
        )),
    };

    match result {
        Ok(CommandOutput::Json(value)) => match write_json(&mut io::stdout(), &value) {
            Ok(()) => 0,
            Err(error) => {
                let _ = writeln!(io::stderr(), "kata-dan-cni: {error}");
                1
            }
        },
        Ok(CommandOutput::Empty) => 0,
        Err(error) => {
            let _ = writeln!(io::stderr(), "kata-dan-cni: {error}");
            let write_result = write_json(&mut io::stdout(), &serde_json::json!(error.response()));
            if let Err(write_error) = write_result {
                let _ = writeln!(io::stderr(), "kata-dan-cni: {write_error}");
            }
            1
        }
    }
}

pub fn execute_from_process(input: &[u8]) -> Result<CommandOutput, CniError> {
    let environment =
        CniEnvironment::from_process().map_err(|error| with_input_cni_version(error, input))?;
    execute(environment, input)
}

pub fn execute(environment: CniEnvironment, input: &[u8]) -> Result<CommandOutput, CniError> {
    if environment.command == CniCommand::Version {
        return version_response(input).map(CommandOutput::Json);
    }
    dispatch(environment, input)
}

fn write_json(writer: &mut impl Write, value: &Value) -> Result<(), CniError> {
    serde_json::to_writer(&mut *writer, value).map_err(|source| {
        CniError::new(
            CniErrorCode::IoFailure,
            "failed to write CNI JSON response",
            source.to_string(),
        )
    })?;
    writer.write_all(b"\n").map_err(|source| {
        CniError::new(
            CniErrorCode::IoFailure,
            "failed to finish CNI JSON response",
            source.to_string(),
        )
    })
}

fn with_input_cni_version(error: CniError, input: &[u8]) -> CniError {
    let version = serde_json::from_slice::<Value>(input)
        .ok()
        .and_then(|value| {
            value
                .get("cniVersion")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)
        });
    match version {
        Some(version) => error.with_cni_version(version),
        None => error,
    }
}

#[cfg(test)]
mod test_support {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    use dan_core::{
        AttachmentIdentity, DanDevice, DanEndpoint, IpAssignment, MacAddress, NetworkAllocation,
        Route,
    };

    pub(crate) struct TestDir(pub(crate) PathBuf);

    impl TestDir {
        pub(crate) fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "dan-cni-test-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).expect("create isolated test directory");
            Self(path)
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    pub(crate) fn allocation() -> NetworkAllocation {
        NetworkAllocation {
            mac_address: MacAddress([0x02, 0, 0, 0, 0, 0x42]),
            addresses: vec![
                IpAssignment {
                    address: "10.42.0.9".parse().expect("IPv4"),
                    prefix_length: 24,
                },
                IpAssignment {
                    address: "fd42::9".parse().expect("IPv6"),
                    prefix_length: 64,
                },
            ],
            routes: vec![
                Route {
                    destination: "0.0.0.0".parse().expect("default v4"),
                    prefix_length: 0,
                    gateway: Some("10.42.0.1".parse().expect("gateway")),
                },
                Route {
                    destination: "::".parse().expect("default v6"),
                    prefix_length: 0,
                    gateway: Some("fd42::1".parse().expect("gateway")),
                },
                Route {
                    destination: "10.43.0.0".parse().expect("on link route"),
                    prefix_length: 16,
                    gateway: None,
                },
            ],
            mtu: 1400,
        }
    }

    pub(crate) fn attachment(if_name: &str) -> AttachmentIdentity {
        AttachmentIdentity {
            container_id: "sandbox-42".into(),
            if_name: if_name.into(),
        }
    }

    pub(crate) fn device(if_name: &str) -> DanDevice {
        DanDevice {
            guest_if_name: if_name.into(),
            endpoint: DanEndpoint::HostTap {
                tap_name: format!("tap-{if_name}"),
            },
            allocation: allocation(),
        }
    }
}
