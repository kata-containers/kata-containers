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
use crate::yaml;

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_yaml;
use std::fs::read_to_string;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct YamlHeader {
    pub apiVersion: String,
    pub kind: String,
}

#[async_trait]
pub trait K8sObject {
    fn requires_policy(&self) -> bool;

    fn get_metadata_name(&self) -> Result<String>;
    fn get_host_name(&self) -> Result<String>;
    fn get_sandbox_name(&self) -> Result<Option<String>>;
    fn get_namespace(&self) -> Result<String>;
    fn add_policy_annotation(&mut self, encoded_policy: &str);

    async fn get_containers_from_registry(&mut self) -> Result<()>;

    fn get_policy_data(
        &self,
        k8s_object: &dyn yaml::K8sObject,
        infra_policy: &infra::InfraPolicy,
        config_maps: &Vec<config_maps::ConfigMap>,
    ) -> Result<policy::PolicyData>;

    fn get_container_mounts_and_storages(
        &self,
        policy_mounts: &mut Vec<oci::Mount>,
        storages: &mut Vec<policy::SerializedStorage>,
        container: &pod::Container,
        infra_policy: &infra::InfraPolicy,
    ) -> Result<()>;

    fn serialize(&mut self, file_name: &Option<String>) -> Result<()>;
}

pub fn get_input_yaml(yaml_file: &Option<String>) -> Result<String> {
    let yaml_string = if let Some(yaml) = yaml_file {
        read_to_string(&yaml)?
    } else {
        std::io::read_to_string(std::io::stdin())?
    };

    Ok(yaml_string)
}

pub fn get_yaml_header(yaml: &str) -> Result<YamlHeader> {
    return Ok(serde_yaml::from_str(yaml)?);
}
