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
use crate::pause_container;
use crate::pod;
use crate::registry;
use crate::replication_controller;
use crate::utils;
use crate::volumes;
use crate::yaml;

use anyhow::{anyhow, Result};
use base64::{engine::general_purpose, Engine as _};
use log::debug;
use oci::*;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs::read_to_string;
use std::io::Write;

const POLICY_ANNOTATION_KEY: &str = "io.katacontainers.config.agent.policy";

pub struct PodPolicy {
    pod: Option<pod::Pod>,
    deployment: Option<deployment::Deployment>,
    replication_controller: Option<replication_controller::ReplicationController>,

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

impl PodPolicy {
    pub fn from_files(in_out_files: &utils::InOutFiles) -> Result<Self> {
        let mut pod = None;
        let mut deployment = None;
        let mut replication_controller = None;

        let yaml_string = yaml::get_input_yaml(&in_out_files.yaml_file)?;
        let header = yaml::get_yaml_header(&yaml_string)?;

        if header.kind.eq("Pod") {
            let mut pod_object: pod::Pod = serde_yaml::from_str(&yaml_string)?;
            pause_container::add_pause_container(&mut pod_object.spec.containers);
            pod = Some(pod_object);
        } else if header.kind.eq("Deployment") {
            let mut deployment_object: deployment::Deployment = serde_yaml::from_str(&yaml_string)?;
            pause_container::add_pause_container(
                &mut deployment_object.spec.template.spec.containers,
            );
            deployment = Some(deployment_object);
        } else if header.kind.eq("ReplicationController") {
            let mut controller_object: replication_controller::ReplicationController =
                serde_yaml::from_str(&yaml_string)?;
            pause_container::add_pause_container(
                &mut controller_object.spec.template.spec.containers,
            );
            replication_controller = Some(controller_object);
        } else {
            panic!("Unsupported YAML spec kind: {}", &header.kind);
        }

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

        Ok(PodPolicy {
            pod,
            deployment,
            replication_controller,
            yaml_file,
            rules_input_file: in_out_files.rules_file.to_string(),
            infra_policy,
            config_maps,
        })
    }

    pub async fn export_policy(&mut self, in_out_files: &utils::InOutFiles) -> Result<()> {
        let mut policy_data = PolicyData {
            containers: Vec::new(),
            volumes: None,
        };

        if let Some(deployment) = &self.deployment {
            policy_data.containers = self
                .get_policy_data(&deployment.spec.template.spec.containers)
                .await?;
        } else if let Some(controller) = &self.replication_controller {
            policy_data.containers = self
                .get_policy_data(&controller.spec.template.spec.containers)
                .await?;
        } else if let Some(pod) = &self.pod {
            policy_data.containers = self.get_policy_data(&pod.spec.containers).await?;
        } else {
            panic!("Unsupported YAML spec kind!");
        }

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

        if let Some(deployment) = &mut self.deployment {
            add_policy_annotation(
                &mut deployment.spec.template.metadata.annotations,
                &encoded_policy,
            );

            if let Some(containers) = &mut deployment.spec.template.spec.containers {
                // Remove the pause container before serializing.
                containers.remove(0);
            }

            if let Some(yaml) = &self.yaml_file {
                serde_yaml::to_writer(
                    std::fs::OpenOptions::new()
                        .write(true)
                        .truncate(true)
                        .create(true)
                        .open(yaml)
                        .map_err(|e| anyhow!(e))?,
                    &deployment,
                )?;
            } else {
                serde_yaml::to_writer(std::io::stdout(), &deployment)?;
            }
        } else if let Some(controller) = &mut self.replication_controller {
            add_policy_annotation(
                &mut controller.spec.template.metadata.annotations,
                &encoded_policy,
            );

            if let Some(containers) = &mut controller.spec.template.spec.containers {
                // Remove the pause container before serializing.
                containers.remove(0);
            }

            if let Some(yaml) = &self.yaml_file {
                serde_yaml::to_writer(
                    std::fs::OpenOptions::new()
                        .write(true)
                        .truncate(true)
                        .create(true)
                        .open(yaml)
                        .map_err(|e| anyhow!(e))?,
                    &controller,
                )?;
            } else {
                serde_yaml::to_writer(std::io::stdout(), &controller)?;
            }
        } else if let Some(pod) = &mut self.pod {
            add_policy_annotation(&mut pod.metadata.annotations, &encoded_policy);

            if let Some(containers) = &mut pod.spec.containers {
                // Remove the pause container before serializing.
                containers.remove(0);
            }

            if let Some(yaml) = &self.yaml_file {
                serde_yaml::to_writer(
                    std::fs::OpenOptions::new()
                        .write(true)
                        .truncate(true)
                        .create(true)
                        .open(yaml)
                        .map_err(|e| anyhow!(e))?,
                    &pod,
                )?;
            } else {
                serde_yaml::to_writer(std::io::stdout(), &pod)?;
            }
        }

        Ok(())
    }

    async fn get_policy_data(
        &self,
        spec_containers: &Option<Vec<pod::Container>>,
    ) -> Result<Vec<ContainerPolicy>> {
        let mut policy_containers = Vec::new();

        if let Some(containers) = spec_containers {
            for index in 0..containers.len() {
                policy_containers.push(self.get_container_policy_by_yaml_type(index).await?);
            }
        }

        Ok(policy_containers)
    }

