// Copyright (c) 2023 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{anyhow, Result};
use reqwest;
use tokio::time::{sleep, Duration};

static EMPTY_JSON_INPUT: &str = "{\"input\":{}}";
static ALLOWED_JSON_OUTPUT: &str = "{\"result\":true}";

static OPA_V1_URI: &str                 = "http://localhost:8181/v1/";
static OPA_DATA_PATH: &str              = "data/";
static OPA_POLICIES_PATH: &str          = "policies";
static OPA_COCO_POLICY_QUERY: &str      = "coco_policy/";

// Convenience macro to obtain the scope logger
macro_rules! sl {
    () => {
        slog_scope::logger()
    };
}

#[derive(Debug)]
pub struct AgentPolicy {
    opa_policies_uri: String,
    opa_data_uri: String,
    opa_query_uri: String,
    opa_client: reqwest::Client,
}

impl AgentPolicy {
    pub fn new() -> Result<Self> {
        Ok(AgentPolicy {
            opa_policies_uri: OPA_V1_URI.to_string() + OPA_POLICIES_PATH,
            opa_data_uri: OPA_V1_URI.to_string() + OPA_DATA_PATH,
            opa_query_uri: OPA_V1_URI.to_string() + OPA_DATA_PATH + OPA_COCO_POLICY_QUERY,
            opa_client: reqwest::Client::builder().http1_only().build()?
        })
    }

    // Wait for OPA to start.
    pub async fn initialize(&mut self) -> Result<()> {
        for i in 0..50 {
            if i > 0 {
                sleep(Duration::from_millis(100)).await;
                println!("policy initialize: POST failed, retrying");
            }

            // Post a request for a commonly used Agent request.
            if self.post_query("GuestDetailsRequest", EMPTY_JSON_INPUT).await {
                return Ok(())
            }
        }
        Err(anyhow!("failed to connect to OPA"))
    }

    // Post query without input data to OPA.
    pub async fn is_allowed_endpoint(
        &mut self,
        ep: &str
    ) -> bool {
        self.post_query(ep, EMPTY_JSON_INPUT).await
    }

    // Post query with CreateContainerRequest input data to OPA.
    pub async fn is_allowed_create_container_endpoint(
        &mut self,
        ep: &str,
        req: &protocols::agent::CreateContainerRequest
    ) -> bool {
        // Send container's OCI spec in json format as input data for OPA.
        let mut oci_spec = req.OCI.clone();

        let spec = match oci_spec.as_mut() {
            Some(s) => rustjail::grpc_to_oci(s),
            None => {
                error!(sl!(), "no oci spec in the create container request!");
                return false;
            }
        };

        if let Ok(spec_str) = serde_json::to_string(&spec) {
            let post_input = format!(
                "{{\"input\":{{\"oci\":{}}}}}",
                spec_str);

            self.post_query(ep, &post_input).await
        } else {
            error!(sl!(), "log_oci_spec: failed to convert oci spec to json string");
            false
        }
    }

    pub async fn set_policy(
        &mut self,
        rules: &str,
        data: &str
    ) -> Result<()> {
        let mut uri = self.opa_policies_uri.clone();
        info!(sl!(), "set_policy: rules uri {}, input <{}>", uri, rules);
        self.opa_client
            .post(uri)
            .body(rules.to_string())
            .send()
            .await
            .map_err(|e| anyhow!(e))?;

        uri = self.opa_data_uri.clone();
        info!(sl!(), "set_policy: data uri {}, input <{}>", uri, data);
        self.opa_client
            .post(uri)
            .body(data.to_string())
            .send()
            .await
            .map_err(|e| anyhow!(e))?;

        Ok(())
    }

    // Post query to OPA.
    async fn post_query(
        &mut self,
        ep: &str,
        post_input: &str
    ) -> bool {
        let mut allow = false;

        let uri = self.opa_query_uri.clone() + ep;
        info!(sl!(), "post_query: uri {}, input <{}>", uri, post_input);
        let r = self.opa_client.post(uri).body(post_input.to_owned()).send().await;

        match r {
            Ok(response) => {
                if response.status() != http::StatusCode::OK {
                    error!(sl!(), "post_query: POST response status {}", response.status());
                } else {
                    let result_json = response.text().await.unwrap().trim().to_string();
                    allow = result_json.eq(ALLOWED_JSON_OUTPUT);

                    if !allow {
                        error!(sl!(), "post_query: response <{}>", result_json);
                    }
                }
            }
            Err(e) => {
                error!(sl!(), "post_query: POST failed, <{}>", e);
            }
        }

        allow
    }
}
