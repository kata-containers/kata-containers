// Copyright (c) 2023 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

// Allow K8s YAML field names.
#![allow(non_snake_case)]

use crate::config_map;
use crate::cronjob;
use crate::daemon_set;
use crate::deployment;
use crate::job;
use crate::list;
use crate::mount_and_storage;
use crate::no_policy;
use crate::pod;
use crate::policy;
use crate::replica_set;
use crate::replication_controller;
use crate::secret;
use crate::settings;
use crate::stateful_set;
use crate::utils::Config;
use crate::volume;

use async_trait::async_trait;
use core::fmt::Debug;
use log::debug;
use protocols::agent;
use serde::{Deserialize, Serialize};
use std::boxed;
use std::collections::BTreeMap;
use std::fs::read_to_string;

/// K8s API version and resource type.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct YamlHeader {
    pub apiVersion: String,
    pub kind: String,
}

/// Trait implemented by each supportes K8s resource type (e.g., Pod or Deployment).
#[async_trait]
pub trait K8sResource {
    async fn init(
        &mut self,
        config: &Config,
        doc_mapping: &serde_yaml::Value,
        silent_unsupported_fields: bool,
    );

    fn generate_policy(&self, _agent_policy: &policy::AgentPolicy) -> String {
        panic!("Unsupported");
    }

    fn serialize(&mut self, _policy: &str) -> String {
        panic!("Unsupported");
    }

    fn get_sandbox_name(&self) -> Option<String> {
        panic!("Unsupported");
    }

    fn get_namespace(&self) -> Option<String> {
        panic!("Unsupported");
    }

    fn get_container_mounts_and_storages(
        &self,
        _policy_mounts: &mut Vec<policy::KataMount>,
        _storages: &mut Vec<agent::Storage>,
        _container: &pod::Container,
        _settings: &settings::Settings,
    ) {
        panic!("Unsupported");
    }

    fn get_containers(&self) -> &Vec<pod::Container> {
        panic!("Unsupported");
    }

    fn get_annotations(&self) -> &Option<BTreeMap<String, String>> {
        panic!("Unsupported");
    }

    fn use_host_network(&self) -> bool {
        panic!("Unsupported");
    }

    fn use_sandbox_pidns(&self) -> bool {
        panic!("Unsupported");
    }

    fn get_runtime_class_name(&self) -> Option<String> {
        None
    }

    fn get_process_fields(&self, _process: &mut policy::KataProcess) {
        // No need to implement support for securityContext or similar fields
        // for some of the K8s resource types.
    }
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
    let d = serde_yaml::Deserializer::from_str(yaml);

    match kind {
        "ConfigMap" => {
            let config_map: config_map::ConfigMap = serde_ignored::deserialize(d, |path| {
                handle_unused_field(&path.to_string(), silent_unsupported_fields);
            })
            .unwrap();
            debug!("{:#?}", &config_map);
            Ok((boxed::Box::new(config_map), header.kind))
        }
        "DaemonSet" => {
            let daemon: daemon_set::DaemonSet = serde_ignored::deserialize(d, |path| {
                handle_unused_field(&path.to_string(), silent_unsupported_fields);
            })
            .unwrap();
            debug!("{:#?}", &daemon);
            Ok((boxed::Box::new(daemon), header.kind))
        }
        "Deployment" => {
            let deployment: deployment::Deployment = serde_ignored::deserialize(d, |path| {
                handle_unused_field(&path.to_string(), silent_unsupported_fields);
            })
            .unwrap();
            debug!("{:#?}", &deployment);
            Ok((boxed::Box::new(deployment), header.kind))
        }
        "Job" => {
            let job: job::Job = serde_ignored::deserialize(d, |path| {
                handle_unused_field(&path.to_string(), silent_unsupported_fields);
            })
            .unwrap();
            debug!("{:#?}", &job);
            Ok((boxed::Box::new(job), header.kind))
        }
        "CronJob" => {
            let cronJob: cronjob::CronJob = serde_ignored::deserialize(d, |path| {
                handle_unused_field(&path.to_string(), silent_unsupported_fields);
            })
            .unwrap();
            debug!("{:#?}", &cronJob);
            Ok((boxed::Box::new(cronJob), header.kind))
        }
        "List" => {
            let list: list::List = serde_ignored::deserialize(d, |path| {
                handle_unused_field(&path.to_string(), silent_unsupported_fields);
            })
            .unwrap();
            debug!("{:#?}", &list);
            Ok((boxed::Box::new(list), header.kind))
        }
        "Pod" => {
            let pod: pod::Pod = serde_ignored::deserialize(d, |path| {
                handle_unused_field(&path.to_string(), silent_unsupported_fields);
            })
            .unwrap();
            debug!("{:#?}", &pod);
            Ok((boxed::Box::new(pod), header.kind))
        }
        "ReplicaSet" => {
            let set: replica_set::ReplicaSet = serde_ignored::deserialize(d, |path| {
                handle_unused_field(&path.to_string(), silent_unsupported_fields);
            })
            .unwrap();
            debug!("{:#?}", &set);
            Ok((boxed::Box::new(set), header.kind))
        }
        "ReplicationController" => {
            let controller: replication_controller::ReplicationController =
                serde_ignored::deserialize(d, |path| {
                    handle_unused_field(&path.to_string(), silent_unsupported_fields);
                })
                .unwrap();
            debug!("{:#?}", &controller);
            Ok((boxed::Box::new(controller), header.kind))
        }
        "Secret" => {
            let secret: secret::Secret = serde_ignored::deserialize(d, |path| {
                handle_unused_field(&path.to_string(), silent_unsupported_fields);
            })
            .unwrap();
            debug!("{:#?}", &secret);
            Ok((boxed::Box::new(secret), header.kind))
        }
        "StatefulSet" => {
            let set: stateful_set::StatefulSet = serde_ignored::deserialize(d, |path| {
                handle_unused_field(&path.to_string(), silent_unsupported_fields);
            })
            .unwrap();
            debug!("{:#?}", &set);
            Ok((boxed::Box::new(set), header.kind))
        }
        "ClusterRole"
        | "ClusterRoleBinding"
        | "LimitRange"
        | "Namespace"
        | "PersistentVolume"
        | "PersistentVolumeClaim"
        | "PodDisruptionBudget"
        | "PriorityClass"
        | "ResourceQuota"
        | "Role"
        | "RoleBinding"
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
        read_to_string(yaml)?
    } else {
        std::io::read_to_string(std::io::stdin())?
    };

