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

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// See ReplicationController in the Kubernetes API reference.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReplicationController {
    pub apiVersion: String,
    pub kind: String,
    pub metadata: obj_meta::ObjectMeta,
    pub spec: ReplicationControllerSpec,

    #[serde(skip)]
    doc_mapping: serde_yaml::Value,

    #[serde(skip)]
    pub registry_containers: Vec<registry::Container>,

    #[serde(skip)]
    encoded_policy: String,
}

/// See ReplicationControllerSpec in the Kubernetes API reference.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReplicationControllerSpec {
    #[serde(skip_serializing_if = "Option::is_none")]
    replicas: Option<i32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    selector: Option<BTreeMap<String, String>>,

    pub template: pod_template::PodTemplateSpec,

    #[serde(skip_serializing_if = "Option::is_none")]
    minReadySeconds: Option<i32>,
}

#[async_trait]
impl yaml::K8sResource for ReplicationController {
    async fn init(
        &mut self,
        use_cache: bool,
        doc_mapping: &serde_yaml::Value,
        _silent_unsupported_fields: bool,
    ) -> anyhow::Result<()> {
        yaml::k8s_resource_init(
            &mut self.spec.template.spec,
            &mut self.registry_containers,
            use_cache,
        )
        .await?;
        self.doc_mapping = doc_mapping.clone();
        Ok(())
    }

    fn get_metadata_name(&self) -> anyhow::Result<String> {
        self.metadata.get_name()
    }

    fn get_host_name(&self) -> anyhow::Result<String> {
        // Example: "hostname": "no-exist-tdtd7",
        Ok("^".to_string() + &self.get_metadata_name()? + "-[a-z0-9]{5}$")
    }

    fn get_sandbox_name(&self) -> anyhow::Result<Option<String>> {
        Ok(None)
    }

    fn get_namespace(&self) -> anyhow::Result<String> {
        self.metadata.get_namespace()
    }

    fn get_container_mounts_and_storages(
        &self,
        policy_mounts: &mut Vec<oci::Mount>,
        storages: &mut Vec<policy::SerializedStorage>,
        container: &pod::Container,
        infra_policy: &infra::InfraPolicy,
    ) -> anyhow::Result<()> {
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
        config: &utils::Config,
    ) -> anyhow::Result<()> {
        self.encoded_policy = yaml::generate_policy(
            rules,
            infra_policy,
            config_maps,
            config,
            self,
            &self.registry_containers,
            &self.spec.template.spec.containers,
        )?;
        Ok(())
    }

    fn serialize(&mut self) -> String {
        yaml::add_policy_annotation(
            &mut self.doc_mapping,
            "spec.template.metadata",
            &self.encoded_policy,
        );
        serde_yaml::to_string(&self.doc_mapping).unwrap()
    }
}
