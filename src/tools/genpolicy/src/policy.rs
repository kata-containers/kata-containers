// Copyright (c) 2023 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

// Allow OCI spec field names.
#![allow(non_snake_case)]

use crate::config_maps;
use crate::containerd;
use crate::deployment;
use crate::infra;
use crate::kata;
use crate::list;
use crate::pause_container;
use crate::pod;
use crate::registry;
use crate::replication_controller;
use crate::service;
use crate::utils;
use crate::volumes;
use crate::yaml;

use anyhow::{anyhow, Result};
use base64::{engine::general_purpose, Engine as _};
use log::debug;
use oci::*;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::boxed;
use std::collections::BTreeMap;
use std::fs::read_to_string;
use std::io::Write;

/// Creates one of the supported K8s objects from a YAML string.
fn new_k8s_object(kind: &str, yaml: &str) -> Result<boxed::Box<dyn yaml::K8sObject>> {
    match kind {
        "Deployment" => {
            let mut deployment: deployment::Deployment = serde_yaml::from_str(&yaml)?;
            pause_container::add_pause_container(&mut deployment.spec.template.spec.containers);
            debug!("deployment = {:#?}", &deployment);
            Ok(boxed::Box::new(deployment))
        }
        "List" => {
            let list: list::List = serde_yaml::from_str(&yaml).unwrap();
            // pause_container::add_pause_container(&mut deployment.spec.template.spec.containers);
            debug!("list = {:#?}", &list);
            Ok(boxed::Box::new(list))
        }
        "Pod" => {
            let mut pod: pod::Pod = serde_yaml::from_str(&yaml)?;
            pause_container::add_pause_container(&mut pod.spec.containers);
            debug!("pod = {:#?}", &pod);
            Ok(boxed::Box::new(pod))
        }
        "ReplicationController" => {
            let mut controller: replication_controller::ReplicationController =
                serde_yaml::from_str(&yaml)?;
            pause_container::add_pause_container(&mut controller.spec.template.spec.containers);
            debug!("controller = {:#?}", &controller);
            Ok(boxed::Box::new(controller))
        }
        "Service" => {
            let service: service::Service = serde_yaml::from_str(&yaml)?;
            debug!("service = {:#?}", &service);
            Ok(boxed::Box::new(service))
        }
        _ => Err(anyhow!("Unsupported YAML spec kind: {}", kind)),
    }
}

pub struct AgentPolicy {
    k8s_object: boxed::Box<dyn yaml::K8sObject>,

    config_maps: Vec<config_maps::ConfigMap>,

    yaml_file: Option<String>,
    rules_input_file: String,

    infra_policy: infra::InfraPolicy,
}

