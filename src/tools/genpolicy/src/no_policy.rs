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

use async_trait::async_trait;

#[derive(Clone, Debug)]
pub struct NoPolicyResource {
    pub yaml: String,
}

#[async_trait]
impl yaml::K8sResource for NoPolicyResource {
    async fn init(
        &mut self,
        _use_cache: bool,
        _doc_mapping: &serde_yaml::Value,
        _silent_unsupported_fields: bool,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    fn get_metadata_name(&self) -> anyhow::Result<String> {
        panic!("Unsupported");
    }

    fn get_host_name(&self) -> anyhow::Result<String> {
        panic!("Unsupported");
    }

    fn get_sandbox_name(&self) -> anyhow::Result<Option<String>> {
        panic!("Unsupported");
    }

    fn get_namespace(&self) -> anyhow::Result<String> {
        panic!("Unsupported");
    }

    fn get_container_mounts_and_storages(
        &self,
        _policy_mounts: &mut Vec<oci::Mount>,
        _storages: &mut Vec<policy::SerializedStorage>,
        _container: &pod::Container,
        _infra_policy: &infra::InfraPolicy,
    ) -> anyhow::Result<()> {
        panic!("Unsupported");
    }

    fn generate_policy(
        &mut self,
        _rules: &str,
        _infra_policy: &infra::InfraPolicy,
        _config_maps: &Vec<config_map::ConfigMap>,
        _config: &utils::Config,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    fn serialize(&mut self) -> String {
        self.yaml.clone()
    }
}
