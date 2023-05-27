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

impl yaml::K8sObject for ReplicationController {
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
