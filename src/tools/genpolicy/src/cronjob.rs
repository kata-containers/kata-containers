// Copyright (c) 2024 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

// Allow K8s YAML field names.
#![allow(non_snake_case)]

use crate::job;
use crate::obj_meta;
use crate::pod;
use crate::policy;
use crate::settings;
use crate::utils::Config;
use crate::yaml;

use async_trait::async_trait;
use protocols::agent;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// See Reference / Kubernetes API / Workload Resources / CronJob.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CronJob {
    apiVersion: String,
    kind: String,
    metadata: obj_meta::ObjectMeta,
    spec: CronJobSpec,
    #[serde(skip)]
    doc_mapping: serde_yaml::Value,
}

/// See Reference / Kubernetes API / Workload Resources / CronJob.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CronJobSpec {
    jobTemplate: JobTemplateSpec,

    #[serde(skip_serializing_if = "Option::is_none")]
    concurrencyPolicy: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    failedJobsHistoryLimit: Option<i32>,

    schedule: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    startingDeadlineSeconds: Option<i32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    successfulJobsHistoryLimit: Option<i32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    suspend: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    timeZone: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    backoffLimit: Option<i32>,
    // TODO: additional fields.
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JobTemplateSpec {
    #[serde(skip_serializing_if = "Option::is_none")]
    metadata: Option<obj_meta::ObjectMeta>,
    spec: job::JobSpec,
}

#[async_trait]
impl yaml::K8sResource for CronJob {
    async fn init(
        &mut self,
        config: &Config,
        doc_mapping: &serde_yaml::Value,
        _silent_unsupported_fields: bool,
    ) {
        yaml::k8s_resource_init(&mut self.spec.jobTemplate.spec.template.spec, config).await;
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
        yaml::get_container_mounts_and_storages(
            policy_mounts,
            storages,
            container,
            settings,
            &self.spec.jobTemplate.spec.template.spec.volumes,
        );
    }

    fn generate_policy(&self, agent_policy: &policy::AgentPolicy) -> String {
        agent_policy.generate_policy(self)
    }

    fn serialize(&mut self, policy: &str) -> String {
        yaml::add_policy_annotation(
            &mut self.doc_mapping,
            "spec.jobTemplate.spec.template",
            policy,
        );
        serde_yaml::to_string(&self.doc_mapping).unwrap()
    }

    fn get_containers(&self) -> &Vec<pod::Container> {
        &self.spec.jobTemplate.spec.template.spec.containers
    }

    fn get_annotations(&self) -> &Option<BTreeMap<String, String>> {
        if let Some(metadata) = &self.spec.jobTemplate.spec.template.metadata {
            return &metadata.annotations;
        }
        &None
    }

    fn use_host_network(&self) -> bool {
        if let Some(host_network) = self.spec.jobTemplate.spec.template.spec.hostNetwork {
            return host_network;
        }
        false
    }

    fn use_sandbox_pidns(&self) -> bool {
        if let Some(shared) = self
            .spec
            .jobTemplate
            .spec
            .template
            .spec
            .shareProcessNamespace
        {
            return shared;
        }
        false
    }

    fn get_process_fields(&self, process: &mut policy::KataProcess, must_check_passwd: &mut bool) {
        yaml::get_process_fields(
            process,
            &self.spec.jobTemplate.spec.template.spec.securityContext,
            must_check_passwd,
        );
    }

    fn get_sysctls(&self) -> Vec<pod::Sysctl> {
        yaml::get_sysctls(&self.spec.jobTemplate.spec.template.spec.securityContext)
    }
}
