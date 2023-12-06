// Copyright (c) 2023 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

// Allow K8s YAML field names.
#![allow(non_snake_case)]

use crate::agent;
use crate::obj_meta;
use crate::persistent_volume_claim;
use crate::pod;
use crate::pod_template;
use crate::policy;
use crate::settings;
use crate::yaml;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

/// See Reference / Kubernetes API / Workload Resources / StatefulSet.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StatefulSet {
    apiVersion: String,
    kind: String,
    metadata: obj_meta::ObjectMeta,
    spec: StatefulSetSpec,

    #[serde(skip)]
    doc_mapping: serde_yaml::Value,
}

/// See Reference / Kubernetes API / Workload Resources / StatefulSet.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct StatefulSetSpec {
    serviceName: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    replicas: Option<i32>,

    selector: yaml::LabelSelector,

    template: pod_template::PodTemplateSpec,

    #[serde(skip_serializing_if = "Option::is_none")]
    volumeClaimTemplates: Option<Vec<persistent_volume_claim::PersistentVolumeClaim>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    updateStrategy: Option<StatefulSetUpdateStrategy>,

    #[serde(skip_serializing_if = "Option::is_none")]
    revisionHistoryLimit: Option<i32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    minReadySeconds: Option<i32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    podManagementPolicy: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    persistentVolumeClaimRetentionPolicy: Option<StatefulSetPersistentVolumeClaimRetentionPolicy>,
}

/// See Reference / Kubernetes API / Workload Resources / StatefulSet.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct StatefulSetPersistentVolumeClaimRetentionPolicy {
    #[serde(skip_serializing_if = "Option::is_none")]
    whenDeleted: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    whenScaled: Option<String>,
}

/// See Reference / Kubernetes API / Workload Resources / StatefulSet.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct StatefulSetUpdateStrategy {
    #[serde(skip_serializing_if = "Option::is_none")]
    r#type: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    rollingUpdate: Option<RollingUpdateStatefulSetStrategy>,
}

/// See Reference / Kubernetes API / Workload Resources / StatefulSet.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct RollingUpdateStatefulSetStrategy {
    #[serde(skip_serializing_if = "Option::is_none")]
    maxUnavailable: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    partition: Option<i32>,
}

#[async_trait]
impl yaml::K8sResource for StatefulSet {
    async fn init(&mut self, use_cache: bool, doc_mapping: &serde_yaml::Value, _silent: bool) {
        yaml::k8s_resource_init(&mut self.spec.template.spec, use_cache).await;
        self.doc_mapping = doc_mapping.clone();
    }

    fn get_sandbox_name(&self) -> Option<String> {
        None
    }

    fn get_namespace(&self) -> String {
        self.metadata.get_namespace()
    }

    fn get_container_mounts_and_storages(
        &self,
        policy_mounts: &mut Vec<policy::KataMount>,
        storages: &mut Vec<agent::Storage>,
        container: &pod::Container,
        settings: &settings::Settings,
    ) {
        if let Some(volumes) = &self.spec.template.spec.volumes {
            yaml::get_container_mounts_and_storages(
                policy_mounts,
                storages,
                container,
                settings,
                volumes,
            );
        }

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
        if let Some(volume_mounts) = &container.volumeMounts {
            if let Some(claims) = &self.spec.volumeClaimTemplates {
                StatefulSet::get_mounts_and_storages(policy_mounts, volume_mounts, claims);
            }
        }
    }

    fn generate_policy(&self, agent_policy: &policy::AgentPolicy) -> String {
        agent_policy.generate_policy(self)
    }

    fn serialize(&mut self, policy: &str) -> String {
        yaml::add_policy_annotation(&mut self.doc_mapping, "spec.template.metadata", policy);
        serde_yaml::to_string(&self.doc_mapping).unwrap()
    }

    fn get_containers(&self) -> &Vec<pod::Container> {
        &self.spec.template.spec.containers
    }

    fn get_annotations(&self) -> &Option<BTreeMap<String, String>> {
        &self.spec.template.metadata.annotations
    }

    fn use_host_network(&self) -> bool {
        if let Some(host_network) = self.spec.template.spec.hostNetwork {
            return host_network;
        }
        false
    }
}

impl StatefulSet {
    fn get_mounts_and_storages(
        policy_mounts: &mut Vec<policy::KataMount>,
        volume_mounts: &Vec<pod::VolumeMount>,
        claims: &Vec<persistent_volume_claim::PersistentVolumeClaim>,
    ) {
        for mount in volume_mounts {
            for claim in claims {
                if let Some(claim_name) = &claim.metadata.name {
                    if claim_name.eq(&mount.name) {
                        let file_name = Path::new(&mount.mountPath)
                            .file_name()
                            .unwrap()
                            .to_str()
                            .unwrap();
                        // TODO:
                        // - Get the source path below from the settings file.
                        // - Generate proper options value.
                        policy_mounts.push(policy::KataMount {
                            destination: mount.mountPath.clone(),
                            type_: "bind".to_string(),
                            source:
                                "^/run/kata-containers/shared/containers/$(bundle-id)-[a-z0-9]{16}-"
                                    .to_string()
                                    + &file_name
                                    + "$",
                            options: vec![
                                "rbind".to_string(),
                                "rprivate".to_string(),
                                "rw".to_string(),
                            ],
                        });
                    }
                }
            }
        }
    }
}
