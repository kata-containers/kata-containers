// Copyright (c) 2023 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

// Allow K8s YAML field names.
#![allow(non_snake_case)]

use crate::obj_meta;
use crate::pod;
use crate::pod_template;
use crate::volumes;
use crate::yaml;

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

impl yaml::K8sObject for Deployment {
    fn get_metadata_name(&self) -> String {
        return "".to_string();
    }

    fn get_host_name(&self) -> String {
        return "".to_string();
    }

    fn get_sandbox_name(&self) -> Option<String> {
        None
    }

    fn get_namespace(&self) -> String {
        return "default".to_string();
    }

    fn add_policy_annotation(&self, _encoded_policy: &str) {

    }

    fn get_containers(&self) -> Vec<pod::Container> {
        return Vec::new();
    }
    
    fn remove_container(&self, _i: usize) {

    }
    
    fn get_volumes(&self) -> Option<Vec<volumes::Volume>> {
        None
    }

    fn serialize(&self, _file_name: &Option<String>) {

    }
}
