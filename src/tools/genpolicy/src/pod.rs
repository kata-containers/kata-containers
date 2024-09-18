// Copyright (c) 2023 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

// Allow K8s YAML field names.
#![allow(non_snake_case)]

use crate::config_map;
use crate::obj_meta;
use crate::policy;
use crate::registry;
use crate::secret;
use crate::settings;
use crate::utils::Config;
use crate::volume;
use crate::yaml;

use async_trait::async_trait;
use log::{debug, warn};
use protocols::agent;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// See Reference / Kubernetes API / Workload Resources / Pod.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Pod {
    apiVersion: String,
    kind: String,
    pub metadata: obj_meta::ObjectMeta,
    pub spec: PodSpec,

    #[serde(skip)]
    doc_mapping: serde_yaml::Value,
}

/// See Reference / Kubernetes API / Workload Resources / Pod.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PodSpec {
    pub containers: Vec<Container>,

    #[serde(skip_serializing_if = "Option::is_none")]
    nodeSelector: Option<BTreeMap<String, String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    restartPolicy: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtimeClassName: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub initContainers: Option<Vec<Container>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    imagePullSecrets: Option<Vec<LocalObjectReference>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    affinity: Option<Affinity>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub volumes: Option<Vec<volume::Volume>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    nodeName: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    serviceAccountName: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    serviceAccount: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    terminationGracePeriodSeconds: Option<i64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    tolerations: Option<Vec<Toleration>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    hostname: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub hostNetwork: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub shareProcessNamespace: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    dnsConfig: Option<PodDNSConfig>,

    #[serde(skip_serializing_if = "Option::is_none")]
    dnsPolicy: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    topologySpreadConstraints: Option<Vec<TopologySpreadConstraint>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub securityContext: Option<PodSecurityContext>,

    #[serde(skip_serializing_if = "Option::is_none")]
    priorityClassName: Option<String>,
}

/// See Reference / Kubernetes API / Workload Resources / Pod.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Container {
    /// Container image registry information.
    #[serde(skip)]
    pub registry: registry::Container,

    pub name: String,
    pub image: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    imagePullPolicy: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    securityContext: Option<SecurityContext>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub volumeMounts: Option<Vec<VolumeMount>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub volumeDevices: Option<Vec<VolumeDevice>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    env: Option<Vec<EnvVar>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    envFrom: Option<Vec<EnvFromSource>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    resources: Option<ResourceRequirements>,

    #[serde(skip_serializing_if = "Option::is_none")]
    ports: Option<Vec<ContainerPort>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub args: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    lifecycle: Option<Lifecycle>,

    #[serde(skip_serializing_if = "Option::is_none")]
    livenessProbe: Option<Probe>,

    #[serde(skip_serializing_if = "Option::is_none")]
    readinessProbe: Option<Probe>,

    #[serde(skip_serializing_if = "Option::is_none")]
    startupProbe: Option<Probe>,

    #[serde(skip_serializing_if = "Option::is_none")]
    restartPolicy: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub serviceAccountName: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    stdin: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub tty: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminationMessagePath: Option<String>,
}

/// See Reference / Kubernetes API / Workload Resources / Pod.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Affinity {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub podAntiAffinity: Option<PodAntiAffinity>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub podAffinity: Option<PodAffinity>,
    // TODO: additional fields.
}

/// See Reference / Kubernetes API / Workload Resources / Pod.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct PodAffinity {
    #[serde(skip_serializing_if = "Option::is_none")]
    requiredDuringSchedulingIgnoredDuringExecution: Option<Vec<PodAffinityTerm>>,
}

/// See Reference / Kubernetes API / Workload Resources / Pod.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct PodAntiAffinity {
    #[serde(skip_serializing_if = "Option::is_none")]
    preferredDuringSchedulingIgnoredDuringExecution: Option<Vec<WeightedPodAffinityTerm>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    requiredDuringSchedulingIgnoredDuringExecution: Option<Vec<PodAffinityTerm>>,
    // TODO: additional fields.
}

