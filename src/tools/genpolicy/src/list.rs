// Copyright (c) 2023 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

// Allow K8s YAML field names.
#![allow(non_snake_case)]

use crate::config_map;
use crate::infra;
use crate::pod;
use crate::policy;
use crate::registry;
use crate::utils;
use crate::yaml;

use async_trait::async_trait;
use core::fmt::Debug;
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
    async fn init(
        &mut self,
        use_cache: bool,
        _doc_mapping: &serde_yaml::Value,
        silent_unsupported_fields: bool,
    ) -> anyhow::Result<()> {
        for item in &self.items {
            let yaml_string = serde_yaml::to_string(&item)?;
            let (mut resource, _kind) =
                yaml::new_k8s_resource(&yaml_string, silent_unsupported_fields)?;
            resource
                .init(use_cache, item, silent_unsupported_fields)
                .await?;
            self.resources.push(resource);
        }

        Ok(())
    }

    fn get_metadata_name(&self) -> String {
        panic!("Unsupported");
    }

    fn get_host_name(&self) -> String {
        panic!("Unsupported");
    }

    fn get_sandbox_name(&self) -> Option<String> {
        panic!("Unsupported");
    }

    fn get_namespace(&self) -> String {
        panic!("Unsupported");
    }

    fn get_container_mounts_and_storages(
        &self,
        _policy_mounts: &mut Vec<oci::Mount>,
        _storages: &mut Vec<policy::SerializedStorage>,
        _container: &pod::Container,
        _infra_policy: &infra::InfraPolicy,
    ) {
    }

    fn generate_policy(
        &mut self,
        rules: &str,
        infra_policy: &infra::InfraPolicy,
        config_maps: &Vec<config_map::ConfigMap>,
        config: &utils::Config,
    ) -> anyhow::Result<()> {
        for resource in &mut self.resources {
            resource.generate_policy(rules, infra_policy, config_maps, config)?;
        }

        Ok(())
    }

    fn serialize(&mut self) -> String {
        self.items.clear();
        for resource in &mut self.resources {
            let yaml = resource.serialize();
            let document = serde_yaml::Deserializer::from_str(&yaml);
            let doc_value = Value::deserialize(document).unwrap();
            self.items.push(doc_value.clone());
        }
        serde_yaml::to_string(&self).unwrap()
    }

    fn get_containers(&self) -> (&Vec<registry::Container>, &Vec<pod::Container>) {
        panic!("Unsupported");
    }
}
