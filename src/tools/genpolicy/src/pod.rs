// Copyright (c) 2023 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

// Allow K8s YAML field names.
#![allow(non_snake_case)]

use crate::config_maps;
use crate::infra;
use crate::obj_meta;
use crate::policy;
use crate::registry;
use crate::volumes;
use crate::yaml;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// See Pod in the Kubernetes API reference.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pod {
    apiVersion: String,
    kind: String,
    pub metadata: obj_meta::ObjectMeta,
    pub spec: PodSpec,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PodSpec {
    #[serde(skip_serializing_if = "Option::is_none")]
    restartPolicy: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    runtimeClassName: Option<String>,

    pub containers: Vec<Container>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub volumes: Option<Vec<volumes::Volume>>,
}

/// See Container in the Kubernetes API reference.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Container {
    pub image: String,
    pub name: String,

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
}

/// See SecurityContext in the Kubernetes API reference.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SecurityContext {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub readOnlyRootFilesystem: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowPrivilegeEscalation: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub privileged: Option<bool>,
}

/// See ContainerPort in the Kubernetes API reference.
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

/// See EnvVar in the Kubernetes API reference.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnvVar {
    pub name: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    value: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub valueFrom: Option<EnvVarSource>,
}

/// See EnvVarSource in the Kubernetes API reference.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnvVarSource {
    pub configMapKeyRef: ConfigMapKeySelector,
}

/// See ConfigMapKeySelector in the Kubernetes API reference.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigMapKeySelector {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    pub key: String,
    // TODO: optional field.
}

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
        config_maps: &Vec<config_maps::ConfigMap>,
    ) {
        if let Some(source_env) = &self.env {
            for env_variable in source_env {
                let mut src_string = env_variable.name.clone() + "=";
                src_string += &env_variable.get_value(config_maps);
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
}

impl EnvVar {
    pub fn get_value(&self, config_maps: &Vec<config_maps::ConfigMap>) -> String {
        if let Some(value) = &self.value {
            return value.clone();
        } else if let Some(value_from) = &self.valueFrom {
            if let Some(value) = config_maps::get_value(value_from, config_maps) {
                return value.clone();
            }
        } else {
            panic!("Environment variable without value or valueFrom!");
        }

        "".to_string()
    }
}

#[async_trait]
impl yaml::K8sObject for Pod {
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

    fn add_policy_annotation(&mut self, encoded_policy: &str) {
        self.metadata.add_policy_annotation(encoded_policy)
    }

    async fn get_registry_containers(&self) -> Result<Vec<registry::Container>> {
        registry::get_registry_containers(&self.spec.containers).await
    }

    fn get_policy_data(
        &self,
        k8s_object: &dyn yaml::K8sObject,
        infra_policy: &infra::InfraPolicy,
        config_maps: &Vec<config_maps::ConfigMap>,
        registry_containers: &Vec<registry::Container>,
    ) -> Result<policy::PolicyData> {
        policy::get_policy_data(
            k8s_object,
            infra_policy,
            config_maps,
            &self.spec.containers,
            registry_containers,
        )
    }

    fn get_container_mounts_and_storages(
        &self,
        policy_mounts: &mut Vec<oci::Mount>,
        storages: &mut Vec<policy::SerializedStorage>,
        container: &Container,
        infra_policy: &infra::InfraPolicy,
    ) -> Result<()> {
        if let Some(volumes) = &self.spec.volumes {
            for volume in volumes {
                policy::get_container_mounts_and_storages(
                    policy_mounts,
                    storages,
                    container,
                    infra_policy,
                    &volume,
                )?;
            }
        }

        Ok(())
    }

    fn serialize(&mut self, file_name: &Option<String>) -> Result<()> {
        self.spec.containers.remove(0);

        if let Some(yaml) = file_name {
            serde_yaml::to_writer(
                std::fs::OpenOptions::new()
                    .write(true)
                    .truncate(true)
                    .create(true)
                    .open(yaml)
                    .map_err(|e| anyhow!(e))?,
                &self,
            )?;
        } else {
            serde_yaml::to_writer(std::io::stdout(), &self)?;
        }

        Ok(())
    }
}
