// Copyright (c) 2023 Microsoft Corporation
// Copyright (c) 2024 Edgeless Systems GmbH
//
// SPDX-License-Identifier: Apache-2.0
//

//! Policy evaluation for the kata-agent.

use std::{ffi::OsStr, os::unix::ffi::OsStrExt as _};

use anyhow::{bail, Error, Result};
use protocols::agent::CopyFileRequest;
use slog::{debug, error, info, warn};
use tokio::io::AsyncWriteExt;

static POLICY_LOG_FILE: &str = "/tmp/policy.jsonl";
static POLICY_DEFAULT_FILE: &str = "/etc/kata-opa/default-policy.rego";

/// Convenience macro to obtain the scope logger
macro_rules! sl {
    () => {
        slog_scope::logger()
    };
}

/// Singleton policy object.
#[derive(Debug, Default)]
pub struct AgentPolicy {
    /// When true policy errors are ignored, for debug purposes.
    allow_failures: bool,

    /// "/tmp/policy.jsonl" log file for policy activity.
    log_file: Option<tokio::fs::File>,

    /// Regorus engine
    engine: regorus::Engine,
}

#[derive(serde::Deserialize, Debug)]
struct MetadataResponse {
    allowed: bool,
    ops: Option<json_patch::Patch>,
}

impl AgentPolicy {
    /// Create AgentPolicy object.
    pub fn new() -> Self {
        Self {
            allow_failures: false,
            engine: Self::new_engine(),
            ..Default::default()
        }
    }

    fn new_engine() -> regorus::Engine {
        let mut engine = regorus::Engine::new();
        engine.set_strict_builtin_errors(false);
        engine.set_gather_prints(true);
        // assign a slice of the engine data "pstate" to be used as policy state
        engine
            .add_data(
                regorus::Value::from_json_str(
                    r#"{
                        "pstate": {}
                    }"#,
                )
                .unwrap(),
            )
            .unwrap();
        engine
    }

    /// Initialize regorus.
    pub async fn initialize(
        &mut self,
        log_level: usize,
        default_policy_file: String,
        log_file: Option<String>,
    ) -> Result<()> {
        // log file path
        let log_file_path = match log_file {
            Some(path) => path,
            None => POLICY_LOG_FILE.to_string(),
        };
        let log_file_path = log_file_path.as_str();

        if log_level >= slog::Level::Debug.as_usize() {
            self.log_file = Some(
                tokio::fs::OpenOptions::new()
                    .write(true)
                    .truncate(true)
                    .create(true)
                    .open(&log_file_path)
                    .await?,
            );
            debug!(sl!(), "policy: log file: {}", log_file_path);
        }

        // Check if policy file has been set via AgentConfig
        // If empty, use default file.
        let mut default_policy_file = default_policy_file;
        if default_policy_file.is_empty() {
            default_policy_file = POLICY_DEFAULT_FILE.to_string();
        }
        info!(sl!(), "default policy: {default_policy_file}");

        self.engine.add_policy_from_file(default_policy_file)?;
        self.update_allow_failures_flag().await?;
        Ok(())
    }

    async fn apply_patch_to_state(&mut self, patch: json_patch::Patch) -> Result<()> {
        // Convert the current engine data to a JSON value
        let mut state = serde_json::to_value(self.engine.get_data())?;

        // Apply the patch to the state
        json_patch::patch(&mut state, &patch)?;

        // Clear the existing data in the engine
        self.engine.clear_data();

        // Add the patched state back to the engine
        self.engine
            .add_data(regorus::Value::from_json_str(&state.to_string())?)?;

        Ok(())
    }

    /// Ask regorus if an API call should be allowed or not.
    pub async fn allow_request(&mut self, ep: &str, ep_input: &str) -> Result<(bool, String)> {
        debug!(sl!(), "policy check: {ep}");
        self.log_eval_input(ep, ep_input).await;

        let query = format!("data.agent_policy.{ep}");
        self.engine.set_input_json(ep_input)?;

        let results = self.engine.eval_query(query, false)?;

        let prints = match self.engine.take_prints() {
            Ok(p) => p.join(" "),
            Err(e) => format!("Failed to get policy log: {e}"),
        };

        if results.result.len() != 1 {
            // Results are empty when AllowRequestsFailingPolicy is used to allow a Request that hasn't been defined in the policy
            if self.allow_failures {
                return Ok((true, prints));
            }
            bail!(
                "policy check: unexpected eval_query result len {:?}",
                results
            );
        }

        if results.result[0].expressions.len() != 1 {
            bail!(
                "policy check: unexpected eval_query result expressions {:?}",
                results
            );
        }

        let mut allow = match &results.result[0].expressions[0].value {
            regorus::Value::Bool(b) => *b,

            // Match against a specific variant that could be interpreted as MetadataResponse
            regorus::Value::Object(obj) => {
                let json_str = serde_json::to_string(obj)?;

                self.log_eval_input(ep, &json_str).await;

                let metadata_response: MetadataResponse = serde_json::from_str(&json_str)?;

                if metadata_response.allowed {
                    if let Some(ops) = metadata_response.ops {
                        self.apply_patch_to_state(ops).await?;
                    }
                }
                metadata_response.allowed
            }

            _ => {
                error!(sl!(), "allow_request: unexpected eval_query result type");
                bail!(
                    "policy check: unexpected eval_query result type {:?}",
                    results
                );
            }
        };

        if !allow && self.allow_failures {
            warn!(sl!(), "policy: ignoring error for {ep}");
            allow = true;
        }

        Ok((allow, prints))
    }

    /// Replace the Policy in regorus.
    pub async fn set_policy(&mut self, policy: &str) -> Result<()> {
        self.engine = Self::new_engine();
        self.engine
            .add_policy("agent_policy".to_string(), policy.to_string())?;
        self.update_allow_failures_flag().await?;
        Ok(())
    }

    async fn log_eval_input(&mut self, ep: &str, input: &str) {
        if let Some(log_file) = &mut self.log_file {
            match ep {
                "StatsContainerRequest" | "ReadStreamRequest" | "SetPolicyRequest" => {
                    // - StatsContainerRequest and ReadStreamRequest are called
                    //   relatively often, so we're not logging them, to avoid
                    //   growing this log file too much.
                    // - Confidential Containers Policy documents are relatively
                    //   large, so we're not logging them here, for SetPolicyRequest.
                    //   The Policy text can be obtained directly from the pod YAML.
                }
                _ => {
                    let log_entry = format!("{{\"kind\":\"{ep}\",\"request\":{input}}}\n");

                    if let Err(e) = log_file.write_all(log_entry.as_bytes()).await {
                        warn!(sl!(), "policy: log_eval_input: write_all failed: {}", e);
                    } else if let Err(e) = log_file.flush().await {
                        warn!(sl!(), "policy: log_eval_input: flush failed: {}", e);
                    }
                }
            }
        }
    }

    async fn update_allow_failures_flag(&mut self) -> Result<()> {
        self.allow_failures = match self.allow_request("AllowRequestsFailingPolicy", "{}").await {
            Ok((allowed, _prints)) => {
                if allowed {
                    warn!(
                        sl!(),
                        "policy: AllowRequestsFailingPolicy is enabled - will ignore errors"
                    );
                }
                allowed
            }
            Err(_) => false,
        };
        Ok(())
    }
}

