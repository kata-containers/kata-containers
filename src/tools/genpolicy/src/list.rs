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
use core::fmt::Debug;
use protocols::agent;
use serde::{Deserialize, Serialize};
use serde_yaml::Value;
use std::boxed;
use std::marker::{Send, Sync};

#[derive(Debug, Serialize, Deserialize)]
pub struct List {
    apiVersion: String,
    kind: String,

    items: Vec<serde_yaml::Value>,

    #[serde(skip)]
    resources: Vec<boxed::Box<dyn yaml::K8sResource + Sync + Send>>,
}

impl Debug for dyn yaml::K8sResource + Send + Sync {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "K8sResource")
    }
}

#[async_trait]
impl yaml::K8sResource for List {
    async fn init(&mut self, config: &Config, _doc_mapping: &serde_yaml::Value, silent: bool) {
        // Create K8sResource objects for each item in this List.
        for item in &self.items {
            let yaml_string = serde_yaml::to_string(&item).unwrap();
            let (mut resource, _kind) = yaml::new_k8s_resource(&yaml_string, silent).unwrap();
            resource.init(config, item, silent).await;
            self.resources.push(resource);
        }
    }

    fn get_container_mounts_and_storages(
        &self,
        _policy_mounts: &mut Vec<policy::KataMount>,
        _storages: &mut Vec<agent::Storage>,
        _container: &pod::Container,
        _settings: &settings::Settings,
    ) {
    }

    fn generate_policy(&self, agent_policy: &policy::AgentPolicy) -> String {
        let mut policies: Vec<String> = Vec::new();
        for resource in &self.resources {
            policies.push(resource.generate_policy(agent_policy));
        }
        policies.join(":")
    }

    fn serialize(&mut self, policy: &str) -> String {
        let policies: Vec<&str> = policy.split(':').collect();
        let len = policies.len();
        assert!(len == self.resources.len());

        self.items.clear();
        for (i, p) in policies.iter().enumerate().take(len) {
            let yaml = self.resources[i].serialize(p);
            let document = serde_yaml::Deserializer::from_str(&yaml);
            let doc_value = Value::deserialize(document).unwrap();
            self.items.push(doc_value.clone());
        }
        serde_yaml::to_string(&self).unwrap()
    }
}