    // Create ContainerPolicy object based on:
    // - Container image configuration, pulled from the registry.
    // - containerd default values for each container.
    // - Kata Containers namespaces.
    // - K8s infrastructure information.
    pub async fn get_container_policy_by_yaml_type(
        &self,
        container_index: usize,
    ) -> Result<ContainerPolicy> {
        if let Some(deployment) = &self.deployment {
            if let Some(containers) = &deployment.spec.template.spec.containers {
                self.get_container_policy(container_index, &containers[container_index])
                    .await
            } else {
                Err(anyhow!("No containers in Deployment pod template!"))
            }
        } else if let Some(controller) = &self.replication_controller {
            if let Some(containers) = &controller.spec.template.spec.containers {
                self.get_container_policy(container_index, &containers[container_index])
                    .await
            } else {
                Err(anyhow!("No containers in Deployment pod template!"))
            }
        } else if let Some(pod) = &self.pod {
            if let Some(containers) = &pod.spec.containers {
                self.get_container_policy(container_index, &containers[container_index])
                    .await
            } else {
                Err(anyhow!("No containers in Pod spec!"))
            }
        } else {
            panic!("Unsupported YAML spec kind!");
        }
    }

    pub async fn get_container_policy(
        &self,
        container_index: usize,
        yaml_container: &pod::Container,
    ) -> Result<ContainerPolicy> {
        let is_pause_container = container_index == 0;

        let mut pod_name = String::new();
        if let Some(deployment) = &self.deployment {
            if let Some(name) = &deployment.metadata.name {
                pod_name = name.clone();
            }
        } else if let Some(controller) = &self.replication_controller {
            if let Some(name) = &controller.metadata.name {
                pod_name = name.clone();
            }
        } else if let Some(pod) = &self.pod {
            if let Some(name) = &pod.metadata.name {
                pod_name = name.clone();
            }
        } else {
            panic!("Unsupported YAML spec kind!");
        }

        // Example: "hostname": "^busybox-cc$",
        let mut hostname = "^".to_string() + &pod_name;
        if self.deployment.is_some() {
            // Example: "hostname": "^busybox-cc-5bdd867667-xxmdz$",
            hostname += "-[a-z0-9]{10}-[a-z0-9]{5}"
        } else if self.replication_controller.is_some() {
            // Example: "hostname": "no-exist-tdtd7",
            hostname += "-[a-z0-9]{5}";
        }
        hostname += "$";

        let registry_container = registry::Container::new(&yaml_container.image).await?;
        let mut infra_container = &self.infra_policy.pause_container;
        if !is_pause_container {
            infra_container = &self.infra_policy.other_container;
        }

        let mut root: Option<Root> = None;
        if let Some(infra_root) = &infra_container.root {
            let mut policy_root = infra_root.clone();
            policy_root.readonly = yaml_container.read_only_root_filesystem();
            root = Some(policy_root);
        }

        let mut annotations = BTreeMap::new();
        infra::get_annotations(&mut annotations, infra_container)?;
        if self.pod.is_some() {
            annotations.insert(
                "io.kubernetes.cri.sandbox-name".to_string(),
                pod_name.to_string(),
            );
        }
        if !is_pause_container {
            let mut image_name = yaml_container.image.to_string();
            if image_name.find(':').is_none() {
                image_name += ":latest";
            }
            annotations.insert(
                "io.kubernetes.cri.image-name".to_string(),
                image_name,
            );
        }

        let mut namespace = "default".to_string();
        if let Some(deployment) = &self.deployment {
            if let Some(yaml_namespace) = &deployment.metadata.namespace {
                namespace = yaml_namespace.clone();
            }
        } else if let Some(pod) = &self.pod {
            if let Some(yaml_namespace) = &pod.metadata.namespace {
                namespace = yaml_namespace.clone();
            }
        }
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

        if container_index != 0 {
            process.env.push("HOSTNAME=".to_string() + &pod_name);
        }

        yaml_container.get_env_variables(&mut process.env, &self.config_maps);

        infra::get_process(&mut process, &infra_container)?;
        process.noNewPrivileges = !yaml_container.allow_privilege_escalation();

        let mut mounts = containerd::get_mounts(is_pause_container, privileged_container);
        self.infra_policy.get_policy_mounts(
            &mut mounts,
            &infra_container.mounts,
            &yaml_container,
            container_index == 0,
        )?;

        let image_layers = registry_container.get_image_layers();
        let mut storages = Default::default();
        get_image_layer_storages(&mut storages, &image_layers, &root)?;

        self.get_mounts_and_storages(
            &mut mounts,
            &mut storages,
            yaml_container,
            &self.infra_policy,
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

    fn get_mounts_and_storages(
        &self,
        policy_mounts: &mut Vec<oci::Mount>,
        storages: &mut Vec<SerializedStorage>,
        container: &pod::Container,
        infra_policy: &infra::InfraPolicy,
    ) -> Result<()> {
        if let Some(pod) = &self.pod {
            if let Some(volumes) = &pod.spec.volumes {
                for volume in volumes {
                    self.get_container_mounts_and_storages(
                        policy_mounts,
                        storages,
                        container,
                        infra_policy,
                        volume,
                    )?;
                }
            }
        }

        Ok(())
    }

    fn get_container_mounts_and_storages(
        &self,
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

fn add_policy_annotation(anno: &mut Option<BTreeMap<String, String>>, encoded_policy: &str) {
    if let Some(annotations) = anno {
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
        *anno = Some(annotations);
    }
}
