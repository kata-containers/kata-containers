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
use crate::utils;
use crate::yaml;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// See Service in the Kubernetes API reference.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Service {
    apiVersion: String,
    kind: String,
    metadata: obj_meta::ObjectMeta,
    spec: ServiceSpec,
}

/// See ServiceSpec in the Kubernetes API reference.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServiceSpec {
    #[serde(skip_serializing_if = "Option::is_none")]
    selector: Option<BTreeMap<String, String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    ports: Option<Vec<ServicePort>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    r#type: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    ipFamilies: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    ipFamilyPolicy: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    clusterIP: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    clusterIPs: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    externalIPs: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    sessionAffinity: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    loadBalancerIP: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    loadBalancerSourceRanges: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    loadBalancerClass: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    externalName: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    externalTrafficPolicy: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    internalTrafficPolicy: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    healthCheckNodePort: Option<i32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    publishNotReadyAddresses: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    sessionAffinityConfig: Option<SessionAffinityConfig>,

    #[serde(skip_serializing_if = "Option::is_none")]
    allocateLoadBalancerNodePorts: Option<bool>,
}

/// See ServicePort in the Kubernetes API reference.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServicePort {
    port: i32,

    #[serde(skip_serializing_if = "Option::is_none")]
    targetPort: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    protocol: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    nodePort: Option<i32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    appProtocol: Option<String>,
}

/// See SessionAffinityConfig in the Kubernetes API reference.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionAffinityConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    clientIP: Option<ClientIPConfig>,
}

/// See ClientIPConfig in the Kubernetes API reference.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClientIPConfig {
    timeoutSeconds: i32,
}

#[async_trait]
impl yaml::K8sObject for Service {
    async fn initialize(&mut self) -> Result<()> {
        Ok(())
    }

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

    fn get_container_mounts_and_storages(
        &self,
        _policy_mounts: &mut Vec<oci::Mount>,
        _storages: &mut Vec<policy::SerializedStorage>,
        _container: &pod::Container,
        _infra_policy: &infra::InfraPolicy,
    ) -> Result<()> {
        Err(anyhow!("Unsupported"))?
    }

    fn generate_policy(
        &mut self,
        _rules: &str,
        _infra_policy: &infra::InfraPolicy,
        _config_maps: &Vec<config_maps::ConfigMap>,
        _in_out_files: &utils::InOutFiles,
    ) -> Result<()> {
        Err(anyhow!("Unsupported"))
    }

    fn serialize(&self, _in_out_files: &utils::InOutFiles) -> Result<()> {
        Err(anyhow!("Unsupported"))
    }
}