/// See Reference / Kubernetes API / Workload Resources / Pod.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct WeightedPodAffinityTerm {
    weight: i32,
    podAffinityTerm: PodAffinityTerm,
}

/// See Reference / Kubernetes API / Workload Resources / Pod.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct PodAffinityTerm {
    topologyKey: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    labelSelector: Option<yaml::LabelSelector>,
    // TODO: additional fields.
}

/// See Reference / Kubernetes API / Workload Resources / Pod.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Probe {
    #[serde(skip_serializing_if = "Option::is_none")]
    exec: Option<ExecAction>,

    #[serde(skip_serializing_if = "Option::is_none")]
    initialDelaySeconds: Option<i32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    timeoutSeconds: Option<i32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    periodSeconds: Option<i32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    failureThreshold: Option<i32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    successThreshold: Option<i32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    httpGet: Option<HTTPGetAction>,

    #[serde(skip_serializing_if = "Option::is_none")]
    tcpSocket: Option<TCPSocketAction>,
    // TODO: additional fields.
}

/// See Reference / Kubernetes API / Workload Resources / Pod.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct TCPSocketAction {
    port: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    host: Option<String>,
}

/// See Reference / Kubernetes API / Workload Resources / Pod.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct HTTPGetAction {
    port: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    host: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    scheme: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    httpHeaders: Option<Vec<HTTPHeader>>,
    // TODO: additional fields.
}

/// See Reference / Kubernetes API / Workload Resources / Pod.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct HTTPHeader {
    name: String,
    value: String,
}

/// See Reference / Kubernetes API / Workload Resources / Pod.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct SecurityContext {
    #[serde(skip_serializing_if = "Option::is_none")]
    readOnlyRootFilesystem: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    allowPrivilegeEscalation: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    privileged: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    capabilities: Option<Capabilities>,

    #[serde(skip_serializing_if = "Option::is_none")]
    runAsUser: Option<i64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    seccompProfile: Option<SeccompProfile>,
}

/// See Reference / Kubernetes API / Workload Resources / Pod.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct SeccompProfile {
    #[serde(rename = "type")]
    profile_type: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    localhostProfile: Option<String>,
}

/// See Reference / Kubernetes API / Workload Resources / Pod.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PodSecurityContext {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runAsUser: Option<i64>,
    // TODO: additional fields.
}

/// See Reference / Kubernetes API / Workload Resources / Pod.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Lifecycle {
    #[serde(skip_serializing_if = "Option::is_none")]
    postStart: Option<LifecycleHandler>,

    #[serde(skip_serializing_if = "Option::is_none")]
    preStop: Option<LifecycleHandler>,
}

/// See Reference / Kubernetes API / Workload Resources / Pod.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct LifecycleHandler {
    #[serde(skip_serializing_if = "Option::is_none")]
    exec: Option<ExecAction>,
    // TODO: additional fields.
}

/// See Reference / Kubernetes API / Workload Resources / Pod.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct ExecAction {
    command: Vec<String>,
}

/// See Reference / Kubernetes API / Workload Resources / Pod.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Capabilities {
    #[serde(skip_serializing_if = "Option::is_none")]
    add: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    drop: Option<Vec<String>>,
}

/// See Reference / Kubernetes API / Workload Resources / Pod.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct ContainerPort {
    containerPort: i32,

    #[serde(skip_serializing_if = "Option::is_none")]
    hostIP: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    hostPort: Option<i32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    protocol: Option<String>,
}

/// See Reference / Kubernetes API / Workload Resources / Pod.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct EnvVar {
    name: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    value: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    valueFrom: Option<EnvVarSource>,
}

