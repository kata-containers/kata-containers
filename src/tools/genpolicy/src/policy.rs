// Copyright (c) 2023 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

// Allow OCI spec field names.
#![allow(non_snake_case)]

use crate::config_map;
use crate::containerd;
use crate::mount_and_storage;
use crate::no_policy;
use crate::pod;
use crate::policy;
use crate::secret;
use crate::utils;
use crate::yaml;

use anyhow::Result;
use base64::{engine::general_purpose, Engine as _};
use log::debug;
use oci_spec::runtime as oci;
use protocols::agent;
use serde::{Deserialize, Serialize};
use serde_yaml::Value;
use std::boxed;
use std::collections::{BTreeMap, BTreeSet};
use std::fs::read_to_string;
use std::io::Write;

/// Intermediary format of policy data.
pub struct AgentPolicy {
    /// K8s resources described by the input YAML file.
    pub resources: Vec<boxed::Box<dyn yaml::K8sResource + Send + Sync>>,

    /// K8s ConfigMap resources described by an additional input YAML file
    /// or by the "main" input YAML file, containing additional pod settings.
    config_maps: Vec<config_map::ConfigMap>,

    /// K8s Secret resources, containing additional pod settings.
    secrets: Vec<secret::Secret>,

    /// Rego rules read from a file (rules.rego).
    pub rules: String,

    /// Policy settings.
    pub config: utils::Config,
}

/// Representation of the policy_data field from the output policy text.
#[derive(Debug, Serialize)]
pub struct PolicyData {
    /// Policy properties for each container allowed to be executed in a pod.
    pub containers: Vec<ContainerPolicy>,

    /// Settings read from genpolicy-settings.json.
    pub common: CommonData,

    /// Sandbox settings read from genpolicy-settings.json.
    pub sandbox: SandboxData,

    /// Settings read from genpolicy-settings.json, related directly to each
    /// kata agent endpoint, that get added to the output policy.
    pub request_defaults: RequestDefaults,
}

/// OCI Container spec. This struct is very similar to the Spec struct from
/// Kata Containers. The main difference is that the Annotations field below
/// is ordered, thus resulting in the same output policy contents every time
/// when this apps runs with the same inputs. Also, it preserves the upper
/// case field names, for consistency with the structs used by agent's rpc.rs.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct KataSpec {
    /// Version of the Open Container Initiative Runtime Specification with which the bundle complies.
    #[serde(default)]
    pub Version: String,

    /// Process configures the container process.
    #[serde(default)]
    pub Process: KataProcess,

    /// Root configures the container's root filesystem.
    pub Root: KataRoot,

    /// Mounts configures additional mounts (on top of Root).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub Mounts: Vec<KataMount>,

    /// Hooks configures callbacks for container lifecycle events.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub Hooks: Option<oci::Hooks>,

    /// Annotations contains arbitrary metadata for the container.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub Annotations: BTreeMap<String, String>,

    /// Linux is platform-specific configuration for Linux based containers.
    #[serde(default)]
    pub Linux: KataLinux,
}

/// OCI container Process struct. This struct is very similar to the Process
/// struct generated from oci.proto. The main difference is that it preserves
/// the upper case field names from oci.proto, for consistency with the structs
/// used by agent's rpc.rs.
#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq)]
pub struct KataProcess {
    /// Terminal creates an interactive terminal for the container.
    #[serde(default)]
    pub Terminal: bool,

    /// User specifies user information for the process.
    #[serde(default)]
    pub User: KataUser,

    /// Args specifies the binary and arguments for the application to execute.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub Args: Vec<String>,

    /// Env populates the process environment for the process.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub Env: Vec<String>,

    /// Cwd is the current working directory for the process and must be
    /// relative to the container's root.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub Cwd: String,

    /// Capabilities are Linux capabilities that are kept for the process.
    #[serde(default)]
    pub Capabilities: KataLinuxCapabilities,

    /// NoNewPrivileges controls whether additional privileges could be gained by processes in the container.
    #[serde(default)]
    pub NoNewPrivileges: bool,
}

/// OCI container User struct. This struct is very similar to the User
/// struct generated from oci.proto. The main difference is that it preserves
/// the upper case field names from oci.proto, for consistency with the structs
/// used by agent's rpc.rs.
#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq)]
pub struct KataUser {
    /// UID is the user id.
    pub UID: u32,

    /// GID is the group id.
    pub GID: u32,

    /// AdditionalGids are additional group ids set for the container's process.
    pub AdditionalGids: BTreeSet<u32>,

