// Copyright (c) 2023 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

// Allow K8s YAML field names.
#![allow(non_snake_case)]

use crate::yaml;

use anyhow::Result;
use log::info;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs::File;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigMap {
    apiVersion: String,
    kind: String,
    pub metadata: Metadata,
    pub data: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Metadata {
    name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Data {
    data: String,
}

impl ConfigMap {
    pub fn new(file: &str) -> Result<Self> {
        info!("Reading ConfigMap...");
        let config_map: ConfigMap = serde_yaml::from_reader(File::open(file)?)?;
        info!("\nRead ConfigMap => {:#?}", config_map);

        Ok(config_map)
    }

    pub fn get_value(&self, value_from: &yaml::ValueFrom) -> Option<String> {
        if self.metadata.name.eq(&value_from.configMapKeyRef.name) {
            if let Some(value) = self.data.get(&value_from.configMapKeyRef.key) {
                return Some(value.clone())
            }
        }

        None
    }
}

pub fn get_value(value_from: &yaml::ValueFrom, config_maps: &Vec<ConfigMap>) -> Option<String> {
    for config_map in config_maps {
        if let Some(value) = config_map.get_value(value_from) {
            return Some(value)
        }
    }

    None
}
