// Copyright (c) 2023 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

// Allow K8s YAML field names.
#![allow(non_snake_case)]

use crate::infra;
use crate::obj_meta;
use crate::persistent_volume_claim;
use crate::pod;
use crate::pod_template;
use crate::policy;
use crate::registry;
use crate::yaml;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// See Reference / Kubernetes API / Workload Resources / StatefulSet.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StatefulSet {
    pub apiVersion: String,
    pub kind: String,
    pub metadata: obj_meta::ObjectMeta,
    pub spec: StatefulSetSpec,

    #[serde(skip)]
    doc_mapping: serde_yaml::Value,

    #[serde(skip)]
    pub registry_containers: Vec<registry::Container>,
}

/// See Reference / Kubernetes API / Workload Resources / StatefulSet.
#[derive(Clone, Debug, Serialize, Deserialize)]
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
impl yaml::K8sResource for StatefulSet {
    async fn init(
        &mut self,
        use_cache: bool,
        doc_mapping: &serde_yaml::Value,
        _silent_unsupported_fields: bool,
    ) -> anyhow::Result<()> {
        yaml::k8s_resource_init(
            &mut self.spec.template.spec,
            &mut self.registry_containers,
            use_cache,
        )
        .await?;
        self.doc_mapping = doc_mapping.clone();
        Ok(())
    }

    fn get_metadata_name(&self) -> String {
        self.metadata.get_name()
    }

    fn get_host_name(&self) -> String {
        // Example: "hostname": "no-exist-tdtd7",
        "^".to_string() + &self.get_metadata_name() + "-[a-z0-9]*$"
    }

    fn get_sandbox_name(&self) -> Option<String> {
        None
    }

    fn get_namespace(&self) -> String {
        self.metadata.get_namespace()
    }

    fn get_container_mounts_and_storages(
        &self,
        policy_mounts: &mut Vec<oci::Mount>,
        _storages: &mut Vec<policy::SerializedStorage>,
        _container: &pod::Container,
        _infra_policy: &infra::InfraPolicy,
    ) {
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
                                    let file_name = Path::new(&mount.mountPath)
                                        .file_name()
                                        .unwrap()
                                        .to_str()
                                        .unwrap();
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

    fn generate_policy(&self, agent_policy: &policy::AgentPolicy) -> String {
        yaml::generate_policy(self, agent_policy)
    }

    fn serialize(&mut self, policy: &str) -> String {
        yaml::add_policy_annotation(&mut self.doc_mapping, "spec.template.metadata", policy);
        serde_yaml::to_string(&self.doc_mapping).unwrap()
    }

    fn get_containers(&self) -> (&Vec<registry::Container>, &Vec<pod::Container>) {
        (
            &self.registry_containers,
            &self.spec.template.spec.containers,
        )
    }
}