/// FileType represents the S_IFMT part of the POSIX file mode such that it's easier to check in
/// Rego.
#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, Default, PartialEq)]
pub enum FileType {
    #[default]
    Unknown,
    Regular,
    Directory,
    Symlink,
}

impl From<u32> for FileType {
    fn from(raw_mode: u32) -> Self {
        match raw_mode & libc::S_IFMT {
            libc::S_IFREG => Self::Regular,
            libc::S_IFDIR => Self::Directory,
            libc::S_IFLNK => Self::Symlink,
            _ => Self::Unknown,
        }
    }
}

/// PolicyCopyFileRequest is a pre-processed variant of the CopyFileRequest that avoids byte
/// manipulation in Rego rules.
#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct PolicyCopyFileRequest {
    pub path: String,
    pub file_type: FileType,
    pub symlink_target: Option<String>,

    // Below fields are copied from the original request. They are not used by the genpolicy rules,
    // but might be relevant for alternative rule sets. The data field is intentionally omitted to
    // reduce serde overhead and protect the rules engine.

    pub file_size: i64,
    pub file_mode: u32,
    pub dir_mode: u32,
    pub uid: i32,
    pub gid: i32,
    pub offset: i64,
}

impl std::convert::TryFrom<&CopyFileRequest> for PolicyCopyFileRequest {
    type Error = Error;

