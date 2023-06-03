// Copyright (c) 2023 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

// Allow K8s YAML field names.
#![allow(non_snake_case)]

use crate::config_map;
use crate::infra;
use crate::obj_meta;
use crate::pause_container;
use crate::persistent_volume_claim;
use crate::pod;
use crate::pod_template;
use crate::policy;
use crate::registry;
use crate::utils;
use crate::yaml;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use base64::{engine::general_purpose, Engine as _};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// See Reference / Kubernetes API / Workload Resources / StatefulSet.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StatefulSet {
    pub apiVersion: String,
    pub kind: String,
    pub metadata: obj_meta::ObjectMeta,
    pub spec: StatefulSetSpec,

    #[serde(skip)]
    pub registry_containers: Vec<registry::Container>,
}

/// See Reference / Kubernetes API / Workload Resources / StatefulSet.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StatefulSetSpec {
    serviceName: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    replicas: Option<i32>,

    selector: yaml::LabelSelector,

    pub template: pod_template::PodTemplateSpec,

    #[serde(skip_serializing_if = "Option::is_none")]
    volumeClaimTemplates: Option<Vec<persistent_volume_claim::PersistentVolumeClaim>>, // TODO: additional fields.
}

#[async_trait]
impl yaml::K8sObject for StatefulSet {
    async fn initialize(&mut self, use_cached_files: bool) -> Result<()> {
        pause_container::add_pause_container(&mut self.spec.template.spec.containers);
        self.registry_containers = registry::get_registry_containers(
            use_cached_files,
            &self.spec.template.spec.containers,
        )
        .await?;
        Ok(())
    }

    fn requires_policy(&self) -> bool {
        true
    }

    fn get_metadata_name(&self) -> Result<String> {
        self.metadata.get_name()
    }

    fn get_host_name(&self) -> Result<String> {
        // Example: "hostname": "no-exist-tdtd7",
        Ok("^".to_string() + &self.get_metadata_name()? + "-[a-z0-9]*$")
    }

    fn get_sandbox_name(&self) -> Result<Option<String>> {
        Ok(None)
    }

    fn get_namespace(&self) -> Result<String> {
        self.metadata.get_namespace()
    }

    fn get_container_mounts_and_storages(
        &self,
        policy_mounts: &mut Vec<oci::Mount>,
        _storages: &mut Vec<policy::SerializedStorage>,
        _container: &pod::Container,
        _infra_policy: &infra::InfraPolicy,
    ) -> Result<()> {
        // Example:
        //
        // containers:
        //   - name: nginx
        //     image: "nginx"
        //     volumeMounts:
        //       - mountPath: /usr/share/nginx/html
        //         name: www
        // ...
        //
        // volumeClaimTemplates:
        //   - metadata:
        //       name: www
        //     spec:
        //       accessModes:
        //         - ReadWriteOnce
        //       resources:
        //         requests:
        //           storage: 1Gi
        for container in &self.spec.template.spec.containers {
            if let Some(volume_mounts) = &container.volumeMounts {
                for mount in volume_mounts {
                    if let Some(claims) = &self.spec.volumeClaimTemplates {
                        for claim in claims {
                            if let Some(claim_name) = &claim.metadata.name {
                                if claim_name.eq(&mount.name) {
                                    if let Some(file_name) = Path::new(&mount.mountPath).file_name()
                                    {
                                        if let Some(file_name) = file_name.to_str() {
                                            // TODO:
                                            // - Get the source path below from the infra module.
                                            // - Generate proper options value.
                                            policy_mounts.push(oci::Mount {
                                                destination: mount.mountPath.clone(),
                                                r#type: "bind".to_string(),
                                                source: "^/run/kata-containers/shared/containers/$(bundle-id)-[a-z0-9]{16}-".to_string() 
                                                    + &file_name + "$",
                                                options: vec!["rbind".to_string(), "rprivate".to_string(), "rw".to_string()],
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    fn generate_policy(
        &mut self,
        rules: &str,
        infra_policy: &infra::InfraPolicy,
        config_maps: &Vec<config_map::ConfigMap>,
        in_out_files: &utils::InOutFiles,
    ) -> Result<()> {
        let mut policy_containers = Vec::new();

        for i in 0..self.spec.template.spec.containers.len() {
            policy_containers.push(policy::get_container_policy(
                self,
                infra_policy,
                config_maps,
                &self.spec.template.spec.containers[i],
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
        self.spec
            .template
            .metadata
            .add_policy_annotation(&encoded_policy);

        self.spec.template.spec.containers.remove(0);
        Ok(())
    }

    fn serialize(&mut self) -> Result<String> {
        Ok(serde_yaml::to_string(&self)?)
    }
}
