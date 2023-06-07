// Copyright (c) 2023 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

// Allow K8s YAML field names.
#![allow(non_snake_case)]

use crate::config_map;
use crate::infra;
use crate::obj_meta;
use crate::policy;
use crate::registry;
use crate::utils;
use crate::volume;
use crate::yaml;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// See Reference / Kubernetes API / Workload Resources / Pod.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pod {
    apiVersion: String,
    kind: String,
    pub metadata: obj_meta::ObjectMeta,
    pub spec: PodSpec,

    #[serde(skip)]
    registry_containers: Vec<registry::Container>,
}

/// See Reference / Kubernetes API / Workload Resources / Pod.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PodSpec {
    #[serde(skip_serializing_if = "Option::is_none")]
    nodeSelector: Option<BTreeMap<String, String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    restartPolicy: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    runtimeClassName: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub initContainers: Option<Vec<Container>>,

    pub containers: Vec<Container>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub volumes: Option<Vec<volume::Volume>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub serviceAccountName: Option<String>,
}

/// See Reference / Kubernetes API / Workload Resources / Pod.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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
}

/// See Reference / Kubernetes API / Workload Resources / Pod.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Probe {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exec: Option<ExecAction>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub initialDelaySeconds: Option<i32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeoutSeconds: Option<i32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub failureThreshold: Option<i32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub successThreshold: Option<i32>,
    // TODO: additional fiels.
}

/// See Reference / Kubernetes API / Workload Resources / Pod.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
pub struct Lifecycle {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub postStart: Option<LifecycleHandler>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub preStop: Option<LifecycleHandler>,
}

/// See Reference / Kubernetes API / Workload Resources / Pod.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LifecycleHandler {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exec: Option<ExecAction>,
    // TODO: additional fiels.
}

/// See Reference / Kubernetes API / Workload Resources / Pod.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecAction {
    pub command: Vec<String>,
}

/// See Reference / Kubernetes API / Workload Resources / Pod.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Capabilities {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub add: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub drop: Option<Vec<String>>,
}

/// See Reference / Kubernetes API / Workload Resources / Pod.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
pub struct EnvVar {
    pub name: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    value: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub valueFrom: Option<EnvVarSource>,
}

/// See Reference / Kubernetes API / Workload Resources / Pod.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnvVarSource {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub configMapKeyRef: Option<ConfigMapKeySelector>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub fieldRef: Option<ObjectFieldSelector>,
}

/// See Reference / Kubernetes API / Workload Resources / Pod.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigMapKeySelector {
    pub key: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub optional: Option<bool>,
}

/// See Reference / Kubernetes API / Common Definitions / ObjectFieldSelector.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectFieldSelector {
    pub fieldPath: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub apiVersion: Option<String>,
}

/// See Reference / Kubernetes API / Workload Resources / Pod.
/// See VolumeMount in the Kubernetes API reference.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VolumeMount {
    pub mountPath: String,
    pub name: String,
    // TODO: additional fields.
}

/// See ResourceRequirements in the Kubernetes API reference.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceRequirements {
    #[serde(skip_serializing_if = "Option::is_none")]
    requests: Option<BTreeMap<String, String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    limits: Option<BTreeMap<String, String>>,
    // TODO: claims field.
}

impl Container {
    pub fn get_env_variables(
        &self,
        dest_env: &mut Vec<String>,
        config_maps: &Vec<config_map::ConfigMap>,
        namespace: &str,
    ) -> Result<()> {
        if let Some(source_env) = &self.env {
            for env_variable in source_env {
                let mut src_string = env_variable.name.clone() + "=";
                src_string += &env_variable.get_value(config_maps, namespace)?;
                if !dest_env.contains(&src_string) {
                    dest_env.push(src_string.clone());
                }
            }
        }
        Ok(())
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
    ) -> Result<String> {
        if let Some(value) = &self.value {
            return Ok(value.clone());
        } else if let Some(value_from) = &self.valueFrom {
            if let Some(value) = config_map::get_value(value_from, config_maps) {
                return Ok(value.clone());
            } else if let Some(field_ref) = &value_from.fieldRef {
                let path: &str = &field_ref.fieldPath;
                match path {
                    "metadata.namespace" => return Ok(namespace.to_string()),
                    "status.podIP" => return Ok("$(pod-ip)".to_string()),
                    _ => {
                        return Err(anyhow!(
                            "Unsupported field reference {}",
                            &field_ref.fieldPath
                        ))
                    }
                }
            }
        } else {
            panic!("Environment variable without value or valueFrom!");
        }

        Err(anyhow!("Unknown EnvVar value - {}", &self.name))
    }
}

#[async_trait]
impl yaml::K8sObject for Pod {
    async fn initialize(&mut self, use_cached_files: bool) -> Result<()> {
        yaml::init_k8s_object(
            &mut self.spec.containers,
            &mut self.registry_containers,
            use_cached_files,
        )
        .await
    }

    fn requires_policy(&self) -> bool {
        true
    }

    fn get_metadata_name(&self) -> Result<String> {
        self.metadata.get_name()
    }

    fn get_host_name(&self) -> Result<String> {
        // Example: "hostname": "^busybox-cc$",
        Ok("^".to_string() + &self.get_metadata_name()? + "$")
    }

    fn get_sandbox_name(&self) -> Result<Option<String>> {
        Ok(Some(self.get_metadata_name()?))
    }

    fn get_namespace(&self) -> Result<String> {
        self.metadata.get_namespace()
    }

    fn get_container_mounts_and_storages(
        &self,
        policy_mounts: &mut Vec<oci::Mount>,
        storages: &mut Vec<policy::SerializedStorage>,
        container: &Container,
        infra_policy: &infra::InfraPolicy,
    ) -> Result<()> {
        if let Some(volumes) = &self.spec.volumes {
            yaml::get_container_mounts_and_storages(
                policy_mounts,
                storages,
                container,
                infra_policy,
                volumes,
            )
        } else {
            Ok(())
        }
    }

    fn generate_policy(
        &mut self,
        rules: &str,
        infra_policy: &infra::InfraPolicy,
        config_maps: &Vec<config_map::ConfigMap>,
        in_out_files: &utils::InOutFiles,
    ) -> Result<()> {
        let encoded_policy = yaml::generate_policy(
            rules,
            infra_policy,
            config_maps,
            in_out_files,
            self,
            &self.registry_containers,
            &self.spec.containers,
        )?;

        self.metadata.add_policy_annotation(&encoded_policy);

        // Remove the pause container before serializing.
        self.spec.containers.remove(0);
        Ok(())
    }

    fn serialize(&mut self) -> Result<String> {
        Ok(serde_yaml::to_string(&self)?)
    }
}

impl Container {
    pub fn apply_capabilities(&self, capabilities: &mut oci::LinuxCapabilities) -> Result<()> {
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

        Ok(())
    }
}
