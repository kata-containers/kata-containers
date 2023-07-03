// Copyright (c) 2023 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

// Allow K8s YAML field names.
#![allow(non_snake_case)]

use crate::obj_meta;
use crate::pod;
use crate::pod_template;
use crate::policy;
use crate::registry;
use crate::yaml;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// See Reference / Kubernetes API / Workload Resources / ReplicaSet.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReplicaSet {
    pub apiVersion: String,
    pub kind: String,
    pub metadata: obj_meta::ObjectMeta,
    pub spec: ReplicaSetSpec,

    #[serde(skip)]
    doc_mapping: serde_yaml::Value,

    #[serde(skip)]
    pub registry_containers: Vec<registry::Container>,
}

/// See ReplicaSetSpec in the Kubernetes API reference.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReplicaSetSpec {
    selector: yaml::LabelSelector,
    pub template: pod_template::PodTemplateSpec,

    #[serde(skip_serializing_if = "Option::is_none")]
    replicas: Option<i32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    minReadySeconds: Option<i32>,
}

#[async_trait]
impl yaml::K8sResource for ReplicaSet {
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

    fn get_yaml_host_name(&self) -> Option<String> {
        if let Some(hostname) = &self.spec.template.spec.hostname {
            return Some(hostname.clone());
        }
        None
    }

    fn get_host_name(&self) -> String {
        // Example: "hostname": "no-exist-tdtd7",
        "^".to_string() + &self.metadata.get_name() + "-[a-z0-9]{5}$"
    }

    fn get_sandbox_name(&self) -> Option<String> {
        None
    }

    fn get_namespace(&self) -> String {
        self.metadata.get_namespace()
    }

    fn get_container_mounts_and_storages(
        &self,
        policy_mounts: &mut Vec<oci::Mount>,
        storages: &mut Vec<policy::SerializedStorage>,
        container: &pod::Container,
        agent_policy: &policy::AgentPolicy,
    ) {
        if let Some(volumes) = &self.spec.template.spec.volumes {
            yaml::get_container_mounts_and_storages(
                policy_mounts,
                storages,
                container,
                agent_policy,
                volumes,
            );
        }
    }

    fn generate_policy(&self, agent_policy: &policy::AgentPolicy) -> String {
        agent_policy.generate_policy(self)
    }

    fn serialize(&mut self, policy: &str) -> String {
        yaml::add_policy_annotation(&mut self.doc_mapping, "spec.template.metadata", policy);
        serde_yaml::to_string(&self.doc_mapping).unwrap()
    }

    fn get_containers(&self) -> (&Vec<registry::Container>, &Vec<pod::Container>) {
        (
            &self.registry_containers,
            &self.spec.template.spec.containers,
        )
    }

    fn get_annotations(&self) -> Option<BTreeMap<String, String>> {
        if let Some(annotations) = &self.spec.template.metadata.annotations {
            return Some(annotations.clone());
        }
        None
    }
}
