// Copyright (c) 2023 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

// Allow OCI spec field names.
#![allow(non_snake_case)]

use crate::config_map;
use crate::containerd;
use crate::infra;
use crate::kata;
use crate::pod;
use crate::policy;
use crate::registry;
use crate::secret;
use crate::utils;
use crate::volume;
use crate::yaml;

use anyhow::{anyhow, Result};
use base64::{engine::general_purpose, Engine as _};
use log::debug;
use oci::*;
use serde::{Deserialize, Serialize};
use serde_yaml::Value;
use sha2::{Digest, Sha256};
use std::boxed;
use std::collections::BTreeMap;
use std::fs::read_to_string;
use std::io::Write;

pub struct AgentPolicy {
    resources: Vec<boxed::Box<dyn yaml::K8sResource + Send + Sync>>,
    config_maps: Vec<config_map::ConfigMap>,
    secrets: Vec<secret::Secret>,
    pub rules: String,
    pub infra_policy: infra::InfraPolicy,
    pub config: utils::Config,
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
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct OciSpec {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ociVersion: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub process: Option<OciProcess>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub root: Option<Root>,

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
    exec_commands: Vec<String>,
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
    pub async fn from_files(config: &utils::Config) -> Result<AgentPolicy> {
        let mut config_maps = Vec::new();
        let mut secrets = Vec::new();
        let mut resources = Vec::new();
        let yaml_contents = yaml::get_input_yaml(&config.yaml_file)?;

        for document in serde_yaml::Deserializer::from_str(&yaml_contents) {
            let doc_mapping = Value::deserialize(document)?;
            let yaml_string = serde_yaml::to_string(&doc_mapping)?;
            let (mut resource, kind) =
                yaml::new_k8s_resource(&yaml_string, config.silent_unsupported_fields)?;
            resource
                .init(
                    config.use_cache,
                    &doc_mapping,
                    config.silent_unsupported_fields,
                )
                .await;
            resources.push(resource);

            if kind.eq("ConfigMap") {
                let config_map: config_map::ConfigMap = serde_yaml::from_str(&yaml_string)?;
                debug!("{:#?}", &config_map);
                config_maps.push(config_map);
            } else if kind.eq("Secret") {
                let secret: secret::Secret = serde_yaml::from_str(&yaml_string)?;
                debug!("{:#?}", &secret);
                secrets.push(secret);
            }
        }

        let infra_policy = infra::InfraPolicy::new(&config.infra_data_file)?;

        if let Some(config_map_files) = &config.config_map_files {
            for file in config_map_files {
                config_maps.push(config_map::ConfigMap::new(&file)?);
            }
        }

        if let Ok(rules) = read_to_string(&config.rules_file) {
            Ok(AgentPolicy {
                resources,
                rules,
                infra_policy,
                config_maps,
                secrets,
                config: config.clone(),
            })
        } else {
            panic!("Cannot open file {}. Please copy it to the current directory or specify the path to it using the -i parameter.",
                &config.rules_file);
        }
    }

    pub fn export_policy(&mut self) {
        let mut yaml_string = String::new();
        for i in 0..self.resources.len() {
            let policy = self.resources[i].generate_policy(self);
            yaml_string += &self.resources[i].serialize(&policy);
        }

        if let Some(yaml_file) = &self.config.yaml_file {
            std::fs::OpenOptions::new()
                .write(true)
                .truncate(true)
                .create(true)
                .open(yaml_file)
                .unwrap()
                .write_all(&yaml_string.as_bytes())
                .unwrap();
        } else {
            std::io::stdout()
                .write_all(&yaml_string.as_bytes())
                .unwrap();
        }
    }

    pub fn generate_policy(&self, resource: &dyn yaml::K8sResource) -> String {
        let yaml_containers = resource.get_containers();
        let mut policy_containers = Vec::new();

        for i in 0..yaml_containers.len() {
            policy_containers.push(self.get_container_policy(
                resource,
                &yaml_containers[i],
                i == 0,
                resource.use_host_network(),
            ));
        }

        let policy_data = policy::PolicyData {
            containers: policy_containers,
        };

        let json_data = serde_json::to_string_pretty(&policy_data).unwrap();
        let policy = self.rules.clone() + "\npolicy_data := " + &json_data;
        if let Some(file_name) = &self.config.output_policy_file {
            policy::export_decoded_policy(&policy, &file_name);
        }
        general_purpose::STANDARD.encode(policy.as_bytes())
    }

    pub fn get_container_policy(
        &self,
        resource: &dyn yaml::K8sResource,
        yaml_container: &pod::Container,
        is_pause_container: bool,
        use_host_network: bool,
    ) -> ContainerPolicy {
        let infra_container = if is_pause_container {
            &self.infra_policy.pause_container
        } else {
            &self.infra_policy.other_container
        };

        let mut root: Option<Root> = None;
        if let Some(infra_root) = &infra_container.root {
            let mut policy_root = infra_root.clone();
            policy_root.readonly = yaml_container.read_only_root_filesystem();
            root = Some(policy_root);
        }

        let mut annotations = BTreeMap::new();
        infra::get_annotations(&mut annotations, infra_container);
        if let Some(name) = resource.get_sandbox_name() {
            annotations.insert("io.kubernetes.cri.sandbox-name".to_string(), name);
        }

        if !is_pause_container {
            let mut image_name = yaml_container.image.to_string();
            if image_name.find(':').is_none() {
                image_name += ":latest";
            }
            annotations.insert("io.kubernetes.cri.image-name".to_string(), image_name);
        }

        let namespace = resource.get_namespace();
        annotations.insert(
            "io.kubernetes.cri.sandbox-namespace".to_string(),
            namespace.clone(),
        );

        if !yaml_container.name.is_empty() {
            annotations.insert(
                "io.kubernetes.cri.container-name".to_string(),
                yaml_container.name.to_string(),
            );
        }

        if is_pause_container {
            let mut network_namespace = "^/var/run/netns/cni".to_string();
            if use_host_network {
                network_namespace += "test";
            }
            network_namespace += "-[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$";
            annotations.insert("nerdctl/network-namespace".to_string(), network_namespace);
        }

        // Start with the Default Unix Spec from
        // https://github.com/containerd/containerd/blob/release/1.6/oci/spec.go#L132
        let is_privileged = yaml_container.is_privileged();
        let mut process = containerd::get_process(is_privileged);

        if let Some(capabilities) = &mut process.capabilities {
            yaml_container.apply_capabilities(capabilities);
        }

        let (yaml_has_command, yaml_has_args) = yaml_container.get_process_args(&mut process.args);
        yaml_container
            .registry
            .get_process(&mut process, yaml_has_command, yaml_has_args);

        if !is_pause_container {
            process.env.push("HOSTNAME=".to_string() + "$(host-name)");
        }

        let service_account_name = if let Some(s) = &yaml_container.serviceAccountName {
            s.clone()
        } else {
            "default".to_string()
        };

        yaml_container.get_env_variables(
            &mut process.env,
            &self.config_maps,
            &self.secrets,
            &namespace,
            &resource.get_annotations(),
            &service_account_name,
        );

        substitute_env_variables(&mut process.env);
        substitute_args_env_variables(&mut process.args, &process.env);

        infra::get_process(&mut process, &infra_container);
        process.noNewPrivileges = !yaml_container.allow_privilege_escalation();

        let mut mounts = containerd::get_mounts(is_pause_container, is_privileged);
        self.infra_policy.get_policy_mounts(
            &mut mounts,
            &infra_container.mounts,
            yaml_container,
            is_pause_container,
        );

        let image_layers = yaml_container.registry.get_image_layers();
        let mut storages = Default::default();
        get_image_layer_storages(&mut storages, &image_layers, &root);
        resource.get_container_mounts_and_storages(
            &mut mounts,
            &mut storages,
            yaml_container,
            self,
        );

        let mut linux = containerd::get_linux(is_privileged);
        linux.namespaces = kata::get_namespaces(is_pause_container, use_host_network);
        infra::get_linux(&mut linux, &infra_container.linux);

        let exec_commands = yaml_container.get_exec_commands();

        ContainerPolicy {
            oci: OciSpec {
                ociVersion: Some("1.1.0-rc.1".to_string()),
                process: Some(process),
                root,
                mounts,
                hooks: None,
                annotations: Some(annotations),
                linux: Some(linux),
            },
            storages,
            exec_commands,
        }
    }

    pub fn get_container_mounts_and_storages(
        &self,
        policy_mounts: &mut Vec<oci::Mount>,
        storages: &mut Vec<SerializedStorage>,
        container: &pod::Container,
        volume: &volume::Volume,
    ) {
        if let Some(volume_mounts) = &container.volumeMounts {
            for volume_mount in volume_mounts {
                if volume_mount.name.eq(&volume.name) {
                    self.infra_policy.get_mount_and_storage(
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

fn get_image_layer_storages(
    storages: &mut Vec<SerializedStorage>,
    image_layers: &Vec<registry::ImageLayer>,
    root: &Option<Root>,
) {
    if let Some(root_mount) = root {
        let mut new_storages: Vec<SerializedStorage> = Vec::new();

        let mut overlay_storage = SerializedStorage {
            driver: "overlayfs".to_string(),
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

        let mut lowerdirs: Vec<String> = Vec::new();
        let mut previous_chain_id = String::new();

        for layer in image_layers {
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

            let options = vec![
                "ro".to_string(),
                "io.katacontainers.fs-opt.block_device=file".to_string(),
                "io.katacontainers.fs-opt.is-layer".to_string(),
                "io.katacontainers.fs-opt.root-hash=".to_string() + &layer.verity_hash,
            ];
            let layer_name = name_to_hash(&chain_id);

            new_storages.push(SerializedStorage {
                driver: "blk".to_string(),
                driver_options: Vec::new(),
                source: String::new(), // TODO
                fstype: "tar".to_string(),
                options,
                mount_point: layers_path.clone() + &layer_name,
                fs_group: SerializedFsGroup {
                    group_id: 0,
                    group_change_policy: 0,
                },
            });

            let mut fs_opt_layer = "io.katacontainers.fs-opt.layer=".to_string();
            fs_opt_layer += &layer_name;
            fs_opt_layer += ",tar,ro,io.katacontainers.fs-opt.block_device=file,io.katacontainers.fs-opt.is-layer,io.katacontainers.fs-opt.root-hash=";
            fs_opt_layer += &layer.verity_hash;
            overlay_storage.options.push(fs_opt_layer);

            lowerdirs.push(layer_name);
        }

        new_storages.reverse();
        for storage in new_storages {
            storages.push(storage);
        }

        overlay_storage.options.reverse();
        overlay_storage.options.insert(0,
            "io.katacontainers.fs-opt.layer-src-prefix=/var/lib/containerd/io.containerd.snapshotter.v1.tardev/layers".to_string()
        );
        overlay_storage
            .options
            .push("io.katacontainers.fs-opt.overlay-rw".to_string());

        lowerdirs.reverse();
        overlay_storage
            .options
            .push("lowerdir=".to_string() + &lowerdirs.join(":"));

        storages.push(overlay_storage);
    }
}

// TODO: avoid copying this code from snapshotter.rs
/// Converts the given name to a string representation of its sha256 hash.
fn name_to_hash(name: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(name);
    format!("{:x}", hasher.finalize())
}

/// Creates a text file including the Rego rules and data.
pub fn export_decoded_policy(policy: &str, file_name: &str) {
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .truncate(true)
        .create(true)
        .open(file_name)
        .map_err(|e| anyhow!(e))
        .unwrap();
    f.write_all(policy.as_bytes()).unwrap();
    f.flush().map_err(|e| anyhow!(e)).unwrap();
}

fn substitute_env_variables(env: &mut Vec<String>) {
    loop {
        let mut substituted = false;

        for i in 0..env.len() {
            let env_var = env[i].clone();
            let components: Vec<&str> = env_var.split('=').collect();
            if components.len() == 2 {
                if let Some((start, end)) = find_subst_target(&components[1]) {
                    if let Some(new_value) = substitute_variable(&components[1], start, end, env) {
                        let new_var = components[0].to_string() + "=" + &new_value;
                        debug!("Replacing env variable <{}> with <{}>", &env[i], &new_var);
                        env[i] = new_var;
                        substituted = true;
                    }
                }
            }
        }

        if !substituted {
            break;
        }
    }
}

fn find_subst_target(env_value: &str) -> Option<(usize, usize)> {
    if let Some(mut start) = env_value.find("$(") {
        start += 2;
        if env_value.len() > start {
            if let Some(end) = env_value[start..].find(")") {
                return Some((start, start + end));
            }
        }
    }

    None
}

fn substitute_variable(
    env_var: &str,
    name_start: usize,
    name_end: usize,
    env: &Vec<String>,
) -> Option<String> {
    let internal_vars = vec![
        "bundle-id",
        "host-ip",
        "node-name",
        "pod-ip",
        "pod-uid",
        "sandbox-id",
        "sandbox-name",
        "sandbox-namespace",
    ];

    assert!(name_start < name_end);
    assert!(name_end < env_var.len());
    let name = env_var[name_start..name_end].to_string();
    debug!("Searching for the value of <{}>", &name);

    for other_var in env {
        let components: Vec<&str> = other_var.split('=').collect();
        if components[0].eq(&name) {
            debug!("Found {} in <{}>", &name, &other_var);
            if components.len() == 2 {
                let mut replace = true;
                let value = &components[1];

                if let Some((start, end)) = find_subst_target(value) {
                    if internal_vars.contains(&&value[start..end]) {
                        // Variables used internally for Policy don't get expanded
                        // in the current design, so it's OK to use them as replacement
                        // in other env variables or command arguments.
                    } else {
                        // Don't substitute if the value includes variables to be
                        // substituted, to avoid circular substitutions.
                        replace = false;
                    }
                }

                if replace {
                    let from = "$(".to_string() + &name + ")";
                    return Some(env_var.replace(&from, value));
                }
            }
        }
    }

    None
}

fn substitute_args_env_variables(args: &mut Vec<String>, env: &Vec<String>) {
    for arg in args {
        substitute_arg_env_variables(arg, env);
    }
}

fn substitute_arg_env_variables(arg: &mut String, env: &Vec<String>) {
    loop {
        let mut substituted = false;

        if let Some((start, end)) = find_subst_target(arg) {
            if let Some(new_value) = substitute_variable(arg, start, end, env) {
                debug!(
                    "substitute_arg_env_variables: replacing {} with {}",
                    &arg[start..end],
                    &new_value
                );
                *arg = new_value;
                substituted = true;
            }
        }

        if !substituted {
            break;
        }
    }
}
