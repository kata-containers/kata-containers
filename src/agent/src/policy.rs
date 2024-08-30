// Copyright (c) 2023 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::Result;
use protobuf::MessageDyn;
use tokio::io::AsyncWriteExt;

use crate::rpc::ttrpc_error;
use crate::{AGENT_CONFIG, AGENT_POLICY};

static POLICY_LOG_FILE: &str = "/tmp/policy.txt";

/// Convenience macro to obtain the scope logger
macro_rules! sl {
    () => {
        slog_scope::logger()
    };
}

async fn allow_request(policy: &mut AgentPolicy, ep: &str, request: &str) -> ttrpc::Result<()> {
    match policy.allow_request(ep, request).await {
        Ok((allowed, prints)) => {
            if allowed {
                Ok(())
            } else {
                Err(ttrpc_error(
                    ttrpc::Code::PERMISSION_DENIED,
                    format!("{ep} is blocked by policy: {prints}"),
                ))
            }
        }
        Err(e) => Err(ttrpc_error(
            ttrpc::Code::INTERNAL,
            format!("{ep}: internal error {e}"),
        )),
    }
}

pub async fn is_allowed(req: &(impl MessageDyn + serde::Serialize)) -> ttrpc::Result<()> {
    let request = serde_json::to_string(req).unwrap();
    let mut policy = AGENT_POLICY.lock().await;
    allow_request(&mut policy, req.descriptor_dyn().name(), &request).await
}

pub async fn do_set_policy(req: &protocols::agent::SetPolicyRequest) -> ttrpc::Result<()> {
    let request = serde_json::to_string(req).unwrap();
    let mut policy = AGENT_POLICY.lock().await;
    allow_request(&mut policy, "SetPolicyRequest", &request).await?;
    policy
        .set_policy(&req.policy)
        .await
        .map_err(|e| ttrpc_error(ttrpc::Code::INVALID_ARGUMENT, e))
}

/// Singleton policy object.
#[derive(Debug, Default)]
pub struct AgentPolicy {
    /// When true policy errors are ignored, for debug purposes.
    allow_failures: bool,

    /// "/tmp/policy.txt" log file for policy activity.
    log_file: Option<tokio::fs::File>,

    /// Regorus engine
    engine: regorus::Engine,
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
        engine
    }

    /// Initialize regorus.
    pub async fn initialize(&mut self, default_policy_file: &str) -> Result<()> {
        if AGENT_CONFIG.log_level.as_usize() >= slog::Level::Debug.as_usize() {
            self.log_file = Some(
                tokio::fs::OpenOptions::new()
                    .write(true)
                    .truncate(true)
                    .create(true)
                    .open(POLICY_LOG_FILE)
                    .await?,
            );
            debug!(sl!(), "policy: log file: {}", POLICY_LOG_FILE);
        }

        self.engine.add_policy_from_file(default_policy_file)?;
        self.update_allow_failures_flag().await?;
        Ok(())
    }

    /// Ask regorus if an API call should be allowed or not.
    async fn allow_request(&mut self, ep: &str, ep_input: &str) -> Result<(bool, String)> {
        debug!(sl!(), "policy check: {ep}");
        self.log_eval_input(ep, ep_input).await;

        let query = format!("data.agent_policy.{ep}");
        self.engine.set_input_json(ep_input)?;

        let mut allow = match self.engine.eval_bool_query(query, false) {
            Ok(a) => a,
            Err(e) => {
                if !self.allow_failures {
                    return Err(e);
                }
                false
            }
        };

        if !allow && self.allow_failures {
            warn!(sl!(), "policy: ignoring error for {ep}");
            allow = true;
        }

        let prints = match self.engine.take_prints() {
            Ok(p) => p.join(" "),
            Err(e) => format!("Failed to get policy log: {e}"),
        };

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
                    let log_entry = format!("[\"ep\":\"{ep}\",{input}],\n\n");

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