/// See Reference / Kubernetes API / Workload Resources / Pod.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EnvVarSource {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub configMapKeyRef: Option<ConfigMapKeySelector>,

    #[serde(skip_serializing_if = "Option::is_none")]
    fieldRef: Option<ObjectFieldSelector>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub secretKeyRef: Option<SecretKeySelector>,

    #[serde(skip_serializing_if = "Option::is_none")]
    resourceFieldRef: Option<ResourceFieldSelector>,
}

/// See Reference / Kubernetes API / Workload Resources / Pod.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SecretKeySelector {
    pub key: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    optional: Option<bool>,
}

/// See Reference / Kubernetes API / Workload Resources / Pod.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConfigMapKeySelector {
    pub key: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    optional: Option<bool>,
}

/// See Reference / Kubernetes API / Workload Resources / Pod.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EnvFromSource {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub configMapRef: Option<ConfigMapEnvSource>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub secretRef: Option<SecretEnvSource>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefix: Option<String>,
}

/// See Reference / Kubernetes API / Workload Resources / Pod.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SecretEnvSource {
    pub name: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    optional: Option<bool>,
}

/// See Reference / Kubernetes API / Workload Resources / Pod.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConfigMapEnvSource {
    pub name: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    optional: Option<bool>,
}

/// See Reference / Kubernetes API / Common Definitions / ResourceFieldSelector.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct ResourceFieldSelector {
    resource: String,
    // TODO: additional fields.
}

/// See Reference / Kubernetes API / Common Definitions / ObjectFieldSelector.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ObjectFieldSelector {
    fieldPath: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    apiVersion: Option<String>,
}

/// See Reference / Kubernetes API / Workload Resources / Pod.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VolumeMount {
    pub mountPath: String,
    pub name: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub mountPropagation: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub subPathExpr: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub readOnly: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub subPath: Option<String>,
    // TODO: additional fields.
}

/// See Reference / Kubernetes API / Workload Resources / Pod.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VolumeDevice {
    pub devicePath: String,
    pub name: String,
}

/// See Reference / Kubernetes API / Workload Resources / Pod.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct ResourceRequirements {
    #[serde(skip_serializing_if = "Option::is_none")]
    requests: Option<BTreeMap<String, String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    limits: Option<BTreeMap<String, String>>,
    // TODO: claims field.
}

/// See Reference / Kubernetes API / Workload Resources / Pod.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Toleration {
    #[serde(skip_serializing_if = "Option::is_none")]
    key: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    operator: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    value: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    effect: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    tolerationSeconds: Option<i64>,
}

/// See Reference / Kubernetes API / Common Definitions / LocalObjectReference.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct LocalObjectReference {
    name: String,
}

/// See Reference / Kubernetes API / Workload Resources / Pod.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct PodDNSConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    nameservers: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<Vec<PodDNSConfigOption>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    searches: Option<Vec<String>>,
}

/// See Reference / Kubernetes API / Workload Resources / Pod.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct PodDNSConfigOption {
    name: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    value: Option<String>,
}

/// See Reference / Kubernetes API / Workload Resources / Pod.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct TopologySpreadConstraint {
    maxSkew: i32,
    topologyKey: String,
    whenUnsatisfiable: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    labelSelector: Option<yaml::LabelSelector>,

    #[serde(skip_serializing_if = "Option::is_none")]
    matchLabelKeys: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    minDomains: Option<i32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    nodeAffinityPolicy: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    nodeTaintsPolicy: Option<String>,
}

impl Container {
    pub async fn init(&mut self, config: &Config) {
        // Load container image properties from the registry.
        self.registry = registry::get_container(config, &self.image).await.unwrap();
    }

