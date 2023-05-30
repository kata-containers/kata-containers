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
    async fn initialize(&mut self) -> Result<()>;

    fn requires_policy(&self) -> bool;

    fn generate_policy(
        &mut self,
        rules: &str,
        infra_policy: &infra::InfraPolicy,
        config_maps: &Vec<config_maps::ConfigMap>,
        in_out_files: &utils::InOutFiles,
    ) -> Result<()>;

    fn serialize(&self) -> Result<String>;

    fn get_metadata_name(&self) -> Result<String>;
    fn get_host_name(&self) -> Result<String>;
    fn get_sandbox_name(&self) -> Result<Option<String>>;
    fn get_namespace(&self) -> Result<String>;

    fn get_container_mounts_and_storages(
        &self,
        policy_mounts: &mut Vec<oci::Mount>,
        storages: &mut Vec<policy::SerializedStorage>,
        container: &pod::Container,
        infra_policy: &infra::InfraPolicy,
    ) -> Result<()>;
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