    Ok(yaml_string)
}

pub fn get_yaml_header(yaml: &str) -> anyhow::Result<YamlHeader> {
    Ok(serde_yaml::from_str(yaml)?)
}

pub async fn k8s_resource_init(spec: &mut pod::PodSpec, config: &Config) {
    for container in &mut spec.containers {
        container.init(config).await;
    }

    pod::add_pause_container(&mut spec.containers, config).await;

    if let Some(init_containers) = &spec.initContainers {
        for container in init_containers {
            let mut new_container = container.clone();
            new_container.init(config).await;
            spec.containers.insert(1, new_container);
        }
    }
}

pub fn get_container_mounts_and_storages(
    policy_mounts: &mut Vec<policy::KataMount>,
    storages: &mut Vec<agent::Storage>,
    container: &pod::Container,
    settings: &settings::Settings,
    volumes_option: &Option<Vec<volume::Volume>>,
) {
    if let Some(volumes) = volumes_option {
        if let Some(volume_mounts) = &container.volumeMounts {
            for volume in volumes {
                for volume_mount in volume_mounts {
                    if volume_mount.name.eq(&volume.name) {
                        mount_and_storage::get_mount_and_storage(
                            settings,
                            policy_mounts,
                            storages,
                            volume,
                            volume_mount,
                        );
                    }
                }
            }
        }
    }

    // Add storage and mount for each volume defined in the docker container image
    // configuration layer.
    if let Some(volumes) = &container.registry.config_layer.config.Volumes {
        for volume in volumes {
            debug!("get_container_mounts_and_storages: {:?}", &volume);

            mount_and_storage::get_image_mount_and_storage(
                settings,
                policy_mounts,
                storages,
                volume.0,
            );
        }
    }
}

/// Add the "io.katacontainers.config.agent.policy" annotation into
/// a serde representation of a K8s resource YAML.
pub fn add_policy_annotation(
    mut ancestor: &mut serde_yaml::Value,
    metadata_path: &str,
    policy: &str,
) {
    let annotations_key = serde_yaml::Value::String("annotations".to_string());
    let policy_key = serde_yaml::Value::String("io.katacontainers.config.agent.policy".to_string());
    let policy_value = serde_yaml::Value::String(policy.to_string());

    if !metadata_path.is_empty() {
        let path_components = metadata_path.split('.');
        for name in path_components {
            ancestor = ancestor.get_mut(name).unwrap();
        }
    }

    // Add metadata to the output if the input YAML didn't include it.
    let metadata = "metadata";
    if ancestor.get(metadata).is_none() {
        let new_mapping = serde_yaml::Value::Mapping(serde_yaml::Mapping::new());
        ancestor
            .as_mapping_mut()
            .unwrap()
            .insert(serde_yaml::Value::String(metadata.to_string()), new_mapping);
    }
    ancestor = ancestor.get_mut(metadata).unwrap();

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

pub fn remove_policy_annotation(annotations: &mut BTreeMap<String, String>) {
    annotations.remove("io.katacontainers.config.agent.policy");
}

/// Report a fatal error if this app encounters an unsupported input YAML field,
/// unless the user requested this app to ignore unsupported input fields.
/// "Silent unsupported fields" is an expert level feature, because some of
/// the fields ignored silently might be relevant for the output policy,
/// with hard to predict outcomes.
fn handle_unused_field(path: &str, silent_unsupported_fields: bool) {
    if !silent_unsupported_fields {
        panic!("Unsupported field: {}", path);
    }
}

pub fn get_process_fields(
    process: &mut policy::KataProcess,
    security_context: &Option<pod::PodSecurityContext>,
) {
    if let Some(context) = security_context {
        if let Some(uid) = context.runAsUser {
            process.User.UID = uid.try_into().unwrap();
        }
    }
}
