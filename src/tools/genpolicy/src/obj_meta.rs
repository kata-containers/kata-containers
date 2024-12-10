// Copyright (c) 2023 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

// Allow K8s YAML field names.
#![allow(non_snake_case)]

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

/// See ObjectMeta in the Kubernetes API reference.
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct ObjectMeta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    generateName: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    labels: Option<BTreeMap<String, String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<BTreeMap<String, String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub uid: Option<String>,
}

impl ObjectMeta {
    pub fn get_name(&self) -> String {
        if let Some(name) = &self.name {
            name.clone()
        } else if self.generateName.is_some() {
            "$(generated-name)".to_string()
        } else {
            String::new()
        }
    }

    pub fn get_namespace(&self) -> Option<String> {
        self.namespace.as_ref().cloned()
    }
}

impl fmt::Debug for ObjectMeta {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug_struct = f.debug_struct("ObjectMeta");

        if let Some(ref name) = self.name {
            debug_struct.field("name", name);
        }
        if let Some(ref generate_name) = self.generateName {
            debug_struct.field("generateName", generate_name);
        }
        if let Some(ref labels) = self.labels {
            debug_struct.field("labels", labels);
        }
        if let Some(ref annotations) = self.annotations {
            let truncated_annotations: BTreeMap<_, _> = annotations
                .iter()
                .map(|(key, value)| {
                    if value.len() > 4096 {
                        (
                            key,
                            format!("{}<... truncated ...>", &value[..4096].to_string()),
                        )
                    } else {
                        (key, value.to_string())
                    }
                })
                .collect();
            debug_struct.field("annotations", &truncated_annotations);
        }
        if let Some(ref namespace) = self.namespace {
            debug_struct.field("namespace", namespace);
        }

        debug_struct.finish()
    }
}
