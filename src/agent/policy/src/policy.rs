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

    use protobuf::MessageField;
    use protocols::agent::{
        CopyFileRequest, CreateSandboxRequest, ExecProcessRequest, KernelModule, Routes,
        SetPolicyRequest, StartContainerRequest, UpdateRoutesRequest,
    };
    use protocols::oci::{Process, Spec};
    use protocols::types::Route;
    use rstest::rstest;

    /// Serialize a protobuf request struct to a JSON string the way production
    /// code does in `src/agent/src/policy.rs` → `is_allowed_with_entrypoint`.
    macro_rules! req_json {
        ($req:expr) => {
            serde_json::to_string(&$req).expect("serde_json::to_string failed")
        };
    }

    // =========================================================================
    // Paths to the bundled .rego policies (compile-time, relative to crate root).
    // Used only by initialize() tests and the bundled-policy rstest table.
    // =========================================================================
    const ALLOW_ALL: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../kata-opa/allow-all.rego");
    const ALLOW_ALL_EXCEPT_EXEC: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../kata-opa/allow-all-except-exec-process.rego"
    );
    const ALLOW_SET_POLICY: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../kata-opa/allow-set-policy.rego"
    );

    // =========================================================================
    // Helpers
    // =========================================================================

    /// Load a bundled .rego file directly into a fresh engine, bypassing
    /// `initialize()` so no log file or default-policy path is required.
    async fn policy_with_file(path: &str) -> AgentPolicy {
        let mut p = AgentPolicy::new();
        p.engine.add_policy_from_file(path).unwrap();
        p
    }

    /// Load an inline Rego string into a fresh engine via `set_policy()`.
    async fn policy_with_str(rego: &str) -> AgentPolicy {
        let mut p = AgentPolicy::new();
        p.set_policy(rego).await.unwrap();
        p
    }

    /// Assert a single `allow_request` call on an already-loaded engine.
    ///
    /// - `want_allowed`   — expected `allowed` flag.
    /// - `want_print`     — if `Some(s)`, asserts `prints.contains(s)`.
    /// - `want_not_print` — if `Some(s)`, asserts `!prints.contains(s)`.
    ///
    /// An `Err` result is treated as `allowed = false` so callers don't need a
    /// separate `match` block for policies that produce no result for an endpoint.
    async fn assert_allow(
        p: &mut AgentPolicy,
        ep: &str,
        input: &str,
        want_allowed: bool,
        want_print: Option<&str>,
    ) {
        assert_allow_ext(p, ep, input, want_allowed, want_print, None).await;
    }

    /// Extended form used when a `want_not_print` check is also needed.
    async fn assert_allow_ext(
        p: &mut AgentPolicy,
        ep: &str,
        input: &str,
        want_allowed: bool,
        want_print: Option<&str>,
        want_not_print: Option<&str>,
    ) {
        let (allowed, prints) = match p.allow_request(ep, input).await {
            Ok(pair) => pair,
            Err(e) => {
                assert!(
                    !want_allowed,
                    "allow_request({ep}) returned Err ({e}), expected allow=true",
                    ep = ep,
                    e = e,
                );
                return; // Err counts as denied — nothing more to check
            }
        };
        assert_eq!(
            allowed,
            want_allowed,
            "ep={ep} input={input}: got allowed={allowed}, want {want_allowed}\nprints={prints}",
            ep = ep,
            input = input,
            allowed = allowed,
            want_allowed = want_allowed,
            prints = prints,
        );
        if let Some(needle) = want_print {
            assert!(
                prints.contains(needle),
                "ep={ep}: expected {needle:?} in prints, got: {prints:?}",
                ep = ep,
                needle = needle,
                prints = prints,
            );
        }
        if let Some(needle) = want_not_print {
            assert!(
                !prints.contains(needle),
                "ep={ep}: expected {needle:?} NOT in prints, got: {prints:?}",
                ep = ep,
                needle = needle,
                prints = prints,
            );
        }
    }

    // =========================================================================
    // CopyFileRequest translation tests (pre-existing, ported to rstest)
    // =========================================================================

    // Successful conversions: (file_mode, path, data, expected_output)
    #[rstest]
    #[case(
        "regular",
        CopyFileRequest { file_mode: libc::S_IFREG as u32, path: "/foo/bar".to_owned(), ..Default::default() },
        Some(PolicyCopyFileRequest { file_mode: libc::S_IFREG as u32, file_type: FileType::Regular, path: "/foo/bar".to_owned(), ..Default::default() }),
    )]
    #[case(
        "directory",
        CopyFileRequest { file_mode: libc::S_IFDIR as u32, path: "/foo".to_owned(), ..Default::default() },
        Some(PolicyCopyFileRequest { file_mode: libc::S_IFDIR as u32, file_type: FileType::Directory, path: "/foo".to_owned(), ..Default::default() }),
    )]
    #[case(
        "socket",
        CopyFileRequest { file_mode: libc::S_IFSOCK as u32, path: "/foo/sock".to_owned(), ..Default::default() },
        Some(PolicyCopyFileRequest { file_mode: libc::S_IFSOCK as u32, file_type: FileType::Unknown, path: "/foo/sock".to_owned(), ..Default::default() }),
    )]
    #[case(
        "mixed",
        CopyFileRequest { file_mode: libc::S_IFDIR as u32 | libc::S_IFREG as u32, path: "/foo/dunno".to_owned(), ..Default::default() },
        Some(PolicyCopyFileRequest { file_mode: libc::S_IFDIR as u32 | libc::S_IFREG as u32, file_type: FileType::Unknown, path: "/foo/dunno".to_owned(), ..Default::default() }),
    )]
    #[case(
        "all",
        CopyFileRequest { file_mode: libc::S_IFMT as u32, path: "/wat".to_owned(), ..Default::default() },
        Some(PolicyCopyFileRequest { file_mode: libc::S_IFMT as u32, file_type: FileType::Unknown, path: "/wat".to_owned(), ..Default::default() }),
    )]
    #[case(
        "none",
        CopyFileRequest { file_mode: 0, path: "/0".to_owned(), ..Default::default() },
        Some(PolicyCopyFileRequest { file_mode: 0, file_type: FileType::Unknown, path: "/0".to_owned(), ..Default::default() }),
    )]
    #[case(
        "link/valid",
        CopyFileRequest { data: b"..data/foo".to_vec(), file_mode: libc::S_IFLNK as u32, path: "/foo/lnk".to_owned(), ..Default::default() },
        Some(PolicyCopyFileRequest { file_mode: libc::S_IFLNK as u32, file_type: FileType::Symlink, symlink_target: Some("..data/foo".to_owned()), path: "/foo/lnk".to_owned(), ..Default::default() }),
    )]
    #[case(
        "link/invalid",
        CopyFileRequest { file_mode: libc::S_IFLNK as u32, data: vec![0x00, 0xFF, 0xFF, 0x00], ..Default::default() },
        None,
    )]
    fn test_copyfile_translation(
        #[case] _label: &str,
        #[case] input: CopyFileRequest,
        #[case] expected: Option<PolicyCopyFileRequest>,
    ) {
        let result: Result<PolicyCopyFileRequest> = (&input).try_into();
        match expected {
            Some(want) => assert_eq!(
                result.unwrap_or_else(|e| panic!(
                    "test case {_label}: unexpected Err: {e}",
                    _label = _label,
                    e = e
                )),
                want,
                "test case {_label}",
                _label = _label,
            ),
            None => assert!(
                result.is_err(),
                "test case {_label}: expected Err, got {result:?}",
                _label = _label,
                result = result,
            ),
        }
    }

    // =========================================================================
    // OPA engine tests
    //
    // Convention for the return value of allow_request:
    //   Ok((true,  prints)) — request allowed; prints is the policy print() log
    //   Ok((false, prints)) — request denied;  prints carries any debug output
    //   Err(e)              — engine error (bad JSON, bad policy, …)
    //
    // Use assert_allow() for the common pattern of "call allow_request, check
    // flag, optionally check a prints substring".  Reserve inline assertions
    // only when additional context (e.g. multiple prints checks) is needed.
    // =========================================================================

    // -------------------------------------------------------------------------
    // test_validate_and_invalid_input
    //
    // Combines TestValidate + TestValidateWithInvalidInput:
    // - matching input → allowed
    // - non-matching → denied
    // - bare non-JSON string → Err with non-empty message
    // -------------------------------------------------------------------------
    #[tokio::test]
    async fn test_validate_and_invalid_input() {
        const REGO: &str = r#"
            package agent_policy
            default allow = false
            allow if { input.foo == "bar" }
        "#;
        let mut p = policy_with_str(REGO).await;

        assert_allow(&mut p, "allow", r#"{"foo":"bar"}"#, true, None).await;
        assert_allow(&mut p, "allow", r#"{"foo":"buzz"}"#, false, None).await;

        let err = p.allow_request("allow", "not-json").await.unwrap_err();
        assert!(
            !err.to_string().is_empty(),
            "error message must be non-empty"
        );
    }

    // -------------------------------------------------------------------------
    // test_set_policy_invalid_inputs
    //
    // Combines TestPolicyEmpty_Fail + TestInvalidPolicyFail: both empty string
    // and syntactically broken Rego must return Err with a non-empty message.
    // -------------------------------------------------------------------------
    #[rstest]
    #[case("empty", "")]
    #[case("bad_syntax", "not valid rego {{{")]
    #[tokio::test]
    async fn test_set_policy_invalid_inputs(#[case] _label: &str, #[case] policy: &str) {
        let mut p = AgentPolicy::new();
        let err = p.set_policy(policy).await.unwrap_err();
        assert!(
            !err.to_string().is_empty(),
            "expected non-empty error for invalid policy ({_label})",
            _label = _label,
        );
    }

    // -------------------------------------------------------------------------
    // test_allow_except_exec_denies_exec
    //
    // Bundled allow-all-except-exec-process.rego: ExecProcessRequest := false
    // while all other endpoints remain true.
    // -------------------------------------------------------------------------
    #[tokio::test]
    async fn test_allow_except_exec_denies_exec() {
        let mut p = policy_with_file(ALLOW_ALL_EXCEPT_EXEC).await;
        // Use a real ExecProcessRequest struct (default / empty) serialised the
        // same way production code does.
        let exec_req = req_json!(ExecProcessRequest::default());
        assert_allow(&mut p, "ExecProcessRequest", &exec_req, false, None).await;

        let start_req = req_json!(StartContainerRequest::default());
        assert_allow(&mut p, "StartContainerRequest", &start_req, true, None).await;
    }

    // -------------------------------------------------------------------------
    // test_initialize_loads_policy / test_initialize_missing_file_errors
    //
    // After initialize() with allow-all.rego requests succeed (policy in memory).
    // A non-existent path must return Err with a non-empty message.
    // -------------------------------------------------------------------------
    #[tokio::test]
    async fn test_initialize_loads_policy() {
        let mut p = AgentPolicy::new();
        p.initialize(0, ALLOW_ALL.to_string(), None).await.unwrap();
        // Use a real struct to verify end-to-end serialisation through initialize().
        let req = req_json!(StartContainerRequest {
            container_id: "test-container".to_owned(),
            ..Default::default()
        });
        assert_allow(&mut p, "StartContainerRequest", &req, true, None).await;
    }

    #[tokio::test]
    async fn test_initialize_missing_file_errors() {
        let mut p = AgentPolicy::new();
        let err = p
            .initialize(0, "/nonexistent/policy.rego".to_string(), None)
            .await
            .unwrap_err();
        assert!(
            !err.to_string().is_empty(),
            "error message must be non-empty"
        );
    }

    // -------------------------------------------------------------------------
    // Bundled policy smoke-tests
    //
    // Verify the three shipped .rego files against representative endpoints.
    // allow-set-policy only defines SetPolicyRequest; every other endpoint
    // produces an empty eval result → Err → treated as deny.
    //
    //   policy file           | endpoint           | expected
    //   ----------------------|--------------------|--------
    //   allow-all             | ExecProcessRequest | true
    //   allow-all-except-exec | ExecProcessRequest | false   (also tested by test_allow_except_exec_denies_exec)
    //   allow-all             | SetPolicyRequest   | true
    //   allow-set-policy      | SetPolicyRequest   | true
    // -------------------------------------------------------------------------
    #[rstest]
    #[case(ALLOW_ALL, "ExecProcessRequest", true)]
    #[case(ALLOW_ALL_EXCEPT_EXEC, "ExecProcessRequest", false)]
    #[case(ALLOW_ALL, "SetPolicyRequest", true)]
    #[case(ALLOW_SET_POLICY, "SetPolicyRequest", true)]
    #[tokio::test]
    async fn test_bundled_policy(
        #[case] policy_file: &str,
        #[case] endpoint: &str,
        #[case] expected: bool,
    ) {
        let mut p = policy_with_file(policy_file).await;
        let req = match endpoint {
            "SetPolicyRequest" => req_json!(SetPolicyRequest {
                policy: "x".to_owned(),
                ..Default::default()
            }),
            _ => req_json!(ExecProcessRequest::default()),
        };
        assert_allow(&mut p, endpoint, &req, expected, None).await;
    }

    // =========================================================================
    // Advanced tests — patterns from src/tools/genpolicy/rules.rego
    //
    // Each test uses an inline policy mirroring a specific rules.rego pattern
    // and asserts both the allow/deny flag AND specific print() trace output.
    // =========================================================================

    // -------------------------------------------------------------------------
    // print() observability — trace messages appear on both allow and deny paths;
    // the terminal "allowed" print is absent when the rule is not satisfied.
    //
    // Mirrors the dense print() instrumentation throughout rules.rego.
    // -------------------------------------------------------------------------
    #[tokio::test]
    async fn test_prints_captured_on_allow_and_deny() {
        let mut p = policy_with_str(
            r#"
            package agent_policy
            default CopyFileRequest := false
            CopyFileRequest if {
                print("CopyFileRequest: checking path =", input.path)
                startswith(input.path, "/tmp/")
                print("CopyFileRequest: allowed")
            }
            "#,
        )
        .await;

        let allow_req = req_json!(PolicyCopyFileRequest {
            path: "/tmp/test".to_owned(),
            ..Default::default()
        });
        let deny_req = req_json!(PolicyCopyFileRequest {
            path: "/etc/passwd".to_owned(),
            ..Default::default()
        });

        // Allow path: both prints fire — check for terminal print (implies opening also fired).
        assert_allow_ext(
            &mut p,
            "CopyFileRequest",
            &allow_req,
            true,
            Some("CopyFileRequest: allowed"),
            None,
        )
        .await;

        // Deny path: opening print fires, terminal does not.
        assert_allow_ext(
            &mut p,
            "CopyFileRequest",
            &deny_req,
            false,
            Some("CopyFileRequest: checking path"),
            Some("CopyFileRequest: allowed"),
        )
        .await;
    }

    // -------------------------------------------------------------------------
    // CopyFileRequest file_type matrix — Regular / Directory / Symlink branches
    // from rules.rego allow_copy_file, including traversal + symlink-target checks.
    //
    // A shared const carries the policy used by both the parametrized table and
    // the dedicated print-assertion test, avoiding duplicate inline Rego strings.
    // -------------------------------------------------------------------------

    /// Shared CopyFileRequest policy used by test_copy_file_type_path_cases
    /// and test_copy_file_symlink_prints.
    const COPY_FILE_REGO: &str = r#"
        package agent_policy
        import future.keywords.if

        default CopyFileRequest := false

        CopyFileRequest if {
            print("CopyFileRequest: file_type =", input.file_type, "path =", input.path)
            input.file_type == "Regular"
            startswith(input.path, "/data/")
            not regex.match("(^|/)\\.\\.($|/)", input.path)
            print("CopyFileRequest regular: true")
        }
        CopyFileRequest if {
            print("CopyFileRequest: file_type =", input.file_type, "path =", input.path)
            input.file_type == "Directory"
            startswith(input.path, "/data/")
            not regex.match("(^|/)\\.\\.($|/)", input.path)
            print("CopyFileRequest directory: true")
        }
        CopyFileRequest if {
            print("CopyFileRequest: file_type =", input.file_type, "path =", input.path)
            input.file_type == "Symlink"
            regex.match("^/.+/.+", input.path)
            not startswith(input.symlink_target, "/")
            not regex.match("(^|/)\\.\\.($|/)", input.symlink_target)
            print("CopyFileRequest symlink: true")
        }
    "#;

    // Notes on edge cases documented in test labels:
    // - "regular-traversal": /data/../etc/shadow contains `..` → traversal denied
    // - "symlink-ok": plain relative target "sibling" (not "../sibling") — `../`
    //   matches the traversal regex `(^|/)\.\.($|/)` at the start, so only
    //   targets without any `..` component are safe
    // - "symlink-toplevel": /link has only one path component; the nested-path
    //   regex `^/.+/.+` requires at least two, so /link is denied
    /// Build a [`PolicyCopyFileRequest`] JSON string (the pre-processed form
    /// that production `rpc.rs` passes to `is_allowed_with_entrypoint` for
    /// `CopyFileRequest`).
    fn copy_req(file_type: FileType, path: &str, symlink_target: Option<&str>) -> String {
        req_json!(PolicyCopyFileRequest {
            file_type,
            path: path.to_owned(),
            symlink_target: symlink_target.map(str::to_owned),
            ..Default::default()
        })
    }

    #[rstest]
    #[case("regular-ok", FileType::Regular, "/data/cfg.json", None, true)]
    #[case(
        "regular-traversal",
        FileType::Regular,
        "/data/../etc/shadow",
        None,
        false
    )]
    #[case("regular-bad-root", FileType::Regular, "/etc/passwd", None, false)]
    #[case("dir-ok", FileType::Directory, "/data/subdir", None, true)]
    #[case("dir-traversal", FileType::Directory, "/data/../secret", None, false)]
    #[case(
        "symlink-ok",
        FileType::Symlink,
        "/data/subdir/lnk",
        Some("sibling"),
        true
    )]
    #[case(
        "symlink-abs",
        FileType::Symlink,
        "/data/subdir/lnk",
        Some("/etc/passwd"),
        false
    )]
    #[case("symlink-toplevel", FileType::Symlink, "/link", Some("sibling"), false)]
    #[tokio::test]
    async fn test_copy_file_type_path_cases(
        #[case] _label: &str,
        #[case] file_type: FileType,
        #[case] path: &str,
        #[case] symlink_target: Option<&str>,
        #[case] expected: bool,
    ) {
        let mut p = policy_with_str(COPY_FILE_REGO).await;
        let input = copy_req(file_type, path, symlink_target);
        assert_allow(&mut p, "CopyFileRequest", &input, expected, None).await;
    }

    /// Verifies that the branch-level print() messages fire correctly for
    /// symlinks: "symlink: true" only on allow; "file_type" trace on deny.
    #[tokio::test]
    async fn test_copy_file_symlink_prints() {
        let mut p = policy_with_str(COPY_FILE_REGO).await;

        let allow_input = copy_req(FileType::Symlink, "/data/subdir/link", Some("sibling"));
        assert_allow(
            &mut p,
            "CopyFileRequest",
            &allow_input,
            true,
            Some("CopyFileRequest symlink: true"),
        )
        .await;

        let deny_input = copy_req(FileType::Symlink, "/data/subdir/link", Some("/etc/passwd"));
        assert_allow(
            &mut p,
            "CopyFileRequest",
            &deny_input,
            false,
            Some("CopyFileRequest: file_type"),
        )
        .await;
    }

    // -------------------------------------------------------------------------
    // CreateSandboxRequest — rules.rego pattern: guest_hook_path, kernel_modules,
    // and sandbox_pidns are each independently enforced with print() tracing.
    //
    // Uses real CreateSandboxRequest structs serialised via serde_json::to_string().
    // -------------------------------------------------------------------------

    const CREATE_SANDBOX_REGO: &str = r#"
        package agent_policy
        import future.keywords.if
        default CreateSandboxRequest := false
        CreateSandboxRequest if {
            print("CreateSandboxRequest: guest_hook_path =", input.guest_hook_path)
            count(input.guest_hook_path) == 0
            print("CreateSandboxRequest: kernel_modules =", input.kernel_modules)
            count(input.kernel_modules) == 0
            print("CreateSandboxRequest: sandbox_pidns =", input.sandbox_pidns)
            input.sandbox_pidns == false
            print("CreateSandboxRequest: true")
        }
    "#;

    #[tokio::test]
    async fn test_create_sandbox_valid() {
        let mut p = policy_with_str(CREATE_SANDBOX_REGO).await;
        let req = req_json!(CreateSandboxRequest {
            guest_hook_path: "".to_owned(),
            sandbox_pidns: false,
            ..Default::default()
        });
        assert_allow(
            &mut p,
            "CreateSandboxRequest",
            &req,
            true,
            Some("CreateSandboxRequest: true"),
        )
        .await;
    }

    #[tokio::test]
    async fn test_create_sandbox_hook_path_blocked() {
        let mut p = policy_with_str(CREATE_SANDBOX_REGO).await;
        let req = req_json!(CreateSandboxRequest {
            guest_hook_path: "/usr/share/oci/hooks".to_owned(),
            sandbox_pidns: false,
            ..Default::default()
        });
        assert_allow(
            &mut p,
            "CreateSandboxRequest",
            &req,
            false,
            Some("guest_hook_path"),
        )
        .await;
    }

    #[tokio::test]
    async fn test_create_sandbox_pidns_blocked() {
        let mut p = policy_with_str(CREATE_SANDBOX_REGO).await;
        let req = req_json!(CreateSandboxRequest {
            guest_hook_path: "".to_owned(),
            sandbox_pidns: true,
            ..Default::default()
        });
        assert_allow(&mut p, "CreateSandboxRequest", &req, false, None).await;
    }

    #[tokio::test]
    async fn test_create_sandbox_kernel_modules_blocked() {
        let mut p = policy_with_str(CREATE_SANDBOX_REGO).await;
        let mut km = KernelModule::new();
        km.name = "virtio_blk".to_owned();
        let req = req_json!(CreateSandboxRequest {
            guest_hook_path: "".to_owned(),
            sandbox_pidns: false,
            kernel_modules: vec![km],
            ..Default::default()
        });
        assert_allow(&mut p, "CreateSandboxRequest", &req, false, None).await;
    }

    // -------------------------------------------------------------------------
    // ExecProcessRequest — rules.rego allow_exec_process_input:
    // string_user must be null, SelinuxLabel and ApparmorProfile must be empty.
    //
    // Uses real ExecProcessRequest + oci::Process structs serialised via
    // serde_json::to_string() so field names are guaranteed to match what the
    // Rego rule inspects (string_user → null, process.Args, process.SelinuxLabel,
    // process.ApparmorProfile).
    // -------------------------------------------------------------------------

    const EXEC_PROCESS_REGO: &str = r#"
        package agent_policy
        import future.keywords.if
        default ExecProcessRequest := false
        ExecProcessRequest if {
            print("ExecProcessRequest: checking input")
            is_null(input.string_user)
            i_process := input.process
            count(i_process.SelinuxLabel) == 0
            count(i_process.ApparmorProfile) == 0
            print("ExecProcessRequest: Args =", i_process.Args)
            i_process.Args[0] == "/bin/sh"
            print("ExecProcessRequest: true")
        }
    "#;

    /// Build an ExecProcessRequest with no string_user (serialises as null)
    /// and the given process args + optional SelinuxLabel.
    fn exec_req(args: &[&str], selinux_label: &str) -> ExecProcessRequest {
        let mut proc = Process::new();
        proc.Args = args.iter().map(|s| s.to_string()).collect();
        proc.SelinuxLabel = selinux_label.to_owned();
        ExecProcessRequest {
            process: MessageField::some(proc),
            // string_user left as MessageField::none() → serialises as null
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn test_exec_process_valid() {
        let mut p = policy_with_str(EXEC_PROCESS_REGO).await;
        let req = req_json!(exec_req(&["/bin/sh", "-c", "ls"], ""));
        assert_allow(
            &mut p,
            "ExecProcessRequest",
            &req,
            true,
            Some("ExecProcessRequest: true"),
        )
        .await;
    }

    #[tokio::test]
    async fn test_exec_process_wrong_cmd() {
        let mut p = policy_with_str(EXEC_PROCESS_REGO).await;
        let req = req_json!(exec_req(&["/bin/bash", "-c", "ls"], ""));
        assert_allow(
            &mut p,
            "ExecProcessRequest",
            &req,
            false,
            Some("ExecProcessRequest: Args"),
        )
        .await;
    }

    #[tokio::test]
    async fn test_exec_process_selinux_blocked() {
        let mut p = policy_with_str(EXEC_PROCESS_REGO).await;
        let req = req_json!(exec_req(&["/bin/sh"], "system_u:system_r:container_t:s0"));
        assert_allow(&mut p, "ExecProcessRequest", &req, false, None).await;
    }

    // -------------------------------------------------------------------------
    // Annotation key allowlisting — rules.rego allow_anno_key_value:
    // io.kubernetes.cri.* prefix always permitted; unknown keys denied.
    // -------------------------------------------------------------------------
    #[tokio::test]
    async fn test_annotation_key_allowlisting() {
        let mut p = policy_with_str(
            r#"
            package agent_policy
            import future.keywords.every
            import future.keywords.if
            default CreateContainerRequest := false
            CreateContainerRequest if {
                print("CreateContainerRequest: checking annotations")
                every i_key, _ in input.OCI.Annotations { allow_annotation_key(i_key) }
                print("CreateContainerRequest: annotations ok")
            }
            allow_annotation_key(key) if {
                print("allow_annotation_key: key =", key)
                startswith(key, "io.kubernetes.cri.")
                print("allow_annotation_key: cri prefix allowed")
            }
            allow_annotation_key(key) if {
                print("allow_annotation_key: key =", key)
                key == "io.katacontainers.pkg.oci.container_type"
                print("allow_annotation_key: kata key allowed")
            }
            "#,
        )
        .await;

        let mut allowed_spec = Spec::new();
        allowed_spec.Annotations = [
            (
                "io.kubernetes.cri.container-type".to_owned(),
                "container".to_owned(),
            ),
            (
                "io.kubernetes.cri.sandbox-name".to_owned(),
                "my-pod".to_owned(),
            ),
            (
                "io.katacontainers.pkg.oci.container_type".to_owned(),
                "pod_container".to_owned(),
            ),
        ]
        .into();
        let allow_req = req_json!(protocols::agent::CreateContainerRequest {
            OCI: MessageField::some(allowed_spec),
            ..Default::default()
        });

        let mut denied_spec = Spec::new();
        denied_spec.Annotations = [
            (
                "io.kubernetes.cri.container-type".to_owned(),
                "container".to_owned(),
            ),
            (
                "com.example.custom-label".to_owned(),
                "sneaky-value".to_owned(),
            ),
        ]
        .into();
        let deny_req = req_json!(protocols::agent::CreateContainerRequest {
            OCI: MessageField::some(denied_spec),
            ..Default::default()
        });

        assert_allow(
            &mut p,
            "CreateContainerRequest",
            &allow_req,
            true,
            Some("CreateContainerRequest: annotations ok"),
        )
        .await;
        assert_allow(
            &mut p,
            "CreateContainerRequest",
            &deny_req,
            false,
            Some("allow_annotation_key: key"),
        )
        .await;
    }

    // -------------------------------------------------------------------------
    // Regex-based sandbox name matching — rules.rego allow_sandbox_name.
    // -------------------------------------------------------------------------
    #[rstest]
    #[case(
        "match",
        "my-app-a1b2c",
        true,
        Some("CreateSandboxRequest: name match ok")
    )]
    #[case("too-short", "my-app-ab", false, None)]
    #[case("wrong-prefix", "other-a1b2c", false, None)]
    #[tokio::test]
    async fn test_regex_sandbox_name_matching(
        #[case] _label: &str,
        #[case] hostname: &str,
        #[case] expected: bool,
        #[case] want_print: Option<&str>,
    ) {
        let mut p = policy_with_str(
            r#"
            package agent_policy
            import future.keywords.if
            default CreateSandboxRequest := false
            CreateSandboxRequest if {
                p_name_regex := "^my-app-[a-z0-9]{5}$"
                print("CreateSandboxRequest: p_name_regex =", p_name_regex)
                print("CreateSandboxRequest: i_name =", input.hostname)
                regex.match(p_name_regex, input.hostname)
                print("CreateSandboxRequest: name match ok")
            }
            "#,
        )
        .await;
        let req = req_json!(CreateSandboxRequest {
            hostname: hostname.to_owned(),
            ..Default::default()
        });
        assert_allow(&mut p, "CreateSandboxRequest", &req, expected, want_print).await;
    }

    // -------------------------------------------------------------------------
    // AllowRequestsFailingPolicy — rules.rego debug bypass flag.
    //
    // allow_failures=true overrides:
    //   • explicit `default := false` results (not just empty/undefined ones)
    //   • completely undefined endpoints (Err → Ok(true))
    // -------------------------------------------------------------------------
    #[tokio::test]
    async fn test_allow_failures_bypass() {
        let mut p = policy_with_str(
            r#"
            package agent_policy
            import future.keywords.if
            default AllowRequestsFailingPolicy := true
            default ExecProcessRequest := false
            "#,
        )
        .await;

        // Explicit false is overridden by the bypass
        let exec_req = req_json!(ExecProcessRequest::default());
        assert_allow(&mut p, "ExecProcessRequest", &exec_req, true, None).await;

        // Completely undefined endpoint → Ok(true) instead of Err
        let result = p.allow_request("UndefinedEndpoint", "{}").await;
        assert!(
            result.is_ok(),
            "allow_failures must convert undefined endpoint to Ok, got: {result:?}",
            result = result,
        );
        let (allowed, _) = result.unwrap();
        assert!(
            allowed,
            "allow_failures must return true for undefined endpoint"
        );
    }

    // -------------------------------------------------------------------------
    // UpdateRoutesRequest — rules.rego forbidden_source_regex /
    // forbidden_device_names, with per-route print() tracing.
    // -------------------------------------------------------------------------

    /// Build an [`UpdateRoutesRequest`] JSON string from (source, device) pairs.
    ///
    /// `UpdateRoutesRequest.routes` wraps `Routes { Routes: Vec<Route> }`, so the
    /// Rego rule iterates `input.routes.Routes[_]` not `input.routes[_]`.
    fn update_routes_req(pairs: &[(&str, &str)]) -> String {
        let route_list: Vec<Route> = pairs
            .iter()
            .map(|(src, dev)| Route {
                source: src.to_string(),
                device: dev.to_string(),
                ..Default::default()
            })
            .collect();
        let mut routes_msg = Routes::new();
        routes_msg.Routes = route_list;
        req_json!(UpdateRoutesRequest {
            routes: MessageField::some(routes_msg),
            ..Default::default()
        })
    }

    #[rstest]
    #[case("valid-routes",    &[("10.0.0.0","eth0"),("192.168.1.0","eth1")][..], true,  Some("UpdateRoutesRequest: true"))]
    #[case("link-local",      &[("169.254.0.1","eth0")][..],                     false, Some("route.source"))]
    #[case("loopback-device", &[("10.0.0.0","lo")][..],                          false, None)]
    #[tokio::test]
    async fn test_update_routes_request(
        #[case] _label: &str,
        #[case] routes: &[(&str, &str)],
        #[case] expected: bool,
        #[case] want_print: Option<&str>,
    ) {
        let mut p = policy_with_str(
            r#"
            package agent_policy
            import future.keywords.every
            import future.keywords.if
            default UpdateRoutesRequest := false
            UpdateRoutesRequest if {
                print("UpdateRoutesRequest: checking routes")
                forbidden_source_regex := ["^169\\.254\\..*", "^127\\..*"]
                forbidden_devices := ["lo", "docker0"]
                every route in input.routes.Routes {
                    print("UpdateRoutesRequest: route.source =", route.source)
                    every regex in forbidden_source_regex { not regex.match(regex, route.source) }
                    print("UpdateRoutesRequest: route.device =", route.device)
                    not route.device in forbidden_devices
                }
                print("UpdateRoutesRequest: true")
            }
            "#,
        )
        .await;
        let input = update_routes_req(routes);
        assert_allow(&mut p, "UpdateRoutesRequest", &input, expected, want_print).await;
    }

    // -------------------------------------------------------------------------
    // set_policy atomically replaces the engine — old policy gone, new one active.
    // -------------------------------------------------------------------------
    #[tokio::test]
    async fn test_set_policy_replaces_previous_policy() {
        let mut p = AgentPolicy::new();

        p.set_policy(
            r#"package agent_policy
               default ExecProcessRequest := false
               ExecProcessRequest if { print("policy v1: Args =", input.process.Args)
                                       input.process.Args[0] == "ping" }"#,
        )
        .await
        .unwrap();

        assert_allow(
            &mut p,
            "ExecProcessRequest",
            &req_json!(exec_req(&["ping", "8.8.8.8"], "")),
            true,
            Some("policy v1"),
        )
        .await;
        assert_allow(
            &mut p,
            "ExecProcessRequest",
            &req_json!(exec_req(&["curl", "http://example.com"], "")),
            false,
            None,
        )
        .await;

        p.set_policy(
            r#"package agent_policy
               default ExecProcessRequest := false
               ExecProcessRequest if { print("policy v2: Args =", input.process.Args)
                                       input.process.Args[0] == "curl" }"#,
        )
        .await
        .unwrap();

        assert_allow(
            &mut p,
            "ExecProcessRequest",
            &req_json!(exec_req(&["curl", "http://example.com"], "")),
            true,
            Some("policy v2"),
        )
        .await;
        assert_allow(
            &mut p,
            "ExecProcessRequest",
            &req_json!(exec_req(&["ping", "8.8.8.8"], "")),
            false,
            None,
        )
        .await;
    }
}
