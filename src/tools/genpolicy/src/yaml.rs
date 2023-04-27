// Copyright (c) 2023 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

// Allow K8s YAML field names.
#![allow(non_snake_case)]

use anyhow::{anyhow, Result};
use base64::{engine::general_purpose, Engine as _};
use log::info;
use serde::{Deserialize, Serialize};
use serde_yaml;
use std::collections::BTreeMap;
use std::fs::{read_to_string, File};
use std::io::Write;

// Example:
//
// apiVersion: v1
// kind: Pod
// metadata:
// ...
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Yaml {
    apiVersion: String,
    kind: String,
    pub metadata: Metadata,
    pub spec: Spec,
}

// Example:
//
// metadata:
//   labels:
//     run: busybox
//   name: busybox-cc
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Metadata {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    labels: Option<BTreeMap<String, String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    annotations: BTreeMap<String, String>,

    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub namespace: String,
}

// Example:
//
// spec:
//   restartPolicy: Never
//   runtimeClassName: kata-cc
//   containers:
//   - image: docker.io/library/busybox:1.36.0
// ...
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Spec {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    restartPolicy: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    runtimeClassName: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub containers: Option<Vec<Container>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub volumes: Option<Vec<Volume>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    replicas: Option<u32>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    selector: Option<Selector>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template: Option<Template>,
}

// Example:
//
// - image: docker.io/library/busybox:1.36.0
//   name: busybox
//   volumeMounts:
//   - mountPath: /busy1
//     name: data
// ...
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Container {
    pub image: String,
    pub name: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub securityContext: Option<SecurityContext>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub volumeMounts: Option<Vec<VolumeMount>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub env: Option<Vec<EnvVariable>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resources: Option<Resources>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ports: Option<Vec<Port>>,
}

// Example:
//
// securityContext:
//   readOnlyRootFilesystem: true
//   allowPrivilegeEscalation: false
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SecurityContext {
    #[serde(default = "default_false")]
    pub readOnlyRootFilesystem: bool,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowPrivilegeEscalation: Option<bool>,
}
fn default_false() -> bool {
    false
}

// Example:
//
// - mountPath: /busy1
//   name: data
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct VolumeMount {
    pub mountPath: String,
    pub name: String,
}

// Example:
//
// - name: my-pod-volume
//   persistentVolumeClaim:
//      claimName: my-volume-claim
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Volume {
    pub name: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emptyDir: Option<EmptyDirVolume>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hostPath: Option<HostPathVolume>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub persistentVolumeClaim: Option<VolumeClaimVolume>,
}

// Example:
//
// hostPath:
//   path: /dev/sev
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct HostPathVolume {
    pub path: String,
}

// Example:
//
// emptyDir: {}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct EmptyDirVolume {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sizeLimit: Option<String>,
}

// Example:
//
// claimName: my-volume-claim
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct VolumeClaimVolume {
    pub claimName: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct Selector {
    matchLabels: Option<BTreeMap<String, String>>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Template {
    pub metadata: Metadata,
    pub spec: TemplateSpec,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TemplateSpec {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nodeSelector: Option<BTreeMap<String, String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    runtimeClassName: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub containers: Option<Vec<Container>>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct EnvVariable {
    name: String,
    value: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Resources {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    requests: Option<HardwareResources>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    limits: Option<HardwareResources>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct HardwareResources {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    cpu: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    memory: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Port {
    containerPort: u16,

    #[serde(default, skip_serializing_if = "String::is_empty")]
    name: String,
}

impl Yaml {
    pub fn new(yaml_file: &Option<String>) -> Result<Self> {
        info!("Reading YAML...");

        let mut yaml_data: Yaml = if let Some(yaml) = yaml_file {
            serde_yaml::from_reader(File::open(yaml)?)?
        } else {
            serde_yaml::from_reader(std::io::stdin())?
        };

        info!("\nRead YAML => {:#?}", yaml_data);

        if yaml_data.is_deployment() {
            if let Some(template) = &mut yaml_data.spec.template {
                Self::add_pause_container(&mut template.spec.containers);
            }
        } else {
            Self::add_pause_container(&mut yaml_data.spec.containers);
        }

        Ok(yaml_data)
    }

    pub fn is_deployment(&self) -> bool {
        self.kind.eq("Deployment")
    }

    fn add_pause_container(spec_containers: &mut Option<Vec<Container>>) {
        if let Some(containers) = spec_containers {
            info!("Adding pause container...");
            let pause_container = Container {
                image: "mcr.microsoft.com/oss/kubernetes/pause:3.6".to_string(),
                securityContext: Some(SecurityContext {
                    readOnlyRootFilesystem: true,
                    allowPrivilegeEscalation: Some(false),
                }),
                ..Default::default()
            };
            containers.insert(0, pause_container);
            info!("pause container added.");
        }
    }

    pub fn export_policy(
        &self,
        json_data: &str,
        rules_input_file: &str,
        yaml_file: &Option<String>,
        output_policy_file: &Option<String>,
    ) -> Result<()> {
        info!("Adding policy to YAML");

        let mut policy = read_to_string(&rules_input_file)?;
        policy += "\npolicy_data := ";
        policy += json_data;

        if let Some(file_name) = output_policy_file {
            export_policy_data(&policy, &file_name)?;
        }

        let encoded_policy = general_purpose::STANDARD.encode(policy.as_bytes());

        let mut yaml_data = self.clone();
        if yaml_data.is_deployment() {
            if let Some(template) = &mut yaml_data.spec.template {
                template
                    .metadata
                    .annotations
                    .entry("io.katacontainers.config.agent.policy".to_string())
                    .and_modify(|v| *v = encoded_policy.to_string())
                    .or_insert(encoded_policy.to_string());

                if let Some(containers) = &mut template.spec.containers {
                    // Remove the pause container before serializing.
                    containers.remove(0);
                }
            } else {
                return Err(anyhow!("Deployment spec without pod template!"));
            }
        } else {
            yaml_data
                .metadata
                .annotations
                .entry("io.katacontainers.config.agent.policy".to_string())
                .and_modify(|v| *v = encoded_policy.to_string())
                .or_insert(encoded_policy.to_string());

            if let Some(containers) = &mut yaml_data.spec.containers {
                // Remove the pause container before serializing.
                containers.remove(0);
            }
        }

        if let Some(yaml) = yaml_file {
            serde_yaml::to_writer(
                std::fs::OpenOptions::new()
                    .write(true)
                    .truncate(true)
                    .create(true)
                    .open(yaml)
                    .map_err(|e| anyhow!(e))?,
                &yaml_data,
            )?;
        } else {
            serde_yaml::to_writer(std::io::stdout(), &yaml_data)?;
        }

        Ok(())
    }
}

impl Container {
    pub fn get_env_variables(&self, dest_env: &mut Vec<String>) {
        if let Some(source_env) = &self.env {
            for env_variable in source_env {
                let mut src_string = env_variable.name.clone() + "=";
                src_string += &env_variable.value;
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
}

fn export_policy_data(policy: &str, file_name: &str) -> Result<()> {
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
