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

use anyhow::Result;
use base64::{engine::general_purpose, Engine as _};
use log::debug;
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
    pub request_defaults: RequestDefaults,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct KataSpec {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub Version: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub Process: Option<KataProcess>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub Root: Option<KataRoot>,

    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub Mounts: Vec<KataMount>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub Hooks: Option<oci::Hooks>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub Annotations: Option<BTreeMap<String, String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub Linux: Option<KataLinux>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct KataProcess {
    pub Terminal: bool,
    pub User: KataUser,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub Args: Vec<String>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub Env: Vec<String>,

    #[serde(skip_serializing_if = "String::is_empty")]
    pub Cwd: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub Capabilities: Option<KataLinuxCapabilities>,

    pub NoNewPrivileges: bool,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq)]
pub struct KataUser {
    /// UID is the user id.
    pub UID: u32,

    /// GID is the group id.
    pub GID: u32,

    /// AdditionalGids are additional group ids set for the container's process.
    pub AdditionalGids: Vec<u32>,

    /// Username is the user name.
    pub Username: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct KataRoot {
    /// Path is the absolute path to the container's root filesystem.
    pub Path: String,

    /// Readonly makes the root filesystem for the container readonly before the process is executed.
    #[serde(default)]
    pub Readonly: bool,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq)]
pub struct KataLinux {
    // UIDMapping specifies user mappings for supporting user namespaces.
    // UIDMappings: Vec<KataLinuxIDMapping>,

    // GIDMapping specifies group mappings for supporting user namespaces.
    // GIDMappings: Vec<KataLinuxIDMapping>,

    // Sysctl are a set of key value pairs that are set for the container on start
    // Sysctl: BTreeMap<String, String>,

    // Resources contain cgroup information for handling resource constraints
    // for the container
    // LinuxResources Resources = 4;
    // CgroupsPath specifies the path to cgroups that are created and/or joined by the container.
    // The path is expected to be relative to the cgroups mountpoint.
    // If resources are specified, the cgroups at CgroupsPath will be updated based on resources.
    // CgroupsPath: String,

    /// Namespaces contains the namespaces that are created and/or joined by the container
    pub Namespaces: Vec<KataLinuxNamespace>,

    // Devices are a list of device nodes that are created for the container
    // repeated LinuxDevice Devices = 7  [(gogoproto.nullable) = false];

    // Seccomp specifies the seccomp security settings for the container.
    // LinuxSeccomp Seccomp = 8;

    // RootfsPropagation is the rootfs mount propagation mode for the container.
    // string RootfsPropagation = 9;
    /// MaskedPaths masks over the provided paths inside the container.
    pub MaskedPaths: Vec<String>,

    /// ReadonlyPaths sets the provided paths as RO inside the container.
    pub ReadonlyPaths: Vec<String>,
    // MountLabel specifies the selinux context for the mounts in the container.
    // string MountLabel = 12;

