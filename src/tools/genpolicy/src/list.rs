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
use crate::utils;
use crate::yaml;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use core::fmt::Debug;
use serde::{Deserialize, Serialize};
use serde_yaml::{Mapping, Value};
use std::boxed;
use std::marker::{Send, Sync};

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct List {
    apiVersion: String,
    kind: String,

    items: Vec<Mapping>,

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
    async fn init(&mut self, use_cache: bool, _yaml: &str) -> Result<()> {
        for item in &self.items {
            let yaml_string = serde_yaml::to_string(&item)?;
            let (mut resource, _kind) = yaml::new_k8s_resource(&yaml_string)?;
            resource.init(use_cache, &yaml_string).await?;
            self.resources.push(resource);
        }

        Ok(())
    }
    async fn init2(&mut self, use_cache: bool, doc_mapping: &serde_yaml::Value) -> Result<()> {
        Err(anyhow!("Unsupported"))
    }

    fn requires_policy(&self) -> bool {
        for resource in &self.resources {
            if resource.requires_policy() {
                return true;
            }
        }

        false
    }

    fn get_metadata_name(&self) -> Result<String> {
        Err(anyhow!("Unsupported"))
    }

    fn get_host_name(&self) -> Result<String> {
        Err(anyhow!("Unsupported"))
    }

    fn get_sandbox_name(&self) -> Result<Option<String>> {
        Err(anyhow!("Unsupported"))
    }

    fn get_namespace(&self) -> Result<String> {
        Err(anyhow!("Unsupported"))
    }

    fn get_container_mounts_and_storages(
        &self,
        _policy_mounts: &mut Vec<oci::Mount>,
        _storages: &mut Vec<policy::SerializedStorage>,
        _container: &pod::Container,
        _infra_policy: &infra::InfraPolicy,
    ) -> Result<()> {
        Ok(())
    }

    fn generate_policy(
        &mut self,
        rules: &str,
        infra_policy: &infra::InfraPolicy,
        config_maps: &Vec<config_map::ConfigMap>,
        in_out_files: &utils::InOutFiles,
    ) -> Result<()> {
        for resource in &mut self.resources {
            if resource.requires_policy() {
                resource.generate_policy(rules, infra_policy, config_maps, in_out_files)?;
            }
        }

        Ok(())
    }

    fn serialize(&mut self) -> Result<String> {
        self.items.clear();
        for resource in &mut self.resources {
            let yaml = resource.serialize()?;
            let document = serde_yaml::Deserializer::from_str(&yaml);
            let doc_value = Value::deserialize(document)?;
            if let Some(doc_mapping) = doc_value.as_mapping() {
                self.items.push(doc_mapping.clone());
            }
        }

        Ok(serde_yaml::to_string(&self)?)
    }
}
