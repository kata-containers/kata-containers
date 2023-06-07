// Copyright (c) 2023 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

// Allow K8s YAML field names.
#![allow(non_snake_case)]

use crate::config_map;
use crate::infra;
use crate::obj_meta;
use crate::pod;
use crate::pod_template;
use crate::policy;
use crate::registry;
use crate::utils;
use crate::yaml;

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Reference / Kubernetes API / Workload Resources / Deployment.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Deployment {
    apiVersion: String,
    kind: String,
    pub metadata: obj_meta::ObjectMeta,
    pub spec: DeploymentSpec,

    #[serde(skip)]
    registry_containers: Vec<registry::Container>,
}

/// Reference / Kubernetes API / Workload Resources / Deployment.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeploymentSpec {
    #[serde(skip_serializing_if = "Option::is_none")]
    replicas: Option<i32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    selector: Option<yaml::LabelSelector>,

    #[serde(skip_serializing_if = "Option::is_none")]
    strategy: Option<DeploymentStrategy>,

    pub template: pod_template::PodTemplateSpec,
    // TODO: additional fields.
}

/// Reference / Kubernetes API / Workload Resources / Deployment.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DeploymentStrategy {
    #[serde(skip_serializing_if = "Option::is_none")]
    r#type: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    rollingUpdate: Option<RollingUpdateDeployment>,
}

/// Reference / Kubernetes API / Workload Resources / Deployment.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RollingUpdateDeployment {
    #[serde(skip_serializing_if = "Option::is_none")]
    maxSurge: Option<i32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    maxUnavailable: Option<i32>,
}

#[async_trait]
impl yaml::K8sObject for Deployment {
    async fn initialize(&mut self, use_cached_files: bool) -> Result<()> {
        yaml::init_k8s_object(
            &mut self.spec.template.spec.containers,
            &mut self.registry_containers,
            use_cached_files,
        )
        .await
    }

    fn requires_policy(&self) -> bool {
        true
    }

    fn get_metadata_name(&self) -> Result<String> {
        self.metadata.get_name()
    }

    fn get_host_name(&self) -> Result<String> {
        // Deployment pod names have variable lengths for some reason.
        Ok("^".to_string() + &self.get_metadata_name()? + "-[a-z0-9]*-[a-z0-9]{5}$")
    }

    fn get_sandbox_name(&self) -> Result<Option<String>> {
        Ok(None)
    }

    fn get_namespace(&self) -> Result<String> {
        self.metadata.get_namespace()
    }

    fn get_container_mounts_and_storages(
        &self,
        policy_mounts: &mut Vec<oci::Mount>,
        storages: &mut Vec<policy::SerializedStorage>,
        container: &pod::Container,
        infra_policy: &infra::InfraPolicy,
    ) -> Result<()> {
        if let Some(volumes) = &self.spec.template.spec.volumes {
            yaml::get_container_mounts_and_storages(
                policy_mounts,
                storages,
                container,
                infra_policy,
                volumes,
            )
        } else {
            Ok(())
        }
    }

    fn generate_policy(
        &mut self,
        rules: &str,
        infra_policy: &infra::InfraPolicy,
        config_maps: &Vec<config_map::ConfigMap>,
        in_out_files: &utils::InOutFiles,
    ) -> Result<()> {
        let encoded_policy = yaml::generate_policy(
            rules,
            infra_policy,
            config_maps,
            in_out_files,
            self,
            &self.registry_containers,
            &self.spec.template.spec.containers,
        )?;

        self.spec
            .template
            .metadata
            .add_policy_annotation(&encoded_policy);

        // Remove the pause container before serializing.
        self.spec.template.spec.containers.remove(0);
        Ok(())
    }

    fn serialize(&mut self) -> Result<String> {
        Ok(serde_yaml::to_string(&self)?)
    }
}
