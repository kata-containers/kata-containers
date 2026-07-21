// Copyright (c) 2023 Microsoft Corporation
// Copyright (c) 2024 Edgeless Systems GmbH
//
// SPDX-License-Identifier: Apache-2.0
//

//! Policy evaluation for the kata-agent.

use std::num::{NonZeroU32, NonZeroUsize};
use std::{ffi::OsStr, os::unix::ffi::OsStrExt as _};

use anyhow::{bail, Error, Result};
use protocols::agent::CopyFileRequest;
use regorus::PolicyLengthConfig;
use slog::{debug, error, info, warn};
use tokio::io::AsyncWriteExt;

// Regorus' built-in policy length limits (1024 cols / 1 MiB / 20 000 lines)
// reject realistic policies emitted by `genpolicy`. In particular, container
// `Env` values such as NVIDIA_REQUIRE_CUDA on the upstream NVIDIA CUDA images
// can exceed 1 KiB on a single line. These constants raise the per-engine
// limits to values that comfortably fit any policy we expect to evaluate
// while still rejecting pathological/minified input.
//
// See microsoft/regorus#624 for the upstream API.
const POLICY_MAX_COL: u32 = 64 * 1024; // 64 KiB per line
const POLICY_MAX_FILE_BYTES: usize = 16 * 1024 * 1024; // 16 MiB per file
const POLICY_MAX_LINES: usize = 200_000;

static POLICY_LOG_FILE: &str = "/tmp/policy.jsonl";
static POLICY_DEFAULT_FILE: &str = "/etc/kata-opa/default-policy.rego";

