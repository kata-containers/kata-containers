// Copyright (c) 2023 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{anyhow, Result};
use protocols::agent;
use reqwest::Client;
use serde::{Deserialize, Serialize};
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

/// OPA input data for CreateContainerRequest.
#[derive(Debug, Serialize, Deserialize)]
struct CreateContainerRequestInput {
    input: CreateContainerRequestData,
}

/// OPA input data for CreateContainerRequest.
#[derive(Debug, Serialize, Deserialize)]
struct CreateContainerRequestData {
    oci: oci::Spec,
    storages: Vec<agent::Storage>,
}

/// OPA input data for CreateSandboxRequest.
#[derive(Debug, Serialize, Deserialize)]
struct CreateSandboxRequestInput {
    input: CreateSandboxRequestData,
}

/// OPA input data for CreateSandboxRequest.
#[derive(Debug, Serialize, Deserialize)]
struct CreateSandboxRequestData {
    storages: Vec<agent::Storage>,
}

/// OPA input data for ExecProcessRequest.
#[derive(Debug, Serialize)]
struct ExecProcessRequestInput {
    input: ExecProcessRequestData,
}

/// OPA input data for ExecProcessRequest.
#[derive(Debug, Serialize, Deserialize)]
struct ExecProcessRequestData {
    // container_id: String,
    // exec_id: String,
    // user: oci::User,
    process: oci::Process,
}

/// OPA input data for PullImageRequest.
#[derive(Debug, Serialize, Deserialize)]
struct PullImageRequestInput {
    input: PullImageRequestData,
}

/// OPA input data for PullImageRequest.
#[derive(Debug, Serialize, Deserialize)]
struct PullImageRequestData {
    image: String,
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
}

impl AgentPolicy {
    /// Create AgentPolicy object.
    pub fn new(opa_uri: &str, coco_policy: &str) -> Result<Self> {
        Ok(AgentPolicy {
            allow_failures: false,
            query_path: opa_uri.to_string() + OPA_DATA_PATH + coco_policy + "/",
            policy_path: opa_uri.to_string() + OPA_POLICIES_PATH + coco_policy,
            opa_client: Client::builder().http1_only().build()?,
        })
    }

    /// Wait for OPA to start and connect to it.
    pub async fn initialize(&mut self) -> Result<()> {
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
        self.post_query(ep, &post_input).await.unwrap_or(false)
    }

    /// Check if the current Policy allows a CreateContainerRequest, based on
    /// request's inputs.
    pub async fn is_allowed_create_container(
        &mut self,
        ep: &str,
        req: &agent::CreateContainerRequest,
    ) -> bool {
        let opa_input = CreateContainerRequestInput {
            input: CreateContainerRequestData {
                oci: rustjail::grpc_to_oci(&req.OCI),
                storages: req.storages.clone(),
            },
        };
        let post_input = serde_json::to_string(&opa_input).unwrap();
        Self::log_create_container_input(&post_input).await;
        self.post_query(ep, &post_input).await.unwrap_or(false)
    }

    /// Check if the current Policy allows an ExecProcessRequest, based on
    /// request's inputs.
    pub async fn is_allowed_exec_process(
        &mut self,
        ep: &str,
        req: &agent::ExecProcessRequest,
    ) -> bool {
        let opa_input = ExecProcessRequestInput {
            input: ExecProcessRequestData {
                // TODO: should other fields of grpc_process be validated as well?
                process: rustjail::process_grpc_to_oci(&req.process),
            },
        };
        let post_input = serde_json::to_string(&opa_input).unwrap();
        self.post_query(ep, &post_input).await.unwrap_or(false)
    }

    /// Replace the Policy in OPA.
    pub async fn set_policy(&mut self, policy: &str) -> Result<()> {
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

    async fn log_create_container_input(ci: &str) {
        // TODO: disable this log by default and allow it to be enabled
        // through Policy.
        let log_entry = ci.to_string() + "\n\n";

        let mut f = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("/tmp/oci.json")
            .await
            .unwrap();
        f.write_all(log_entry.as_bytes()).await.unwrap();
        f.flush().await.unwrap();
    }
}
