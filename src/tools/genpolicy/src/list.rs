// Copyright (c) 2023 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

// Allow K8s YAML field names.
#![allow(non_snake_case)]

use crate::config_maps;
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
    k8s_objects: Vec<boxed::Box<dyn yaml::K8sObject + Sync + Send>>,
}

impl Debug for dyn yaml::K8sObject + Send + Sync {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "K8sObject")
    }
}

#[async_trait]
impl yaml::K8sObject for List {
    async fn initialize(&mut self, use_cached_files: bool) -> Result<()> {
        for item in &self.items {
            let yaml_string = serde_yaml::to_string(&item)?;
            let header = yaml::get_yaml_header(&yaml_string)?;
            let mut k8s_object = yaml::new_k8s_object(&header.kind, &yaml_string)?;
            k8s_object.initialize(use_cached_files).await?;
            self.k8s_objects.push(k8s_object);
        }

        Ok(())
    }

    fn requires_policy(&self) -> bool {
        for k8s_object in &self.k8s_objects {
            if k8s_object.requires_policy() {
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
        config_maps: &Vec<config_maps::ConfigMap>,
        in_out_files: &utils::InOutFiles,
    ) -> Result<()> {
        for k8s_object in &mut self.k8s_objects {
            if k8s_object.requires_policy() {
                k8s_object.generate_policy(rules, infra_policy, config_maps, in_out_files)?;
            }
        }

        Ok(())
    }

    fn serialize(&mut self) -> Result<String> {
        self.items.clear();
        for k8s_object in &mut self.k8s_objects {
            let yaml = k8s_object.serialize()?;
            let document = serde_yaml::Deserializer::from_str(&yaml);
            let doc_value = Value::deserialize(document)?;
            if let Some(doc_mapping) = doc_value.as_mapping() {
                self.items.push(doc_mapping.clone());
            }
        }

        Ok(serde_yaml::to_string(&self)?)
    }
}