/// Closed-door baseline used in strict builds when no explicit policy is provided.
/// It denies every security-relevant request (every endpoint is left undefined, so
/// policy evaluation fails closed) except `SetPolicyRequest`, which is the channel
/// through which an authorized policy is delivered.
#[cfg(feature = "strict-policy")]
static STRICT_DEFAULT_POLICY: &str =
    "package agent_policy\n\ndefault SetPolicyRequest := true\n";

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

    /// Strict builds: set once an authorized policy has been activated. After
    /// activation, SetPolicy is rejected (policy is one-shot; changing it requires
    /// a new verifier-authorized epoch), so the host cannot swap the policy at runtime.
    #[cfg(feature = "strict-policy")]
    policy_activated: bool,

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
        engine.set_policy_length_config(PolicyLengthConfig {
            max_col: NonZeroU32::new(POLICY_MAX_COL).unwrap(),
            max_file_bytes: NonZeroUsize::new(POLICY_MAX_FILE_BYTES).unwrap(),
            max_lines: NonZeroUsize::new(POLICY_MAX_LINES).unwrap(),
        });
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

        // Strict builds never fall back to a permissive default shipped in the guest
        // image: if no explicit (attested) policy was provided, install the compiled-in
        // closed-door baseline so the guest denies all security-relevant requests until
        // an authorized policy is delivered.
        #[cfg(feature = "strict-policy")]
        if default_policy_file.is_empty() {
            info!(
                sl!(),
                "strict-policy: no explicit policy provided; loading closed-door baseline"
            );
            self.engine
                .add_policy("strict-default.rego".to_string(), STRICT_DEFAULT_POLICY.to_string())?;
            self.update_allow_failures_flag().await?;
            return Ok(());
        }

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

    /// FR-6: capture the current policy state (`pstate`) so a transaction can roll it back.
    /// The policy applies its state-mutating `ops` during authorization; snapshotting before
    /// authorization and restoring on abort ensures a failed operation leaves no committed
    /// enforcer state (equivalent to runhcs/OpenGCS `WithMetadataRollback`).
    #[cfg(feature = "strict-policy")]
    pub fn snapshot_state(&self) -> Result<String> {
        Ok(serde_json::to_value(self.engine.get_data())?.to_string())
    }

    /// FR-6: restore policy state captured by `snapshot_state` (transaction rollback).
    #[cfg(feature = "strict-policy")]
    pub fn restore_state(&mut self, snapshot: &str) -> Result<()> {
        self.engine.clear_data();
        self.engine
            .add_data(regorus::Value::from_json_str(snapshot)?)?;
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

        // FR-8: on denial, emit a structured, rule-attributable decision object. It
        // records the endpoint, the denied rule, and the request's top-level field names
        // (never values), so denials are auditable without leaking env values, sealed
        // secrets, or policy text.
        if !allow {
            let decision = crate::decision::DecisionObject::for_denial(ep, ep_input);
            self.log_decision(&decision).await;
        }

        Ok((allow, prints))
    }

    /// FR-8: append a structured decision object to the policy log. The object carries no
    /// request values, so this cannot leak workload data.
    async fn log_decision(&mut self, decision: &crate::decision::DecisionObject) {
        debug!(sl!(), "policy decision"; "endpoint" => &decision.endpoint, "decision" => decision.decision, "failed-rule" => &decision.failed_rule);
        if let Some(log_file) = &mut self.log_file {
            let line = format!("{}\n", decision.to_json());
            if let Err(e) = log_file.write_all(line.as_bytes()).await {
                warn!(sl!(), "policy: log_decision: write_all failed: {}", e);
            } else if let Err(e) = log_file.flush().await {
                warn!(sl!(), "policy: log_decision: flush failed: {}", e);
            }
        }
    }

    /// Replace the Policy in regorus.
    pub async fn set_policy(&mut self, policy: &str) -> Result<()> {
        // Strict builds: policy activation is one-shot. Once an authorized policy is
        // active, reject any attempt to replace it (changing policy requires a new
        // verifier-authorized epoch), so the host cannot weaken policy at runtime.
        #[cfg(feature = "strict-policy")]
        if self.policy_activated {
            bail!("strict-policy: policy already activated; SetPolicy is one-shot");
        }
        self.engine = Self::new_engine();
        self.engine
            .add_policy("agent_policy".to_string(), policy.to_string())?;
        self.update_allow_failures_flag().await?;
        #[cfg(feature = "strict-policy")]
        {
            self.policy_activated = true;
        }
        Ok(())
    }

    /// FR-1a: apply a verified policy fragment's Rego module to the live engine.
    ///
    /// This is the **only** sanctioned runtime extension of an active policy. Unlike
    /// `set_policy` it is **additive** — it adds a named module via `add_policy` and does
    /// NOT rebuild the engine, so it bypasses the FR-12 one-shot lock without weakening it
    /// (`set_policy` stays rejected after activation). The fragment module must declare a
    /// package inside the reserved fragment namespace (`agent_policy.fragments`, optionally
    /// scoped to one of the fragment's `includes`), so a fragment can only *add* rules in
    /// its own namespace and can never redefine or shadow a base `agent_policy` rule. The
    /// base policy is authored to consult `data.agent_policy.fragments.*`.
    pub fn apply_fragment_module(
        &mut self,
        name: &str,
        rego: &str,
        includes: &[String],
    ) -> Result<()> {
        let pkg = Self::rego_package(rego)
            .ok_or_else(|| anyhow::anyhow!("fragment module has no package declaration"))?;

        let mut allowed = vec!["agent_policy.fragments".to_string()];
        for ns in includes {
            allowed.push(format!("agent_policy.fragments.{ns}"));
        }
        if !allowed.iter().any(|a| a == &pkg) {
            bail!(
                "fragment module package {:?} is outside the permitted fragment namespaces {:?}",
                pkg,
                allowed
            );
        }

        // Additive merge; never resets the engine, never touches the one-shot lock.
        self.engine.add_policy(name.to_string(), rego.to_string())?;
        Ok(())
    }

    /// Extract the top-level `package` path from a Rego module (e.g. "agent_policy.fragments").
    fn rego_package(rego: &str) -> Option<String> {
        for line in rego.lines() {
            let l = line.trim();
            if let Some(rest) = l.strip_prefix("package ") {
                let pkg = rest.trim();
                if !pkg.is_empty() {
                    return Some(pkg.to_string());
                }
            }
        }
        None
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
        // In strict builds the "ignore requests failing policy" escape hatch is
        // compiled out: requests that fail policy evaluation are always denied,
        // regardless of any AllowRequestsFailingPolicy value in the policy.
        #[cfg(feature = "strict-policy")]
        {
            self.allow_failures = false;
            return Ok(());
        }
        #[cfg(not(feature = "strict-policy"))]
        {
            self.allow_failures = match self.allow_request("AllowRequestsFailingPolicy", "{}").await
            {
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
    // libc::S_IF* are mode_t, which is u16 on Darwin/BSD and u32 on Linux. The
    // `as u32` cast is required for Darwin but a no-op on Linux, which trips
    // clippy::unnecessary_cast. This is the documented libc-portability case
    // from https://github.com/rust-lang/rust-clippy/issues/6466.
    #[allow(clippy::unnecessary_cast)]
    fn from(raw_mode: u32) -> Self {
        const S_IFMT: u32 = libc::S_IFMT as u32;
        const S_IFREG: u32 = libc::S_IFREG as u32;
        const S_IFDIR: u32 = libc::S_IFDIR as u32;
        const S_IFLNK: u32 = libc::S_IFLNK as u32;
        match raw_mode & S_IFMT {
            S_IFREG => Self::Regular,
            S_IFDIR => Self::Directory,
            S_IFLNK => Self::Symlink,
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
// libc::S_IF* constants are u16 on Darwin/BSD and u32 on Linux, and the test
// cases below cast them to u32 to match the file_mode field type. The cast is
// a no-op on Linux (see https://github.com/rust-lang/rust-clippy/issues/6466).
#[allow(clippy::unnecessary_cast)]
mod tests {
    use super::*;
    use std::convert::TryInto;

    use protocols::agent::CopyFileRequest;

    // FR-1a helper: evaluate `data.agent_policy.<ep>` on a policy's engine and return
    // whether it is boolean-true. Synchronous (no async runtime needed).
    fn eval_bool(p: &mut AgentPolicy, ep: &str) -> bool {
        p.engine.set_input_json("{}").unwrap();
        let r = p
            .engine
            .eval_query(format!("data.agent_policy.{ep}"), false)
            .unwrap();
        matches!(
            r.result.first().and_then(|x| x.expressions.first()).map(|e| &e.value),
            Some(regorus::Value::Bool(true))
        )
    }

    /// TC-F1.1: a verified fragment module flips a specific decision from deny→allow, and
    /// base rules are otherwise unaffected.
    #[test]
    fn test_fragment_module_flips_deny_to_allow() {
        let mut p = AgentPolicy::new();
        // Base policy: exec denied unless a fragment fact grants it.
        let base = "package agent_policy\n\
            default ExecProcessRequest := false\n\
            ExecProcessRequest := data.agent_policy.fragments.exec_allowed\n";
        p.engine.add_policy("agent_policy".to_string(), base.to_string()).unwrap();
        assert!(!eval_bool(&mut p, "ExecProcessRequest"), "denied before fragment");

        // Apply a verified fragment module in the reserved namespace.
        let module = "package agent_policy.fragments\nexec_allowed := true\n";
        p.apply_fragment_module("frag:issuerA:1", module, &[]).unwrap();
        assert!(eval_bool(&mut p, "ExecProcessRequest"), "allowed after fragment");
    }

    /// TC-F1.2: a fragment module outside the permitted fragment namespaces is rejected —
    /// it can never redefine/shadow a base rule or contribute outside its `includes`.
    #[test]
    fn test_fragment_module_namespace_is_enforced() {
        let mut p = AgentPolicy::new();
        // A module trying to live in the base package is refused.
        let base_ns = "package agent_policy\ndefault ExecProcessRequest := true\n";
        assert!(p.apply_fragment_module("evil", base_ns, &[]).is_err());

        // A sub-namespace not in `includes` is refused; one that is, is accepted.
        let mount_ns = "package agent_policy.fragments.mount\nallowed := true\n";
        assert!(p
            .apply_fragment_module("m", mount_ns, &["exec".to_string()])
            .is_err());
        assert!(p
            .apply_fragment_module("m", mount_ns, &["mount".to_string()])
            .is_ok());
    }

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
                    file_mode: libc::S_IFREG as u32,
                    path: "/foo/bar".to_owned(),
                    ..Default::default()
                },
                output: Some(PolicyCopyFileRequest {
                    file_mode: libc::S_IFREG as u32,
                    file_type: FileType::Regular,
                    path: "/foo/bar".to_owned(),
                    ..Default::default()
                }),
            },
            TestCase {
                name: "directory".to_owned(),
                input: CopyFileRequest {
                    file_mode: libc::S_IFDIR as u32,
                    path: "/foo".to_owned(),
                    ..Default::default()
                },
                output: Some(PolicyCopyFileRequest {
                    file_mode: libc::S_IFDIR as u32,
                    file_type: FileType::Directory,
                    path: "/foo".to_owned(),
                    ..Default::default()
                }),
            },
            TestCase {
                name: "socket".to_owned(),
                input: CopyFileRequest {
                    file_mode: libc::S_IFSOCK as u32,
                    path: "/foo/sock".to_owned(),
                    ..Default::default()
                },
                output: Some(PolicyCopyFileRequest {
                    file_mode: libc::S_IFSOCK as u32,
                    file_type: FileType::Unknown,
                    path: "/foo/sock".to_owned(),
                    ..Default::default()
                }),
            },
            TestCase {
                name: "mixed".to_owned(),
                input: CopyFileRequest {
                    file_mode: libc::S_IFDIR as u32 | libc::S_IFREG as u32,
                    path: "/foo/dunno".to_owned(),
                    ..Default::default()
                },
                output: Some(PolicyCopyFileRequest {
                    file_mode: libc::S_IFDIR as u32 | libc::S_IFREG as u32,
                    file_type: FileType::Unknown,
                    path: "/foo/dunno".to_owned(),
                    ..Default::default()
                }),
            },
            TestCase {
                name: "all".to_owned(),
                input: CopyFileRequest {
                    file_mode: libc::S_IFMT as u32,
                    path: "/wat".to_owned(),
                    ..Default::default()
                },
                output: Some(PolicyCopyFileRequest {
                    file_mode: libc::S_IFMT as u32,
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
                    file_mode: libc::S_IFLNK as u32,
                    path: "/foo/lnk".to_owned(),
                    ..Default::default()
                },
                output: Some(PolicyCopyFileRequest {
                    file_mode: libc::S_IFLNK as u32,
                    file_type: FileType::Symlink,
                    symlink_target: Some("..data/foo".to_owned()),
                    path: "/foo/lnk".to_owned(),
                    ..Default::default()
                }),
            },
            TestCase {
                name: "link/invalid".to_owned(),
                input: CopyFileRequest {
                    file_mode: libc::S_IFLNK as u32,
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
