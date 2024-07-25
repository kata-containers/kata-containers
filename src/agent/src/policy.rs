// Copyright (c) 2023 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{bail, Result};
use protobuf::MessageDyn;
use serde::{Deserialize, Serialize};
use serde_json::json;
use slog::Drain;
use tokio::io::AsyncWriteExt;

use crate::rpc::ttrpc_error;
use crate::AGENT_POLICY;

static POLICY_LOG_FILE: &str = "/tmp/policy.txt";

/// Convenience macro to obtain the scope logger
macro_rules! sl {
    () => {
        slog_scope::logger()
    };
}

async fn allow_request(
    policy: &mut AgentPolicy,
    ep: &str,
    req: &(impl MessageDyn + serde::Serialize),
) -> ttrpc::Result<()> {
    match policy.allow_request(ep, req).await {
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
    let mut policy = AGENT_POLICY.lock().await;
    allow_request(&mut policy, req.descriptor_dyn().name(), req).await
}

pub async fn do_set_policy(req: &protocols::agent::SetPolicyRequest) -> ttrpc::Result<()> {
    let mut policy = AGENT_POLICY.lock().await;
    allow_request(&mut policy, "SetPolicyRequest", req).await?;
    policy
        .set_policy(&req.policy)
        .await
        .map_err(|e| ttrpc_error(ttrpc::Code::INVALID_ARGUMENT, e))
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct AgentPolicyState {
    #[serde(skip_serializing_if = "Option::is_none")]
    sandbox_name: Option<String>,
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

    state: AgentPolicyState,
}

#[allow(unused)]
#[derive(serde::Deserialize, Debug)]
struct MetadataResponse {
    allowed: bool,
    metadata: Option<Vec<Option<Metadata>>>,
}

#[allow(unused)]
#[derive(serde::Deserialize, Debug)]
struct Metadata {
    action: String,
    name: String,
    key: String,
    value: serde_json::Value,
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
        if sl!().is_enabled(slog::Level::Debug) {
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
        self.engine.set_input_json("{}")?;
        Ok(())
    }

    /// Ask regorus if an API call should be allowed or not.
    // async fn allow_request(&mut self, ep: &str, ep_input: &str) -> Result<(bool, String)> {
    async fn allow_request(
        &mut self,
        ep: &str,
        req: &(impl MessageDyn + serde::Serialize),
    ) -> Result<(bool, String)> {
        let mut root_value = serde_json::to_value(req).unwrap();
        root_value["policy_state"] = serde_json::to_value(&self.state).unwrap();
        let ep_input = serde_json::to_string(&root_value).unwrap();

        return self.allow_request_string(ep, &ep_input).await;
    }

    async fn process_metadata(&mut self, metadata: Vec<Option<Metadata>>) -> Result<()> {
        // Iterate over each metadataAction in the metadata map
        for action in metadata {
            // Check if the action is "add"
            if let Some(metadata_action) = action {
                match metadata_action.action.as_str() {
                    "add" => {
                        self.log_eval_input("process_metadata", "add").await;
                        // Create the JSON value with the action's key and name
                        let json_value = json!({
                            metadata_action.name: {
                                metadata_action.key: metadata_action.value
                            }
                        });

                        // Add data to the engine using the JSON value
                        self.engine.add_data(regorus::Value::from(json_value))?;

                        self.log_eval_input("process_metadata", "added!").await;
                    }

                    "remove" => {
                        self.log_eval_input("process_metadata", "remove").await;

                        // get_data()
                        // patch data
                        // clear_data()
                        // add_data(patch)

                        self.log_eval_input("process_metadata", "removed!").await;
                    }

                    "update" => {
                        self.log_eval_input("process_metadata", "update").await;

                        // get_data()
                        // patch data
                        // clear_data()
                        // add_data(patch)

                        self.log_eval_input("process_metadata", "updated!").await;
                    }

                    _ => {
                        self.log_eval_input("process_metadata", "not handled").await;
                        // Handle other actions or do nothing
                    }
                }
            } else {
                self.log_eval_input("process_metadata", "detected null action")
                    .await;
            }
        }

        Ok(())
    }

    async fn allow_request_string(&mut self, ep: &str, ep_input: &str) -> Result<(bool, String)> {
        debug!(sl!(), "policy check: {ep}");
        self.log_eval_input(ep, ep_input).await;

        let query = format!("data.agent_policy.{ep}");
        self.engine.set_input_json(ep_input)?;

        let results = self.engine.eval_query(query, false)?;

        if results.result.len() != 1 {
            bail!("policy check: unexpected eval_query results {:?}", results);
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

                let metadata_response: MetadataResponse = serde_json::from_str(&json_str)?;

                let obj_str = format!("metadata_response found: {:?}", metadata_response);
                self.log_eval_input("allow_request_string", &obj_str).await;

                if metadata_response.allowed {
                    if let Some(metadata) = metadata_response.metadata {
                        // perform state changes based on metadata
                        self.process_metadata(metadata).await?;
                    }
                }
                metadata_response.allowed
            }

            _ => {
                self.log_eval_input("allow_request_string", "bailing").await;
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

        let prints = match self.engine.take_prints() {
            Ok(p) => p.join(" "),
            Err(e) => format!("Failed to get policy log: {e}"),
        };

        // self.log_eval_input("rego prints: ", &prints).await;

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
        self.allow_failures = match self
            .allow_request_string("AllowRequestsFailingPolicy", "{}")
            .await
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
