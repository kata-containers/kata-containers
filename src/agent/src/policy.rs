// Copyright (c) 2023 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{anyhow, Result};
use reqwest::Client;
use tokio::time::{sleep, Duration};

use tokio::io::{AsyncWriteExt};


static EMPTY_JSON_INPUT: &str = "{\"input\":{}}";
static ALLOWED_JSON_OUTPUT: &str = "{\"result\":true}";

static OPA_V1_URI: &str                 = "http://localhost:8181/v1";
static OPA_DATA_PATH: &str              = "/data";
static OPA_POLICIES_PATH: &str          = "/policies";

static COCO_POLICY_NAME: &str           = "/coco_policy";
static COCO_DATA_NAME: &str             = "/coco_data";

// Convenience macro to obtain the scope logger
macro_rules! sl {
    () => {
        slog_scope::logger()
    };
}

#[derive(Debug)]
pub struct AgentPolicy {
    opa_data_uri: String,
    coco_policy_query_prefix: String,
    coco_policy_id_uri: String,

    opa_client: Client,
}

impl AgentPolicy {
    pub fn new() -> Result<Self> {
        Ok(AgentPolicy {
            opa_data_uri:               OPA_V1_URI.to_string() + OPA_DATA_PATH,
            coco_policy_query_prefix:   OPA_V1_URI.to_string() + OPA_DATA_PATH + COCO_POLICY_NAME + "/",
            coco_policy_id_uri:         OPA_V1_URI.to_string() + OPA_POLICIES_PATH + COCO_POLICY_NAME,

            opa_client: Client::builder().http1_only().build()?
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

    async fn log_string(s: &[u8], f: &str) {
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(f)
            .await
            .unwrap();

        let mut written_length = 0;
        while written_length < s.len() {
            written_length += file.write(&s[written_length..]).await.unwrap();
        }

        file.flush().await.unwrap();
    }

    pub async fn set_policy(
        &mut self,
        rules: &str,
        data: &str
    ) -> Result<()> {
        Self::log_string(rules.as_bytes(), "/tmp/rules.txt").await;
        Self::log_string(data.as_bytes(), "/tmp/data.txt").await;

        // Delete the old rules.
        let mut uri = self.coco_policy_id_uri.clone();
        info!(sl!(), "set_policy: deleting rules, uri {}", uri);
        self.opa_client
            .delete(uri)
            .send()
            .await
            .map_err(|e| anyhow!(e))?;

        // Delete the old data.
        uri = self.opa_data_uri.clone() + COCO_DATA_NAME;
        info!(sl!(), "set_policy: deleting data, uri {}", uri);
        self.opa_client
            .delete(uri)
            .send()
            .await
            .map_err(|e| anyhow!(e))?;

        // Put the new data.
        uri = self.opa_data_uri.clone();
        info!(sl!(), "set_policy: data uri {}", uri);
        self.opa_client
            .put(uri)
            .body(data.to_string())
            .send()
            .await
            .map_err(|e| anyhow!(e))?;

        // Put the new rules.
        uri = self.coco_policy_id_uri.clone();
        info!(sl!(), "set_policy: rules uri {}", uri);
        self.opa_client
            .put(uri)
            .body(rules.to_string())
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

        let uri = self.coco_policy_query_prefix.clone() + ep;
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
