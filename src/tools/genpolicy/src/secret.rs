// Copyright (c) 2023 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

// Allow K8s YAML field names.
#![allow(non_snake_case)]

use crate::obj_meta;
use crate::pod;
use crate::policy;
use crate::utils::Config;
use crate::yaml;

use async_trait::async_trait;
use base64::{engine::general_purpose, Engine as _};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// See Reference / Kubernetes API / Config and Storage Resources / Secret.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Secret {
    #[serde(skip)]
    doc_mapping: serde_yaml::Value,

    apiVersion: String,
    kind: String,
    metadata: obj_meta::ObjectMeta,

    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<BTreeMap<String, String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    immutable: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    r#type: Option<String>,
    // TODO: additional fields.
}

impl Secret {
    pub fn get_value(&self, value_from: &pod::EnvVarSource) -> Option<String> {
        if let Some(key_ref) = &value_from.secretKeyRef {
            if let Some(name) = &key_ref.name {
                if let Some(my_name) = &self.metadata.name {
                    if my_name.eq(name) {
                        if let Some(data) = &self.data {
                            if let Some(value) = data.get(&key_ref.key) {
                                let value_bytes = general_purpose::STANDARD.decode(value).unwrap();
                                let value_string = std::str::from_utf8(&value_bytes).unwrap();
                                return Some(value_string.to_string());
                            }
                        }
                    }
                }
            }
        }

        None
    }

    pub fn get_key_value_pairs(&self) -> Option<Vec<String>> {
        //eg ["key1=secret1", "key2=secret2"]
        self.data
            .as_ref()?
            .keys()
            .map(|key| {
                let value = self.data.as_ref().unwrap().get(key).unwrap();
                let value_bytes = general_purpose::STANDARD.decode(value).unwrap();
                let value_string = std::str::from_utf8(&value_bytes).unwrap();
                format!("{key}={value_string}")
            })
            .collect::<Vec<String>>()
            .into()
    }
}

pub fn get_value(value_from: &pod::EnvVarSource, secrets: &Vec<Secret>) -> Option<String> {
    for secret in secrets {
        if let Some(value) = secret.get_value(value_from) {
            return Some(value);
        }
    }

    None
}

pub fn get_values(secret_name: &str, secrets: &Vec<Secret>) -> Option<Vec<String>> {
    for secret in secrets {
        if let Some(existing_secret_name) = &secret.metadata.name {
            if existing_secret_name == secret_name {
                return secret.get_key_value_pairs();
            }
        }
    }

    None
}

#[async_trait]
impl yaml::K8sResource for Secret {
    async fn init(&mut self, _config: &Config, doc_mapping: &serde_yaml::Value, _silent: bool) {
        self.doc_mapping = doc_mapping.clone();
    }

    fn generate_policy(&self, _agent_policy: &policy::AgentPolicy) -> String {
        "".to_string()
    }

    fn serialize(&mut self, _policy: &str) -> String {
        serde_yaml::to_string(&self.doc_mapping).unwrap()
    }
}