    pub fn get_env_variables(
        &self,
        dest_env: &mut Vec<String>,
        config_maps: &Vec<config_map::ConfigMap>,
        secrets: &Vec<secret::Secret>,
        namespace: &str,
        annotations: &Option<BTreeMap<String, String>>,
        service_account_name: &str,
    ) {
        if let Some(source_env) = &self.env {
            for env_variable in source_env {
                let value = env_variable.get_value(
                    config_maps,
                    secrets,
                    namespace,
                    annotations,
                    service_account_name,
                );
                let src_string = format!("{}={value}", &env_variable.name);

                if !dest_env.contains(&src_string) {
                    dest_env.push(src_string.clone());
                }
            }
        }

        if let Some(env_from_sources) = &self.envFrom {
            for env_from_source in env_from_sources {
                let env_from_source_values = env_from_source.get_values(config_maps, secrets);

                for value in env_from_source_values {
                    if !dest_env.contains(&value) {
                        dest_env.push(value.clone());
                    }
                }
            }
        }
    }

    pub fn is_privileged(&self) -> bool {
        if let Some(context) = &self.securityContext {
            if let Some(privileged) = context.privileged {
                return privileged;
            }
        }
        false
    }

    pub fn read_only_root_filesystem(&self) -> bool {
        if let Some(context) = &self.securityContext {
            if let Some(read_only) = context.readOnlyRootFilesystem {
                return read_only;
            }
        }
        false
    }

    pub fn get_process_args(&self, policy_args: &mut Vec<String>) -> (bool, bool) {
        let mut yaml_has_command = true;
        let mut yaml_has_args = true;

        if let Some(commands) = &self.command {
            for command in commands {
                policy_args.push(command.clone());
            }
        } else {
            yaml_has_command = false;
        }

        if let Some(args) = &self.args {
            for arg in args {
                policy_args.push(arg.clone());
            }
        } else {
            yaml_has_args = false;
        }

        (yaml_has_command, yaml_has_args)
    }

    pub fn get_exec_commands(&self) -> Vec<Vec<String>> {
        let mut commands = Vec::new();

        if let Some(probe) = &self.livenessProbe {
            if let Some(exec) = &probe.exec {
                commands.push(exec.command.clone());
            }
        }

        if let Some(probe) = &self.readinessProbe {
            if let Some(exec) = &probe.exec {
                commands.push(exec.command.clone());
            }
        }

        if let Some(probe) = &self.startupProbe {
            if let Some(exec) = &probe.exec {
                commands.push(exec.command.clone());
            }
        }

        if let Some(lifecycle) = &self.lifecycle {
            if let Some(postStart) = &lifecycle.postStart {
                if let Some(exec) = &postStart.exec {
                    commands.push(exec.command.clone());
                }
            }
            if let Some(preStop) = &lifecycle.preStop {
                if let Some(exec) = &preStop.exec {
                    commands.push(exec.command.clone());
                }
            }
        }

        commands
    }
}

impl EnvFromSource {
    pub fn get_values(
        &self,
        config_maps: &Vec<config_map::ConfigMap>,
        secrets: &Vec<secret::Secret>,
    ) -> Vec<String> {
        if let Some(config_map_env_source) = &self.configMapRef {
            if let Some(value) = config_map::get_values(&config_map_env_source.name, config_maps) {
                return value.clone();
            } else {
                panic!(
                    "Couldn't get values from configmap ref: {}",
                    &config_map_env_source.name
                );
            }
        }

        if let Some(secret_env_source) = &self.secretRef {
            if let Some(value) = secret::get_values(&secret_env_source.name, secrets) {
                return value.clone();
            } else {
                panic!(
                    "Couldn't get values from secret ref: {}",
                    &secret_env_source.name
                );
            }
        }
        panic!("envFrom: no configmap or secret source found!");
    }
}