// Example:
//
// policy_data := {
//   "containers": [
//     {
//       "oci": {
//         "ociVersion": "1.1.0-rc.1",
// ...
#[derive(Debug, Serialize, Deserialize)]
pub struct PolicyData {
    pub containers: Vec<ContainerPolicy>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub volumes: Option<Volumes>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct OciSpec {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ociVersion: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub process: Option<OciProcess>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub root: Option<Root>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub hostname: Option<String>,

    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub mounts: Vec<Mount>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub hooks: Option<Hooks>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<BTreeMap<String, String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub linux: Option<Linux>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct OciProcess {
    pub terminal: bool,
    pub user: User,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub env: Vec<String>,

    #[serde(skip_serializing_if = "String::is_empty")]
    pub cwd: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<LinuxCapabilities>,

    pub noNewPrivileges: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ContainerPolicy {
    pub oci: OciSpec,
    storages: Vec<SerializedStorage>,
}

// TODO: can struct Storage from agent.proto be used here?
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SerializedStorage {
    pub driver: String,
    pub driver_options: Vec<String>,
    pub source: String,
    pub fstype: String,
    pub options: Vec<String>,
    pub mount_point: String,
    pub fs_group: SerializedFsGroup,
}

// TODO: can struct FsGroup from agent.proto be used here?
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SerializedFsGroup {
    pub group_id: u32,
    pub group_change_policy: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Volumes {
    pub emptyDir: Option<EmptyDirVolume>,
    pub persistentVolumeClaim: Option<PersistentVolumeClaimVolume>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EmptyDirVolume {
    pub mount_type: String,
    pub mount_point: String,
    pub mount_source: String,
    pub driver: String,
    pub source: String,
    pub fstype: String,
    pub options: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PersistentVolumeClaimVolume {
    pub mount_type: String,
    pub mount_source: String,
}

impl AgentPolicy {
    pub fn from_files(in_out_files: &utils::InOutFiles) -> Result<AgentPolicy> {
        let yaml_string = yaml::get_input_yaml(&in_out_files.yaml_file)?;
        let header = yaml::get_yaml_header(&yaml_string)?;

        let mut config_maps = Vec::new();
        if let Some(config_map_files) = &in_out_files.config_map_files {
            for file in config_map_files {
                config_maps.push(config_maps::ConfigMap::new(&file)?);
            }
        }

        let infra_policy = infra::InfraPolicy::new(&in_out_files.infra_data_file)?;

        let mut yaml_file = None;
        if let Some(yaml_path) = &in_out_files.yaml_file {
            yaml_file = Some(yaml_path.to_string());
        }

        Ok(AgentPolicy {
            k8s_object: new_k8s_object(&header.kind, &yaml_string)?,
            yaml_file,
            rules_input_file: in_out_files.rules_file.to_string(),
            infra_policy,
            config_maps,
        })
    }

    pub async fn export_policy(&mut self, in_out_files: &utils::InOutFiles) -> Result<()> {
        if !self.k8s_object.as_ref().requires_policy() {
            return Ok(());
        }

        let registry_containers = self.k8s_object.get_registry_containers().await?;

        let policy_data = self.k8s_object.get_policy_data(
            self.k8s_object.as_ref(),
            &self.infra_policy,
            &self.config_maps,
            &registry_containers,
        )?;

        let json_data = serde_json::to_string_pretty(&policy_data)
            .map_err(|e| anyhow!(e))
            .unwrap();

        debug!("============================================");
        debug!("Adding policy to YAML");

        let mut policy = read_to_string(&self.rules_input_file)?;
        policy += "\npolicy_data := ";
        policy += &json_data;

        if let Some(file_name) = &in_out_files.output_policy_file {
            export_decoded_policy(&policy, &file_name)?;
        }

        let encoded_policy = general_purpose::STANDARD.encode(policy.as_bytes());
        self.k8s_object.add_policy_annotation(&encoded_policy);
        self.k8s_object.serialize(&self.yaml_file)?;

        Ok(())
    }
}

fn get_image_layer_storages(
    storages: &mut Vec<SerializedStorage>,
    image_layers: &Vec<registry::ImageLayer>,
    root: &Option<Root>,
) -> Result<()> {
    if let Some(root_mount) = root {
        let mut overlay_storage = SerializedStorage {
            driver: "blk".to_string(),
            driver_options: Vec::new(),
            source: String::new(), // TODO
            fstype: "tar-overlay".to_string(),
            options: Vec::new(),
            mount_point: root_mount.path.clone(),
            fs_group: SerializedFsGroup {
                group_id: 0,
                group_change_policy: 0,
            },
        };

        // TODO: load this path from data.json.
        let layers_path = "/run/kata-containers/sandbox/layers/".to_string();

        let mut previous_chain_id = String::new();
        for layer in image_layers {
            let verity_option = "kata.dm-verity=".to_string() + &layer.verity_hash;

            // See https://github.com/opencontainers/image-spec/blob/main/config.md#layer-chainid
            let chain_id = if previous_chain_id.is_empty() {
                layer.diff_id.clone()
            } else {
                let mut hasher = Sha256::new();
                hasher.update(previous_chain_id.clone() + " " + &layer.diff_id);
                format!("sha256:{:x}", hasher.finalize())
            };
            debug!(
                "previous_chain_id = {}, chain_id = {}",
                &previous_chain_id, &chain_id
            );
            previous_chain_id = chain_id.clone();

            let layer_name = name_to_hash(&chain_id);

            storages.push(SerializedStorage {
                driver: "blk".to_string(),
                driver_options: Vec::new(),
                source: String::new(), // TODO
                fstype: "tar".to_string(),
                options: vec!["ro".to_string(), verity_option],
                mount_point: layers_path.clone() + &layer_name,
                fs_group: SerializedFsGroup {
                    group_id: 0,
                    group_change_policy: 0,
                },
            });

            overlay_storage
                .options
                .push("kata.layer=".to_string() + &layer_name + "," + &layer.verity_hash);
        }

        storages.push(overlay_storage);
    }

    Ok(())
}

// TODO: avoid copying this code from snapshotter.rs
/// Converts the given name to a string representation of its sha256 hash.
fn name_to_hash(name: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(name);
    format!("{:x}", hasher.finalize())
}

/// Creates a text file including the Rego rules and data.
fn export_decoded_policy(policy: &str, file_name: &str) -> Result<()> {
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .truncate(true)
        .create(true)
        .open(file_name)
        .map_err(|e| anyhow!(e))?;
    f.write_all(policy.as_bytes()).map_err(|e| anyhow!(e))?;
    f.flush().map_err(|e| anyhow!(e))?;
    Ok(())
}

pub fn get_policy_data(
    k8s_object: &dyn yaml::K8sObject,
    infra_policy: &infra::InfraPolicy,
    config_maps: &Vec<config_maps::ConfigMap>,
    yaml_containers: &Vec<pod::Container>,
    registry_containers: &Vec<registry::Container>,
) -> Result<PolicyData> {
    let mut policy_containers = Vec::new();

    for i in 0..yaml_containers.len() {
        policy_containers.push(get_container_policy(
            k8s_object,
            infra_policy,
            config_maps,
            &yaml_containers[i],
            i == 0,
            &registry_containers[i],
        )?);
    }

    Ok(PolicyData {
        containers: policy_containers,
        volumes: None,
    })
}

fn get_container_policy(
    k8s_object: &dyn yaml::K8sObject,
    infra_policy: &infra::InfraPolicy,
    config_maps: &Vec<config_maps::ConfigMap>,
    yaml_container: &pod::Container,
    is_pause_container: bool,
    registry_container: &registry::Container,
) -> Result<ContainerPolicy> {
    let pod_name = k8s_object.get_metadata_name()?;
    let hostname = k8s_object.get_host_name()?;

    let mut infra_container = &infra_policy.pause_container;
    if !is_pause_container {
        infra_container = &infra_policy.other_container;
    }

    let mut root: Option<Root> = None;
    if let Some(infra_root) = &infra_container.root {
        let mut policy_root = infra_root.clone();
        policy_root.readonly = yaml_container.read_only_root_filesystem();
        root = Some(policy_root);
    }

    let mut annotations = BTreeMap::new();
    infra::get_annotations(&mut annotations, infra_container)?;
    if let Some(name) = k8s_object.get_sandbox_name()? {
        annotations.insert("io.kubernetes.cri.sandbox-name".to_string(), name);
    }

    if !is_pause_container {
        let mut image_name = yaml_container.image.to_string();
        if image_name.find(':').is_none() {
            image_name += ":latest";
        }
        annotations.insert("io.kubernetes.cri.image-name".to_string(), image_name);
    }

    let namespace = k8s_object.get_namespace()?;
    annotations.insert("io.kubernetes.cri.sandbox-namespace".to_string(), namespace);

    if !yaml_container.name.is_empty() {
        annotations.insert(
            "io.kubernetes.cri.container-name".to_string(),
            yaml_container.name.to_string(),
        );
    }

    // Start with the Default Unix Spec from
    // https://github.com/containerd/containerd/blob/release/1.6/oci/spec.go#L132
    let privileged_container = yaml_container.is_privileged();
    let mut process = containerd::get_process(privileged_container);
    let (yaml_has_command, yaml_has_args) = yaml_container.get_process_args(&mut process.args);

    registry_container.get_process(&mut process, yaml_has_command, yaml_has_args)?;

    if !is_pause_container {
        process.env.push("HOSTNAME=".to_string() + &pod_name);
    }

    yaml_container.get_env_variables(&mut process.env, config_maps);

    infra::get_process(&mut process, &infra_container)?;
    process.noNewPrivileges = !yaml_container.allow_privilege_escalation();

    let mut mounts = containerd::get_mounts(is_pause_container, privileged_container);
    infra_policy.get_policy_mounts(
        &mut mounts,
        &infra_container.mounts,
        &yaml_container,
        is_pause_container,
    )?;

    let image_layers = registry_container.get_image_layers();
    let mut storages = Default::default();
    get_image_layer_storages(&mut storages, &image_layers, &root)?;

    k8s_object.get_container_mounts_and_storages(
        &mut mounts,
        &mut storages,
        &yaml_container,
        infra_policy,
    )?;

    let mut linux = containerd::get_linux(privileged_container);
    linux.namespaces = kata::get_namespaces();
    infra::get_linux(&mut linux, &infra_container.linux)?;

    Ok(ContainerPolicy {
        oci: OciSpec {
            ociVersion: Some("1.1.0-rc.1".to_string()),
            process: Some(process),
            root,
            hostname: Some(hostname),
            mounts,
            hooks: None,
            annotations: Some(annotations),
            linux: Some(linux),
        },
        storages,
    })
}

pub fn get_container_mounts_and_storages(
    policy_mounts: &mut Vec<oci::Mount>,
    storages: &mut Vec<SerializedStorage>,
    container: &pod::Container,
    infra_policy: &infra::InfraPolicy,
    volume: &volumes::Volume,
) -> Result<()> {
    if let Some(volume_mounts) = &container.volumeMounts {
        for volume_mount in volume_mounts {
            if volume_mount.name.eq(&volume.name) {
                infra_policy.get_mount_and_storage(
                    policy_mounts,
                    storages,
                    volume,
                    volume_mount,
                )?;
            }
        }
    }
    Ok(())
}
