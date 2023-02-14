// Copyright (c) 2023 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{anyhow, Result};
use reqwest;
use tokio::time::{sleep, Duration};

static EMPTY_JSON_INPUT: &str = "{\"input\":{}}";
static ALLOWED_JSON_OUTPUT: &str = "{\"result\":true}";

// Convenience macro to obtain the scope logger
macro_rules! sl {
    () => {
        slog_scope::logger()
    };
}

#[derive(Debug)]
pub struct AgentPolicy {
    opa_uri: String,
    opa_client: reqwest::Client,
}

impl AgentPolicy {
    pub fn new() -> Result<Self> {
        Ok(AgentPolicy {
            opa_uri: "http://localhost:8181/v1/data/coco_policy/".to_string(),
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
            if self.post_to_opa("GuestDetailsRequest", EMPTY_JSON_INPUT).await {
                return Ok(())
            }
        }
        Err(anyhow!("failed to connect to OPA"))
    }

    // Post query to OPA.
    async fn post_to_opa(
        &mut self,
        ep: &str,
        post_input: &str
    ) -> bool {
        let mut allow = false;

        let uri = self.opa_uri.clone() + ep;
        info!(sl!(), "post_to_opa: uri {}, input <{}>", uri, post_input);
        let r = self.opa_client.post(uri).body(post_input.to_owned()).send().await;

        match r {
            Ok(response) => {
                if response.status() != http::StatusCode::OK {
                    error!(sl!(), "post_to_opa: POST response status {}", response.status());
                } else {
                    let result_json = response.text().await.unwrap().trim().to_string();
                    allow = result_json.eq(ALLOWED_JSON_OUTPUT);

                    if !allow {
                        error!(sl!(), "post_to_opa: response <{}>", result_json);
                    }
                }
            }
            Err(e) => {
                error!(sl!(), "post_to_opa: POST failed, <{}>", e);
            }
        }

        allow
    }

    // Post query without input data to OPA.
    pub async fn is_allowed_endpoint(
        &mut self,
        ep: &str
    ) -> bool {
        self.post_to_opa(ep, EMPTY_JSON_INPUT).await
    }

    // Post query with CreateContainerRequest input data to OPA.
    pub async fn is_allowed_create_container_endpoint(
        &mut self,
        ep: &str,
        req: &protocols::agent::CreateContainerRequest,
        index: usize
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
                "{{\"input\":{{\"index\":{},\"oci\":{}}}}}",
                index,
                spec_str);

            self.post_to_opa(ep, &post_input).await
        } else {
            error!(sl!(), "log_oci_spec: failed to convert oci spec to json string");
            false
        }
    }
}