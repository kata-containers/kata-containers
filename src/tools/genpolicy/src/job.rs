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
use crate::utils::Config;
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
    pub template: pod_template::PodTemplateSpec,

    #[serde(skip_serializing_if = "Option::is_none")]
    backoffLimit: Option<i32>,
    // TODO: additional fields.
}

#[async_trait]
impl yaml::K8sResource for Job {
    async fn init(
        &mut self,
        config: &Config,
        doc_mapping: &serde_yaml::Value,
        _silent_unsupported_fields: bool,
    ) {
        yaml::k8s_resource_init(&mut self.spec.template.spec, config).await;
        self.doc_mapping = doc_mapping.clone();
    }

    fn get_sandbox_name(&self) -> Option<String> {
        let job_name = yaml::name_regex_from_meta(&self.metadata);
        job_name.map(pod_name_regex)
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
        yaml::get_container_mounts_and_storages(
            policy_mounts,
            storages,
            container,
            settings,
            &self.spec.template.spec,
        );
    }

    fn generate_initdata_anno(&self, agent_policy: &policy::AgentPolicy) -> String {
        agent_policy.generate_initdata_anno(self)
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

    fn get_process_fields(&self, process: &mut policy::KataProcess, must_check_passwd: &mut bool) {
        yaml::get_process_fields(
            process,
            &self.spec.template.spec.securityContext,
            must_check_passwd,
        );
    }

    fn get_sysctls(&self) -> Vec<pod::Sysctl> {
        yaml::get_sysctls(&self.spec.template.spec.securityContext)
    }
}

pub fn pod_name_regex(job_name: String) -> String {
    // Job name - optional index - generateNameSuffix
    // https://github.com/kubernetes/kubernetes/blob/b35c5c0a301d326fdfa353943fca077778544ac6/pkg/controller/job/job_controller.go#L1767
    // https://github.com/kubernetes/kubernetes/blob/b35c5c0a301d326fdfa353943fca077778544ac6/pkg/controller/job/indexed_job_utils.go#L501
    // Very long job names are handled incorrectly (job_name would need to be truncated, but
    // index/suffix len are unknown here)
    let suffix = yaml::GENERATE_NAME_SUFFIX_REGEX;
    format!("{job_name}(-[0-9]+)?-{suffix}")
}
