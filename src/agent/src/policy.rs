// Copyright (c) 2022 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::Result;
use reqwest;
use tokio::time::{sleep, Duration};
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
}

impl AgentPolicy {
    pub fn from_opa_uri(uri: &str) -> Result<AgentPolicy> {
        Ok(AgentPolicy {
            opa_uri: uri.to_string(),
        })
    }

    #[instrument]
    pub async fn initialize(&self) -> Result<()> {
        let request_uri = self.opa_uri.to_string() + "GuestDetailsRequest";
        let post_input = "{{ \"input\": {{}} }}".to_string();
        let client = reqwest::Client::new();

        for i in 0..50 {
            if i > 0 {
                sleep(Duration::from_millis(100)).await;
                println!("policy initialize: POST failed, retrying");
            }

            if let Ok(_) = client
                .post(request_uri.to_owned())
                .body(post_input.to_owned())
                .send()
                .await {

                break;
            }
        }
        Ok(())
    }

    #[instrument]
    async fn post_to_opa(
        &mut self,
        ep: &str,
        post_input: &str
    ) -> bool {
        let mut allow = false;
        let client = reqwest::Client::new();
        let uri = self.opa_uri.to_string() + ep;
        let input_with_key = format!("{{\"input\":{}}}", post_input);

        info!(sl!(), "post_to_opa: uri {}, input <{}>", uri, input_with_key);
        let response = client.post(uri)
            .body(input_with_key)
            .send()
            .await
            .unwrap();
        assert_eq!(allow, false);

        let status = response.status();
        if status != http::StatusCode::OK {
            assert_eq!(allow, false);
            error!(sl!(), "post_to_opa: POST response status {}", status);
        } else {
            let result_json = response.text().await.unwrap().trim().to_string();
            allow = result_json.eq("{\"result\":true}");

            if !allow {
                error!(sl!(), "post_to_opa: response <{}>", result_json);
            }
        }

        allow
    }

    #[instrument]
    pub async fn is_allowed_endpoint(
        &mut self,
        ep: &str
    ) -> bool {
        self.post_to_opa(ep, "{}").await
    }

    #[instrument]
    pub async fn is_allowed_create_container_endpoint(
        &mut self,
        ep: &str,
        req: &protocols::agent::CreateContainerRequest,
        index: usize
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
            let index_and_oci = format!(
                "{{ \"index\":{}, \"oci\":{} }}",
                index,
                spec_str);

            self.post_to_opa(ep, &index_and_oci).await
        } else {
            error!(sl!(), "log_oci_spec: failed convert oci spec to json string");
            false
        }
    }
}