    /// Username is the user name.
    pub Username: String,
}

/// OCI container Root struct. This struct is very similar to the Root
/// struct generated from oci.proto. The main difference is that it preserves the
/// upper case field names from oci.proto, for consistency with the structs used
/// by agent's rpc.rs.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct KataRoot {
    /// Path is the absolute path to the container's root filesystem.
    pub Path: String,

    /// Readonly makes the root filesystem for the container readonly before the process is executed.
    #[serde(default)]
    pub Readonly: bool,
}

/// OCI container Linux struct. This struct is similar to the Linux struct
/// generated from oci.proto, but includes just the fields that are currently
/// relevant for automatic generation of policy.
#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq)]
pub struct KataLinux {
    /// Namespaces contains the namespaces that are created and/or joined by the container
    #[serde(default)]
    pub Namespaces: Vec<KataLinuxNamespace>,

    /// MaskedPaths masks over the provided paths inside the container.
    #[serde(default)]
    pub MaskedPaths: Vec<String>,

    /// ReadonlyPaths sets the provided paths as RO inside the container.
    #[serde(default)]
    pub ReadonlyPaths: Vec<String>,

    /// Devices contains devices to be created inside the container.
    #[serde(default)]
    pub Devices: Vec<KataLinuxDevice>,

    /// Sysctls contains sysctls to be applied inside the container.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub Sysctl: BTreeMap<String, String>,
}

/// OCI container LinuxNamespace struct. This struct is similar to the LinuxNamespace
/// struct generated from oci.proto, but includes just the fields that are currently
/// relevant for automatic generation of policy.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct KataLinuxNamespace {
    /// Type is the type of namespace
    pub Type: String,

    /// Path is a path to an existing namespace persisted on disk that can be joined
    /// and is of the same type
    pub Path: String,
}

/// OCI container LinuxDevice struct. This struct is similar to the LinuxDevice
/// struct generated from oci.proto, but includes just the fields that are currently
/// relevant for automatic generation of policy.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct KataLinuxDevice {
    /// Type is the type of device.
    pub Type: String,

    /// Path is the path where the device should be created.
    pub Path: String,
}

/// OCI container LinuxCapabilities struct. This struct is very similar to the
/// LinuxCapabilities struct generated from oci.proto. The main difference is
/// that it preserves the upper case field names from oci.proto, for consistency
/// with the structs used by agent's rpc.rs.
#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq)]
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

/// OCI container Mount struct. This struct is very similar to the Mount
/// struct generated from oci.proto. The main difference is that it preserves
/// the field names from oci.proto, for consistency with the structs used by
/// agent's rpc.rs.
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

/// Policy data for a container, included in the output of this app.
#[derive(Debug, Serialize)]
pub struct ContainerPolicy {
    /// Data compared with req.OCI for CreateContainerRequest calls.
    pub OCI: KataSpec,

    /// Data compared with req.storages for CreateContainerRequest calls.
    storages: Vec<agent::Storage>,

    /// Data compared with req.devices for CreateContainerRequest calls.
    devices: Vec<agent::Device>,

    /// Data compared with req.sandbox_pidns for CreateContainerRequest calls.
    sandbox_pidns: bool,

    /// Allow list of ommand lines that are allowed to be executed using
    /// ExecProcessRequest. By default, all ExecProcessRequest calls are blocked
    /// by the policy.
    exec_commands: Vec<Vec<String>>,
}

/// See Reference / Kubernetes API / Config and Storage Resources / Volume.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Volumes {
    /// K8s EmptyDir Volume.
    pub emptyDir: Option<EmptyDirVolume>,

    /// K8s PersistentVolumeClaim Volume.
    pub persistentVolumeClaim: Option<PersistentVolumeClaimVolume>,
}

/// See Reference / Kubernetes API / Config and Storage Resources / Volume.
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

/// See Reference / Kubernetes API / Config and Storage Resources / Volume.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PersistentVolumeClaimVolume {
    pub mount_type: String,
    pub mount_source: String,
}

/// CreateContainerRequest settings from genpolicy-settings.json.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateContainerRequestDefaults {
    /// Allow env variables that match any of these regexes.
    allow_env_regex: Vec<String>,
}

/// ExecProcessRequest settings from genpolicy-settings.json.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExecProcessRequestDefaults {
    /// Allow these commands to be executed. This field has been deprecated - use allowed_commands instead.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commands: Option<Vec<String>>,

    /// Allow these commands to be executed.
    pub allowed_commands: Vec<Vec<String>>,

    /// Allow commands matching these regexes to be executed.
    regex: Vec<String>,
}