    fn try_from(req: &CopyFileRequest) -> Result<Self> {
        let file_type = req.file_mode.into();
        let symlink_target: Option<String> = match file_type {
            FileType::Symlink => {
                if let Some(s) = OsStr::from_bytes(&req.data).to_str() {
                    Some(s.to_owned())
                } else {
                    bail!("invalid symlink content")
                }
            }
            _ => None,
        };

        Ok(PolicyCopyFileRequest {
            path: req.path.clone(),
            file_type,
            symlink_target,
            file_size: req.file_size,
            file_mode: req.file_mode,
            dir_mode: req.dir_mode,
            uid: req.uid,
            gid: req.gid,
            offset: req.offset,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::convert::TryInto;

    use protocols::agent::CopyFileRequest;

    struct TestCase {
        name: String,
        input: CopyFileRequest,
        output: Option<PolicyCopyFileRequest>,
    }

    #[test]
    fn test_copyfile_translation() {
        let test_cases = [
            TestCase {
                name: "regular".to_owned(),
                input: CopyFileRequest {
                    file_mode: libc::S_IFREG,
                    path: "/foo/bar".to_owned(),
                    ..Default::default()
                },
                output: Some(PolicyCopyFileRequest {
                    file_mode: libc::S_IFREG,
                    file_type: FileType::Regular,
                    path: "/foo/bar".to_owned(),
                    ..Default::default()
                }),
            },
            TestCase {
                name: "directory".to_owned(),
                input: CopyFileRequest {
                    file_mode: libc::S_IFDIR,
                    path: "/foo".to_owned(),
                    ..Default::default()
                },
                output: Some(PolicyCopyFileRequest {
                    file_mode: libc::S_IFDIR,
                    file_type: FileType::Directory,
                    path: "/foo".to_owned(),
                    ..Default::default()
                }),
            },
            TestCase {
                name: "socket".to_owned(),
                input: CopyFileRequest {
                    file_mode: libc::S_IFSOCK,
                    path: "/foo/sock".to_owned(),
                    ..Default::default()
                },
                output: Some(PolicyCopyFileRequest {
                    file_mode: libc::S_IFSOCK,
                    file_type: FileType::Unknown,
                    path: "/foo/sock".to_owned(),
                    ..Default::default()
                }),
            },
            TestCase {
                name: "mixed".to_owned(),
                input: CopyFileRequest {
                    file_mode: libc::S_IFDIR | libc::S_IFREG,
                    path: "/foo/dunno".to_owned(),
                    ..Default::default()
                },
                output: Some(PolicyCopyFileRequest {
                    file_mode: libc::S_IFDIR | libc::S_IFREG,
                    file_type: FileType::Unknown,
                    path: "/foo/dunno".to_owned(),
                    ..Default::default()
                }),
            },
            TestCase {
                name: "all".to_owned(),
                input: CopyFileRequest {
                    file_mode: libc::S_IFMT,
                    path: "/wat".to_owned(),
                    ..Default::default()
                },
                output: Some(PolicyCopyFileRequest {
                    file_mode: libc::S_IFMT,
                    file_type: FileType::Unknown,
                    path: "/wat".to_owned(),
                    ..Default::default()
                }),
            },
            TestCase {
                name: "none".to_owned(),
                input: CopyFileRequest {
                    file_mode: 0,
                    path: "/0".to_owned(),
                    ..Default::default()
                },
                output: Some(PolicyCopyFileRequest {
                    file_mode: 0,
                    file_type: FileType::Unknown,
                    path: "/0".to_owned(),
                    ..Default::default()
                }),
            },
            TestCase {
                name: "link/valid".to_owned(),
                input: CopyFileRequest {
                    data: b"..data/foo".to_vec(),
                    file_mode: libc::S_IFLNK,
                    path: "/foo/lnk".to_owned(),
                    ..Default::default()
                },
                output: Some(PolicyCopyFileRequest {
                    file_mode: libc::S_IFLNK,
                    file_type: FileType::Symlink,
                    symlink_target: Some("..data/foo".to_owned()),
                    path: "/foo/lnk".to_owned(),
                    ..Default::default()
                }),
            },
            TestCase {
                name: "link/invalid".to_owned(),
                input: CopyFileRequest {
                    file_mode: libc::S_IFLNK,
                    data: vec![0x00, 0xFF, 0xFF, 0x00],
                    ..Default::default()
                },
                output: None,
            },
        ];

        for test_case in test_cases {
            let output_res: Result<PolicyCopyFileRequest> = (&test_case.input).try_into();
            if let Some(expected) = test_case.output {
                let output = output_res.expect(&format!("test case {}", &test_case.name));
                assert_eq!(expected, output, "test case {}", &test_case.name)
            } else {
                assert!(
                    output_res.is_err(),
                    "test case {}\nunexpected success: {:?}",
                    &test_case.name,
                    output_res
                )
            }
        }
    }
}
