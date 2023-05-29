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
use crate::yaml;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct List {
    apiVersion: String,
    kind: String,

    items: Vec<ListEntry>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, untagged)]
enum ListEntry {
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
    },
}

#[async_trait]
impl yaml::K8sObject for List {
    fn requires_policy(&self) -> bool {
        false
    }

    fn get_metadata_name(&self) -> Result<String> {
        Err(anyhow!("Unsupported"))?
    }

    fn get_host_name(&self) -> Result<String> {
        Err(anyhow!("Unsupported"))?
    }

    fn get_sandbox_name(&self) -> Result<Option<String>> {
        Err(anyhow!("Unsupported"))?
    }

    fn get_namespace(&self) -> Result<String> {
        Err(anyhow!("Unsupported"))?
    }

    fn add_policy_annotation(&mut self, _encoded_policy: &str) {}

    async fn get_containers_from_registry(&mut self) -> Result<()> {
        Err(anyhow!("Unsupported"))?
    }

    fn get_policy_data(
        &self,
        _k8s_object: &dyn yaml::K8sObject,
        _infra_policy: &infra::InfraPolicy,
        _config_maps: &Vec<config_maps::ConfigMap>,
    ) -> Result<policy::PolicyData> {
        Err(anyhow!("Unsupported"))?
    }

    fn get_container_mounts_and_storages(
        &self,
        _policy_mounts: &mut Vec<oci::Mount>,
        _storages: &mut Vec<policy::SerializedStorage>,
        _container: &pod::Container,
        _infra_policy: &infra::InfraPolicy,
    ) -> Result<()> {
        Err(anyhow!("Unsupported"))?
    }

    fn serialize(&mut self, _file_name: &Option<String>) -> Result<()> {
        Err(anyhow!("Unsupported"))?
    }
}
