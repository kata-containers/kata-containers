// Copyright (c) 2023 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

// Allow K8s YAML field names.
#![allow(non_snake_case)]

use crate::config_map;
use crate::daemon_set;
use crate::deployment;
use crate::infra;
use crate::job;
use crate::list;
use crate::no_policy;
use crate::pause_container;
use crate::pod;
use crate::policy;
use crate::registry;
use crate::replica_set;
use crate::replication_controller;
use crate::stateful_set;
use crate::utils;
use crate::volume;

use anyhow::anyhow;
use async_trait::async_trait;
use base64::{engine::general_purpose, Engine as _};
use core::fmt::Debug;
use log::debug;
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
pub trait K8sResource {
    async fn init(
        &mut self,
        use_cache: bool,
        doc_mapping: &serde_yaml::Value,
        silent_unsupported_fields: bool,
    ) -> anyhow::Result<()>;

    fn generate_policy(
        &mut self,
        rules: &str,
        infra_policy: &infra::InfraPolicy,
        config_maps: &Vec<config_map::ConfigMap>,
        config: &utils::Config,
    ) -> anyhow::Result<()>;

    fn serialize(&mut self) -> String;

    fn get_metadata_name(&self) -> String;
    fn get_host_name(&self) -> String;
    fn get_sandbox_name(&self) -> Option<String>;
    fn get_namespace(&self) -> String;

    fn get_container_mounts_and_storages(
        &self,
        policy_mounts: &mut Vec<oci::Mount>,
        storages: &mut Vec<policy::SerializedStorage>,
        container: &pod::Container,
        infra_policy: &infra::InfraPolicy,
    );
}

/// See Reference / Kubernetes API / Common Definitions / LabelSelector.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct LabelSelector {
    #[serde(skip_serializing_if = "Option::is_none")]
    matchLabels: Option<BTreeMap<String, String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    matchExpressions: Option<Vec<LabelSelectorRequirement>>,
}

/// See Reference / Kubernetes API / Common Definitions / LabelSelector.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct LabelSelectorRequirement {
    key: String,
    operator: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    values: Option<Vec<String>>,
}

/// Creates one of the supported K8s objects from a YAML string.
pub fn new_k8s_resource(
    yaml: &str,
    silent_unsupported_fields: bool,
) -> anyhow::Result<(boxed::Box<dyn K8sResource + Sync + Send>, String)> {
    let header = get_yaml_header(yaml)?;
    let kind: &str = &header.kind;
    let d = serde_yaml::Deserializer::from_str(&yaml);

    match kind {
        "ConfigMap" => {
            let config_map: config_map::ConfigMap = serde_ignored::deserialize(d, |path| {
                handle_unused_field(&path.to_string(), silent_unsupported_fields);
            }).unwrap();
            debug!("{:#?}", &config_map);
            Ok((boxed::Box::new(config_map), header.kind))
        }
        "DaemonSet" => {
            let daemon: daemon_set::DaemonSet = serde_ignored::deserialize(d, |path| {
                handle_unused_field(&path.to_string(), silent_unsupported_fields);
            }).unwrap();
            debug!("{:#?}", &daemon);
            Ok((boxed::Box::new(daemon), header.kind))
        }
        "Deployment" => {
            let deployment: deployment::Deployment = serde_ignored::deserialize(d, |path| {
                handle_unused_field(&path.to_string(), silent_unsupported_fields);
            }).unwrap();
            debug!("{:#?}", &deployment);
            Ok((boxed::Box::new(deployment), header.kind))
        }
        "Job" => {
            let job: job::Job = serde_ignored::deserialize(d, |path| {
                handle_unused_field(&path.to_string(), silent_unsupported_fields);
            }).unwrap();
            debug!("{:#?}", &job);
            Ok((boxed::Box::new(job), header.kind))
        }
        "List" => {
            let list: list::List = serde_ignored::deserialize(d, |path| {
                handle_unused_field(&path.to_string(), silent_unsupported_fields);
            }).unwrap();
            debug!("{:#?}", &list);
            Ok((boxed::Box::new(list), header.kind))
        }
        "Pod" => {
            let pod: pod::Pod = serde_ignored::deserialize(d, |path| {
                handle_unused_field(&path.to_string(), silent_unsupported_fields);
            }).unwrap();
            debug!("{:#?}", &pod);
            Ok((boxed::Box::new(pod), header.kind))
        }
        "ReplicationController" => {
            let controller: replication_controller::ReplicationController =
                serde_ignored::deserialize(d, |path| {
                    handle_unused_field(&path.to_string(), silent_unsupported_fields);
                }).unwrap();
            debug!("{:#?}", &controller);
            Ok((boxed::Box::new(controller), header.kind))
        }
        "ReplicaSet" => {
            let set: replica_set::ReplicaSet = serde_ignored::deserialize(d, |path| {
                handle_unused_field(&path.to_string(), silent_unsupported_fields);
            }).unwrap();
            debug!("{:#?}", &set);
            Ok((boxed::Box::new(set), header.kind))
        }
        "StatefulSet" => {
            let set: stateful_set::StatefulSet = serde_ignored::deserialize(d, |path| {
                handle_unused_field(&path.to_string(), silent_unsupported_fields);
            }).unwrap();
            debug!("{:#?}", &set);
            Ok((boxed::Box::new(set), header.kind))
        }
        "ClusterRole"
        | "ClusterRoleBinding"
        | "LimitRange"
        | "Namespace"
        | "PersistentVolume"
        | "PersistentVolumeClaim"
        | "ResourceQuota"
        | "Secret"
        | "Service"
        | "ServiceAccount" => {
            let no_policy = no_policy::NoPolicyResource {
                yaml: yaml.to_string(),
            };
            debug!("{:#?}", &no_policy);
            Ok((boxed::Box::new(no_policy), header.kind))
        }
        _ => todo!("Unsupported YAML spec kind: {}", kind),
    }
}

