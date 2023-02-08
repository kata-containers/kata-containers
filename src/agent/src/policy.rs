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
    pub async fn is_allowed_endpoint(&mut self, ep: &str) -> bool {
        let mut allowed = false;

        for _ in 0..self.max_loop_count {
            let result = reqwest::get(self.opa_uri.to_owned() + ep).await;

            match result {
                Err(e) => {
                    error!(sl!(), "is_allowed_endpoint: GET error {}", e);
                }
                Ok(response) => {
                    let status = response.status();
                    if status != http::StatusCode::OK {
                        error!(sl!(), "is_allowed_endpoint: GET status code {}", status);
                    } else {
                        let body = response.text().await.unwrap();
                        allowed = body.eq("{\"result\":true}");

                        // OPA is up an running, so don't retry in the future.
                        self.max_loop_count = 1;
                        break;
                    }
                }
            }
        }

        allowed
    }
}