/// UpdateRoutesRequest settings from genpolicy-settings.json.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UpdateRoutesRequestDefaults {
    /// Forbid adding routes to devices of these names.
    forbidden_device_names: Vec<String>,

    /// Forbid adding routes originating from these addresses.
    forbidden_source_regex: Vec<String>,
}

/// UpdateInterfaceRequest settings from genpolicy-settings.json.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UpdateInterfaceRequestDefaults {
    /// Raw flag bitmask explicitly allowed to configure
    allow_raw_flags: u32,

    /// Explicitly blocked interface names. Intent is to block changes to loopback interface.
    forbidden_names: Vec<String>,

    /// Explicitly blocked mac addresses. Intent is to block changes to loopback interface.
    forbidden_hw_addrs: Vec<String>,
}

/// Settings specific to each kata agent endpoint, loaded from
/// genpolicy-settings.json.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RequestDefaults {
    /// Settings for CreateContainerRequest.
    pub CreateContainerRequest: CreateContainerRequestDefaults,

    /// Guest file paths matching these regular expressions can be copied by the Host.
    pub CopyFileRequest: Vec<String>,

    /// Commands allowed to be executed by the Host in all Guest containers.
    pub ExecProcessRequest: ExecProcessRequestDefaults,

    /// Allow the host to update routes for devices other than the loopback.
    pub UpdateRoutesRequest: UpdateRoutesRequestDefaults,

    /// Allow the host to configure only used raw_flags and reject names/mac addresses of the loopback.
    pub UpdateInterfaceRequest: UpdateInterfaceRequestDefaults,

    /// Allow the Host to close stdin for a container. Typically used with WriteStreamRequest.
    pub CloseStdinRequest: bool,

    /// Allow Host reading from Guest containers stdout and stderr.
    pub ReadStreamRequest: bool,

    /// Allow Host to update Guest mounts.
    pub UpdateEphemeralMountsRequest: bool,

    /// Allow Host writing to Guest containers stdin.
    pub WriteStreamRequest: bool,
}

/// Struct used to read data from the settings file and copy that data into the policy.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CommonData {
    /// Path to the shared container files - e.g., "/run/kata-containers/shared/containers".
    pub cpath: String,

    /// Path to the container root - e.g., "/run/kata-containers/$(bundle-id)/rootfs".
    pub root_path: String,

    /// Regex prefix for shared file paths - e.g., "^$(cpath)/$(bundle-id)-[a-z0-9]{16}-".
    pub sfprefix: String,

    /// Regex for an IPv4 address.
    pub ipv4_a: String,

    /// Regex for an IP port number.
    pub ip_p: String,

    /// Regex for a K8s service name (RFC 1035), after downward API transformation.
    pub svc_name_downward_env: String,

    // Regex for a DNS label (e.g., host name).
    pub dns_label: String,

    /// Default capabilities for a non-privileged container.
    pub default_caps: Vec<String>,

    /// Default capabilities for a privileged container.
    pub privileged_caps: Vec<String>,
}

/// Configuration from "kubectl config".
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClusterConfig {
    /// Pause container image reference.
    pub pause_container_image: String,
    /// Whether or not the cluster uses the guest pull mechanism
    /// In guest pull, host can't look into layers to determine GID.
    /// See issue https://github.com/kata-containers/kata-containers/issues/11162
    pub guest_pull: bool,
}

/// Struct used to read data from the settings file and copy that data into the policy.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SandboxData {
    /// Expected value of the CreateSandboxRequest storages field.
    pub storages: Vec<agent::Storage>,
}

enum K8sEnvFromSource {
    ConfigMap(config_map::ConfigMap),
    Secret(secret::Secret),
}

