// Copyright (c) 2022 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::Result;
use reqwest;
use tracing::instrument;

// Convenience macro to obtain the scope logger
macro_rules! sl {
    () => {
        slog_scope::logger()
    };
}

#[derive(Debug)]
pub struct AgentPolicy {
    opa_uri: String,
    max_loop_count: u32,
}

impl AgentPolicy {
    #[instrument]
    pub fn from_opa_uri(uri: &str) -> Result<AgentPolicy> {
        Ok(AgentPolicy {
            opa_uri: uri.to_string(),
            max_loop_count: 100,
        })
    }

    #[instrument]
    async fn post_to_opa(
        &mut self,
        ep: &str,
        post_input: &str
    ) -> String {
        let client = reqwest::Client::new();

        for _ in 0..self.max_loop_count {
            let uri = self.opa_uri.to_owned() + ep;
            let input_with_key = format!("{{\"input\":{}}}", post_input);

            info!(sl!(), "post_to_opa: uri {}, input <{}>", uri, input_with_key);
            let result = client
                .post(uri)
                .body(input_with_key)
                .send()
                .await;

            match result {
                Err(e) => {
                    error!(sl!(), "post_to_opa: POST error {}", e);
                }
                Ok(response) => {
                    let status = response.status();
                    if status != http::StatusCode::OK {
                        error!(sl!(), "post_to_opa: POST response status {}", status);
                    } else {
                        // OPA is up an running, so don't retry in the future.
                        self.max_loop_count = 1;

                        let response = response.text().await.unwrap();
                        // info!(sl!(), "post_to_opa: response <{}>", response);
                        return response;
                    }
                }
            }
        }

        error!(sl!(), "post_to_opa: returning empty string!");
        return String::new();
    }

    #[instrument]
    pub async fn is_allowed_endpoint(
        &mut self,
        ep: &str
    ) -> bool {
        self.post_to_opa(ep, "{}").await.eq("{\"result\":true}")
    }

    pub async fn is_allowed_create_container_endpoint(
        &mut self,
        ep: &str,
        req: &protocols::agent::CreateContainerRequest
    ) -> bool {
        let mut oci_spec = req.OCI.clone();

        let spec = match oci_spec.as_mut() {
            Some(s) => rustjail::grpc_to_oci(s),
            None => {
                error!(sl!(), "no oci spec in the create container request!");
                return false;
            }
        };

        if let Ok(spec_str) = serde_json::to_string(&spec) {
            self.post_to_opa(ep, &spec_str).await.eq("{\"result\":true}")
        } else {
            error!(sl!(), "log_oci_spec: failed convert oci spec to json string");
            false
        }
    }
}