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
use crate::yaml;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// See Deployment in the Kubernetes API reference.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Deployment {
    apiVersion: String,
    kind: String,
    pub metadata: obj_meta::ObjectMeta,
    pub spec: DeploymentSpec,
}

/// See DeploymentSpec in the Kubernetes API reference.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeploymentSpec {
    #[serde(skip_serializing_if = "Option::is_none")]
    replicas: Option<u32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    selector: Option<LabelSelector>,

    #[serde(skip_serializing_if = "Option::is_none")]
    strategy: Option<DeploymentStrategy>,

    pub template: pod_template::PodTemplate,
    // TODO: additional fields.
}

/// See DeploymentStrategy in the Kubernetes API reference.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DeploymentStrategy {
    #[serde(skip_serializing_if = "Option::is_none")]
    r#type: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    rollingUpdate: Option<RollingUpdateDeployment>,
}

/// See RollingUpdateDeployment in the Kubernetes API reference.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RollingUpdateDeployment {
    #[serde(skip_serializing_if = "Option::is_none")]
    maxSurge: Option<i32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    maxUnavailable: Option<i32>,
}

/// See LabelSelector in the Kubernetes API reference.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LabelSelector {
    matchLabels: Option<BTreeMap<String, String>>,
}

#[async_trait]
impl yaml::K8sObject for Deployment {
    fn get_metadata_name(&self) -> String {
        self.metadata.get_name()
    }

    fn get_host_name(&self) -> String {
        // Example: "hostname": "^busybox-cc-5bdd867667-xxmdz$",
        "^".to_string() + &self.get_metadata_name() + "-[a-z0-9]{10}-[a-z0-9]{5}$"
    }

    fn get_sandbox_name(&self) -> Option<String> {
        None
    }

    fn get_namespace(&self) -> String {
        self.metadata.get_namespace()
    }

    fn add_policy_annotation(&mut self, encoded_policy: &str) {
        self.spec.template.metadata.add_policy_annotation(encoded_policy)
    }

    async fn get_registry_containers(&self) -> Result<Vec<registry::Container>> {
        registry::get_registry_containers(&self.spec.template.spec.containers).await
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