impl AgentPolicy {
    pub async fn from_files(config: &utils::Config) -> Result<AgentPolicy> {
        let mut config_maps = Vec::new();
        let mut secrets = Vec::new();
        let mut resources = Vec::new();
        let yaml_contents = yaml::get_input_yaml(&config.yaml_file)?;

        for document in serde_yaml::Deserializer::from_str(&yaml_contents) {
            let doc_mapping = Value::deserialize(document)?;
            if doc_mapping != Value::Null {
                let yaml_string = serde_yaml::to_string(&doc_mapping)?;
                let silent = config.silent_unsupported_fields;
                let (mut resource, kind) = yaml::new_k8s_resource(&yaml_string, silent)?;

                // Filter out resources that don't match the runtime class name.
                if let Some(resource_runtime_name) = resource.get_runtime_class_name() {
                    if !config.runtime_class_names.is_empty()
                        && !config
                            .runtime_class_names
                            .iter()
                            .any(|prefix| resource_runtime_name.starts_with(prefix))
                    {
                        resource =
                            boxed::Box::new(no_policy::NoPolicyResource { yaml: yaml_string });
                        resources.push(resource);
                        continue;
                    }
                }

                resource.init(config, &doc_mapping, silent).await;

                // ConfigMap and Secret documents contain additional input for policy generation.
                if kind.eq("ConfigMap") {
                    let config_map: config_map::ConfigMap = serde_yaml::from_str(&yaml_string)?;
                    debug!("{:#?}", &config_map);
                    config_maps.push(config_map);
                } else if kind.eq("Secret") {
                    let secret: secret::Secret = serde_yaml::from_str(&yaml_string)?;
                    debug!("{:#?}", &secret);
                    secrets.push(secret);
                }

                // Although copies of ConfigMap and Secret resources get created above,
                // those resources still have to be present in the resources vector, because
                // the elements of this vector will eventually be used to create the output
                // YAML file.
                resources.push(resource);
            }
        }

        if let Some(config_files) = &config.config_files {
            for resource_file in config_files {
                for config_resource in parse_config_file(resource_file.to_string(), config).await? {
                    match config_resource {
                        K8sEnvFromSource::ConfigMap(config_map) => {
                            config_maps.push(config_map);
                        }
                        K8sEnvFromSource::Secret(secret) => {
                            secrets.push(secret);
                        }
                    }
                }
            }
        }

        if let Ok(rules) = read_to_string(&config.rego_rules_path) {
            Ok(AgentPolicy {
                resources,
                rules,
                config_maps,
                secrets,
                config: config.clone(),
            })
        } else {
            panic!("Cannot open file {}. Please copy it to the current directory or specify the path to it using the -p parameter.",
                &config.rego_rules_path);
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
                .write_all(yaml_string.as_bytes())
                .unwrap();
        } else {
            // When input YAML came through stdin, print the output YAML to stdout.
            std::io::stdout().write_all(yaml_string.as_bytes()).unwrap();
        }
    }

    pub fn generate_policy(&self, resource: &dyn yaml::K8sResource) -> String {
        let yaml_containers = resource.get_containers();
        let mut policy_containers = Vec::new();

        for (i, yaml_container) in yaml_containers.iter().enumerate() {
            policy_containers.push(self.get_container_policy(resource, yaml_container, i == 0));
        }

        let policy_data = policy::PolicyData {
            containers: policy_containers,
            request_defaults: self.config.settings.request_defaults.clone(),
            common: self.config.settings.common.clone(),
            sandbox: self.config.settings.sandbox.clone(),
        };

        let json_data = serde_json::to_string_pretty(&policy_data).unwrap();
        let policy = format!("{}\npolicy_data := {json_data}", &self.rules);
        if self.config.raw_out {
            std::io::stdout().write_all(policy.as_bytes()).unwrap();
        }
        general_purpose::STANDARD.encode(policy.as_bytes())
    }

