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
use log::debug;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs::File;

/// See Reference / Kubernetes API / Config and Storage Resources / ConfigMap.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConfigMap {
    apiVersion: String,
    kind: String,
    pub metadata: obj_meta::ObjectMeta,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<BTreeMap<String, String>>,

    // When parsing a YAML file, binaryData is encoded as base64.
    // Therefore, this is a BTreeMap<String, String> instead of BTreeMap<String, Vec<u8>>.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub binaryData: Option<BTreeMap<String, String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    immutable: Option<bool>,

    #[serde(skip)]
    doc_mapping: serde_yaml::Value,
}

impl ConfigMap {
    pub fn new(file: &str) -> anyhow::Result<Self> {
        debug!("Reading ConfigMap...");
        let config_map: ConfigMap = serde_yaml::from_reader(File::open(file)?)?;
        debug!("\nRead ConfigMap => {:#?}", config_map);

        Ok(config_map)
    }

    pub fn get_value(&self, value_from: &pod::EnvVarSource) -> Option<String> {
        if let Some(key_ref) = &value_from.configMapKeyRef {
            if let Some(name) = &key_ref.name {
                if let Some(my_name) = &self.metadata.name {
                    if my_name.eq(name) {
                        if let Some(data) = &self.data {
                            if let Some(value) = data.get(&key_ref.key) {
                                return Some(value.clone());
                            }
                        }
                    }
                }
            }
        }

        None
    }

    pub fn get_key_value_pairs(&self) -> Option<Vec<String>> {
        //eg ["key1=value1", "key2=value2"]
        self.data
            .as_ref()?
            .keys()
            .map(|key| {
                let value = self.data.as_ref().unwrap().get(key).unwrap();
                format!("{key}={value}")
            })
            .collect::<Vec<String>>()
            .into()
    }
}

pub fn get_value(value_from: &pod::EnvVarSource, config_maps: &Vec<ConfigMap>) -> Option<String> {
    for config_map in config_maps {
        if let Some(value) = config_map.get_value(value_from) {
            return Some(value);
        }
    }

    None
}

pub fn get_values(config_map_name: &str, config_maps: &Vec<ConfigMap>) -> Option<Vec<String>> {
    for config_map in config_maps {
        if let Some(existing_configmap_name) = &config_map.metadata.name {
            if config_map_name == existing_configmap_name {
                return config_map.get_key_value_pairs();
            }
        }
    }

    None
}

#[async_trait]
impl yaml::K8sResource for ConfigMap {
    async fn init(
        &mut self,
        _config: &Config,
        doc_mapping: &serde_yaml::Value,
        _silent_unsupported_fields: bool,
    ) {
        self.doc_mapping = doc_mapping.clone();
    }

    fn generate_policy(&self, _agent_policy: &policy::AgentPolicy) -> String {
        "".to_string()
    }

    fn serialize(&mut self, _policy: &str) -> String {
        serde_yaml::to_string(&self.doc_mapping).unwrap()
    }

    fn get_annotations(&self) -> &Option<BTreeMap<String, String>> {
        &self.metadata.annotations
    }
}
