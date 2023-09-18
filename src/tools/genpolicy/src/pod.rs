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
use crate::volume;
use crate::yaml;

use async_trait::async_trait;
use log::warn;
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
    runtimeClassName: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub initContainers: Option<Vec<Container>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub imagePullSecrets: Option<Vec<LocalObjectReference>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub affinity: Option<Affinity>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub volumes: Option<Vec<volume::Volume>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub serviceAccountName: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub serviceAccount: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminationGracePeriodSeconds: Option<i64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub tolerations: Option<Vec<Toleration>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub hostname: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub hostNetwork: Option<bool>,
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
    pub imagePullPolicy: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub securityContext: Option<SecurityContext>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub volumeMounts: Option<Vec<VolumeMount>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<Vec<EnvVar>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub resources: Option<ResourceRequirements>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub ports: Option<Vec<ContainerPort>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub args: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub lifecycle: Option<Lifecycle>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub livenessProbe: Option<Probe>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub readinessProbe: Option<Probe>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub startupProbe: Option<Probe>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub serviceAccountName: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub stdin: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub tty: Option<bool>,
}

/// See Reference / Kubernetes API / Workload Resources / Pod.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Affinity {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub podAntiAffinity: Option<PodAntiAffinity>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub podAffinity: Option<PodAffinity>,
    // TODO: additional fields.
}

/// See Reference / Kubernetes API / Workload Resources / Pod.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PodAffinity {
    #[serde(skip_serializing_if = "Option::is_none")]
    requiredDuringSchedulingIgnoredDuringExecution: Option<Vec<PodAffinityTerm>>,
}

/// See Reference / Kubernetes API / Workload Resources / Pod.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PodAntiAffinity {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preferredDuringSchedulingIgnoredDuringExecution: Option<Vec<WeightedPodAffinityTerm>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub requiredDuringSchedulingIgnoredDuringExecution: Option<Vec<PodAffinityTerm>>,
    // TODO: additional fields.
}

/// See Reference / Kubernetes API / Workload Resources / Pod.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WeightedPodAffinityTerm {
    pub weight: i32,
    pub podAffinityTerm: PodAffinityTerm,
}

/// See Reference / Kubernetes API / Workload Resources / Pod.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PodAffinityTerm {
    topologyKey: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    labelSelector: Option<yaml::LabelSelector>,
    // TODO: additional fields.
}

/// See Reference / Kubernetes API / Workload Resources / Pod.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Probe {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exec: Option<ExecAction>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub initialDelaySeconds: Option<i32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeoutSeconds: Option<i32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub periodSeconds: Option<i32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub failureThreshold: Option<i32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub successThreshold: Option<i32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub httpGet: Option<HTTPGetAction>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub tcpSocket: Option<TCPSocketAction>,
    // TODO: additional fiels.
}

/// See Reference / Kubernetes API / Workload Resources / Pod.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TCPSocketAction {
    pub port: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
}

/// See Reference / Kubernetes API / Workload Resources / Pod.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HTTPGetAction {
    pub port: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub scheme: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub httpHeaders: Option<Vec<HTTPHeader>>,
    // TODO: additional fiels.
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HTTPHeader {
    name: String,
    value: String,
}

/// See Reference / Kubernetes API / Workload Resources / Pod.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SecurityContext {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub readOnlyRootFilesystem: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowPrivilegeEscalation: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub privileged: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<Capabilities>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub runAsUser: Option<i64>,
}

/// See Reference / Kubernetes API / Workload Resources / Pod.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Lifecycle {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub postStart: Option<LifecycleHandler>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub preStop: Option<LifecycleHandler>,
}

/// See Reference / Kubernetes API / Workload Resources / Pod.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LifecycleHandler {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exec: Option<ExecAction>,
    // TODO: additional fiels.
}

/// See Reference / Kubernetes API / Workload Resources / Pod.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExecAction {
    pub command: Vec<String>,
}

/// See Reference / Kubernetes API / Workload Resources / Pod.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Capabilities {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub add: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub drop: Option<Vec<String>>,
}

/// See Reference / Kubernetes API / Workload Resources / Pod.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ContainerPort {
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
pub struct EnvVar {
    pub name: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    value: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub valueFrom: Option<EnvVarSource>,
}

/// See Reference / Kubernetes API / Workload Resources / Pod.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EnvVarSource {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub configMapKeyRef: Option<ConfigMapKeySelector>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub fieldRef: Option<ObjectFieldSelector>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub secretKeyRef: Option<SecretKeySelector>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub resourceFieldRef: Option<ResourceFieldSelector>,
}

/// See Reference / Kubernetes API / Workload Resources / Pod.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SecretKeySelector {
    pub key: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub optional: Option<bool>,
}

/// See Reference / Kubernetes API / Workload Resources / Pod.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConfigMapKeySelector {
    pub key: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub optional: Option<bool>,
}

/// See Reference / Kubernetes API / Common Definitions / ObjectFieldSelector.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResourceFieldSelector {
    pub resource: String,
    // TODO: additional fields.
}

/// See Reference / Kubernetes API / Common Definitions / ObjectFieldSelector.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ObjectFieldSelector {
    pub fieldPath: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub apiVersion: Option<String>,
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
    // TODO: additional fields.
}

