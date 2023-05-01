// Copyright (c) 2023 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

// Allow K8s YAML field names.
#![allow(non_snake_case)]

use crate::config_maps;

use anyhow::{anyhow, Result};
use base64::{engine::general_purpose, Engine as _};
use log::info;
use serde::{Deserialize, Serialize};
use serde_yaml;
use std::collections::BTreeMap;
use std::fs::{read_to_string, File};
use std::io::Write;

const POLICY_ANNOTATION_KEY: &str = "io.katacontainers.config.agent.policy";

// Example:
//
// apiVersion: v1
// kind: Pod
// metadata:
// ...
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Metadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    labels: Option<BTreeMap<String, String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    annotations: Option<BTreeMap<String, String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
}

// Example:
//
// spec:
//   restartPolicy: Never
//   runtimeClassName: kata-cc
//   containers:
//   - image: docker.io/library/busybox:1.36.0
// ...
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Spec {
    #[serde(skip_serializing_if = "Option::is_none")]
    restartPolicy: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    runtimeClassName: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub containers: Option<Vec<Container>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub volumes: Option<Vec<Volume>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    replicas: Option<u32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    selector: Option<Selector>,

    #[serde(skip_serializing_if = "Option::is_none")]
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
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Container {
    pub image: String,
    pub name: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub securityContext: Option<SecurityContext>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub volumeMounts: Option<Vec<VolumeMount>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<Vec<EnvVariable>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub resources: Option<Resources>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub ports: Option<Vec<Port>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub args: Option<Vec<String>>,
}

// Example:
//
// securityContext:
//   readOnlyRootFilesystem: true
//   allowPrivilegeEscalation: false
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

// Example:
//
// - mountPath: /busy1
//   name: data
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VolumeMount {
    pub mountPath: String,
    pub name: String,
}

// Example:
//
// - name: my-pod-volume
//   persistentVolumeClaim:
//      claimName: my-volume-claim
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Volume {
    pub name: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub emptyDir: Option<EmptyDirVolume>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub hostPath: Option<HostPathVolume>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub persistentVolumeClaim: Option<VolumeClaimVolume>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub configMap: Option<ConfigMapVolume>,
}

// Example:
//
// hostPath:
//   path: /dev/sev
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostPathVolume {
    pub path: String,
}

// Example:
//
// emptyDir: {}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EmptyDirVolume {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sizeLimit: Option<String>,
}

// Example:
//
// claimName: my-volume-claim
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VolumeClaimVolume {
    pub claimName: String,
}

// Example:
//
// volumes:
// - name: config-volume
//   configMap:
//     name: name-of-your-configmap
//     items:
//     - key: your-file.json
//       path: keys
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigMapVolume {
    pub name: String,
    pub items: Vec<ConfigMapVolumeItem>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigMapVolumeItem {
    pub key: String,
    pub path: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Selector {
    matchLabels: Option<BTreeMap<String, String>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Template {
    pub metadata: Metadata,
    pub spec: TemplateSpec,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemplateSpec {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nodeSelector: Option<BTreeMap<String, String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    runtimeClassName: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub containers: Option<Vec<Container>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnvVariable {
    name: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    value: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub valueFrom: Option<ValueFrom>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Resources {
    #[serde(skip_serializing_if = "Option::is_none")]
    requests: Option<HardwareResources>,

    #[serde(skip_serializing_if = "Option::is_none")]
    limits: Option<HardwareResources>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HardwareResources {
    #[serde(skip_serializing_if = "Option::is_none")]
    cpu: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    memory: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Port {
    containerPort: u16,

    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValueFrom {
    pub configMapKeyRef: ConfigMapKeyRef,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigMapKeyRef {
    pub name: String,
    pub key: String,
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
                name: String::new(),
                securityContext: Some(SecurityContext {
                    readOnlyRootFilesystem: Some(true),
                    allowPrivilegeEscalation: Some(false),
                    privileged: None,
                }),
                volumeMounts: None,
                env: None,
                resources: None,
                ports: None,
                command: None,
                args: None,
            };
            containers.insert(0, pause_container);
            info!("pause container added.");
        }
    }

    pub fn export_policy(
        &mut self,
        json_data: &str,
        rules_input_file: &str,
        yaml_file: &Option<String>,
        output_policy_file: &Option<String>,
    ) -> Result<()> {
        info!("============================================");
        info!("Adding policy to YAML");

        let mut policy = read_to_string(&rules_input_file)?;
        policy += "\npolicy_data := ";
        policy += json_data;
        info!("Decoded policy length {:?} characters", policy.len());

        if let Some(file_name) = output_policy_file {
            export_decoded_policy(&policy, &file_name)?;
        }

        let encoded_policy = general_purpose::STANDARD.encode(policy.as_bytes());
        info!("Encoded policy length {:?} characters", encoded_policy.len());

        if self.is_deployment() {
            info!("Adding policy to Deployment YAML");

            if let Some(template) = &mut self.spec.template {
                Self::add_policy_annotation(&mut template.metadata.annotations, &encoded_policy);

                if let Some(containers) = &mut template.spec.containers {
                    // Remove the pause container before serializing.
                    containers.remove(0);
                }
            } else {
                return Err(anyhow!("Deployment YAML without pod template!"));
            }
        } else {
            info!("Adding policy to Pod YAML");

            Self::add_policy_annotation(&mut self.metadata.annotations, &encoded_policy);

            if let Some(containers) = &mut self.spec.containers {
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
                &self,
            )?;
        } else {
            serde_yaml::to_writer(std::io::stdout(), &self)?;
        }

        Ok(())
    }

    fn add_policy_annotation(anno: &mut Option<BTreeMap<String, String>>, encoded_policy: &str) {
        if let Some(annotations) = anno {
            annotations
                .entry(POLICY_ANNOTATION_KEY.to_string())
                .and_modify(|v| *v = encoded_policy.to_string())
                .or_insert(encoded_policy.to_string());
        } else {
            let mut annotations = BTreeMap::new();
            annotations.insert(
                POLICY_ANNOTATION_KEY.to_string(),
                encoded_policy.to_string(),
            );
            *anno = Some(annotations);
        }
    }
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

fn export_decoded_policy(policy: &str, file_name: &str) -> Result<()> {
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

impl EnvVariable {
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
