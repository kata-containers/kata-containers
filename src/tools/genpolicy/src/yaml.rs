// Copyright (c) 2023 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

// Allow K8s YAML field names.
#![allow(non_snake_case)]

use crate::config_maps;
use crate::daemon_set;
use crate::deployment;
use crate::list;
use crate::infra;
use crate::no_policy_obj;
use crate::pod;
use crate::policy;
use crate::replication_controller;
use crate::stateful_set;
use crate::utils;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use log::debug;
use core::fmt::Debug;
use serde::{Deserialize, Serialize};
use serde_yaml;
use std::boxed;
use std::collections::BTreeMap;
use std::fs::read_to_string;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct YamlHeader {
    pub apiVersion: String,
    pub kind: String,
}

#[async_trait]
pub trait K8sObject {
    async fn initialize(&mut self, use_cached_files: bool) -> Result<()>;

    fn requires_policy(&self) -> bool;

    fn generate_policy(
        &mut self,
        rules: &str,
        infra_policy: &infra::InfraPolicy,
        config_maps: &Vec<config_maps::ConfigMap>,
        in_out_files: &utils::InOutFiles,
    ) -> Result<()>;

    fn serialize(&mut self) -> Result<String>;

    fn get_metadata_name(&self) -> Result<String>;
    fn get_host_name(&self) -> Result<String>;
    fn get_sandbox_name(&self) -> Result<Option<String>>;
    fn get_namespace(&self) -> Result<String>;

    fn get_container_mounts_and_storages(
        &self,
        policy_mounts: &mut Vec<oci::Mount>,
        storages: &mut Vec<policy::SerializedStorage>,
        container: &pod::Container,
        infra_policy: &infra::InfraPolicy,
    ) -> Result<()>;
}

/// See Reference / Kubernetes API / Common Definitions / LabelSelector.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LabelSelector {
    #[serde(skip_serializing_if = "Option::is_none")]
    matchLabels: Option<BTreeMap<String, String>>,

    // TODO: additional fields.
}

/// Creates one of the supported K8s objects from a YAML string.
pub fn new_k8s_object(kind: &str, yaml: &str) -> Result<boxed::Box<dyn K8sObject + Sync + Send>> {
    match kind {
        "DaemonSet" => {
            let daemon: daemon_set::DaemonSet = serde_yaml::from_str(&yaml)?;
            debug!("{:#?}", &daemon);
            Ok(boxed::Box::new(daemon))
        }
        "Deployment" => {
            let deployment: deployment::Deployment = serde_yaml::from_str(&yaml)?;
            debug!("{:#?}", &deployment);
            Ok(boxed::Box::new(deployment))
        }
        "List" => {
            let list: list::List = serde_yaml::from_str(&yaml)?;
            debug!("{:#?}", &list);
            Ok(boxed::Box::new(list))
        }
        "Pod" => {
            let pod: pod::Pod = serde_yaml::from_str(&yaml)?;
            debug!("{:#?}", &pod);
            Ok(boxed::Box::new(pod))
        }
        "ReplicationController" => {
            let controller: replication_controller::ReplicationController =
                serde_yaml::from_str(&yaml)?;
            debug!("{:#?}", &controller);
            Ok(boxed::Box::new(controller))
        }
        "StatefulSet" => {
            let set: stateful_set::StatefulSet = serde_yaml::from_str(&yaml)?;
            debug!("{:#?}", &set);
            Ok(boxed::Box::new(set))
        }
        "LimitRange" | "Namespace" | "ResourceQuota" | "Service" => {
            let no_policy = no_policy_obj::NoPolicyObject {
                yaml: yaml.to_string(),
            };
            debug!("{:#?}", &no_policy);
            Ok(boxed::Box::new(no_policy))
        }
        _ => Err(anyhow!("Unsupported YAML spec kind: {}", kind)),
    }
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
