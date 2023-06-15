// Copyright (c) 2023 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

// Allow K8s YAML field names.
#![allow(non_snake_case)]

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// See ObjectMeta in the Kubernetes API reference.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ObjectMeta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    labels: Option<BTreeMap<String, String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<BTreeMap<String, String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
}

impl ObjectMeta {
    pub fn get_name(&self) -> Result<String> {
        if let Some(name) = &self.name {
            Ok(name.clone())
        } else {
            Ok(String::new())
        }
    }

    pub fn get_namespace(&self) -> Result<String> {
        if let Some(namespace) = &self.namespace {
            Ok(namespace.clone())
        } else {
            Ok("default".to_string())
        }
    }
}
