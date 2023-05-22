// Copyright (c) 2023 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

// Allow K8s YAML field names.
#![allow(non_snake_case)]

use crate::obj_meta;
use crate::pod;
use crate::pod_template;

use anyhow::{anyhow, Result};
use base64::{engine::general_purpose, Engine as _};
use log::debug;
use serde::{Deserialize, Serialize};
use serde_yaml;
use std::collections::BTreeMap;
use std::fs::{read_to_string, File};
use std::io::Write;

const POLICY_ANNOTATION_KEY: &str = "io.katacontainers.config.agent.policy";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Yaml {
    apiVersion: String,
    kind: String,
    pub metadata: obj_meta::ObjectMeta,
    pub spec: Spec,
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
    pub containers: Option<Vec<pod::Container>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub volumes: Option<Vec<Volume>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    replicas: Option<u32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    selector: Option<Selector>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub template: Option<pod_template::PodTemplate>,
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

impl Yaml {
    pub fn new(yaml_file: &Option<String>) -> Result<Self> {
        debug!("Reading YAML...");

        let mut yaml_data: Yaml = if let Some(yaml) = yaml_file {
            serde_yaml::from_reader(File::open(yaml)?)?
        } else {
            serde_yaml::from_reader(std::io::stdin())?
        };

        debug!("\nRead YAML => {:#?}", yaml_data);

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

    fn add_pause_container(spec_containers: &mut Option<Vec<pod::Container>>) {
        if let Some(containers) = spec_containers {
            debug!("Adding pause container...");
            let pause_container = pod::Container {
                image: "mcr.microsoft.com/oss/kubernetes/pause:3.6".to_string(),
                name: String::new(),
                imagePullPolicy: None,
                securityContext: Some(pod::SecurityContext {
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
            debug!("pause container added.");
        }
    }

    pub fn export_policy(
        &mut self,
        json_data: &str,
        rules_input_file: &str,
        yaml_file: &Option<String>,
        output_policy_file: &Option<String>,
    ) -> Result<()> {
        debug!("============================================");
        debug!("Adding policy to YAML");

        let mut policy = read_to_string(&rules_input_file)?;
        policy += "\npolicy_data := ";
        policy += json_data;
        debug!("Decoded policy length {:?} characters", policy.len());

        if let Some(file_name) = output_policy_file {
            export_decoded_policy(&policy, &file_name)?;
        }

        let encoded_policy = general_purpose::STANDARD.encode(policy.as_bytes());
        debug!(
            "Encoded policy length {:?} characters",
            encoded_policy.len()
        );

        if self.is_deployment() {
            debug!("Adding policy to Deployment YAML");

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
            debug!("Adding policy to Pod YAML");

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
