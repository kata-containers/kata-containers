// Copyright (c) 2023 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

// Allow K8s YAML field names.
#![allow(non_snake_case)]

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const POLICY_ANNOTATION_KEY: &str = "io.katacontainers.config.agent.policy";

/// See ObjectMeta in the Kubernetes API reference.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectMeta {
    #[serde(skip_serializing_if = "Option::is_none")]
    labels: Option<BTreeMap<String, String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<BTreeMap<String, String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
}

impl ObjectMeta {
    pub fn get_name(&self) -> String {
        if let Some(name) = &self.name {
            name.clone()
        } else {
            String::new()
        }
    }

    pub fn get_namespace(&self) -> String {
        if let Some(namespace) = &self.namespace {
            namespace.clone()
        } else {
            "default".to_string()
        }
    }

    pub fn add_policy_annotation(&mut self, encoded_policy: &str) {
        if let Some(annotations) = &mut self.annotations {
            annotations
                .entry(POLICY_ANNOTATION_KEY.to_string())
                .and_modify(|v| *v = encoded_policy.to_string())
                .or_insert(encoded_policy.to_string());
        } else {
            let mut annotations = BTreeMap::new();
            annotations.insert(
                POLICY_ANNOTATION_KEY.to_string(),
                encoded_policy.to_string(),
            );
            self.annotations = Some(annotations);
        }
    }
}
