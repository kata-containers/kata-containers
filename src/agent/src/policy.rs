// Copyright (c) 2023 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;
use tokio::time::{sleep, Duration};

static EMPTY_JSON_INPUT: &str = "{\"input\":{}}";

static OPA_DATA_PATH: &str = "/data";
static OPA_POLICIES_PATH: &str = "/policies";

static POLICY_LOG_FILE: &str = "/tmp/policy.txt";

/// Convenience macro to obtain the scope logger
macro_rules! sl {
    () => {
        slog_scope::logger()
    };
}

/// Example of HTTP response from OPA: {"result":true}
#[derive(Debug, Serialize, Deserialize)]
struct AllowResponse {
    result: bool,
}

/// Singleton policy object.
#[derive(Debug, Default)]
pub struct AgentPolicy {
    /// When true policy errors are ignored, for debug purposes.
    allow_failures: bool,

    /// OPA path used to query if an Agent gRPC request should be allowed.
    /// The request name (e.g., CreateContainerRequest) must be added to
    /// this path.
    query_path: String,

    /// OPA path used to add or delete a rego format Policy.
    policy_path: String,

    /// Client used to connect a single time to the OPA service and reused
    /// for all the future communication with OPA.
    opa_client: Option<reqwest::Client>,

    /// "/tmp/policy.txt" log file for policy activity.
    log_file: Option<tokio::fs::File>,
}

impl AgentPolicy {
    /// Create AgentPolicy object.
    pub fn new() -> Self {
        Self {
            allow_failures: false,
            ..Default::default()
        }
    }

    /// Wait for OPA to start and connect to it.
    pub async fn initialize(
        &mut self,
        debug_enabled: bool,
        opa_uri: &str,
        policy_name: &str,
        default_policy: &str,
    ) -> Result<()> {
        if debug_enabled {
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

        self.query_path = format!("{opa_uri}{OPA_DATA_PATH}{policy_name}/");
        self.policy_path = format!("{opa_uri}{OPA_POLICIES_PATH}{policy_name}");
        let opa_client = reqwest::Client::builder().http1_only().build()?;
        let policy = tokio::fs::read_to_string(default_policy).await?;

        // This loop is necessary to get the opa_client connected to the
        // OPA service while that service is starting. Future requests to
        // OPA are expected to work without retrying, after connecting
        // successfully for the first time.
        for i in 0..50 {
            if i > 0 {
                sleep(Duration::from_millis(100)).await;
                debug!(sl!(), "policy initialize: PUT failed, retrying");
            }

            // Set-up the default policy.
            if opa_client
                .put(&self.policy_path)
                .body(policy.clone())
                .send()
                .await
                .is_ok()
            {
                // Check if requests causing policy errors should actually
                // be allowed. That is an insecure configuration but is
                // useful for allowing insecure pods to start, then connect to
                // them and inspect Guest logs for the root cause of a failure.
                if let Ok(allow_failures) = self
                    .post_query("AllowRequestsFailingPolicy", EMPTY_JSON_INPUT)
                    .await
                {
                    self.allow_failures = allow_failures;
                } else {
                    // post_query failed so the the default, secure, value
                    // allow_failures: false will be used.
                }

                self.opa_client = Some(opa_client);
                return Ok(());
            }
        }
        bail!("Failed to connect to OPA")
    }

    /// Ask OPA to check if an API call should be allowed or not.
    pub async fn is_allowed_endpoint(&mut self, ep: &str, request: &str) -> bool {
        let post_input = format!("{{\"input\":{request}}}");
        self.log_opa_input(ep, &post_input).await;
        match self.post_query(ep, &post_input).await {
            Err(e) => {
                debug!(
                    sl!(),
                    "policy: failed to query endpoint {}: {:?}. Returning false.", ep, e
                );
                false
            }
            Ok(allowed) => allowed,
        }
    }

    /// Replace the Policy in OPA.
    pub async fn set_policy(&mut self, policy: &str) -> Result<()> {
        if let Some(opa_client) = &mut self.opa_client {
            // Delete the old rules.
            opa_client.delete(&self.policy_path).send().await?;

            // Put the new rules.
            opa_client
                .put(&self.policy_path)
                .body(policy.to_string())
                .send()
                .await?;

            // Check if requests causing policy errors should actually be allowed.
            // That is an insecure configuration but is useful for allowing insecure
            // pods to start, then connect to them and inspect Guest logs for the
            // root cause of a failure.
            self.allow_failures = self
                .post_query("AllowRequestsFailingPolicy", EMPTY_JSON_INPUT)
                .await?;

            Ok(())
        } else {
            bail!("Agent Policy is not initialized")
        }
    }

    // Post query to OPA.
    async fn post_query(&mut self, ep: &str, post_input: &str) -> Result<bool> {
        debug!(sl!(), "policy check: {ep}");

        if let Some(opa_client) = &mut self.opa_client {
            let uri = format!("{}{ep}", &self.query_path);
            let response = opa_client
                .post(uri)
                .body(post_input.to_string())
                .send()
                .await?;

            if response.status() != http::StatusCode::OK {
                bail!("policy: POST {} response status {}", ep, response.status());
            }

            let http_response = response.text().await?;
            let opa_response: serde_json::Result<AllowResponse> =
                serde_json::from_str(&http_response);

            match opa_response {
                Ok(resp) => {
                    if !resp.result {
                        if self.allow_failures {
                            warn!(
                                sl!(),
                                "policy: POST {} response <{}>. Ignoring error!", ep, http_response
                            );
                            return Ok(true);
                        } else {
                            error!(sl!(), "policy: POST {} response <{}>", ep, http_response);
                        }
                    }
                    Ok(resp.result)
                }
                Err(_) => {
                    warn!(
                        sl!(),
                        "policy: endpoint {} not found in policy. Returning false.", ep,
                    );
                    Ok(false)
                }
            }
        } else {
            bail!("Agent Policy is not initialized")
        }
    }

    async fn log_opa_input(&mut self, ep: &str, input: &str) {
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
                        warn!(sl!(), "policy: log_opa_input: write_all failed: {}", e);
                    } else if let Err(e) = log_file.flush().await {
                        warn!(sl!(), "policy: log_opa_input: flush failed: {}", e);
                    }
                }
            }
        }
    }
}