    // IntelRdt contains Intel Resource Director Technology (RDT) information
    // for handling resource constraints (e.g., L3 cache) for the container
    // LinuxIntelRdt IntelRdt = 13;
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct KataLinuxIDMapping {
    /// HostID is the starting UID/GID on the host to be mapped to 'ContainerID'
    HostID: u32,

    /// ContainerID is the starting UID/GID in the container
    ContainerID: u32,

    /// Size is the number of IDs to be mapped
    Size: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct KataLinuxNamespace {
    /// Type is the type of namespace
    pub Type: String,

    /// Path is a path to an existing namespace persisted on disk that can be joined
    /// and is of the same type
    pub Path: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct KataLinuxCapabilities {
	// Ambient is the ambient set of capabilities that are kept.
	pub Ambient: Vec<String>,

    /// Bounding is the set of capabilities checked by the kernel.
	pub Bounding: Vec<String>,

	/// Effective is the set of capabilities checked by the kernel.
	pub Effective: Vec<String>,

	/// Inheritable is the capabilities preserved across execve.
	pub Inheritable: Vec<String>,

	/// Permitted is the limiting superset for effective capabilities.
	pub Permitted: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct KataMount {
	/// destination is the path inside the container expect when it starts with "tmp:/"
	pub destination: String,

	/// source is the path inside the container expect when it starts with "vm:/dev/" or "tmp:/"
	/// the path which starts with "vm:/dev/" refers the guest vm's "/dev",
	/// especially, "vm:/dev/hostfs/" refers to the shared filesystem.
	/// "tmp:/" is a temporary directory which is used for temporary mounts.
    #[serde(default)]
	pub source: String,

    pub type_: String,
	pub options: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ContainerPolicy {
    pub OCI: KataSpec,
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
    pub fs_group: Option<SerializedFsGroup>,
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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateContainerRequestDefaults {
    /// Allow env variables that match any of these regexes.
    allow_env_regex: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RequestDefaults {
    pub CreateContainerRequest: CreateContainerRequestDefaults,

    /// Guest file paths matching these regular expressions can be copied by the Host.
    pub CopyFileRequest: Vec<String>,

    /// Array of commands allowed to be executed by the Host in all Guest containers.
    pub ExecProcessRequest: Vec<String>,

    /// Allow Host reading from Guest containers stdout and stderr.
    pub ReadStreamRequest: bool,

    /// Allow Host writing to Guest containers stdin.
    pub WriteStreamRequest: bool,
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
            if self.config.base64_out {
                println!("{}", policy);
            }
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
            request_defaults: self.infra_policy.request_defaults.clone(),
        };

        let json_data = serde_json::to_string_pretty(&policy_data).unwrap();
        let policy = self.rules.clone() + "\npolicy_data := " + &json_data;
        if self.config.raw_out {
            policy::base64_out(&policy);
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

        let mut root: Option<KataRoot> = None;
        if let Some(infra_root) = &infra_container.Root {
            let mut policy_root = infra_root.clone();
            policy_root.Readonly = yaml_container.read_only_root_filesystem();
            root = Some(policy_root);
        }

        let mut annotations = if let Some(mut a) = resource.get_annotations() {
            yaml::remove_policy_annotation(&mut a);
            a
        } else {
            BTreeMap::new()
        };
        infra::add_annotations(&mut annotations, infra_container);
        if let Some(name) = resource.get_sandbox_name() {
            annotations
                .entry("io.kubernetes.cri.sandbox-name".to_string())
                .or_insert(name);
        }

        if !is_pause_container {
            let mut image_name = yaml_container.image.to_string();
            if image_name.find(':').is_none() {
                image_name += ":latest";
            }
            annotations
                .entry("io.kubernetes.cri.image-name".to_string())
                .or_insert(image_name);
        }

        let namespace = resource.get_namespace();
        annotations.insert(
            "io.kubernetes.cri.sandbox-namespace".to_string(),
            namespace.clone(),
        );

        if !yaml_container.name.is_empty() {
            annotations
                .entry("io.kubernetes.cri.container-name".to_string())
                .or_insert(yaml_container.name.clone());
        }

        if is_pause_container {
            let mut network_namespace = "^/var/run/netns/cni".to_string();
            if use_host_network {
                network_namespace += "test";
            }
            network_namespace += "-[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$";
            annotations
                .entry("nerdctl/network-namespace".to_string())
                .or_insert(network_namespace);
        }

        // Start with the Default Unix Spec from
        // https://github.com/containerd/containerd/blob/release/1.6/oci/spec.go#L132
        let is_privileged = yaml_container.is_privileged();
        let mut process = containerd::get_process(is_privileged);

        if let Some(capabilities) = &mut process.Capabilities {
            yaml_container.apply_capabilities(capabilities);
        }

        let (yaml_has_command, yaml_has_args) = yaml_container.get_process_args(&mut process.Args);
        yaml_container
            .registry
            .get_process(&mut process, yaml_has_command, yaml_has_args);

        if let Some(tty) = yaml_container.tty {
            process.Terminal = tty;
            if tty && !is_pause_container {
                process.Env.push("TERM=".to_string() + "xterm");
            }
        }

        if !is_pause_container {
            process.Env.push("HOSTNAME=".to_string() + "$(host-name)");
        }

        let service_account_name = if let Some(s) = &yaml_container.serviceAccountName {
            s.clone()
        } else {
            "default".to_string()
        };

        yaml_container.get_env_variables(
            &mut process.Env,
            &self.config_maps,
            &self.secrets,
            &namespace,
            &resource.get_annotations(),
            &service_account_name,
        );

        substitute_env_variables(&mut process.Env);
        substitute_args_env_variables(&mut process.Args, &process.Env);

        infra::get_process(&mut process, &infra_container);
        process.NoNewPrivileges = !yaml_container.allow_privilege_escalation();

        let mut mounts = containerd::get_mounts(is_pause_container, is_privileged);
        self.infra_policy.get_policy_mounts(
            &mut mounts,
            &infra_container.Mounts,
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
        linux.Namespaces = kata::get_namespaces(is_pause_container, use_host_network);
        infra::get_linux(&mut linux, &infra_container.Linux);

        let exec_commands = yaml_container.get_exec_commands();

        ContainerPolicy {
            OCI: KataSpec {
                Version: Some("1.1.0-rc.1".to_string()),
                Process: Some(process),
                Root: root,
                Mounts: mounts,
                Hooks: None,
                Annotations: Some(annotations),
                Linux: Some(linux),
            },
            storages,
            exec_commands,
        }
    }

    pub fn get_container_mounts_and_storages(
        &self,
        policy_mounts: &mut Vec<policy::KataMount>,
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
    root: &Option<KataRoot>,
) {
    if let Some(root_mount) = root {
        let mut new_storages: Vec<SerializedStorage> = Vec::new();

        let mut overlay_storage = SerializedStorage {
            driver: "overlayfs".to_string(),
            driver_options: Vec::new(),
            source: String::new(), // TODO
            fstype: "tar-overlay".to_string(),
            options: Vec::new(),
            mount_point: root_mount.Path.clone(),
            fs_group: None,
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
                fs_group: None,
            });

            let mut layer_info = format!(
                "{layer_name},tar,ro,io.katacontainers.fs-opt.block_device=file,\
                io.katacontainers.fs-opt.is-layer,io.katacontainers.fs-opt.root-hash="
            );
            layer_info += &layer.verity_hash;
            let encoded_info = general_purpose::STANDARD.encode(layer_info.as_bytes());
            overlay_storage
                .options
                .push(format!("io.katacontainers.fs-opt.layer={encoded_info}"));

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

pub fn base64_out(policy: &str) {
    std::io::stdout().write_all(policy.as_bytes()).unwrap();
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
