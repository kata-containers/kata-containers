// Copyright (c) 2023 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

// Allow K8s YAML field names.
#![allow(non_snake_case)]

use crate::policy;
use crate::utils::Config;
use crate::yaml;

use async_trait::async_trait;

#[derive(Clone, Debug)]
pub struct NoPolicyResource {
    pub yaml: String,
}

#[async_trait]
impl yaml::K8sResource for NoPolicyResource {
    async fn init(
        &mut self,
        _config: &Config,
        _doc_mapping: &serde_yaml::Value,
        _silent_unsupported_fields: bool,
    ) {
    }

    fn generate_policy(&self, _agent_policy: &policy::AgentPolicy) -> String {
        "".to_string()
    }

    fn serialize(&mut self, _policy: &str) -> String {
        self.yaml.clone()
    }
}