pub fn get_input_yaml(yaml_file: &Option<String>) -> anyhow::Result<String> {
    let yaml_string = if let Some(yaml) = yaml_file {
        read_to_string(&yaml)?
    } else {
        std::io::read_to_string(std::io::stdin())?
    };

    Ok(yaml_string)
}

pub fn get_yaml_header(yaml: &str) -> anyhow::Result<YamlHeader> {
    return Ok(serde_yaml::from_str(yaml)?);
}

pub async fn k8s_resource_init(
    spec: &mut pod::PodSpec,
    registry_containers: &mut Vec<registry::Container>,
    use_cache: bool,
) -> anyhow::Result<()> {
    pause_container::add_pause_container(&mut spec.containers);

    if let Some(init_containers) = &spec.initContainers {
        for container in init_containers {
            spec.containers.insert(1, container.clone());
        }
    }

    *registry_containers = registry::get_registry_containers(use_cache, &spec.containers).await?;
    Ok(())
}

pub fn get_container_mounts_and_storages(
    policy_mounts: &mut Vec<oci::Mount>,
    storages: &mut Vec<policy::SerializedStorage>,
    container: &pod::Container,
    infra_policy: &infra::InfraPolicy,
    volumes: &Vec<volume::Volume>,
) {
    for volume in volumes {
        policy::get_container_mounts_and_storages(
            policy_mounts,
            storages,
            container,
            infra_policy,
            &volume,
        );
    }
}

pub fn generate_policy(
    rules: &str,
    infra_policy: &infra::InfraPolicy,
    config_maps: &Vec<config_map::ConfigMap>,
    config: &utils::Config,
    k8s_object: &dyn K8sResource,
    registry_containers: &Vec<registry::Container>,
    yaml_containers: &Vec<pod::Container>,
) -> anyhow::Result<String> {
    let mut policy_containers = Vec::new();

    for i in 0..yaml_containers.len() {
        policy_containers.push(policy::get_container_policy(
            k8s_object,
            infra_policy,
            config_maps,
            &yaml_containers[i],
            i == 0,
            &registry_containers[i],
        )?);
    }

    let policy_data = policy::PolicyData {
        containers: policy_containers,
    };

    let json_data = serde_json::to_string_pretty(&policy_data)
        .map_err(|e| anyhow!(e))
        .unwrap();
    let policy = rules.to_string() + "\npolicy_data := " + &json_data;

    if let Some(file_name) = &config.output_policy_file {
        policy::export_decoded_policy(&policy, &file_name)?;
    }
    Ok(general_purpose::STANDARD.encode(policy.as_bytes()))
}

pub fn add_policy_annotation(
    mut ancestor: &mut serde_yaml::Value,
    metadata_path: &str,
    policy: &str,
) {
    let annotations_key = serde_yaml::Value::String("annotations".to_string());
    let policy_key = serde_yaml::Value::String("io.katacontainers.config.agent.policy".to_string());
    let policy_value = serde_yaml::Value::String(policy.to_string());

    let path_components = metadata_path.split('.');
    for name in path_components {
        ancestor = ancestor.get_mut(&name).unwrap();
    }

    if let Some(annotations) = ancestor.get_mut(&annotations_key) {
        if let Some(annotation) = annotations.get_mut(&policy_key) {
            *annotation = policy_value;
        } else if let Some(mapping_mut) = annotations.as_mapping_mut() {
            mapping_mut.insert(policy_key, policy_value);
        } else {
            let mut new_annotations = serde_yaml::Mapping::new();
            new_annotations.insert(policy_key, policy_value);
            *annotations = serde_yaml::Value::Mapping(new_annotations);
        }
    } else {
        let mut new_annotations = serde_yaml::Mapping::new();
        new_annotations.insert(policy_key, policy_value);
        ancestor
            .as_mapping_mut()
            .unwrap()
            .insert(annotations_key, serde_yaml::Value::Mapping(new_annotations));
    }
}

fn handle_unused_field(path: &str, silent_unsupported_fields: bool) {
    if !silent_unsupported_fields {
        panic!("Unsupported field: {}", path);
    }
}