    pub fn get_container_policy(
        &self,
        resource: &dyn yaml::K8sResource,
        yaml_container: &pod::Container,
        is_pause_container: bool,
    ) -> ContainerPolicy {
        let c_settings = self
            .config
            .settings
            .get_container_settings(is_pause_container);
        let mut root = c_settings.Root.clone();
        root.Readonly = yaml_container.read_only_root_filesystem();

        let namespace = resource.get_namespace().unwrap_or_default();

        let use_host_network = resource.use_host_network();
        let annotations = get_container_annotations(
            resource,
            yaml_container,
            is_pause_container,
            &namespace,
            c_settings,
            use_host_network,
        );

        let is_privileged = yaml_container.is_privileged();
        let process = self.get_container_process(
            resource,
            yaml_container,
            is_pause_container,
            &namespace,
            c_settings,
            is_privileged,
        );

        let mut mounts = containerd::get_mounts(is_pause_container, is_privileged);
        mount_and_storage::get_policy_mounts(
            &self.config.settings,
            &mut mounts,
            yaml_container,
            is_pause_container,
        );

        let mut storages = Default::default();
        resource.get_container_mounts_and_storages(
            &mut mounts,
            &mut storages,
            yaml_container,
            &self.config.settings,
        );

        let mut linux = containerd::get_linux(is_privileged);
        linux.Namespaces = get_kata_namespaces(is_pause_container, use_host_network);

        if !c_settings.Linux.MaskedPaths.is_empty() {
            linux.MaskedPaths.clone_from(&c_settings.Linux.MaskedPaths);
        }
        if !c_settings.Linux.ReadonlyPaths.is_empty() {
            linux
                .ReadonlyPaths
                .clone_from(&c_settings.Linux.ReadonlyPaths);
        }

        let sandbox_pidns = if is_pause_container {
            false
        } else {
            resource.use_sandbox_pidns()
        };
        let exec_commands = yaml_container.get_exec_commands();

        let mut devices: Vec<agent::Device> = vec![];
        if let Some(volumeDevices) = &yaml_container.volumeDevices {
            for volumeDevice in volumeDevices {
                let mut device = agent::Device::new();
                device.set_container_path(volumeDevice.devicePath.clone());
                devices.push(device);

                linux.Devices.push(KataLinuxDevice {
                    Type: "".to_string(),
                    Path: volumeDevice.devicePath.clone(),
                })
            }
        }
        for default_device in &c_settings.Linux.Devices {
            linux.Devices.push(default_device.clone())
        }

        linux.Sysctl.extend(c_settings.Linux.Sysctl.clone());
        for sysctl in resource.get_sysctls() {
            linux.Sysctl.insert(sysctl.name, sysctl.value);
        }

        ContainerPolicy {
            OCI: KataSpec {
                Version: self.config.settings.kata_config.oci_version.clone(),
                Process: process,
                Root: root,
                Mounts: mounts,
                Hooks: None,
                Annotations: annotations,
                Linux: linux,
            },
            storages,
            devices,
            sandbox_pidns,
            exec_commands,
        }
    }

    fn get_container_process(
        &self,
        resource: &dyn yaml::K8sResource,
        yaml_container: &pod::Container,
        is_pause_container: bool,
        namespace: &str,
        c_settings: &KataSpec,
        is_privileged: bool,
    ) -> KataProcess {
        // Start with the Default Unix Spec from
        // https://github.com/containerd/containerd/blob/release/1.6/oci/spec.go#L132
        let mut process = containerd::get_process(is_privileged, &self.config.settings.common);

        yaml_container.apply_capabilities(&mut process.Capabilities, &self.config.settings.common);

        let (yaml_has_command, yaml_has_args) = yaml_container.get_process_args(&mut process.Args);
        yaml_container
            .registry
            .get_process(&mut process, yaml_has_command, yaml_has_args);

        if let Some(tty) = yaml_container.tty {
            process.Terminal = tty;
            if tty && !is_pause_container {
                process.Env.push("TERM=xterm".to_string());
            }
        }

        if !is_pause_container {
            process.Env.push("HOSTNAME=$(host-name)".to_string());
        }

        let service_account_name = if let Some(s) = &yaml_container.serviceAccountName {
            s
        } else {
            "default"
        };

        yaml_container.get_env_variables(
            &mut process.Env,
            &self.config_maps,
            &self.secrets,
            namespace,
            resource.get_annotations(),
            service_account_name,
        );

        substitute_env_variables(&mut process.Env);
        substitute_args_env_variables(&mut process.Args, &process.Env);

        c_settings.get_process_fields(&mut process);
        let mut must_check_passwd = false;
        resource.get_process_fields(&mut process, &mut must_check_passwd);

        // The actual GID of the process run by the CRI
        // Depends on the contents of /etc/passwd in the container
        if must_check_passwd {
            process.User.GID = yaml_container
                .registry
                .get_gid_from_passwd_uid(process.User.UID)
                .unwrap_or(0);
        }
        yaml_container.get_process_fields(&mut process);

        // The last step containerd always does is add the User.GID to AdditionalGids
        // The sandbox path does not respect the securityContext fsGroup/supplementalGroups
        if is_pause_container {
            process.User.AdditionalGids.clear();
        }
        process.User.AdditionalGids.insert(process.User.GID);

        process
    }
}

impl KataSpec {
    fn add_annotations(&self, annotations: &mut BTreeMap<String, String>) {
        for a in &self.Annotations {
            annotations.entry(a.0.clone()).or_insert(a.1.clone());
        }
    }

