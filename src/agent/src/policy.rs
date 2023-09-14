// Copyright (c) 2023 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{anyhow, bail, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;
use tokio::time::{sleep, Duration};

static EMPTY_JSON_INPUT: &str = "{\"input\":{}}";

static OPA_DATA_PATH: &str = "/data";
static OPA_POLICIES_PATH: &str = "/policies";

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
#[derive(Debug)]
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
    opa_client: Client,

    /// "/tmp/policy.txt" log file for policy activity.
    log_file: Option<tokio::fs::File>,
}

impl AgentPolicy {
    /// Create AgentPolicy object.
    pub fn new(opa_uri: &str, coco_policy: &str) -> Result<Self> {
        Ok(AgentPolicy {
            allow_failures: false,
            query_path: opa_uri.to_string() + OPA_DATA_PATH + coco_policy + "/",
            policy_path: opa_uri.to_string() + OPA_POLICIES_PATH + coco_policy,
            opa_client: Client::builder().http1_only().build().unwrap(),
            log_file: None,
        })
    }

    /// Wait for OPA to start and connect to it.
    pub async fn initialize(&mut self) -> Result<()> {
        let log_file = tokio::fs::OpenOptions::new()
            .write(true)
            .truncate(true)
            .create(true)
            .open("/tmp/policy.txt")
            .await
            .unwrap();
        self.log_file = Some(log_file);

        for i in 0..50 {
            if i > 0 {
                sleep(Duration::from_millis(100)).await;
                println!("policy initialize: POST failed, retrying");
            }

            // Check in a loop if requests causing policy errors should
            // actually be allowed. That is an insecure configuration but is
            // useful for allowing insecure pods to start, then connect to
            // them and inspect Guest logs for the root cause of a failure.
            //
            // The loop is necessary to get the opa_client connected to the
            // OPA service. Future requests to OPA are expected to work
            // without retrying, once the OPA Service had a chance to start.
            if let Ok(allow_failures) = self
                .post_query("AllowRequestsFailingPolicy", EMPTY_JSON_INPUT)
                .await
            {
                self.allow_failures = allow_failures;
                return Ok(());
            }
        }
        Err(anyhow!("failed to connect to OPA"))
    }

    /// Post query to OPA for endpoints that don't require OPA input data.
    pub async fn is_allowed_endpoint(&mut self, ep: &str, request: &str) -> bool {
        let post_input = "{\"input\":".to_string() + request + "}";
        self.log_opa_input(ep, &post_input).await;
        self.post_query(ep, &post_input).await.unwrap_or(false)
    }

    /// Replace the Policy in OPA.
    pub async fn set_policy(&mut self, policy: &str) -> Result<()> {
        check_policy_hash(policy)?;

        // Delete the old rules.
        self.opa_client
            .delete(&self.policy_path)
            .send()
            .await
            .map_err(|e| anyhow!(e))?;

        // Put the new rules.
        self.opa_client
            .put(&self.policy_path)
            .body(policy.to_string())
            .send()
            .await
            .map_err(|e| anyhow!(e))?;

        // Check if requests causing policy errors should actually be allowed.
        // That is an insecure configuration but is useful for allowing insecure
        // pods to start, then connect to them and inspect Guest logs for the
        // root cause of a failure.
        self.allow_failures = self
            .post_query("AllowRequestsFailingPolicy", EMPTY_JSON_INPUT)
            .await?;
        Ok(())
    }

    // Post query to OPA.
    async fn post_query(&mut self, ep: &str, post_input: &str) -> Result<bool> {
        info!(sl!(), "policy check: {}", ep);

        let uri = self.query_path.clone() + ep;
        let response = self
            .opa_client
            .post(uri)
            .body(post_input.to_string())
            .send()
            .await
            .map_err(|e| anyhow!(e))?;

        if response.status() != http::StatusCode::OK {
            return Err(anyhow!(
                "policy: post_query: POST response status {}",
                response.status()
            ));
        }

        let http_response = response.text().await.unwrap();
        let opa_response: serde_json::Result<AllowResponse> = serde_json::from_str(&http_response);

        match opa_response {
            Ok(resp) => {
                if !resp.result {
                    if self.allow_failures {
                        warn!(
                            sl!(),
                            "policy: post_query: response <{}>. Ignoring error!", http_response
                        );
                        return Ok(true);
                    } else {
                        error!(sl!(), "policy: post_query: response <{}>", http_response);
                    }
                }
                Ok(resp.result)
            }
            Err(_) => {
                warn!(
                    sl!(),
                    "policy: post_query: {} not found in policy. Returning false.", ep,
                );
                Ok(false)
            }
        }
    }

    async fn log_opa_input(&mut self, ep: &str, opa_input: &str) {
        // TODO: disable this log by default and allow it to be enabled
        // through Policy.

        if let Some(log_file) = &mut self.log_file {
            match ep {
                "StatsContainerRequest" | "ReadStreamRequest" | "SetPolicyRequest" => {}
                _ => {
                    let log_entry = "# ".to_string() + ep + "\n\n" + opa_input + "\n\n";
                    log_file.write_all(log_entry.as_bytes()).await.unwrap();
                    log_file.flush().await.unwrap();
                }
            }
        }
    }
}

pub fn check_policy_hash(policy: &str) -> Result<()> {
    let mut hasher = Sha256::new();
    hasher.update(policy.as_bytes());
    let digest = hasher.finalize();
    debug!(sl!(), "policy: calculated hash ({:?})", digest.as_slice());

    let mut firmware = sev::firmware::guest::Firmware::open()?;
    let report_data: [u8; 64] = [0; 64];
    let report = firmware.get_report(None, Some(report_data), Some(0))?;

    if report.host_data != digest.as_slice() {
        bail!(
            "Unexpected policy hash ({:?}), expected ({:?})",
            digest.as_slice(),
            report.host_data
        );
    }

    Ok(())
}
