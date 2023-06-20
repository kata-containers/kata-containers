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
use crate::volume;
use crate::yaml;

use anyhow::Result;
use async_trait::async_trait;
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

    #[serde(skip)]
    registry_containers: Vec<registry::Container>,
}

/// See Reference / Kubernetes API / Workload Resources / Pod.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PodSpec {
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

    pub containers: Vec<Container>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub volumes: Option<Vec<volume::Volume>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub serviceAccountName: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminationGracePeriodSeconds: Option<i64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub tolerations: Option<Vec<Toleration>>,
}

/// See Reference / Kubernetes API / Workload Resources / Pod.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Container {
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
    pub readinessProbe: Option<Probe>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub livenessProbe: Option<Probe>,
}

/// See Reference / Kubernetes API / Workload Resources / Pod.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Affinity {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub podAntiAffinity: Option<PodAntiAffinity>,
    // TODO: additional fields.
}

/// See Reference / Kubernetes API / Workload Resources / Pod.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PodAntiAffinity {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preferredDuringSchedulingIgnoredDuringExecution: Option<Vec<WeightedPodAffinityTerm>>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    labelSelector: Option<yaml::LabelSelector>,

    topologyKey: String,
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
    // TODO: additional fiels.
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
    // TODO: additional fiels.
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
    pub fn get_env_variables(
        &self,
        dest_env: &mut Vec<String>,
        config_maps: &Vec<config_map::ConfigMap>,
        namespace: &str,
    ) {
        if let Some(source_env) = &self.env {
            for env_variable in source_env {
                let mut src_string = env_variable.name.clone() + "=";
                src_string += &env_variable.get_value(config_maps, namespace);
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

        if let Some(probe) = &self.readinessProbe {
            if let Some(exec) = &probe.exec {
                commands.push(exec.command.join(" "));
            }
        }
        if let Some(lifecycle) = &self.lifecycle {
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
        namespace: &str,
    ) -> String {
        if let Some(value) = &self.value {
            return value.clone();
        } else if let Some(value_from) = &self.valueFrom {
            if let Some(value) = config_map::get_value(value_from, config_maps) {
                return value.clone();
            } else if let Some(field_ref) = &value_from.fieldRef {
                let path: &str = &field_ref.fieldPath;
                match path {
                    "metadata.namespace" => return namespace.to_string(),
                    "status.podIP" => return "$(pod-ip)".to_string(),
                    "spec.nodeName" => return "$(node-name)".to_string(),
                    _ => panic!("Unsupported field reference: {}", &field_ref.fieldPath),
                }
            }
        } else {
            panic!("Environment variable without value or valueFrom!");
        }

        panic!("Unknown EnvVar value: {}", &self.name);
    }
}

#[async_trait]
impl yaml::K8sResource for Pod {
    async fn init(
        &mut self,
        use_cache: bool,
        doc_mapping: &serde_yaml::Value,
        _silent_unsupported_fields: bool,
    ) -> Result<()> {
        yaml::k8s_resource_init(&mut self.spec, &mut self.registry_containers, use_cache).await?;
        self.doc_mapping = doc_mapping.clone();
        Ok(())
    }

    fn get_metadata_name(&self) -> String {
        self.metadata.get_name()
    }

    fn get_host_name(&self) -> String {
        // Example: "hostname": "^busybox-cc$",
        "^".to_string() + &self.get_metadata_name() + "$"
    }

    fn get_sandbox_name(&self) -> Option<String> {
        Some(self.get_metadata_name())
    }

    fn get_namespace(&self) -> String {
        self.metadata.get_namespace()
    }

    fn get_container_mounts_and_storages(
        &self,
        policy_mounts: &mut Vec<oci::Mount>,
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
        yaml::generate_policy(self, agent_policy)
    }

    fn serialize(&mut self, policy: &str) -> String {
        yaml::add_policy_annotation(&mut self.doc_mapping, "metadata", policy);
        serde_yaml::to_string(&self.doc_mapping).unwrap()
    }

    fn get_containers(&self) -> (&Vec<registry::Container>, &Vec<Container>) {
        (
            &self.registry_containers,
            &self.spec.containers,
        )
    }
}

impl Container {
    pub fn apply_capabilities(&self, capabilities: &mut oci::LinuxCapabilities) {
        if let Some(securityContext) = &self.securityContext {
            if let Some(yaml_capabilities) = &securityContext.capabilities {
                if let Some(add) = &yaml_capabilities.add {
                    for c in add {
                        let cap = "CAP_".to_string() + &c;

                        if !capabilities.bounding.contains(&cap) {
                            capabilities.bounding.push(cap.clone());
                        }
                        if !capabilities.permitted.contains(&cap) {
                            capabilities.permitted.push(cap.clone());
                        }
                        if !capabilities.effective.contains(&cap) {
                            capabilities.effective.push(cap.clone());
                        }
                    }
                }
                if let Some(drop) = &yaml_capabilities.drop {
                    for c in drop {
                        let cap = "CAP_".to_string() + &c;

                        capabilities.bounding.retain(|x| !x.eq(&cap));
                        capabilities.permitted.retain(|x| !x.eq(&cap));
                        capabilities.effective.retain(|x| !x.eq(&cap));
                    }
                }
            }
        }
    }
}
