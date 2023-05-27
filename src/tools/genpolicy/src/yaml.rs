// Copyright (c) 2023 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

// Allow K8s YAML field names.
#![allow(non_snake_case)]

use crate::pod;
use crate::volumes;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_yaml;
use std::fs::read_to_string;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct YamlHeader {
    pub apiVersion: String,
    pub kind: String,
}

pub trait K8sObject {
    fn get_metadata_name(&self) -> String;
    fn get_host_name(&self) -> String;
    fn get_sandbox_name(&self) -> Option<String>;
    fn get_namespace(&self) -> String;
    fn add_policy_annotation(&self, encoded_policy: &str);
    fn get_containers(&self) -> Vec<pod::Container>;
    fn remove_container(&self, i: usize);
    fn get_volumes(&self) -> Option<Vec<volumes::Volume>>;
    fn serialize(&self, file_name: &Option<String>);
}

pub fn get_input_yaml(yaml_file: &Option<String>) -> Result<String> {
    let yaml_string = if let Some(yaml) = yaml_file {
        read_to_string(&yaml)?
    } else {
        std::io::read_to_string(std::io::stdin())?
    };

    Ok(yaml_string)
}

pub fn get_yaml_header(yaml: &str) -> Result<YamlHeader> {
    return Ok(serde_yaml::from_str(yaml)?);
}