impl EnvVar {
    pub fn get_value(
        &self,
        config_maps: &Vec<config_map::ConfigMap>,
        secrets: &Vec<secret::Secret>,
        namespace: &str,
        annotations: &Option<BTreeMap<String, String>>,
        service_account_name: &str,
    ) -> String {
        if let Some(value) = &self.value {
            return value.clone();
        }

        if let Some(value_from) = &self.valueFrom {
            if let Some(value) = config_map::get_value(value_from, config_maps) {
                return value.clone();
            }

            if let Some(value) = secret::get_value(value_from, secrets) {
                return value.clone();
            }

            if let Some(field_ref) = &value_from.fieldRef {
                let path: &str = &field_ref.fieldPath;
                match path {
                    "metadata.name" => return "$(sandbox-name)".to_string(),
                    "metadata.namespace" => return namespace.to_string(),
                    "metadata.uid" => return "$(pod-uid)".to_string(),
                    "status.hostIP" => return "$(host-ip)".to_string(),
                    "status.podIP" => return "$(pod-ip)".to_string(),
                    "spec.nodeName" => return "$(node-name)".to_string(),
                    "spec.serviceAccountName" => return service_account_name.to_string(),
                    _ => {
                        if let Some(value) = self.get_annotation_value(path, annotations) {
                            return value;
                        } else {
                            panic!(
                                "Env var: unsupported field reference: {}",
                                &field_ref.fieldPath
                            )
                        }
                    }
                }
            }

            if value_from.resourceFieldRef.is_some() {
                // TODO: should resource fields such as "limits.cpu" or "limits.memory"
                // be handled in a different way?
                return "$(resource-field)".to_string();
            }
        } else {
            panic!("Environment variable without value or valueFrom!");
        }

        panic!("Couldn't get the value of env var: {}", &self.name);
    }

    fn get_annotation_value(
        &self,
        reference: &str,
        anno: &Option<BTreeMap<String, String>>,
    ) -> Option<String> {
        let prefix = "metadata.annotations['";
        let suffix = "']";
        if reference.starts_with(prefix) && reference.ends_with(suffix) {
            if let Some(annotations) = anno {
                let start = prefix.len();
                let end = reference.len() - 2;
                let annotation = reference[start..end].to_string();

                if let Some(value) = annotations.get(&annotation) {
                    return Some(value.clone());
                } else {
                    warn!(
                        "Can't find the value of annotation {}. Allowing any value.",
                        &annotation
                    );
                }
            }

            // TODO: should missing annotations be handled differently?
            return Some("$(todo-annotation)".to_string());
        }
        None
    }
}

#[async_trait]
impl yaml::K8sResource for Pod {
    async fn init(&mut self, config: &Config, doc_mapping: &serde_yaml::Value, _silent: bool) {
        yaml::k8s_resource_init(&mut self.spec, config).await;
        self.doc_mapping = doc_mapping.clone();
    }

    fn get_sandbox_name(&self) -> Option<String> {
        let name = self.metadata.get_name();
        if !name.is_empty() {
            return Some(name);
        }
        panic!("No pod name.");
    }

    fn get_namespace(&self) -> Option<String> {
        self.metadata.get_namespace()
    }

    fn get_container_mounts_and_storages(
        &self,
        policy_mounts: &mut Vec<policy::KataMount>,
        storages: &mut Vec<agent::Storage>,
        container: &Container,
        settings: &settings::Settings,
    ) {
        yaml::get_container_mounts_and_storages(
            policy_mounts,
            storages,
            container,
            settings,
            &self.spec.volumes,
        );
    }

    fn generate_policy(&self, agent_policy: &policy::AgentPolicy) -> String {
        agent_policy.generate_policy(self)
    }

    fn serialize(&mut self, policy: &str) -> String {
        yaml::add_policy_annotation(&mut self.doc_mapping, "", policy);
        serde_yaml::to_string(&self.doc_mapping).unwrap()
    }

    fn get_containers(&self) -> &Vec<Container> {
        &self.spec.containers
    }

    fn get_annotations(&self) -> &Option<BTreeMap<String, String>> {
        &self.metadata.annotations
    }

    fn use_host_network(&self) -> bool {
        if let Some(host_network) = self.spec.hostNetwork {
            return host_network;
        }
        false
    }

