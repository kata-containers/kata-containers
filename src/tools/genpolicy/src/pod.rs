// Copyright (c) 2023 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

// Allow K8s YAML field names.
#![allow(non_snake_case)]

use crate::config_maps;
use crate::infra;
use crate::obj_meta;
use crate::pause_container;
use crate::policy;
use crate::registry;
use crate::utils;
use crate::volumes;
use crate::yaml;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use base64::{engine::general_purpose, Engine as _};
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

    #[serde(skip)]
    registry_containers: Vec<registry::Container>,
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
    async fn initialize(&mut self) -> Result<()> {
        pause_container::add_pause_container(&mut self.spec.containers);
        self.registry_containers = registry::get_registry_containers(&self.spec.containers).await?;
        Ok(())
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

    fn generate_policy(
        &mut self,
        rules: &str,
        infra_policy: &infra::InfraPolicy,
        config_maps: &Vec<config_maps::ConfigMap>,
        in_out_files: &utils::InOutFiles,
    ) -> Result<()> {
        let mut policy_containers = Vec::new();

        for i in 0..self.spec.containers.len() {
            policy_containers.push(policy::get_container_policy(
                self,
                infra_policy,
                config_maps,
                &self.spec.containers[i],
                i == 0,
                &self.registry_containers[i],
            )?);
        }

        let policy_data = policy::PolicyData {
            containers: policy_containers,
        };

        let json_data = serde_json::to_string_pretty(&policy_data)
            .map_err(|e| anyhow!(e))
            .unwrap();

        let policy = rules.to_string() + "\npolicy_data := " + &json_data;

        if let Some(file_name) = &in_out_files.output_policy_file {
            policy::export_decoded_policy(&policy, &file_name)?;
        }

        let encoded_policy = general_purpose::STANDARD.encode(policy.as_bytes());
        self.metadata.add_policy_annotation(&encoded_policy);

        // Remove the pause container before serializing.
        self.spec.containers.remove(0);
        Ok(())
    }

    fn serialize(&self) -> Result<String> {
        Ok(serde_yaml::to_string(&self)?)
    }
}