    fn get_process_fields(&self, process: &mut KataProcess) {
        if process.User.UID == 0 {
            process.User.UID = self.Process.User.UID;
        }
        if process.User.GID == 0 {
            process.User.GID = self.Process.User.GID;
        }

        process.User.AdditionalGids = self.Process.User.AdditionalGids.clone();
        process.User.Username = String::from(&self.Process.User.Username);
        add_missing_strings(&self.Process.Args, &mut process.Args);

        add_missing_strings(&self.Process.Env, &mut process.Env);
    }
}

async fn parse_config_file(
    yaml_file: String,
    config: &utils::Config,
) -> Result<Vec<K8sEnvFromSource>> {
    let mut k8sRes = Vec::new();
    let yaml_contents = yaml::get_input_yaml(&Some(yaml_file))?;
    for document in serde_yaml::Deserializer::from_str(&yaml_contents) {
        let doc_mapping = Value::deserialize(document)?;
        if doc_mapping != Value::Null {
            let yaml_string = serde_yaml::to_string(&doc_mapping)?;
            let silent = config.silent_unsupported_fields;
            let (mut resource, kind) = yaml::new_k8s_resource(&yaml_string, silent)?;

            resource.init(config, &doc_mapping, silent).await;

            // ConfigMap and Secret documents contain additional input for policy generation.
            if kind.eq("ConfigMap") {
                let config_map: config_map::ConfigMap = serde_yaml::from_str(&yaml_string)?;
                debug!("{:#?}", &config_map);
                k8sRes.push(K8sEnvFromSource::ConfigMap(config_map));
            } else if kind.eq("Secret") {
                let secret: secret::Secret = serde_yaml::from_str(&yaml_string)?;
                debug!("{:#?}", &secret);
                k8sRes.push(K8sEnvFromSource::Secret(secret));
            }
        }
    }

    Ok(k8sRes)
}

fn substitute_env_variables(env: &mut Vec<String>) {
    loop {
        let mut substituted = false;

        for i in 0..env.len() {
            let components: Vec<&str> = env[i].split('=').collect();
            if components.len() == 2 {
                if let Some((start, end)) = find_subst_target(components[1]) {
                    if let Some(new_value) = substitute_variable(components[1], start, end, env) {
                        let new_var = format!("{}={new_value}", &components[0]);
                        debug!("Replacing env variable <{}> with <{new_var}>", &env[i]);
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
            if let Some(end) = env_value[start..].find(')') {
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
    // Variables generated by this application.
    let internal_vars = [
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
                    let from = format!("$({name})");
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

fn get_container_annotations(
    resource: &dyn yaml::K8sResource,
    yaml_container: &pod::Container,
    is_pause_container: bool,
    namespace: &str,
    c_settings: &KataSpec,
    use_host_network: bool,
) -> BTreeMap<String, String> {
    let mut annotations = if let Some(a) = resource.get_annotations() {
        let mut a_cloned = a.clone();
        yaml::remove_policy_annotation(&mut a_cloned);
        a_cloned
    } else {
        BTreeMap::new()
    };

    c_settings.add_annotations(&mut annotations);

    if let Some(name) = resource.get_sandbox_name() {
        annotations
            .entry("io.kubernetes.cri.sandbox-name".to_string())
            .or_insert(name);
    }

    if !is_pause_container {
        let mut image_name = yaml_container.image.clone();
        if image_name.find(':').is_none() {
            image_name += ":latest";
        }
        annotations
            .entry("io.kubernetes.cri.image-name".to_string())
            .or_insert(image_name);
    }

    annotations.insert(
        "io.kubernetes.cri.sandbox-namespace".to_string(),
        namespace.to_string(),
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

    annotations
}

fn add_missing_strings(src: &Vec<String>, dest: &mut Vec<String>) {
    for src_string in src {
        if !dest.contains(src_string) {
            dest.push(src_string.clone());
        }
    }
    debug!("src = {:?}, dest = {:?}", src, dest)
}

pub fn get_kata_namespaces(
    is_pause_container: bool,
    use_host_network: bool,
) -> Vec<KataLinuxNamespace> {
    let mut namespaces: Vec<KataLinuxNamespace> = vec![KataLinuxNamespace {
        Type: "ipc".to_string(),
        Path: "".to_string(),
    }];

    if !is_pause_container || !use_host_network {
        namespaces.push(KataLinuxNamespace {
            Type: "uts".to_string(),
            Path: "".to_string(),
        });
    }

    namespaces.push(KataLinuxNamespace {
        Type: "mount".to_string(),
        Path: "".to_string(),
    });

    namespaces
}
