// Copyright (c) 2023 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

// Allow K8s YAML field names.
#![allow(non_snake_case)]

use crate::config_maps;
use crate::infra;
use crate::obj_meta;
use crate::pod;
use crate::policy;
use crate::replication_controller;
use crate::service;
use crate::utils;
use crate::yaml;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct List {
    apiVersion: String,
    kind: String,

    items: Vec<Item>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, untagged)]
enum Item {
    Service {
        apiVersion: String,
        kind: String,
        metadata: obj_meta::ObjectMeta,
        spec: service::ServiceSpec,
    },
    ReplicationController {
        apiVersion: String,
        kind: String,
        metadata: obj_meta::ObjectMeta,
        spec: replication_controller::ReplicationControllerSpec,

        #[serde(skip)]
        replication_controller: Option<replication_controller::ReplicationController>,
    },
}

#[async_trait]
impl yaml::K8sObject for List {
    async fn initialize(&mut self) -> Result<()> {
        for item in &mut self.items {
            match item {
                Item::ReplicationController {
                    apiVersion,
                    kind,
                    metadata,
                    spec,
                    replication_controller,
                } => {
                    let mut controller = replication_controller::ReplicationController {
                        apiVersion: apiVersion.clone(),
                        kind: kind.clone(),
                        metadata: metadata.clone(),
                        spec: spec.clone(),
                        registry_containers: Vec::new(),
                    };
                    controller.initialize().await?;
                    *replication_controller = Some(controller);
                }
                _ => {}
            };
        }
        Ok(())
    }

    fn requires_policy(&self) -> bool {
        for item in &self.items {
            match item {
                Item::ReplicationController {
                    apiVersion: _,
                    kind: _,
                    metadata: _,
                    spec: _,
                    replication_controller: _,
                } => {
                    return true;
                }
                _ => {}
            }
        }
        false
    }

    fn get_metadata_name(&self) -> Result<String> {
        Err(anyhow!("Unsupported"))
    }

    fn get_host_name(&self) -> Result<String> {
        Err(anyhow!("Unsupported"))
    }

    fn get_sandbox_name(&self) -> Result<Option<String>> {
        Err(anyhow!("Unsupported"))
    }

    fn get_namespace(&self) -> Result<String> {
        Err(anyhow!("Unsupported"))
    }

    fn get_container_mounts_and_storages(
        &self,
        _policy_mounts: &mut Vec<oci::Mount>,
        _storages: &mut Vec<policy::SerializedStorage>,
        _container: &pod::Container,
        _infra_policy: &infra::InfraPolicy,
    ) -> Result<()> {
        Ok(())
    }

    fn generate_policy(
        &mut self,
        rules: &str,
        infra_policy: &infra::InfraPolicy,
        config_maps: &Vec<config_maps::ConfigMap>,
        in_out_files: &utils::InOutFiles,
    ) -> Result<()> {
        for item in &mut self.items {
            match item {
                Item::ReplicationController {
                    apiVersion: _,
                    kind: _,
                    metadata: _,
                    spec,
                    replication_controller,
                } => {
                    if let Some(controller) = replication_controller {
                        controller.generate_policy(
                            rules,
                            infra_policy,
                            config_maps,
                            in_out_files,
                        )?;
                        // Copy the policy annotation.
                        spec.template.metadata.annotations =
                            controller.spec.template.metadata.annotations.clone();
                    }
                }
                _ => {}
            }
        }

        Ok(())
    }

    fn serialize(&self, in_out_files: &utils::InOutFiles) -> Result<()> {
        if let Some(yaml) = &in_out_files.yaml_file {
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