    fn use_sandbox_pidns(&self) -> bool {
        if let Some(shared) = self.spec.shareProcessNamespace {
            return shared;
        }
        false
    }

    fn get_runtime_class_name(&self) -> Option<String> {
        self.spec
            .runtimeClassName
            .clone()
            .or_else(|| Some(String::new()))
    }

    fn get_process_fields(&self, process: &mut policy::KataProcess) {
        yaml::get_process_fields(process, &self.spec.securityContext);
    }
}

impl Container {
    pub fn apply_capabilities(
        &self,
        capabilities: &mut policy::KataLinuxCapabilities,
        defaults: &policy::CommonData,
    ) {
        assert!(capabilities.Ambient.is_empty());
        assert!(capabilities.Inheritable.is_empty());

        if let Some(securityContext) = &self.securityContext {
            if let Some(yaml_capabilities) = &securityContext.capabilities {
                if let Some(drop) = &yaml_capabilities.drop {
                    for c in drop {
                        if c == "ALL" {
                            capabilities.Bounding.clear();
                            capabilities.Permitted.clear();
                            capabilities.Effective.clear();
                        } else {
                            let cap = "CAP_".to_string() + c;

                            capabilities.Bounding.retain(|x| !x.eq(&cap));
                            capabilities.Permitted.retain(|x| !x.eq(&cap));
                            capabilities.Effective.retain(|x| !x.eq(&cap));
                        }
                    }
                }
                if let Some(add) = &yaml_capabilities.add {
                    for c in add {
                        let cap = "CAP_".to_string() + c;

                        if !capabilities.Bounding.contains(&cap) {
                            capabilities.Bounding.push(cap.clone());
                        }
                        if !capabilities.Permitted.contains(&cap) {
                            capabilities.Permitted.push(cap.clone());
                        }
                        if !capabilities.Effective.contains(&cap) {
                            capabilities.Effective.push(cap.clone());
                        }
                    }
                }
            }
        }
        compress_default_capabilities(capabilities, defaults);
    }

    pub fn get_process_fields(&self, process: &mut policy::KataProcess) {
        if let Some(context) = &self.securityContext {
            if let Some(uid) = context.runAsUser {
                process.User.UID = uid.try_into().unwrap();
            }
            if let Some(allow) = context.allowPrivilegeEscalation {
                process.NoNewPrivileges = !allow
            }
        }
    }
}

fn compress_default_capabilities(
    capabilities: &mut policy::KataLinuxCapabilities,
    defaults: &policy::CommonData,
) {
    assert!(capabilities.Ambient.is_empty());
    assert!(capabilities.Inheritable.is_empty());

    compress_capabilities(&mut capabilities.Bounding, defaults);
    compress_capabilities(&mut capabilities.Permitted, defaults);
    compress_capabilities(&mut capabilities.Effective, defaults);
}

fn compress_capabilities(capabilities: &mut Vec<String>, defaults: &policy::CommonData) {
    let default_caps = if capabilities == &defaults.default_caps {
        "$(default_caps)"
    } else if capabilities == &defaults.privileged_caps {
        "$(privileged_caps)"
    } else {
        ""
    };

    if !default_caps.is_empty() {
        capabilities.clear();
        capabilities.push(default_caps.to_string());
    }
}

pub async fn add_pause_container(containers: &mut Vec<Container>, config: &Config) {
    debug!("Adding pause container...");
    let mut pause_container = Container {
        image: config.settings.cluster_config.pause_container_image.clone(),
        name: String::new(),
        imagePullPolicy: None,
        securityContext: Some(SecurityContext {
            readOnlyRootFilesystem: Some(true),
            allowPrivilegeEscalation: Some(false),
            privileged: None,
            capabilities: None,
            runAsUser: None,
            seccompProfile: None,
        }),
        ..Default::default()
    };
    pause_container.init(config).await;
    containers.insert(0, pause_container);
    debug!("pause container added.");
}
