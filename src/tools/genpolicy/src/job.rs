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
use crate::settings;
use crate::yaml;

use async_trait::async_trait;
use protocols::agent;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// See Reference / Kubernetes API / Workload Resources / Job.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Job {
    apiVersion: String,
    kind: String,
    metadata: obj_meta::ObjectMeta,
    spec: JobSpec,

    #[serde(skip)]
    doc_mapping: serde_yaml::Value,
}

/// See Reference / Kubernetes API / Workload Resources / Job.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JobSpec {
    template: pod_template::PodTemplateSpec,

    #[serde(skip_serializing_if = "Option::is_none")]
    backoffLimit: Option<i32>,
    // TODO: additional fields.
}

#[async_trait]
impl yaml::K8sResource for Job {
    async fn init(
        &mut self,
        use_cache: bool,
        doc_mapping: &serde_yaml::Value,
        _silent_unsupported_fields: bool,
    ) {
        yaml::k8s_resource_init(&mut self.spec.template.spec, use_cache).await;
        self.doc_mapping = doc_mapping.clone();
    }

    fn get_sandbox_name(&self) -> Option<String> {
        None
    }

    fn get_namespace(&self) -> Option<String> {
        self.metadata.get_namespace()
    }

    fn get_container_mounts_and_storages(
        &self,
        policy_mounts: &mut Vec<policy::KataMount>,
        storages: &mut Vec<agent::Storage>,
        container: &pod::Container,
        settings: &settings::Settings,
    ) {
        if let Some(volumes) = &self.spec.template.spec.volumes {
            yaml::get_container_mounts_and_storages(
                policy_mounts,
                storages,
                container,
                settings,
                volumes,
            );
        }
    }

    fn generate_policy(&self, agent_policy: &policy::AgentPolicy) -> String {
        agent_policy.generate_policy(self)
    }

    fn serialize(&mut self, policy: &str) -> String {
        yaml::add_policy_annotation(&mut self.doc_mapping, "spec.template", policy);
        serde_yaml::to_string(&self.doc_mapping).unwrap()
    }

    fn get_containers(&self) -> &Vec<pod::Container> {
        &self.spec.template.spec.containers
    }

    fn get_annotations(&self) -> &Option<BTreeMap<String, String>> {
        if let Some(metadata) = &self.spec.template.metadata {
            return &metadata.annotations;
        }
        &None
    }

    fn use_host_network(&self) -> bool {
        if let Some(host_network) = self.spec.template.spec.hostNetwork {
            return host_network;
        }
        false
    }

    fn use_sandbox_pidns(&self) -> bool {
        if let Some(shared) = self.spec.template.spec.shareProcessNamespace {
            return shared;
        }
        false
    }
}
