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

#[derive(Clone, Debug)]
pub struct NoPolicyObject {
    pub yaml: String,
}

#[async_trait]
impl yaml::K8sObject for NoPolicyObject {
    async fn initialize(&mut self, _use_cached_files: bool) -> Result<()> {
        Ok(())
    }

    fn requires_policy(&self) -> bool {
        false
    }

    fn get_metadata_name(&self) -> Result<String> {
        Err(anyhow!("Unsupported"))?
    }

    fn get_host_name(&self) -> Result<String> {
        Err(anyhow!("Unsupported"))?
    }

    fn get_sandbox_name(&self) -> Result<Option<String>> {
        Err(anyhow!("Unsupported"))?
    }

    fn get_namespace(&self) -> Result<String> {
        Err(anyhow!("Unsupported"))?
    }

    fn get_container_mounts_and_storages(
        &self,
        _policy_mounts: &mut Vec<oci::Mount>,
        _storages: &mut Vec<policy::SerializedStorage>,
        _container: &pod::Container,
        _infra_policy: &infra::InfraPolicy,
    ) -> Result<()> {
        Err(anyhow!("Unsupported"))?
    }

    fn generate_policy(
        &mut self,
        _rules: &str,
        _infra_policy: &infra::InfraPolicy,
        _config_maps: &Vec<config_map::ConfigMap>,
        _in_out_files: &utils::InOutFiles,
    ) -> Result<()> {
        Err(anyhow!("Unsupported"))
    }

    fn serialize(&mut self) -> Result<String> {
        Ok(self.yaml.clone())
    }
}
