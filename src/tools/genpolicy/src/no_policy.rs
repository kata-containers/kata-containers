// Copyright (c) 2023 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

// Allow K8s YAML field names.
#![allow(non_snake_case)]

use crate::pod;
use crate::policy;
use crate::settings;
use crate::utils::Config;
use crate::yaml;

use async_trait::async_trait;
use protocols::agent;
use std::collections::BTreeMap;

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

    fn get_sandbox_name(&self) -> Option<String> {
        panic!("Unsupported");
    }

    fn get_container_mounts_and_storages(
        &self,
        _policy_mounts: &mut Vec<policy::KataMount>,
        _storages: &mut Vec<agent::Storage>,
        _container: &pod::Container,
        _settings: &settings::Settings,
    ) {
        panic!("Unsupported");
    }

    fn generate_policy(&self, _agent_policy: &policy::AgentPolicy) -> String {
        "".to_string()
    }

    fn serialize(&mut self, _policy: &str) -> String {
        self.yaml.clone()
    }

    fn get_containers(&self) -> &Vec<pod::Container> {
        panic!("Unsupported");
    }

    fn get_annotations(&self) -> &Option<BTreeMap<String, String>> {
        panic!("Unsupported");
    }

    fn use_host_network(&self) -> bool {
        panic!("Unsupported");
    }

    fn use_sandbox_pidns(&self) -> bool {
        panic!("Unsupported");
    }
}
