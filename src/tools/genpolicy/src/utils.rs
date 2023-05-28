// Copyright (c) 2023 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::obj_meta;
use crate::pod;
use crate::registry;

use anyhow::Result;
use log::debug;
use std::collections::BTreeMap;

const POLICY_ANNOTATION_KEY: &str = "io.katacontainers.config.agent.policy";

pub struct InOutFiles {
    pub yaml_file: Option<String>,
    pub rules_file: String,
    pub infra_data_file: String,
    pub output_policy_file: Option<String>,
    pub config_map_files: Option<Vec<String>>,
}

impl InOutFiles {
    pub fn new(
        yaml_file: Option<String>,
        input_files_path: Option<String>,
        output_policy_file: Option<String>,
        config_map_files: &Vec<String>,
    ) -> Self {
        let mut input_path = ".".to_string();
        if let Some(path) = input_files_path {
            input_path = path.clone();
        }
        let rules_file = input_path.to_owned() + "/rules.rego";
        debug!("Rules file: {:?}", &rules_file);

        let infra_data_file = input_path.to_owned() + "/data.json";
        debug!("Infra data file: {:?}", &infra_data_file);

        let cm_files = if !config_map_files.is_empty() {
            Some(config_map_files.clone())
        } else {
            None
        };

        Self {
            yaml_file,
            rules_file,
            infra_data_file,
            output_policy_file,
            config_map_files: cm_files,
        }
    }
}

pub fn get_metadata_name(meta: &obj_meta::ObjectMeta) -> String {
    if let Some(name) = &meta.name {
        name.clone()
    } else {
        String::new()
    }
}

pub fn get_metadata_namespace(meta: &obj_meta::ObjectMeta) -> String {
    if let Some(namespace) = &meta.namespace {
        namespace.clone()
    } else {
        "default".to_string()
    }
}

pub fn add_policy_annotation(meta: &mut obj_meta::ObjectMeta, encoded_policy: &str) {
    if let Some(annotations) = &mut meta.annotations {
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
        meta.annotations = Some(annotations);
    }
}

pub async fn get_registry_containers(
    yaml_containers: &Vec<pod::Container>,
) -> Result<Vec<registry::Container>> {
    let mut registry_containers = Vec::new();

    for yaml_container in yaml_containers {
        registry_containers.push(registry::Container::new(&yaml_container.image).await?);
    }

    Ok(registry_containers)
}