/// See Reference / Kubernetes API / Workload Resources / Pod.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResourceRequirements {
    #[serde(skip_serializing_if = "Option::is_none")]
    requests: Option<BTreeMap<String, String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    limits: Option<BTreeMap<String, String>>,
    // TODO: claims field.
}

/// See Reference / Kubernetes API / Workload Resources / Pod.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Toleration {
    #[serde(skip_serializing_if = "Option::is_none")]
    operator: Option<String>,
    // TODO: additional fields.
}

/// See Reference / Kubernetes API / Common Definitions / LocalObjectReference.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LocalObjectReference {
    pub name: String,
}

impl Container {
    pub async fn init(&mut self, use_cache: bool) {
        self.registry = registry::get_container(use_cache, &self.image)
            .await
            .unwrap();
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
                let mut src_string = env_variable.name.clone() + "=";

                src_string += &env_variable.get_value(
                    config_maps,
                    secrets,
                    namespace,
                    annotations,
                    service_account_name,
                );

                if !dest_env.contains(&src_string) {
                    dest_env.push(src_string.clone());
                }
            }
        }
    }

    pub fn allow_privilege_escalation(&self) -> bool {
        if let Some(context) = &self.securityContext {
            if let Some(allow) = context.allowPrivilegeEscalation {
                return allow;
            }
        }
        true
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

    pub fn is_privileged(&self) -> bool {
        if let Some(context) = &self.securityContext {
            if let Some(privileged) = context.privileged {
                return privileged;
            }
        }
        false
    }

    pub fn get_exec_commands(&self) -> Vec<String> {
        let mut commands = Vec::new();

        if let Some(probe) = &self.livenessProbe {
            if let Some(exec) = &probe.exec {
                commands.push(exec.command.join(" "));
            }
        }

        if let Some(probe) = &self.readinessProbe {
            if let Some(exec) = &probe.exec {
                commands.push(exec.command.join(" "));
            }
        }

        if let Some(probe) = &self.startupProbe {
            if let Some(exec) = &probe.exec {
                commands.push(exec.command.join(" "));
            }
        }

        if let Some(lifecycle) = &self.lifecycle {
            if let Some(postStart) = &lifecycle.postStart {
                if let Some(exec) = &postStart.exec {
                    commands.push(exec.command.join(" "));
                }
            }
            if let Some(preStop) = &lifecycle.preStop {
                if let Some(exec) = &preStop.exec {
                    commands.push(exec.command.join(" "));
                }
            }
        }

        commands
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
        } else if let Some(value_from) = &self.valueFrom {
            if let Some(value) = config_map::get_value(value_from, config_maps) {
                return value.clone();
            } else if let Some(value) = secret::get_value(value_from, secrets) {
                return value.clone();
            } else if let Some(field_ref) = &value_from.fieldRef {
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
            } else if value_from.resourceFieldRef.is_some() {
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
    async fn init(
        &mut self,
        use_cache: bool,
        doc_mapping: &serde_yaml::Value,
        _silent_unsupported_fields: bool,
    ) {
        yaml::k8s_resource_init(&mut self.spec, use_cache).await;
        self.doc_mapping = doc_mapping.clone();
    }

    fn get_sandbox_name(&self) -> Option<String> {
        let name = self.metadata.get_name();
        if !name.is_empty() {
            return Some(name);
        }
        panic!("No pod name.");
    }

    fn get_namespace(&self) -> String {
        self.metadata.get_namespace()
    }

    fn get_container_mounts_and_storages(
        &self,
        policy_mounts: &mut Vec<policy::KataMount>,
        storages: &mut Vec<policy::SerializedStorage>,
        container: &Container,
        agent_policy: &policy::AgentPolicy,
    ) {
        if let Some(volumes) = &self.spec.volumes {
            yaml::get_container_mounts_and_storages(
                policy_mounts,
                storages,
                container,
                agent_policy,
                volumes,
            );
        }
    }

    fn generate_policy(&self, agent_policy: &policy::AgentPolicy) -> String {
        agent_policy.generate_policy(self)
    }

    fn serialize(&mut self, policy: &str) -> String {
        yaml::add_policy_annotation(&mut self.doc_mapping, "metadata", policy);
        serde_yaml::to_string(&self.doc_mapping).unwrap()
    }

    fn get_containers(&self) -> &Vec<Container> {
        &self.spec.containers
    }

    fn get_annotations(&self) -> Option<BTreeMap<String, String>> {
        if let Some(annotations) = &self.metadata.annotations {
            return Some(annotations.clone());
        }
        None
    }

    fn use_host_network(&self) -> bool {
        if let Some(host_network) = self.spec.hostNetwork {
            return host_network;
        }
        false
    }
}

impl Container {
    pub fn apply_capabilities(&self, capabilities: &mut policy::KataLinuxCapabilities) {
        if let Some(securityContext) = &self.securityContext {
            if let Some(yaml_capabilities) = &securityContext.capabilities {
                if let Some(drop) = &yaml_capabilities.drop {
                    for c in drop {
                        if c == "ALL" {
                            capabilities.Bounding.clear();
                            capabilities.Permitted.clear();
                            capabilities.Effective.clear();
                        } else {
                            let cap = "CAP_".to_string() + &c;

                            capabilities.Bounding.retain(|x| !x.eq(&cap));
                            capabilities.Permitted.retain(|x| !x.eq(&cap));
                            capabilities.Effective.retain(|x| !x.eq(&cap));
                        }
                    }
                }
                if let Some(add) = &yaml_capabilities.add {
                    for c in add {
                        let cap = "CAP_".to_string() + &c;

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
    }
}
