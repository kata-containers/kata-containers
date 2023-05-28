// Copyright (c) 2023 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

// Allow K8s YAML field names.
#![allow(non_snake_case)]

use crate::config_maps;
use crate::infra;
use crate::obj_meta;
use crate::pod;
use crate::pod_template;
use crate::policy;
use crate::registry;
use crate::utils;
use crate::yaml;

use async_trait::async_trait;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// See ReplicationController in the Kubernetes API reference.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReplicationController {
    apiVersion: String,
    kind: String,
    pub metadata: obj_meta::ObjectMeta,
    pub spec: ReplicationControllerSpec,
}

/// See ReplicationControllerSpec in the Kubernetes API reference.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReplicationControllerSpec {
    #[serde(skip_serializing_if = "Option::is_none")]
    replicas: Option<i32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    selector: Option<BTreeMap<String, String>>,

    pub template: pod_template::PodTemplate,

    #[serde(skip_serializing_if = "Option::is_none")]
    minReadySeconds: Option<i32>,
}

#[async_trait]
impl yaml::K8sObject for ReplicationController {
    fn get_metadata_name(&self) -> String {
        utils::get_metadata_name(&self.metadata)
    }

    fn get_host_name(&self) -> String {
        // Example: "hostname": "no-exist-tdtd7",
        "^".to_string() + &self.get_metadata_name() + "-[a-z0-9]{5}$"
    }

    fn get_sandbox_name(&self) -> Option<String> {
        None
    }

    fn get_namespace(&self) -> String {
        utils::get_metadata_namespace(&self.metadata)
    }

    fn add_policy_annotation(&mut self, encoded_policy: &str) {
        utils::add_policy_annotation(&mut self.spec.template.metadata, encoded_policy)
    }

    async fn get_registry_containers(&self) -> Result<Vec<registry::Container>> {
        utils::get_registry_containers(&self.spec.template.spec.containers).await
    }

    fn get_policy_data(
        &self,
        k8s_object: &dyn yaml::K8sObject,
        infra_policy: &infra::InfraPolicy,
        config_maps: &Vec<config_maps::ConfigMap>,
        registry_containers: &Vec<registry::Container>,
    ) -> Result<policy::PolicyData> {
        policy::get_policy_data(
            k8s_object,
            infra_policy,
            config_maps,
            &self.spec.template.spec.containers,
            registry_containers,
        )
    }

    // fn remove_container(&self, _i: usize) {}

    fn get_container_mounts_and_storages(
        &self,
        _policy_mounts: &mut Vec<oci::Mount>,
        _storages: &mut Vec<policy::SerializedStorage>,
        _container: &pod::Container,
        _infra_policy: &infra::InfraPolicy,
    ) -> Result<()> {
        Ok(())
    }

    fn serialize(&mut self, file_name: &Option<String>) -> Result<()> {
        self.spec.template.spec.containers.remove(0);

        if let Some(yaml) = file_name {
            serde_yaml::to_writer(
                std::fs::OpenOptions::new()
                    .write(true)
                    .truncate(true)
                    .create(true)
                    .open(yaml)
                    .map_err(|e| anyhow!(e))?,
                &self,
            )?;
        } else {
            serde_yaml::to_writer(std::io::stdout(), &self)?;
        }

        Ok(())
    }
}
